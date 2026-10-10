use super::*;

pub(super) enum Action {
    New,
    Open(PathBuf),
    Select(usize),
    Close(usize),
    Reorder(usize, usize),
    Rename(usize),
    Delete(usize),
    SourceDelete(PathBuf),
    Reading(usize, Option<features::Orientation>),
}
pub(super) struct Header {
    pub name: String,
    pub path: String,
    pub dirty: bool,
}
#[derive(Clone, Copy)]
enum Closing {
    Prompt(usize),
    Saving(usize),
}

pub struct Workspace {
    tabs: Vec<App>,
    active: usize,
    directory: PathBuf,
    ctx: egui::Context,
    paste: Arc<AtomicBool>,
    fullscreen: bool,
    closing: Option<Closing>,
    managing: Option<(usize, bool, String)>,
    source_delete: Option<PathBuf>,
    exiting: bool,
    allow_exit: bool,
    last_settings: Vec<u8>,
}

pub(super) fn same_path(a: &std::path::Path, b: &std::path::Path) -> bool {
    let normalize = |p: &std::path::Path| {
        std::fs::canonicalize(p)
            .or_else(|_| std::path::absolute(p))
            .unwrap_or_else(|_| p.to_path_buf())
            .to_string_lossy()
            .replace('/', "\\")
            .to_lowercase()
    };
    normalize(a) == normalize(b)
}

impl Workspace {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        directory: PathBuf,
        initial: Option<PathBuf>,
    ) -> Self {
        let workspace = Self::initialize(cc.egui_ctx.clone(), directory, initial);
        #[cfg(windows)]
        crate::native_input::install(cc, workspace.paste.clone());
        workspace
    }
    fn initialize(ctx: egui::Context, directory: PathBuf, initial: Option<PathBuf>) -> Self {
        let settings = storage::read_settings(&directory);
        let mut workspace = Self {
            tabs: Vec::new(),
            active: 0,
            directory,
            ctx,
            paste: Arc::new(AtomicBool::new(false)),
            fullscreen: false,
            closing: None,
            managing: None,
            source_delete: None,
            exiting: false,
            allow_exit: false,
            last_settings: Vec::new(),
        };
        let entries = settings.tabs.unwrap_or_else(|| {
            settings
                .last
                .into_iter()
                .map(|path| storage::SessionTab {
                    recovery: workspace.directory.join("previous_canvas.icanvas"),
                    path: Some(path),
                    unnamed: settings.unnamed,
                    reading: None,
                })
                .collect()
        });
        for entry in entries {
            if let Some(path) = &entry.path
                && workspace.find_path(path).is_some()
            {
                continue;
            }
            let mut tab = workspace.blank();
            tab.recovery = entry.recovery;
            tab.reading = entry.reading;
            if let Some(path) = entry.path {
                if !entry.unnamed {
                    tab.current = Some(path.clone());
                }
                tab.begin_open(path);
                tab.opening_unnamed = entry.unnamed;
            }
            workspace.tabs.push(tab);
        }
        if workspace.tabs.is_empty() {
            workspace.add_new();
        }
        workspace.active = settings.active_tab.min(workspace.tabs.len() - 1);
        if let Some(path) = initial {
            workspace.open(path);
        }
        workspace.refresh_headers();
        workspace
    }
    fn blank(&self) -> App {
        let mut tab = App::initialize_mode(self.ctx.clone(), self.directory.clone(), None, false);
        tab.paste_requested = self.paste.clone();
        tab.fullscreen = self.fullscreen;
        if let Some(active) = self.tabs.get(self.active) {
            tab.language = active.language;
            tab.random_order = active.random_order;
            tab.preload = active.preload;
            tab.canvas.packed = active.canvas.packed;
        }
        tab.recovery = self.next_recovery();
        tab
    }
    fn next_recovery(&self) -> PathBuf {
        for number in 1u64.. {
            let name = if number == 1 {
                "previous_canvas.icanvas".into()
            } else {
                format!("previous_canvas_{number:02}.icanvas")
            };
            let path = self.directory.join(name);
            if !path.exists()
                && !self.tabs.iter().any(|tab| {
                    same_path(&path, &tab.recovery)
                        || tab.current.as_ref().is_some_and(|p| same_path(p, &path))
                })
            {
                return path;
            }
        }
        unreachable!()
    }
    fn activate(&mut self, index: usize) {
        if index >= self.tabs.len() || self.active == index {
            return;
        }
        self.tabs[self.active].finish_drag();
        self.tabs[self.active].previews.clear();
        self.tabs[self.active].preload_decoder = None;
        self.tabs[self.active].settings_open = false;
        self.active = index;
        self.tabs[index].tab_scroll_to_active = true;
    }
    fn add_new(&mut self) {
        let tab = self.blank();
        self.tabs.push(tab);
        self.activate(self.tabs.len() - 1);
    }
    fn find_path(&self, path: &std::path::Path) -> Option<usize> {
        self.tabs.iter().position(|tab| {
            tab.current
                .as_ref()
                .or(tab.opening_path.as_ref())
                .is_some_and(|p| same_path(p, path))
                || (tab.saved_once && same_path(&tab.recovery, path))
        })
    }
    fn open(&mut self, path: PathBuf) {
        if let Some(index) = self.find_path(&path) {
            self.activate(index);
            return;
        }
        self.add_new();
        let tab = &mut self.tabs[self.active];
        tab.current = Some(path.clone());
        tab.begin_open(path);
    }
    fn refresh_headers(&mut self) {
        let language = self.tabs[self.active].language;
        let random_order = self.tabs[self.active].random_order;
        let preload = self.tabs[self.active].preload;
        let headers: Vec<_> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| Header {
                name: tab
                    .current
                    .as_ref()
                    .or(tab.opening_path.as_ref().filter(|_| !tab.opening_unnamed))
                    .and_then(|p| p.file_name())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| format!("{} {}", language.text("未命名版面"), index + 1)),
                path: tab
                    .current
                    .as_ref()
                    .unwrap_or(&tab.recovery)
                    .display()
                    .to_string(),
                dirty: tab.dirty(),
            })
            .collect();
        let paths: Vec<Vec<PathBuf>> = self
            .tabs
            .iter()
            .map(|tab| {
                let mut paths = vec![tab.recovery.clone()];
                paths.extend(tab.current.clone());
                paths.extend(tab.opening_path.clone());
                paths.extend(tab.job.as_ref().map(|job| job.path.clone()));
                paths
            })
            .collect();
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            tab.language = language;
            tab.random_order = random_order;
            tab.preload = preload;
            tab.fullscreen = self.fullscreen;
            tab.reserved_paths = paths
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .flat_map(|(_, p)| p.iter().cloned())
                .collect();
        }
        self.tabs[self.active].tab_headers = headers;
        self.tabs[self.active].active_tab = self.active;
    }
    fn settings(&self) -> Settings {
        let active = &self.tabs[self.active];
        let entries = self
            .tabs
            .iter()
            .map(|tab| storage::SessionTab {
                path: tab
                    .current
                    .clone()
                    .or(tab.opening_path.clone())
                    .or_else(|| {
                        (tab.saved_once || tab.recovery.exists()).then(|| tab.recovery.clone())
                    }),
                recovery: tab.recovery.clone(),
                reading: tab.reading.as_ref().map(features::Reading::snapshot),
                unnamed: if tab.open_job.is_some() {
                    tab.opening_unnamed
                } else {
                    tab.current.is_none()
                },
            })
            .collect::<Vec<_>>();
        Settings {
            language: active.language,
            preload: active.preload,
            packed: active.canvas.packed,
            last: entries[self.active].path.clone(),
            unnamed: entries[self.active].unnamed,
            random_order: active.random_order,
            tabs: Some(entries),
            active_tab: self.active,
        }
    }
    fn persist(&mut self) -> bool {
        let settings = self.settings();
        let bytes = serde_json::to_vec(&settings).expect("serializable settings");
        if bytes == self.last_settings {
            return true;
        }
        match storage::write_settings(&self.directory, &settings) {
            Ok(()) => {
                self.last_settings = bytes;
                true
            }
            Err(error) => {
                self.tabs[self.active].error = Some(error);
                false
            }
        }
    }
    fn remove(&mut self, index: usize) {
        if let Some(job) = &self.tabs[index].import_job {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.active = 0;
            self.add_new();
        } else if index < self.active {
            self.active -= 1;
        } else {
            self.active = self.active.min(self.tabs.len() - 1);
        }
        self.closing = None;
        self.refresh_headers();
        self.persist();
    }
    fn request_close(&mut self, index: usize) {
        self.activate(index);
        if self.tabs[index].open_job.is_some() || self.tabs[index].import_busy() {
            return;
        }
        self.tabs[index].finish_drag();
        self.closing = Some(Closing::Prompt(index));
    }
    fn save_and_close(&mut self, index: usize) {
        self.closing = Some(Closing::Saving(index));
        if self.tabs[index].job.is_none() {
            self.tabs[index].save(true, None);
        }
    }
    fn poll(&mut self) {
        // All document jobs and autosave clocks progress, including inactive tabs.
        self.refresh_headers();
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            if matches!(self.closing, Some(Closing::Prompt(i)) if i == index)
                || self.managing.as_ref().is_some_and(|(i, _, _)| *i == index)
                || self.source_delete.is_some()
            {
                tab.timer = Instant::now();
            }
            tab.poll(&self.ctx);
        }
        if self.closing.is_none()
            && self.managing.is_none()
            && self.source_delete.is_none()
            && !self.exiting
            && self.tabs[self.active].error.is_none()
            && let Some(index) = self.tabs.iter().position(|tab| tab.error.is_some())
        {
            self.activate(index);
        }
        if let Some(Closing::Saving(index)) = self.closing {
            let tab = &mut self.tabs[index];
            if tab.error.is_some() {
                self.closing = None;
                self.active = index;
            } else if tab.job.is_none() {
                if tab.dirty() || !tab.saved_once || tab.current.is_none() {
                    tab.save(true, None);
                } else {
                    self.remove(index);
                }
            }
        }
        if self.exiting {
            if let Some(index) = self.tabs.iter().position(|tab| tab.error.is_some()) {
                self.active = index;
                self.exiting = false;
            } else {
                for tab in &mut self.tabs {
                    if tab.job.is_none() && (tab.dirty() || !tab.saved_once) {
                        tab.save(false, None);
                    }
                }
                if self
                    .tabs
                    .iter()
                    .all(|tab| tab.job.is_none() && !tab.dirty() && tab.saved_once)
                    && self.persist()
                {
                    self.allow_exit = true;
                }
            }
        }
        self.persist();
    }
    fn handle_action(&mut self, action: Action) {
        match action {
            Action::New => self.add_new(),
            Action::Open(path) => self.open(path),
            Action::Select(index) => self.activate(index),
            Action::Close(index) => self.request_close(index),
            Action::SourceDelete(path) => self.source_delete = Some(path),
            Action::Reading(index, orientation) => {
                self.activate(index);
                self.tabs[index].set_reading(orientation);
            }
            Action::Rename(index) | Action::Delete(index) => {
                self.activate(index);
                if self.tabs[index].job.is_none()
                    && self.tabs[index].open_job.is_none()
                    && !self.tabs[index].import_busy()
                {
                    let path = self.tabs[index]
                        .current
                        .as_ref()
                        .unwrap_or(&self.tabs[index].recovery);
                    self.managing = Some((
                        index,
                        matches!(action, Action::Delete(_)),
                        path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    ));
                }
            }
            Action::Reorder(from, to) => {
                if from != to && from < self.tabs.len() && to < self.tabs.len() {
                    self.tabs[self.active].finish_drag();
                    let tab = self.tabs.remove(from);
                    self.tabs.insert(to, tab);
                    self.active = if self.active == from {
                        to
                    } else if from < self.active && to >= self.active {
                        self.active - 1
                    } else if from > self.active && to <= self.active {
                        self.active + 1
                    } else {
                        self.active
                    };
                }
            }
        }
        self.refresh_headers();
        self.persist();
    }
    fn close_dialog(&mut self, ctx: &egui::Context) {
        let Some(Closing::Prompt(index)) = self.closing else {
            return;
        };
        let language = self.tabs[index].language;
        let mut choice = None;
        egui::Window::new(language.text("關閉分頁"))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading(language.text("關閉前要保存此分頁嗎？"));
                ui.label(language.text("不保存只會放棄最近一次保存後的變更。"));
                ui.horizontal(|ui| {
                    if ui
                        .add(theme::primary(language.text("保存後關閉")))
                        .clicked()
                    {
                        choice = Some(0);
                    }
                    if ui
                        .add_enabled(
                            self.tabs[index].job.is_none(),
                            egui::Button::new(language.text("不保存並關閉")),
                        )
                        .clicked()
                    {
                        choice = Some(1);
                    }
                    if ui.button(language.text("取消")).clicked() {
                        choice = Some(2);
                    }
                });
            });
        match choice {
            Some(0) => self.save_and_close(index),
            Some(1) => self.remove(index),
            Some(2) => self.closing = None,
            _ => {}
        }
    }
}
impl Workspace {
    fn update_ui(&mut self, ctx: &egui::Context) {
        self.poll();
        if self.allow_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.closing.is_none()
                && self.managing.is_none()
                && self.source_delete.is_none()
                && self
                    .tabs
                    .iter()
                    .all(|tab| tab.open_job.is_none() && !tab.import_busy() && tab.error.is_none())
            {
                for tab in &mut self.tabs {
                    tab.finish_drag();
                }
                self.exiting = true;
            }
        }
        self.refresh_headers();
        let blocked = self.closing.is_some()
            || self.managing.is_some()
            || self.source_delete.is_some()
            || self.exiting;
        if !blocked {
            self.tabs[self.active].shortcuts(ctx);
        } else if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            self.tabs[self.active].toggle_fullscreen(ctx);
        }
        self.tabs[self.active].external_blocked = blocked;
        self.tabs[self.active].workspace(ctx);
        self.fullscreen = self.tabs[self.active].fullscreen;
        if !blocked {
            if let Some(action) = self.tabs[self.active].tab_action.take() {
                self.handle_action(action);
            }
            self.tabs[self.active].dialogs(ctx);
        } else {
            self.tabs[self.active].tab_action = None;
        }
        self.close_dialog(ctx);
        self.manage_dialog(ctx);
        self.source_delete_dialog(ctx);
        if self.exiting {
            egui::Window::new(self.tabs[self.active].language.text("正在保存所有分頁…"))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.spinner();
                });
        }
        ctx.request_repaint_after(Duration::from_millis(33));
    }
}

impl eframe::App for Workspace {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_ui(ctx);
    }
}

pub(super) fn tab_button(
    ui: &mut egui::Ui,
    index: usize,
    header: &Header,
    active: bool,
    language: crate::i18n::Language,
) -> (egui::Response, Option<Action>) {
    let label = format!("{}{}", if header.dirty { "● " } else { "" }, header.name);
    let galley = ui.painter().layout_no_wrap(
        label,
        egui::FontId::proportional(13.0),
        ui.visuals().text_color(),
    );
    let (rect, _) = ui.allocate_exact_size(Vec2::new(galley.size().x + 44.0, 30.0), Sense::hover());
    let close_rect = Rect::from_min_max(Pos2::new(rect.max.x - 28.0, rect.min.y), rect.max);
    let name_rect = Rect::from_min_max(rect.min, Pos2::new(close_rect.min.x, rect.max.y));
    let response = ui.interact(
        name_rect,
        ui.id().with(("tab", index)),
        Sense::click_and_drag(),
    );
    let close = ui.interact(
        close_rect,
        ui.id().with(("tab_close", index)),
        Sense::click(),
    );
    let whole = ui.interact(rect, ui.id().with(("tab_drop", index)), Sense::hover());
    let fill = if active {
        ui.visuals().selection.bg_fill
    } else if whole.hovered() {
        ui.visuals().widgets.hovered.bg_fill
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    ui.painter().rect(
        rect,
        5.0,
        fill,
        Stroke::new(
            1.0_f32,
            if active {
                ACCENT
            } else {
                ui.visuals().widgets.inactive.bg_stroke.color
            },
        ),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(
        Pos2::new(rect.min.x + 10.0, rect.center().y - galley.size().y / 2.0),
        galley,
        ui.visuals().text_color(),
    );
    if close.hovered() {
        ui.painter().rect_filled(
            close_rect.shrink(3.0),
            3.0,
            ui.visuals().widgets.hovered.bg_fill,
        );
    }
    ui.painter().text(
        close_rect.center(),
        egui::Align2::CENTER_CENTER,
        "×",
        egui::FontId::proportional(14.0),
        ui.visuals().text_color(),
    );
    let mut action = response.clicked().then_some(Action::Select(index));
    if close.on_hover_text(language.text("關閉分頁")).clicked() {
        action = Some(Action::Close(index));
    }
    response.dnd_set_drag_payload(index);
    if let Some(from) = whole.dnd_hover_payload::<usize>()
        && *from != index
    {
        let x = if *from < index {
            rect.right()
        } else {
            rect.left()
        };
        ui.painter().line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(2.0_f32, ACCENT),
        );
    }
    if let Some(from) = whole.dnd_release_payload::<usize>() {
        action = Some(Action::Reorder(*from, index));
    }
    response.context_menu(|ui| {
        ui.menu_button(language.text("漫畫閱讀模式"), |ui| {
            for (label, orientation) in [
                ("直向閱讀", Some(features::Orientation::Vertical)),
                ("橫向閱讀", Some(features::Orientation::Horizontal)),
                ("返回原版面", None),
            ] {
                if ui.button(language.text(label)).clicked() {
                    action = Some(Action::Reading(index, orientation));
                    ui.close_menu();
                }
            }
        });
        ui.separator();
        if ui.button(language.text("重新命名")).clicked() {
            action = Some(Action::Rename(index));
            ui.close_menu();
        }
        if ui.button(language.text("刪除版面檔案")).clicked() {
            action = Some(Action::Delete(index));
            ui.close_menu();
        }
    });
    (response.on_hover_text(&header.path), action)
}

impl Workspace {
    fn rename_file(&mut self, index: usize, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty()
            || name.ends_with('.')
            || name
                .chars()
                .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        {
            return Err(self.tabs[index].language.text("請輸入有效的檔名").into());
        }
        let tab = &mut self.tabs[index];
        let old = tab.current.as_ref().unwrap_or(&tab.recovery).clone();
        let new = old.with_file_name(format!(
            "{}.icanvas",
            name.strip_suffix(".icanvas").unwrap_or(name)
        ));
        if new == old {
            return Ok(());
        }
        if new.exists() || tab.reserved_paths.iter().any(|p| same_path(p, &new)) {
            return Err(tab.language.text("此檔名已被使用").into());
        }
        if old.exists() || tab.saved_once {
            std::fs::rename(&old, &new).map_err(storage::error)?;
        }
        tab.current = Some(new);
        if !tab.saved_once {
            tab.save(true, None);
        }
        Ok(())
    }
    fn delete_file(&mut self, index: usize) -> Result<(), String> {
        let tab = &self.tabs[index];
        let path = tab.current.as_ref().unwrap_or(&tab.recovery);
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(storage::error(e)),
        }
        self.remove(index);
        Ok(())
    }
    fn manage_dialog(&mut self, ctx: &egui::Context) {
        let Some((index, deleting, mut name)) = self.managing.take() else {
            return;
        };
        self.tabs[index].timer = Instant::now();
        let language = self.tabs[index].language;
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new(language.text(if deleting {
            "刪除版面檔案"
        } else {
            "重新命名"
        }))
        .id(egui::Id::new("manage_tab"))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            if deleting {
                ui.label(
                    language.text("將刪除版面檔案及放棄此分頁的變更，無法復原。來源圖片會保留。"),
                );
                ui.label(
                    self.tabs[index]
                        .current
                        .as_ref()
                        .unwrap_or(&self.tabs[index].recovery)
                        .display()
                        .to_string(),
                );
            } else {
                ui.text_edit_singleline(&mut name);
            }
            ui.horizontal(|ui| {
                confirm = ui
                    .button(language.text(if deleting {
                        "確認刪除"
                    } else {
                        "確認改名"
                    }))
                    .clicked();
                cancel = ui.button(language.text("取消")).clicked();
            });
        });
        if confirm {
            let result = if deleting {
                self.delete_file(index)
            } else {
                self.rename_file(index, &name)
            };
            if let Err(error) = result {
                self.tabs[index].error = Some(error);
            }
            self.refresh_headers();
            self.persist();
        } else if !cancel {
            self.managing = Some((index, deleting, name));
        }
    }
}

impl Workspace {
    fn source_delete_ready(&self) -> bool {
        self.tabs
            .iter()
            .all(|tab| tab.job.is_none() && tab.open_job.is_none() && !tab.import_busy())
    }
    fn delete_source(&mut self, path: &std::path::Path) -> Result<(), String> {
        if !self.source_delete_ready() {
            return Err(self.tabs[self.active]
                .language
                .text("請等待匯入或保存完成")
                .into());
        }
        let aliases: HashSet<PathBuf> = self
            .tabs
            .iter()
            .flat_map(|tab| tab.canvas.items.iter())
            .filter(|item| same_path(&item.source, path))
            .map(|item| item.source.clone())
            .collect();
        // Never change document state until the filesystem operation has succeeded.
        std::fs::remove_file(path).map_err(storage::error)?;
        for tab in &mut self.tabs {
            tab.finish_drag();
            let matches =
                |item: &Item| aliases.contains(&item.source) || same_path(&item.source, path);
            let before = tab.canvas.items.len();
            tab.canvas.items.retain(|item| !matches(item));
            tab.history.retain_items(|item| !matches(item));
            tab.selected
                .retain(|id| tab.canvas.items.iter().any(|item| item.id == *id));
            tab.image_menu = None;
            if tab.canvas.items.len() != before {
                tab.previews.clear();
                tab.decoder = media::Decoder::new(self.ctx.clone());
                tab.preload_decoder = None;
                tab.changed();
            }
        }
        self.refresh_headers();
        Ok(())
    }
    fn source_delete_dialog(&mut self, ctx: &egui::Context) {
        let Some(path) = self.source_delete.clone() else {
            return;
        };
        let language = self.tabs[self.active].language;
        let counts: Vec<usize> = self
            .tabs
            .iter()
            .map(|tab| {
                tab.canvas
                    .items
                    .iter()
                    .filter(|item| same_path(&item.source, &path))
                    .count()
            })
            .collect();
        let ready = self.source_delete_ready();
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new("Source delete")
            .id(egui::Id::new("source_delete_confirmation"))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_max_width(520.0);
                ui.label(
                    language.text("永久刪除以下來源檔案，並移除所有已開啟分頁中引用它的圖片。"),
                );
                ui.label(path.display().to_string());
                ui.label(crate::localized!(
                    language,
                    "影響 {} 個分頁、{} 張圖片",
                    "Affects {} tabs and {} images",
                    counts.iter().filter(|n| **n > 0).count(),
                    counts.iter().sum::<usize>()
                ));
                ui.label(
                    language.text("此操作無法復原來源檔案；未開啟的版面及既有封裝備份不會改寫。"),
                );
                if !ready {
                    ui.label(language.text("請等待匯入或保存完成"));
                }
                ui.horizontal(|ui| {
                    confirm = ui
                        .add_enabled(ready, egui::Button::new(language.text("確認刪除")))
                        .clicked();
                    cancel = ui.button(language.text("取消")).clicked();
                });
            });
        if cancel {
            self.source_delete = None;
        }
        if confirm {
            if let Err(error) = self.delete_source(&path) {
                self.tabs[self.active].error = Some(error);
            }
            self.source_delete = None;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let app =
            Workspace::initialize(egui::Context::default(), dir.path().join("save file"), None);
        (dir, app)
    }
    fn settle(workspace: &mut Workspace) {
        let start = Instant::now();
        loop {
            workspace.poll();
            if workspace
                .tabs
                .iter()
                .all(|tab| tab.job.is_none() && tab.open_job.is_none())
                && !matches!(workspace.closing, Some(Closing::Saving(_)))
                && (!workspace.exiting || workspace.allow_exit)
            {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn add_item(app: &mut App, x: f64) {
        app.canvas.items.push(Item {
            id: 1,
            source: app.directory.join("fixture.png"),
            position: [x, 2.0],
            size: [30.0, 20.0],
        });
        app.canvas.center = [x, 50.0];
        app.canvas.zoom = 2.0;
        app.selected.insert(1);
        app.changed();
    }
    fn ui_frame(w: &mut Workspace, events: Vec<egui::Event>) -> egui::FullOutput {
        let ctx = w.ctx.clone();
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(720.0, 480.0))),
                events,
                ..Default::default()
            },
            |ctx| w.update_ui(ctx),
        )
    }
    fn text_position(output: &egui::FullOutput, label: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == label
                {
                    return Some(text.visual_bounding_rect().center());
                }
                None
            })
            .unwrap_or_else(|| panic!("missing UI label: {label}"))
    }
    fn click(w: &mut Workspace, at: Pos2) -> egui::FullOutput {
        for pressed in [true, false] {
            ui_frame(
                w,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::default(),
                    },
                ],
            );
        }
        ui_frame(w, Vec::new())
    }
    #[test]
    fn reading_source_delete_and_tab_switch_do_not_restore_removed_images() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        let source = w.tabs[0].canvas.items[0].source.clone();
        std::fs::create_dir_all(&w.directory).unwrap();
        std::fs::write(&source, b"source").unwrap();
        ui_frame(&mut w, vec![]);
        w.handle_action(Action::Reading(0, Some(features::Orientation::Vertical)));
        w.add_new();
        assert!(w.tabs[1].reading.is_none());
        w.delete_source(&source).unwrap();
        w.activate(0);
        ui_frame(&mut w, vec![]);
        assert!(w.tabs[0].reading.as_ref().unwrap().positions.is_empty());
        w.handle_action(Action::Reading(0, None));
        w.tabs[0].undo();
        assert!(w.tabs[0].canvas.items.is_empty());
    }
    #[test]
    fn blank_context_menu_opens_settings_and_gathers_at_click() {
        let (_dir, mut w) = setup();
        w.tabs[0].language = crate::i18n::Language::English;
        add_item(&mut w.tabs[0], 10.0);
        ui_frame(&mut w, vec![]);
        let at = w.tabs[0].canvas_rect.left_top() + Vec2::new(25.0, 25.0);
        let expected = w.tabs[0].world(at);
        for pressed in [true, false] {
            ui_frame(
                &mut w,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        let output = ui_frame(&mut w, vec![]);
        click(&mut w, text_position(&output, "Gather here"));
        let item = &w.tabs[0].canvas.items[0];
        assert_eq!(
            [
                item.position[0] + item.size[0] / 2.0,
                item.position[1] + item.size[1] / 2.0
            ],
            expected
        );
        w.tabs[0].undo();
        for pressed in [true, false] {
            ui_frame(
                &mut w,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        let output = ui_frame(&mut w, vec![]);
        let menu_settings = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(t) = &shape.shape
                    && t.galley.job.text == "Settings"
                {
                    Some(t.visual_bounding_rect().center())
                } else {
                    None
                }
            })
            .max_by(|a, b| a.y.total_cmp(&b.y))
            .unwrap();
        click(&mut w, menu_settings);
        assert!(w.tabs[0].settings_open);
        let output = ui_frame(&mut w, vec![]);
        assert!(text_position(&output, "Preload nearby images").y > 0.0);
    }
    #[test]
    fn gather_preserves_sizes_centers_and_undo() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 100.0);
        let mut item = w.tabs[0].canvas.items[0].clone();
        item.id = 2;
        item.position = [500.0, 300.0];
        item.size = [50.0, 60.0];
        w.tabs[0].canvas.items.push(item);
        let before = w.tabs[0].canvas.items.clone();
        w.tabs[0].gather(Some([20.0, 40.0]));
        let items = &w.tabs[0].canvas.items;
        assert_eq!(items[0].position, [-20.0, 10.0]);
        assert_eq!(items[1].position, [10.0, 10.0]);
        assert_eq!(items[0].size, before[0].size);
        w.tabs[0].undo();
        assert_eq!(w.tabs[0].canvas.items, before);
        w.tabs[0].gather(None);
        assert_eq!(w.tabs[0].canvas.items[0].position, [285.0, 151.0]);
    }
    #[test]
    fn reading_scroll_arrows_zoom_restore_and_session_preserve_original_layout() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        w.tabs[0].canvas.items[0].size = [200.0, 600.0];
        let mut second = w.tabs[0].canvas.items[0].clone();
        second.id = 2;
        second.position = [1000.0, 2000.0];
        w.tabs[0].canvas.items.push(second);
        let original = w.tabs[0].canvas.clone();
        ui_frame(&mut w, vec![]);
        w.handle_action(Action::Reading(0, Some(features::Orientation::Vertical)));
        ui_frame(&mut w, vec![]);
        assert_eq!(
            w.tabs[0].reading.as_ref().unwrap().positions[&2],
            [0.0, 600.0]
        );
        ui_frame(
            &mut w,
            vec![egui::Event::Key {
                key: egui::Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(w.tabs[0].reading.as_ref().unwrap().page, 1);
        let before = w.tabs[0].reading.as_ref().unwrap().center;
        let at = w.tabs[0].canvas_rect.center();
        ui_frame(
            &mut w,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -100.0),
                    modifiers: Default::default(),
                },
            ],
        );
        assert!(w.tabs[0].reading.as_ref().unwrap().center[1] > before[1]);
        let ctx = w.ctx.clone();
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let before_zoom = w.tabs[0].reading.as_ref().unwrap().zoom;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(720.0, 480.0))),
                modifiers,
                events: vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(0.0, 100.0),
                        modifiers,
                    },
                ],
                ..Default::default()
            },
            |ctx| w.update_ui(ctx),
        );
        assert!(w.tabs[0].reading.as_ref().unwrap().zoom > before_zoom);
        assert_eq!(w.tabs[0].canvas, original);
        w.handle_action(Action::Reading(0, Some(features::Orientation::Horizontal)));
        assert_eq!(
            w.tabs[0].reading.as_ref().unwrap().positions[&2],
            [200.0, 0.0]
        );
        w.tabs[0].save(false, None);
        settle(&mut w);
        w.persist();
        let mut restored =
            Workspace::initialize(egui::Context::default(), w.directory.clone(), None);
        settle(&mut restored);
        assert_eq!(restored.tabs[0].canvas, original);
        assert_eq!(
            restored.tabs[0].reading.as_ref().unwrap().orientation,
            features::Orientation::Horizontal
        );
        restored.handle_action(Action::Reading(0, None));
        assert_eq!(restored.tabs[0].canvas, original);
    }
    #[test]
    fn preload_prioritizes_nearby_limits_budget_and_persists_settings() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 0.0);
        w.tabs[0].canvas.zoom = 1.0;
        w.tabs[0].canvas.center = [0.0; 2];
        w.tabs[0].canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0));
        w.tabs[0].canvas.items.clear();
        for n in 0..5 {
            let path = w.directory.join(format!("preload{n}.png"));
            std::fs::create_dir_all(&w.directory).unwrap();
            image::RgbaImage::new(8, 8).save(&path).unwrap();
            w.tabs[0].canvas.items.push(Item {
                id: n + 1,
                source: path,
                position: [600.0 + n as f64 * 1000.0, 0.0],
                size: [512.0, 512.0],
            });
        }
        w.tabs[0].preload = storage::PreloadSettings {
            enabled: true,
            extra_mb: 1,
        };
        let ctx = w.ctx.clone();
        w.tabs[0].preload_images(&ctx);
        assert_eq!(w.tabs[0].previews.len(), 1);
        let deadline = Instant::now() + Duration::from_secs(3);
        while w.tabs[0].previews.values().any(|p| p.pending) && Instant::now() < deadline {
            w.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(w.tabs[0].previews.values().all(|p| p.texture.is_some()));
        w.tabs[0].canvas.items[0].position = [0.0, 0.0];
        w.tabs[0].preload_images(&ctx);
        assert!(
            w.tabs[0].previews[&w.directory.join("preload0.png")]
                .texture
                .is_some()
        );
        w.tabs[0].canvas.items[0].position = [600.0, 0.0];
        assert!(
            w.tabs[0]
                .previews
                .contains_key(&w.directory.join("preload0.png"))
        );
        w.persist();
        let settings = storage::read_settings(&w.directory);
        assert_eq!(settings.preload, w.tabs[0].preload);
        w.tabs[0].preload.extra_mb = 0;
        w.tabs[0].preload_images(&ctx);
        assert!(w.tabs[0].previews.is_empty());
        w.tabs[0].preload.enabled = false;
        w.tabs[0].preload_images(&ctx);
        assert!(w.tabs[0].preload_decoder.is_none());
    }
    #[test]
    fn source_delete_waits_for_saves_and_suspends_all_autosave_clocks() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        let source = w.tabs[0].canvas.items[0].source.clone();
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, b"source").unwrap();
        w.tabs[0].save(false, None);
        w.add_new();
        add_item(&mut w.tabs[1], 20.0);
        w.handle_action(Action::SourceDelete(source.clone()));
        assert!(!w.source_delete_ready());
        assert!(w.delete_source(&source).is_err());
        assert!(source.exists());
        w.tabs[1].timer = Instant::now() - Duration::from_secs(61);
        w.poll();
        assert!(w.tabs[1].job.is_none());
        settle(&mut w);
        assert!(w.source_delete_ready());
        w.delete_source(&source).unwrap();
        assert!(w.tabs.iter().all(|tab| tab.canvas.items.is_empty()));
    }
    #[test]
    fn source_delete_removes_all_open_references_and_cannot_be_undone() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        let source = w.tabs[0].canvas.items[0].source.clone();
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, b"source").unwrap();
        let keep = source.with_file_name("keep.png");
        std::fs::write(&keep, b"keep").unwrap();
        w.add_new();
        add_item(&mut w.tabs[1], 20.0);
        let before = w.tabs[1].canvas.items.clone();
        let mut other = before[0].clone();
        other.id = 2;
        other.source = keep.clone();
        w.tabs[1].canvas.items.push(other);
        let after = w.tabs[1].canvas.items.clone();
        w.tabs[1].history.commit(before, &after);
        w.tabs[1].undo();
        w.tabs[1].redo();
        w.delete_source(&source).unwrap();
        assert!(!source.exists());
        assert!(keep.exists());
        assert!(w.tabs[0].canvas.items.is_empty());
        assert_eq!(w.tabs[1].canvas.items.len(), 1);
        for tab in &mut w.tabs {
            tab.undo();
            tab.redo();
            assert!(tab.canvas.items.iter().all(|item| item.source != source));
            tab.save(false, None);
        }
        settle(&mut w);
        let mut restored =
            Workspace::initialize(egui::Context::default(), w.directory.clone(), None);
        settle(&mut restored);
        assert!(
            restored.tabs.iter().all(|tab| tab
                .canvas
                .items
                .iter()
                .all(|item| item.source != source))
        );
    }
    #[test]
    fn source_delete_failure_preserves_images_and_packed_delete_targets_asset() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        let source = w.tabs[0].canvas.items[0].source.clone();
        assert!(w.delete_source(&source).is_err());
        assert_eq!(w.tabs[0].canvas.items.len(), 1);
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, b"original").unwrap();
        w.tabs[0].canvas.packed = true;
        let packed = w.directory.join("packed.icanvas");
        storage::save(&w.tabs[0].canvas, &packed).unwrap();
        w.tabs[0].canvas = storage::load(&packed, &w.directory.join("assets")).unwrap();
        let asset = w.tabs[0].canvas.items[0].source.clone();
        assert_ne!(asset, source);
        w.delete_source(&asset).unwrap();
        assert!(source.exists());
        assert!(!asset.exists());
        assert!(packed.exists());
        assert!(w.tabs[0].canvas.items.is_empty());
    }
    #[test]
    fn image_right_click_targets_topmost_and_source_delete_requires_confirmation() {
        let (_dir, mut w) = setup();
        w.tabs[0].language = crate::i18n::Language::English;
        add_item(&mut w.tabs[0], 10.0);
        let source = w.tabs[0].canvas.items[0].source.clone();
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, b"fixture").unwrap();
        let mut top = w.tabs[0].canvas.items[0].clone();
        top.id = 2;
        w.tabs[0].canvas.items.push(top);
        ui_frame(&mut w, vec![]);
        let at = w.tabs[0].rect(&w.tabs[0].canvas.items[1]).center();
        for pressed in [true, false] {
            ui_frame(
                &mut w,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert_eq!(w.tabs[0].image_menu, Some(2));
        let output = ui_frame(&mut w, vec![]);
        assert!(text_position(&output, "Remove image").x > 0.0);
        let output = click(&mut w, text_position(&output, "Source delete"));
        assert!(w.source_delete.is_some());
        assert!(source.exists());
        click(&mut w, text_position(&output, "Cancel"));
        assert!(w.source_delete.is_none());
        assert_eq!(w.tabs[0].canvas.items.len(), 2);
        w.handle_action(Action::SourceDelete(source.clone()));
        let output = ui_frame(&mut w, vec![]);
        click(&mut w, text_position(&output, "Confirm deletion"));
        assert!(!source.exists());
        assert!(w.tabs[0].canvas.items.is_empty());
    }
    #[test]
    fn failed_file_operations_keep_tab_and_dialog_suspends_autosave() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        w.tabs[0].save(false, None);
        settle(&mut w);
        let old = w.tabs[0].recovery.clone();
        std::fs::remove_file(&old).unwrap();
        assert!(w.rename_file(0, "Missing").is_err());
        assert!(w.tabs[0].current.is_none());
        std::fs::create_dir(&old).unwrap();
        assert!(w.delete_file(0).is_err());
        assert_eq!(w.tabs.len(), 1);
        w.handle_action(Action::Delete(0));
        add_item(&mut w.tabs[0], 20.0);
        w.tabs[0].timer = Instant::now() - Duration::from_secs(61);
        w.poll();
        assert!(w.tabs[0].job.is_none());
        assert!(w.managing.is_some());
    }
    #[test]
    fn pointer_drag_reorders_and_context_menu_confirms_deletion() {
        let (_dir, mut w) = setup();
        w.tabs[0].language = crate::i18n::Language::English;
        add_item(&mut w.tabs[0], 10.0);
        w.add_new();
        add_item(&mut w.tabs[1], 20.0);
        ui_frame(&mut w, vec![]);
        let output = ui_frame(&mut w, vec![]);
        let from = text_position(&output, "● Untitled canvas 2");
        let to = text_position(&output, "● Untitled canvas 1");
        ui_frame(
            &mut w,
            vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton {
                    pos: from,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        ui_frame(&mut w, vec![egui::Event::PointerMoved(to)]);
        ui_frame(&mut w, vec![egui::Event::PointerMoved(to)]);
        ui_frame(
            &mut w,
            vec![egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(w.tabs[0].canvas.items[0].position[0], 20.0);
        assert_eq!(w.active, 0);
        let output = ui_frame(&mut w, vec![]);
        let at = text_position(&output, "● Untitled canvas 1");
        for pressed in [true, false] {
            ui_frame(
                &mut w,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        let output = ui_frame(&mut w, vec![]);
        assert!(text_position(&output, "Rename").x > 0.0);
        let output = click(&mut w, text_position(&output, "Delete canvas file"));
        assert!(
            w.managing
                .as_ref()
                .is_some_and(|(_, deleting, _)| *deleting)
        );
        click(&mut w, text_position(&output, "Cancel"));
        assert!(w.managing.is_none());
        assert_eq!(w.tabs.len(), 2);
    }
    #[test]
    fn rename_delete_and_reorder_persist_without_mixing_documents() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        w.tabs[0].save(false, None);
        settle(&mut w);
        let old = w.tabs[0].recovery.clone();
        w.rename_file(0, "Renamed").unwrap();
        let renamed = w.tabs[0].current.clone().unwrap();
        assert!(!old.exists());
        assert!(renamed.exists());
        w.add_new();
        add_item(&mut w.tabs[1], 20.0);
        assert!(w.rename_file(1, "Renamed").is_err());
        assert!(w.rename_file(1, "../bad").is_err());
        w.tabs[1].save(false, None);
        settle(&mut w);
        let second = w.tabs[1].recovery.clone();
        w.handle_action(Action::Reorder(0, 1));
        assert_eq!(w.active, 0);
        assert_eq!(w.tabs[0].canvas.items[0].position[0], 20.0);
        let mut restored =
            Workspace::initialize(egui::Context::default(), w.directory.clone(), None);
        settle(&mut restored);
        assert_eq!(restored.active, 0);
        assert_eq!(restored.tabs[1].current.as_ref(), Some(&renamed));
        assert_eq!(restored.tabs[0].canvas.items[0].position[0], 20.0);
        restored.delete_file(1).unwrap();
        assert!(!renamed.exists());
        assert!(second.exists());
        assert_eq!(restored.tabs.len(), 1);
    }
    #[test]
    fn real_tab_buttons_new_switch_close_cancel_and_save() {
        let (_dir, mut w) = setup();
        w.tabs[0].language = crate::i18n::Language::English;
        ui_frame(&mut w, Vec::new());
        let output = ui_frame(&mut w, Vec::new());
        assert!(text_position(&output, "Edit").y < text_position(&output, "Untitled canvas 1").y);
        let output = click(&mut w, text_position(&output, "+"));
        assert_eq!(w.tabs.len(), 2);
        assert_eq!(w.active, 1);
        let output = click(&mut w, text_position(&output, "Untitled canvas 1"));
        assert_eq!(w.active, 0);
        let output = click(&mut w, text_position(&output, "×"));
        assert!(matches!(w.closing, Some(Closing::Prompt(0))));
        let output = click(&mut w, text_position(&output, "Cancel"));
        assert!(w.closing.is_none());
        assert_eq!(w.tabs.len(), 2);
        let output = click(&mut w, text_position(&output, "×"));
        click(&mut w, text_position(&output, "Save and close"));
        settle(&mut w);
        assert_eq!(w.tabs.len(), 1);
    }

    #[test]
    fn tabs_keep_independent_history_selection_view_and_restore_all() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        let before = w.tabs[0].canvas.items.clone();
        w.tabs[0].canvas.items[0].position[0] = 20.0;
        let after = w.tabs[0].canvas.items.clone();
        w.tabs[0].history.commit(before, &after);
        w.handle_action(Action::New);
        add_item(&mut w.tabs[1], 40.0);
        w.handle_action(Action::New);
        add_item(&mut w.tabs[2], 60.0);
        w.handle_action(Action::Select(0));
        assert!(w.tabs[0].selected.contains(&1));
        w.tabs[0].undo();
        assert_eq!(w.tabs[0].canvas.items[0].position[0], 10.0);
        assert_eq!(w.tabs[1].canvas.items[0].position[0], 40.0);
        assert_eq!(w.tabs[0].canvas.center, [10.0, 50.0]);
        w.handle_action(Action::Select(1));
        for tab in &mut w.tabs {
            tab.timer = Instant::now() - Duration::from_secs(61);
        }
        settle(&mut w);
        let paths: HashSet<_> = w.tabs.iter().map(|tab| tab.recovery.clone()).collect();
        assert_eq!(paths.len(), 3);
        assert!(paths.iter().all(|p| p.exists()));
        assert_eq!(w.tabs[0].recovery.file_stem().unwrap(), "previous_canvas");
        assert_eq!(
            w.tabs[1].recovery.file_stem().unwrap(),
            "previous_canvas_02"
        );
        let mut restored = Workspace::initialize(w.ctx.clone(), w.directory.clone(), None);
        settle(&mut restored);
        assert_eq!(restored.tabs.len(), 3);
        assert_eq!(restored.active, 1);
        for (a, b) in restored.tabs.iter().zip(&w.tabs) {
            assert_eq!(a.canvas, b.canvas);
            assert!(a.current.is_none());
        }
    }
    #[test]
    fn simultaneous_manual_saves_never_share_a_name_and_open_reuses_tab() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        w.handle_action(Action::New);
        add_item(&mut w.tabs[1], 20.0);
        w.tabs[0].save(true, None);
        w.refresh_headers();
        w.tabs[1].save(true, None);
        let a = w.tabs[0].job.as_ref().unwrap().path.clone();
        let b = w.tabs[1].job.as_ref().unwrap().path.clone();
        assert_ne!(a, b);
        settle(&mut w);
        w.handle_action(Action::Open(a.clone()));
        assert_eq!(w.tabs.len(), 2);
        assert_eq!(w.active, 0);
        w.tabs[0].save(true, Some(b));
        assert!(w.tabs[0].error.is_some());
        assert!(w.tabs[0].job.is_none());
        assert_eq!(
            storage::load(&a, &w.directory.join("assets"))
                .unwrap()
                .items[0]
                .position[0],
            10.0
        );
    }
    #[test]
    fn close_cancel_discard_and_save_keep_other_tabs_unchanged() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        w.handle_action(Action::New);
        add_item(&mut w.tabs[1], 20.0);
        w.request_close(1);
        assert!(matches!(w.closing, Some(Closing::Prompt(1))));
        w.closing = None;
        assert_eq!(w.tabs.len(), 2);
        assert!(w.tabs[1].dirty());
        let discarded = w.tabs[1].recovery.clone();
        w.request_close(1);
        w.remove(1);
        assert_eq!(w.tabs.len(), 1);
        assert!(!discarded.exists());
        w.handle_action(Action::New);
        add_item(&mut w.tabs[1], 30.0);
        let saved = w.tabs[1].date_path();
        w.request_close(1);
        w.save_and_close(1);
        settle(&mut w);
        assert_eq!(w.tabs.len(), 1);
        assert!(saved.exists());
        assert_eq!(w.tabs[0].canvas.items[0].position[0], 10.0);
        let mut restored = Workspace::initialize(w.ctx.clone(), w.directory.clone(), None);
        settle(&mut restored);
        assert_eq!(restored.tabs.len(), 1);
    }
    #[test]
    fn failed_tab_close_retains_document_and_exit_saves_all_tabs() {
        let (_dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        w.tabs[0].canvas.packed = true;
        w.request_close(0);
        w.save_and_close(0);
        settle(&mut w);
        assert_eq!(w.tabs.len(), 1);
        assert!(w.tabs[0].error.is_some());
        assert!(w.closing.is_none());
        w.tabs[0].error = None;
        w.tabs[0].canvas.packed = false;
        w.handle_action(Action::New);
        add_item(&mut w.tabs[1], 20.0);
        w.exiting = true;
        settle(&mut w);
        assert!(w.allow_exit);
        let mut restored = Workspace::initialize(w.ctx.clone(), w.directory.clone(), None);
        settle(&mut restored);
        assert_eq!(restored.tabs.len(), 2);
        assert_eq!(restored.tabs[0].canvas.items[0].position[0], 10.0);
        assert_eq!(restored.tabs[1].canvas.items[0].position[0], 20.0);
    }
    #[test]
    fn migrate_old_settings_and_keep_recovery_files_safe() {
        let (dir, mut w) = setup();
        add_item(&mut w.tabs[0], 10.0);
        w.exiting = true;
        settle(&mut w);
        let recovery = w.tabs[0].recovery.clone();
        let old = serde_json::json!({"packed":false,"last":recovery,"unnamed":true,"random_order":false,"language":"en"});
        storage::atomic_write(
            &w.directory.join("settings.json"),
            &serde_json::to_vec(&old).unwrap(),
        )
        .unwrap();
        let mut restored = Workspace::initialize(w.ctx.clone(), w.directory.clone(), None);
        settle(&mut restored);
        assert_eq!(restored.tabs.len(), 1);
        assert!(restored.tabs[0].current.is_none());
        assert_eq!(restored.tabs[0].language, crate::i18n::Language::English);
        restored.handle_action(Action::New);
        assert!(
            restored.tabs[1]
                .recovery
                .ends_with("previous_canvas_02.icanvas")
        );
        let _ = dir;
    }
    #[test]
    fn tab_strip_and_edit_row_render_in_both_languages() {
        for language in [
            crate::i18n::Language::English,
            crate::i18n::Language::TraditionalChinese,
        ] {
            let (_dir, mut w) = setup();
            w.tabs[0].language = language;
            for _ in 0..8 {
                w.handle_action(Action::New);
            }
            w.refresh_headers();
            let ctx = w.ctx.clone();
            for _ in 0..3 {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(720.0, 480.0))),
                        ..Default::default()
                    },
                    |ctx| w.tabs[w.active].workspace(ctx),
                );
            }
            assert!(w.tabs[w.active].canvas_rect.width() <= 720.0);
            assert!(w.tabs[w.active].canvas_rect.height() > 250.0);
            assert!(!w.tabs[w.active].tab_scroll_to_active);
        }
    }
}
