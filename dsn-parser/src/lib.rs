use pyo3::prelude::*;
use std::path::Path;

pub mod pcb;
pub use pcb::{
    ComponentGroup, Image, Layer, Library, Net, NetClass, Network, Padstack, Pcb, Pin,
    PlacedComponent, PlacedVia, Placement, RoutingRule, Shape, Structure, Wiring, Wire,
};

pub fn parse_file_rust(path: &str) -> anyhow::Result<Pcb> {
    let text = std::fs::read_to_string(path)?;
    pcb::parse_dsn(&text)
}

/// A Python module implemented in Rust.
#[pymodule]
fn dsn_parser(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(py_parse_file, m)?)?;
    m.add_function(wrap_pyfunction!(py_parse_string, m)?)?;
    Ok(())
}

#[pyfunction]
pub fn py_parse_file(infile: String) -> PyResult<Pcb> {
    tracing::info!("Parsing {infile:?}...");
    let dsn_string = std::fs::read_to_string(Path::new(&infile)).expect("failed to read file");
    py_parse_string(&dsn_string)
}

#[pyfunction]
fn py_parse_string(text: &str) -> PyResult<Pcb> {
    Ok(pcb::parse_dsn(text)?)
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
            tracing::info!("Parsing file {dsn_file:?}...");
            let dsn = parse_file_rust(
                &dsn_file
                    .into_os_string()
                    .into_string()
                    .expect("failed to convert to String"),
            )
            .unwrap();
            eprintln!("id={} layers={} nets={} wires={}",
                dsn.id, dsn.structure.layers.len(),
                dsn.network.nets.len(), dsn.wiring.wires.len());
        }
    }
}
