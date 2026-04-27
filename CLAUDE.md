# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

A Rust workspace that parses **Spectra DSN files** (PCB design format used by KiCad/FreeRouting). The library exposes both a Rust API and Python bindings via PyO3.

## Workspace layout

```
dsn-parser/   # Core library crate — parser + Pcb data structure + Python bindings
cli/          # CLI binary that wraps the library (supports --output for batch routing)
gui/          # egui desktop visualizer (dsn-viewer binary) with Auto-Route button
router/       # PCB auto-router (A* maze router, multi-layer, live progress events)
```

## Run the viewer

```bash
cargo run -p dsn-viewer
```

## Common commands

```bash
# Build
cargo build

# Run all tests
cargo test

# Run tests for a single crate
cargo test -p dsn-parser

# Run a specific test
cargo test -p dsn-parser parse_simple_dsn

# Lint
cargo clippy --no-deps
cargo clippy --no-deps --fix

# Format (requires nightly)
cargo +nightly fmt

# Build Python wheel (requires maturin)
maturin build
```

The `Justfile` at the repo root mirrors these commands as `just build`, `just lint`, etc.

## Architecture

### Parsing pipeline

1. `dsn-parser/src/dsn.pest` — Pest grammar for DSN S-expressions (parentheses, identifiers, strings, comments).
2. `dsn-parser/src/pcb.rs` — `parse_dsn()` drives the pest parser using `Rule::file`, walks the resulting parse tree, and builds the `Pcb` struct.
3. `dsn-parser/src/lib.rs` — Public surface: `parse_file(path)` and `parse_string(text)`, both returning `Result<Pcb>`. Also declares the `#[pymodule]` entry point so the same functions are callable from Python.

### Critical grammar note

`iden` **must** be atomic (`@{ ... }`). Without `@`, pest inserts implicit `WHITESPACE` between repetitions and multi-word tokens like `pcb ./file.dsn` collapse into a single atom. The `WHITESPACE` rule handles inter-token spacing; within `iden` itself, `WHITE_SPACE` (Unicode category) is used in the negative lookahead.

### Key types

- **`Pcb`** (`pcb.rs`) — top-level output struct. Fields map to DSN sections: `structure` (`Structure`), `placement` (`Placement`), `library` (`Library`), `network` (`Network`), `wiring` (`Wiring`). Annotated with `#[pyo3::pyclass]`; only primitive fields have `#[pyo3(get)]` — nested structs are Rust-only.
- **`SExpr`** (`pcb.rs`) — intermediate S-expression tree built from the pest parse tree before semantic extraction. Helper methods: `tag()`, `children()`, `atom_children()`, `find_all()`, `find_first()`.
- The pest grammar produces a `Rule::file` pair; its children are `Rule::expr` items, each wrapping one `Rule::iden`, `Rule::string`, or `Rule::sexpr`. `convert_pair()` walks this tree into `SExpr`.
- `(string_quote ")` in DSN files breaks the quoted-string grammar rule; `preprocess()` replaces it before parsing.

### GUI (`gui/`)

`eframe`/`egui` desktop app. Entry point: `DsnViewerApp` in `gui/src/app.rs`.

- **File open**: `rfd::FileDialog` → calls `dsn_parser::parse_file_rust()`
- **Canvas**: custom `egui::Painter` with pan (drag) and zoom-toward-cursor (scroll). Coordinate transform: `screen = canvas_origin + pan + board_coords × zoom`.
- **Draws**: board outline, wires per layer (color-coded), pads with rotation applied, ratsnest (star pattern from first pad to all others in a net), keepout outlines, component labels.
- **Sidebar**: layer visibility toggles with color swatches, net/component/trace stats, fit-to-window button.

### Router (`router/`)

A\* maze router with multi-pass Rip-up and Reroute (RnR) over a 3D grid `(ix, iy, layer)`.

- **`RouterConfig`** — `grid_pitch` (DSN units per cell, default 100), `via_cost` (default 5), `max_iterations` (default 500k), `max_rnr_passes` (default 20).
- **`ProgressEvent`** enum — sent over `std::sync::mpsc::SyncSender` for live GUI updates: `StartNet`, `NetRouted { wires, vias }`, `NetFailed`, `PassComplete { pass, routed, total }`, `Finished { wiring }`.
- **`route(pcb, config, tx)`** — main entry point. Net order: shortest bounding-box diagonal first. Steiner tree uses nearest-neighbor ordering (always connect the pad closest to the current tree). Runs initial pass then RnR passes.
- **`grid.rs`** — `GridMap`: `cells: Vec<bool>` (blocked?) + `net_owner: Vec<u32>` (FREE=0, PERM=u32::MAX, 1..n=net_id). `is_ghost_blocked()` passes through net-owned cells; only PERM blocks. `clear_net(id)` O(total_cells) rip-up. DRC clearance radius: `ceil((clearance + wire_width) / pitch - 1.0)` cells.
- **`bfs.rs`** — A\* with flat `Vec<u32>` dist array. `ghost_route()` ignores net-owned cells (used for RnR blocker detection). `apply_path_to_grid()` re-applies a saved path after rip-up.
- **RnR algorithm**: for each failed net — ghost-route the full Steiner tree to find ALL blocking nets, rip up 1..max_rip most-frequent blockers, route target net, re-route ripped nets in nearest-first order, commit on success or restore on failure. max_rip escalates from 1 to 16 over passes.
- **Routing performance**: release mode routes a 74-net board in ~18s with 96% completion (71/74 nets); a 165-net board achieves ~99% completion.

### Python bindings

The crate type is `["lib", "cdylib"]`. PyO3 feature flags `abi3-py39` and `auto-initialize` are set. Build with `maturin build` to produce a wheel targeting Python ≥ 3.9.

### Test data

`dsn-parser/tests/` contains 30+ real-world `.dsn` files from the freerouting project. The glob-based integration test in `lib.rs` parses every file in that directory, making it a good regression suite for grammar changes.
