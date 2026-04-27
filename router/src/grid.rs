use dsn_parser::{Pcb, Shape};

pub struct GridMap {
    pub width: usize,
    pub height: usize,
    pub num_layers: usize,
    pub pitch: f64,
    pub origin_x: f64,
    pub origin_y: f64,
    pub layer_names: Vec<String>,
    /// Hard obstacles: keepouts and pre-placed wires. Never cleared.
    perm: Vec<bool>,
    /// How many nets use each cell in the current pass.
    pub occupancy: Vec<u8>,
    /// Accumulated congestion penalty across passes (PathFinder history term).
    pub history: Vec<u32>,
}

impl GridMap {
    pub fn new(pcb: &Pcb, pitch: f64) -> Self {
        let (min_x, min_y, max_x, max_y) = board_bounds(pcb);
        let margin = pitch;
        let origin_x = min_x - margin;
        let origin_y = min_y - margin;
        let width = (((max_x - min_x) + 2.0 * margin) / pitch).ceil() as usize + 1;
        let height = (((max_y - min_y) + 2.0 * margin) / pitch).ceil() as usize + 1;
        let num_layers = pcb.structure.layers.len().max(1);
        let layer_names: Vec<String> =
            pcb.structure.layers.iter().map(|l| l.name.clone()).collect();
        let total = num_layers * height * width;

        let mut grid = GridMap {
            width,
            height,
            num_layers,
            pitch,
            origin_x,
            origin_y,
            layer_names,
            perm: vec![false; total],
            occupancy: vec![0; total],
            history: vec![0; total],
        };

        for keepout in &pcb.structure.keepouts {
            grid.mark_shape_bbox_perm(keepout);
        }
        for wire in &pcb.wiring.wires {
            let layer = grid.layer_index(&wire.layer).unwrap_or(0);
            grid.mark_path_perm(&wire.path, layer);
        }

        grid
    }

    // ── Coordinate helpers ────────────────────────────────────────────────────

    pub fn world_to_grid(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let ix = ((x - self.origin_x) / self.pitch).round() as i64;
        let iy = ((y - self.origin_y) / self.pitch).round() as i64;
        if ix >= 0 && iy >= 0 && ix < self.width as i64 && iy < self.height as i64 {
            Some((ix as usize, iy as usize))
        } else {
            None
        }
    }

    pub fn grid_to_world(&self, ix: usize, iy: usize) -> (f64, f64) {
        (self.origin_x + ix as f64 * self.pitch, self.origin_y + iy as f64 * self.pitch)
    }

    pub fn layer_index(&self, name: &str) -> Option<usize> {
        self.layer_names.iter().position(|n| n == name)
    }

    #[inline(always)]
    pub fn flat(&self, ix: usize, iy: usize, layer: usize) -> usize {
        layer * self.height * self.width + iy * self.width + ix
    }

    // ── PathFinder cost ───────────────────────────────────────────────────────

    /// Traversal cost for (ix, iy, layer) under the current pass's congestion state.
    /// Returns `u32::MAX` for hard obstacles (PERM); otherwise:
    ///   1 + present_factor × occupancy + history
    #[inline]
    pub fn pf_cost(&self, ix: usize, iy: usize, layer: usize, present_factor: u32) -> u32 {
        if ix >= self.width || iy >= self.height || layer >= self.num_layers {
            return u32::MAX;
        }
        let idx = self.flat(ix, iy, layer);
        if self.perm[idx] {
            return u32::MAX;
        }
        let occ = self.occupancy[idx] as u32;
        1 + present_factor.saturating_mul(occ) + self.history[idx]
    }

    // ── Pass management ───────────────────────────────────────────────────────

    /// Clear all occupancy at the start of each PathFinder pass.
    pub fn reset_occupancy(&mut self) {
        self.occupancy.fill(0);
    }

    /// For every cell with occupancy > 1, add `h_inc` to its history penalty.
    pub fn update_history(&mut self, h_inc: u32) {
        for i in 0..self.occupancy.len() {
            if self.occupancy[i] > 1 {
                self.history[i] = self.history[i].saturating_add(h_inc);
            }
        }
    }

    /// True when no cell is shared by more than one net (DRC-legal solution).
    pub fn is_legal(&self) -> bool {
        self.occupancy.iter().all(|&o| o <= 1)
    }

    // ── Occupancy marking ─────────────────────────────────────────────────────

    /// Mark `(ix, iy, layer)` and a Manhattan-radius halo as used by one net.
    /// Call this for every cell on a routed path (including via cells on all layers).
    pub fn mark_occupancy(&mut self, ix: usize, iy: usize, layer: usize, radius: usize) {
        let r = radius as i64;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() + dy.abs() > r {
                    continue;
                }
                let nx = ix as i64 + dx;
                let ny = iy as i64 + dy;
                if nx >= 0 && ny >= 0 && nx < self.width as i64 && ny < self.height as i64 {
                    let idx = self.flat(nx as usize, ny as usize, layer);
                    if !self.perm[idx] {
                        self.occupancy[idx] = self.occupancy[idx].saturating_add(1);
                    }
                }
            }
        }
    }

    /// Temporarily zero occupancy at pad cells so the router can always reach them.
    pub fn expose_pads(&mut self, pads: &[(usize, usize)]) {
        let nl = self.num_layers;
        for &(ix, iy) in pads {
            for l in 0..nl {
                if ix < self.width && iy < self.height {
                    let idx = self.flat(ix, iy, l);
                    let is_perm = self.perm[idx];
                    if !is_perm {
                        self.occupancy[idx] = 0;
                    }
                }
            }
        }
    }

    // ── PERM marking ─────────────────────────────────────────────────────────

    pub fn set_perm(&mut self, ix: usize, iy: usize, layer: usize) {
        if ix < self.width && iy < self.height && layer < self.num_layers {
            let idx = self.flat(ix, iy, layer);
            self.perm[idx] = true;
        }
    }

    fn mark_path_perm(&mut self, path: &[(f64, f64)], layer: usize) {
        for w in path.windows(2) {
            let (x0, y0) = w[0];
            let (x1, y1) = w[1];
            let steps = (((x1 - x0).abs() + (y1 - y0).abs()) / self.pitch).ceil() as usize + 1;
            for i in 0..=steps {
                let t = if steps == 0 { 0.0 } else { i as f64 / steps as f64 };
                let x = x0 + t * (x1 - x0);
                let y = y0 + t * (y1 - y0);
                if let Some((ix, iy)) = self.world_to_grid(x, y) {
                    self.set_perm(ix, iy, layer);
                }
            }
        }
    }

    fn mark_shape_bbox_perm(&mut self, shape: &Shape) {
        let pts = shape_points(shape);
        if pts.is_empty() {
            return;
        }
        let min_x = pts.iter().map(|(x, _)| *x).fold(f64::MAX, f64::min);
        let min_y = pts.iter().map(|(_, y)| *y).fold(f64::MAX, f64::min);
        let max_x = pts.iter().map(|(x, _)| *x).fold(f64::MIN, f64::max);
        let max_y = pts.iter().map(|(_, y)| *y).fold(f64::MIN, f64::max);

        let ix0 = (((min_x - self.origin_x) / self.pitch).floor() as i64).max(0) as usize;
        let iy0 = (((min_y - self.origin_y) / self.pitch).floor() as i64).max(0) as usize;
        let ix1 =
            ((((max_x - self.origin_x) / self.pitch).ceil() as i64).min(self.width as i64 - 1))
                as usize;
        let iy1 =
            ((((max_y - self.origin_y) / self.pitch).ceil() as i64).min(self.height as i64 - 1))
                as usize;

        for layer in 0..self.num_layers {
            for iy in iy0..=iy1 {
                for ix in ix0..=ix1 {
                    self.set_perm(ix, iy, layer);
                }
            }
        }
    }
}

fn shape_points(shape: &Shape) -> Vec<(f64, f64)> {
    match shape {
        Shape::Path { coords, .. } | Shape::Polygon { coords, .. } => coords.clone(),
        Shape::Rect { x1, y1, x2, y2, .. } => {
            vec![(*x1, *y1), (*x2, *y1), (*x2, *y2), (*x1, *y2)]
        }
        Shape::Circle { cx, cy, diameter, .. } => {
            let r = diameter / 2.0;
            (0..16)
                .map(|i| {
                    let a = i as f64 * std::f64::consts::TAU / 16.0;
                    (cx + r * a.cos(), cy + r * a.sin())
                })
                .collect()
        }
    }
}

fn board_bounds(pcb: &Pcb) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    if let Some(boundary) = &pcb.structure.boundary {
        for (x, y) in shape_points(boundary) {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    for cg in &pcb.placement.components {
        for p in &cg.places {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
    }
    if min_x == f64::MAX {
        (0.0, 0.0, 10000.0, 10000.0)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}
