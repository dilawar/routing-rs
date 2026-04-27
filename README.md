# router-rs

> **Note: This codebase was generated with [Claude Code](https://claude.ai/code) (Anthropic AI).**

A Rust workspace that parses **Spectra DSN files** (PCB design format used by KiCad / FreeRouting),
auto-routes them with a PathFinder congestion-based router, and visualises the result in an egui
desktop GUI.

## Demo

### Charger board — 31 nets, 2 layers (145 wires, 12 vias)
![Charger board routed](docs/demo-charger.png)

### FastTest board — 20 nets, 2 layers (127 wires, 20 vias)
![FastTest board routed](docs/demo-fast-test.png)

---

## Features

- **PathFinder router** — congestion-based multi-pass algorithm (Ebeling et al., 1995) using
  [`pathfinding`](https://crates.io/crates/pathfinding) for A\*; 8-directional moves (45° diagonals),
  path simplification, and DRC-legal convergence
- **Parser** — full Spectra DSN grammar via [pest](https://pest.rs); handles all real-world
  quirks (`string_quote`, Unicode identifiers, …)
- **GUI** (`router-rs`) — egui desktop app with pan/zoom, per-layer visibility, ratsnest display,
  live routing progress, Auto-Route / Clear Routing, and export buttons
- **CLI** — route and export to DSN, KiCad PCB, Gerber, or SVG from the command line

## Workspace

```
dsn-parser/   # Core library: DSN parser + typed PCB structs
router/       # PathFinder auto-router (A* + congestion costs + export)
cli/          # CLI: route / export DSN, KiCad, Gerber, SVG
gui/          # router-rs: egui desktop app
dsn-files/    # 56 real-world DSN test files (from freerouting project)
```

## Quick start

```bash
# Desktop GUI (opens file dialog)
cargo run -p router-rs

# Pre-load a file
cargo run -p router-rs -- dsn-files/Issue313-FastTest.dsn

# Route and write output DSN
cargo run -p cli -- dsn-files/Issue313-FastTest.dsn -o routed.dsn

# Route and export SVG snapshot
cargo run -p cli -- dsn-files/Issue367-Charger.dsn -o routed.dsn --svg board.svg

# Route and export KiCad PCB
cargo run -p cli -- dsn-files/Issue367-Charger.dsn -o routed.dsn --kicad routed.kicad_pcb

# Route and export Gerber files (one .gbr per copper layer)
cargo run -p cli -- dsn-files/Issue367-Charger.dsn -o routed.dsn --gerber-dir gerber/

# Run all parser tests
cargo test -p dsn-parser
```

## Export formats

After routing, the board can be exported in four formats:

| Format | CLI flag | GUI button | Notes |
|--------|----------|------------|-------|
| DSN (Spectra) | `-o <file.dsn>` | — | Full round-trip; re-importable by FreeRouting/KiCad |
| KiCad PCB | `--kicad <file.kicad_pcb>` | **Export KiCad PCB…** | KiCad 6 s-expression; open directly in KiCad PCB editor |
| Gerber RS-274X | `--gerber-dir <dir/>` | **Export Gerber…** | One `.gbr` per copper layer; suitable for PCB fabrication |
| SVG | `--svg <file.svg>` | — | Vector snapshot for documentation |

> GUI export buttons appear in the sidebar once the board has been routed. KiCad PCB opens a
> save-file dialog; Gerber opens a folder picker.

## Library usage

```rust
use dsn_parser::parse_file_rust;

let pcb = parse_file_rust("board.dsn")?;
println!("{} layers, {} nets", pcb.structure.layers.len(), pcb.network.nets.len());

// Route it
let wiring = router::route(&pcb, Default::default(), None)?;
println!("{} wires, {} vias", wiring.wires.len(), wiring.vias.len());
```

## Router — how it works

**PathFinder** (Ebeling et al., 1995) — the same algorithm used in FPGA place-and-route (VPR):

1. Every pass re-routes **all** nets using 8-directional A\* (cardinal + 45° diagonal moves);
   cell cost = `1 + present_factor × occupancy + history`
2. `present_factor` grows each pass — overused cells become increasingly expensive
3. After each pass, `history` is incremented for persistently contested cells
4. The loop stops on the first DRC-legal pass (no cell shared by two nets)
5. Wire paths are simplified: consecutive same-direction steps are merged into single segments

Unlike rip-up-and-reroute, convergence is **guaranteed** when a legal solution exists.

## Build

```bash
cargo build           # debug
cargo build --release # optimised
cargo test            # all tests
cargo clippy --no-deps
```

## Credits

DSN test files from the [freerouting](https://github.com/freerouting/freerouting) project.
