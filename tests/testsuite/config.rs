use std::borrow::Borrow;
use std::collections;
use std::fs;

use crate::support::{paths, project};
use cargo::core::{enable_nightly_features, Shell};
use cargo::util::config::{self, Config};
use cargo::util::toml::{self, VecStringOrBool as VSOB};
use serde::Deserialize;

fn lines_match(a: &str, b: &str) -> bool {
    // Perform a small amount of normalization for filesystem paths before we
    // send this to the `lines_match` function.
    crate::support::lines_match(&a.replace("\\", "/"), &b.replace("\\", "/"))
}

#[cargo_test]
fn read_env_vars_for_config() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
            build = "build.rs"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
            use std::env;
            fn main() {
                assert_eq!(env::var("NUM_JOBS").unwrap(), "100");
            }
        "#,
        )
        .build();

    p.cargo("build").env("CARGO_BUILD_JOBS", "100").run();
}

fn write_config(config: &str) {
    let path = paths::root().join(".cargo/config");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, config).unwrap();
}

fn new_config(env: &[(&str, &str)]) -> Config {
    enable_nightly_features(); // -Z advanced-env
    let output = Box::new(fs::File::create(paths::root().join("shell.out")).unwrap());
    let shell = Shell::from_write(output);
    let cwd = paths::root();
    let homedir = paths::home();
    let env = env
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let mut config = Config::new(shell, cwd, homedir);
    config.set_env(env);
    config
        .configure(
            0,
            None,
            &None,
            false,
            false,
            false,
            &None,
            &["advanced-env".into()],
        )
        .unwrap();
    config
}

fn assert_error<E: Borrow<failure::Error>>(error: E, msgs: &str) {
    let causes = error
        .borrow()
        .iter_chain()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    if !lines_match(msgs, &causes) {
        panic!(
            "Did not find expected:\n{}\nActual error:\n{}\n",
            msgs, causes
        );
    }
}

#[cargo_test]
fn get_config() {
    write_config(
        "\
[S]
f1 = 123
",
    );

    let config = new_config(&[]);

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct S {
        f1: Option<i64>,
    }
    let s: S = config.get("S").unwrap();
    assert_eq!(s, S { f1: Some(123) });
    let config = new_config(&[("CARGO_S_F1", "456")]);
    let s: S = config.get("S").unwrap();
    assert_eq!(s, S { f1: Some(456) });
}

#[cargo_test]
fn config_unused_fields() {
    write_config(
        "\
[S]
unused = 456
",
    );

    let config = new_config(&[("CARGO_S_UNUSED2", "1"), ("CARGO_S2_UNUSED", "2")]);

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct S {
        f1: Option<i64>,
    }
    // This prints a warning (verified below).
    let s: S = config.get("S").unwrap();
    assert_eq!(s, S { f1: None });
    // This does not print anything, we cannot easily/reliably warn for
    // environment variables.
    let s: S = config.get("S2").unwrap();
    assert_eq!(s, S { f1: None });

    // Verify the warnings.
    drop(config); // Paranoid about flushing the file.
    let path = paths::root().join("shell.out");
    let output = fs::read_to_string(path).unwrap();
    let expected = "\
warning: unused key `S.unused` in config file `[..]/.cargo/config`
";
    if !lines_match(expected, &output) {
        panic!(
            "Did not find expected:\n{}\nActual error:\n{}\n",
            expected, output
        );
    }
}

#[cargo_test]
fn config_load_toml_profile() {
    write_config(
        "\
[profile.dev]
opt-level = 's'
lto = true
codegen-units=4
debug = true
debug-assertions = true
rpath = true
panic = 'abort'
overflow-checks = true
incremental = true

[profile.dev.build-override]
opt-level = 1

[profile.dev.overrides.bar]
codegen-units = 9
",
    );

    let config = new_config(&[
        ("CARGO_PROFILE_DEV_CODEGEN_UNITS", "5"),
        ("CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_UNITS", "11"),
        ("CARGO_PROFILE_DEV_OVERRIDES_env_CODEGEN_UNITS", "13"),
        ("CARGO_PROFILE_DEV_OVERRIDES_bar_OPT_LEVEL", "2"),
    ]);

    // TODO: don't use actual `tomlprofile`.
    let p: toml::TomlProfile = config.get("profile.dev").unwrap();
    let mut overrides = collections::BTreeMap::new();
    let key = toml::ProfilePackageSpec::Spec(::cargo::core::PackageIdSpec::parse("bar").unwrap());
    let o_profile = toml::TomlProfile {
        opt_level: Some(toml::TomlOptLevel("2".to_string())),
        lto: None,
        codegen_units: Some(9),
        debug: None,
        debug_assertions: None,
        rpath: None,
        panic: None,
        overflow_checks: None,
        incremental: None,
        overrides: None,
        build_override: None,
    };
    overrides.insert(key, o_profile);
    let key = toml::ProfilePackageSpec::Spec(::cargo::core::PackageIdSpec::parse("env").unwrap());
    let o_profile = toml::TomlProfile {
        opt_level: None,
        lto: None,
        codegen_units: Some(13),
        debug: None,
        debug_assertions: None,
        rpath: None,
        panic: None,
        overflow_checks: None,
        incremental: None,
        overrides: None,
        build_override: None,
    };
    overrides.insert(key, o_profile);

    assert_eq!(
        p,
        toml::TomlProfile {
            opt_level: Some(toml::TomlOptLevel("s".to_string())),
            lto: Some(toml::StringOrBool::Bool(true)),
            codegen_units: Some(5),
            debug: Some(toml::U32OrBool::Bool(true)),
            debug_assertions: Some(true),
            rpath: Some(true),
            panic: Some("abort".to_string()),
            overflow_checks: Some(true),
            incremental: Some(true),
            overrides: Some(overrides),
            build_override: Some(Box::new(toml::TomlProfile {
                opt_level: Some(toml::TomlOptLevel("1".to_string())),
                lto: None,
                codegen_units: Some(11),
                debug: None,
                debug_assertions: None,
                rpath: None,
                panic: None,
                overflow_checks: None,
                incremental: None,
                overrides: None,
                build_override: None
            }))
        }
    );
}

#[cargo_test]
fn config_deserialize_any() {
    // Some tests to exercise deserialize_any for deserializers that need to
    // be told the format.
    write_config(
        "\
a = true
b = ['b']
c = ['c']
",
    );

    let config = new_config(&[
        ("CARGO_ENVB", "false"),
        ("CARGO_C", "['d']"),
        ("CARGO_ENVL", "['a', 'b']"),
    ]);

    let a = config.get::<VSOB>("a").unwrap();
    match a {
        VSOB::VecString(_) => panic!("expected bool"),
        VSOB::Bool(b) => assert_eq!(b, true),
    }
    let b = config.get::<VSOB>("b").unwrap();
    match b {
        VSOB::VecString(l) => assert_eq!(l, vec!["b".to_string()]),
        VSOB::Bool(_) => panic!("expected list"),
    }
    let c = config.get::<VSOB>("c").unwrap();
    match c {
        VSOB::VecString(l) => assert_eq!(l, vec!["c".to_string(), "d".to_string()]),
        VSOB::Bool(_) => panic!("expected list"),
    }
    let envb = config.get::<VSOB>("envb").unwrap();
    match envb {
        VSOB::VecString(_) => panic!("expected bool"),
        VSOB::Bool(b) => assert_eq!(b, false),
    }
    let envl = config.get::<VSOB>("envl").unwrap();
    match envl {
        VSOB::VecString(l) => assert_eq!(l, vec!["a".to_string(), "b".to_string()]),
        VSOB::Bool(_) => panic!("expected list"),
    }
}

#[cargo_test]
fn config_toml_errors() {
    write_config(
        "\
[profile.dev]
opt-level = 'foo'
",
    );

    let config = new_config(&[]);

    assert_error(
        config.get::<toml::TomlProfile>("profile.dev").unwrap_err(),
        "error in [..]/.cargo/config: \
         could not load config key `profile.dev.opt-level`: \
         must be an integer, `z`, or `s`, but found: foo",
    );

    let config = new_config(&[("CARGO_PROFILE_DEV_OPT_LEVEL", "asdf")]);

    assert_error(
        config.get::<toml::TomlProfile>("profile.dev").unwrap_err(),
        "error in environment variable `CARGO_PROFILE_DEV_OPT_LEVEL`: \
         could not load config key `profile.dev.opt-level`: \
         must be an integer, `z`, or `s`, but found: asdf",
    );
}

#[cargo_test]
fn load_nested() {
    write_config(
        "\
[nest.foo]
f1 = 1
f2 = 2
[nest.bar]
asdf = 3
",
    );

    let config = new_config(&[
        ("CARGO_NEST_foo_f2", "3"),
        ("CARGO_NESTE_foo_f1", "1"),
        ("CARGO_NESTE_foo_f2", "3"),
        ("CARGO_NESTE_bar_asdf", "3"),
    ]);

    type Nested = collections::HashMap<String, collections::HashMap<String, u8>>;

    let n: Nested = config.get("nest").unwrap();
    let mut expected = collections::HashMap::new();
    let mut foo = collections::HashMap::new();
    foo.insert("f1".to_string(), 1);
    foo.insert("f2".to_string(), 3);
    expected.insert("foo".to_string(), foo);
    let mut bar = collections::HashMap::new();
    bar.insert("asdf".to_string(), 3);
    expected.insert("bar".to_string(), bar);
    assert_eq!(n, expected);

    let n: Nested = config.get("neste").unwrap();
    assert_eq!(n, expected);
}

#[cargo_test]
fn get_errors() {
    write_config(
        "\
[S]
f1 = 123
f2 = 'asdf'
big = 123456789
",
    );

    let config = new_config(&[("CARGO_E_S", "asdf"), ("CARGO_E_BIG", "123456789")]);
    assert_error(
        config.get::<i64>("foo").unwrap_err(),
        "missing config key `foo`",
    );
    assert_error(
        config.get::<i64>("foo.bar").unwrap_err(),
        "missing config key `foo.bar`",
    );
    assert_error(
        config.get::<i64>("S.f2").unwrap_err(),
        "error in [..]/.cargo/config: `S.f2` expected an integer, but found a string",
    );
    assert_error(
        config.get::<u8>("S.big").unwrap_err(),
        "error in [..].cargo/config: could not load config key `S.big`: \
         invalid value: integer `123456789`, expected u8",
    );

    // Environment variable type errors.
    assert_error(
        config.get::<i64>("e.s").unwrap_err(),
        "error in environment variable `CARGO_E_S`: invalid digit found in string",
    );
    assert_error(
        config.get::<i8>("e.big").unwrap_err(),
        "error in environment variable `CARGO_E_BIG`: \
         could not load config key `e.big`: \
         invalid value: integer `123456789`, expected i8",
    );

    #[derive(Debug, Deserialize)]
    struct S {
        f1: i64,
        f2: String,
        f3: i64,
        big: i64,
    }
    assert_error(
        config.get::<S>("S").unwrap_err(),
        "missing config key `S.f3`",
    );
}

#[cargo_test]
fn config_get_option() {
    write_config(
        "\
[foo]
f1 = 1
",
    );

    let config = new_config(&[("CARGO_BAR_ASDF", "3")]);

    assert_eq!(config.get::<Option<i32>>("a").unwrap(), None);
    assert_eq!(config.get::<Option<i32>>("a.b").unwrap(), None);
    assert_eq!(config.get::<Option<i32>>("foo.f1").unwrap(), Some(1));
    assert_eq!(config.get::<Option<i32>>("bar.asdf").unwrap(), Some(3));
    assert_eq!(config.get::<Option<i32>>("bar.zzzz").unwrap(), None);
}

#[cargo_test]
fn config_bad_toml() {
    write_config("asdf");
    let config = new_config(&[]);
    assert_error(
        config.get::<i32>("foo").unwrap_err(),
        "\
could not load Cargo configuration
Caused by:
  could not parse TOML configuration in `[..]/.cargo/config`
Caused by:
  could not parse input as TOML
Caused by:
  expected an equals, found eof at line 1",
    );
}

#[cargo_test]
fn config_get_list() {
    write_config(
        "\
l1 = []
l2 = ['one', 'two']
l3 = 123
l4 = ['one', 'two']

[nested]
l = ['x']

[nested2]
l = ['y']

[nested-empty]
",
    );

    type L = Vec<String>;

    let config = new_config(&[
        ("CARGO_L4", "['three', 'four']"),
        ("CARGO_L5", "['a']"),
        ("CARGO_ENV_EMPTY", "[]"),
        ("CARGO_ENV_BLANK", ""),
        ("CARGO_ENV_NUM", "1"),
        ("CARGO_ENV_NUM_LIST", "[1]"),
        ("CARGO_ENV_TEXT", "asdf"),
        ("CARGO_LEPAIR", "['a', 'b']"),
        ("CARGO_NESTED2_L", "['z']"),
        ("CARGO_NESTEDE_L", "['env']"),
        ("CARGO_BAD_ENV", "[zzz]"),
    ]);

    assert_eq!(config.get::<L>("unset").unwrap(), vec![] as Vec<String>);
    assert_eq!(config.get::<L>("l1").unwrap(), vec![] as Vec<String>);
    assert_eq!(config.get::<L>("l2").unwrap(), vec!["one", "two"]);
    assert_error(
        config.get::<L>("l3").unwrap_err(),
        "\
invalid configuration for key `l3`
expected a list, but found a integer for `l3` in [..]/.cargo/config",
    );
    assert_eq!(
        config.get::<L>("l4").unwrap(),
        vec!["one", "two", "three", "four"]
    );
    assert_eq!(config.get::<L>("l5").unwrap(), vec!["a"]);
    assert_eq!(config.get::<L>("env-empty").unwrap(), vec![] as Vec<String>);
    assert_error(
        config.get::<L>("env-blank").unwrap_err(),
        "error in environment variable `CARGO_ENV_BLANK`: \
         should have TOML list syntax, found ``",
    );
    assert_error(
        config.get::<L>("env-num").unwrap_err(),
        "error in environment variable `CARGO_ENV_NUM`: \
         should have TOML list syntax, found `1`",
    );
    assert_error(
        config.get::<L>("env-num-list").unwrap_err(),
        "error in environment variable `CARGO_ENV_NUM_LIST`: \
         expected string, found integer",
    );
    assert_error(
        config.get::<L>("env-text").unwrap_err(),
        "error in environment variable `CARGO_ENV_TEXT`: \
         should have TOML list syntax, found `asdf`",
    );
    // "invalid number" here isn't the best error, but I think it's just toml.rs.
    assert_error(
        config.get::<L>("bad-env").unwrap_err(),
        "error in environment variable `CARGO_BAD_ENV`: \
         could not parse TOML list: invalid number at line 1",
    );

    // Try some other sequence-like types.
    assert_eq!(
        config
            .get::<(String, String, String, String)>("l4")
            .unwrap(),
        (
            "one".to_string(),
            "two".to_string(),
            "three".to_string(),
            "four".to_string()
        )
    );
    assert_eq!(config.get::<(String,)>("l5").unwrap(), ("a".to_string(),));

    // Tuple struct
    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct TupS(String, String);
    assert_eq!(
        config.get::<TupS>("lepair").unwrap(),
        TupS("a".to_string(), "b".to_string())
    );

    // Nested with an option.
    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct S {
        l: Option<Vec<String>>,
    }
    assert_eq!(config.get::<S>("nested-empty").unwrap(), S { l: None });
    assert_eq!(
        config.get::<S>("nested").unwrap(),
        S {
            l: Some(vec!["x".to_string()]),
        }
    );
    assert_eq!(
        config.get::<S>("nested2").unwrap(),
        S {
            l: Some(vec!["y".to_string(), "z".to_string()]),
        }
    );
    assert_eq!(
        config.get::<S>("nestede").unwrap(),
        S {
            l: Some(vec!["env".to_string()]),
        }
    );
}

#[cargo_test]
fn config_get_other_types() {
    write_config(
        "\
ns = 123
ns2 = 456
",
    );

    let config = new_config(&[("CARGO_NSE", "987"), ("CARGO_NS2", "654")]);

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct NewS(i32);
    assert_eq!(config.get::<NewS>("ns").unwrap(), NewS(123));
    assert_eq!(config.get::<NewS>("ns2").unwrap(), NewS(654));
    assert_eq!(config.get::<NewS>("nse").unwrap(), NewS(987));
    assert_error(
        config.get::<NewS>("unset").unwrap_err(),
        "missing config key `unset`",
    );
}

#[cargo_test]
fn config_relative_path() {
    write_config(&format!(
        "\
p1 = 'foo/bar'
p2 = '../abc'
p3 = 'b/c'
abs = '{}'
",
        paths::home().display(),
    ));

    let config = new_config(&[("CARGO_EPATH", "a/b"), ("CARGO_P3", "d/e")]);

    assert_eq!(
        config
            .get::<config::ConfigRelativePath>("p1")
            .unwrap()
            .path(),
        paths::root().join("foo/bar")
    );
    assert_eq!(
        config
            .get::<config::ConfigRelativePath>("p2")
            .unwrap()
            .path(),
        paths::root().join("../abc")
    );
    assert_eq!(
        config
            .get::<config::ConfigRelativePath>("p3")
            .unwrap()
            .path(),
        paths::root().join("d/e")
    );
    assert_eq!(
        config
            .get::<config::ConfigRelativePath>("abs")
            .unwrap()
            .path(),
        paths::home()
    );
    assert_eq!(
        config
            .get::<config::ConfigRelativePath>("epath")
            .unwrap()
            .path(),
        paths::root().join("a/b")
    );
}

#[cargo_test]
fn config_get_integers() {
    write_config(
        "\
npos = 123456789
nneg = -123456789
i64max = 9223372036854775807
",
    );

    let config = new_config(&[
        ("CARGO_EPOS", "123456789"),
        ("CARGO_ENEG", "-1"),
        ("CARGO_EI64MAX", "9223372036854775807"),
    ]);

    assert_eq!(
        config.get::<u64>("i64max").unwrap(),
        9_223_372_036_854_775_807
    );
    assert_eq!(
        config.get::<i64>("i64max").unwrap(),
        9_223_372_036_854_775_807
    );
    assert_eq!(
        config.get::<u64>("ei64max").unwrap(),
        9_223_372_036_854_775_807
    );
    assert_eq!(
        config.get::<i64>("ei64max").unwrap(),
        9_223_372_036_854_775_807
    );

    assert_error(
        config.get::<u32>("nneg").unwrap_err(),
        "error in [..].cargo/config: \
         could not load config key `nneg`: \
         invalid value: integer `-123456789`, expected u32",
    );
    assert_error(
        config.get::<u32>("eneg").unwrap_err(),
        "error in environment variable `CARGO_ENEG`: \
         could not load config key `eneg`: \
         invalid value: integer `-1`, expected u32",
    );
    assert_error(
        config.get::<i8>("npos").unwrap_err(),
        "error in [..].cargo/config: \
         could not load config key `npos`: \
         invalid value: integer `123456789`, expected i8",
    );
    assert_error(
        config.get::<i8>("epos").unwrap_err(),
        "error in environment variable `CARGO_EPOS`: \
         could not load config key `epos`: \
         invalid value: integer `123456789`, expected i8",
    );
}
