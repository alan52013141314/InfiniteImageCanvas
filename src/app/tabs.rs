use super::*;

pub(super) enum Action {
    New,
    Open(PathBuf),
    Select(usize),
    Close(usize),
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
                unnamed: if tab.open_job.is_some() {
                    tab.opening_unnamed
                } else {
                    tab.current.is_none()
                },
            })
            .collect::<Vec<_>>();
        Settings {
            language: active.language,
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
            if matches!(self.closing, Some(Closing::Prompt(i)) if i == index) {
                tab.timer = Instant::now();
            }
            tab.poll(&self.ctx);
        }
        if self.closing.is_none()
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
        let blocked = self.closing.is_some() || self.exiting;
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
