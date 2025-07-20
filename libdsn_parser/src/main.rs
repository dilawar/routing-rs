//! A small cli utility to print parsed output as JSON.

use libdsn_parser;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // First argument is always a file.
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 1 {
        println!("USAGE: {} <file_name>", env!("CARGO_BIN_NAME"));
        std::process::exit(0);
    }

    let infile = Path::new(&args.first().expect("no valid path"));
    anyhow::ensure!(infile.exists(), "{infile:?} doesn't exists");
    tracing::info!("Parsing file {infile:?}");
    let result = libdsn_parser::parse_file(infile)?;
    println!("{result:?}");
    Ok(())
}
