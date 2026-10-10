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
        let Some(reading) = &mut self.reading else {
            return;
        };
        if let Some(item) = self.canvas.items.get(reading.page) {
            let cross = usize::from(reading.orientation == Orientation::Horizontal);
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
                let next = (reading.zoom * (delta * 0.002).exp()).clamp(0.00001, 10000.0);
                for (n, d) in offset.iter().enumerate() {
                    reading.center[n] += d / reading.zoom - d / next;
                }
                reading.zoom = next;
            } else {
                reading.center[axis] -= delta / reading.zoom;
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
                    (rect.width().max(rect.height()) * ctx.pixels_per_point())
                        .ceil()
                        .clamp(1.0, 4096.0) as u32,
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
            let cost = side as u64 * side as u64 * 4;
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
                && (preview.texture.is_none() || preview.side != side)
            {
                // Drop larger cached textures before issuing a budget-reduced replacement.
                if preview.side > side {
                    preview.texture = None;
                }
                if decoder.tx.try_send(media::Request { path, side }).is_ok() {
                    preview.pending = true;
                }
            }
        }
    }
}
