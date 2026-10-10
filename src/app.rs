mod tabs;
use crate::{
    media,
    model::{Canvas, History, Item},
    storage::{self, Settings},
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
pub use tabs::Workspace;

use crate::theme::{self, ACCENT};

fn native_drop_position(ctx: &egui::Context) -> Option<Pos2> {
    // OLE file drags can suppress ordinary pointer events. Sample the drop's actual screen point.
    #[cfg(all(windows, not(test)))]
    {
        use windows_sys::{Win32::Foundation::POINT, Win32::UI::WindowsAndMessaging::GetCursorPos};
        let mut point = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut point) } != 0
            && let Some(inner) = ctx.input(|i| i.viewport().inner_rect)
        {
            let scale = ctx.pixels_per_point();
            return Some(Pos2::new(
                point.x as f32 / scale - inner.min.x,
                point.y as f32 / scale - inner.min.y,
            ));
        }
    }
    let _ = ctx;
    None
}
enum AfterSave {
    None,
    Close,
    Open(PathBuf),
    New,
}
struct SaveJob {
    rx: mpsc::Receiver<Result<(), String>>,
    path: PathBuf,
    revision: u64,
    named: bool,
}
struct ImportJob {
    rx: mpsc::Receiver<crate::importer::Batch>,
    cancel: Arc<AtomicBool>,
    found: Arc<AtomicUsize>,
    anchor: [f64; 2],
}
struct Preview {
    texture: Option<egui::TextureHandle>,
    side: u32,
    pending: bool,
    next: Option<Instant>,
    touched: Instant,
    error: Option<String>,
}
enum Drag {
    Move {
        start: [f64; 2],
        before: Vec<Item>,
    },
    Resize {
        id: u64,
        anchor: [f64; 2],
        before: Vec<Item>,
        original: [f64; 2],
    },
    Select {
        start: Pos2,
        additive: bool,
        previous: HashSet<u64>,
    },
}

pub struct App {
    tab_mode: bool,
    external_blocked: bool,
    tab_action: Option<tabs::Action>,
    image_menu: Option<u64>,
    tab_headers: Vec<tabs::Header>,
    active_tab: usize,
    tab_scroll_to_active: bool,
    top_ui_height: f32,
    recovery: PathBuf,
    reserved_paths: Vec<PathBuf>,
    canvas: Canvas,
    history: History,
    selected: HashSet<u64>,
    directory: PathBuf,
    current: Option<PathBuf>,
    revision: u64,
    saved_revision: u64,
    saved_once: bool,
    timer: Instant,
    job: Option<SaveJob>,
    after: AfterSave,
    allow_close: bool,
    previews: HashMap<PathBuf, Preview>,
    decoder: media::Decoder,
    drag: Option<Drag>,
    status: String,
    error: Option<String>,
    settings_open: bool,
    language: crate::i18n::Language,
    fullscreen: bool,
    paste_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    canvas_rect: Rect,
    open_job: Option<mpsc::Receiver<Result<Canvas, String>>>,
    opening_path: Option<PathBuf>,
    opening_unnamed: bool,
    random_order: bool,
    import_job: Option<ImportJob>,
    pending_import: Option<(crate::importer::Batch, [f64; 2])>,
}

impl App {
    #[cfg(test)]
    fn initialize(ctx: egui::Context, directory: PathBuf, initial: Option<PathBuf>) -> Self {
        Self::initialize_mode(ctx, directory, initial, true)
    }
    fn initialize_mode(
        ctx: egui::Context,
        directory: PathBuf,
        initial: Option<PathBuf>,
        restore: bool,
    ) -> Self {
        ctx.set_theme(egui::Theme::Dark);
        ctx.set_visuals(egui::Visuals::dark());
        let mut fonts = egui::FontDefinitions::default();
        for path in ["C:/Windows/Fonts/msjh.ttc", "C:/Windows/Fonts/mingliu.ttc"] {
            if let Ok(bytes) = std::fs::read(path) {
                fonts
                    .font_data
                    .insert("chinese".into(), egui::FontData::from_owned(bytes).into());
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .push("chinese".into());
                break;
            }
        }
        ctx.set_fonts(fonts);
        theme::configure(&ctx);
        let settings = storage::read_settings(&directory);
        let mut app = Self {
            tab_mode: !restore,
            external_blocked: false,
            tab_action: None,
            image_menu: None,
            tab_headers: Vec::new(),
            active_tab: 0,
            tab_scroll_to_active: true,
            top_ui_height: 100.0,
            recovery: directory.join("previous_canvas.icanvas"),
            reserved_paths: Vec::new(),
            canvas: Canvas {
                packed: settings.packed,
                ..Default::default()
            },
            history: History::default(),
            selected: HashSet::new(),
            directory,
            current: None,
            revision: 0,
            saved_revision: 0,
            saved_once: false,
            timer: Instant::now(),
            job: None,
            after: AfterSave::None,
            allow_close: false,
            previews: HashMap::new(),
            decoder: media::Decoder::new(ctx.clone()),
            drag: None,
            status: settings.language.text("將圖片拖入畫布，開始整理").into(),
            error: None,
            settings_open: false,
            language: settings.language,
            fullscreen: false,
            paste_requested: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            canvas_rect: Rect::NOTHING,
            open_job: None,
            opening_path: None,
            opening_unnamed: false,
            random_order: settings.random_order,
            import_job: None,
            pending_import: None,
        };
        let restoring_unnamed = initial.is_none() && settings.unnamed;
        let path = if restore {
            initial.or(settings.last).filter(|p| p.exists())
        } else {
            None
        };
        if let Some(path) = path {
            app.begin_open(path);
            app.opening_unnamed = restoring_unnamed;
        }
        app
    }
    fn change_language(&mut self, language: crate::i18n::Language) {
        if self.job.is_some() || self.language == language {
            return;
        }
        let mut settings = storage::read_settings(&self.directory);
        settings.language = language;
        match storage::write_settings(&self.directory, &settings) {
            Ok(()) => {
                self.language = language;
                self.status = language.text("將圖片拖入畫布，開始整理").into();
            }
            Err(error) => self.error = Some(self.language.error(&error)),
        }
    }
    fn changed(&mut self) {
        self.revision += 1;
    }
    fn dirty(&self) -> bool {
        self.revision != self.saved_revision
    }
    fn message(&mut self, result: Result<(), String>) {
        if let Err(e) = result {
            self.error = Some(self.language.error(&e));
        }
    }
    fn date_path(&self) -> PathBuf {
        let date = chrono::Local::now().format("%Y%d%m").to_string();
        if !self.tab_mode {
            return storage::next_name(&self.directory, &date);
        }
        for number in 1u64.. {
            let path = self
                .directory
                .join(format!("canvas_{date}_{number:02}.icanvas"));
            if !path.exists()
                && !self
                    .reserved_paths
                    .iter()
                    .any(|other| tabs::same_path(other, &path))
            {
                return path;
            }
        }
        unreachable!()
    }
    fn save(&mut self, manual: bool, alternate: Option<PathBuf>) {
        if self.job.is_some() || self.open_job.is_some() {
            return;
        }
        let named = alternate.is_some() || self.current.is_some() || manual;
        let path = alternate.unwrap_or_else(|| {
            self.current.clone().unwrap_or_else(|| {
                if manual {
                    self.date_path()
                } else {
                    self.recovery.clone()
                }
            })
        });
        if self
            .reserved_paths
            .iter()
            .any(|other| tabs::same_path(other, &path))
        {
            self.error = Some(
                self.language
                    .text("此檔案已在其他分頁使用，請另選保存位置。")
                    .into(),
            );
            return;
        }
        let managed = self.tab_mode;
        let canvas = self.canvas.clone();
        let output = path.clone();
        let directory = self.directory.clone();
        let random_order = self.random_order;
        let language = self.language;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = storage::save(&canvas, &output).and_then(|()| {
                if managed {
                    return Ok(());
                }
                storage::write_settings(
                    &directory,
                    &Settings {
                        packed: canvas.packed,
                        last: Some(output),
                        unnamed: !named,
                        random_order,
                        language,
                        ..Default::default()
                    },
                )
            });
            let _ = tx.send(result);
        });
        self.job = Some(SaveJob {
            rx,
            path,
            revision: self.revision,
            named,
        });
        self.status = self.language.text("正在保存…").into();
    }
    fn begin_open(&mut self, path: PathBuf) {
        self.opening_unnamed = false;
        let (tx, rx) = mpsc::channel();
        let input = path.clone();
        let assets = self.directory.join("assets");
        std::thread::spawn(move || {
            let _ = tx.send(storage::load(&input, &assets));
        });
        self.open_job = Some(rx);
        self.opening_path = Some(path);
        self.status = self.language.text("正在開啟版面…").into();
    }
    fn transition(&mut self, after: AfterSave) {
        if self.tab_mode {
            match after {
                AfterSave::New => {
                    self.tab_action = Some(tabs::Action::New);
                    return;
                }
                AfterSave::Open(path) => {
                    self.tab_action = Some(tabs::Action::Open(path));
                    return;
                }
                _ => {}
            }
        }
        self.finish_drag();
        self.after = after;
        if self.job.is_some() || self.dirty() || !self.saved_once {
            self.save(false, None);
        } else {
            self.perform_after();
        }
    }
    fn perform_after(&mut self) {
        match std::mem::replace(&mut self.after, AfterSave::None) {
            AfterSave::None => {}
            AfterSave::Close => self.allow_close = true,
            AfterSave::Open(path) => self.begin_open(path),
            AfterSave::New => {
                self.canvas = Canvas {
                    packed: self.canvas.packed,
                    ..Default::default()
                };
                self.current = None;
                self.history = History::default();
                self.selected.clear();
                self.previews.clear();
                self.changed();
                self.saved_once = false;
                self.status = self.language.text("新建未命名版面").into();
            }
        }
    }
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(job) = &self.import_job {
            match job.rx.try_recv() {
                Ok(batch) => {
                    let anchor = job.anchor;
                    self.import_job = None;
                    self.receive_import(batch, anchor);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.import_job = None;
                    self.error = Some(
                        self.language
                            .text("圖片掃描工作意外中止；尚未加入圖片")
                            .into(),
                    );
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(job) = &self.job {
            let outcome = match job.rx.try_recv() {
                Ok(r) => Some(r),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err(self.language.text("保存工作意外中止，請重試").into()))
                }
                _ => None,
            };
            if let Some(result) = outcome {
                let job = self.job.take().unwrap();
                match result {
                    Ok(()) => {
                        self.saved_revision = job.revision;
                        self.saved_once = true;
                        if job.named {
                            self.current = Some(job.path.clone());
                        }
                        self.status = crate::localized!(
                            self.language,
                            "已保存 · {}",
                            "Saved · {}",
                            job.path.display()
                        );
                        if !matches!(self.after, AfterSave::None) {
                            if self.dirty() {
                                self.save(false, None);
                            } else {
                                self.perform_after();
                            }
                        }
                    }
                    Err(e) => {
                        self.after = AfterSave::None;
                        let e = self.language.error(&e);
                        self.error = Some(crate::localized!(
                            self.language,
                            "保存失敗：{e}\n內容仍保留在畫布中。",
                            "Save failed: {e}\nYour changes are still on the canvas."
                        ));
                    }
                }
            }
        }
        if let Some(rx) = &self.open_job {
            let outcome = match rx.try_recv() {
                Ok(r) => Some(r),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err(self.language.text("載入工作意外中止").into()))
                }
                _ => None,
            };
            if let Some(result) = outcome {
                self.open_job = None;
                let path = self.opening_path.take().unwrap();
                match result {
                    Ok(canvas) => {
                        self.canvas = canvas;
                        self.current = if self.opening_unnamed {
                            None
                        } else {
                            Some(path.clone())
                        };
                        self.changed();
                        self.saved_revision = self.revision;
                        self.saved_once = true;
                        self.history = History::default();
                        self.selected.clear();
                        self.previews.clear();
                        self.status = crate::localized!(
                            self.language,
                            "已開啟 · {}",
                            "Opened · {}",
                            path.display()
                        );
                        if !self.tab_mode {
                            self.message(storage::write_settings(
                                &self.directory,
                                &Settings {
                                    packed: self.canvas.packed,
                                    last: Some(path),
                                    unnamed: self.current.is_none(),
                                    random_order: self.random_order,
                                    language: self.language,
                                    ..Default::default()
                                },
                            ));
                        }
                    }
                    Err(e) => {
                        let e = self.language.error(&e);
                        self.error = Some(crate::localized!(
                            self.language,
                            "無法開啟版面：{e}",
                            "Could not open canvas: {e}"
                        ));
                    }
                }
            }
        }
        while let Ok(decoded) = self.decoder.rx.try_recv() {
            if let Some(preview) = self.previews.get_mut(&decoded.path) {
                preview.pending = false;
                match decoded.result {
                    Ok((pixels, delay)) => {
                        let color = egui::ColorImage::from_rgba_unmultiplied(
                            [pixels.width() as usize, pixels.height() as usize],
                            pixels.as_raw(),
                        );
                        if let Some(texture) = &mut preview.texture {
                            texture.set(color, egui::TextureOptions::LINEAR);
                        } else {
                            preview.texture = Some(ctx.load_texture(
                                decoded.path.to_string_lossy(),
                                color,
                                egui::TextureOptions::LINEAR,
                            ));
                        }
                        preview.side = decoded.side;
                        preview.next = delay.map(|d| Instant::now() + d);
                        preview.error = None;
                    }
                    Err(e) => {
                        preview.error = Some(e);
                        preview.next = None;
                    }
                }
            }
        }
        if self.timer.elapsed() >= Duration::from_secs(60) {
            self.timer = Instant::now();
            if self.dirty() && self.error.is_none() {
                self.save(false, None);
            }
        }
        ctx.request_repaint_after(Duration::from_millis(33));
    }
    fn import_busy(&self) -> bool {
        self.import_job.is_some() || self.pending_import.is_some()
    }
    fn start_import(&mut self, paths: Vec<PathBuf>, at: Option<Pos2>) {
        if self.open_job.is_some() || self.import_busy() {
            return;
        }
        let anchor = at.map(|p| self.world(p)).unwrap_or(self.canvas.center);
        let cancel = Arc::new(AtomicBool::new(false));
        let found = Arc::new(AtomicUsize::new(0));
        let worker_cancel = cancel.clone();
        let worker_found = found.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::importer::scan(
                paths,
                true,
                worker_cancel,
                worker_found,
            ));
        });
        self.import_job = Some(ImportJob {
            rx,
            cancel,
            found,
            anchor,
        });
        self.status = self.language.text("正在掃描圖片與子資料夾…").into();
    }
    #[cfg(test)]
    fn import(&mut self, paths: Vec<PathBuf>, at: Option<Pos2>) {
        let anchor = at.map(|p| self.world(p)).unwrap_or(self.canvas.center);
        let batch = crate::importer::scan(
            paths,
            true,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicUsize::new(0)),
        );
        self.receive_import(batch, anchor);
    }
    fn receive_import(&mut self, batch: crate::importer::Batch, anchor: [f64; 2]) {
        if batch.images.len() > 100 {
            self.pending_import = Some((batch, anchor));
            self.status = self.language.text("請確認本批匯入圖片數量").into();
        } else {
            self.apply_import(batch, anchor);
        }
    }
    fn confirm_import(&mut self) {
        if let Some((batch, anchor)) = self.pending_import.take() {
            self.apply_import(batch, anchor);
        }
    }
    fn cancel_import(&mut self) {
        if let Some(job) = self.import_job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.pending_import = None;
        self.status = self.language.text("已取消匯入，畫布保持原狀").into();
    }
    fn apply_import(&mut self, mut prepared: crate::importer::Batch, position: [f64; 2]) {
        let count = prepared.images.len();
        if count > 0 {
            let before = self.canvas.items.clone();
            self.selected.clear();
            if self.random_order {
                crate::model::shuffle_batch(&mut prepared.images);
            }
            let sizes = prepared.images.iter().map(|i| i.size).collect::<Vec<_>>();
            let positions = crate::model::arrange_batch(&sizes, position);
            let first_id = self.canvas.items.iter().map(|i| i.id).max().unwrap_or(0) + 1;
            for (n, (image, position)) in prepared.images.into_iter().zip(positions).enumerate() {
                let id = first_id + n as u64;
                self.canvas.items.push(Item {
                    id,
                    source: image.path,
                    position,
                    size: image.size,
                });
                self.selected.insert(id);
            }
            self.history.commit(before, &self.canvas.items);
            self.changed();
            self.status = crate::localized!(
                self.language,
                "已匯入 {count} 張圖片",
                "Imported {count} images"
            );
        } else {
            self.status = self.language.text("資料夾中沒有可匯入的圖片").into();
        }
        if !prepared.warnings.is_empty() {
            self.error = Some(crate::localized!(
                self.language,
                "{} 個檔案／資料夾無法載入：\n{}",
                "{} files/folders could not be loaded:\n{}",
                prepared.warnings.len(),
                prepared
                    .warnings
                    .into_iter()
                    .take(12)
                    .map(|warning| self.language.error(&warning))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
    }
    fn paste(&mut self) {
        let result = (|| {
            let mut clipboard = arboard::Clipboard::new().map_err(storage::error)?;
            let pixels = clipboard.get_image().map_err(|_| {
                self.language
                    .text("剪貼簿內沒有圖片，請先複製圖片或截圖")
                    .to_string()
            })?;
            self.import_clipboard_pixels(&pixels)
        })();
        self.message(result);
    }
    fn import_clipboard_pixels(&mut self, pixels: &arboard::ImageData<'_>) -> Result<(), String> {
        let path = self
            .current
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or(&self.directory)
            .join("assets")
            .join(format!("clipboard-{}.png", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(path.parent().unwrap()).map_err(storage::error)?;
        image::save_buffer_with_format(
            &path,
            &pixels.bytes,
            pixels.width as u32,
            pixels.height as u32,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(storage::error)?;
        self.start_import(vec![path], None);
        Ok(())
    }
    fn pick_images(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter(
                self.language.text("圖片"),
                &["png", "jpg", "jpeg", "webp", "bmp", "gif"],
            )
            .pick_files()
        {
            self.start_import(paths, None);
        }
    }
    fn pick_folder(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text("匯入資料夾（包含所有子資料夾）"))
            .pick_folder()
        {
            self.start_import(vec![path], None);
        }
    }
    fn pick_open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.directory)
            .add_filter(self.language.text("圖片版面"), &["icanvas"])
            .pick_file()
        {
            self.transition(AfterSave::Open(path));
        }
    }
    fn alternate_save(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.directory)
            .set_file_name(self.date_path().file_name().unwrap().to_string_lossy())
            .add_filter(self.language.text("圖片版面"), &["icanvas"])
            .save_file()
        {
            self.save(true, Some(path));
        }
    }
    fn delete(&mut self) {
        let before = self.canvas.items.clone();
        self.canvas.items.retain(|i| !self.selected.contains(&i.id));
        if before != self.canvas.items {
            self.history.commit(before, &self.canvas.items);
            self.selected.clear();
            self.changed();
        }
    }
    fn layer(&mut self, front: bool) {
        let before = self.canvas.items.clone();
        self.canvas.items.sort_by_key(|i| {
            if front {
                self.selected.contains(&i.id)
            } else {
                !self.selected.contains(&i.id)
            }
        });
        if before != self.canvas.items {
            self.history.commit(before, &self.canvas.items);
            self.changed();
        }
    }
    fn undo(&mut self) {
        if self.history.undo(&mut self.canvas) {
            self.selected.clear();
            self.changed();
        }
    }
    fn redo(&mut self) {
        if self.history.redo(&mut self.canvas) {
            self.selected.clear();
            self.changed();
        }
    }
    fn fit(&mut self) {
        if self.canvas.items.is_empty() {
            self.canvas.center = [0.0; 2];
            self.canvas.zoom = 1.0;
        } else {
            let mut min = [f64::INFINITY; 2];
            let mut max = [f64::NEG_INFINITY; 2];
            for item in &self.canvas.items {
                for a in 0..2 {
                    min[a] = min[a].min(item.position[a]);
                    max[a] = max[a].max(item.position[a] + item.size[a]);
                }
            }
            self.canvas.center = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
            self.canvas.zoom = ((self.canvas_rect.width() as f64 - 100.0) / (max[0] - min[0]))
                .min((self.canvas_rect.height() as f64 - 100.0) / (max[1] - min[1]))
                .clamp(0.00001, 10000.0);
        }
        self.changed();
    }
    fn world(&self, p: Pos2) -> [f64; 2] {
        self.canvas.world(
            [p.x as f64, p.y as f64],
            [
                self.canvas_rect.center().x as f64,
                self.canvas_rect.center().y as f64,
            ],
        )
    }
    fn rect(&self, item: &Item) -> Rect {
        let p = self.canvas.screen(
            item.position,
            [
                self.canvas_rect.center().x as f64,
                self.canvas_rect.center().y as f64,
            ],
        );
        Rect::from_min_size(
            Pos2::new(p[0] as f32, p[1] as f32),
            Vec2::new(
                (item.size[0] * self.canvas.zoom) as f32,
                (item.size[1] * self.canvas.zoom) as f32,
            ),
        )
    }
    fn finish_drag(&mut self) {
        if let Some(Drag::Move { before, .. } | Drag::Resize { before, .. }) = self.drag.take() {
            self.history.commit(before, &self.canvas.items);
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        // Preserve key presses even when both key events occur between rendered frames.
        let paste_pressed = self.paste_requested.swap(false, Ordering::Relaxed);
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            self.toggle_fullscreen(ctx);
        }
        if self.external_blocked
            || ctx.wants_keyboard_input()
            || self.open_job.is_some()
            || self.error.is_some()
            || self.settings_open
            || self.import_busy()
            || !matches!(self.after, AfterSave::None)
        {
            return;
        }
        let (ctrl, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
        let paste_event =
            ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))));
        if paste_pressed || paste_event {
            self.paste();
        }
        if ctrl {
            if ctx.input(|i| i.key_pressed(egui::Key::S)) {
                if shift {
                    self.alternate_save();
                } else {
                    self.save(true, None);
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::O)) {
                self.pick_open();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N)) {
                self.transition(AfterSave::New);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::A)) {
                self.selected = self.canvas.items.iter().map(|i| i.id).collect();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Z)) {
                if shift {
                    self.redo();
                } else {
                    self.undo();
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Y)) {
                self.redo();
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.delete();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
            self.fit();
        }
    }
    fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }
    fn toolbar_visibility(&self, ctx: &egui::Context) -> (bool, bool) {
        if !self.fullscreen {
            return (true, true);
        }
        let screen = ctx.screen_rect();
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let top = pointer.is_some_and(|p| p.y <= screen.top() + self.top_ui_height)
            || ctx.memory(|m| m.any_popup_open());
        let bottom = pointer.is_some_and(|p| p.y >= screen.bottom() - 32.0);
        (top, bottom)
    }
    fn workspace(&mut self, ctx: &egui::Context) {
        if self.fullscreen {
            // The canvas keeps the entire viewport while edge controls float above it.
            let mut ui = egui::Ui::new(
                ctx.clone(),
                egui::Id::new("fullscreen_canvas"),
                egui::UiBuilder::new()
                    .max_rect(ctx.screen_rect())
                    .layer_id(egui::LayerId::background()),
            );
            self.canvas_ui(&mut ui, ctx);
            self.toolbar(ctx);
        } else {
            self.toolbar(ctx);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| self.canvas_ui(ui, ctx));
        }
    }
    fn toolbar(&mut self, ctx: &egui::Context) {
        let ready = !self.external_blocked
            && self.open_job.is_none()
            && !self.import_busy()
            && matches!(self.after, AfterSave::None)
            && self.error.is_none();
        let (show_top, show_bottom) = self.toolbar_visibility(ctx);
        if show_top {
            egui::TopBottomPanel::top("toolbar")
                .frame(
                    egui::Frame::NONE
                        .fill(theme::PANEL)
                        .inner_margin(egui::Margin::symmetric(16, 10)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        theme::logo(ui);
                        if ui.available_width() > 830.0 {
                            ui.label(
                                egui::RichText::new(self.language.text("無限圖片畫布"))
                                    .strong()
                                    .size(16.0),
                            );
                            ui.add_space(12.0);
                        }
                        ui.add_enabled_ui(ready, |ui| {
                            ui.menu_button(self.language.text("檔案"), |ui| {
                                if ui.button(self.language.text("新建版面")).clicked() {
                                    self.transition(AfterSave::New);
                                    ui.close_menu();
                                }
                                if ui.button(self.language.text("開啟版面…")).clicked() {
                                    self.pick_open();
                                    ui.close_menu();
                                }
                                if ui
                                    .add_enabled(
                                        self.job.is_none(),
                                        egui::Button::new(self.language.text("另存新檔…")),
                                    )
                                    .clicked()
                                {
                                    self.alternate_save();
                                    ui.close_menu();
                                }
                            });
                            ui.add_enabled_ui(ready, |ui| {
                                ui.menu_button(self.language.text("編輯"), |ui| {
                                    if ui.button(self.language.text("復原    Ctrl+Z")).clicked() {
                                        self.undo();
                                        ui.close_menu();
                                    }
                                    if ui.button(self.language.text("重做    Ctrl+Y")).clicked() {
                                        self.redo();
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    ui.add_enabled_ui(!self.selected.is_empty(), |ui| {
                                        if ui.button(self.language.text("移到最前")).clicked() {
                                            self.layer(true);
                                            ui.close_menu();
                                        }
                                        if ui.button(self.language.text("移到最後")).clicked() {
                                            self.layer(false);
                                            ui.close_menu();
                                        }
                                        if ui.button(self.language.text("刪除選取圖片")).clicked()
                                        {
                                            self.delete();
                                            ui.close_menu();
                                        }
                                    });
                                });
                            });
                            ui.separator();
                            if ui.button(self.language.text("匯入圖片")).clicked() {
                                self.pick_images();
                            }
                            if ui
                                .button(self.language.text("資料夾"))
                                .on_hover_text(
                                    self.language.text("匯入資料夾及所有子資料夾中的圖片"),
                                )
                                .clicked()
                            {
                                self.pick_folder();
                            }
                            if ui
                                .button(self.language.text("貼上"))
                                .on_hover_text(self.language.text("貼上剪貼簿圖片 · Ctrl+V"))
                                .clicked()
                            {
                                self.paste();
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(if self.fullscreen {
                                    self.language.text("退出全螢幕")
                                } else {
                                    self.language.text("全螢幕")
                                })
                                .on_hover_text("F11")
                                .clicked()
                            {
                                self.toggle_fullscreen(ctx);
                            }
                            ui.add_enabled_ui(ready, |ui| {
                                if ui.button(self.language.text("設定")).clicked() {
                                    self.settings_open = true;
                                }
                                if ui
                                    .add_enabled(
                                        self.job.is_none(),
                                        theme::primary(self.language.text("保存")),
                                    )
                                    .on_hover_text(self.language.text("保存版面 · Ctrl+S"))
                                    .clicked()
                                {
                                    self.save(true, None);
                                }
                            });
                        });
                    });
                });
            let document_bar = egui::TopBottomPanel::top("document_bar")
                .frame(
                    egui::Frame::NONE
                        .fill(theme::BACKGROUND)
                        .inner_margin(egui::Margin::symmetric(16, 6)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_height(32.0);
                        if self.tab_mode {
                            let width = (ui.available_width() - 215.0).max(160.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(width, 32.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    egui::ScrollArea::horizontal().id_salt("tab_strip").show(
                                        ui,
                                        |ui| {
                                            ui.horizontal(|ui| {
                                                for (index, header) in
                                                    self.tab_headers.iter().enumerate()
                                                {
                                                    let (response, action) = tabs::tab_button(
                                                        ui,
                                                        index,
                                                        header,
                                                        index == self.active_tab,
                                                        self.language,
                                                    );
                                                    if index == self.active_tab
                                                        && self.tab_scroll_to_active
                                                    {
                                                        response.scroll_to_me(Some(
                                                            egui::Align::Center,
                                                        ));
                                                    }
                                                    if action.is_some() {
                                                        self.tab_action = action;
                                                    }
                                                }
                                                if ui
                                                    .button("+")
                                                    .on_hover_text(self.language.text("新建分頁"))
                                                    .clicked()
                                                {
                                                    self.tab_action = Some(tabs::Action::New);
                                                }
                                            });
                                        },
                                    );
                                },
                            );
                            self.tab_scroll_to_active = false;
                        } else {
                            let name = self
                                .current
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or(self.language.text("未命名版面").into());
                            ui.label(
                                egui::RichText::new(if self.dirty() { "●" } else { "○" })
                                    .color(if self.dirty() { ACCENT } else { theme::MUTED }),
                            )
                            .on_hover_text(if self.dirty() {
                                self.language.text("尚有變更")
                            } else {
                                self.language.text("已保存")
                            });
                            ui.add_sized([170.0, 24.0], egui::Label::new(&name).truncate())
                                .on_hover_text(&name);
                            if !self.selected.is_empty() {
                                ui.label(
                                    egui::RichText::new(crate::localized!(
                                        self.language,
                                        "已選 {} 張",
                                        "{} selected",
                                        self.selected.len()
                                    ))
                                    .small()
                                    .color(ACCENT),
                                );
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add_enabled(
                                    ready,
                                    egui::Button::new(self.language.text("顯示全部")).frame(false),
                                )
                                .clicked()
                            {
                                self.fit();
                            }
                            ui.label(
                                egui::RichText::new(format!("{:.1}%", self.canvas.zoom * 100.0))
                                    .small()
                                    .color(theme::MUTED),
                            );
                            ui.separator();
                            ui.label(
                                egui::RichText::new(crate::localized!(
                                    self.language,
                                    "{} 張圖片",
                                    "{} images",
                                    self.canvas.items.len()
                                ))
                                .small()
                                .color(theme::MUTED),
                            );
                        });
                    });
                });
            self.top_ui_height = document_bar.response.rect.bottom() - ctx.screen_rect().top();
        }
        if show_bottom {
            egui::TopBottomPanel::bottom("status")
                .frame(
                    egui::Frame::NONE
                        .fill(theme::PANEL)
                        .inner_margin(egui::Margin::symmetric(16, 5)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let width = (ui.available_width() - 300.0).max(160.0);
                        ui.add_sized(
                            [width, 20.0],
                            egui::Label::new(
                                egui::RichText::new(&self.status)
                                    .small()
                                    .color(theme::MUTED),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&self.status);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                            egui::RichText::new(self.language.text("中鍵平移  ·  滾輪縮放  ·  Shift 多選"))
                                .small()
                                .color(theme::MUTED),
                        )
                        .on_hover_text(
                            self.language.text("拖曳圖片角點等比縮放；在空白處拖曳框選。Ctrl+Z 復原，Ctrl+S 保存。"),
                        );
                        });
                    });
                });
        }
    }
    fn canvas_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        self.canvas_rect = response.rect;
        painter.rect_filled(response.rect, 0.0, theme::BACKGROUND);
        let (top_ui, bottom_ui) = self.toolbar_visibility(ctx);
        let blocked = self.external_blocked
            || (self.fullscreen && (top_ui || bottom_ui))
            || self.settings_open
            || self.error.is_some()
            || self.open_job.is_some()
            || self.import_busy()
            || !matches!(self.after, AfterSave::None);
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        if !blocked && response.secondary_clicked() {
            self.finish_drag();
            self.image_menu = response.interact_pointer_pos().and_then(|p| {
                self.canvas
                    .items
                    .iter()
                    .rev()
                    .find(|item| self.rect(item).contains(p))
                    .map(|item| item.id)
            });
        }
        if !blocked && let Some(id) = self.image_menu {
            response.context_menu(|ui| {
                if ui.button(self.language.text("移除圖片")).clicked() {
                    self.selected = [id].into_iter().collect();
                    self.delete();
                    ui.close_menu();
                }
                for (label, front) in [("移到最前", true), ("移到最後", false)] {
                    if ui.button(self.language.text(label)).clicked() {
                        self.selected = [id].into_iter().collect();
                        self.layer(front);
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button("Source delete").clicked() {
                    if let Some(item) = self.canvas.items.iter().find(|item| item.id == id) {
                        self.tab_action = Some(tabs::Action::SourceDelete(item.source.clone()));
                    }
                    ui.close_menu();
                }
            });
        }
        if !blocked && !response.context_menu_opened() {
            if let Some(p) = pointer.filter(|p| response.rect.contains(*p)) {
                let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    self.canvas.zoom_at(
                        (scroll as f64 * 0.002).exp(),
                        [p.x as f64, p.y as f64],
                        [
                            response.rect.center().x as f64,
                            response.rect.center().y as f64,
                        ],
                    );
                    self.changed();
                }
                if ctx.input(|i| i.pointer.button_down(egui::PointerButton::Middle)) {
                    let d = ctx.input(|i| i.pointer.delta());
                    if d != Vec2::ZERO {
                        self.canvas.center[0] -= d.x as f64 / self.canvas.zoom;
                        self.canvas.center[1] -= d.y as f64 / self.canvas.zoom;
                        self.changed();
                    }
                    ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                }
                if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                    // A fast drag may press, move and release between two redraws.
                    let p = ctx
                        .input(|i| {
                            i.events.iter().find_map(|event| match event {
                                egui::Event::PointerButton {
                                    pos,
                                    button: egui::PointerButton::Primary,
                                    pressed: true,
                                    ..
                                } => Some(*pos),
                                _ => None,
                            })
                        })
                        .unwrap_or(p);
                    let additive = ctx.input(|i| i.modifiers.shift || i.modifiers.command);
                    let corner = self
                        .canvas
                        .items
                        .iter()
                        .rev()
                        .filter(|item| self.selected.contains(&item.id))
                        .find_map(|item| {
                            let r = self.rect(item);
                            [
                                r.left_top(),
                                r.right_top(),
                                r.left_bottom(),
                                r.right_bottom(),
                            ]
                            .into_iter()
                            .enumerate()
                            .find(|(_, c)| c.distance(p) <= 9.0)
                            .map(|(n, _)| (item.clone(), n))
                        });
                    if let Some((item, n)) = corner {
                        let anchor = [
                            if n % 2 == 0 {
                                item.position[0] + item.size[0]
                            } else {
                                item.position[0]
                            },
                            if n < 2 {
                                item.position[1] + item.size[1]
                            } else {
                                item.position[1]
                            },
                        ];
                        self.drag = Some(Drag::Resize {
                            id: item.id,
                            anchor,
                            before: self.canvas.items.clone(),
                            original: item.size,
                        });
                    } else if let Some(id) = self
                        .canvas
                        .items
                        .iter()
                        .rev()
                        .find(|item| self.rect(item).contains(p))
                        .map(|i| i.id)
                    {
                        if additive {
                            if !self.selected.insert(id) {
                                self.selected.remove(&id);
                            }
                        } else if !self.selected.contains(&id) {
                            self.selected.clear();
                            self.selected.insert(id);
                        }
                        self.drag = Some(Drag::Move {
                            start: self.world(p),
                            before: self.canvas.items.clone(),
                        });
                    } else {
                        let previous = self.selected.clone();
                        if !additive {
                            self.selected.clear();
                        }
                        self.drag = Some(Drag::Select {
                            start: p,
                            additive,
                            previous,
                        });
                    }
                }
            }
            if let Some(p) = pointer {
                let world = self.world(p);
                if ctx.input(|i| {
                    i.pointer.primary_down()
                        || i.pointer.button_released(egui::PointerButton::Primary)
                }) {
                    match &self.drag {
                        Some(Drag::Move { start, before }) => {
                            let delta = [world[0] - start[0], world[1] - start[1]];
                            let mut changed = false;
                            for (item, old) in self.canvas.items.iter_mut().zip(before) {
                                if self.selected.contains(&item.id) {
                                    let position =
                                        [old.position[0] + delta[0], old.position[1] + delta[1]];
                                    if position != item.position {
                                        item.position = position;
                                        changed = true;
                                    }
                                }
                            }
                            if changed {
                                self.changed();
                            }
                        }
                        Some(Drag::Resize {
                            id,
                            anchor,
                            original,
                            ..
                        }) => {
                            if let Some(item) = self.canvas.items.iter_mut().find(|i| i.id == *id) {
                                let d = [world[0] - anchor[0], world[1] - anchor[1]];
                                let scale = (d[0].abs() / original[0])
                                    .max(d[1].abs() / original[1])
                                    .max(0.001);
                                let size = [original[0] * scale, original[1] * scale];
                                let position = [
                                    if d[0] < 0.0 {
                                        anchor[0] - size[0]
                                    } else {
                                        anchor[0]
                                    },
                                    if d[1] < 0.0 {
                                        anchor[1] - size[1]
                                    } else {
                                        anchor[1]
                                    },
                                ];
                                if item.size != size || item.position != position {
                                    item.size = size;
                                    item.position = position;
                                    self.changed();
                                }
                            }
                        }
                        Some(Drag::Select {
                            start,
                            additive,
                            previous,
                        }) => {
                            let box_rect = Rect::from_two_pos(*start, p);
                            self.selected = if *additive {
                                previous.clone()
                            } else {
                                HashSet::new()
                            };
                            for item in &self.canvas.items {
                                if self.rect(item).intersects(box_rect) {
                                    self.selected.insert(item.id);
                                }
                            }
                            painter.rect_filled(
                                box_rect,
                                0.0,
                                Color32::from_rgba_unmultiplied(116, 210, 190, 25),
                            );
                            painter.rect_stroke(
                                box_rect,
                                0.0,
                                Stroke::new(1.0_f32, ACCENT),
                                egui::StrokeKind::Inside,
                            );
                        }
                        _ => {}
                    }
                }
            }
            if ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                self.finish_drag();
            }
            let paths = ctx.input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .filter_map(|f| f.path.clone())
                    .collect::<Vec<_>>()
            });
            if !paths.is_empty() {
                let drop_position = native_drop_position(ctx).or(pointer);
                self.start_import(paths, drop_position);
            }
        }
        if self.canvas.items.is_empty() {
            theme::empty_state(&painter, response.rect, self.language);
        }
        let now = Instant::now();
        let visible = self
            .canvas
            .items
            .iter()
            .filter(|item| self.rect(item).intersects(response.rect))
            .count()
            .max(1);
        // Distribute a texture budget across visible images instead of limiting image count.
        let texture_budget = media::memory_budget().min(256 * 1024 * 1024);
        let max_side =
            ((texture_budget as f64 / (visible as f64 * 4.0)).sqrt() as u32).clamp(1, 4096);
        self.previews
            .retain(|_, p| p.pending || p.touched.elapsed() < Duration::from_secs(1));
        for item in &self.canvas.items {
            let rect = self.rect(item);
            if !rect.intersects(response.rect) {
                continue;
            }
            let side = (rect.width().max(rect.height()) * ctx.pixels_per_point())
                .ceil()
                .clamp(1.0, max_side as f32) as u32;
            let side = side.next_power_of_two().min(max_side);
            let preview = self
                .previews
                .entry(item.source.clone())
                .or_insert_with(|| Preview {
                    texture: None,
                    side: 0,
                    pending: false,
                    next: None,
                    touched: now,
                    error: None,
                });
            preview.touched = now;
            if !preview.pending
                && preview.error.is_none()
                && (preview.texture.is_none()
                    || side > preview.side
                    || preview.side > max_side
                    || preview.next.is_some_and(|n| n <= now))
                && self
                    .decoder
                    .tx
                    .try_send(media::Request {
                        path: item.source.clone(),
                        side,
                    })
                    .is_ok()
            {
                preview.pending = true;
            }
            painter.rect_filled(rect, 0.0, Color32::from_rgb(42, 47, 55));
            if let Some(texture) = &preview.texture {
                painter.image(
                    texture.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                let text = if let Some(error) = &preview.error {
                    crate::localized!(
                        self.language,
                        "無法載入圖片\n{}\n{}",
                        "Unable to load image\n{}\n{}",
                        item.source.display(),
                        self.language.error(error)
                    )
                } else {
                    self.language.text("載入中…").into()
                };
                painter.with_clip_rect(rect.intersect(response.rect)).text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(14.0),
                    Color32::LIGHT_GRAY,
                );
            }
            if self.selected.contains(&item.id) {
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(2.0_f32, ACCENT),
                    egui::StrokeKind::Outside,
                );
                for p in [
                    rect.left_top(),
                    rect.right_top(),
                    rect.left_bottom(),
                    rect.right_bottom(),
                ] {
                    painter.rect_filled(Rect::from_center_size(p, Vec2::splat(8.0)), 1.0, ACCENT);
                }
            }
        }
        // Keep offscreen resources briefly, then release them; item metadata is never dropped.
        self.previews
            .retain(|_, p| p.pending || p.touched.elapsed() < Duration::from_secs(3));
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(job) = &self.import_job {
            let found = job.found.load(Ordering::Relaxed);
            egui::Window::new(self.language.text("掃描圖片"))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(crate::localized!(
                        self.language,
                        "正在掃描資料夾與所有子資料夾… 已找到 {found} 張圖片",
                        "Scanning folder and subfolders… Found {found} images"
                    ));
                    ui.spinner();
                    if ui.button(self.language.text("取消掃描")).clicked() {
                        self.cancel_import();
                    }
                });
        }
        if let Some((batch, _)) = &self.pending_import {
            let count = batch.images.len();
            let skipped = batch.warnings.len();
            egui::Window::new(self.language.text("確認大量圖片匯入"))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading(crate::localized!(
                        self.language,
                        "準備匯入 {count} 張圖片",
                        "Ready to import {count} images"
                    ));
                    ui.label(
                        self.language
                            .text("本批超過 100 張，確認後才會一次加入畫布。"),
                    );
                    if skipped > 0 {
                        ui.label(crate::localized!(
                            self.language,
                            "另有 {skipped} 個檔案／資料夾無法讀取，將略過。",
                            "{skipped} unreadable files/folders will be skipped."
                        ));
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add(theme::primary(crate::localized!(
                                self.language,
                                "匯入 {count} 張",
                                "Import {count} images"
                            )))
                            .clicked()
                        {
                            self.confirm_import();
                        }
                        if ui.button(self.language.text("取消")).clicked() {
                            self.cancel_import();
                        }
                    });
                });
        }
        if self.settings_open {
            let mut open = true;
            egui::Window::new(self.language.text("設定"))
                .open(&mut open)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .default_width(420.0)
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height((ctx.screen_rect().height() - 120.0).max(180.0))
                        .show(ui, |ui| {
                            ui.label(format!(
                                "InfiniteImageCanvas v{}",
                                env!("CARGO_PKG_VERSION")
                            ));
                            ui.separator();
                            ui.label("Language / 語言");
                            let previous_language = self.language;
                            ui.add_enabled_ui(self.job.is_none(), |ui| {
                                ui.horizontal(|ui| {
                                    ui.selectable_value(
                                        &mut self.language,
                                        crate::i18n::Language::TraditionalChinese,
                                        "繁體中文",
                                    );
                                    ui.selectable_value(
                                        &mut self.language,
                                        crate::i18n::Language::English,
                                        "English",
                                    );
                                });
                            });
                            if self.language != previous_language {
                                let chosen = self.language;
                                self.language = previous_language;
                                self.change_language(chosen);
                            }
                            ui.separator();
                            ui.label(egui::RichText::new(self.language.text("圖片保存")).strong());
                            ui.add_space(4.0);
                            let mut packed = self.canvas.packed;
                            ui.radio_value(
                                &mut packed,
                                false,
                                self.language.text("引用原圖路徑 · 檔案較小"),
                            );
                            ui.radio_value(
                                &mut packed,
                                true,
                                self.language.text("封裝原圖 · 可攜至其他電腦"),
                            );
                            if packed != self.canvas.packed {
                                self.canvas.packed = packed;
                                self.changed();
                            }
                            ui.separator();
                            ui.label(egui::RichText::new(self.language.text("匯入順序")).strong());
                            let before = self.random_order;
                            ui.radio_value(
                                &mut self.random_order,
                                false,
                                self.language.text("依載入順序"),
                            );
                            ui.radio_value(
                                &mut self.random_order,
                                true,
                                self.language.text("隨機順序"),
                            );
                            if self.random_order != before {
                                self.changed();
                            }
                            ui.label(
                                egui::RichText::new(
                                    self.language
                                        .text("圖片自動貼齊排列：3 張為 2＋1，5 張為 3＋2。"),
                                )
                                .small()
                                .color(theme::MUTED),
                            );
                            ui.separator();
                            ui.label(egui::RichText::new(self.language.text("自動保存")).strong());
                            ui.label(self.language.text("每分鐘保存變更，啟動時恢復最後版面。"));
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(crate::localized!(
                                        self.language,
                                        "保存位置：{}",
                                        "Save location: {}",
                                        self.directory.display()
                                    ))
                                    .small()
                                    .color(theme::MUTED),
                                )
                                .truncate(),
                            )
                            .on_hover_text(self.directory.display().to_string());
                            ui.weak(
                                self.language
                                    .text("引用模式需要保留原圖；剪貼簿圖片會保存至 assets。"),
                            );
                        });
                });
            self.settings_open = open;
        }
        if let Some(error) = self.error.clone() {
            egui::Window::new(self.language.text("需要處理"))
                .collapsible(false)
                .resizable(true)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(250.0)
                        .show(ui, |ui| {
                            ui.label(error);
                        });
                    ui.horizontal(|ui| {
                        if ui.button(self.language.text("重試保存")).clicked() {
                            self.error = None;
                            self.save(false, None);
                        }
                        if ui.button(self.language.text("另選保存位置")).clicked() {
                            self.error = None;
                            self.alternate_save();
                        }
                        if ui.button(self.language.text("返回畫布")).clicked() {
                            self.error = None;
                        }
                    });
                });
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.open_job.is_none() && self.error.is_none() && !self.import_busy() {
                self.transition(AfterSave::Close);
            }
        }
        if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.shortcuts(ctx);
        self.workspace(ctx);
        self.dialogs(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fullscreen_edge_controls_do_not_resize_canvas() {
        let (_dir, ctx, mut app) = setup();
        app.fullscreen = true;
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0));
        for (position, expected) in [
            (Pos2::new(640.0, 400.0), (false, false)),
            (Pos2::new(640.0, 10.0), (true, false)),
            (Pos2::new(640.0, 795.0), (false, true)),
            (Pos2::new(640.0, 400.0), (false, false)),
        ] {
            for _ in 0..2 {
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![egui::Event::PointerMoved(position)],
                    ..Default::default()
                };
                let _ = ctx.run(input, |ctx| {
                    assert_eq!(app.toolbar_visibility(ctx), expected);
                    app.workspace(ctx);
                });
                assert_eq!(app.canvas_rect, screen);
            }
        }
        app.fullscreen = false;
        assert_eq!(app.toolbar_visibility(&ctx), (true, true));
    }

    #[test]
    fn toolbar_controls_fit_small_windows_at_supported_scale_factors() {
        for language in [
            crate::i18n::Language::TraditionalChinese,
            crate::i18n::Language::English,
        ] {
            for scale in [1.0, 1.25, 1.5, 2.0] {
                for width in [720.0, 960.0, 1280.0] {
                    let (_dir, ctx, mut app) = setup();
                    app.language = language;
                    ctx.set_pixels_per_point(scale);
                    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 480.0));
                    let mut output = None;
                    for _ in 0..3 {
                        output = Some(ctx.run(
                            egui::RawInput {
                                screen_rect: Some(screen),
                                ..Default::default()
                            },
                            |ctx| app.workspace(ctx),
                        ));
                    }
                    let required = [
                        "檔案",
                        "匯入圖片",
                        "資料夾",
                        "貼上",
                        "保存",
                        "設定",
                        "全螢幕",
                        "編輯",
                        "顯示全部",
                    ];
                    let required = required.map(|key| language.text(key));
                    let mut found = std::collections::HashSet::new();
                    let mut rows = std::collections::HashMap::new();
                    for clipped in output.unwrap().shapes {
                        if let egui::Shape::Text(text) = clipped.shape {
                            let label = text.galley.job.text.as_str();
                            if required.contains(&label) {
                                let bounds = text.visual_bounding_rect();
                                assert!(
                                    screen.expand(1.0).contains_rect(bounds),
                                    "{label} outside {width}px at {scale}: {bounds:?}"
                                );
                                assert!(
                                    clipped.clip_rect.expand(1.0).contains_rect(bounds),
                                    "{label} clipped at {width}px / {scale}"
                                );
                                rows.insert(label.to_owned(), bounds.center().y);
                                found.insert(label.to_owned());
                            }
                        }
                    }
                    assert!(
                        (rows[language.text("檔案")] - rows[language.text("編輯")]).abs() < 1.0,
                        "File and Edit must share the top row"
                    );
                    assert_eq!(
                        found.len(),
                        required.len(),
                        "missing controls at {width}px / {scale}"
                    );
                }
            }
        }
    }

    #[test]
    fn language_persists_without_changing_canvas_or_old_settings() {
        let (_dir, ctx, mut app) = setup();
        let original = app.canvas.clone();
        app.change_language(crate::i18n::Language::English);
        assert_eq!(app.canvas, original);
        let restored = App::initialize(ctx.clone(), app.directory.clone(), None);
        assert_eq!(restored.language, crate::i18n::Language::English);
        app.save(false, None);
        settle(&mut app, &ctx);
        assert_eq!(
            storage::read_settings(&app.directory).language,
            crate::i18n::Language::English
        );
        app.change_language(crate::i18n::Language::TraditionalChinese);
        assert_eq!(
            storage::read_settings(&app.directory).language,
            crate::i18n::Language::TraditionalChinese
        );
        let old: Settings =
            serde_json::from_str(r#"{"packed":false,"last":null,"unnamed":false}"#).unwrap();
        assert_eq!(old.language, crate::i18n::Language::TraditionalChinese);
    }

    fn setup() -> (tempfile::TempDir, egui::Context, App) {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let app = App::initialize(ctx.clone(), dir.path().join("save file"), None);
        (dir, ctx, app)
    }
    fn settle(app: &mut App, ctx: &egui::Context) {
        let start = Instant::now();
        while app.job.is_some() || app.open_job.is_some() || app.import_job.is_some() {
            app.poll(ctx);
            assert!(start.elapsed() < Duration::from_secs(10), "worker timeout");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn fixture(dir: &std::path::Path) -> PathBuf {
        let p = dir.join("圖片.png");
        image::RgbaImage::from_pixel(30, 20, image::Rgba([255, 120, 80, 255]))
            .save(&p)
            .unwrap();
        p
    }
    fn fixture_batch(dir: &std::path::Path, count: usize) -> Vec<PathBuf> {
        let base = fixture(dir);
        let bytes = std::fs::read(base).unwrap();
        (0..count)
            .map(|n| {
                let path = dir.join(format!("batch-{n:04}.png"));
                std::fs::write(&path, &bytes).unwrap();
                path
            })
            .collect()
    }
    #[test]
    fn autosave_unnamed_then_manual_name_and_restore() {
        let (dir, ctx, mut app) = setup();
        app.import(vec![fixture(dir.path())], None);
        app.timer = Instant::now() - Duration::from_secs(61);
        app.poll(&ctx);
        settle(&mut app, &ctx);
        assert!(app.current.is_none());
        assert!(app.directory.join("previous_canvas.icanvas").exists());
        let modified = std::fs::metadata(app.directory.join("previous_canvas.icanvas"))
            .unwrap()
            .modified()
            .unwrap();
        app.timer = Instant::now() - Duration::from_secs(61);
        app.poll(&ctx);
        assert!(app.job.is_none());
        assert_eq!(
            modified,
            std::fs::metadata(app.directory.join("previous_canvas.icanvas"))
                .unwrap()
                .modified()
                .unwrap()
        );
        app.save(true, None);
        settle(&mut app, &ctx);
        assert!(
            app.current
                .as_ref()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("canvas_")
        );
        let mut restored = App::initialize(ctx.clone(), app.directory.clone(), None);
        settle(&mut restored, &ctx);
        assert_eq!(restored.canvas, app.canvas);
        assert_eq!(restored.current, app.current);
    }
    #[test]
    fn close_during_save_includes_edits_made_after_snapshot() {
        let (dir, ctx, mut app) = setup();
        app.import(vec![fixture(dir.path())], None);
        app.save(true, None);
        app.canvas.items[0].position = [88.0, 99.0];
        app.changed();
        app.transition(AfterSave::Close);
        settle(&mut app, &ctx);
        assert!(app.allow_close);
        assert!(!app.dirty());
        let loaded = storage::load(app.current.as_ref().unwrap(), dir.path()).unwrap();
        assert_eq!(loaded.items[0].position, [88.0, 99.0]);
    }
    #[test]
    fn failed_close_keeps_document_and_alternate_save_recovers() {
        let (dir, ctx, mut app) = setup();
        app.import(vec![fixture(dir.path())], None);
        let blocked = dir.path().join("not-a-folder");
        std::fs::write(&blocked, b"occupied").unwrap();
        app.current = Some(blocked.join("a.icanvas"));
        app.transition(AfterSave::Close);
        settle(&mut app, &ctx);
        assert!(!app.allow_close);
        assert!(app.dirty());
        assert!(app.error.is_some());
        assert_eq!(app.canvas.items.len(), 1);
        app.error = None;
        app.save(true, Some(dir.path().join("recovered.icanvas")));
        settle(&mut app, &ctx);
        assert!(!app.dirty());
        app.transition(AfterSave::Close);
        assert!(app.allow_close);
    }
    #[test]
    fn open_existing_then_close_saves_back_to_same_path() {
        let (dir, ctx, mut app) = setup();
        app.import(vec![fixture(dir.path())], None);
        app.save(true, None);
        settle(&mut app, &ctx);
        let path = app.current.clone().unwrap();
        app.begin_open(path.clone());
        settle(&mut app, &ctx);
        app.canvas.zoom = 2.0;
        app.changed();
        app.transition(AfterSave::Close);
        settle(&mut app, &ctx);
        assert_eq!(storage::load(&path, dir.path()).unwrap().zoom, 2.0);
    }
    #[test]
    fn multi_selection_layers_delete_undo_redo() {
        let (dir, _ctx, mut app) = setup();
        app.import(fixture_batch(dir.path(), 3), None);
        app.selected = [1, 2].into_iter().collect();
        app.layer(true);
        assert_eq!(
            app.canvas.items.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![3, 1, 2]
        );
        app.delete();
        assert_eq!(app.canvas.items.len(), 1);
        app.undo();
        assert_eq!(app.canvas.items.len(), 3);
        app.redo();
        assert_eq!(app.canvas.items.len(), 1);
    }
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| app.canvas_ui(ui, ctx));
            },
        );
    }
    fn button(p: Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: p,
            button,
            pressed,
            modifiers: Default::default(),
        }
    }
    #[test]
    fn real_egui_input_moves_image_and_middle_button_pans() {
        let (dir, ctx, mut app) = setup();
        app.import(vec![fixture(dir.path())], None);
        frame(&mut app, &ctx, vec![]);
        let p = app.rect(&app.canvas.items[0]).center();
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(p),
                button(p, egui::PointerButton::Primary, true),
            ],
        );
        let q = p + Vec2::new(50.0, 30.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(q)]);
        frame(
            &mut app,
            &ctx,
            vec![button(q, egui::PointerButton::Primary, false)],
        );
        assert_eq!(app.canvas.items[0].position, [50.0, 30.0]);
        app.undo();
        assert_eq!(app.canvas.items[0].position, [0.0, 0.0]);
        let p = Pos2::new(100.0, 100.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(p)]);
        frame(
            &mut app,
            &ctx,
            vec![button(p, egui::PointerButton::Middle, true)],
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(p + Vec2::new(80.0, 40.0))],
        );
        assert_eq!(app.canvas.center, [-80.0, -40.0]);
        assert_eq!(app.canvas.items[0].position, [0.0, 0.0]);
    }
    #[test]
    fn dropped_files_enter_same_import_path() {
        let (dir, ctx, mut app) = setup();
        let p = fixture(dir.path());
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))),
                dropped_files: vec![egui::DroppedFile {
                    path: Some(p),
                    ..Default::default()
                }],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.canvas_ui(ui, ctx));
            },
        );
        settle(&mut app, &ctx);
        assert_eq!(app.canvas.items.len(), 1);
    }

    #[test]
    fn switching_document_waits_for_clean_save_already_running() {
        let (dir, ctx, mut app) = setup();
        app.import(vec![fixture(dir.path())], None);
        app.save(true, None);
        settle(&mut app, &ctx);
        app.save(true, None);
        app.transition(AfterSave::New);
        assert!(app.job.is_some());
        assert!(!app.canvas.items.is_empty());
        settle(&mut app, &ctx);
        assert!(app.current.is_none());
        assert!(app.canvas.items.is_empty());
        assert!(!app.saved_once);
    }
    #[test]
    fn corner_resize_preserves_ratio_and_undo() {
        let (dir, ctx, mut app) = setup();
        app.import(vec![fixture(dir.path())], None);
        frame(&mut app, &ctx, vec![]);
        let p = app.rect(&app.canvas.items[0]).right_bottom();
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(p),
                button(p, egui::PointerButton::Primary, true),
            ],
        );
        let q = p + Vec2::new(30.0, 20.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(q)]);
        frame(
            &mut app,
            &ctx,
            vec![button(q, egui::PointerButton::Primary, false)],
        );
        assert_eq!(app.canvas.items[0].size, [60.0, 40.0]);
        app.undo();
        assert_eq!(app.canvas.items[0].size, [30.0, 20.0]);
    }
    #[test]
    fn f11_emits_toggle_commands_without_changing_canvas() {
        let (_dir, ctx, mut app) = setup();
        let original = app.canvas.clone();
        for expected in [true, false] {
            let output = ctx.run(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: egui::Key::F11,
                        physical_key: Some(egui::Key::F11),
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }],
                    ..Default::default()
                },
                |ctx| app.shortcuts(ctx),
            );
            assert!(output.viewport_output.values().any(|v| {
                v.commands
                    .contains(&egui::ViewportCommand::Fullscreen(expected))
            }));
            let _ = ctx.run(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: egui::Key::F11,
                        physical_key: Some(egui::Key::F11),
                        pressed: false,
                        repeat: false,
                        modifiers: Default::default(),
                    }],
                    ..Default::default()
                },
                |_| {},
            );
        }
        assert_eq!(original, app.canvas);
    }
    #[test]
    fn explicitly_opened_previous_canvas_is_a_named_target() {
        let (dir, ctx, mut app) = setup();
        let path = dir.path().join("previous_canvas.icanvas");
        storage::save(&Canvas::default(), &path).unwrap();
        app.begin_open(path.clone());
        settle(&mut app, &ctx);
        assert_eq!(app.current, Some(path.clone()));
        app.canvas.zoom = 3.0;
        app.changed();
        app.transition(AfterSave::Close);
        settle(&mut app, &ctx);
        assert_eq!(storage::load(&path, dir.path()).unwrap().zoom, 3.0);
    }
    #[test]
    fn batch_drop_uses_pointer_as_top_left_at_nonunit_zoom() {
        let (dir, ctx, mut app) = setup();
        frame(&mut app, &ctx, vec![]);
        app.canvas.zoom = 2.0;
        app.canvas.center = [100.0, -50.0];
        let pointer = Pos2::new(250.0, 200.0);
        let anchor = app.world(pointer);
        app.import(fixture_batch(dir.path(), 5), Some(pointer));
        assert_eq!(app.canvas.items[0].position, anchor);
        assert_eq!(app.canvas.items[1].position, [anchor[0] + 30.0, anchor[1]]);
        assert_eq!(app.canvas.items[3].position, [anchor[0], anchor[1] + 20.0]);
        app.random_order = true;
        app.changed();
        app.save(true, None);
        settle(&mut app, &ctx);
        assert!(storage::read_settings(&app.directory).random_order);
        let mut restored = App::initialize(ctx.clone(), app.directory.clone(), None);
        settle(&mut restored, &ctx);
        assert!(restored.random_order);
    }
    #[test]
    fn fast_drag_in_one_frame_still_moves_image() {
        let (dir, ctx, mut app) = setup();
        app.import(vec![fixture(dir.path())], None);
        frame(&mut app, &ctx, vec![]);
        let p = app.rect(&app.canvas.items[0]).center();
        let q = p + Vec2::new(50.0, 30.0);
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(p),
                button(p, egui::PointerButton::Primary, true),
                egui::Event::PointerMoved(q),
                button(q, egui::PointerButton::Primary, false),
            ],
        );
        assert_eq!(app.canvas.items[0].position, [50.0, 30.0]);
    }
    #[test]
    fn clipboard_pixels_survive_reference_and_packed_reopen() {
        let (dir, ctx, mut app) = setup();
        let pixels = arboard::ImageData {
            width: 3,
            height: 2,
            bytes: std::borrow::Cow::Owned([12u8, 34, 56, 255].repeat(6)),
        };
        app.import_clipboard_pixels(&pixels).unwrap();
        settle(&mut app, &ctx);
        let source = app.canvas.items[0].source.clone();
        assert_eq!(
            image::open(&source).unwrap().to_rgba8().as_raw(),
            pixels.bytes.as_ref()
        );
        app.save(true, None);
        settle(&mut app, &ctx);
        let path = app.current.clone().unwrap();
        let loaded = storage::load(&path, dir.path()).unwrap();
        assert!(loaded.items[0].source.exists());
        app.canvas.packed = true;
        app.changed();
        app.save(true, None);
        settle(&mut app, &ctx);
        std::fs::remove_file(source).unwrap();
        let loaded = storage::load(&path, dir.path()).unwrap();
        assert_eq!(
            image::ImageReader::open(&loaded.items[0].source)
                .unwrap()
                .with_guessed_format()
                .unwrap()
                .decode()
                .unwrap()
                .to_rgba8()
                .as_raw(),
            pixels.bytes.as_ref()
        );
    }
    #[test]
    fn hundred_imports_directly_but_101_requires_explicit_confirmation() {
        let (dir, ctx, mut app) = setup();
        let paths = fixture_batch(dir.path(), 101);
        app.start_import(paths[..100].to_vec(), None);
        settle(&mut app, &ctx);
        assert_eq!(app.canvas.items.len(), 100);
        assert!(app.pending_import.is_none());
        let original = app.canvas.clone();
        let original_revision = app.revision;
        app.start_import(paths.clone(), None);
        settle(&mut app, &ctx);
        assert_eq!(app.pending_import.as_ref().unwrap().0.images.len(), 101);
        assert_eq!(app.canvas, original);
        assert_eq!(app.revision, original_revision);
        app.cancel_import();
        assert_eq!(app.canvas, original);
        assert!(app.pending_import.is_none());
        app.start_import(paths, None);
        settle(&mut app, &ctx);
        app.confirm_import();
        assert_eq!(app.canvas.items.len(), 201);
        assert_eq!(app.selected.len(), 101);
        app.undo();
        assert_eq!(app.canvas.items, original.items);
    }
    #[test]
    fn recursive_folder_import_holds_batch_until_confirmation() {
        let (dir, ctx, mut app) = setup();
        let folder = dir.path().join("folder");
        let child = folder.join("nested");
        std::fs::create_dir_all(&child).unwrap();
        fixture_batch(&child, 101);
        app.start_import(vec![folder], None);
        settle(&mut app, &ctx);
        // fixture_batch also creates its master image, so all 102 distinct supported files count.
        assert_eq!(app.pending_import.as_ref().unwrap().0.images.len(), 102);
        assert!(app.canvas.items.is_empty());
        app.confirm_import();
        assert_eq!(app.canvas.items.len(), 102);
    }
    #[test]
    fn cancelling_scan_never_adds_its_eventual_result() {
        let (dir, ctx, mut app) = setup();
        let paths = fixture_batch(dir.path(), 10);
        app.start_import(paths, None);
        app.cancel_import();
        app.poll(&ctx);
        assert!(app.canvas.items.is_empty());
        assert!(!app.import_busy());
    }
    #[test]
    #[ignore = "explicit performance run; writes test-output/benchmark.csv"]
    fn benchmark_unique_images_and_large_canvas() {
        std::fs::create_dir_all("test-output").unwrap();
        let mut report = "unique_images,import_ms,first_frame_ms,save_ms,load_ms\n".to_string();
        for count in [100, 1000, 10000] {
            let (dir, ctx, mut app) = setup();
            let master = dir.path().join("master.png");
            image::RgbImage::from_pixel(480, 320, image::Rgb([50, 150, 210]))
                .save(&master)
                .unwrap();
            let bytes = std::fs::read(master).unwrap();
            let mut paths = Vec::new();
            for n in 0..count {
                let p = dir.path().join(format!("image-{n}.png"));
                std::fs::write(&p, &bytes).unwrap();
                paths.push(p);
            }
            let started = Instant::now();
            app.import(paths, None);
            app.confirm_import();
            let import_ms = started.elapsed().as_millis();
            for (n, item) in app.canvas.items.iter_mut().enumerate() {
                item.position = [(n % 100) as f64 * 520.0, (n / 100) as f64 * 360.0];
            }
            let started = Instant::now();
            frame(&mut app, &ctx, vec![]);
            let frame_ms = started.elapsed().as_millis();
            let output = dir.path().join("large.icanvas");
            let started = Instant::now();
            storage::save(&app.canvas, &output).unwrap();
            let save_ms = started.elapsed().as_millis();
            let started = Instant::now();
            let restored = storage::load(&output, dir.path()).unwrap();
            let load_ms = started.elapsed().as_millis();
            assert_eq!(restored, app.canvas);
            assert_eq!(restored.items.len(), count);
            report.push_str(&format!(
                "{count},{import_ms},{frame_ms},{save_ms},{load_ms}\n"
            ));
        }
        std::fs::write("test-output/benchmark.csv", &report).unwrap();
        println!("{report}");
    }
}
