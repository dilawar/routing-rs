use dsn_parser::Pcb;
use std::collections::HashMap;

/// Build map from "RefDes-PinName" to world coordinates (x, y).
pub fn build_pad_positions(pcb: &Pcb) -> HashMap<String, (f64, f64)> {
    let mut map = HashMap::new();

    for comp_group in &pcb.placement.components {
        let Some(image) = pcb.library.images.iter().find(|img| img.name == comp_group.image)
        else {
            continue;
        };

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
