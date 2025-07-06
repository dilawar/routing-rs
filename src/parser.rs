//! DSN parser.

use std::collections::HashMap;
use std::path::Path;

pub fn parse_file<P: AsRef<Path> + std::fmt::Debug>(infile: P) -> anyhow::Result<Pcb> {
    tracing::info!("Parsing {infile:?}...");
    let dsn_string = std::fs::read_to_string(infile).expect("failed to parse");
    parse_string(&dsn_string)
}

fn parse_string(dsn: &str) -> anyhow::Result<Pcb> {
    tracing::info!("Parsing content\n\n{dsn}");
    Ok(Pcb::default())
}

#[derive(Debug, Default)]
pub struct Pcb {
    data: HashMap<String, String>,
}

#[cfg(test)]
mod test {
    use super::*;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_parser() {
        for dsn_file in glob::glob("./tests/*.dsn")
            .expect("failed to read glob pattern")
            .flatten()
        {
            parse_file(&dsn_file).unwrap();
        }
    }
}
