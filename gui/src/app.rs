use dsn_parser::{pcb::Shape, Pcb};
use egui::{Color32, Painter, Pos2, Rect, Sense, Stroke, Vec2};
use router::ProgressEvent;
use std::sync::mpsc::Receiver;

// ─── Colour scheme ───────────────────────────────────────────────────────────

const BG_COLOR: Color32 = Color32::from_rgb(20, 20, 20);
const BOARD_COLOR: Color32 = Color32::from_rgb(30, 60, 30);
const LAYER_COLORS: &[Color32] = &[
    Color32::from_rgb(200, 60, 60),   // F.Cu  – red
    Color32::from_rgb(60, 120, 220),  // B.Cu  – blue
    Color32::from_rgb(80, 200, 80),   // inner – green
    Color32::from_rgb(220, 180, 60),  // inner – yellow
    Color32::from_rgb(180, 80, 220),  // inner – purple
    Color32::from_rgb(60, 200, 200),  // inner – cyan
    Color32::from_rgb(220, 120, 40),  // inner – orange
    Color32::from_rgb(160, 160, 160), // inner – grey
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

    // routing
    route_rx: Option<Receiver<ProgressEvent>>,
    in_progress_wires: Vec<dsn_parser::Wire>,
    in_progress_vias: Vec<dsn_parser::PlacedVia>,
    route_progress: f32,
    route_done: usize,
    route_total: usize,
    route_status: String,
}

impl DsnViewerApp {
    pub fn with_file(path: Option<std::path::PathBuf>) -> Self {
        let mut app = Self {
            show_ratsnest: true,
            show_keepouts: true,
            show_component_labels: true,
            ..Default::default()
        };
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
                self.route_rx = None;
                self.in_progress_wires.clear();
                self.in_progress_vias.clear();
                self.route_status.clear();
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
        // Only set defaults on first load; preserve user prefs on reload.
        if self.pcb.is_none() {
            self.show_ratsnest = true;
            self.show_keepouts = true;
            self.show_component_labels = true;
        }
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

    fn to_screen(&self, canvas_origin: Pos2, x: f64, y: f64) -> Pos2 {
        Pos2::new(
            canvas_origin.x + self.pan.x + (x as f32) * self.zoom,
            canvas_origin.y + self.pan.y + (y as f32) * self.zoom,
        )
    }

    fn fit_to(&mut self, rect: Rect, pcb: &Pcb) {
        let (min_x, min_y, max_x, max_y) = board_bounds(pcb);
        if max_x <= min_x || max_y <= min_y {
            return;
        }
        self.zoom = (rect.width() * 0.9 / (max_x - min_x) as f32)
            .min(rect.height() * 0.9 / (max_y - min_y) as f32);
        self.pan = Vec2::new(
            rect.width() * 0.05 - min_x as f32 * self.zoom,
            rect.height() * 0.05 - min_y as f32 * self.zoom,
        );
    }

    fn draw_pcb(&self, painter: &Painter, origin: Pos2, pcb: &Pcb) {
        if let Some(boundary) = &pcb.structure.boundary {
            self.fill_board(painter, origin, boundary);
        }

        if self.show_keepouts {
            for ko in &pcb.structure.keepouts {
                self.draw_shape_outline(painter, origin, ko, KEEPOUT_COLOR, 0.5);
            }
        }

        // Committed wires
        for wire in &pcb.wiring.wires {
            if let Some(color) = self.layer_color(&wire.layer) {
                draw_polyline(
                    painter,
                    wire.path.iter().map(|(x, y)| self.to_screen(origin, *x, *y)),
                    Stroke::new((wire.width as f32 * self.zoom).max(1.0), color),
                );
            }
        }

        // In-progress wires from routing thread (dimmed)
        for wire in &self.in_progress_wires {
            if let Some(color) = self.layer_color(&wire.layer) {
                draw_polyline(
                    painter,
                    wire.path.iter().map(|(x, y)| self.to_screen(origin, *x, *y)),
                    Stroke::new(
                        (wire.width as f32 * self.zoom).max(1.0),
                        color.gamma_multiply(0.55),
                    ),
                );
            }
        }

        for via in &pcb.wiring.vias {
            let pos = self.to_screen(origin, via.x, via.y);
            painter.circle_filled(pos, (3.0 * self.zoom).max(2.0), Color32::WHITE);
        }
        for via in &self.in_progress_vias {
            let pos = self.to_screen(origin, via.x, via.y);
            painter.circle_filled(
                pos,
                (3.0 * self.zoom).max(2.0),
                Color32::WHITE.gamma_multiply(0.55),
            );
        }

        self.draw_pads(painter, origin, pcb);

        if self.show_ratsnest {
            self.draw_ratsnest(painter, origin, pcb);
        }

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
            screen.push(screen[0]);
            painter.add(egui::Shape::line(screen, Stroke::new(width_px, color)));
        }
    }

    fn draw_pads(&self, painter: &Painter, origin: Pos2, pcb: &Pcb) {
        for comp_group in &pcb.placement.components {
            let Some(image) = pcb.library.images.iter().find(|i| i.name == comp_group.image)
            else {
                continue;
            };
            for placed in &comp_group.places {
                let (cx, cy) = (placed.x, placed.y);
                let rot_rad = placed.rotation.to_radians();
                let (sin_r, cos_r) = (rot_rad.sin(), rot_rad.cos());

                for pin in &image.pins {
                    let pos = self.to_screen(
                        origin,
                        cx + pin.x * cos_r - pin.y * sin_r,
                        cy + pin.x * sin_r + pin.y * cos_r,
                    );
                    painter.circle_filled(pos, (2.5 * self.zoom).max(2.0), PAD_COLOR);
                    painter.circle_stroke(
                        pos,
                        (2.5 * self.zoom).max(2.0),
                        Stroke::new(0.5, Color32::BLACK),
                    );
                }

                if self.show_component_labels {
                    painter.text(
                        self.to_screen(origin, cx, cy),
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
        let mut routed_wire_count: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for w in pcb.wiring.wires.iter().chain(&self.in_progress_wires) {
            if let Some(n) = w.net.as_deref() {
                *routed_wire_count.entry(n).or_insert(0) += 1;
            }
        }

        let pad_positions = router::pad_map::build_pad_positions(pcb);
        for net in &pcb.network.nets {
            if net.pins.len() < 2 {
                continue;
            }
            let wire_count = routed_wire_count.get(net.name.as_str()).copied().unwrap_or(0);
            if wire_count >= net.pins.len() - 1 {
                continue;
            }
            let positions: Vec<Pos2> = net
                .pins
                .iter()
                .filter_map(|p| pad_positions.get(p.as_str()))
                .map(|(x, y)| self.to_screen(origin, *x, *y))
                .collect();
            if positions.len() < 2 {
                continue;
            }
            for other in &positions[1..] {
                painter.line_segment([positions[0], *other], Stroke::new(0.5, RATSNEST_COLOR));
            }
        }
    }

    fn clear_routing(&mut self) {
        if let Some(pcb) = &mut self.pcb {
            pcb.wiring = dsn_parser::pcb::Wiring::default();
        }
        self.route_rx = None;
        self.in_progress_wires.clear();
        self.in_progress_vias.clear();
        self.route_progress = 0.0;
        self.route_done = 0;
        self.route_status.clear();
    }

    /// Drain routing thread events; returns true when the display needs a repaint.
    fn poll_routing(&mut self) -> bool {
        let Some(rx) = &self.route_rx else { return false };
        let mut changed = false;
        loop {
            match rx.try_recv() {
                Ok(event) => {
                    changed = true;
                    match event {
                        ProgressEvent::StartNet { name, idx, total } => {
                            self.route_total = total;
                            self.route_done = idx;
                            self.route_progress =
                                if total > 0 { idx as f32 / total as f32 } else { 0.0 };
                            self.route_status = format!("Routing {name} ({}/{total})", idx + 1);
                        }
                        ProgressEvent::NetRouted { wires, vias, .. } => {
                            self.in_progress_wires.extend(wires);
                            self.in_progress_vias.extend(vias);
                        }
                        ProgressEvent::NetFailed { .. } => {}
                        ProgressEvent::PassComplete { pass, routed, total } => {
                            self.route_done = routed;
                            self.route_total = total;
                            self.route_progress =
                                if total > 0 { routed as f32 / total as f32 } else { 0.0 };
                            self.route_status = if routed < total {
                                format!("Pass {pass}: {routed}/{total} — refining…")
                            } else {
                                format!("Pass {pass}: all {total} nets routed")
                            };
                        }
                        ProgressEvent::Finished { wiring } => {
                            let (wc, vc) = (wiring.wires.len(), wiring.vias.len());
                            if let Some(pcb) = &mut self.pcb {
                                pcb.wiring = wiring;
                            }
                            self.route_rx = None;
                            self.in_progress_wires.clear();
                            self.in_progress_vias.clear();
                            self.route_progress = 1.0;
                            self.route_done = self.route_total;
                            self.route_status = format!("Done — {wc} wires, {vc} vias");
                            break;
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.route_rx = None;
                    self.route_status = "Router thread exited".to_string();
                    break;
                }
            }
        }
        changed
    }
}

// ─── eframe App impl ─────────────────────────────────────────────────────────

impl eframe::App for DsnViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.poll_routing() || self.route_rx.is_some() {
            ctx.request_repaint();
        }

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
                ui.label(format!(
                    "Components: {}",
                    pcb.placement.components.iter().map(|c| c.places.len()).sum::<usize>()
                ));
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

                ui.separator();

                if self.route_rx.is_some() {
                    // ── Routing in progress ──
                    ui.label(self.route_status.as_str());
                    ui.add(
                        egui::ProgressBar::new(self.route_progress)
                            .text(format!("{}/{}", self.route_done, self.route_total))
                            .animate(true),
                    );
                    if ui.button("Cancel").clicked() {
                        self.route_rx = None;
                        self.in_progress_wires.clear();
                        self.in_progress_vias.clear();
                        self.route_status = "Cancelled".to_string();
                    }
                } else {
                    // ── Idle ──
                    let has_wires =
                        !pcb.wiring.wires.is_empty() || !pcb.wiring.vias.is_empty();

                    if ui.button("Auto-Route").clicked() {
                        let pcb_clone = pcb.clone();
                        let (tx, rx) = std::sync::mpsc::sync_channel(128);
                        self.route_rx = Some(rx);
                        self.in_progress_wires.clear();
                        self.in_progress_vias.clear();
                        self.route_progress = 0.0;
                        self.route_done = 0;
                        self.route_status = "Starting…".to_string();
                        let ctx2 = ctx.clone();
                        std::thread::spawn(move || {
                            let _ = router::route(&pcb_clone, Default::default(), Some(&tx));
                            ctx2.request_repaint();
                        });
                    }

                    if !self.route_status.is_empty() {
                        ui.label(
                            egui::RichText::new(self.route_status.as_str())
                                .color(Color32::from_rgb(100, 200, 100)),
                        );
                    }

                    // Export — shown when wires exist; must come before any
                    // &mut self calls (clear_routing) so the pcb borrow can end here.
                    if has_wires {
                        ui.separator();
                        ui.label("Export");
                        if ui.button("Export KiCad PCB…").clicked() {
                            let pcb_clone = pcb.clone();
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("KiCad PCB", &["kicad_pcb"])
                                .set_file_name("routed.kicad_pcb")
                                .save_file()
                            {
                                let content = router::export::to_kicad_pcb(
                                    &pcb_clone,
                                    &pcb_clone.wiring,
                                );
                                if let Err(e) = std::fs::write(&path, content) {
                                    self.error = Some(format!("Export failed: {e}"));
                                }
                            }
                        }
                        if ui.button("Export Gerber…").clicked() {
                            let pcb_clone = pcb.clone();
                            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                let layers = router::export::to_gerber_layers(
                                    &pcb_clone,
                                    &pcb_clone.wiring,
                                );
                                for (name, content) in layers {
                                    if let Err(e) =
                                        std::fs::write(dir.join(&name), content)
                                    {
                                        self.error =
                                            Some(format!("Export failed ({name}): {e}"));
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    // Destructive action last; pcb borrow has already ended above.
                    if has_wires && ui.button("Clear Routing").clicked() {
                        self.clear_routing();
                    }
                }
            }
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::canvas(&ctx.style()))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                let response = ui.allocate_rect(rect, Sense::drag());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 0.0, BG_COLOR);

                if self.fit_requested {
                    if let Some(pcb) = self.pcb.take() {
                        self.fit_to(rect, &pcb);
                        self.pcb = Some(pcb);
                        self.fit_requested = false;
                    }
                }

                if response.dragged() {
                    self.pan += response.drag_delta();
                }

                let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    let factor = if scroll > 0.0 { 1.1f32 } else { 1.0 / 1.1 };
                    if let Some(hover) = response.hover_pos() {
                        let before = hover - rect.min;
                        self.pan = before + (self.pan - before) * factor;
                    }
                    self.zoom *= factor;
                }

                if let Some(pcb) = &self.pcb {
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
    for wire in pcb.wiring.wires.iter() {
        for (x, y) in &wire.path {
            min_x = min_x.min(*x);
            min_y = min_y.min(*y);
            max_x = max_x.max(*x);
            max_y = max_y.max(*y);
        }
    }
    if min_x == f64::MAX { (0.0, 0.0, 1.0, 1.0) } else { (min_x, min_y, max_x, max_y) }
}
