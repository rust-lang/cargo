//! Tests for the --config CLI option.

use super::config::{assert_error, assert_match, read_output, write_config, ConfigBuilder};
use cargo::util::config::Definition;
use cargo_test_support::{paths, project};
use std::fs;

#[cargo_test]
fn config_gated() {
    // Requires -Zunstable-options
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build --config --config build.jobs=1")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] the `--config` flag is unstable, [..]
See [..]
See [..]
",
        )
        .run();
}

#[cargo_test]
fn basic() {
    // Simple example.
    let config = ConfigBuilder::new().config_arg("foo='bar'").build();
    assert_eq!(config.get::<String>("foo").unwrap(), "bar");
}

#[cargo_test]
fn cli_priority() {
    // Command line takes priority over files and env vars.
    write_config(
        "
        demo_list = ['a']
        [build]
        jobs = 3
        rustc = 'file'
        [term]
        verbose = false
        ",
    );
    let config = ConfigBuilder::new().build();
    assert_eq!(config.get::<i32>("build.jobs").unwrap(), 3);
    assert_eq!(config.get::<String>("build.rustc").unwrap(), "file");
    assert_eq!(config.get::<bool>("term.verbose").unwrap(), false);

    let config = ConfigBuilder::new()
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_BUILD_RUSTC", "env")
        .env("CARGO_TERM_VERBOSE", "false")
        .config_arg("build.jobs=1")
        .config_arg("build.rustc='cli'")
        .config_arg("term.verbose=true")
        .build();
    assert_eq!(config.get::<i32>("build.jobs").unwrap(), 1);
    assert_eq!(config.get::<String>("build.rustc").unwrap(), "cli");
    assert_eq!(config.get::<bool>("term.verbose").unwrap(), true);
}

#[cargo_test]
fn merges_array() {
    // Array entries are appended.
    write_config(
        "
        [build]
        rustflags = ['--file']
        ",
    );
    let config = ConfigBuilder::new()
        .config_arg("build.rustflags = ['--cli']")
        .build();
    assert_eq!(
        config.get::<Vec<String>>("build.rustflags").unwrap(),
        ["--file", "--cli"]
    );

    // With normal env.
    let config = ConfigBuilder::new()
        .env("CARGO_BUILD_RUSTFLAGS", "--env1 --env2")
        .config_arg("build.rustflags = ['--cli']")
        .build();
    // The order of cli/env is a little questionable here, but would require
    // much more complex merging logic.
    assert_eq!(
        config.get::<Vec<String>>("build.rustflags").unwrap(),
        ["--file", "--cli", "--env1", "--env2"]
    );

    // With advanced-env.
    let config = ConfigBuilder::new()
        .unstable_flag("advanced-env")
        .env("CARGO_BUILD_RUSTFLAGS", "--env")
        .config_arg("build.rustflags = ['--cli']")
        .build();
    assert_eq!(
        config.get::<Vec<String>>("build.rustflags").unwrap(),
        ["--file", "--cli", "--env"]
    );

    // Merges multiple instances.
    let config = ConfigBuilder::new()
        .config_arg("build.rustflags=['--one']")
        .config_arg("build.rustflags=['--two']")
        .build();
    assert_eq!(
        config.get::<Vec<String>>("build.rustflags").unwrap(),
        ["--file", "--one", "--two"]
    );
}

#[cargo_test]
fn string_list_array() {
    // Using the StringList type.
    write_config(
        "
        [build]
        rustflags = ['--file']
        ",
    );
    let config = ConfigBuilder::new()
        .config_arg("build.rustflags = ['--cli']")
        .build();
    assert_eq!(
        config
            .get::<cargo::util::config::StringList>("build.rustflags")
            .unwrap()
            .as_slice(),
        ["--file", "--cli"]
    );

    // With normal env.
    let config = ConfigBuilder::new()
        .env("CARGO_BUILD_RUSTFLAGS", "--env1 --env2")
        .config_arg("build.rustflags = ['--cli']")
        .build();
    assert_eq!(
        config
            .get::<cargo::util::config::StringList>("build.rustflags")
            .unwrap()
            .as_slice(),
        ["--file", "--cli", "--env1", "--env2"]
    );

    // With advanced-env.
    let config = ConfigBuilder::new()
        .unstable_flag("advanced-env")
        .env("CARGO_BUILD_RUSTFLAGS", "['--env']")
        .config_arg("build.rustflags = ['--cli']")
        .build();
    assert_eq!(
        config
            .get::<cargo::util::config::StringList>("build.rustflags")
            .unwrap()
            .as_slice(),
        ["--file", "--cli", "--env"]
    );
}

#[cargo_test]
fn merges_table() {
    // Tables are merged.
    write_config(
        "
        [foo]
        key1 = 1
        key2 = 2
        key3 = 3
        ",
    );
    let config = ConfigBuilder::new()
        .config_arg("foo.key2 = 4")
        .config_arg("foo.key3 = 5")
        .config_arg("foo.key4 = 6")
        .build();
    assert_eq!(config.get::<i32>("foo.key1").unwrap(), 1);
    assert_eq!(config.get::<i32>("foo.key2").unwrap(), 4);
    assert_eq!(config.get::<i32>("foo.key3").unwrap(), 5);
    assert_eq!(config.get::<i32>("foo.key4").unwrap(), 6);

    // With env.
    let config = ConfigBuilder::new()
        .env("CARGO_FOO_KEY3", "7")
        .env("CARGO_FOO_KEY4", "8")
        .env("CARGO_FOO_KEY5", "9")
        .config_arg("foo.key2 = 4")
        .config_arg("foo.key3 = 5")
        .config_arg("foo.key4 = 6")
        .build();
    assert_eq!(config.get::<i32>("foo.key1").unwrap(), 1);
    assert_eq!(config.get::<i32>("foo.key2").unwrap(), 4);
    assert_eq!(config.get::<i32>("foo.key3").unwrap(), 5);
    assert_eq!(config.get::<i32>("foo.key4").unwrap(), 6);
    assert_eq!(config.get::<i32>("foo.key5").unwrap(), 9);
}

#[cargo_test]
fn merge_array_mixed_def_paths() {
    // Merging of arrays with different def sites.
    write_config(
        "
        paths = ['file']
        ",
    );
    // Create a directory for CWD to differentiate the paths.
    let somedir = paths::root().join("somedir");
    fs::create_dir(&somedir).unwrap();
    let config = ConfigBuilder::new()
        .cwd(&somedir)
        .config_arg("paths=['cli']")
        // env is currently ignored for get_list()
        .env("CARGO_PATHS", "env")
        .build();
    let paths = config.get_list("paths").unwrap().unwrap();
    // The definition for the root value is somewhat arbitrary, but currently starts with the file because that is what is loaded first.
    assert_eq!(paths.definition, Definition::Path(paths::root()));
    assert_eq!(paths.val.len(), 2);
    assert_eq!(paths.val[0].0, "file");
    assert_eq!(paths.val[0].1.root(&config), paths::root());
    assert_eq!(paths.val[1].0, "cli");
    assert_eq!(paths.val[1].1.root(&config), somedir);
}

#[cargo_test]
fn unused_key() {
    // Unused key passed on command line.
    let config = ConfigBuilder::new()
        .config_arg("build={jobs=1, unused=2}")
        .build();

    config.build_config().unwrap();
    let output = read_output(config);
    let expected = "\
warning: unused config key `build.unused` in `--config cli option`
";
    assert_match(expected, &output);
}

#[cargo_test]
fn rerooted_remains() {
    // Re-rooting keeps cli args.
    let somedir = paths::root().join("somedir");
    fs::create_dir_all(somedir.join(".cargo")).unwrap();
    fs::write(
        somedir.join(".cargo").join("config"),
        "
        a = 'file1'
        b = 'file2'
        ",
    )
    .unwrap();
    let mut config = ConfigBuilder::new()
        .cwd(&somedir)
        .config_arg("b='cli1'")
        .config_arg("c='cli2'")
        .build();
    assert_eq!(config.get::<String>("a").unwrap(), "file1");
    assert_eq!(config.get::<String>("b").unwrap(), "cli1");
    assert_eq!(config.get::<String>("c").unwrap(), "cli2");

    config.reload_rooted_at(paths::root()).unwrap();

    assert_eq!(config.get::<Option<String>>("a").unwrap(), None);
    assert_eq!(config.get::<String>("b").unwrap(), "cli1");
    assert_eq!(config.get::<String>("c").unwrap(), "cli2");
}

#[cargo_test]
fn bad_parse() {
    // Fail to TOML parse.
    let config = ConfigBuilder::new().config_arg("abc").build_err();
    assert_error(
        config.unwrap_err(),
        "\
failed to parse --config argument `abc`

Caused by:
  expected an equals, found eof at line 1 column 4",
    );
}

#[cargo_test]
fn too_many_values() {
    // Currently restricted to only 1 value.
    let config = ConfigBuilder::new().config_arg("a=1\nb=2").build_err();
    assert_error(
        config.unwrap_err(),
        "\
--config argument `a=1
b=2` expected exactly one key=value pair, got 2 keys",
    );

    let config = ConfigBuilder::new().config_arg("").build_err();
    assert_error(
        config.unwrap_err(),
        "\
         --config argument `` expected exactly one key=value pair, got 0 keys",
    );
}

#[cargo_test]
fn bad_cv_convert() {
    // ConfigValue does not support all TOML types.
    let config = ConfigBuilder::new().config_arg("a=2019-12-01").build_err();
    assert_error(
        config.unwrap_err(),
        "\
failed to convert --config argument `a=2019-12-01`

Caused by:
  failed to parse key `a`

Caused by:
  found TOML configuration value of unknown type `datetime`",
    );
}

#[cargo_test]
fn fail_to_merge_multiple_args() {
    // Error message when multiple args fail to merge.
    let config = ConfigBuilder::new()
        .config_arg("foo='a'")
        .config_arg("foo=['a']")
        .build_err();
    // This is a little repetitive, but hopefully the user can figure it out.
    assert_error(
        config.unwrap_err(),
        "\
failed to merge --config argument `foo=['a']`

Caused by:
  failed to merge key `foo` between --config cli option and --config cli option

Caused by:
  failed to merge config value from `--config cli option` into `--config cli option`: \
  expected string, but found array",
    );
}
