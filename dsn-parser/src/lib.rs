use pyo3::prelude::*;
use std::path::Path;

mod pcb;
mod syntax_tree;

use pcb::Pcb;

/// A Python module implemented in Rust.
#[pymodule]
fn dsn_parser(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_file, m)?)?;
    m.add_function(wrap_pyfunction!(parse_string, m)?)?;
    Ok(())
}

#[pyfunction]
pub fn parse_file(infile: String) -> PyResult<Pcb> {
    tracing::info!("Parsing {infile:?}...");
    let dsn_string = std::fs::read_to_string(Path::new(&infile)).expect("failed to parse");
    parse_string(&dsn_string)
}

#[pyfunction]
fn parse_string(text: &str) -> PyResult<Pcb> {
    let mut pcb = Pcb::default();
    pcb.parse(text)?;
    Ok(pcb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_parser() {
        for dsn_file in glob::glob("./tests/*.dsn")
            .expect("failed to read glob pattern")
            .flatten()
        {
            tracing::info!("Parsing file {dsn_file:?}...");
            let dsn = parse_file(
                dsn_file
                    .into_os_string()
                    .into_string()
                    .expect("failed to convert to String"),
            )
            .unwrap();
            eprintln!("{dsn:?}");
        }
    }
}
