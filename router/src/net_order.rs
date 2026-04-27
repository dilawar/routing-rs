use dsn_parser::pcb::Net;
use std::collections::HashMap;

/// Sort nets by bounding-box diagonal of known pad positions (shortest first).
/// `filter` selects which nets should be included.
pub fn order_net_indices<F>(
    nets: &[Net],
    pad_positions: &HashMap<String, (f64, f64)>,
    filter: F,
) -> Vec<usize>
where
    F: Fn(&Net) -> bool,
{
    let mut scored: Vec<(usize, f64)> = nets
        .iter()
        .enumerate()
        .filter(|(_, net)| filter(net))
        .filter_map(|(i, net)| {
            let positions: Vec<(f64, f64)> = net
                .pins
                .iter()
                .filter_map(|p| pad_positions.get(p.as_str()).copied())
                .collect();
            if positions.len() < 2 {
                return None;
            }
            let min_x = positions.iter().map(|(x, _)| *x).fold(f64::MAX, f64::min);
            let min_y = positions.iter().map(|(_, y)| *y).fold(f64::MAX, f64::min);
            let max_x = positions.iter().map(|(x, _)| *x).fold(f64::MIN, f64::max);
            let max_y = positions.iter().map(|(_, y)| *y).fold(f64::MIN, f64::max);
            let diag = ((max_x - min_x).powi(2) + (max_y - min_y).powi(2)).sqrt();
            Some((i, diag))
        })
        .collect();

    scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().map(|(i, _)| i).collect()
}

/// Keep the old `order_nets` signature for any external callers.
pub fn order_nets<'a>(
    nets: &'a [Net],
    pad_positions: &HashMap<String, (f64, f64)>,
) -> Vec<&'a Net> {
    let indices = order_net_indices(nets, pad_positions, |n| n.pins.len() >= 2);
    indices.into_iter().map(|i| &nets[i]).collect()
}
