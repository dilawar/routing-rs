use argh::FromArgs;
use std::path::Path;

#[derive(FromArgs)]
/// Parse a DSN file, optionally route it and write the result.
struct Cli {
    /// DSN input file.
    #[argh(positional)]
    infile: String,

    /// output DSN file; if given, the board is auto-routed before writing.
    #[argh(option, short = 'o')]
    output: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let cli: Cli = argh::from_env();
    let infile = Path::new(&cli.infile);
    anyhow::ensure!(infile.exists(), "{infile:?} doesn't exist");

    let src = std::fs::read_to_string(infile)?;
    let pcb = dsn_parser::pcb::parse_dsn(&src)?;

    if let Some(outpath) = &cli.output {
        eprintln!(
            "Routing {} nets on {} layers…",
            pcb.network.nets.len(),
            pcb.structure.layers.len()
        );
        let wiring = router::route(&pcb, Default::default(), None)?;
        eprintln!(
            "Done: {} wires, {} vias",
            wiring.wires.len(),
            wiring.vias.len()
        );
        let routed_dsn = router::serialise::write_wiring(&src, &wiring);
        std::fs::write(outpath, routed_dsn)?;
        eprintln!("Written to {outpath}");
    } else {
        println!("{pcb:?}");
    }

    Ok(())
}
