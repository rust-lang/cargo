//! Tests for the --config CLI option.

use std::{collections::HashMap, fs};

use cargo::util::context::Definition;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::paths;
use cargo_test_support::prelude::*;
use cargo_test_support::str;

use super::config::{
    assert_error, read_output, write_config_at, write_config_toml, GlobalContextBuilder,
};

#[cargo_test]
fn basic() {
    // Simple example.
    let gctx = GlobalContextBuilder::new()
        .config_arg("foo='bar'")
        .config_arg("net.git-fetch-with-cli=true")
        .build();
    assert_eq!(gctx.get::<String>("foo").unwrap(), "bar");
    assert_eq!(gctx.net_config().unwrap().git_fetch_with_cli, Some(true));
}

#[cargo_test]
fn cli_priority() {
    // Command line takes priority over files and env vars.
    write_config_toml(
        "
        demo_list = ['a']
        [build]
        jobs = 3
        rustc = 'file'
        [term]
        quiet = false
        verbose = false
        ",
    );
    let gctx = GlobalContextBuilder::new().build();
    assert_eq!(gctx.get::<i32>("build.jobs").unwrap(), 3);
    assert_eq!(gctx.get::<String>("build.rustc").unwrap(), "file");
    assert_eq!(gctx.get::<bool>("term.quiet").unwrap(), false);
    assert_eq!(gctx.get::<bool>("term.verbose").unwrap(), false);

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_BUILD_RUSTC", "env")
        .env("CARGO_TERM_VERBOSE", "false")
        .env("CARGO_NET_GIT_FETCH_WITH_CLI", "false")
        .config_arg("build.jobs=1")
        .config_arg("build.rustc='cli'")
        .config_arg("term.verbose=true")
        .config_arg("net.git-fetch-with-cli=true")
        .build();
    assert_eq!(gctx.get::<i32>("build.jobs").unwrap(), 1);
    assert_eq!(gctx.get::<String>("build.rustc").unwrap(), "cli");
    assert_eq!(gctx.get::<bool>("term.verbose").unwrap(), true);
    assert_eq!(gctx.net_config().unwrap().git_fetch_with_cli, Some(true));

    // Setting both term.verbose and term.quiet is invalid and is tested
    // in the run test suite.
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_TERM_QUIET", "false")
        .config_arg("term.quiet=true")
        .build();
    assert_eq!(gctx.get::<bool>("term.quiet").unwrap(), true);
}

#[cargo_test]
fn merge_primitives_for_multiple_cli_occurrences() {
    let config_path0 = ".cargo/file0.toml";
    write_config_at(config_path0, "k = 'file0'");
    let config_path1 = ".cargo/file1.toml";
    write_config_at(config_path1, "k = 'file1'");

    // k=env0
    let gctx = GlobalContextBuilder::new().env("CARGO_K", "env0").build();
    assert_eq!(gctx.get::<String>("k").unwrap(), "env0");

    // k=env0
    // --config k='cli0'
    // --config k='cli1'
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_K", "env0")
        .config_arg("k='cli0'")
        .config_arg("k='cli1'")
        .build();
    assert_eq!(gctx.get::<String>("k").unwrap(), "cli1");

    // Env has a lower priority when comparing with file from CLI arg.
    //
    // k=env0
    // --config k='cli0'
    // --config k='cli1'
    // --config .cargo/file0.toml
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_K", "env0")
        .config_arg("k='cli0'")
        .config_arg("k='cli1'")
        .config_arg(config_path0)
        .build();
    assert_eq!(gctx.get::<String>("k").unwrap(), "file0");

    // k=env0
    // --config k='cli0'
    // --config k='cli1'
    // --config .cargo/file0.toml
    // --config k='cli2'
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_K", "env0")
        .config_arg("k='cli0'")
        .config_arg("k='cli1'")
        .config_arg(config_path0)
        .config_arg("k='cli2'")
        .build();
    assert_eq!(gctx.get::<String>("k").unwrap(), "cli2");

    // k=env0
    // --config k='cli0'
    // --config k='cli1'
    // --config .cargo/file0.toml
    // --config k='cli2'
    // --config .cargo/file1.toml
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_K", "env0")
        .config_arg("k='cli0'")
        .config_arg("k='cli1'")
        .config_arg(config_path0)
        .config_arg("k='cli2'")
        .config_arg(config_path1)
        .build();
    assert_eq!(gctx.get::<String>("k").unwrap(), "file1");
}

#[cargo_test]
fn merges_array() {
    // Array entries are appended.
    write_config_toml(
        "
        [build]
        rustflags = ['--file']
        ",
    );
    let gctx = GlobalContextBuilder::new()
        .config_arg("build.rustflags = ['--cli']")
        .build();
    assert_eq!(
        gctx.get::<Vec<String>>("build.rustflags").unwrap(),
        ["--file", "--cli"]
    );

    // With normal env.
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_BUILD_RUSTFLAGS", "--env1 --env2")
        .config_arg("build.rustflags = ['--cli']")
        .build();
    assert_eq!(
        gctx.get::<Vec<String>>("build.rustflags").unwrap(),
        ["--file", "--env1", "--env2", "--cli"]
    );

    // With advanced-env.
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("advanced-env")
        .env("CARGO_BUILD_RUSTFLAGS", "--env")
        .config_arg("build.rustflags = ['--cli']")
        .build();
    assert_eq!(
        gctx.get::<Vec<String>>("build.rustflags").unwrap(),
        ["--file", "--env", "--cli"]
    );

    // Merges multiple instances.
    let gctx = GlobalContextBuilder::new()
        .config_arg("build.rustflags=['--one']")
        .config_arg("build.rustflags=['--two']")
        .build();
    assert_eq!(
        gctx.get::<Vec<String>>("build.rustflags").unwrap(),
        ["--file", "--one", "--two"]
    );
}

#[cargo_test]
fn string_list_array() {
    // Using the StringList type.
    write_config_toml(
        "
        [build]
        rustflags = ['--file']
        ",
    );
    let gctx = GlobalContextBuilder::new()
        .config_arg("build.rustflags = ['--cli']")
        .build();
    assert_eq!(
        gctx.get::<cargo::util::context::StringList>("build.rustflags")
            .unwrap()
            .as_slice(),
        ["--file", "--cli"]
    );

    // With normal env.
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_BUILD_RUSTFLAGS", "--env1 --env2")
        .config_arg("build.rustflags = ['--cli']")
        .build();
    assert_eq!(
        gctx.get::<cargo::util::context::StringList>("build.rustflags")
            .unwrap()
            .as_slice(),
        ["--file", "--env1", "--env2", "--cli"]
    );

    // With advanced-env.
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("advanced-env")
        .env("CARGO_BUILD_RUSTFLAGS", "['--env']")
        .config_arg("build.rustflags = ['--cli']")
        .build();
    assert_eq!(
        gctx.get::<cargo::util::context::StringList>("build.rustflags")
            .unwrap()
            .as_slice(),
        ["--file", "--env", "--cli"]
    );
}

#[cargo_test]
fn merges_table() {
    // Tables are merged.
    write_config_toml(
        "
        [foo]
        key1 = 1
        key2 = 2
        key3 = 3
        ",
    );
    let gctx = GlobalContextBuilder::new()
        .config_arg("foo.key2 = 4")
        .config_arg("foo.key3 = 5")
        .config_arg("foo.key4 = 6")
        .build();
    assert_eq!(gctx.get::<i32>("foo.key1").unwrap(), 1);
    assert_eq!(gctx.get::<i32>("foo.key2").unwrap(), 4);
    assert_eq!(gctx.get::<i32>("foo.key3").unwrap(), 5);
    assert_eq!(gctx.get::<i32>("foo.key4").unwrap(), 6);

    // With env.
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_FOO_KEY3", "7")
        .env("CARGO_FOO_KEY4", "8")
        .env("CARGO_FOO_KEY5", "9")
        .config_arg("foo.key2 = 4")
        .config_arg("foo.key3 = 5")
        .config_arg("foo.key4 = 6")
        .build();
    assert_eq!(gctx.get::<i32>("foo.key1").unwrap(), 1);
    assert_eq!(gctx.get::<i32>("foo.key2").unwrap(), 4);
    assert_eq!(gctx.get::<i32>("foo.key3").unwrap(), 5);
    assert_eq!(gctx.get::<i32>("foo.key4").unwrap(), 6);
    assert_eq!(gctx.get::<i32>("foo.key5").unwrap(), 9);
}

#[cargo_test]
fn merge_array_mixed_def_paths() {
    // Merging of arrays with different def sites.
    write_config_toml(
        "
        paths = ['file']
        ",
    );
    // Create a directory for CWD to differentiate the paths.
    let somedir = paths::root().join("somedir");
    fs::create_dir(&somedir).unwrap();
    let gctx = GlobalContextBuilder::new()
        .cwd(&somedir)
        .config_arg("paths=['cli']")
        // env is currently ignored for get_list()
        .env("CARGO_PATHS", "env")
        .build();
    let paths = gctx.get_list("paths").unwrap().unwrap();
    // The definition for the root value is somewhat arbitrary, but currently starts with the file because that is what is loaded first.
    assert_eq!(paths.definition, Definition::Path(paths::root()));
    assert_eq!(paths.val.len(), 2);
    assert_eq!(paths.val[0].0, "file");
    assert_eq!(paths.val[0].1.root(&gctx), paths::root());
    assert_eq!(paths.val[1].0, "cli");
    assert_eq!(paths.val[1].1.root(&gctx), somedir);
}

#[cargo_test]
fn enforces_format() {
    // These dotted key expressions should all be fine.
    let gctx = GlobalContextBuilder::new()
        .config_arg("a=true")
        .config_arg(" b.a = true ")
        .config_arg("c.\"b\".'a'=true")
        .config_arg("d.\"=\".'='=true")
        .config_arg("e.\"'\".'\"'=true")
        .build();
    assert_eq!(gctx.get::<bool>("a").unwrap(), true);
    assert_eq!(
        gctx.get::<HashMap<String, bool>>("b").unwrap(),
        HashMap::from([("a".to_string(), true)])
    );
    assert_eq!(
        gctx.get::<HashMap<String, HashMap<String, bool>>>("c")
            .unwrap(),
        HashMap::from([("b".to_string(), HashMap::from([("a".to_string(), true)]))])
    );
    assert_eq!(
        gctx.get::<HashMap<String, HashMap<String, bool>>>("d")
            .unwrap(),
        HashMap::from([("=".to_string(), HashMap::from([("=".to_string(), true)]))])
    );
    assert_eq!(
        gctx.get::<HashMap<String, HashMap<String, bool>>>("e")
            .unwrap(),
        HashMap::from([("'".to_string(), HashMap::from([("\"".to_string(), true)]))])
    );

    // But anything that's not a dotted key expression should be disallowed.
    let _ = GlobalContextBuilder::new()
        .config_arg("[a] foo=true")
        .build_err()
        .unwrap_err();
    let _ = GlobalContextBuilder::new()
        .config_arg("a = true\nb = true")
        .build_err()
        .unwrap_err();

    // We also disallow overwriting with tables since it makes merging unclear.
    let _ = GlobalContextBuilder::new()
        .config_arg("a = { first = true, second = false }")
        .build_err()
        .unwrap_err();
    let _ = GlobalContextBuilder::new()
        .config_arg("a = { first = true }")
        .build_err()
        .unwrap_err();
}

#[cargo_test]
fn unused_key() {
    // Unused key passed on command line.
    let gctx = GlobalContextBuilder::new()
        .config_arg("build.unused = 2")
        .build();

    gctx.build_config().unwrap();
    let output = read_output(gctx);
    let expected = str![[r#"
[WARNING] unused config key `build.unused` in `--config cli option`

"#]];
    assert_e2e().eq(&output, expected);
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
    let mut gctx = GlobalContextBuilder::new()
        .cwd(&somedir)
        .config_arg("b='cli1'")
        .config_arg("c='cli2'")
        .build();
    assert_eq!(gctx.get::<String>("a").unwrap(), "file1");
    assert_eq!(gctx.get::<String>("b").unwrap(), "cli1");
    assert_eq!(gctx.get::<String>("c").unwrap(), "cli2");

    gctx.reload_rooted_at(paths::root()).unwrap();

    assert_eq!(gctx.get::<Option<String>>("a").unwrap(), None);
    assert_eq!(gctx.get::<String>("b").unwrap(), "cli1");
    assert_eq!(gctx.get::<String>("c").unwrap(), "cli2");
}

#[cargo_test]
fn bad_parse() {
    // Fail to TOML parse.
    let gctx = GlobalContextBuilder::new().config_arg("abc").build_err();
    assert_error(
        gctx.unwrap_err(),
        "\
failed to parse value from --config argument `abc` as a dotted key expression

Caused by:
  TOML parse error at line 1, column 4
  |
1 | abc
  |    ^
expected `.`, `=`
",
    );

    let gctx = GlobalContextBuilder::new().config_arg("").build_err();
    assert_error(
        gctx.unwrap_err(),
        "--config argument `` was not a TOML dotted key expression (such as `build.jobs = 2`)",
    );
}

#[cargo_test]
fn too_many_values() {
    // Currently restricted to only 1 value.
    let gctx = GlobalContextBuilder::new()
        .config_arg("a=1\nb=2")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        "\
--config argument `a=1
b=2` was not a TOML dotted key expression (such as `build.jobs = 2`)",
    );
}

#[cargo_test]
fn no_disallowed_values() {
    let gctx = GlobalContextBuilder::new()
        .config_arg("registry.token=\"hello\"")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        "registry.token cannot be set through --config for security reasons",
    );
    let gctx = GlobalContextBuilder::new()
        .config_arg("registries.crates-io.token=\"hello\"")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        "registries.crates-io.token cannot be set through --config for security reasons",
    );
    let gctx = GlobalContextBuilder::new()
        .config_arg("registry.secret-key=\"hello\"")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        "registry.secret-key cannot be set through --config for security reasons",
    );
    let gctx = GlobalContextBuilder::new()
        .config_arg("registries.crates-io.secret-key=\"hello\"")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        "registries.crates-io.secret-key cannot be set through --config for security reasons",
    );
}

#[cargo_test]
fn no_inline_table_value() {
    // Disallow inline tables
    let gctx = GlobalContextBuilder::new()
        .config_arg("a.b={c = \"d\"}")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        "--config argument `a.b={c = \"d\"}` sets a value to an inline table, which is not accepted",
    );
}

#[cargo_test]
fn no_array_of_tables_values() {
    // Disallow array-of-tables when not in dotted form
    let gctx = GlobalContextBuilder::new()
        .config_arg("[[a.b]]\nc = \"d\"")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        "\
--config argument `[[a.b]]
c = \"d\"` was not a TOML dotted key expression (such as `build.jobs = 2`)",
    );
}

#[cargo_test]
fn no_comments() {
    // Disallow comments in dotted form.
    let gctx = GlobalContextBuilder::new()
        .config_arg("a.b = \"c\" # exactly")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        "\
--config argument `a.b = \"c\" # exactly` includes non-whitespace decoration",
    );

    let gctx = GlobalContextBuilder::new()
        .config_arg("# exactly\na.b = \"c\"")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        "\
--config argument `# exactly\na.b = \"c\"` includes non-whitespace decoration",
    );
}

#[cargo_test]
fn bad_cv_convert() {
    // ConfigValue does not support all TOML types.
    let gctx = GlobalContextBuilder::new()
        .config_arg("a=2019-12-01")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
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
    let gctx = GlobalContextBuilder::new()
        .config_arg("foo='a'")
        .config_arg("foo=['a']")
        .build_err();
    // This is a little repetitive, but hopefully the user can figure it out.
    assert_error(
        gctx.unwrap_err(),
        "\
failed to merge --config argument `foo=['a']`

Caused by:
  failed to merge key `foo` between --config cli option and --config cli option

Caused by:
  failed to merge config value from `--config cli option` into `--config cli option`: \
  expected string, but found array",
    );
}

#[cargo_test]
fn cli_path() {
    // --config path_to_file
    fs::write(paths::root().join("myconfig.toml"), "key = 123").unwrap();
    let gctx = GlobalContextBuilder::new()
        .cwd(paths::root())
        .config_arg("myconfig.toml")
        .build();
    assert_eq!(gctx.get::<u32>("key").unwrap(), 123);

    let gctx = GlobalContextBuilder::new()
        .config_arg("missing.toml")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        "\
failed to parse value from --config argument `missing.toml` as a dotted key expression

Caused by:
  TOML parse error at line 1, column 13
  |
1 | missing.toml
  |             ^
expected `.`, `=`
",
    );
}
