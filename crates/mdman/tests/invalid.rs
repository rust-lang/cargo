//! Tests for errors and invalid input.

use std::path::PathBuf;

use mdman::{Format, ManMap};
use snapbox::prelude::*;

fn run(name: &str, expected_error: impl IntoData) {
    let input = PathBuf::from(format!("tests/invalid/{}", name));
    match mdman::convert(&input, Format::Man, None, ManMap::new()) {
        Ok(_) => {
            panic!("expected {} to fail", name);
        }
        Err(e) => {
            snapbox::assert_data_eq!(e.to_string(), expected_error.raw());
        }
    }
}

macro_rules! test( ($name:ident, $file_name:expr, $error:expr) => (
    #[test]
    fn $name() { run($file_name, $error); }
) );

test!(
    nested,
    "nested.md",
    "Error rendering \"template\" line 4, col 1: options blocks cannot be nested"
);

test!(
    not_inside_options,
    "not-inside-options.md",
    "Error rendering \"template\" line 3, col 1: option must be in options block"
);
