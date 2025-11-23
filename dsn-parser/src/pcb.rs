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
#[grammar = "../grammars/dsn.pest"]
struct DsnParser;

/// Parse a given DSN string
pub fn parse_dsn(input: &str) -> anyhow::Result<Pcb> {
    let dsn = DsnParser::parse(Rule::file, input)?.next().unwrap();
    let pcb = Pcb::default();
    for line in dsn.into_inner() {
        // tracing::debug!("Rule is {line:?}");
        match line.as_rule() {
            Rule::sexpr => {
                tracing::info!("sexpr: {line:#?}");
            }
            Rule::WHITESPACE | Rule::COMMENT => {
                tracing::debug!("whitespace/comment");
            }
            Rule::EOI => {
                tracing::debug!("EOI");
            }
            _ => {
                tracing::debug!("unsupported {}", line.into_inner())
            }
        }
    }
    Ok(pcb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[tracing_test::traced_test]
    fn test_parse_simple_dsn() {
        let simple = r#"(pcb C:\Users\Owner\Desktop\hw_48\hw_48.dsn
                (parser
                    (host_cad "KiCad's cad")
                    (string_quote ")
                    (host_version "(5.1.5)-3")
                )
                (resolution um 10)
                (design
                    (unit mm)
                    (layers (signal F.Cu) (signal B.Cu))
                    (component U1 (at 12.7 7.5) (rotate 90))
                    (net N$1 (pin U1 1) (pin R1 2))
                )
            )"#;
        parse_dsn(simple).expect("parsing failed");
    }

    #[test]
    #[tracing_test::traced_test]
    fn test_parse_atom() {
        for atom in [
            r#"(string_quote ")"#,
            r#"(comment "")"#,
            r#"(comment "'")"#,
            r#"(abc 123)"#,
            r#"(abc -123)"#,
            r#"(abc-1# -123)"#,
            r#"(MC-BD/R# -1.23)"#,
            r#"(host_version "(5.1.5)-3")"#,
            r#"(host_cad "KiCad's cad")"#,
        ] {
            let parsed = DsnParser::parse(Rule::atom, atom).unwrap();
            tracing::warn!("Parsed atom string {parsed:?}");
            assert_eq!(parsed.as_str(), atom);
        }
    }

    #[test]
    #[tracing_test::traced_test]
    fn test_parse_sexpr() {
        pest::set_error_detail(true);
        const INPUT: &str = r#"(parser 
                (host_version "(5.1.5)-3")
                (acbc 123)
                (xyz_abc zyz_abd)
                (host_cad "KiCad's cad")
                (string_quote ")
            )"#;
        let parsed = DsnParser::parse(Rule::sexpr, INPUT)
            .unwrap()
            .next()
            .unwrap();
        tracing::info!("{parsed:#?}");
    }
}
