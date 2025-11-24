//! A small cli utility to print parsed output as JSON.

use argh::FromArgs;
use std::path::Path;
use tracing_subscriber::EnvFilter;

#[derive(FromArgs)]
/// Parse DSN flie and output JSON
struct Cli {
    /// dsn file.
    #[argh(positional)]
    infile: String,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // First argument is always a file.
    let cli: Cli = argh::from_env();
    let infile = Path::new(&cli.infile);
    anyhow::ensure!(infile.exists(), "{infile:?} doesn't exists");
    tracing::info!("Parsing file {infile:?}");
    let result = dsn_parser::parse_file(infile.to_string_lossy().into())?;
    println!("{result:?}");
    Ok(())
}
