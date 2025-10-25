//! PCB Structure
//!
//! See the official documentation.

use pest::Parser;
use pest_derive::Parser;

#[pyo3::pyclass]
#[derive(Debug, Default)]
pub struct Pcb {
    pub id: String,
    pub parser: Option<String>,
    pub capacitance_resolution: Option<String>,
    pub conductance_resolution: Option<String>,
    pub current_resolution: Option<String>,
    pub inductance_resolution: Option<String>,
    pub resistance_resolution: Option<String>,
    pub resolution: Option<String>,
    pub voltage_resolution: Option<String>,
    pub time_resolution: Option<String>,
    pub unit: Option<String>,
    pub stucture: Option<String>,
    pub placement: Option<String>,
    pub library: Option<String>,
    pub floor_plan: Option<String>,
    pub part_library: Option<String>,
    pub network: Option<String>,
    pub wiring: Option<String>,
    pub color: Option<String>,
}

impl Pcb {
    /// Parse into Pcb
    pub fn parse(&mut self, text: &str) -> anyhow::Result<()> {
        tracing::debug!("Parsing {text:?}");
        Ok(())
    }
}

#[derive(Parser)]
#[grammar = "dsn.pest"]
struct DsnParser;

/// Parse a given DSN string
pub fn parse_dsn(input: &str) -> anyhow::Result<Pcb> {
    let dsn = DsnParser::parse(Rule::file, input)?.next().unwrap();
    let pcb = Pcb::default();
    for line in dsn.into_inner() {
        match line.as_rule() {
            Rule::sexpr => {
                tracing::info!("sexpr: {}", line.into_inner());
            }
            Rule::WHITESPACE | Rule::COMMENT => {
                tracing::debug!("whitespace/comment");
            }
            Rule::EOI => {
                tracing::debug!("EOI");
            }
        }
        tracing::info!("Rule is {rule:?}");
    }
    Ok(pcb)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        ; minimal DSN-like sample
        (design
          (unit mm)
          (layers (signal F.Cu) (signal B.Cu))
          (component U1 (at 12.7 7.5) (rotate 90))
          (net N$1 (pin U1 1) (pin R1 2))
        )
    "#;

    #[test]
    #[tracing_test::traced_test]
    fn test_parse_simple_dsn() {
        parse_dsn(SAMPLE).expect("parse ok");
    }
}
