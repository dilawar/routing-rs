//! PCB Structure
//!
//! See the official documentation.

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use thiserror::Error;

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

#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    SExpr(SExpr),
    String(String),
    Number(f64),
    Symbol(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SExpr {
    pub head: String,
    pub args: Vec<Atom>,
}

#[derive(Debug, Error)]
pub enum DsnError {
    #[error("pest parse error: {0}")]
    Pest(#[from] pest::error::Error<Rule>),

    #[error("invalid number literal: {0}")]
    Number(String),

    #[error("unexpected grammar node: {0:?}")]
    Unexpected(Rule),
}

impl SExpr {
    /// Helper to fetch a nested S-expr by head name.
    pub fn find_all<'a>(&'a self, head: &'a str) -> impl Iterator<Item = &'a SExpr> {
        self.args.iter().filter_map(move |a| match a {
            Atom::SExpr(s) if s.head.eq_ignore_ascii_case(head) => Some(s),
            _ => None,
        })
    }
}

pub fn parse_dsn(input: &str) -> Result<Vec<SExpr>, DsnError> {
    let pairs = DsnParser::parse(Rule::file, input)?;
    let mut out = Vec::new();
    for p in pairs {
        match p.as_rule() {
            Rule::sexpr => out.push(build_sexpr(p)?),
            Rule::COMMENT | Rule::WHITESPACE => { /* skip */ }
            Rule::EOI | Rule::file => { /* container */ }
            _ => return Err(DsnError::Unexpected(p.as_rule())),
        }
    }
    Ok(out)
}

fn build_sexpr(pair: Pair<Rule>) -> Result<SExpr, DsnError> {
    debug_assert_eq!(pair.as_rule(), Rule::sexpr);
    let mut inner = pair.into_inner(); // symbol then zero+ atoms
    let head = inner
        .next()
        .and_then(|p| {
            if p.as_rule() == Rule::symbol {
                Some(p.as_str().to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| DsnError::Unexpected(Rule::symbol))?;

    let mut args = Vec::new();
    for p in inner {
        match p.as_rule() {
            Rule::atom => args.push(build_atom(p)?),
            Rule::WHITESPACE | Rule::COMMENT => {}
            _ => return Err(DsnError::Unexpected(p.as_rule())),
        }
    }
    Ok(SExpr { head, args })
}

fn build_atom(pair: Pair<Rule>) -> Result<Atom, DsnError> {
    debug_assert_eq!(pair.as_rule(), Rule::atom);
    let mut inner = pair.into_inner();
    let p = inner
        .next()
        .ok_or_else(|| DsnError::Unexpected(Rule::atom))?;
    Ok(match p.as_rule() {
        Rule::sexpr => Atom::SExpr(build_sexpr(p)?),
        Rule::string => Atom::String(unescape_string(p.as_str())),
        Rule::number => {
            let s = p.as_str();
            match s.parse::<f64>() {
                Ok(n) => Atom::Number(n),
                Err(_) => return Err(DsnError::Number(s.to_string())),
            }
        }
        Rule::symbol => Atom::Symbol(p.as_str().to_string()),
        _ => return Err(DsnError::Unexpected(p.as_rule())),
    })
}

fn unescape_string(raw: &str) -> String {
    // raw is like "\"text\"", quick unescape for common sequences
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    let _ = chars.next(); // skip leading "
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    match n {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        'r' => out.push('\r'),
                        other => {
                            out.push('\\');
                            out.push(other);
                        }
                    }
                } else {
                    out.push('\\');
                }
            }
            '"' => break, // closing quote
            other => out.push(other),
        }
    }
    out
}

/// Convenience: walk all top-level S-exprs with a given head.
pub fn filter_top<'a>(roots: &'a [SExpr], head: &'a str) -> impl Iterator<Item = &'a SExpr> {
    roots
        .iter()
        .filter(move |s| s.head.eq_ignore_ascii_case(head))
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
    fn parses_sample() {
        let roots = parse_dsn(SAMPLE).expect("parse ok");
        assert!(!roots.is_empty());
        let design = filter_top(&roots, "design").next().expect("has design");
        assert_eq!(design.head, "design");
        let nets: Vec<_> = design.find_all("net").collect();
        assert_eq!(nets.len(), 1);
    }
}
