use cargo_platform::{CheckCfg, ExpectedValues};
use std::collections::HashSet;

#[test]
fn print_check_cfg_none() {
    let mut check_cfg = CheckCfg::default();

    check_cfg.parse_print_check_cfg_line("cfg_a").unwrap();
    assert_eq!(
        *check_cfg.expecteds.get("cfg_a").unwrap(),
        ExpectedValues::Some(HashSet::from([None]))
    );
}

#[test]
fn print_check_cfg_empty() {
    let mut check_cfg = CheckCfg::default();

    check_cfg.parse_print_check_cfg_line("cfg_b=").unwrap();
    assert_eq!(
        *check_cfg.expecteds.get("cfg_b").unwrap(),
        ExpectedValues::Some(HashSet::from([]))
    );
}

#[test]
fn print_check_cfg_any() {
    let mut check_cfg = CheckCfg::default();

    check_cfg.parse_print_check_cfg_line("cfg_c=any()").unwrap();
    assert_eq!(
        *check_cfg.expecteds.get("cfg_c").unwrap(),
        ExpectedValues::Any
    );

    check_cfg.parse_print_check_cfg_line("cfg_c").unwrap();
    assert_eq!(
        *check_cfg.expecteds.get("cfg_c").unwrap(),
        ExpectedValues::Any
    );
}

#[test]
fn print_check_cfg_value() {
    let mut check_cfg = CheckCfg::default();

    check_cfg
        .parse_print_check_cfg_line("cfg_d=\"test\"")
        .unwrap();
    assert_eq!(
        *check_cfg.expecteds.get("cfg_d").unwrap(),
        ExpectedValues::Some(HashSet::from([Some("test".to_string())]))
    );

    check_cfg
        .parse_print_check_cfg_line("cfg_d=\"tmp\"")
        .unwrap();
    assert_eq!(
        *check_cfg.expecteds.get("cfg_d").unwrap(),
        ExpectedValues::Some(HashSet::from([
            Some("test".to_string()),
            Some("tmp".to_string())
        ]))
    );
}

#[test]
fn print_check_cfg_none_and_value() {
    let mut check_cfg = CheckCfg::default();

    check_cfg.parse_print_check_cfg_line("cfg").unwrap();
    check_cfg.parse_print_check_cfg_line("cfg=\"foo\"").unwrap();
    assert_eq!(
        *check_cfg.expecteds.get("cfg").unwrap(),
        ExpectedValues::Some(HashSet::from([None, Some("foo".to_string())]))
    );
}

#[test]
fn print_check_cfg_quote_in_value() {
    let mut check_cfg = CheckCfg::default();

    check_cfg
        .parse_print_check_cfg_line("cfg_e=\"quote_in_value\"\"")
        .unwrap();
    assert_eq!(
        *check_cfg.expecteds.get("cfg_e").unwrap(),
        ExpectedValues::Some(HashSet::from([Some("quote_in_value\"".to_string())]))
    );
}

#[test]
fn print_check_cfg_value_and_any() {
    let mut check_cfg = CheckCfg::default();

    // having both a value and `any()` shouldn't be possible but better
    // handle this correctly anyway

    check_cfg
        .parse_print_check_cfg_line("cfg_1=\"foo\"")
        .unwrap();
    check_cfg.parse_print_check_cfg_line("cfg_1=any()").unwrap();
    assert_eq!(
        *check_cfg.expecteds.get("cfg_1").unwrap(),
        ExpectedValues::Any
    );

    check_cfg.parse_print_check_cfg_line("cfg_2=any()").unwrap();
    check_cfg
        .parse_print_check_cfg_line("cfg_2=\"foo\"")
        .unwrap();
    assert_eq!(
        *check_cfg.expecteds.get("cfg_2").unwrap(),
        ExpectedValues::Any
    );
}

#[test]
#[should_panic]
fn print_check_cfg_missing_quote_value() {
    let mut check_cfg = CheckCfg::default();
    check_cfg.parse_print_check_cfg_line("foo=bar").unwrap();
}
