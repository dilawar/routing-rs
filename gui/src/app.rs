use dsn_parser::{pcb::Shape, Pcb};
use egui::{Color32, Painter, Pos2, Rect, Sense, Stroke, Vec2};

// ─── Colour scheme ───────────────────────────────────────────────────────────

const BG_COLOR: Color32 = Color32::from_rgb(20, 20, 20);
const BOARD_COLOR: Color32 = Color32::from_rgb(30, 60, 30);
const LAYER_COLORS: &[Color32] = &[
    Color32::from_rgb(200, 60, 60),   // F.Cu  – red
    Color32::from_rgb(60, 120, 220),  // B.Cu  – blue
    Color32::from_rgb(80, 200, 80),   // inner – green
    Color32::from_rgb(220, 180, 60),  // inner – yellow
    Color32::from_rgb(180, 80, 220),  // inner – purple
];
const RATSNEST_COLOR: Color32 = Color32::from_rgb(160, 160, 60);
const KEEPOUT_COLOR: Color32 = Color32::from_rgb(200, 80, 80);
const PAD_COLOR: Color32 = Color32::from_rgb(220, 180, 40);
const COMPONENT_LABEL_COLOR: Color32 = Color32::from_rgb(200, 200, 200);

// ─── Layer visibility state ──────────────────────────────────────────────────

struct LayerVis {
    name: String,
    visible: bool,
    color: Color32,
}

// ─── App state ───────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct DsnViewerApp {
    pcb: Option<Pcb>,
    error: Option<String>,

    // viewport
    pan: Vec2,
    zoom: f32,
    fit_requested: bool,

    // visibility
    show_ratsnest: bool,
    show_keepouts: bool,
    show_component_labels: bool,
    layers: Vec<LayerVis>,

    last_dir: Option<std::path::PathBuf>,
}

impl DsnViewerApp {
    pub fn with_file(path: Option<std::path::PathBuf>) -> Self {
        let mut app = Self::default();
        if let Some(p) = path {
            app.load_file(&p);
        }
        app
    }

    fn load_file(&mut self, path: &std::path::Path) {
        if let Some(dir) = path.parent() {
            self.last_dir = Some(dir.to_path_buf());
        }
        match dsn_parser::parse_file_rust(&path.to_string_lossy()) {
            Ok(pcb) => {
                self.build_layer_vis(&pcb);
                self.pcb = Some(pcb);
                self.error = None;
                self.fit_requested = true;
            }
            Err(e) => {
                self.error = Some(e.to_string());
                self.pcb = None;
            }
        }
    }

    fn build_layer_vis(&mut self, pcb: &Pcb) {
        self.layers = pcb
            .structure
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| LayerVis {
                name: l.name.clone(),
                visible: true,
                color: LAYER_COLORS[i % LAYER_COLORS.len()],
            })
            .collect();
        self.show_ratsnest = true;
        self.show_keepouts = true;
        self.show_component_labels = true;
    }

    fn layer_color(&self, layer_name: &str) -> Option<Color32> {
        self.layers.iter().find_map(|lv| {
            if lv.name == layer_name && lv.visible {
                Some(lv.color)
            } else {
                None
            }
        })
    }

    /// Convert a DSN coordinate (raw integer units) to a canvas Pos2.
    fn to_screen(&self, canvas_origin: Pos2, x: f64, y: f64) -> Pos2 {
        // DSN Y axis is positive-downward in some files; keep it as-is and let
        // the user flip if needed.
        let sx = canvas_origin.x + self.pan.x + (x as f32) * self.zoom;
        let sy = canvas_origin.y + self.pan.y + (y as f32) * self.zoom;
        Pos2::new(sx, sy)
    }

    /// Fit the board into the given rect.
    fn fit_to(&mut self, rect: Rect, pcb: &Pcb) {
        let (min_x, min_y, max_x, max_y) = board_bounds(pcb);
        if max_x <= min_x || max_y <= min_y {
            return;
        }
        let board_w = (max_x - min_x) as f32;
        let board_h = (max_y - min_y) as f32;
        let scale_x = rect.width() * 0.9 / board_w;
        let scale_y = rect.height() * 0.9 / board_h;
        self.zoom = scale_x.min(scale_y);
        self.pan = Vec2::new(
            rect.width() * 0.05 - min_x as f32 * self.zoom,
            rect.height() * 0.05 - min_y as f32 * self.zoom,
        );
    }

    fn draw_pcb(&self, painter: &Painter, origin: Pos2, pcb: &Pcb) {
        // Board fill
        if let Some(boundary) = &pcb.structure.boundary {
            self.fill_board(painter, origin, boundary);
        }

        // Keepouts
        if self.show_keepouts {
            for ko in &pcb.structure.keepouts {
                self.draw_shape_outline(painter, origin, ko, KEEPOUT_COLOR, 0.5);
            }
        }

        // Wires / traces
        for wire in &pcb.wiring.wires {
            if let Some(color) = self.layer_color(&wire.layer) {
                draw_polyline(
                    painter,
                    wire.path
                        .iter()
                        .map(|(x, y)| self.to_screen(origin, *x, *y)),
                    Stroke::new((wire.width as f32 * self.zoom).max(1.0), color),
                );
            }
        }

        // Placed vias
        for via in &pcb.wiring.vias {
            let pos = self.to_screen(origin, via.x, via.y);
            painter.circle_filled(pos, (3.0 * self.zoom).max(2.0), Color32::WHITE);
        }

        // Pads
        self.draw_pads(painter, origin, pcb);

        // Ratsnest
        if self.show_ratsnest {
            self.draw_ratsnest(painter, origin, pcb);
        }

        // Boundary outline (on top of fill)
        if let Some(boundary) = &pcb.structure.boundary {
            self.draw_shape_outline(
                painter,
                origin,
                boundary,
                Color32::from_rgb(180, 220, 180),
                1.5,
            );
        }
    }

    fn fill_board(&self, painter: &Painter, origin: Pos2, shape: &Shape) {
        let points = shape_points(shape);
        if points.len() >= 3 {
            let screen: Vec<Pos2> =
                points.iter().map(|(x, y)| self.to_screen(origin, *x, *y)).collect();
            // Use PathShape so non-convex board outlines fill correctly.
            painter.add(egui::Shape::Path(egui::epaint::PathShape {
                points: screen,
                closed: true,
                fill: BOARD_COLOR,
                stroke: egui::epaint::PathStroke::NONE,
            }));
        }
    }

    fn draw_shape_outline(
        &self,
        painter: &Painter,
        origin: Pos2,
        shape: &Shape,
        color: Color32,
        width_px: f32,
    ) {
        let points = shape_points(shape);
        if points.is_empty() {
            return;
        }
        let mut screen: Vec<Pos2> =
            points.iter().map(|(x, y)| self.to_screen(origin, *x, *y)).collect();
        if screen.len() > 1 {
            screen.push(screen[0]); // close
            painter.add(egui::Shape::line(screen, Stroke::new(width_px, color)));
        }
    }

    fn draw_pads(&self, painter: &Painter, origin: Pos2, pcb: &Pcb) {
        // Build lookup: image_name → pins
        for comp_group in &pcb.placement.components {
            // Find image
            let image = pcb
                .library
                .images
                .iter()
                .find(|img| img.name == comp_group.image);
            let Some(image) = image else { continue };

            for placed in &comp_group.places {
                let cx = placed.x;
                let cy = placed.y;
                let rot_rad = placed.rotation.to_radians();
                let (sin_r, cos_r) = (rot_rad.sin(), rot_rad.cos());

                for pin in &image.pins {
                    // Rotate pin offset by component rotation
                    let rx = pin.x * cos_r - pin.y * sin_r;
                    let ry = pin.x * sin_r + pin.y * cos_r;
                    let pos = self.to_screen(origin, cx + rx, cy + ry);
                    painter.circle_filled(pos, (2.5 * self.zoom).max(2.0), PAD_COLOR);
                    painter.circle_stroke(
                        pos,
                        (2.5 * self.zoom).max(2.0),
                        Stroke::new(0.5, Color32::BLACK),
                    );
                }

                // Component label
                if self.show_component_labels {
                    let label_pos = self.to_screen(origin, cx, cy);
                    painter.text(
                        label_pos,
                        egui::Align2::CENTER_CENTER,
                        &placed.id,
                        egui::FontId::monospace((9.0 * self.zoom).clamp(7.0, 14.0)),
                        COMPONENT_LABEL_COLOR,
                    );
                }
            }
        }
    }

    fn draw_ratsnest(&self, painter: &Painter, origin: Pos2, pcb: &Pcb) {
        // Skip nets that already have traces — a net with any routed wire is
        // considered fully routed for display purposes.
        let routed: std::collections::HashSet<&str> = pcb
            .wiring
            .wires
            .iter()
            .filter_map(|w| w.net.as_deref())
            .collect();

        let pad_positions = build_pad_positions(pcb);

        for net in &pcb.network.nets {
            if net.pins.len() < 2 || routed.contains(net.name.as_str()) {
                continue;
            }
            let positions: Vec<Pos2> = net
                .pins
                .iter()
                .filter_map(|pin_ref| pad_positions.get(pin_ref.as_str()))
                .map(|(x, y)| self.to_screen(origin, *x, *y))
                .collect();

            if positions.len() < 2 {
                continue;
            }
            // Star pattern from first pad to all others
            let center = positions[0];
            for other in &positions[1..] {
                painter.line_segment(
                    [center, *other],
                    Stroke::new(0.5, RATSNEST_COLOR),
                );
            }
        }
    }
}

// ─── eframe App impl ─────────────────────────────────────────────────────────

impl eframe::App for DsnViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Sidebar
        egui::SidePanel::left("sidebar").min_width(180.0).show(ctx, |ui| {
            ui.heading("DSN Viewer");
            ui.separator();

            if ui.button("Open DSN file…").clicked() {
                let mut dialog = rfd::FileDialog::new().add_filter("DSN files", &["dsn"]);
                if let Some(dir) = &self.last_dir {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(path) = dialog.pick_file() {
                    self.load_file(&path);
                }
            }

            if let Some(err) = &self.error {
                ui.colored_label(Color32::RED, err);
            }

            if let Some(pcb) = &self.pcb {
                ui.separator();
                ui.label(format!("ID: {}", pcb.id));
                ui.label(format!("Unit: {} (×{})", pcb.resolution_unit, pcb.resolution_value));
                ui.label(format!("Nets: {}", pcb.network.nets.len()));
                ui.label(format!("Components: {}", pcb.placement.components.iter().map(|c| c.places.len()).sum::<usize>()));
                ui.label(format!("Traces: {}", pcb.wiring.wires.len()));

                ui.separator();
                ui.label("Layers");
                for lv in &mut self.layers {
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, lv.color);
                        ui.checkbox(&mut lv.visible, &lv.name);
                    });
                }

                ui.separator();
                ui.checkbox(&mut self.show_ratsnest, "Ratsnest");
                ui.checkbox(&mut self.show_keepouts, "Keepouts");
                ui.checkbox(&mut self.show_component_labels, "Labels");

                ui.separator();
                if ui.button("Fit to window").clicked() {
                    self.fit_requested = true;
                }
            }
        });

        // Canvas
        egui::CentralPanel::default()
            .frame(egui::Frame::canvas(&ctx.style()))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                let response = ui.allocate_rect(rect, Sense::drag());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 0.0, BG_COLOR);

                if self.fit_requested {
                    // Temporarily take pcb out to avoid borrow conflict.
                    if let Some(pcb) = self.pcb.take() {
                        self.fit_to(rect, &pcb);
                        self.pcb = Some(pcb);
                        self.fit_requested = false;
                    }
                }

                // Pan
                if response.dragged() {
                    self.pan += response.drag_delta();
                }

                // Zoom with scroll
                let scroll = ctx.input(|i| i.raw_scroll_delta.y);
                if scroll != 0.0 {
                    let factor = if scroll > 0.0 { 1.1f32 } else { 1.0 / 1.1 };
                    if let Some(hover) = response.hover_pos() {
                        // Zoom toward cursor: keep the board point under the cursor fixed.
                        let before: Vec2 = hover - rect.min;
                        self.pan = before + (self.pan - before) * factor;
                    }
                    self.zoom *= factor;
                }

                if let Some(pcb) = &self.pcb {
                    // Safety clone to avoid borrow conflict with self methods
                    self.draw_pcb(&painter, rect.min, pcb);
                }
            });
    }
}

// ─── Geometry helpers ────────────────────────────────────────────────────────

fn shape_points(shape: &Shape) -> Vec<(f64, f64)> {
    match shape {
        Shape::Path { coords, .. } | Shape::Polygon { coords, .. } => coords.clone(),
        Shape::Rect { x1, y1, x2, y2, .. } => {
            vec![(*x1, *y1), (*x2, *y1), (*x2, *y2), (*x1, *y2)]
        }
        Shape::Circle { cx, cy, diameter, .. } => {
            let r = diameter / 2.0;
            (0..32)
                .map(|i| {
                    let a = i as f64 * std::f64::consts::TAU / 32.0;
                    (cx + r * a.cos(), cy + r * a.sin())
                })
                .collect()
        }
    }
}

fn draw_polyline(painter: &Painter, points: impl Iterator<Item = Pos2>, stroke: Stroke) {
    let pts: Vec<Pos2> = points.collect();
    if pts.len() >= 2 {
        painter.add(egui::Shape::line(pts, stroke));
    }
}

/// Compute the bounding box of the board (from boundary or all components).
fn board_bounds(pcb: &Pcb) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    let collect = |coords: &[(f64, f64)], min_x: &mut f64, min_y: &mut f64, max_x: &mut f64, max_y: &mut f64| {
        for (x, y) in coords {
            *min_x = min_x.min(*x);
            *min_y = min_y.min(*y);
            *max_x = max_x.max(*x);
            *max_y = max_y.max(*y);
        }
    };

    if let Some(boundary) = &pcb.structure.boundary {
        collect(&shape_points(boundary), &mut min_x, &mut min_y, &mut max_x, &mut max_y);
    }

    // Always include all component positions so out-of-boundary placements don't clip.
    for cg in &pcb.placement.components {
        for p in &cg.places {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
    }

    if min_x == f64::MAX {
        (0.0, 0.0, 1.0, 1.0)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}

/// Returns map from "RefDes-PinName" → (world_x, world_y).
fn build_pad_positions(pcb: &Pcb) -> std::collections::HashMap<String, (f64, f64)> {
    let mut map = std::collections::HashMap::new();

    for comp_group in &pcb.placement.components {
        let image = pcb
            .library
            .images
            .iter()
            .find(|img| img.name == comp_group.image);
        let Some(image) = image else { continue };

        for placed in &comp_group.places {
            let cx = placed.x;
            let cy = placed.y;
            let rot_rad = placed.rotation.to_radians();
            let (sin_r, cos_r) = (rot_rad.sin(), rot_rad.cos());

            for pin in &image.pins {
                let rx = pin.x * cos_r - pin.y * sin_r;
                let ry = pin.x * sin_r + pin.y * cos_r;
                let key = format!("{}-{}", placed.id, pin.name);
                map.insert(key, (cx + rx, cy + ry));
            }
        }
    }

    map
}
