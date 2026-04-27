# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**router-rs** — a Rust workspace that parses Spectra DSN files (PCB design format used by
KiCad/FreeRouting) and auto-routes them using a PathFinder congestion-based algorithm.

## Workspace layout

```
dsn-parser/   # Core library: DSN parser + typed Pcb structs
router/       # PathFinder auto-router + export (KiCad PCB, Gerber, serialise)
cli/          # CLI binary: route / export DSN, KiCad, Gerber, SVG
gui/          # router-rs desktop app (egui): visualise, route, export
dsn-files/    # 56 real-world DSN test files (from freerouting project)
```

## Common commands

```bash
# Build
cargo build

# Run the desktop GUI
cargo run -p router-rs

# Run all tests
cargo test

# Run tests for a single crate
cargo test -p dsn-parser

# Lint
cargo clippy --no-deps

# Format (requires nightly)
cargo +nightly fmt
```

## Architecture

### Parsing pipeline

1. `dsn-parser/src/dsn.pest` — Pest grammar for DSN S-expressions.
2. `dsn-parser/src/pcb.rs` — `parse_dsn()` walks the pest parse tree and builds the `Pcb` struct.
3. `dsn-parser/src/lib.rs` — Public surface: `parse_file_rust(path)` and `parse_dsn(text)`.

### Critical grammar note

`iden` **must** be atomic (`@{ ... }`). Without `@`, pest inserts implicit `WHITESPACE` between
repetitions and multi-word tokens collapse into a single atom.

### Key types (`dsn-parser`)

- **`Pcb`** — top-level output struct. Fields: `structure`, `placement`, `library`, `network`, `wiring`.
- **`SExpr`** — intermediate S-expression tree. Helpers: `tag()`, `children()`, `find_all()`, `find_first()`.
- `(string_quote ")` in DSN files is handled by `preprocess()` before parsing.

### Router (`router/`)

PathFinder (Ebeling et al., 1995) over a 3-D grid `(ix, iy, layer)`.

- **`RouterConfig`** — `grid_pitch` (default 100 DSN units), `via_cost` (default 15),
  `max_pf_passes` (default 50), `present_factor_step`, `history_increment`.
- **`ProgressEvent`** — sent over `mpsc::SyncSender` for live GUI updates:
  `StartNet`, `NetRouted`, `NetFailed`, `PassComplete`, `Finished`.
- **`route(pcb, config, tx)`** — main entry point. Nets ordered by shortest bounding-box diagonal.
  Steiner tree grows nearest-neighbor. All nets re-routed every pass.
- **`grid.rs`** — `GridMap`: `perm` (hard obstacles), `occupancy` (current-pass congestion),
  `history` (cross-pass penalty). `pf_cost()` = `1 + present_factor × occupancy + history`.
- **`bfs.rs`** — multi-source A\* via virtual `None` super-node (pathfinding crate).
  8-directional moves (cardinal cost×1, diagonal cost×1.5). Chebyshev heuristic.
  `simplify_path()` collapses consecutive same-direction steps into single segments.
- **`export.rs`** — `to_kicad_pcb()` and `to_gerber_layers()` for post-routing export.
- **`serialise.rs`** — `write_wiring()` patches routed wires back into the original DSN text.

### GUI (`gui/`)

`eframe`/`egui` desktop app. Entry point: `RouterApp` in `gui/src/app.rs`.

- **File open**: `rfd::FileDialog` → `dsn_parser::parse_file_rust()`
- **Canvas**: pan (drag) + zoom-toward-cursor (scroll).
  Transform: `screen = canvas_origin + pan + board_coords × zoom`.
- **Draws**: board outline, wires per layer (KiCad colors), pads with rotation, ratsnest,
  keepout outlines, component labels, in-progress wires (dimmed during routing).
- **Sidebar**: layer toggles, stats, fit-to-window, Auto-Route / Cancel / Clear Routing,
  Export KiCad PCB…, Export Gerber…

### Test data

`dsn-files/` contains 56 real-world `.dsn` files from the freerouting project.
The glob-based integration test in `dsn-parser/src/lib.rs` parses every file — good regression
suite for grammar changes.
