# DSN Parser

> **Note: This codebase was generated with [Claude Code](https://claude.ai/code) (Anthropic AI). The parser, visualiser, and all associated code were written by an AI assistant.**

A Rust crate and Python library for parsing Spectra DSN files (the PCB design format used by KiCad and FreeRouting), plus an egui-based desktop visualiser.

## Features

- Parse any Spectra DSN file into typed Rust structs (layers, components, nets, traces, vias, keepouts)
- Python bindings via PyO3
- `dsn-viewer` — desktop GUI to visualise PCB files with pan/zoom, layer toggles, and ratsnest display

## Workspace

```
dsn-parser/   # Core library (Rust + Python bindings)
cli/          # CLI tool: parse a DSN file and print debug output
gui/          # dsn-viewer: egui desktop visualiser
```

## Usage

### Visualiser

```bash
cargo run -p dsn-viewer                          # open file dialog
cargo run -p dsn-viewer -- path/to/board.dsn    # pre-load a file
```

### CLI

```bash
cargo run -p cli -- path/to/board.dsn
```

### Library

```rust
let pcb = dsn_parser::parse_file_rust("board.dsn")?;
println!("{} layers, {} nets", pcb.structure.layers.len(), pcb.network.nets.len());
```

### Python

```python
import dsn_parser
pcb = dsn_parser.parse_file("board.dsn")
```

## Build

```bash
cargo build            # Rust
cargo test             # run all tests
maturin build          # Python wheel
```

## Credits

- DSN test files collected from the [freerouting](https://github.com/freerouting/freerouting) project.
