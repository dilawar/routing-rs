# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

A Rust workspace that parses **Spectra DSN files** (PCB design format used by KiCad/FreeRouting). The library exposes both a Rust API and Python bindings via PyO3.

## Workspace layout

```
dsn-parser/   # Core library crate — parser + Pcb data structure + Python bindings
cli/          # CLI binary that wraps the library
gui/          # egui desktop visualizer (dsn-viewer binary)
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

### Python bindings

The crate type is `["lib", "cdylib"]`. PyO3 feature flags `abi3-py39` and `auto-initialize` are set. Build with `maturin build` to produce a wheel targeting Python ≥ 3.9.

### Test data

`dsn-parser/tests/` contains 30+ real-world `.dsn` files from the freerouting project. The glob-based integration test in `lib.rs` parses every file in that directory, making it a good regression suite for grammar changes.
