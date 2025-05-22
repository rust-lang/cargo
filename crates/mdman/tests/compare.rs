//! Compares input to expected output.

use std::path::PathBuf;

use mdman::{Format, ManMap};
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
        let expected_path = PathBuf::from(format!(
            "tests/compare/expected/{}.{}",
            name,
            format.extension(section)
        ));
        snapbox::assert_data_eq!(result, snapbox::Data::read_from(&expected_path, None).raw());
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
