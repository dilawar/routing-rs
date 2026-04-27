use std::collections::HashMap;
use dsn_parser::Pcb;
use dsn_parser::pcb::Wiring;

fn dsn_to_mm(pcb: &Pcb) -> f64 {
    match pcb.resolution_unit.to_lowercase().as_str() {
        "mil" => 0.0254,
        "um" | "μm" | "micron" => 0.001,
        "mm" => 1.0,
        "inch" | "in" => 25.4,
        _ => 0.001,
    }
}

fn kicad_layer_name(layers: &[dsn_parser::pcb::Layer], dsn_name: &str) -> String {
    // Find the layer by name and map its positional index to KiCad layer numbers.
    let n = layers.len();
    for (pos, layer) in layers.iter().enumerate() {
        if layer.name == dsn_name {
            if n <= 1 {
                return "F.Cu".to_string();
            }
            if pos == 0 {
                return "F.Cu".to_string();
            }
            if pos == n - 1 {
                return "B.Cu".to_string();
            }
            return format!("In{}.Cu", pos);
        }
    }
    dsn_name.to_string()
}

fn kicad_layer_id(layers: &[dsn_parser::pcb::Layer], dsn_name: &str) -> u32 {
    let n = layers.len();
    for (pos, layer) in layers.iter().enumerate() {
        if layer.name == dsn_name {
            if n <= 1 {
                return 0;
            }
            if pos == 0 {
                return 0;
            }
            if pos == n - 1 {
                return 31;
            }
            return pos as u32;
        }
    }
    0
}

pub fn to_kicad_pcb(pcb: &Pcb, wiring: &Wiring) -> String {
    let scale = dsn_to_mm(pcb);
    let layers = &pcb.structure.layers;

    let net_index: HashMap<&str, usize> = pcb
        .network
        .nets
        .iter()
        .enumerate()
        .map(|(i, n)| (n.name.as_str(), i + 1))
        .collect();

    let mut out = String::with_capacity(65536);

    out.push_str("(kicad_pcb (version 20211014) (generator \"dsn-router\")\n");
    out.push_str("  (general (thickness 1.6))\n");
    out.push_str("  (paper \"A4\")\n");

    // Layer table
    out.push_str("  (layers\n");
    let n = layers.len();
    for (pos, layer) in layers.iter().enumerate() {
        let kicad_id = if n <= 1 {
            0u32
        } else if pos == 0 {
            0
        } else if pos == n - 1 {
            31
        } else {
            pos as u32
        };
        let kicad_name = if n <= 1 {
            "F.Cu".to_string()
        } else if pos == 0 {
            "F.Cu".to_string()
        } else if pos == n - 1 {
            "B.Cu".to_string()
        } else {
            layer.name.clone()
        };
        out.push_str(&format!(
            "    ({} \"{}\" signal)\n",
            kicad_id, kicad_name
        ));
    }
    out.push_str("  )\n");

    // Net declarations
    out.push_str("  (net 0 \"\")\n");
    for (i, net) in pcb.network.nets.iter().enumerate() {
        out.push_str(&format!("  (net {} \"{}\")\n", i + 1, net.name));
    }

    // Wire segments
    for wire in &wiring.wires {
        if wire.path.len() < 2 {
            continue;
        }
        let w_mm = wire.width * scale;
        let layer_name = kicad_layer_name(layers, &wire.layer);
        let layer_id = kicad_layer_id(layers, &wire.layer);
        let net_id = wire
            .net
            .as_deref()
            .and_then(|n| net_index.get(n))
            .copied()
            .unwrap_or(0);

        for pair in wire.path.windows(2) {
            let (x1, y1) = pair[0];
            let (x2, y2) = pair[1];
            out.push_str(&format!(
                "  (segment (start {:.6} {:.6}) (end {:.6} {:.6}) (width {:.6}) (layer \"{}\") (net {}))\n",
                x1 * scale,
                y1 * scale,
                x2 * scale,
                y2 * scale,
                w_mm,
                layer_name,
                net_id,
            ));
        }
        let _ = layer_id;
    }

    // Vias
    for via in &wiring.vias {
        let x_mm = via.x * scale;
        let y_mm = via.y * scale;
        let net_id = via
            .net
            .as_deref()
            .and_then(|n| net_index.get(n))
            .copied()
            .unwrap_or(0);
        let front = kicad_layer_name(layers, layers.first().map(|l| l.name.as_str()).unwrap_or("F.Cu"));
        let back = kicad_layer_name(layers, layers.last().map(|l| l.name.as_str()).unwrap_or("B.Cu"));
        out.push_str(&format!(
            "  (via (at {:.6} {:.6}) (size 0.8) (drill 0.4) (layers \"{}\" \"{}\") (net {}))\n",
            x_mm, y_mm, front, back, net_id,
        ));
    }

    out.push_str(")\n");
    out
}

pub fn to_gerber_layers(pcb: &Pcb, wiring: &Wiring) -> Vec<(String, String)> {
    let scale = dsn_to_mm(pcb);
    let layers = &pcb.structure.layers;

    // Group wires by DSN layer name.
    let mut layer_wires: HashMap<String, Vec<&dsn_parser::pcb::Wire>> = HashMap::new();
    for wire in &wiring.wires {
        if wire.path.len() >= 2 {
            layer_wires.entry(wire.layer.clone()).or_default().push(wire);
        }
    }

    let mut result = Vec::new();

    // Emit one Gerber file per layer that has at least one wire.
    // Preserve DSN layer order so output is deterministic.
    let layer_names: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        let mut ordered: Vec<String> = Vec::new();
        // First, layers in structure order.
        for layer in layers {
            if layer_wires.contains_key(&layer.name) && seen.insert(layer.name.clone()) {
                ordered.push(layer.name.clone());
            }
        }
        // Then any wire layers not listed in structure (shouldn't normally happen).
        for name in layer_wires.keys() {
            if seen.insert(name.clone()) {
                ordered.push(name.clone());
            }
        }
        ordered
    };

    for dsn_name in &layer_names {
        let wires = match layer_wires.get(dsn_name) {
            Some(w) => w,
            None => continue,
        };

        let kicad_name = kicad_layer_name(layers, dsn_name);

        // Collect distinct widths for aperture table.
        let mut widths: Vec<f64> = Vec::new();
        for wire in wires {
            let w = (wire.width * scale * 1e6).round() / 1e6;
            if !widths.iter().any(|&x| (x - w).abs() < 1e-9) {
                widths.push(w);
            }
        }
        widths.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let aperture_map: HashMap<u64, usize> = widths
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let key = (w * 1e6).round() as u64;
                (key, i + 10)
            })
            .collect();

        let mut content = String::with_capacity(32768);
        content.push_str(&format!("G04 Layer: {}*\n", kicad_name));
        content.push_str("%FSLAX46Y46*%\n");
        content.push_str("%MOMM*%\n");
        content.push_str("%LPD*%\n");
        content.push_str("G01*\n");

        // Aperture definitions
        for (i, &w) in widths.iter().enumerate() {
            content.push_str(&format!("%ADD{}C,{:.6}*%\n", i + 10, w));
        }

        let mut current_aperture: Option<usize> = None;

        for wire in wires {
            let w_mm = wire.width * scale;
            let w_key = (w_mm * 1e6).round() as u64;
            let aperture = aperture_map[&w_key];

            if current_aperture != Some(aperture) {
                content.push_str(&format!("D{}*\n", aperture));
                current_aperture = Some(aperture);
            }

            let mut first = true;
            for &(x, y) in &wire.path {
                let gx = (x * scale * 1_000_000.0).round() as i64;
                let gy = (y * scale * 1_000_000.0).round() as i64;
                if first {
                    content.push_str(&format!("X{:07}Y{:07}D02*\n", gx, gy));
                    first = false;
                } else {
                    content.push_str(&format!("X{:07}Y{:07}D01*\n", gx, gy));
                }
            }
        }

        content.push_str("M02*\n");

        let filename = format!("{}.gbr", dsn_name);
        result.push((filename, content));
    }

    result
}
