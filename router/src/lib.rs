pub mod grid;
pub mod bfs;
pub mod export;
pub mod net_order;
pub mod pad_map;
pub mod serialise;

use dsn_parser::{Pcb, Wire, PlacedVia, pcb::{Net, Wiring}};
use grid::GridMap;
use std::sync::mpsc::SyncSender;

// ─── Public API ───────────────────────────────────────────────────────────────

pub struct RouterConfig {
    /// DSN units per grid cell.
    pub grid_pitch: f64,
    /// Extra A* cost for a via (layer transition).
    pub via_cost: u32,
    /// Maximum PathFinder passes.
    pub max_pf_passes: usize,
    /// Per-pass increment to the present-congestion penalty factor.
    /// Pass N uses `present_factor = N * present_factor_step`.
    pub present_factor_step: u32,
    /// Per-pass increment to history cost for overused cells.
    pub history_increment: u32,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            grid_pitch: 100.0,
            via_cost: 15,       // discourage unnecessary layer changes
            max_pf_passes: 50,
            present_factor_step: 2,
            history_increment: 1,
        }
    }
}

pub enum ProgressEvent {
    StartNet { name: String, idx: usize, total: usize },
    NetRouted { name: String, wires: Vec<Wire>, vias: Vec<PlacedVia> },
    NetFailed { name: String },
    PassComplete { pass: usize, routed: usize, total: usize },
    Finished { wiring: Wiring },
}

// ─── Internal ─────────────────────────────────────────────────────────────────

struct DrcParams {
    default_width: f64,
    default_clearance: f64,
    default_via: String,
}

// ─── Entry point ──────────────────────────────────────────────────────────────

pub fn route(
    pcb: &Pcb,
    config: RouterConfig,
    tx: Option<&SyncSender<ProgressEvent>>,
) -> anyhow::Result<Wiring> {
    let pad_positions = pad_map::build_pad_positions(pcb);
    let mut grid = GridMap::new(pcb, config.grid_pitch);
    let drc = compute_drc(pcb);

    // Resolve pad grid coordinates per net.
    let pad_grid: Vec<Vec<(usize, usize)>> = pcb
        .network
        .nets
        .iter()
        .map(|net| {
            net.pins
                .iter()
                .filter_map(|p| pad_positions.get(p.as_str()))
                .filter_map(|&(x, y)| grid.world_to_grid(x, y))
                .collect()
        })
        .collect();

    let pre_routed: std::collections::HashSet<&str> = pcb
        .wiring
        .wires
        .iter()
        .filter_map(|w| w.net.as_deref())
        .collect();

    let sorted_indices = net_order::order_net_indices(
        &pcb.network.nets,
        &pad_positions,
        |n| !pre_routed.contains(n.name.as_str()) && n.pins.len() >= 2,
    );
    let total = sorted_indices.len();

    // Best result seen so far (most nets routed, then first legal pass).
    let mut best: Vec<Option<(Vec<Wire>, Vec<PlacedVia>)>> = vec![None; pcb.network.nets.len()];
    let mut best_routed = 0usize;
    let mut best_is_legal = false;

    for pass in 0..=config.max_pf_passes {
        grid.reset_occupancy();

        // present_factor grows each pass so congestion becomes increasingly expensive.
        let present_factor = (pass as u32).saturating_mul(config.present_factor_step);

        let mut pass_result: Vec<Option<(Vec<Wire>, Vec<PlacedVia>)>> =
            vec![None; pcb.network.nets.len()];
        let mut routed_count = 0usize;

        for (progress_idx, &net_idx) in sorted_indices.iter().enumerate() {
            let net = &pcb.network.nets[net_idx];
            let net_id = (net_idx as u32) + 1;

            if let Some(tx) = tx {
                let _ = tx.send(ProgressEvent::StartNet {
                    name: net.name.clone(),
                    idx: progress_idx,
                    total,
                });
            }

            let (wire_width, via_padstack) =
                net_class_params(pcb, &net.name, drc.default_width, &drc.default_via);
            let clearance_cells =
                ((drc.default_clearance + wire_width) / config.grid_pitch - 1.0).ceil() as usize;
            let clearance_cells = clearance_cells.max(1);

            // Ensure pad cells are reachable regardless of other nets' clearance halos.
            grid.expose_pads(&pad_grid[net_idx]);

            match route_net_steiner(
                &mut grid,
                net,
                net_id,
                &pad_grid[net_idx],
                &config,
                wire_width,
                clearance_cells,
                &via_padstack,
                present_factor,
            ) {
                Some((wires, vias)) => {
                    routed_count += 1;
                    if let Some(tx) = tx {
                        let _ = tx.send(ProgressEvent::NetRouted {
                            name: net.name.clone(),
                            wires: wires.clone(),
                            vias: vias.clone(),
                        });
                    }
                    pass_result[net_idx] = Some((wires, vias));
                }
                None => {
                    if let Some(tx) = tx {
                        let _ = tx.send(ProgressEvent::NetFailed { name: net.name.clone() });
                    }
                }
            }
        }

        let legal = grid.is_legal();

        if let Some(tx) = tx {
            let _ = tx.send(ProgressEvent::PassComplete {
                pass,
                routed: routed_count,
                total,
            });
        }

        // Keep the best result: prefer legal over illegal; among equals, more nets routed.
        let better = match (legal, best_is_legal) {
            (true, false) => true,
            (true, true) => routed_count > best_routed,
            (false, false) => routed_count > best_routed,
            (false, true) => false,
        };
        if better {
            best = pass_result;
            best_routed = routed_count;
            best_is_legal = legal;
        }

        if legal && routed_count == total {
            break;
        }

        grid.update_history(config.history_increment);
    }

    let mut wiring = Wiring {
        wires: pcb.wiring.wires.clone(),
        vias: pcb.wiring.vias.clone(),
    };
    for opt in &best {
        if let Some((wires, vias)) = opt {
            wiring.wires.extend(wires.iter().cloned());
            wiring.vias.extend(vias.iter().cloned());
        }
    }

    if let Some(tx) = tx {
        let _ = tx.send(ProgressEvent::Finished { wiring: wiring.clone() });
    }

    Ok(wiring)
}

// ─── Steiner tree routing ─────────────────────────────────────────────────────

/// Connect all pads for a net using a growing nearest-neighbor Steiner tree.
/// Returns (wires, vias) on success, or None if any segment fails to route.
fn route_net_steiner(
    grid: &mut GridMap,
    net: &Net,
    net_id: u32,
    pad_grid: &[(usize, usize)],
    config: &RouterConfig,
    wire_width: f64,
    clearance_cells: usize,
    via_padstack: &str,
    present_factor: u32,
) -> Option<(Vec<Wire>, Vec<PlacedVia>)> {
    if pad_grid.len() < 2 {
        return None;
    }

    // Sources grow as we connect pads one by one (all layers at each point).
    let mut sources: Vec<bfs::State> = (0..grid.num_layers)
        .map(|l| (pad_grid[0].0, pad_grid[0].1, l))
        .collect();

    let mut remaining: Vec<usize> = (1..pad_grid.len()).collect();
    let mut tree_pads: Vec<(usize, usize)> = vec![pad_grid[0]];
    let mut all_wires: Vec<Wire> = Vec::new();
    let mut all_vias: Vec<PlacedVia> = Vec::new();

    while !remaining.is_empty() {
        // Connect the unvisited pad closest to the current Steiner tree.
        let next_pos = remaining
            .iter()
            .enumerate()
            .min_by_key(|(_, &ri)| {
                let (tx, ty) = pad_grid[ri];
                tree_pads
                    .iter()
                    .map(|&(sx, sy)| {
                        let dx = tx as i64 - sx as i64;
                        let dy = ty as i64 - sy as i64;
                        dx * dx + dy * dy
                    })
                    .min()
                    .unwrap_or(i64::MAX)
            })
            .map(|(pos, _)| pos)
            .unwrap();

        let ri = remaining.remove(next_pos);
        let target = pad_grid[ri];

        grid.expose_pads(&[target]);

        let result = bfs::route_net(
            grid,
            &sources,
            &[target],
            config,
            present_factor,
            via_padstack,
            &net.name,
            wire_width,
            clearance_cells,
        )?;

        // Grow the source set with the newly routed path.
        for &cell in &result.path_cells {
            sources.push(cell);
        }
        for l in 0..grid.num_layers {
            sources.push((target.0, target.1, l));
        }
        tree_pads.push(target);
        all_wires.extend(result.wires);
        all_vias.extend(result.vias);
    }

    // Mark the first pad's cells too (they were never "routed to").
    for l in 0..grid.num_layers {
        grid.mark_occupancy(pad_grid[0].0, pad_grid[0].1, l, clearance_cells);
    }
    let _ = net_id; // net_id reserved for future per-net diagnostics

    if all_wires.is_empty() {
        None
    } else {
        Some((all_wires, all_vias))
    }
}

// ─── DRC helpers ─────────────────────────────────────────────────────────────

fn compute_drc(pcb: &Pcb) -> DrcParams {
    let default_width = pcb.structure.rules.first().map(|r| r.width).unwrap_or(200.0);
    let default_clearance = pcb
        .structure
        .rules
        .first()
        .and_then(|r| r.clearances.first())
        .map(|(v, _)| *v)
        .unwrap_or(200.0);
    let default_via = pcb.structure.vias.first().cloned().unwrap_or_default();
    DrcParams { default_width, default_clearance, default_via }
}

fn net_class_params(
    pcb: &Pcb,
    net_name: &str,
    default_width: f64,
    default_via: &str,
) -> (f64, String) {
    for class in &pcb.network.classes {
        if class.nets.iter().any(|n| n == net_name) {
            let width = class.rule.as_ref().map(|r| r.width).unwrap_or(default_width);
            let via = class.via.clone().unwrap_or_else(|| default_via.to_string());
            return (width, via);
        }
    }
    (default_width, default_via.to_string())
}
