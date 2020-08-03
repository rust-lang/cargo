//! Compares input to expected output.
//!
//! Use the MDMAN_BLESS environment variable to automatically update the
//! expected output.

use mdman::{Format, ManMap};
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use url::Url;

fn run(name: &str) {
    let input = PathBuf::from(format!("tests/compare/{}.md", name));
    let url = Some(Url::parse("https://example.org/").unwrap());
    let mut map = ManMap::new();
    map.insert(
        ("other-cmd".to_string(), 1),
        "https://example.org/commands/other-cmd.html".to_string(),
    );

    for &format in &[Format::Man, Format::Md, Format::Text] {
        let section = mdman::extract_section(&input).unwrap();
        let result = mdman::convert(&input, format, url.clone(), map.clone()).unwrap();
        let expected_path = format!(
            "tests/compare/expected/{}.{}",
            name,
            format.extension(section)
        );
        if std::env::var("MDMAN_BLESS").is_ok() {
            std::fs::write(&expected_path, result).unwrap();
        } else {
            let expected = std::fs::read_to_string(&expected_path).unwrap();
            // Fix if Windows checked out with autocrlf.
            let expected = expected.replace("\r\n", "\n");
            assert_eq!(expected, result);
        }
    }
}

macro_rules! test( ($name:ident) => (
    #[test]
    fn $name() { run(stringify!($name)); }
) );

test!(formatting);
test!(links);
test!(options);
test!(tables);
test!(vars);
