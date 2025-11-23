//! Syntax tree

use sexpr_parser::{Parser, SexprFactory};

pub fn parse(text: &str) -> sexpr_parser::Result<'_, S> {
    SF.parse(text)
}

/// Your amazing S-expression data structure
#[derive(Debug, PartialEq)]
pub enum S {
    Nil,
    Int(i64),
    Float(f64),
    Symbol(String),
    String(String),
    Pair(Box<(S, S)>),
}

impl S {
    /// Walk and print the tree.
    pub fn walk_and_print(self) {
        match self {
            S::Nil => {}
            S::Int(x) => println!("int {x}"),
            S::Float(x) => println!("float {x}"),
            S::Symbol(x) => println!("symbol {x}"),
            S::String(x) => println!("string {x}"),
            S::Pair(x) => {
                let (a, b) = *x;
                a.walk_and_print();
                b.walk_and_print();
            }
        }
    }
}

pub struct SF;

impl SexprFactory for SF {
    type Sexpr = S;
    type Integer = i64;
    type Float = f64;

    fn int(&mut self, x: i64) -> S {
        S::Int(x)
    }

    fn float(&mut self, x: f64) -> Self::Sexpr {
        S::Float(x)
    }

    fn symbol(&mut self, x: &str) -> Self::Sexpr {
        S::Symbol(x.to_string())
    }

    fn string(&mut self, x: &str) -> Self::Sexpr {
        S::String(x.to_string())
    }

    fn list(&mut self, x: Vec<Self::Sexpr>) -> Self::Sexpr {
        let mut tail = S::Nil;
        for item in x.into_iter().rev() {
            tail = S::Pair(Box::new((item, tail)))
        }
        tail
    }

    fn pair(&mut self, a: Self::Sexpr, b: Self::Sexpr) -> Self::Sexpr {
        S::Pair(Box::new((a, b)))
    }
}

/// Let's test the eval!
#[cfg(test)]
mod tests {
    use super::*;

    /// Let's check that the parser works as expected
    #[test]
    fn test_parser() {
        let text = "(+ (* 15 2) 62)";
        let s = SF.parse(text).unwrap();
        assert_eq!(format!("{s:?}"), 
            "Pair((Symbol(\"+\"), Pair((Pair((Symbol(\"*\"), Pair((Int(15), Pair((Int(2), Nil)))))), Pair((Int(62), Nil))))))"
        );
    }

    #[test]
    #[tracing_test::traced_test]
    fn test_parse_expressions() {
        let sexps = vec![
            "92",
            "(+ 62 30)",
            "(/ 92 0)",
            "nan",
            "(abc 1 xyz)",
            "(+ (* 15 2) 62)",
        ];

        let mut results = vec![];
        for s in sexps {
            let root = SF.parse(s).unwrap();
            results.push(format!("{root:?}"));
        }

        tracing::info!("{results:?}");
        assert_eq!(results, vec!["Int(92)", 
                "Pair((Symbol(\"+\"), Pair((Int(62), Pair((Int(30), Nil))))))", 
                "Pair((Symbol(\"/\"), Pair((Int(92), Pair((Int(0), Nil))))))", 
                "Float(NaN)", 
                "Pair((Symbol(\"abc\"), Pair((Int(1), Pair((Symbol(\"xyz\"), Nil))))))", 
                "Pair((Symbol(\"+\"), Pair((Pair((Symbol(\"*\"), Pair((Int(15), Pair((Int(2), Nil)))))), Pair((Int(62), Nil))))))",
            ]);
    }
}
