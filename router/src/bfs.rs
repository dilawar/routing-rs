use std::collections::HashSet;
use dsn_parser::pcb::{PlacedVia, Wire};
use pathfinding::prelude::astar;
use crate::{RouterConfig, grid::GridMap};

pub type State = (usize, usize, usize); // (ix, iy, layer)

// `None` is the virtual super-source that connects to all real sources at cost 0,
// enabling multi-source A* with a single call to the pathfinding crate.
type PfNode = Option<State>;

pub struct RouteResult {
    pub wires: Vec<Wire>,
    pub vias: Vec<PlacedVia>,
    /// Every grid cell on the path (used by caller to mark occupancy).
    pub path_cells: Vec<State>,
    /// Grid (ix, iy) positions where layer transitions occur.
    pub via_grid_cells: Vec<(usize, usize)>,
}

/// Find the lowest-cost path from any source to any cell in `targets` (matched by (ix, iy)
/// ignoring layer), then commit occupancy to the grid and return wire/via geometry.
pub fn route_net(
    grid: &mut GridMap,
    sources: &[State],
    targets: &[(usize, usize)],
    config: &RouterConfig,
    present_factor: u32,
    via_padstack: &str,
    net_name: &str,
    wire_width: f64,
    clearance_cells: usize,
) -> Option<RouteResult> {
    let path = find_path(grid, sources, targets, config, present_factor)?;
    Some(commit_path(grid, &path, via_padstack, net_name, wire_width, clearance_cells))
}

// ── A* via pathfinding crate ──────────────────────────────────────────────────

fn find_path(
    grid: &GridMap,
    sources: &[State],
    targets: &[(usize, usize)],
    config: &RouterConfig,
    present_factor: u32,
) -> Option<Vec<State>> {
    let target_set: HashSet<(usize, usize)> =
        targets.iter().copied().collect();

    let (w, h, nl) = (grid.width as i64, grid.height as i64, grid.num_layers as i64);
    let via_cost = config.via_cost;

    let (path, _cost) = astar(
        &None::<State>,
        |node: &PfNode| -> Vec<(PfNode, u32)> {
            // Virtual super-source expands to all real sources at zero cost.
            let Some(&(ix, iy, layer)) = node.as_ref() else {
                return sources.iter().map(|&s| (Some(s), 0u32)).collect();
            };

            let mut nbrs: Vec<(PfNode, u32)> = Vec::with_capacity(6);

            // 4-directional moves on the same layer.
            for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let nx = ix as i64 + dx;
                let ny = iy as i64 + dy;
                if nx < 0 || ny < 0 || nx >= w || ny >= h {
                    continue;
                }
                let cost = grid.pf_cost(nx as usize, ny as usize, layer, present_factor);
                if cost < u32::MAX {
                    nbrs.push((Some((nx as usize, ny as usize, layer)), cost));
                }
            }

            // Layer transitions (vias).
            for delta in [-1i64, 1] {
                let nl2 = layer as i64 + delta;
                if nl2 < 0 || nl2 >= nl {
                    continue;
                }
                let cost = grid.pf_cost(ix, iy, nl2 as usize, present_factor);
                if cost < u32::MAX {
                    nbrs.push((Some((ix, iy, nl2 as usize)), via_cost.saturating_add(cost)));
                }
            }

            nbrs
        },
        |node: &PfNode| -> u32 {
            let Some(&(ix, iy, _)) = node.as_ref() else { return 0; };
            targets
                .iter()
                .map(|&(tx, ty)| ix.abs_diff(tx) as u32 + iy.abs_diff(ty) as u32)
                .min()
                .unwrap_or(0)
        },
        |node: &PfNode| -> bool {
            let Some(&(ix, iy, _)) = node.as_ref() else { return false; };
            target_set.contains(&(ix, iy))
        },
    )?;

    // Drop the leading virtual super-source node (always `None`).
    Some(path.into_iter().flatten().collect())
}

// ── Path → wires/vias + occupancy marking ────────────────────────────────────

fn commit_path(
    grid: &mut GridMap,
    path: &[State],
    via_padstack: &str,
    net_name: &str,
    wire_width: f64,
    clearance_cells: usize,
) -> RouteResult {
    let mut wires: Vec<Wire> = Vec::new();
    let mut vias: Vec<PlacedVia> = Vec::new();
    let mut via_grid_cells: Vec<(usize, usize)> = Vec::new();
    let mut seg_start = 0usize;

    for i in 1..=path.len() {
        let at_end = i == path.len();
        let layer_changed = !at_end && path[i].2 != path[seg_start].2;

        if at_end || layer_changed {
            let seg = &path[seg_start..i];
            if seg.len() >= 2 {
                let pts = seg.iter().map(|&(ix, iy, _)| grid.grid_to_world(ix, iy)).collect();
                let layer_name = grid.layer_names.get(seg[0].2).cloned().unwrap_or_default();
                wires.push(Wire {
                    layer: layer_name,
                    width: wire_width,
                    path: pts,
                    net: Some(net_name.to_string()),
                });
            }
            if layer_changed {
                let (vx, vy, _) = path[i - 1];
                let (wx, wy) = grid.grid_to_world(vx, vy);
                vias.push(PlacedVia {
                    padstack: via_padstack.to_string(),
                    x: wx,
                    y: wy,
                    net: Some(net_name.to_string()),
                });
                via_grid_cells.push((vx, vy));
            }
            seg_start = i;
        }
    }

    // Mark occupancy for this net's cells (including DRC clearance halo).
    for &(ix, iy, layer) in path {
        grid.mark_occupancy(ix, iy, layer, clearance_cells);
    }
    // Via positions need clearance on all layers.
    for &(vx, vy) in &via_grid_cells {
        for l in 0..grid.num_layers {
            grid.mark_occupancy(vx, vy, l, clearance_cells);
        }
    }

    RouteResult {
        wires,
        vias,
        path_cells: path.to_vec(),
        via_grid_cells,
    }
}
