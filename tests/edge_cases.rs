use rustfix;
use std::collections::HashSet;
use std::fs;

#[test]
fn multiple_fix_options_yield_no_suggestions() {
    let json = fs::read_to_string("./tests/edge-cases/skip-multi-option-lints.json").unwrap();
    let expected_suggestions =
        rustfix::get_suggestions_from_json(&json, &HashSet::new(), rustfix::Filter::Everything)
            .unwrap();
    assert!(expected_suggestions.is_empty());
}

#[test]
fn out_of_bounds_test() {
    let json = fs::read_to_string("./tests/edge-cases/out_of_bounds.recorded.json").unwrap();
    let expected_suggestions =
        rustfix::get_suggestions_from_json(&json, &HashSet::new(), rustfix::Filter::Everything)
            .unwrap();
    assert!(expected_suggestions.is_empty());
}
