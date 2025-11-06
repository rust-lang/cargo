//! Tests for `include` config field.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

use super::config::{GlobalContextBuilder, assert_error, write_config_at, write_config_toml};

#[cargo_test]
fn gated() {
    // Requires -Z flag.
    write_config_toml("include='other.toml'");
    write_config_at(
        ".cargo/other.toml",
        "
        othervalue = 1
        ",
    );
    let gctx = GlobalContextBuilder::new().build();
    assert_eq!(gctx.get::<Option<i32>>("othervalue").unwrap(), None);
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build();
    assert_eq!(gctx.get::<i32>("othervalue").unwrap(), 1);
}

#[cargo_test]
fn simple() {
    // Simple test.
    write_config_at(
        ".cargo/config.toml",
        "
        include = 'other.toml'
        key1 = 1
        key2 = 2
        ",
    );
    write_config_at(
        ".cargo/other.toml",
        "
        key2 = 3
        key3 = 4
        ",
    );
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build();
    assert_eq!(gctx.get::<i32>("key1").unwrap(), 1);
    assert_eq!(gctx.get::<i32>("key2").unwrap(), 2);
    assert_eq!(gctx.get::<i32>("key3").unwrap(), 4);
}

#[cargo_test]
fn enable_in_unstable_config() {
    // config-include enabled in the unstable config table:
    write_config_at(
        ".cargo/config.toml",
        "
        include = 'other.toml'
        key1 = 1
        key2 = 2

        [unstable]
        config-include = true
        ",
    );
    write_config_at(
        ".cargo/other.toml",
        "
        key2 = 3
        key3 = 4
        ",
    );
    let gctx = GlobalContextBuilder::new()
        .nightly_features_allowed(true)
        .build();
    assert_eq!(gctx.get::<i32>("key1").unwrap(), 1);
    assert_eq!(gctx.get::<i32>("key2").unwrap(), 2);
    assert_eq!(gctx.get::<i32>("key3").unwrap(), 4);
}

#[cargo_test]
fn mix_of_hierarchy_and_include() {
    write_config_at(
        "foo/.cargo/config.toml",
        "
        include = 'other.toml'
        key1 = 1

        # also make sure unstable flags merge in the correct order
        [unstable]
        features = ['1']
        ",
    );
    write_config_at(
        "foo/.cargo/other.toml",
        "
        key1 = 2
        key2 = 2

        [unstable]
        features = ['2']
        ",
    );
    write_config_at(
        ".cargo/config.toml",
        "
        include = 'other.toml'
        key1 = 3
        key2 = 3
        key3 = 3

        [unstable]
        features = ['3']
        ",
    );
    write_config_at(
        ".cargo/other.toml",
        "
        key1 = 4
        key2 = 4
        key3 = 4
        key4 = 4

        [unstable]
        features = ['4']
        ",
    );
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .cwd("foo")
        .nightly_features_allowed(true)
        .build();
    assert_eq!(gctx.get::<i32>("key1").unwrap(), 1);
    assert_eq!(gctx.get::<i32>("key2").unwrap(), 2);
    assert_eq!(gctx.get::<i32>("key3").unwrap(), 3);
    assert_eq!(gctx.get::<i32>("key4").unwrap(), 4);
    assert_eq!(
        gctx.get::<Vec<String>>("unstable.features").unwrap(),
        vec![
            "4".to_string(),
            "3".to_string(),
            "2".to_string(),
            "1".to_string()
        ]
    );
}

#[cargo_test]
fn mix_of_hierarchy_and_include_with_enable_in_unstable_config() {
    // `mix_of_hierarchy_and_include`, but with the config-include
    // feature itself enabled in the unstable config table:
    write_config_at(
        "foo/.cargo/config.toml",
        "
        include = 'other.toml'
        key1 = 1

        # also make sure unstable flags merge in the correct order
        [unstable]
        features = ['1']
        config-include = true
        ",
    );
    write_config_at(
        "foo/.cargo/other.toml",
        "
        key1 = 2
        key2 = 2

        [unstable]
        features = ['2']
        ",
    );
    write_config_at(
        ".cargo/config.toml",
        "
        include = 'other.toml'
        key1 = 3
        key2 = 3
        key3 = 3

        [unstable]
        features = ['3']
        ",
    );
    write_config_at(
        ".cargo/other.toml",
        "
        key1 = 4
        key2 = 4
        key3 = 4
        key4 = 4

        [unstable]
        features = ['4']
        ",
    );
    let gctx = GlobalContextBuilder::new()
        .cwd("foo")
        .nightly_features_allowed(true)
        .build();
    assert_eq!(gctx.get::<i32>("key1").unwrap(), 1);
    assert_eq!(gctx.get::<i32>("key2").unwrap(), 2);
    assert_eq!(gctx.get::<i32>("key3").unwrap(), 3);
    assert_eq!(gctx.get::<i32>("key4").unwrap(), 4);
    assert_eq!(
        gctx.get::<Vec<String>>("unstable.features").unwrap(),
        vec![
            "4".to_string(),
            "3".to_string(),
            "2".to_string(),
            "1".to_string()
        ]
    );
}

#[cargo_test]
fn works_with_cli() {
    write_config_at(
        ".cargo/config.toml",
        "
        include = 'other.toml'
        [build]
        rustflags = ['-W', 'unused']
        ",
    );
    write_config_at(
        ".cargo/other.toml",
        "
        [build]
        rustflags = ['-W', 'unsafe-code']
        ",
    );
    let p = project().file("src/lib.rs", "").build();
    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]-W unused`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check -v -Z config-include")
        .masquerade_as_nightly_cargo(&["config-include"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]-W unsafe-code -W unused`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn left_to_right_bottom_to_top() {
    // How it merges multiple nested includes.
    write_config_at(
        ".cargo/config.toml",
        "
        include = ['left-middle.toml', 'right-middle.toml']
        top = 1
        ",
    );
    write_config_at(
        ".cargo/right-middle.toml",
        "
        include = 'right-bottom.toml'
        top = 0
        right-middle = 0
        ",
    );
    write_config_at(
        ".cargo/right-bottom.toml",
        "
        top = -1
        right-middle = -1
        right-bottom = -1
        ",
    );
    write_config_at(
        ".cargo/left-middle.toml",
        "
        include = 'left-bottom.toml'
        top = -2
        right-middle = -2
        right-bottom = -2
        left-middle = -2
        ",
    );
    write_config_at(
        ".cargo/left-bottom.toml",
        "
        top = -3
        right-middle = -3
        right-bottom = -3
        left-middle = -3
        left-bottom = -3
        ",
    );
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build();
    assert_eq!(gctx.get::<i32>("top").unwrap(), 1);
    assert_eq!(gctx.get::<i32>("right-middle").unwrap(), 0);
    assert_eq!(gctx.get::<i32>("right-bottom").unwrap(), -1);
    assert_eq!(gctx.get::<i32>("left-middle").unwrap(), -2);
    assert_eq!(gctx.get::<i32>("left-bottom").unwrap(), -3);
}

#[cargo_test]
fn missing_file() {
    // Error when there's a missing file.
    write_config_toml("include='missing.toml'");
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        str![[r#"
could not load Cargo configuration

Caused by:
  failed to load config include `missing.toml` from `[ROOT]/.cargo/config.toml`

Caused by:
  failed to read configuration file `[ROOT]/.cargo/missing.toml`

Caused by:
  [NOT_FOUND]
"#]],
    );
}

#[cargo_test]
fn wrong_file_extension() {
    // Error when it doesn't end with `.toml`.
    write_config_toml("include='config.png'");
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        str![[r#"
could not load Cargo configuration

Caused by:
  expected a config include path ending with `.toml`, but found `config.png` from `[ROOT]/.cargo/config.toml`
"#]],
    );
}

#[cargo_test]
fn cycle() {
    // Detects a cycle.
    write_config_at(".cargo/config.toml", "include='one.toml'");
    write_config_at(".cargo/one.toml", "include='two.toml'");
    write_config_at(".cargo/two.toml", "include='config.toml'");
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        str![[r#"
could not load Cargo configuration

Caused by:
  failed to load config include `one.toml` from `[ROOT]/.cargo/config.toml`

Caused by:
  failed to load config include `two.toml` from `[ROOT]/.cargo/one.toml`

Caused by:
  failed to load config include `config.toml` from `[ROOT]/.cargo/two.toml`

Caused by:
  config `include` cycle detected with path `[ROOT]/.cargo/config.toml`
"#]],
    );
}

#[cargo_test]
fn cli_include() {
    // Using --config with include.
    // CLI takes priority over files.
    write_config_at(
        ".cargo/config.toml",
        "
        foo = 1
        bar = 2
        ",
    );
    write_config_at(".cargo/config-foo.toml", "foo = 2");
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .config_arg("include='.cargo/config-foo.toml'")
        .build();
    assert_eq!(gctx.get::<i32>("foo").unwrap(), 2);
    assert_eq!(gctx.get::<i32>("bar").unwrap(), 2);
}

#[cargo_test]
fn bad_format() {
    // Not a valid format.
    write_config_toml("include = 1");
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        str![[r#"
could not load Cargo configuration

Caused by:
  expected a string or list of strings, but found integer at `include` in `[ROOT]/.cargo/config.toml
"#]],
    );
}

#[cargo_test]
fn cli_include_failed() {
    // Error message when CLI include fails to load.
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .config_arg("include='foobar.toml'")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        str![[r#"
failed to load --config include

Caused by:
  failed to load config include `foobar.toml` from `--config cli option`

Caused by:
  failed to read configuration file `[ROOT]/foobar.toml`

Caused by:
  [NOT_FOUND]
"#]],
    );
}

#[cargo_test]
fn cli_merge_failed() {
    // Error message when CLI include merge fails.
    write_config_toml("foo = ['a']");
    write_config_at(
        ".cargo/other.toml",
        "
        foo = 'b'
        ",
    );
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .config_arg("include='.cargo/other.toml'")
        .build_err();
    // Maybe this error message should mention it was from an include file?
    assert_error(
        gctx.unwrap_err(),
        str![[r#"
failed to merge key `foo` between [ROOT]/.cargo/config.toml and [ROOT]/.cargo/other.toml

Caused by:
  failed to merge config value from `[ROOT]/.cargo/other.toml` into `[ROOT]/.cargo/config.toml`: expected array, but found string
"#]],
    );
}

#[cargo_test]
fn cli_include_take_priority_over_env() {
    write_config_at(".cargo/include.toml", "k='include'");

    // k=env
    let gctx = GlobalContextBuilder::new().env("CARGO_K", "env").build();
    assert_eq!(gctx.get::<String>("k").unwrap(), "env");

    // k=env
    // --config 'include=".cargo/include.toml"'
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_K", "env")
        .unstable_flag("config-include")
        .config_arg("include='.cargo/include.toml'")
        .build();
    assert_eq!(gctx.get::<String>("k").unwrap(), "include");

    // k=env
    // --config '.cargo/foo.toml'
    write_config_at(".cargo/foo.toml", "include='include.toml'");
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_K", "env")
        .unstable_flag("config-include")
        .config_arg(".cargo/foo.toml")
        .build();
    assert_eq!(gctx.get::<String>("k").unwrap(), "include");
}

#[cargo_test]
fn inline_table_style() {
    write_config_at(
        ".cargo/config.toml",
        "
        include = ['simple.toml', { path = 'other.toml' }]
        key1 = 1
        key2 = 2
        ",
    );
    write_config_at(
        ".cargo/simple.toml",
        "
        key2 = 3
        key3 = 4
        ",
    );
    write_config_at(
        ".cargo/other.toml",
        "
        key3 = 5
        key4 = 6
        ",
    );

    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build();
    assert_eq!(gctx.get::<i32>("key1").unwrap(), 1);
    assert_eq!(gctx.get::<i32>("key2").unwrap(), 2);
    assert_eq!(gctx.get::<i32>("key3").unwrap(), 5);
    assert_eq!(gctx.get::<i32>("key4").unwrap(), 6);
}

#[cargo_test]
fn array_of_tables_style() {
    write_config_at(
        ".cargo/config.toml",
        "
        key1 = 1
        key2 = 2

        [[include]]
        path = 'other1.toml'

        [[include]]
        path = 'other2.toml'
        ",
    );
    write_config_at(
        ".cargo/other1.toml",
        "
        key2 = 3
        key3 = 4
        ",
    );
    write_config_at(
        ".cargo/other2.toml",
        "
        key3 = 5
        key4 = 6
        ",
    );

    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build();
    assert_eq!(gctx.get::<i32>("key1").unwrap(), 1);
    assert_eq!(gctx.get::<i32>("key2").unwrap(), 2);
    assert_eq!(gctx.get::<i32>("key3").unwrap(), 5);
    assert_eq!(gctx.get::<i32>("key4").unwrap(), 6);
}

#[cargo_test]
fn table_with_unknown_fields() {
    // Unknown fields should be ignored for forward compatibility
    write_config_at(
        ".cargo/config.toml",
        "
        key1 = 1

        [[include]]
        path = 'other.toml'
        unknown_foo = true
        unknown_bar = 123
        ",
    );
    write_config_at(
        ".cargo/other.toml",
        "
        key2 = 2
        ",
    );

    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build();
    assert_eq!(gctx.get::<i32>("key1").unwrap(), 1);
    assert_eq!(gctx.get::<i32>("key2").unwrap(), 2);
}

#[cargo_test]
fn table_missing_required_field() {
    // Missing required field should fail
    write_config_at(
        ".cargo/config.toml",
        "
        key1 = 1
        [[include]]
        random_field = true
        ",
    );

    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        str![[r#"
could not load Cargo configuration

Caused by:
  missing field `path` at `include[0]` in `[ROOT]/.cargo/config.toml`
"#]],
    );
}

#[cargo_test]
fn optional_include_missing_and_existing() {
    write_config_at(
        ".cargo/config.toml",
        "
        key1 = 1

        [[include]]
        path = 'missing.toml'
        optional = true

        [[include]]
        path = 'other.toml'
        optional = true
        ",
    );
    write_config_at(
        ".cargo/other.toml",
        "
        key2 = 2
        ",
    );

    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build();
    assert_eq!(gctx.get::<i32>("key1").unwrap(), 1);
    assert_eq!(gctx.get::<i32>("key2").unwrap(), 2);
}

#[cargo_test]
fn optional_false_missing_file() {
    write_config_at(
        ".cargo/config.toml",
        "
        key1 = 1

        [[include]]
        path = 'missing.toml'
        optional = false
        ",
    );

    let gctx = GlobalContextBuilder::new()
        .unstable_flag("config-include")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        str![[r#"
could not load Cargo configuration

Caused by:
  failed to load config include `missing.toml` from `[ROOT]/.cargo/config.toml`

Caused by:
  failed to read configuration file `[ROOT]/.cargo/missing.toml`

Caused by:
  [NOT_FOUND]
"#]],
    );
}
