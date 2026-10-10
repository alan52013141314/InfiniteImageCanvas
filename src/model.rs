use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub id: u64,
    pub source: PathBuf,
    pub position: [f64; 2],
    pub size: [f64; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Canvas {
    pub version: u32,
    pub items: Vec<Item>,
    pub center: [f64; 2],
    pub zoom: f64,
    pub packed: bool,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            version: 1,
            items: vec![],
            center: [0.0; 2],
            zoom: 1.0,
            packed: false,
        }
    }
}

impl Canvas {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("不支援的版面版本".into());
        }
        if !self.zoom.is_finite() || self.zoom <= 0.0 || !self.center.iter().all(|v| v.is_finite())
        {
            return Err("無效的畫布座標".into());
        }
        let mut ids = HashSet::new();
        for item in &self.items {
            if !ids.insert(item.id)
                || !item.position.iter().all(|v| v.is_finite())
                || !item.size.iter().all(|v| v.is_finite() && *v > 0.0)
            {
                return Err("無效的圖片資料".into());
            }
        }
        Ok(())
    }

    pub fn world(&self, screen: [f64; 2], origin: [f64; 2]) -> [f64; 2] {
        [
            self.center[0] + (screen[0] - origin[0]) / self.zoom,
            self.center[1] + (screen[1] - origin[1]) / self.zoom,
        ]
    }
    pub fn screen(&self, world: [f64; 2], origin: [f64; 2]) -> [f64; 2] {
        [
            (world[0] - self.center[0]) * self.zoom + origin[0],
            (world[1] - self.center[1]) * self.zoom + origin[1],
        ]
    }
    pub fn zoom_at(&mut self, factor: f64, pointer: [f64; 2], origin: [f64; 2]) {
        let before = self.world(pointer, origin);
        self.zoom = (self.zoom * factor).clamp(0.00001, 10000.0);
        let after = self.world(pointer, origin);
        for i in 0..2 {
            self.center[i] += before[i] - after[i];
        }
    }
}

/// Row-major packing: contiguous widths within a row, next row below its tallest image.
pub fn arrange_batch(sizes: &[[f64; 2]], origin: [f64; 2]) -> Vec<[f64; 2]> {
    let columns = (sizes.len() as f64).sqrt().ceil().max(1.0) as usize;
    let mut positions = Vec::with_capacity(sizes.len());
    let mut x = origin[0];
    let mut y = origin[1];
    let mut height = 0.0_f64;
    for (n, size) in sizes.iter().enumerate() {
        if n > 0 && n % columns == 0 {
            x = origin[0];
            y += height;
            height = 0.0;
        }
        positions.push([x, y]);
        x += size[0];
        height = height.max(size[1]);
    }
    positions
}

pub fn shuffle_batch<T>(items: &mut [T]) {
    // UUID v4 supplies independent OS-random entropy; rejection sampling avoids modulo bias.
    for n in (1..items.len()).rev() {
        let bound = (n + 1) as u64;
        let limit = u64::MAX - u64::MAX % bound;
        let random = loop {
            let uuid = uuid::Uuid::new_v4();
            let b = uuid.as_bytes();
            let value = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[12], b[13], b[14], b[15]]);
            if value < limit {
                break value;
            }
        };
        items.swap(n, (random % bound) as usize);
    }
}

#[derive(Default)]
pub struct History {
    undo: Vec<Vec<Item>>,
    redo: Vec<Vec<Item>>,
}
impl History {
    pub fn retain_items(&mut self, mut keep: impl FnMut(&Item) -> bool) {
        for items in self.undo.iter_mut().chain(self.redo.iter_mut()) {
            items.retain(&mut keep);
        }
    }
    pub fn commit(&mut self, before: Vec<Item>, after: &[Item]) {
        if before != after {
            self.undo.push(before);
            self.redo.clear();
        }
    }
    pub fn undo(&mut self, canvas: &mut Canvas) -> bool {
        if let Some(items) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut canvas.items, items));
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self, canvas: &mut Canvas) -> bool {
        if let Some(items) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut canvas.items, items));
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_layout_matches_one_through_five_examples() {
        let expected = [
            vec![[10.0, 20.0]],
            vec![[10.0, 20.0], [110.0, 20.0]],
            vec![[10.0, 20.0], [110.0, 20.0], [10.0, 80.0]],
            vec![[10.0, 20.0], [110.0, 20.0], [10.0, 80.0], [110.0, 80.0]],
            vec![
                [10.0, 20.0],
                [110.0, 20.0],
                [210.0, 20.0],
                [10.0, 80.0],
                [110.0, 80.0],
            ],
        ];
        for (n, positions) in expected.iter().enumerate() {
            assert_eq!(
                &arrange_batch(&vec![[100.0, 60.0]; n + 1], [10.0, 20.0]),
                positions
            );
        }
        assert!(arrange_batch(&[], [0.0; 2]).is_empty());
    }
    #[test]
    fn mixed_dimensions_touch_horizontally_without_row_overlap() {
        let sizes = [[100.0, 80.0], [40.0, 120.0], [80.0, 50.0], [20.0, 40.0]];
        assert_eq!(
            arrange_batch(&sizes, [0.0; 2]),
            vec![[0.0, 0.0], [100.0, 0.0], [0.0, 120.0], [80.0, 120.0]]
        );
    }
    #[test]
    fn random_order_is_a_permutation_and_varies() {
        let source = (0..20).collect::<Vec<_>>();
        let mut observed = std::collections::HashSet::new();
        for _ in 0..10 {
            let mut values = source.clone();
            shuffle_batch(&mut values);
            observed.insert(values.clone());
            values.sort();
            assert_eq!(values, source);
        }
        assert!(observed.len() > 1);
    }
    #[test]
    fn zoom_keeps_pointer_world_position() {
        let mut c = Canvas {
            center: [1e9, -1e9],
            ..Default::default()
        };
        let p = [400.0, 333.0];
        let o = [600.0, 400.0];
        let w = c.world(p, o);
        c.zoom_at(1.7, p, o);
        assert_eq!(w, c.world(p, o));
        let s = c.screen(w, o);
        assert!((s[0] - p[0]).abs() < 0.001);
    }
    #[test]
    fn history_restores_delete_and_clears_redo_on_edit() {
        let mut c = Canvas::default();
        let mut h = History::default();
        let i = Item {
            id: 1,
            source: "a.png".into(),
            position: [0.0; 2],
            size: [30.0; 2],
        };
        c.items.push(i);
        let before = c.items.clone();
        c.items.clear();
        h.commit(before, &c.items);
        assert!(h.undo(&mut c));
        assert_eq!(c.items.len(), 1);
        assert!(h.redo(&mut c));
        assert!(c.items.is_empty());
        h.undo(&mut c);
        let before = c.items.clone();
        c.items[0].position[0] = 1.0;
        h.commit(before, &c.items);
        assert!(!h.redo(&mut c));
    }
}
