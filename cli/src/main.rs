use argh::FromArgs;
use std::path::Path;

#[derive(FromArgs)]
/// Parse a DSN file, optionally route it and write the result.
struct Cli {
    /// DSN input file.
    #[argh(positional)]
    infile: String,

    /// output DSN file; if given, the board is auto-routed before writing.
    #[argh(option, short = 'o')]
    output: Option<String>,

    /// render the board (post-routing if -o is also given) to an SVG file.
    #[argh(option)]
    svg: Option<String>,

    /// export routed board to a KiCad PCB (.kicad_pcb) file.
    #[argh(option)]
    kicad: Option<String>,

    /// export routed board as Gerber files into this directory.
    #[argh(option)]
    gerber_dir: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let cli: Cli = argh::from_env();
    let infile = Path::new(&cli.infile);
    anyhow::ensure!(infile.exists(), "{infile:?} doesn't exist");

    let src = std::fs::read_to_string(infile)?;
    let mut pcb = dsn_parser::pcb::parse_dsn(&src)?;

    if let Some(outpath) = &cli.output {
        eprintln!(
            "Routing {} nets on {} layers…",
            pcb.network.nets.len(),
            pcb.structure.layers.len()
        );
        let wiring = router::route(&pcb, Default::default(), None)?;
        eprintln!("Done: {} wires, {} vias", wiring.wires.len(), wiring.vias.len());
        let routed_dsn = router::serialise::write_wiring(&src, &wiring);
        std::fs::write(outpath, &routed_dsn)?;
        eprintln!("Written to {outpath}");
        pcb.wiring = wiring;
    }

    if let Some(svg_path) = &cli.svg {
        let svg = render_svg(&pcb);
        std::fs::write(svg_path, svg)?;
        eprintln!("SVG written to {svg_path}");
    }

    if let Some(kicad_path) = &cli.kicad {
        let content = router::export::to_kicad_pcb(&pcb, &pcb.wiring);
        std::fs::write(kicad_path, &content)?;
        eprintln!("KiCad PCB written to {kicad_path}");
    }

    if let Some(gerber_dir) = &cli.gerber_dir {
        let dir = std::path::Path::new(gerber_dir);
        std::fs::create_dir_all(dir)?;
        let layers = router::export::to_gerber_layers(&pcb, &pcb.wiring);
        if layers.is_empty() {
            eprintln!("No routed wires — no Gerber files written.");
        } else {
            for (name, content) in &layers {
                std::fs::write(dir.join(name), content)?;
                eprintln!("  {name}");
            }
            eprintln!("Gerber files written to {gerber_dir}");
        }
    }

    if cli.output.is_none() && cli.svg.is_none() && cli.kicad.is_none() && cli.gerber_dir.is_none() {
        println!("{pcb:?}");
    }

    Ok(())
}

// ─── SVG renderer ────────────────────────────────────────────────────────────

const LAYER_COLORS: &[&str] = &[
    "#c83c3c", "#3c78dc", "#50c850", "#dcb43c",
    "#b450dc", "#3cc8c8", "#dc7828", "#a0a0a0",
];

fn tag(name: &str, attrs: &[(&str, String)], content: Option<&str>) -> String {
    let a = attrs
        .iter()
        .map(|(k, v)| format!(" {}=\"{}\"", k, v))
        .collect::<String>();
    match content {
        Some(c) => format!("<{name}{a}>{c}</{name}>"),
        None => format!("<{name}{a}/>"),
    }
}

fn render_svg(pcb: &dsn_parser::Pcb) -> String {
    let (min_x, min_y, max_x, max_y) = board_bounds(pcb);
    let w = max_x - min_x;
    let h = max_y - min_y;
    let margin = (w + h) * 0.03;
    let vx = min_x - margin;
    let vy = min_y - margin;
    let vw = w + 2.0 * margin;
    let vh = h + 2.0 * margin;

    // DSN Y increases upward; SVG Y increases downward — flip.
    let tx = |x: f64| x - vx;
    let ty = |y: f64| vh - (y - vy);

    let img_h = (1200.0 * vh / vw) as u32;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" \
         viewBox=\"0 0 {vw:.1} {vh:.1}\" width=\"1200\" height=\"{img_h}\">"
    );

    out.push_str(&tag("rect", &[
        ("width", "100%".into()), ("height", "100%".into()),
        ("fill", "#141414".into()),
    ], None));

    // Board fill + outline
    if let Some(boundary) = &pcb.structure.boundary {
        let pts = shape_points(boundary);
        if pts.len() >= 3 {
            let d = path_d(&pts, tx, ty);
            out.push_str(&tag("path", &[
                ("d", d), ("fill", "#1e3c1e".into()),
                ("stroke", "#b4dcb4".into()), ("stroke-width", "0.5".into()),
            ], None));
        }
    }

    // Keepouts
    for ko in &pcb.structure.keepouts {
        let pts = shape_points(ko);
        if pts.len() >= 2 {
            let d = path_d(&pts, tx, ty);
            out.push_str(&tag("path", &[
                ("d", d), ("fill", "none".into()),
                ("stroke", "#c85050".into()), ("stroke-width", "0.4".into()),
                ("stroke-dasharray", "2,1".into()),
            ], None));
        }
    }

    // Wires
    for wire in &pcb.wiring.wires {
        if wire.path.len() < 2 { continue; }
        let color = layer_color(&wire.layer, &pcb.structure.layers);
        let sw = (wire.width as f64).max(vw * 0.003);
        let d = path_d(&wire.path, tx, ty);
        out.push_str(&tag("path", &[
            ("d", d), ("fill", "none".into()),
            ("stroke", color.into()), ("stroke-width", format!("{sw:.2}")),
            ("stroke-linecap", "round".into()), ("stroke-linejoin", "round".into()),
        ], None));
    }

    // Vias
    let via_r = (vw * 0.006).max(1.5);
    for via in &pcb.wiring.vias {
        out.push_str(&tag("circle", &[
            ("cx", format!("{:.2}", tx(via.x))),
            ("cy", format!("{:.2}", ty(via.y))),
            ("r", format!("{via_r:.2}")),
            ("fill", "#ffffff".into()), ("stroke", "#888888".into()),
            ("stroke-width", "0.5".into()),
        ], None));
    }

    // Pads
    let pad_r = (vw * 0.005).max(1.0);
    for cg in &pcb.placement.components {
        let Some(image) = pcb.library.images.iter().find(|i| i.name == cg.image) else {
            continue;
        };
        for placed in &cg.places {
            let rot = placed.rotation.to_radians();
            let (sr, cr) = (rot.sin(), rot.cos());
            for pin in &image.pins {
                let px = placed.x + pin.x * cr - pin.y * sr;
                let py = placed.y + pin.x * sr + pin.y * cr;
                out.push_str(&tag("circle", &[
                    ("cx", format!("{:.2}", tx(px))),
                    ("cy", format!("{:.2}", ty(py))),
                    ("r", format!("{pad_r:.2}")),
                    ("fill", "#dcb428".into()), ("stroke", "#000000".into()),
                    ("stroke-width", "0.3".into()),
                ], None));
            }
        }
    }

    out.push_str("</svg>");
    out
}

fn path_d(pts: &[(f64, f64)], tx: impl Fn(f64) -> f64, ty: impl Fn(f64) -> f64) -> String {
    pts.iter()
        .enumerate()
        .map(|(i, (x, y))| {
            if i == 0 {
                format!("M{:.2},{:.2}", tx(*x), ty(*y))
            } else {
                format!("L{:.2},{:.2}", tx(*x), ty(*y))
            }
        })
        .collect::<String>()
        + "Z"
}

fn layer_color(name: &str, layers: &[dsn_parser::Layer]) -> &'static str {
    let idx = layers.iter().position(|l| l.name == name).unwrap_or(0);
    LAYER_COLORS[idx % LAYER_COLORS.len()]
}

fn shape_points(shape: &dsn_parser::pcb::Shape) -> Vec<(f64, f64)> {
    use dsn_parser::pcb::Shape;
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

fn board_bounds(pcb: &dsn_parser::Pcb) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    let mut acc = |x: f64, y: f64| {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    };

    // Prefer the board boundary outline — it gives the tightest crop.
    // Only fall back to component/wire positions when no boundary is present.
    if let Some(b) = &pcb.structure.boundary {
        for (x, y) in shape_points(b) { acc(x, y); }
        // Include wires so routed traces outside the outline still fit.
        for wire in &pcb.wiring.wires {
            for (x, y) in &wire.path { acc(*x, *y); }
        }
    } else {
        for cg in &pcb.placement.components {
            for p in &cg.places { acc(p.x, p.y); }
        }
        for wire in &pcb.wiring.wires {
            for (x, y) in &wire.path { acc(*x, *y); }
        }
    }
    if min_x == f64::MAX { (0.0, 0.0, 1000.0, 1000.0) } else { (min_x, min_y, max_x, max_y) }
}
