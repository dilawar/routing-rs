use dsn_parser;
use std::path::Path;
use tracing_test::traced_test;

#[test]
#[traced_test]
fn test_parser() {
    for dsn_file in glob::glob("./tests/*.dsn")
        .expect("failed to read glob pattern")
        .flatten()
    {
        dsn_parser::parse_file(&dsn_file).unwrap();
    }
}
