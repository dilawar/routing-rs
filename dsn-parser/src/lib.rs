pub mod pcb;
pub use pcb::{
    ComponentGroup, Image, Layer, Library, Net, NetClass, Network, Padstack, Pcb, Pin,
    PlacedComponent, PlacedVia, Placement, RoutingRule, Shape, Structure, Wiring, Wire,
};

pub fn parse_file_rust(path: &str) -> anyhow::Result<Pcb> {
    let text = std::fs::read_to_string(path)?;
    pcb::parse_dsn(&text)
}

#[cfg(test)]
mod test {
    use super::*;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_parser() {
        for dsn_file in glob::glob("../dsn-files/*.dsn")
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
