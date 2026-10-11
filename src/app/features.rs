use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    Vertical,
    Horizontal,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub orientation: Orientation,
    pub center: [f64; 2],
    pub zoom: f64,
    pub page: usize,
    #[serde(skip)]
    pub positions: HashMap<u64, [f64; 2]>,
}
impl Reading {
    pub fn snapshot(&self) -> Self {
        Self {
            positions: HashMap::new(),
            orientation: self.orientation,
            center: self.center,
            zoom: self.zoom,
            page: self.page,
        }
    }
    fn layout(&mut self, items: &[Item]) {
        self.positions.clear();
        let axis = usize::from(self.orientation == Orientation::Vertical);
        let mut offset = 0.0;
        for item in items {
            let mut p = [0.0; 2];
            p[axis] = offset;
            self.positions.insert(item.id, p);
            offset += item.size[axis];
        }
        self.page = self.page.min(items.len().saturating_sub(1));
    }
}
impl App {
    pub(super) fn fit_reading(&mut self) {
        let Some(reading) = &self.reading else {
            return;
        };
        self.fit_reading_axis(usize::from(reading.orientation == Orientation::Horizontal));
    }
    pub(super) fn fit_reading_axis(&mut self, cross: usize) {
        let Some(reading) = &mut self.reading else {
            return;
        };
        if let Some(item) = self.canvas.items.get(reading.page) {
            let available = if cross == 0 {
                self.canvas_rect.width()
            } else {
                self.canvas_rect.height()
            } as f64;
            reading.zoom = ((available - 40.0).max(1.0) / item.size[cross]).clamp(0.00001, 10000.0);
        }
        self.jump_reading(0);
    }
    pub(super) fn set_reading(&mut self, orientation: Option<Orientation>) {
        self.finish_drag();
        self.navigation = Navigation::default();
        self.reading = orientation.map(|orientation| Reading {
            orientation,
            center: [0.0; 2],
            zoom: 1.0,
            page: 0,
            positions: HashMap::new(),
        });
        if let Some(reading) = &mut self.reading {
            reading.layout(&self.canvas.items);
        }
        self.jump_reading(0);
    }
    fn jump_reading(&mut self, delta: isize) {
        let Some(reading) = &mut self.reading else {
            return;
        };
        reading.layout(&self.canvas.items);
        if self.canvas.items.is_empty() {
            return;
        }
        reading.page = reading
            .page
            .saturating_add_signed(delta)
            .min(self.canvas.items.len() - 1);
        let item = &self.canvas.items[reading.page];
        let position = reading.positions[&item.id];
        let axis = usize::from(reading.orientation == Orientation::Vertical);
        let viewport = [
            self.canvas_rect.width().max(1.0) as f64,
            self.canvas_rect.height().max(1.0) as f64,
        ];
        reading.center = [
            position[0] + item.size[0] / 2.0,
            position[1] + item.size[1] / 2.0,
        ];
        reading.center[axis] = position[axis] + viewport[axis] / (2.0 * reading.zoom);
    }
    pub(super) fn reading_input(&mut self, ctx: &egui::Context, pointer: Option<Pos2>) {
        if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
            if let Some(r) = &mut self.reading {
                r.page = 0;
            }
            self.jump_reading(0);
        }
        let Some(reading) = &mut self.reading else {
            return;
        };
        let axis = usize::from(reading.orientation == Orientation::Vertical);
        if let Some(p) = pointer.filter(|p| self.canvas_rect.contains(*p)) {
            let (scroll, zooming) =
                ctx.input(|i| (i.raw_scroll_delta, i.modifiers.ctrl || i.modifiers.command));
            let delta = if axis == 0 && scroll.x != 0.0 {
                scroll.x
            } else {
                scroll.y
            } as f64;
            if zooming && delta != 0.0 {
                let origin = self.canvas_rect.center();
                let offset = [p.x as f64 - origin.x as f64, p.y as f64 - origin.y as f64];
                let next = (reading.zoom * (delta * self.wheel.zoom * 0.002).exp())
                    .clamp(0.00001, 10000.0);
                for (n, d) in offset.iter().enumerate() {
                    reading.center[n] += d / reading.zoom - d / next;
                }
                reading.zoom = next;
            } else {
                reading.center[axis] -= delta * self.wheel.scroll / reading.zoom;
            }
        }
        // Resolve the current page from the reading position after wheel scrolling.
        if let Some((index, _)) = self
            .canvas
            .items
            .iter()
            .enumerate()
            .rev()
            .find(|(_, item)| {
                reading.positions.get(&item.id).is_some_and(|p| {
                    p[axis]
                        <= reading.center[axis]
                            - (if axis == 0 {
                                self.canvas_rect.width()
                            } else {
                                self.canvas_rect.height()
                            }) as f64
                                / (2.0 * reading.zoom)
                            + 0.00001
                })
            })
        {
            reading.page = index;
        }
        let previous = if axis == 1 {
            egui::Key::ArrowUp
        } else {
            egui::Key::ArrowLeft
        };
        let next = if axis == 1 {
            egui::Key::ArrowDown
        } else {
            egui::Key::ArrowRight
        };
        if ctx.input(|i| i.key_pressed(previous)) {
            self.jump_reading(-1);
        }
        if ctx.input(|i| i.key_pressed(next)) {
            self.jump_reading(1);
        }
    }
    pub(super) fn gather(&mut self, anchor: Option<[f64; 2]>) {
        if self.canvas.items.is_empty() || self.reading.is_some() {
            return;
        }
        self.finish_drag();
        let before = self.canvas.items.clone();
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for item in &before {
            for n in 0..2 {
                min[n] = min[n].min(item.position[n]);
                max[n] = max[n].max(item.position[n] + item.size[n]);
            }
        }
        let anchor = anchor.unwrap_or([(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0]);
        let positions = crate::model::arrange_batch(
            &before.iter().map(|i| i.size).collect::<Vec<_>>(),
            [0.0; 2],
        );
        let mut extent = [0.0_f64; 2];
        for (item, p) in before.iter().zip(&positions) {
            for n in 0..2 {
                extent[n] = extent[n].max(p[n] + item.size[n]);
            }
        }
        for (item, p) in self.canvas.items.iter_mut().zip(positions) {
            item.position = [
                p[0] + anchor[0] - extent[0] / 2.0,
                p[1] + anchor[1] - extent[1] / 2.0,
            ];
        }
        self.history.commit(before, &self.canvas.items);
        self.changed();
    }
    pub(super) fn refresh_reading(&mut self) {
        if let Some(reading) = &mut self.reading {
            if !reading.zoom.is_finite()
                || reading.zoom <= 0.0
                || !reading.center.iter().all(|n| n.is_finite())
            {
                reading.zoom = 1.0;
                reading.center = [0.0; 2];
            }
            reading.layout(&self.canvas.items);
        }
    }
    pub(super) fn preload_images(&mut self, ctx: &egui::Context) {
        if !self.preload.enabled {
            if self.preload_decoder.take().is_some() {
                self.previews.clear();
            }
            return;
        }
        let budget =
            (self.preload.extra_mb.min(65536) as u64 * 1024 * 1024).min(media::memory_budget());
        let mut remaining = budget;
        let visible: HashSet<_> = self
            .canvas
            .items
            .iter()
            .filter(|item| self.rect(item).intersects(self.canvas_rect))
            .map(|item| item.source.clone())
            .collect();
        let mut candidates: Vec<_> = self
            .canvas
            .items
            .iter()
            .filter(|i| !visible.contains(&i.source))
            .map(|item| {
                let rect = self.rect(item);
                (
                    rect.center().distance_sq(self.canvas_rect.center()),
                    item.source.clone(),
                    media::preview_side(
                        rect.width().max(rect.height()) * ctx.pixels_per_point(),
                        4096,
                    ),
                )
            })
            .collect();
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut retained = visible;
        let mut plan = Vec::new();
        for (_, path, side) in candidates {
            if retained.contains(&path) {
                continue;
            }
            // Keep ready larger tiers without undercounting their retained memory.
            let retained_side = self.previews.get(&path).map_or(side, |p| p.side.max(side));
            let cost = retained_side as u64 * retained_side as u64 * 4;
            if cost > remaining {
                continue;
            }
            remaining -= cost;
            retained.insert(path.clone());
            plan.push((path, side));
        }
        self.previews.retain(|path, _| retained.contains(path));
        if plan.is_empty() {
            return;
        }
        let decoder = self
            .preload_decoder
            .get_or_insert_with(|| media::Decoder::new(ctx.clone()));
        for (path, side) in plan {
            let preview = self
                .previews
                .entry(path.clone())
                .or_insert_with(|| Preview {
                    texture: None,
                    side: 0,
                    pending: false,
                    next: None,
                    touched: Instant::now(),
                    error: None,
                });
            preview.touched = Instant::now();
            if !preview.pending
                && preview.error.is_none()
                && (preview.texture.is_none() || preview.side < side)
                && decoder.tx.try_send(media::Request { path, side }).is_ok()
            {
                preview.pending = true;
            }
        }
    }
}

#[derive(Default)]
pub(super) struct Navigation {
    pub hold: Option<(f64, Pos2)>,
    pub anchor: Option<Pos2>,
    pub cancelled: bool,
}
impl App {
    pub(super) fn reading_fit_menu(&mut self, ui: &mut egui::Ui, target: Option<u64>) {
        if self.reading.is_none() {
            return;
        }
        for (label, axis) in [("符合視窗寬度", 0), ("符合視窗高度", 1)] {
            if ui.button(self.language.text(label)).clicked() {
                if let Some(id) = target
                    && let Some(index) = self.canvas.items.iter().position(|item| item.id == id)
                {
                    self.reading.as_mut().unwrap().page = index;
                }
                self.fit_reading_axis(axis);
                ui.close_menu();
            }
        }
        ui.separator();
    }
    pub(super) fn navigation_input(
        &mut self,
        ctx: &egui::Context,
        _painter: &egui::Painter,
        blocked: bool,
    ) {
        let (time, dt, point, pressed, released, down, delta, focused) = ctx.input(|i| {
            (
                i.time,
                i.stable_dt.min(0.05) as f64,
                i.pointer.hover_pos(),
                i.pointer.button_pressed(egui::PointerButton::Middle),
                i.pointer.button_released(egui::PointerButton::Middle),
                i.pointer.button_down(egui::PointerButton::Middle),
                i.pointer.delta(),
                i.focused,
            )
        });
        if blocked || !focused || point.is_none() || ctx.memory(|m| m.any_popup_open()) {
            self.navigation = Navigation::default();
            return;
        }
        let point = point.unwrap();
        if pressed {
            if self.navigation.anchor.take().is_some() {
                self.navigation.hold = None;
                self.navigation.cancelled = true;
            } else if self.canvas_rect.contains(point) {
                self.navigation.hold = Some((time, point));
                self.navigation.cancelled = false;
            }
        }
        if released {
            if !self.navigation.cancelled
                && let Some((start, anchor)) = self.navigation.hold.take()
                && time - start >= 0.5
            {
                self.navigation.anchor = Some(anchor);
            }
            self.navigation.hold = None;
            self.navigation.cancelled = false;
        }
        let painter = ctx
            .layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("auto_scroll_marker"),
            ))
            .with_clip_rect(self.canvas_rect);
        let movement = if let Some(anchor) = self.navigation.anchor {
            let offset = point - anchor;
            painter.circle_stroke(anchor, 12.0, Stroke::new(1.5_f32, ACCENT));
            painter.line_segment(
                [anchor - Vec2::new(6.0, 0.0), anchor + Vec2::new(6.0, 0.0)],
                Stroke::new(1.0_f32, ACCENT),
            );
            painter.line_segment(
                [anchor - Vec2::new(0.0, 6.0), anchor + Vec2::new(0.0, 6.0)],
                Stroke::new(1.0_f32, ACCENT),
            );
            ctx.set_cursor_icon(egui::CursorIcon::AllScroll);
            let speed = |d: f32| (d.abs() - 12.0).clamp(0.0, 1000.0) * d.signum();
            Vec2::new(speed(offset.x), speed(offset.y)) * (4.0 * dt) as f32
        } else if down
            && self.navigation.hold.is_some()
            && !self.navigation.cancelled
            && self.canvas_rect.contains(point)
        {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            -delta
        } else {
            Vec2::ZERO
        };
        if movement != Vec2::ZERO {
            if let Some(reading) = &mut self.reading {
                reading.center[0] += movement.x as f64 / reading.zoom;
                reading.center[1] += movement.y as f64 / reading.zoom;
            } else {
                self.canvas.center[0] += movement.x as f64 / self.canvas.zoom;
                self.canvas.center[1] += movement.y as f64 / self.canvas.zoom;
                self.changed();
            }
        }
    }
}

#[cfg(test)]
mod navigation_tests {
    use super::*;
    fn frame(
        app: &mut App,
        ctx: &egui::Context,
        time: f64,
        events: Vec<egui::Event>,
        focused: bool,
    ) {
        let _ = ctx.run(
            egui::RawInput {
                time: Some(time),
                focused,
                screen_rect: Some(app.canvas_rect),
                events,
                ..Default::default()
            },
            |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                app.navigation_input(ctx, &painter, false);
            },
        );
    }
    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Middle,
            pressed,
            modifiers: Default::default(),
        }
    }
    #[test]
    fn middle_hold_autoscroll_threshold_speed_stop_and_focus_in_both_modes() {
        for reading in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let ctx = egui::Context::default();
            let mut app = App::initialize_mode(ctx.clone(), dir.path().to_path_buf(), None, false);
            app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
            if reading {
                app.set_reading(Some(Orientation::Vertical));
            }
            let anchor = Pos2::new(300.0, 300.0);
            frame(
                &mut app,
                &ctx,
                0.0,
                vec![egui::Event::PointerMoved(anchor), button(anchor, true)],
                true,
            );
            frame(&mut app, &ctx, 0.49, vec![button(anchor, false)], true);
            assert!(app.navigation.anchor.is_none());
            frame(&mut app, &ctx, 1.0, vec![button(anchor, true)], true);
            frame(&mut app, &ctx, 1.51, vec![button(anchor, false)], true);
            assert_eq!(app.navigation.anchor, Some(anchor));
            let center = |a: &App| a.reading.as_ref().map_or(a.canvas.center, |r| r.center);
            let before = center(&app);
            frame(
                &mut app,
                &ctx,
                1.55,
                vec![egui::Event::PointerMoved(anchor + Vec2::new(60.0, 60.0))],
                true,
            );
            let near = center(&app);
            frame(
                &mut app,
                &ctx,
                1.59,
                vec![egui::Event::PointerMoved(anchor + Vec2::new(120.0, 120.0))],
                true,
            );
            let far = center(&app);
            assert!(near[0] > before[0] && near[1] > before[1]);
            assert!(far[0] - near[0] > near[0] - before[0]);
            let point = anchor + Vec2::new(120.0, 120.0);
            frame(&mut app, &ctx, 1.63, vec![button(point, true)], true);
            assert!(app.navigation.anchor.is_none());
            frame(&mut app, &ctx, 2.3, vec![button(point, false)], true);
            assert!(app.navigation.anchor.is_none());
            assert_eq!(center(&app), far);
            app.navigation.anchor = Some(anchor);
            frame(&mut app, &ctx, 2.4, vec![], false);
            assert!(app.navigation.anchor.is_none());
        }
    }
    #[test]
    fn reading_fit_axes_and_wheel_multipliers_preserve_normal_canvas() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = App::initialize_mode(ctx.clone(), dir.path().to_path_buf(), None, false);
        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        app.canvas.items.push(Item {
            id: 1,
            source: dir.path().join("page.png"),
            position: [50.0, 70.0],
            size: [200.0, 400.0],
        });
        let original = app.canvas.clone();
        app.set_reading(Some(Orientation::Vertical));
        app.fit_reading_axis(0);
        assert_eq!(app.reading.as_ref().unwrap().zoom, 3.8);
        app.fit_reading_axis(1);
        assert_eq!(app.reading.as_ref().unwrap().zoom, 1.4);
        let scroll = |app: &mut App, multiplier: f64, zoom: bool| {
            app.wheel = storage::WheelSettings {
                scroll: multiplier,
                zoom: multiplier,
            };
            app.reading.as_mut().unwrap().zoom = 1.0;
            app.reading.as_mut().unwrap().center = [0.0; 2];
            let modifiers = egui::Modifiers {
                ctrl: zoom,
                command: zoom,
                ..Default::default()
            };
            let _ = ctx.run(
                egui::RawInput {
                    modifiers,
                    events: vec![egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(0.0, -100.0),
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ctx| app.reading_input(ctx, Some(Pos2::new(400.0, 300.0))),
            );
            app.reading.as_ref().unwrap().clone()
        };
        let one = scroll(&mut app, 1.0, false);
        let two = scroll(&mut app, 2.0, false);
        assert_eq!(two.center[1], one.center[1] * 2.0);
        let one = scroll(&mut app, 1.0, true);
        let two = scroll(&mut app, 2.0, true);
        assert!((two.zoom - one.zoom.powi(2)).abs() < 1e-10);
        assert_eq!(app.canvas, original);
        let settings = Settings {
            wheel: app.wheel,
            ..Default::default()
        };
        storage::write_settings(dir.path(), &settings).unwrap();
        assert_eq!(storage::read_settings(dir.path()).wheel, app.wheel);
    }
}
