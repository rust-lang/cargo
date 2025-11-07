//! Tests for config settings.

use std::borrow::Borrow;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::os;
use std::path::{Path, PathBuf};

use crate::prelude::*;
use cargo::CargoResult;
use cargo::core::features::{GitFeatures, GitoxideFeatures};
use cargo::core::{PackageIdSpec, Shell};
use cargo::util::auth::RegistryConfig;
use cargo::util::context::Value;
use cargo::util::context::{
    self, Definition, GlobalContext, JobsConfig, SslVersionConfig, StringList,
};
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::str;
use cargo_test_support::{paths, project, project_in_home, symlink_supported, t};
use cargo_util_schemas::manifest::TomlTrimPaths;
use cargo_util_schemas::manifest::TomlTrimPathsValue;
use cargo_util_schemas::manifest::{self as cargo_toml, TomlDebugInfo, VecStringOrBool as VSOB};
use serde::Deserialize;

/// Helper for constructing a `GlobalContext` object.
pub struct GlobalContextBuilder {
    env: HashMap<String, String>,
    unstable: Vec<String>,
    config_args: Vec<String>,
    cwd: Option<PathBuf>,
    root: Option<PathBuf>,
    enable_nightly_features: bool,
}

impl GlobalContextBuilder {
    pub fn new() -> GlobalContextBuilder {
        GlobalContextBuilder {
            env: HashMap::new(),
            unstable: Vec::new(),
            config_args: Vec::new(),
            root: None,
            cwd: None,
            enable_nightly_features: false,
        }
    }

    /// Passes a `-Z` flag.
    pub fn unstable_flag(&mut self, s: impl Into<String>) -> &mut Self {
        self.unstable.push(s.into());
        self
    }

    /// Sets an environment variable.
    pub fn env(&mut self, key: impl Into<String>, val: impl Into<String>) -> &mut Self {
        self.env.insert(key.into(), val.into());
        self
    }

    /// Unconditionally enable nightly features, even on stable channels.
    pub fn nightly_features_allowed(&mut self, allowed: bool) -> &mut Self {
        self.enable_nightly_features = allowed;
        self
    }

    /// Passes a `--config` flag.
    pub fn config_arg(&mut self, arg: impl Into<String>) -> &mut Self {
        self.config_args.push(arg.into());
        self
    }

    /// Sets the current working directory where config files will be loaded.
    ///
    /// Default is the root from [`GlobalContextBuilder::root`] or [`paths::root`].
    pub fn cwd(&mut self, path: impl AsRef<Path>) -> &mut Self {
        let path = path.as_ref();
        let cwd = self
            .root
            .as_ref()
            .map_or_else(|| paths::root().join(path), |r| r.join(path));
        self.cwd = Some(cwd);
        self
    }

    /// Sets the test root directory.
    ///
    /// This generally should not be necessary. It is only useful if you want
    /// to create a [`GlobalContext`] from within a thread. Since Cargo's
    /// testsuite uses thread-local storage, this can be used to avoid accessing
    /// that thread-local storage.
    ///
    /// Default is [`paths::root`].
    pub fn root(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.root = Some(path.into());
        self
    }

    /// Creates the [`GlobalContext`].
    pub fn build(&self) -> GlobalContext {
        self.build_err().unwrap()
    }

    /// Creates the [`GlobalContext`], returning a Result.
    pub fn build_err(&self) -> CargoResult<GlobalContext> {
        let root = self.root.clone().unwrap_or_else(|| paths::root());
        let output = Box::new(fs::File::create(root.join("shell.out")).unwrap());
        let shell = Shell::from_write(output);
        let cwd = self.cwd.clone().unwrap_or_else(|| root.clone());
        let homedir = root.join("home").join(".cargo");
        let mut gctx = GlobalContext::new(shell, cwd, homedir);
        gctx.nightly_features_allowed = self.enable_nightly_features || !self.unstable.is_empty();
        gctx.set_env(self.env.clone());
        gctx.set_search_stop_path(&root);
        gctx.configure(
            0,
            false,
            None,
            false,
            false,
            false,
            &None,
            &self.unstable,
            &self.config_args,
        )?;
        Ok(gctx)
    }
}

fn new_gctx() -> GlobalContext {
    GlobalContextBuilder::new().build()
}

/// Read the output from Config.
pub fn read_output(gctx: GlobalContext) -> String {
    drop(gctx); // Paranoid about flushing the file.
    let path = paths::root().join("shell.out");
    fs::read_to_string(path).unwrap()
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

    p.cargo("check").env("CARGO_BUILD_JOBS", "100").run();
}

pub fn write_config_extless(config: &str) {
    write_config_at(paths::root().join(".cargo/config"), config);
}

pub fn write_config_at(path: impl AsRef<Path>, contents: &str) {
    let path = paths::root().join(path.as_ref());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

pub fn write_config_toml(config: &str) {
    write_config_at(paths::root().join(".cargo/config.toml"), config);
}

#[cfg(unix)]
fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
    os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
    os::windows::fs::symlink_file(target, link)
}

fn make_config_symlink_to_config_toml_absolute() {
    let toml_path = paths::root().join(".cargo/config.toml");
    let symlink_path = paths::root().join(".cargo/config");
    t!(symlink_file(&toml_path, &symlink_path));
}

fn make_config_symlink_to_config_toml_relative() {
    let symlink_path = paths::root().join(".cargo/config");
    t!(symlink_file(Path::new("config.toml"), &symlink_path));
}

fn rename_config_toml_to_config_replacing_with_symlink() {
    let root = paths::root();
    t!(fs::rename(
        root.join(".cargo/config.toml"),
        root.join(".cargo/config")
    ));
    t!(symlink_file(
        Path::new("config"),
        &root.join(".cargo/config.toml")
    ));
}

#[track_caller]
pub fn assert_error<E: Borrow<anyhow::Error>>(error: E, msgs: impl IntoData) {
    let causes = error
        .borrow()
        .chain()
        .enumerate()
        .map(|(i, e)| {
            if i == 0 {
                e.to_string()
            } else {
                format!("Caused by:\n  {}", e)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    assert_e2e().eq(&causes, msgs);
}

#[cargo_test]
fn get_config() {
    write_config_toml(
        "\
[S]
f1 = 123
",
    );

    let gctx = new_gctx();

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct S {
        f1: Option<i64>,
    }
    let s: S = gctx.get("S").unwrap();
    assert_eq!(s, S { f1: Some(123) });
    let gctx = GlobalContextBuilder::new().env("CARGO_S_F1", "456").build();
    let s: S = gctx.get("S").unwrap();
    assert_eq!(s, S { f1: Some(456) });
}

#[cfg(windows)]
#[cargo_test]
fn environment_variable_casing() {
    // Issue #11814: Environment variable names are case-insensitive on Windows.
    let gctx = GlobalContextBuilder::new()
        .env("Path", "abc")
        .env("Two-Words", "abc")
        .env("two_words", "def")
        .build();

    let var = gctx.get_env("PATH").unwrap();
    assert_eq!(var, String::from("abc"));

    let var = gctx.get_env("path").unwrap();
    assert_eq!(var, String::from("abc"));

    let var = gctx.get_env("TWO-WORDS").unwrap();
    assert_eq!(var, String::from("abc"));

    // Make sure that we can still distinguish between dashes and underscores
    // in variable names.
    let var = gctx.get_env("Two_Words").unwrap();
    assert_eq!(var, String::from("def"));
}

#[cargo_test]
fn config_works_without_extension() {
    write_config_extless(
        "\
[foo]
f1 = 1
",
    );

    let gctx = new_gctx();

    assert_eq!(gctx.get::<Option<i32>>("foo.f1").unwrap(), Some(1));

    // It should NOT have warned for the symlink.
    let output = read_output(gctx);
    let expected = str![[r#"
[WARNING] `[ROOT]/.cargo/config` is deprecated in favor of `config.toml`
  |
  = [HELP] if you need to support cargo 1.38 or earlier, you can symlink `config` to `config.toml`

"#]];
    assert_e2e().eq(&output, expected);
}

#[cargo_test]
fn home_config_works_without_extension() {
    write_config_at(
        paths::cargo_home().join("config"),
        "\
[foo]
f1 = 1
",
    );
    let p = project_in_home("foo").file("src/lib.rs", "").build();

    p.cargo("-vV")
        .with_stderr_data(str![[r#"
[WARNING] `[ROOT]/home/.cargo/config` is deprecated in favor of `config.toml`
  |
  = [HELP] if you need to support cargo 1.38 or earlier, you can symlink `config` to `config.toml`

"#]])
        .run();
}

#[cargo_test]
fn config_ambiguous_filename_symlink_doesnt_warn() {
    // Windows requires special permissions to create symlinks.
    // If we don't have permission, just skip this test.
    if !symlink_supported() {
        return;
    };

    write_config_toml(
        "\
[foo]
f1 = 1
",
    );

    make_config_symlink_to_config_toml_absolute();

    let gctx = new_gctx();

    assert_eq!(gctx.get::<Option<i32>>("foo.f1").unwrap(), Some(1));

    // It should NOT have warned for the symlink.
    let output = read_output(gctx);
    assert_e2e().eq(&output, str![[""]]);
}

#[cargo_test]
fn config_ambiguous_filename_symlink_doesnt_warn_relative() {
    // Windows requires special permissions to create symlinks.
    // If we don't have permission, just skip this test.
    if !symlink_supported() {
        return;
    };

    write_config_toml(
        "\
[foo]
f1 = 1
",
    );

    make_config_symlink_to_config_toml_relative();

    let gctx = new_gctx();

    assert_eq!(gctx.get::<Option<i32>>("foo.f1").unwrap(), Some(1));

    // It should NOT have warned for the symlink.
    let output = read_output(gctx);
    assert_e2e().eq(&output, str![[""]]);
}

#[cargo_test]
fn config_ambiguous_filename_symlink_doesnt_warn_backward() {
    // Windows requires special permissions to create symlinks.
    // If we don't have permission, just skip this test.
    if !symlink_supported() {
        return;
    };

    write_config_toml(
        "\
[foo]
f1 = 1
",
    );

    rename_config_toml_to_config_replacing_with_symlink();

    let gctx = new_gctx();

    assert_eq!(gctx.get::<Option<i32>>("foo.f1").unwrap(), Some(1));

    // It should NOT have warned for this situation.
    let output = read_output(gctx);
    assert_e2e().eq(&output, str![[""]]);
}

#[cargo_test]
fn config_ambiguous_filename() {
    write_config_extless(
        "\
[foo]
f1 = 1
",
    );

    write_config_toml(
        "\
[foo]
f1 = 2
",
    );

    let gctx = new_gctx();

    // It should use the value from the one without the extension for
    // backwards compatibility.
    assert_eq!(gctx.get::<Option<i32>>("foo.f1").unwrap(), Some(1));

    // But it also should have warned.
    let output = read_output(gctx);
    let expected = str![[r#"
[WARNING] both `[ROOT]/.cargo/config` and `[ROOT]/.cargo/config.toml` exist. Using `[ROOT]/.cargo/config`

"#]];
    assert_e2e().eq(&output, expected);
}

#[cargo_test]
fn config_unused_fields() {
    write_config_toml(
        "\
[S]
unused = 456
",
    );

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_S_UNUSED2", "1")
        .env("CARGO_S2_UNUSED", "2")
        .build();

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct S {
        f1: Option<i64>,
    }
    // This prints a warning (verified below).
    let s: S = gctx.get("S").unwrap();
    assert_eq!(s, S { f1: None });
    // This does not print anything, we cannot easily/reliably warn for
    // environment variables.
    let s: S = gctx.get("S2").unwrap();
    assert_eq!(s, S { f1: None });

    // Verify the warnings.
    let output = read_output(gctx);
    let expected = str![[r#"
[WARNING] unused config key `S.unused` in `[ROOT]/.cargo/config.toml`

"#]];
    assert_e2e().eq(&output, expected);
}

#[cargo_test]
fn config_load_toml_profile() {
    write_config_toml(
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

[profile.dev.package.bar]
codegen-units = 9

[profile.no-lto]
inherits = 'dev'
dir-name = 'without-lto'
lto = false
",
    );

    let gctx = GlobalContextBuilder::new()
        .unstable_flag("advanced-env")
        .env("CARGO_PROFILE_DEV_CODEGEN_UNITS", "5")
        .env("CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_UNITS", "11")
        .env("CARGO_PROFILE_DEV_PACKAGE_env_CODEGEN_UNITS", "13")
        .env("CARGO_PROFILE_DEV_PACKAGE_bar_OPT_LEVEL", "2")
        .build();

    // TODO: don't use actual `tomlprofile`.
    let p: cargo_toml::TomlProfile = gctx.get("profile.dev").unwrap();
    let mut packages = BTreeMap::new();
    let key =
        cargo_toml::ProfilePackageSpec::Spec(::cargo::core::PackageIdSpec::parse("bar").unwrap());
    let o_profile = cargo_toml::TomlProfile {
        opt_level: Some(cargo_toml::TomlOptLevel("2".to_string())),
        codegen_units: Some(9),
        ..Default::default()
    };
    packages.insert(key, o_profile);
    let key =
        cargo_toml::ProfilePackageSpec::Spec(::cargo::core::PackageIdSpec::parse("env").unwrap());
    let o_profile = cargo_toml::TomlProfile {
        codegen_units: Some(13),
        ..Default::default()
    };
    packages.insert(key, o_profile);

    assert_eq!(
        p,
        cargo_toml::TomlProfile {
            opt_level: Some(cargo_toml::TomlOptLevel("s".to_string())),
            lto: Some(cargo_toml::StringOrBool::Bool(true)),
            codegen_units: Some(5),
            debug: Some(cargo_toml::TomlDebugInfo::Full),
            debug_assertions: Some(true),
            rpath: Some(true),
            panic: Some("abort".to_string()),
            overflow_checks: Some(true),
            incremental: Some(true),
            package: Some(packages),
            build_override: Some(Box::new(cargo_toml::TomlProfile {
                opt_level: Some(cargo_toml::TomlOptLevel("1".to_string())),
                codegen_units: Some(11),
                ..Default::default()
            })),
            ..Default::default()
        }
    );

    let p: cargo_toml::TomlProfile = gctx.get("profile.no-lto").unwrap();
    assert_eq!(
        p,
        cargo_toml::TomlProfile {
            lto: Some(cargo_toml::StringOrBool::Bool(false)),
            dir_name: Some(String::from("without-lto")),
            inherits: Some(String::from("dev")),
            ..Default::default()
        }
    );
}

#[cargo_test]
fn profile_env_var_prefix() {
    // Check for a bug with collision on DEBUG vs DEBUG_ASSERTIONS.
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_PROFILE_DEV_DEBUG_ASSERTIONS", "false")
        .build();
    let p: cargo_toml::TomlProfile = gctx.get("profile.dev").unwrap();
    assert_eq!(p.debug_assertions, Some(false));
    assert_eq!(p.debug, None);

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_PROFILE_DEV_DEBUG", "1")
        .build();
    let p: cargo_toml::TomlProfile = gctx.get("profile.dev").unwrap();
    assert_eq!(p.debug_assertions, None);
    assert_eq!(p.debug, Some(cargo_toml::TomlDebugInfo::Limited));

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_PROFILE_DEV_DEBUG_ASSERTIONS", "false")
        .env("CARGO_PROFILE_DEV_DEBUG", "1")
        .build();
    let p: cargo_toml::TomlProfile = gctx.get("profile.dev").unwrap();
    assert_eq!(p.debug_assertions, Some(false));
    assert_eq!(p.debug, Some(cargo_toml::TomlDebugInfo::Limited));
}

#[cargo_test]
fn config_deserialize_any() {
    // Some tests to exercise deserialize_any for deserializers that need to
    // be told the format.
    write_config_toml(
        "\
a = true
b = ['b']
c = ['c']
",
    );

    // advanced-env
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("advanced-env")
        .env("CARGO_ENVB", "false")
        .env("CARGO_C", "['d']")
        .env("CARGO_ENVL", "['a', 'b']")
        .build();
    assert_eq!(gctx.get::<VSOB>("a").unwrap(), VSOB::Bool(true));
    assert_eq!(
        gctx.get::<VSOB>("b").unwrap(),
        VSOB::VecString(vec!["b".to_string()])
    );
    assert_eq!(
        gctx.get::<VSOB>("c").unwrap(),
        VSOB::VecString(vec!["c".to_string(), "d".to_string()])
    );
    assert_eq!(gctx.get::<VSOB>("envb").unwrap(), VSOB::Bool(false));
    assert_eq!(
        gctx.get::<VSOB>("envl").unwrap(),
        VSOB::VecString(vec!["a".to_string(), "b".to_string()])
    );

    // Demonstrate where merging logic isn't very smart. This could be improved.
    let gctx = GlobalContextBuilder::new().env("CARGO_A", "x y").build();
    assert_error(
        gctx.get::<VSOB>("a").unwrap_err(),
        str![[r#"
error in environment variable `CARGO_A`: could not load config key `a`

Caused by:
  invalid type: string "x y", expected a boolean or vector of strings
"#]],
    );

    // Normal env.
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("advanced-env")
        .env("CARGO_B", "d e")
        .env("CARGO_C", "f g")
        .build();
    assert_eq!(
        gctx.get::<VSOB>("b").unwrap(),
        VSOB::VecString(vec!["b".to_string(), "d".to_string(), "e".to_string()])
    );
    assert_eq!(
        gctx.get::<VSOB>("c").unwrap(),
        VSOB::VecString(vec!["c".to_string(), "f".to_string(), "g".to_string()])
    );

    // config-cli
    // This test demonstrates that ConfigValue::merge isn't very smart.
    // It would be nice if it was smarter.
    let gctx = GlobalContextBuilder::new()
        .config_arg("a = ['a']")
        .build_err();
    assert_error(
        gctx.unwrap_err(),
        str![[r#"
failed to merge key `a` between [ROOT]/.cargo/config.toml and --config cli option

Caused by:
  failed to merge config value from `--config cli option` into `[ROOT]/.cargo/config.toml`: expected boolean, but found array
"#]],
    );

    // config-cli and advanced-env
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("advanced-env")
        .config_arg("b=['clib']")
        .config_arg("c=['clic']")
        .env("CARGO_B", "env1 env2")
        .env("CARGO_C", "['e1', 'e2']")
        .build();
    assert_eq!(
        gctx.get::<VSOB>("b").unwrap(),
        VSOB::VecString(vec![
            "b".to_string(),
            "env1".to_string(),
            "env2".to_string(),
            "clib".to_string(),
        ])
    );
    assert_eq!(
        gctx.get::<VSOB>("c").unwrap(),
        VSOB::VecString(vec![
            "c".to_string(),
            "e1".to_string(),
            "e2".to_string(),
            "clic".to_string(),
        ])
    );
}

#[cargo_test]
fn config_toml_errors() {
    write_config_toml(
        "\
[profile.dev]
opt-level = 'foo'
",
    );

    let gctx = new_gctx();

    assert_error(
        gctx.get::<cargo_toml::TomlProfile>("profile.dev")
            .unwrap_err(),
        str![[r#"
error in [ROOT]/.cargo/config.toml: could not load config key `profile.dev.opt-level`

Caused by:
  must be `0`, `1`, `2`, `3`, `s` or `z`, but found the string: "foo"
"#]],
    );

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_PROFILE_DEV_OPT_LEVEL", "asdf")
        .build();

    assert_error(
        gctx.get::<cargo_toml::TomlProfile>("profile.dev")
            .unwrap_err(),
        str![[r#"
error in environment variable `CARGO_PROFILE_DEV_OPT_LEVEL`: could not load config key `profile.dev.opt-level`

Caused by:
  must be `0`, `1`, `2`, `3`, `s` or `z`, but found the string: "asdf"
"#]],
    );
}

#[cargo_test]
fn load_nested() {
    write_config_toml(
        "\
[nest.foo]
f1 = 1
f2 = 2
[nest.bar]
asdf = 3
",
    );

    let gctx = GlobalContextBuilder::new()
        .unstable_flag("advanced-env")
        .env("CARGO_NEST_foo_f2", "3")
        .env("CARGO_NESTE_foo_f1", "1")
        .env("CARGO_NESTE_foo_f2", "3")
        .env("CARGO_NESTE_bar_asdf", "3")
        .build();

    type Nested = HashMap<String, HashMap<String, u8>>;

    let n: Nested = gctx.get("nest").unwrap();
    let mut expected = HashMap::new();
    let mut foo = HashMap::new();
    foo.insert("f1".to_string(), 1);
    foo.insert("f2".to_string(), 3);
    expected.insert("foo".to_string(), foo);
    let mut bar = HashMap::new();
    bar.insert("asdf".to_string(), 3);
    expected.insert("bar".to_string(), bar);
    assert_eq!(n, expected);

    let n: Nested = gctx.get("neste").unwrap();
    assert_eq!(n, expected);
}

#[cargo_test]
fn get_errors() {
    write_config_toml(
        "\
[S]
f1 = 123
f2 = 'asdf'
big = 123456789
",
    );

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_E_S", "asdf")
        .env("CARGO_E_BIG", "123456789")
        .build();
    assert_error(
        gctx.get::<i64>("foo").unwrap_err(),
        str!["missing config key `foo`"],
    );
    assert_error(
        gctx.get::<i64>("foo.bar").unwrap_err(),
        str!["missing config key `foo.bar`"],
    );
    assert_error(
        gctx.get::<i64>("S.f2").unwrap_err(),
        str!["error in [ROOT]/.cargo/config.toml: `S.f2` expected an integer, but found a string"],
    );
    assert_error(
        gctx.get::<u8>("S.big").unwrap_err(),
        str![[r#"
error in [ROOT]/.cargo/config.toml: could not load config key `S.big`

Caused by:
  invalid value: integer `123456789`, expected u8
"#]],
    );

    // Environment variable type errors.
    assert_error(
        gctx.get::<i64>("e.s").unwrap_err(),
        str!["error in environment variable `CARGO_E_S`: invalid digit found in string"],
    );
    assert_error(
        gctx.get::<i8>("e.big").unwrap_err(),
        str![[r#"
error in environment variable `CARGO_E_BIG`: could not load config key `e.big`

Caused by:
  invalid value: integer `123456789`, expected i8
"#]],
    );

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct S {
        f1: i64,
        f2: String,
        f3: i64,
        big: i64,
    }
    assert_error(gctx.get::<S>("S").unwrap_err(), str!["missing field `f3`"]);
}

#[cargo_test]
fn config_get_option() {
    write_config_toml(
        "\
[foo]
f1 = 1
",
    );

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_BAR_ASDF", "3")
        .build();

    assert_eq!(gctx.get::<Option<i32>>("a").unwrap(), None);
    assert_eq!(gctx.get::<Option<i32>>("a.b").unwrap(), None);
    assert_eq!(gctx.get::<Option<i32>>("foo.f1").unwrap(), Some(1));
    assert_eq!(gctx.get::<Option<i32>>("bar.asdf").unwrap(), Some(3));
    assert_eq!(gctx.get::<Option<i32>>("bar.zzzz").unwrap(), None);
}

#[cargo_test]
fn config_bad_toml() {
    write_config_toml("asdf");
    let gctx = new_gctx();
    assert_error(
        gctx.get::<i32>("foo").unwrap_err(),
        str![[r#"
could not load Cargo configuration

Caused by:
  could not parse TOML configuration in `[ROOT]/.cargo/config.toml`

Caused by:
  TOML parse error at line 1, column 5
  |
1 | asdf
  |     ^
key with no value, expected `=`

"#]],
    );
}

#[cargo_test]
fn config_get_list() {
    write_config_toml(
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

    let gctx = GlobalContextBuilder::new()
        .unstable_flag("advanced-env")
        .env("CARGO_L4", "['three', 'four']")
        .env("CARGO_L5", "['a']")
        .env("CARGO_ENV_EMPTY", "[]")
        .env("CARGO_ENV_BLANK", "")
        .env("CARGO_ENV_NUM", "1")
        .env("CARGO_ENV_NUM_LIST", "[1]")
        .env("CARGO_ENV_TEXT", "asdf")
        .env("CARGO_LEPAIR", "['a', 'b']")
        .env("CARGO_NESTED2_L", "['z']")
        .env("CARGO_NESTEDE_L", "['env']")
        .env("CARGO_BAD_ENV", "[zzz]")
        .build();

    assert_eq!(gctx.get::<L>("unset").unwrap(), vec![] as Vec<String>);
    assert_eq!(gctx.get::<L>("l1").unwrap(), vec![] as Vec<String>);
    assert_eq!(gctx.get::<L>("l2").unwrap(), vec!["one", "two"]);
    assert_error(
        gctx.get::<L>("l3").unwrap_err(),
        str![[r#"
invalid configuration for key `l3`
expected a list, but found a integer for `l3` in [ROOT]/.cargo/config.toml
"#]],
    );
    assert_eq!(
        gctx.get::<L>("l4").unwrap(),
        vec!["one", "two", "three", "four"]
    );
    assert_eq!(gctx.get::<L>("l5").unwrap(), vec!["a"]);
    assert_eq!(gctx.get::<L>("env-empty").unwrap(), vec![] as Vec<String>);
    assert_eq!(gctx.get::<L>("env-blank").unwrap(), vec![] as Vec<String>);
    assert_eq!(gctx.get::<L>("env-num").unwrap(), vec!["1".to_string()]);
    assert_error(
        gctx.get::<L>("env-num-list").unwrap_err(),
        str!["error in environment variable `CARGO_ENV_NUM_LIST`: expected string, found integer"],
    );
    assert_eq!(gctx.get::<L>("env-text").unwrap(), vec!["asdf".to_string()]);
    // "invalid number" here isn't the best error, but I think it's just toml.rs.
    assert_error(
        gctx.get::<L>("bad-env").unwrap_err(),
        str![[r#"
error in environment variable `CARGO_BAD_ENV`: could not parse TOML list: TOML parse error at line 1, column 2
  |
1 | [zzz]
  |  ^^^
string values must be quoted, expected literal string

"#]],
    );

    // Try some other sequence-like types.
    assert_eq!(
        gctx.get::<(String, String, String, String)>("l4").unwrap(),
        (
            "one".to_string(),
            "two".to_string(),
            "three".to_string(),
            "four".to_string()
        )
    );
    assert_eq!(gctx.get::<(String,)>("l5").unwrap(), ("a".to_string(),));

    // Tuple struct
    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct TupS(String, String);
    assert_eq!(
        gctx.get::<TupS>("lepair").unwrap(),
        TupS("a".to_string(), "b".to_string())
    );

    // Nested with an option.
    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct S {
        l: Option<Vec<String>>,
    }
    assert_eq!(gctx.get::<S>("nested-empty").unwrap(), S { l: None });
    assert_eq!(
        gctx.get::<S>("nested").unwrap(),
        S {
            l: Some(vec!["x".to_string()]),
        }
    );
    assert_eq!(
        gctx.get::<S>("nested2").unwrap(),
        S {
            l: Some(vec!["y".to_string(), "z".to_string()]),
        }
    );
    assert_eq!(
        gctx.get::<S>("nestede").unwrap(),
        S {
            l: Some(vec!["env".to_string()]),
        }
    );
}

#[cargo_test]
fn config_get_other_types() {
    write_config_toml(
        "\
ns = 123
ns2 = 456
",
    );

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_NSE", "987")
        .env("CARGO_NS2", "654")
        .build();

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    #[serde(transparent)]
    struct NewS(i32);
    assert_eq!(gctx.get::<NewS>("ns").unwrap(), NewS(123));
    assert_eq!(gctx.get::<NewS>("ns2").unwrap(), NewS(654));
    assert_eq!(gctx.get::<NewS>("nse").unwrap(), NewS(987));
    assert_error(
        gctx.get::<NewS>("unset").unwrap_err(),
        str!["missing config key `unset`"],
    );
}

#[cargo_test]
fn config_relative_path() {
    write_config_toml(&format!(
        "\
p1 = 'foo/bar'
p2 = '../abc'
p3 = 'b/c'
abs = '{}'
",
        paths::home().display(),
    ));

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_EPATH", "a/b")
        .env("CARGO_P3", "d/e")
        .build();

    assert_eq!(
        gctx.get::<context::ConfigRelativePath>("p1")
            .unwrap()
            .resolve_path(&gctx),
        paths::root().join("foo/bar")
    );
    assert_eq!(
        gctx.get::<context::ConfigRelativePath>("p2")
            .unwrap()
            .resolve_path(&gctx),
        paths::root().join("../abc")
    );
    assert_eq!(
        gctx.get::<context::ConfigRelativePath>("p3")
            .unwrap()
            .resolve_path(&gctx),
        paths::root().join("d/e")
    );
    assert_eq!(
        gctx.get::<context::ConfigRelativePath>("abs")
            .unwrap()
            .resolve_path(&gctx),
        paths::home()
    );
    assert_eq!(
        gctx.get::<context::ConfigRelativePath>("epath")
            .unwrap()
            .resolve_path(&gctx),
        paths::root().join("a/b")
    );
}

#[cargo_test]
fn config_get_integers() {
    write_config_toml(
        "\
npos = 123456789
nneg = -123456789
i64max = 9223372036854775807
",
    );

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_EPOS", "123456789")
        .env("CARGO_ENEG", "-1")
        .env("CARGO_EI64MAX", "9223372036854775807")
        .build();

    assert_eq!(
        gctx.get::<u64>("i64max").unwrap(),
        9_223_372_036_854_775_807
    );
    assert_eq!(
        gctx.get::<i64>("i64max").unwrap(),
        9_223_372_036_854_775_807
    );
    assert_eq!(
        gctx.get::<u64>("ei64max").unwrap(),
        9_223_372_036_854_775_807
    );
    assert_eq!(
        gctx.get::<i64>("ei64max").unwrap(),
        9_223_372_036_854_775_807
    );

    assert_error(
        gctx.get::<u32>("nneg").unwrap_err(),
        str![[r#"
error in [ROOT]/.cargo/config.toml: could not load config key `nneg`

Caused by:
  invalid value: integer `-123456789`, expected u32
"#]],
    );
    assert_error(
        gctx.get::<u32>("eneg").unwrap_err(),
        str![[r#"
error in environment variable `CARGO_ENEG`: could not load config key `eneg`

Caused by:
  invalid value: integer `-1`, expected u32
"#]],
    );
    assert_error(
        gctx.get::<i8>("npos").unwrap_err(),
        str![[r#"
error in [ROOT]/.cargo/config.toml: could not load config key `npos`

Caused by:
  invalid value: integer `123456789`, expected i8
"#]],
    );
    assert_error(
        gctx.get::<i8>("epos").unwrap_err(),
        str![[r#"
error in environment variable `CARGO_EPOS`: could not load config key `epos`

Caused by:
  invalid value: integer `123456789`, expected i8
"#]],
    );
}

#[cargo_test]
fn config_get_ssl_version_missing() {
    write_config_toml(
        "\
[http]
hello = 'world'
",
    );

    let gctx = new_gctx();

    assert!(
        gctx.get::<Option<SslVersionConfig>>("http.ssl-version")
            .unwrap()
            .is_none()
    );
}

#[cargo_test]
fn config_get_ssl_version_single() {
    write_config_toml(
        "\
[http]
ssl-version = 'tlsv1.2'
",
    );

    let gctx = new_gctx();

    let a = gctx
        .get::<Option<SslVersionConfig>>("http.ssl-version")
        .unwrap()
        .unwrap();
    match a {
        SslVersionConfig::Single(v) => assert_eq!(&v, "tlsv1.2"),
        SslVersionConfig::Range(_) => panic!("Did not expect ssl version min/max."),
    };
}

#[cargo_test]
fn config_get_ssl_version_min_max() {
    write_config_toml(
        "\
[http]
ssl-version.min = 'tlsv1.2'
ssl-version.max = 'tlsv1.3'
",
    );

    let gctx = new_gctx();

    let a = gctx
        .get::<Option<SslVersionConfig>>("http.ssl-version")
        .unwrap()
        .unwrap();
    match a {
        SslVersionConfig::Single(_) => panic!("Did not expect exact ssl version."),
        SslVersionConfig::Range(range) => {
            assert_eq!(range.min, Some(String::from("tlsv1.2")));
            assert_eq!(range.max, Some(String::from("tlsv1.3")));
        }
    };
}

#[cargo_test]
fn config_get_ssl_version_both_forms_configured() {
    // this is not allowed
    write_config_toml(
        "\
[http]
ssl-version = 'tlsv1.1'
ssl-version.min = 'tlsv1.2'
ssl-version.max = 'tlsv1.3'
",
    );

    let gctx = new_gctx();

    assert_error(
        gctx.get::<SslVersionConfig>("http.ssl-version")
            .unwrap_err(),
        str![[r#"
could not load Cargo configuration

Caused by:
  could not parse TOML configuration in `[ROOT]/.cargo/config.toml`

Caused by:
  TOML parse error at line 3, column 1
  |
3 | ssl-version.min = 'tlsv1.2'
  | ^^^^^^^^^^^
cannot extend value of type string with a dotted key

"#]],
    );
}

#[cargo_test]
/// Assert that unstable options can be configured with the `unstable` table in
/// cargo config files
fn unstable_table_notation() {
    write_config_toml(
        "\
[unstable]
print-im-a-teapot = true
",
    );
    let gctx = GlobalContextBuilder::new()
        .nightly_features_allowed(true)
        .build();
    assert_eq!(gctx.cli_unstable().print_im_a_teapot, true);
}

#[cargo_test]
/// Assert that dotted notation works for configuring unstable options
fn unstable_dotted_notation() {
    write_config_toml(
        "\
unstable.print-im-a-teapot = true
",
    );
    let gctx = GlobalContextBuilder::new()
        .nightly_features_allowed(true)
        .build();
    assert_eq!(gctx.cli_unstable().print_im_a_teapot, true);
}

#[cargo_test]
/// Assert that Zflags on the CLI take precedence over those from config
fn unstable_cli_precedence() {
    write_config_toml(
        "\
unstable.print-im-a-teapot = true
",
    );
    let gctx = GlobalContextBuilder::new()
        .nightly_features_allowed(true)
        .build();
    assert_eq!(gctx.cli_unstable().print_im_a_teapot, true);

    let gctx = GlobalContextBuilder::new()
        .unstable_flag("print-im-a-teapot=no")
        .build();
    assert_eq!(gctx.cli_unstable().print_im_a_teapot, false);
}

#[cargo_test]
/// Assert that attempting to set an unstable flag that doesn't exist via config
/// is ignored on stable
fn unstable_invalid_flag_ignored_on_stable() {
    write_config_toml(
        "\
unstable.an-invalid-flag = 'yes'
",
    );
    assert!(GlobalContextBuilder::new().build_err().is_ok());
}

#[cargo_test]
/// Assert that unstable options can be configured with the `unstable` table in
/// cargo config files
fn unstable_flags_ignored_on_stable() {
    write_config_toml(
        "\
[unstable]
print-im-a-teapot = true
",
    );
    // Enforce stable channel even when testing on nightly.
    let gctx = GlobalContextBuilder::new()
        .nightly_features_allowed(false)
        .build();
    assert_eq!(gctx.cli_unstable().print_im_a_teapot, false);
}

#[cargo_test]
fn table_merge_failure() {
    // Config::merge fails to merge entries in two tables.
    write_config_at(
        "foo/.cargo/config.toml",
        "
        [table]
        key = ['foo']
        ",
    );
    write_config_at(
        ".cargo/config.toml",
        "
        [table]
        key = 'bar'
        ",
    );

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Table {
        key: StringList,
    }
    let gctx = GlobalContextBuilder::new().cwd("foo").build();
    assert_error(
        gctx.get::<Table>("table").unwrap_err(),
        str![[r#"
could not load Cargo configuration

Caused by:
  failed to merge configuration at `[ROOT]/.cargo/config.toml`

Caused by:
  failed to merge key `table` between [ROOT]/foo/.cargo/config.toml and [ROOT]/.cargo/config.toml

Caused by:
  failed to merge key `key` between [ROOT]/foo/.cargo/config.toml and [ROOT]/.cargo/config.toml

Caused by:
  failed to merge config value from `[ROOT]/.cargo/config.toml` into `[ROOT]/foo/.cargo/config.toml`: expected array, but found string
"#]],
    );
}

#[cargo_test]
fn struct_with_opt_inner_struct() {
    // Struct with a key that is Option of another struct.
    // Check that can be defined with environment variable.
    #[derive(Deserialize)]
    struct Inner {
        value: Option<i32>,
    }
    #[derive(Deserialize)]
    struct Foo {
        inner: Option<Inner>,
    }
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_FOO_INNER_VALUE", "12")
        .build();
    let f: Foo = gctx.get("foo").unwrap();
    assert_eq!(f.inner.unwrap().value.unwrap(), 12);
}

#[cargo_test]
fn struct_with_default_inner_struct() {
    // Struct with serde defaults.
    // Check that can be defined with environment variable.
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Inner {
        value: i32,
    }
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Foo {
        inner: Inner,
    }
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_FOO_INNER_VALUE", "12")
        .build();
    let f: Foo = gctx.get("foo").unwrap();
    assert_eq!(f.inner.value, 12);
}

#[cargo_test]
fn overlapping_env_config() {
    // Issue where one key is a prefix of another.
    #[derive(Deserialize)]
    #[serde(rename_all = "kebab-case")]
    struct Ambig {
        debug: Option<u32>,
        debug_assertions: Option<bool>,
    }
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_AMBIG_DEBUG_ASSERTIONS", "true")
        .build();

    let s: Ambig = gctx.get("ambig").unwrap();
    assert_eq!(s.debug_assertions, Some(true));
    assert_eq!(s.debug, None);

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_AMBIG_DEBUG", "0")
        .build();
    let s: Ambig = gctx.get("ambig").unwrap();
    assert_eq!(s.debug_assertions, None);
    assert_eq!(s.debug, Some(0));

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_AMBIG_DEBUG", "1")
        .env("CARGO_AMBIG_DEBUG_ASSERTIONS", "true")
        .build();
    let s: Ambig = gctx.get("ambig").unwrap();
    assert_eq!(s.debug_assertions, Some(true));
    assert_eq!(s.debug, Some(1));
}

#[cargo_test]
fn overlapping_env_with_defaults_errors_out() {
    // Issue where one key is a prefix of another.
    // This is a limitation of mapping environment variables on to a hierarchy.
    // Check that we error out when we hit ambiguity in this way, rather than
    // the more-surprising defaulting through.
    // If, in the future, we can handle this more correctly, feel free to delete
    // this test.
    #[derive(Deserialize, Default)]
    #[serde(default, rename_all = "kebab-case")]
    struct Ambig {
        debug: u32,
        debug_assertions: bool,
    }
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_AMBIG_DEBUG_ASSERTIONS", "true")
        .build();
    let err = gctx.get::<Ambig>("ambig").err().unwrap();
    assert!(format!("{}", err).contains("missing config key `ambig.debug`"));

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_AMBIG_DEBUG", "5")
        .build();
    let s: Ambig = gctx.get("ambig").unwrap();
    assert_eq!(s.debug_assertions, bool::default());
    assert_eq!(s.debug, 5);

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_AMBIG_DEBUG", "1")
        .env("CARGO_AMBIG_DEBUG_ASSERTIONS", "true")
        .build();
    let s: Ambig = gctx.get("ambig").unwrap();
    assert_eq!(s.debug_assertions, true);
    assert_eq!(s.debug, 1);
}

#[cargo_test]
fn struct_with_overlapping_inner_struct_and_defaults() {
    // Struct with serde defaults.
    // Check that can be defined with environment variable.
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Inner {
        value: i32,
    }

    // Containing struct with a prefix of inner
    //
    // This is a limitation of mapping environment variables on to a hierarchy.
    // Check that we error out when we hit ambiguity in this way, rather than
    // the more-surprising defaulting through.
    // If, in the future, we can handle this more correctly, feel free to delete
    // this case.
    #[derive(Deserialize, Default)]
    struct PrefixContainer {
        inn: bool,
        inner: Inner,
    }
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_PREFIXCONTAINER_INNER_VALUE", "12")
        .build();
    let err = gctx
        .get::<PrefixContainer>("prefixcontainer")
        .err()
        .unwrap();
    assert!(format!("{}", err).contains("missing field `inn`"));
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_PREFIXCONTAINER_INNER_VALUE", "12")
        .env("CARGO_PREFIXCONTAINER_INN", "true")
        .build();
    let f: PrefixContainer = gctx.get("prefixcontainer").unwrap();
    assert_eq!(f.inner.value, 12);
    assert_eq!(f.inn, true);

    // Use default attribute of serde, then we can skip setting the inn field
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct PrefixContainerFieldDefault {
        inn: bool,
        inner: Inner,
    }
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_PREFIXCONTAINER_INNER_VALUE", "12")
        .build();
    let f = gctx
        .get::<PrefixContainerFieldDefault>("prefixcontainer")
        .unwrap();
    assert_eq!(f.inner.value, 12);
    assert_eq!(f.inn, false);

    // Containing struct where the inner value's field is a prefix of another
    //
    // This is a limitation of mapping environment variables on to a hierarchy.
    // Check that we error out when we hit ambiguity in this way, rather than
    // the more-surprising defaulting through.
    // If, in the future, we can handle this more correctly, feel free to delete
    // this case.
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct InversePrefixContainer {
        inner_field: bool,
        inner: Inner,
    }
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_INVERSEPREFIXCONTAINER_INNER_VALUE", "12")
        .build();
    let f: InversePrefixContainer = gctx.get("inverseprefixcontainer").unwrap();
    assert_eq!(f.inner_field, bool::default());
    assert_eq!(f.inner.value, 12);
}

#[cargo_test]
fn string_list_tricky_env() {
    // Make sure StringList handles typed env values.
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_KEY1", "123")
        .env("CARGO_KEY2", "true")
        .env("CARGO_KEY3", "1 2")
        .build();
    let x = gctx.get::<StringList>("key1").unwrap();
    assert_eq!(x.as_slice(), &["123".to_string()]);
    let x = gctx.get::<StringList>("key2").unwrap();
    assert_eq!(x.as_slice(), &["true".to_string()]);
    let x = gctx.get::<StringList>("key3").unwrap();
    assert_eq!(x.as_slice(), &["1".to_string(), "2".to_string()]);
}

#[cargo_test]
fn string_list_wrong_type() {
    // What happens if StringList is given then wrong type.
    write_config_toml("some_list = 123");
    let gctx = GlobalContextBuilder::new().build();
    assert_error(
        gctx.get::<StringList>("some_list").unwrap_err(),
        str![[r#"
invalid configuration for key `some_list`
expected a string or array of strings, but found a integer for `some_list` in [ROOT]/.cargo/config.toml
"#]],
    );

    write_config_toml("some_list = \"1 2\"");
    let gctx = GlobalContextBuilder::new().build();
    let x = gctx.get::<StringList>("some_list").unwrap();
    assert_eq!(x.as_slice(), &["1".to_string(), "2".to_string()]);
}

#[cargo_test]
fn string_list_advanced_env() {
    // StringList with advanced env.
    let gctx = GlobalContextBuilder::new()
        .unstable_flag("advanced-env")
        .env("CARGO_KEY1", "[]")
        .env("CARGO_KEY2", "['1 2', '3']")
        .env("CARGO_KEY3", "[123]")
        .build();
    let x = gctx.get::<StringList>("key1").unwrap();
    assert_eq!(x.as_slice(), &[] as &[String]);
    let x = gctx.get::<StringList>("key2").unwrap();
    assert_eq!(x.as_slice(), &["1 2".to_string(), "3".to_string()]);
    assert_error(
        gctx.get::<StringList>("key3").unwrap_err(),
        str!["error in environment variable `CARGO_KEY3`: expected string, found integer"],
    );
}

#[cargo_test]
fn parse_strip_with_string() {
    write_config_toml(
        "\
[profile.release]
strip = 'debuginfo'
",
    );

    let gctx = new_gctx();

    let p: cargo_toml::TomlProfile = gctx.get("profile.release").unwrap();
    let strip = p.strip.unwrap();
    assert_eq!(
        strip,
        cargo_toml::StringOrBool::String("debuginfo".to_string())
    );
}

#[cargo_test]
fn cargo_target_empty_cfg() {
    write_config_toml(
        "\
[build]
target-dir = ''
",
    );

    let gctx = new_gctx();

    assert_error(
        gctx.target_dir().unwrap_err(),
        str!["the target directory is set to an empty string in [ROOT]/.cargo/config.toml"],
    );
}

#[cargo_test]
fn cargo_target_empty_env() {
    let project = project()
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
        .build();

    project.cargo("check")
        .env("CARGO_TARGET_DIR", "")
        .with_stderr_data(str![[r#"
[ERROR] the target directory is set to an empty string in the `CARGO_TARGET_DIR` environment variable

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn all_profile_options() {
    // Check that all profile options can be serialized/deserialized.
    let base_settings = cargo_toml::TomlProfile {
        opt_level: Some(cargo_toml::TomlOptLevel("0".to_string())),
        lto: Some(cargo_toml::StringOrBool::String("thin".to_string())),
        codegen_backend: Some(String::from("example")),
        codegen_units: Some(123),
        debug: Some(cargo_toml::TomlDebugInfo::Limited),
        split_debuginfo: Some("packed".to_string()),
        debug_assertions: Some(true),
        rpath: Some(true),
        panic: Some("abort".to_string()),
        overflow_checks: Some(true),
        incremental: Some(true),
        dir_name: Some(String::from("dir_name")),
        inherits: Some(String::from("debug")),
        strip: Some(cargo_toml::StringOrBool::String("symbols".to_string())),
        package: None,
        build_override: None,
        rustflags: None,
        trim_paths: None,
        hint_mostly_unused: None,
    };
    let mut overrides = BTreeMap::new();
    let key = cargo_toml::ProfilePackageSpec::Spec(PackageIdSpec::parse("foo").unwrap());
    overrides.insert(key, base_settings.clone());
    let profile = cargo_toml::TomlProfile {
        build_override: Some(Box::new(base_settings.clone())),
        package: Some(overrides),
        ..base_settings
    };
    let profile_toml = toml::to_string(&profile).unwrap();
    let roundtrip: cargo_toml::TomlProfile = toml::from_str(&profile_toml).unwrap();
    let roundtrip_toml = toml::to_string(&roundtrip).unwrap();
    assert_e2e().eq(&roundtrip_toml, &profile_toml);
}

#[cargo_test]
fn value_in_array() {
    // Value<String> in an array should work
    let root_path = paths::root().join(".cargo/config.toml");
    write_config_at(
        &root_path,
        "\
[net.ssh]
known-hosts = [
    \"example.com ...\",
    \"example.net ...\",
]
",
    );

    let foo_path = paths::root().join("foo/.cargo/config.toml");
    write_config_at(
        &foo_path,
        "\
[net.ssh]
known-hosts = [
    \"example.org ...\",
]
",
    );

    let gctx = GlobalContextBuilder::new()
        .cwd("foo")
        // environment variables don't actually work for known-hosts due to
        // space splitting, but this is included here just to validate that
        // they work (particularly if other Vec<Value> config vars are added
        // in the future).
        .env("CARGO_NET_SSH_KNOWN_HOSTS", "env-example")
        .build();
    let net_config = gctx.net_config().unwrap();
    let kh = net_config
        .ssh
        .as_ref()
        .unwrap()
        .known_hosts
        .as_ref()
        .unwrap();
    assert_eq!(kh.len(), 4);
    assert_eq!(kh[0].val, "example.com ...");
    assert_eq!(kh[0].definition, Definition::Path(root_path.clone()));
    assert_eq!(kh[1].val, "example.net ...");
    assert_eq!(kh[1].definition, Definition::Path(root_path.clone()));
    assert_eq!(kh[2].val, "example.org ...");
    assert_eq!(kh[2].definition, Definition::Path(foo_path.clone()));
    assert_eq!(kh[3].val, "env-example");
    assert_eq!(
        kh[3].definition,
        Definition::Environment("CARGO_NET_SSH_KNOWN_HOSTS".to_string())
    );
}

#[cargo_test]
fn debuginfo_parsing() {
    let gctx = GlobalContextBuilder::new().build();
    let p: cargo_toml::TomlProfile = gctx.get("profile.dev").unwrap();
    assert_eq!(p.debug, None);

    let env_test_cases = [
        (TomlDebugInfo::None, ["false", "0", "none"].as_slice()),
        (TomlDebugInfo::LineDirectivesOnly, &["line-directives-only"]),
        (TomlDebugInfo::LineTablesOnly, &["line-tables-only"]),
        (TomlDebugInfo::Limited, &["1", "limited"]),
        (TomlDebugInfo::Full, &["true", "2", "full"]),
    ];
    for (expected, config_strs) in env_test_cases {
        for &val in config_strs {
            let gctx = GlobalContextBuilder::new()
                .env("CARGO_PROFILE_DEV_DEBUG", val)
                .build();
            let debug: TomlDebugInfo = gctx.get("profile.dev.debug").unwrap();
            assert_eq!(debug, expected, "failed to parse {val}");
        }
    }

    let toml_test_cases = [
        (TomlDebugInfo::None, ["false", "0", "\"none\""].as_slice()),
        (
            TomlDebugInfo::LineDirectivesOnly,
            &["\"line-directives-only\""],
        ),
        (TomlDebugInfo::LineTablesOnly, &["\"line-tables-only\""]),
        (TomlDebugInfo::Limited, &["1", "\"limited\""]),
        (TomlDebugInfo::Full, &["true", "2", "\"full\""]),
    ];
    for (expected, config_strs) in toml_test_cases {
        for &val in config_strs {
            let gctx = GlobalContextBuilder::new()
                .config_arg(format!("profile.dev.debug={val}"))
                .build();
            let debug: TomlDebugInfo = gctx.get("profile.dev.debug").unwrap();
            assert_eq!(debug, expected, "failed to parse {val}");
        }
    }

    let toml_err_cases = ["\"\"", "\"unrecognized\"", "3"];
    for err_val in toml_err_cases {
        let gctx = GlobalContextBuilder::new()
            .config_arg(format!("profile.dev.debug={err_val}"))
            .build();
        let err = gctx.get::<TomlDebugInfo>("profile.dev.debug").unwrap_err();
        assert!(
            err.to_string()
                .ends_with("could not load config key `profile.dev.debug`")
        );
    }
}

#[cargo_test]
fn build_jobs_missing() {
    write_config_toml(
        "\
[build]
",
    );

    let gctx = new_gctx();

    assert!(
        gctx.get::<Option<JobsConfig>>("build.jobs")
            .unwrap()
            .is_none()
    );
}

#[cargo_test]
fn build_jobs_default() {
    write_config_toml(
        "\
[build]
jobs = \"default\"
",
    );

    let gctx = new_gctx();

    let a = gctx
        .get::<Option<JobsConfig>>("build.jobs")
        .unwrap()
        .unwrap();

    match a {
        JobsConfig::String(v) => assert_eq!(&v, "default"),
        JobsConfig::Integer(_) => panic!("Did not except an integer."),
    }
}

#[cargo_test]
fn build_jobs_integer() {
    write_config_toml(
        "\
[build]
jobs = 2
",
    );

    let gctx = new_gctx();

    let a = gctx
        .get::<Option<JobsConfig>>("build.jobs")
        .unwrap()
        .unwrap();

    match a {
        JobsConfig::String(_) => panic!("Did not except an integer."),
        JobsConfig::Integer(v) => assert_eq!(v, 2),
    }
}

#[cargo_test]
fn trim_paths_parsing() {
    let gctx = GlobalContextBuilder::new().build();
    let p: cargo_toml::TomlProfile = gctx.get("profile.dev").unwrap();
    assert_eq!(p.trim_paths, None);

    let test_cases = [
        (TomlTrimPathsValue::Diagnostics.into(), "diagnostics"),
        (TomlTrimPathsValue::Macro.into(), "macro"),
        (TomlTrimPathsValue::Object.into(), "object"),
    ];
    for (expected, val) in test_cases {
        // env
        let gctx = GlobalContextBuilder::new()
            .env("CARGO_PROFILE_DEV_TRIM_PATHS", val)
            .build();
        let trim_paths: TomlTrimPaths = gctx.get("profile.dev.trim-paths").unwrap();
        assert_eq!(trim_paths, expected, "failed to parse {val}");

        // config.toml
        let gctx = GlobalContextBuilder::new()
            .config_arg(format!("profile.dev.trim-paths='{val}'"))
            .build();
        let trim_paths: TomlTrimPaths = gctx.get("profile.dev.trim-paths").unwrap();
        assert_eq!(trim_paths, expected, "failed to parse {val}");
    }

    let test_cases = [(TomlTrimPaths::none(), false), (TomlTrimPaths::All, true)];

    for (expected, val) in test_cases {
        // env
        let gctx = GlobalContextBuilder::new()
            .env("CARGO_PROFILE_DEV_TRIM_PATHS", format!("{val}"))
            .build();
        let trim_paths: TomlTrimPaths = gctx.get("profile.dev.trim-paths").unwrap();
        assert_eq!(trim_paths, expected, "failed to parse {val}");

        // config.toml
        let gctx = GlobalContextBuilder::new()
            .config_arg(format!("profile.dev.trim-paths={val}"))
            .build();
        let trim_paths: TomlTrimPaths = gctx.get("profile.dev.trim-paths").unwrap();
        assert_eq!(trim_paths, expected, "failed to parse {val}");
    }

    let expected = vec![
        TomlTrimPathsValue::Diagnostics,
        TomlTrimPathsValue::Macro,
        TomlTrimPathsValue::Object,
    ]
    .into();
    let val = r#"["diagnostics", "macro", "object"]"#;
    // config.toml
    let gctx = GlobalContextBuilder::new()
        .config_arg(format!("profile.dev.trim-paths={val}"))
        .build();
    let trim_paths: TomlTrimPaths = gctx.get("profile.dev.trim-paths").unwrap();
    assert_eq!(trim_paths, expected, "failed to parse {val}");
}

#[cargo_test]
fn missing_fields() {
    #[derive(Deserialize, Default, Debug)]
    struct Foo {
        bar: Bar,
    }

    #[derive(Deserialize, Default, Debug)]
    struct Bar {
        bax: bool,
        baz: bool,
    }

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_FOO_BAR_BAZ", "true")
        .build();
    assert_error(
        gctx.get::<Foo>("foo").unwrap_err(),
        str![[r#"
could not load config key `foo.bar`

Caused by:
  missing field `bax`
"#]],
    );
    let gctx: GlobalContext = GlobalContextBuilder::new()
        .env("CARGO_FOO_BAR_BAZ", "true")
        .env("CARGO_FOO_BAR_BAX", "true")
        .build();
    let foo = gctx.get::<Foo>("foo").unwrap();
    assert_eq!(foo.bar.bax, true);
    assert_eq!(foo.bar.baz, true);

    let gctx: GlobalContext = GlobalContextBuilder::new()
        .config_arg("foo.bar.baz=true")
        .build();
    assert_error(
        gctx.get::<Foo>("foo").unwrap_err(),
        str![[r#"
error in --config cli option: could not load config key `foo.bar`

Caused by:
  missing field `bax`
"#]],
    );
}

#[cargo_test]
fn git_features() {
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_UNSTABLE_GIT", "shallow-index")
        .build();
    assert!(do_check(
        gctx,
        Some(GitFeatures {
            shallow_index: true,
            ..GitFeatures::default()
        }),
    ));

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_UNSTABLE_GIT", "shallow-index,abc")
        .build();
    assert_error(
        gctx.get::<Option<cargo::core::CliUnstable>>("unstable")
            .unwrap_err(),
        str![[r#"
error in environment variable `CARGO_UNSTABLE_GIT`: could not load config key `unstable.git`

Caused by:
  unstable 'git' only takes `shallow-index` and `shallow-deps` as valid inputs
"#]],
    );

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_UNSTABLE_GIT", "shallow-deps")
        .build();
    assert!(do_check(
        gctx,
        Some(GitFeatures {
            shallow_index: false,
            shallow_deps: true,
        }),
    ));

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_UNSTABLE_GIT", "true")
        .build();
    assert!(do_check(gctx, Some(GitFeatures::all())));

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_UNSTABLE_GIT_SHALLOW_INDEX", "true")
        .build();
    assert!(do_check(
        gctx,
        Some(GitFeatures {
            shallow_index: true,
            ..Default::default()
        }),
    ));

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_UNSTABLE_GIT_SHALLOW_INDEX", "true")
        .env("CARGO_UNSTABLE_GIT_SHALLOW_DEPS", "true")
        .build();
    assert!(do_check(
        gctx,
        Some(GitFeatures {
            shallow_index: true,
            shallow_deps: true,
            ..Default::default()
        }),
    ));

    write_config_toml(
        "\
[unstable]
git = 'shallow-index'
",
    );
    let gctx = GlobalContextBuilder::new().build();
    assert!(do_check(
        gctx,
        Some(GitFeatures {
            shallow_index: true,
            shallow_deps: false,
        }),
    ));

    write_config_toml(
        "\
    [unstable.git]
    shallow_deps = false
    shallow_index = true
    ",
    );
    let gctx = GlobalContextBuilder::new().build();
    assert!(do_check(
        gctx,
        Some(GitFeatures {
            shallow_index: true,
            shallow_deps: false,
            ..Default::default()
        }),
    ));

    write_config_toml(
        "\
    [unstable.git]
    ",
    );
    let gctx = GlobalContextBuilder::new().build();
    assert!(do_check(gctx, Some(Default::default())));

    fn do_check(gctx: GlobalContext, expect: Option<GitFeatures>) -> bool {
        let unstable_flags = gctx
            .get::<Option<cargo::core::CliUnstable>>("unstable")
            .unwrap()
            .unwrap();
        unstable_flags.git == expect
    }
}

#[cargo_test]
fn gitoxide_features() {
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_UNSTABLE_GITOXIDE", "fetch")
        .build();
    assert!(do_check(
        gctx,
        Some(GitoxideFeatures {
            fetch: true,
            ..GitoxideFeatures::default()
        }),
    ));

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_UNSTABLE_GITOXIDE", "fetch,abc")
        .build();

    assert_error(
        gctx.get::<Option<cargo::core::CliUnstable>>("unstable")
            .unwrap_err(),
        str![[r#"
error in environment variable `CARGO_UNSTABLE_GITOXIDE`: could not load config key `unstable.gitoxide`

Caused by:
  unstable 'gitoxide' only takes `fetch` and `checkout` and `internal-use-git2` as valid inputs, for shallow fetches see `-Zgit=shallow-index,shallow-deps`
"#]],
    );

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_UNSTABLE_GITOXIDE", "true")
        .build();
    assert!(do_check(gctx, Some(GitoxideFeatures::all())));

    let gctx = GlobalContextBuilder::new()
        .env("CARGO_UNSTABLE_GITOXIDE_FETCH", "true")
        .build();
    assert!(do_check(
        gctx,
        Some(GitoxideFeatures {
            fetch: true,
            ..Default::default()
        }),
    ));

    write_config_toml(
        "\
[unstable]
gitoxide = \"fetch\"
",
    );
    let gctx = GlobalContextBuilder::new().build();
    assert!(do_check(
        gctx,
        Some(GitoxideFeatures {
            fetch: true,
            ..GitoxideFeatures::default()
        }),
    ));

    write_config_toml(
        "\
    [unstable.gitoxide]
    fetch = true
    checkout = false
    internal_use_git2 = false
    ",
    );
    let gctx = GlobalContextBuilder::new().build();
    assert!(do_check(
        gctx,
        Some(GitoxideFeatures {
            fetch: true,
            checkout: false,
            internal_use_git2: false,
        }),
    ));

    write_config_toml(
        "\
    [unstable.gitoxide]
    ",
    );
    let gctx = GlobalContextBuilder::new().build();
    assert!(do_check(gctx, Some(Default::default())));

    fn do_check(gctx: GlobalContext, expect: Option<GitoxideFeatures>) -> bool {
        let unstable_flags = gctx
            .get::<Option<cargo::core::CliUnstable>>("unstable")
            .unwrap()
            .unwrap();
        unstable_flags.gitoxide == expect
    }
}

#[cargo_test]
fn nonmergeable_lists() {
    let root_path = paths::root().join(".cargo/config.toml");
    write_config_at(
        &root_path,
        "\
[registries.example]
credential-provider = ['a', 'b']
",
    );

    let foo_path = paths::root().join("foo/.cargo/config.toml");
    write_config_at(
        &foo_path,
        "\
[registries.example]
credential-provider = ['c', 'd']
",
    );

    let gctx = GlobalContextBuilder::new().cwd("foo").build();
    let provider = gctx
        .get::<Option<RegistryConfig>>(&format!("registries.example"))
        .unwrap()
        .unwrap()
        .credential_provider
        .unwrap();
    assert_eq!(provider.path.raw_value(), "c");
    assert_eq!(provider.args, ["d"]);

    let cli_arg = "registries.example.credential-provider=['cli', 'cli-arg']";
    let gctx = GlobalContextBuilder::new()
        .config_arg(cli_arg)
        .cwd("foo")
        .build();
    let provider = gctx
        .get::<Option<RegistryConfig>>(&format!("registries.example"))
        .unwrap()
        .unwrap()
        .credential_provider
        .unwrap();
    // expect: no merge happens; config CLI takes precedence
    assert_eq!(provider.path.raw_value(), "cli");
    assert_eq!(provider.args, ["cli-arg"]);

    let env = "CARGO_REGISTRIES_EXAMPLE_CREDENTIAL_PROVIDER";
    let gctx = GlobalContextBuilder::new()
        .env(env, "env env-arg")
        .cwd("foo")
        .build();
    let provider = gctx
        .get::<Option<RegistryConfig>>(&format!("registries.example"))
        .unwrap()
        .unwrap()
        .credential_provider
        .unwrap();
    // expect: no merge happens; env takes precedence over files
    assert_eq!(provider.path.raw_value(), "env");
    assert_eq!(provider.args, ["env-arg"]);

    let gctx = GlobalContextBuilder::new()
        .env(env, "env env-arg")
        .config_arg(cli_arg)
        .cwd("foo")
        .build();
    let provider = gctx
        .get::<Option<RegistryConfig>>(&format!("registries.example"))
        .unwrap()
        .unwrap()
        .credential_provider
        .unwrap();
    // expect: no merge happens; cli takes precedence over files and env
    assert_eq!(provider.path.raw_value(), "cli");
    assert_eq!(provider.args, ["cli-arg"]);
}

#[cargo_test]
fn build_std() {
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_UNSTABLE_BUILD_STD", "core,std,panic_abort")
        .build();
    let value = gctx
        .get::<Option<cargo::core::CliUnstable>>("unstable")
        .unwrap()
        .unwrap()
        .build_std
        .unwrap();
    assert_eq!(
        value,
        vec![
            "core".to_string(),
            "std".to_string(),
            "panic_abort".to_string(),
        ],
    );

    let gctx = GlobalContextBuilder::new()
        .config_arg("unstable.build-std=['core', 'std,panic_abort']")
        .build();
    let value = gctx
        .get::<Option<cargo::core::CliUnstable>>("unstable")
        .unwrap()
        .unwrap()
        .build_std
        .unwrap();
    assert_eq!(
        value,
        vec![
            "core".to_string(),
            "std".to_string(),
            "panic_abort".to_string(),
        ]
    );

    let gctx = GlobalContextBuilder::new()
        .env(
            "CARGO_UNSTABLE_BUILD_STD_FEATURES",
            "backtrace,panic-unwind,windows_raw_dylib",
        )
        .build();
    let value = gctx
        .get::<Option<cargo::core::CliUnstable>>("unstable")
        .unwrap()
        .unwrap()
        .build_std_features
        .unwrap();
    assert_eq!(
        value,
        vec![
            "backtrace".to_string(),
            "panic-unwind".to_string(),
            "windows_raw_dylib".to_string(),
        ]
    );

    let gctx = GlobalContextBuilder::new()
        .config_arg("unstable.build-std-features=['backtrace', 'panic-unwind,windows_raw_dylib']")
        .build();
    let value = gctx
        .get::<Option<cargo::core::CliUnstable>>("unstable")
        .unwrap()
        .unwrap()
        .build_std_features
        .unwrap();
    assert_eq!(
        value,
        vec![
            "backtrace".to_string(),
            "panic-unwind".to_string(),
            "windows_raw_dylib".to_string(),
        ]
    );
}

#[cargo_test]
fn array_of_any_types() {
    write_config_toml(
        r#"
        ints = [1, 2, 3]

        bools = [true, false, true]

        strings = ["hello", "world", "test"]

        [[tables]]
        name = "first"
        value = 1
        [[tables]]
        name = "second"
        value = 2
        "#,
    );

    let gctx = new_gctx();

    // Test integer array
    let ints: Vec<i32> = gctx.get("ints").unwrap();
    assert_eq!(ints, vec![1, 2, 3]);

    let bools: Vec<bool> = gctx.get("bools").unwrap();
    assert_eq!(bools, vec![true, false, true]);

    #[derive(Deserialize, Debug, PartialEq)]
    struct T {
        name: String,
        value: i32,
    }
    let tables: Vec<T> = gctx.get("tables").unwrap();
    assert_eq!(
        tables,
        vec![
            T {
                name: "first".into(),
                value: 1,
            },
            T {
                name: "second".into(),
                value: 2,
            },
        ]
    );
}

#[cargo_test]
fn array_env() {
    // for environment, only strings are supported.
    let gctx = GlobalContextBuilder::new()
        .env("CARGO_INTS", "3 4 5")
        .env("CARGO_BOOLS", "false true false")
        .env("CARGO_STRINGS", "env1 env2 env3")
        .build();

    assert_error(
        gctx.get::<Vec<i32>>("ints").unwrap_err(),
        str![[r#"
error in environment variable `CARGO_INTS`: failed to parse config at `ints[0]`

Caused by:
  invalid type: string "3", expected i32
"#]],
    );

    assert_error(
        gctx.get::<Vec<bool>>("bools").unwrap_err(),
        str![[r#"
error in environment variable `CARGO_BOOLS`: failed to parse config at `bools[0]`

Caused by:
  invalid type: string "false", expected a boolean
"#]],
    );

    assert_eq!(
        gctx.get::<Vec<String>>("strings").unwrap(),
        vec!["env1".to_string(), "env2".to_string(), "env3".to_string()],
    );
}

#[cargo_test]
fn nested_array() {
    let root_path = paths::root().join(".cargo/config.toml");
    write_config_at(
        &root_path,
        r#"
        nested_ints = [[1, 2], [3, 4]]
        nested_bools = [[true], [false, true]]
        nested_strings = [["a", "b"], ["3", "4"]]
        nested_tables = [
            [
                { x = "a" },
                { x = "b" },
            ],
            [
                { x = "c" },
                { x = "d" },
            ],
        ]
        deeply_nested = [[
            { x = [[[ { x = [], y = 2  } ]]], y = 1 },
        ]]
        "#,
    );

    let gctx = GlobalContextBuilder::new()
        .config_arg("nested_ints = [[5]]")
        .build();

    let nested = gctx.get::<Vec<Vec<i32>>>("nested_ints").unwrap();
    assert_eq!(nested, vec![vec![1, 2], vec![3, 4], vec![5]]);

    // exercising Value and Definition
    let nested = gctx
        .get::<Vec<Value<Vec<Value<i32>>>>>("nested_ints")
        .unwrap();
    let def = Definition::Path(root_path);
    assert_eq!(
        nested,
        vec![
            Value {
                val: vec![
                    Value {
                        val: 1,
                        definition: def.clone(),
                    },
                    Value {
                        val: 2,
                        definition: def.clone(),
                    },
                ],
                definition: def.clone()
            },
            Value {
                val: vec![
                    Value {
                        val: 3,
                        definition: def.clone(),
                    },
                    Value {
                        val: 4,
                        definition: def.clone(),
                    },
                ],
                definition: def.clone(),
            },
            Value {
                val: vec![Value {
                    val: 5,
                    definition: Definition::Cli(None),
                },],
                definition: Definition::Cli(None),
            },
        ]
    );

    let nested = gctx.get::<Vec<Vec<bool>>>("nested_bools").unwrap();
    assert_eq!(nested, vec![vec![true], vec![false, true]]);

    let nested = gctx.get::<Vec<Vec<String>>>("nested_strings").unwrap();
    assert_eq!(
        nested,
        vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["3".to_string(), "4".to_string()]
        ]
    );

    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        x: Vec<Vec<Vec<S>>>,
        y: i32,
    }
    let nested = gctx.get::<Vec<Vec<S>>>("deeply_nested").unwrap();
    assert_eq!(
        nested,
        vec![vec![S {
            x: vec![vec![vec![S { x: vec![], y: 2 }]]],
            y: 1,
        }]],
    );
}

#[cargo_test]
fn mixed_type_array() {
    let root_path = paths::root().join(".cargo/config.toml");
    write_config_at(&root_path, r#"a = [{ x = 1 }]"#);

    let foo_path = paths::root().join("foo/.cargo/config.toml");
    write_config_at(&foo_path, r#"a = [true, [false]]"#);

    let gctx = GlobalContextBuilder::new()
        .cwd("foo")
        .env("CARGO_A", "hello")
        .config_arg("a = [123]")
        .build();

    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(untagged)]
    enum Item {
        B(bool),
        I(i32),
        S(String),
        T { x: i32 },
        L(Vec<bool>),
    }

    use Item::*;

    // Simple vector works
    assert_eq!(
        gctx.get::<Vec<Item>>("a").unwrap(),
        vec![
            T { x: 1 },
            B(true),
            L(vec![false]),
            S("hello".into()),
            I(123)
        ],
    );

    // Value and Definition works
    assert_eq!(
        gctx.get::<Value<Vec<Value<Item>>>>("a").unwrap(),
        Value {
            val: vec![
                Value {
                    val: T { x: 1 },
                    definition: Definition::Path(root_path.clone()),
                },
                Value {
                    val: B(true),
                    definition: Definition::Path(foo_path.clone()),
                },
                Value {
                    val: L(vec![false]),
                    definition: Definition::Path(foo_path.clone()),
                },
                Value {
                    val: S("hello".into()),
                    definition: Definition::Environment("CARGO_A".into()),
                },
                Value {
                    val: I(123),
                    definition: Definition::Cli(None),
                },
            ],
            definition: Definition::Environment("CARGO_A".into()),
        }
    );
}
