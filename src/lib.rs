use pyo3::prelude::*;
use std::path::Path;

mod pcb;
use pcb::Pcb;

/// A Python module implemented in Rust.
#[pymodule]
fn dsn_parser(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_file, m)?)?;
    m.add_function(wrap_pyfunction!(parse_string, m)?)?;
    Ok(())
}

#[pyfunction]
fn parse_file(infile: String) -> PyResult<Pcb> {
    tracing::info!("Parsing {infile:?}...");
    let dsn_string = std::fs::read_to_string(Path::new(&infile)).expect("failed to parse");
    parse_string(&dsn_string)
}

#[pyfunction]
fn parse_string(dsn: &str) -> PyResult<Pcb> {
    tracing::info!("Parsing content\n\n{dsn}");
    Ok(Pcb::default())
}
