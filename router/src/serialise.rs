use dsn_parser::pcb::Wiring;

/// Format a Wiring as a DSN `(wiring ...)` section.
pub fn format_wiring(wiring: &Wiring) -> String {
    let mut out = String::from("(wiring\n");

    for wire in &wiring.wires {
        if wire.path.len() < 2 {
            continue;
        }
        let net_name = wire.net.as_deref().unwrap_or("");
        let mut path_str = format!("(path {} {}", wire.layer, wire.width as i64);
        for (x, y) in &wire.path {
            path_str.push_str(&format!(" {} {}", *x as i64, *y as i64));
        }
        path_str.push(')');
        out.push_str(&format!(
            "  (wire {} (net \"{}\") (type route))\n",
            path_str, net_name
        ));
    }

    for via in &wiring.vias {
        let net_name = via.net.as_deref().unwrap_or("");
        out.push_str(&format!(
            "  (via \"{}\" {} {} (net \"{}\") (type route))\n",
            via.padstack, via.x as i64, via.y as i64, net_name
        ));
    }

    out.push(')');
    out
}

/// Replace or insert the `(wiring ...)` section in the original DSN text.
pub fn write_wiring(original_dsn: &str, wiring: &Wiring) -> String {
    let new_section = format_wiring(wiring);

    // Find the outermost `(wiring` block using paren depth tracking.
    if let Some((start, end)) = find_wiring_section(original_dsn) {
        let mut result = String::with_capacity(original_dsn.len());
        result.push_str(&original_dsn[..start]);
        result.push_str(&new_section);
        result.push_str(&original_dsn[end + 1..]);
        result
    } else {
        // No existing wiring section — insert before the final closing paren of the file.
        if let Some(pos) = original_dsn.rfind(')') {
            let mut result = String::with_capacity(original_dsn.len() + new_section.len() + 2);
            result.push_str(&original_dsn[..pos]);
            result.push('\n');
            result.push_str(&new_section);
            result.push('\n');
            result.push(')');
            result
        } else {
            // Fallback: append
            format!("{}\n{}", original_dsn, new_section)
        }
    }
}

/// Locate the byte range [start, end] of the `(wiring ...)` block.
fn find_wiring_section(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 8 < bytes.len() {
        // Look for `(wiring` at depth 1 (just inside the root `(pcb ...)`)
        if bytes[i] == b'(' {
            let rest = &text[i..];
            if rest.starts_with("(wiring") {
                // Check it's `(wiring` followed by whitespace or `)`
                let after = rest.as_bytes().get(7).copied();
                if matches!(after, Some(b' ') | Some(b'\n') | Some(b'\r') | Some(b'\t') | Some(b')')) {
                    // Find matching close paren
                    let mut depth = 0usize;
                    let mut j = i;
                    let mut in_string = false;
                    while j < bytes.len() {
                        match bytes[j] {
                            b'"' if !in_string => in_string = true,
                            b'"' if in_string => in_string = false,
                            b'(' if !in_string => depth += 1,
                            b')' if !in_string => {
                                depth -= 1;
                                if depth == 0 {
                                    return Some((i, j));
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsn_parser::pcb::{Wiring, Wire};

    #[test]
    fn test_write_wiring_replace() {
        let dsn = "(pcb board.dsn\n(wiring\n(wire (path F.Cu 250 0 0 1000 0) (net \"N1\") (type route))\n)\n)\n";
        let wiring = Wiring {
            wires: vec![Wire {
                layer: "F.Cu".into(),
                width: 200.0,
                path: vec![(0.0, 0.0), (2000.0, 0.0)],
                net: Some("N2".into()),
            }],
            vias: vec![],
        };
        let result = write_wiring(dsn, &wiring);
        assert!(result.contains("N2"));
        assert!(!result.contains("N1"));
        assert!(result.contains("(wiring"));
    }

    #[test]
    fn test_write_wiring_insert() {
        let dsn = "(pcb board.dsn\n(network (net \"N1\"))\n)\n";
        let wiring = Wiring {
            wires: vec![Wire {
                layer: "B.Cu".into(),
                width: 150.0,
                path: vec![(100.0, 100.0), (500.0, 100.0)],
                net: Some("N1".into()),
            }],
            vias: vec![],
        };
        let result = write_wiring(dsn, &wiring);
        assert!(result.contains("(wiring"));
        assert!(result.contains("B.Cu"));
    }
}
