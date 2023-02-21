//! Tests for `include` config field.

use super::config::{assert_error, write_config, write_config_at, ConfigBuilder};
use cargo_test_support::{no_such_file_err_msg, project};

#[cargo_test]
fn gated() {
    // Requires -Z flag.
    write_config("include='other'");
    write_config_at(
        ".cargo/other",
        "
        othervalue = 1
        ",
    );
    let config = ConfigBuilder::new().build();
    assert_eq!(config.get::<Option<i32>>("othervalue").unwrap(), None);
    let config = ConfigBuilder::new().unstable_flag("config-include").build();
    assert_eq!(config.get::<i32>("othervalue").unwrap(), 1);
}

#[cargo_test]
fn simple() {
    // Simple test.
    write_config_at(
        ".cargo/config",
        "
        include = 'other'
        key1 = 1
        key2 = 2
        ",
    );
    write_config_at(
        ".cargo/other",
        "
        key2 = 3
        key3 = 4
        ",
    );
    let config = ConfigBuilder::new().unstable_flag("config-include").build();
    assert_eq!(config.get::<i32>("key1").unwrap(), 1);
    assert_eq!(config.get::<i32>("key2").unwrap(), 2);
    assert_eq!(config.get::<i32>("key3").unwrap(), 4);
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
        .with_stderr(
            "\
[CHECKING] foo v0.0.1 [..]
[RUNNING] `rustc [..]-W unused`
[FINISHED] [..]
",
        )
        .run();
    p.cargo("check -v -Z config-include")
        .masquerade_as_nightly_cargo(&["config-include"])
        .with_stderr(
            "\
[DIRTY] foo v0.0.1 ([..]): the rustflags changed
[CHECKING] foo v0.0.1 [..]
[RUNNING] `rustc [..]-W unsafe-code -W unused`
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn left_to_right() {
    // How it merges multiple includes.
    write_config_at(
        ".cargo/config",
        "
        include = ['one', 'two']
        primary = 1
        ",
    );
    write_config_at(
        ".cargo/one",
        "
        one = 1
        primary = 2
        ",
    );
    write_config_at(
        ".cargo/two",
        "
        two = 2
        primary = 3
        ",
    );
    let config = ConfigBuilder::new().unstable_flag("config-include").build();
    assert_eq!(config.get::<i32>("primary").unwrap(), 1);
    assert_eq!(config.get::<i32>("one").unwrap(), 1);
    assert_eq!(config.get::<i32>("two").unwrap(), 2);
}

#[cargo_test]
fn missing_file() {
    // Error when there's a missing file.
    write_config("include='missing'");
    let config = ConfigBuilder::new()
        .unstable_flag("config-include")
        .build_err();
    assert_error(
        config.unwrap_err(),
        &format!(
            "\
could not load Cargo configuration

Caused by:
  failed to load config include `missing` from `[..]/.cargo/config`

Caused by:
  failed to read configuration file `[..]/.cargo/missing`

Caused by:
  {}",
            no_such_file_err_msg()
        ),
    );
}

#[cargo_test]
fn cycle() {
    // Detects a cycle.
    write_config_at(".cargo/config", "include='one'");
    write_config_at(".cargo/one", "include='two'");
    write_config_at(".cargo/two", "include='config'");
    let config = ConfigBuilder::new()
        .unstable_flag("config-include")
        .build_err();
    assert_error(
        config.unwrap_err(),
        "\
could not load Cargo configuration

Caused by:
  failed to load config include `one` from `[..]/.cargo/config`

Caused by:
  failed to load config include `two` from `[..]/.cargo/one`

Caused by:
  failed to load config include `config` from `[..]/.cargo/two`

Caused by:
  config `include` cycle detected with path `[..]/.cargo/config`",
    );
}

#[cargo_test]
fn cli_include() {
    // Using --config with include.
    // CLI takes priority over files.
    write_config_at(
        ".cargo/config",
        "
        foo = 1
        bar = 2
        ",
    );
    write_config_at(".cargo/config-foo", "foo = 2");
    let config = ConfigBuilder::new()
        .unstable_flag("config-include")
        .config_arg("include='.cargo/config-foo'")
        .build();
    assert_eq!(config.get::<i32>("foo").unwrap(), 2);
    assert_eq!(config.get::<i32>("bar").unwrap(), 2);
}

#[cargo_test]
fn bad_format() {
    // Not a valid format.
    write_config("include = 1");
    let config = ConfigBuilder::new()
        .unstable_flag("config-include")
        .build_err();
    assert_error(
        config.unwrap_err(),
        "\
could not load Cargo configuration

Caused by:
  `include` expected a string or list, but found integer in `[..]/.cargo/config`",
    );
}

#[cargo_test]
fn cli_include_failed() {
    // Error message when CLI include fails to load.
    let config = ConfigBuilder::new()
        .unstable_flag("config-include")
        .config_arg("include='foobar'")
        .build_err();
    assert_error(
        config.unwrap_err(),
        &format!(
            "\
failed to load --config include

Caused by:
  failed to load config include `foobar` from `--config cli option`

Caused by:
  failed to read configuration file `[..]/foobar`

Caused by:
  {}",
            no_such_file_err_msg()
        ),
    );
}

#[cargo_test]
fn cli_merge_failed() {
    // Error message when CLI include merge fails.
    write_config("foo = ['a']");
    write_config_at(
        ".cargo/other",
        "
        foo = 'b'
        ",
    );
    let config = ConfigBuilder::new()
        .unstable_flag("config-include")
        .config_arg("include='.cargo/other'")
        .build_err();
    // Maybe this error message should mention it was from an include file?
    assert_error(
        config.unwrap_err(),
        "\
failed to merge --config key `foo` into `[..]/.cargo/config`

Caused by:
  failed to merge config value from `[..]/.cargo/other` into `[..]/.cargo/config`: \
  expected array, but found string",
    );
}

#[cargo_test]
fn cli_include_take_priority_over_env() {
    write_config_at(".cargo/include.toml", "k='include'");

    // k=env
    let config = ConfigBuilder::new().env("CARGO_K", "env").build();
    assert_eq!(config.get::<String>("k").unwrap(), "env");

    // k=env
    // --config 'include=".cargo/include.toml"'
    let config = ConfigBuilder::new()
        .env("CARGO_K", "env")
        .unstable_flag("config-include")
        .config_arg("include='.cargo/include.toml'")
        .build();
    assert_eq!(config.get::<String>("k").unwrap(), "include");

    // k=env
    // --config '.cargo/foo.toml'
    write_config_at(".cargo/foo.toml", "include='include.toml'");
    let config = ConfigBuilder::new()
        .env("CARGO_K", "env")
        .unstable_flag("config-include")
        .config_arg(".cargo/foo.toml")
        .build();
    assert_eq!(config.get::<String>("k").unwrap(), "include");
}
