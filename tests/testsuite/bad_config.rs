//! Tests for some invalid .cargo/config files.

use crate::prelude::*;
use cargo_test_support::git::cargo_uses_gitoxide;
use cargo_test_support::registry::{self, Package};
use cargo_test_support::{Project, basic_bin_manifest, basic_manifest, project, rustc_host, str};

#[cargo_test]
fn bad1() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                  [target]
                  nonexistent-target = "foo"
            "#,
        )
        .build();
    p.cargo("check -v --target=nonexistent-target")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] expected table for configuration key `target.nonexistent-target`, but found string in [ROOT]/foo/.cargo/config.toml

"#]])
        .run();
}

#[cargo_test]
fn unsupported_float() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                  [http]
                    proxy = 3.0
            "#,
        )
        .build();
    p.cargo("publish -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not load Cargo configuration

Caused by:
  failed to load TOML configuration from `[ROOT]/foo/.cargo/config.toml`

Caused by:
  failed to parse config at `http.proxy`

Caused by:
  unsupported TOML configuration type `float`

"#]])
        .run();
}

#[cargo_test]
fn unsupported_datetime() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [http]
                proxy = 1979-05-27T07:32:00Z
            "#,
        )
        .build();
    p.cargo("publish -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not load Cargo configuration

Caused by:
  failed to load TOML configuration from `[ROOT]/foo/.cargo/config.toml`

Caused by:
  failed to parse config at `http.proxy`

Caused by:
  unsupported TOML configuration type `datetime`

"#]])
        .run();
}

#[cargo_test]
fn unsupported_float_array() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [alias]
                floats = [2.71828, 3.14159]
            "#,
        )
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not load Cargo configuration

Caused by:
  failed to load TOML configuration from `[ROOT]/foo/.cargo/config.toml`

Caused by:
  failed to parse config at `alias.floats[0]`

Caused by:
  unsupported TOML configuration type `float`

"#]])
        .run();
}

#[cargo_test]
fn unsupported_datetime_array() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [alias]
                datetimes = [07:32:00, 1979-05-27T07:32:00Z]
            "#,
        )
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not load Cargo configuration

Caused by:
  failed to load TOML configuration from `[ROOT]/foo/.cargo/config.toml`

Caused by:
  failed to parse config at `alias.datetimes[0]`

Caused by:
  unsupported TOML configuration type `datetime`

"#]])
        .run();
}

#[cargo_test]
fn bad3() {
    let registry = registry::init();
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [http]
                  proxy = true
            "#,
        )
        .build();
    Package::new("foo", "1.0.0").publish();

    p.cargo("publish -v")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to update registry `crates-io`

Caused by:
  error in [ROOT]/foo/.cargo/config.toml: `http.proxy` expected a string, but found a boolean

"#]])
        .run();
}

#[cargo_test]
fn bad4() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [cargo-new]
                  vcs = false
            "#,
        )
        .build();
    p.cargo("new -v foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `foo` package
[ERROR] Failed to create package `foo` at `[ROOT]/foo/foo`

Caused by:
  error in [ROOT]/foo/.cargo/config.toml: `cargo-new.vcs` expected a string, but found a boolean

"#]])
        .run();
}

#[cargo_test]
fn bad6() {
    let registry = registry::init();
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [http]
                  user-agent = true
            "#,
        )
        .build();
    Package::new("foo", "1.0.0").publish();

    p.cargo("publish -v")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to update registry `crates-io`

Caused by:
  error in [ROOT]/foo/.cargo/config.toml: `http.user-agent` expected a string, but found a boolean

"#]])
        .run();
}

#[cargo_test]
fn invalid_global_config() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "0.1.0"
            "#,
        )
        .file(".cargo/config.toml", "4")
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  could not load Cargo configuration

Caused by:
  could not parse TOML configuration in `[ROOT]/foo/.cargo/config.toml`

Caused by:
  TOML parse error at line 1, column 2
    |
  1 | 4
    |  ^
  key with no value, expected `=`

"#]])
        .run();
}

#[cargo_test]
fn bad_cargo_lock() {
    let p = project()
        .file("Cargo.lock", "[[package]]\nfoo = 92")
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse lock file at: [ROOT]/foo/Cargo.lock

Caused by:
  TOML parse error at line 1, column 1
    |
  1 | [[package]]
    | ^^^^^^^^^^^
  missing field `name`

"#]])
        .run();
}

#[cargo_test]
fn duplicate_packages_in_cargo_lock() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
                [[package]]
                name = "foo"
                version = "0.0.1"
                dependencies = [
                 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
                ]

                [[package]]
                name = "bar"
                version = "0.1.0"
                source = "registry+https://github.com/rust-lang/crates.io-index"

                [[package]]
                name = "bar"
                version = "0.1.0"
                source = "registry+https://github.com/rust-lang/crates.io-index"
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse lock file at: [ROOT]/foo/Cargo.lock

Caused by:
  package `bar` is specified twice in the lockfile

"#]])
        .run();
}

#[cargo_test]
fn bad_source_in_cargo_lock() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
                [[package]]
                name = "foo"
                version = "0.0.1"
                dependencies = [
                 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
                ]

                [[package]]
                name = "bar"
                version = "0.1.0"
                source = "You shall not parse"
            "#,
        )
        .build();

    p.cargo("check --verbose")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse lock file at: [ROOT]/foo/Cargo.lock

Caused by:
  TOML parse error at line 12, column 26
     |
  12 |                 source = "You shall not parse"
     |                          ^^^^^^^^^^^^^^^^^^^^^
  invalid source `You shall not parse`

"#]])
        .run();
}

#[cargo_test]
fn bad_dependency_in_lockfile() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
                [[package]]
                name = "foo"
                version = "0.0.1"
                dependencies = [
                 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
                ]
            "#,
        )
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn bad_git_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = {{ git = "{url}" }}
            "#,
                url = if cargo_uses_gitoxide() {
                    "git://host.xz"
                } else {
                    "file:.."
                }
            ),
        )
        .file("src/lib.rs", "")
        .build();

    if cargo_uses_gitoxide() {
        p.cargo("check -v")
            .with_status(101)
            .with_stderr_data(str![[r#"
[UPDATING] git repository `git://host.xz`
[ERROR] failed to get `foo` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `foo`

Caused by:
  Unable to update git://host.xz

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/_empty-[HASH]

Caused by:
  URL "git://host.xz" does not specify a path to a repository

"#]])
            .run();
    } else {
        p.cargo("check -v")
            .with_status(101)
            .with_stderr_data(str![[r#"
[UPDATING] git repository `file:///`
[ERROR] failed to get `foo` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `foo`

Caused by:
  Unable to update file:///

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/_empty-[HASH]

Caused by:
  'file:///' is not a valid local file URI; class=Config (7)

"#]])
            .run();
    };
}

#[cargo_test]
fn bad_crate_type() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [lib]
                crate-type = ["bad_type", "rlib"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[ERROR] unknown crate type: `bad_type`, expected one of: `lib`, `rlib`, `staticlib`, `dylib`, `cdylib`, `bin`, `proc-macro`

[ERROR] could not compile `foo` (lib) due to 1 previous error

"#]])
        .run();
}

#[cargo_test]
fn malformed_override() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [target.x86_64-apple-darwin.freetype]
                native = {
                  foo: "bar"
                }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] newlines are unsupported in inline tables, expected nothing
  --> Cargo.toml:9:27
   |
 9 |                   native = {
   |  ___________________________^
10 | |                   foo: "bar"
   | |_^

"#]])
        .run();
}

#[cargo_test]
fn cargo_toml_missing_package_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
               [package]
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  missing field `package.name`

"#]])
        .run();
}

#[cargo_test]
fn duplicate_binary_names() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
               [package]
               name = "qqq"
               version = "0.1.0"
               edition = "2015"
               authors = ["A <a@a.a>"]

               [[bin]]
               name = "e"
               path = "a.rs"

               [[bin]]
               name = "e"
               path = "b.rs"
            "#,
        )
        .file("a.rs", r#"fn main() -> () {}"#)
        .file("b.rs", r#"fn main() -> () {}"#)
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  found duplicate binary name e, but all binary targets must have a unique name

"#]])
        .run();
}

#[cargo_test]
fn duplicate_example_names() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
               [package]
               name = "qqq"
               version = "0.1.0"
               edition = "2015"
               authors = ["A <a@a.a>"]

               [[example]]
               name = "ex"
               path = "examples/ex.rs"

               [[example]]
               name = "ex"
               path = "examples/ex2.rs"
            "#,
        )
        .file("examples/ex.rs", r#"fn main () -> () {}"#)
        .file("examples/ex2.rs", r#"fn main () -> () {}"#)
        .build();

    p.cargo("check --example ex")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  found duplicate example name ex, but all example targets must have a unique name

"#]])
        .run();
}

#[cargo_test]
fn duplicate_bench_names() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
               [package]
               name = "qqq"
               version = "0.1.0"
               edition = "2015"
               authors = ["A <a@a.a>"]

               [[bench]]
               name = "ex"
               path = "benches/ex.rs"

               [[bench]]
               name = "ex"
               path = "benches/ex2.rs"
            "#,
        )
        .file("benches/ex.rs", r#"fn main () {}"#)
        .file("benches/ex2.rs", r#"fn main () {}"#)
        .build();

    p.cargo("bench")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  found duplicate bench name ex, but all bench targets must have a unique name

"#]])
        .run();
}

#[cargo_test]
fn duplicate_deps() {
    let p = project()
        .file("shim-bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("shim-bar/src/lib.rs", "pub fn a() {}")
        .file("linux-bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("linux-bar/src/lib.rs", "pub fn a() {}")
        .file(
            "Cargo.toml",
            r#"
               [package]
               name = "qqq"
               version = "0.0.1"
               edition = "2015"
               authors = []

               [dependencies]
               bar = { path = "shim-bar" }

               [target.x86_64-unknown-linux-gnu.dependencies]
               bar = { path = "linux-bar" }
            "#,
        )
        .file("src/main.rs", r#"fn main () {}"#)
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  Dependency 'bar' has different source paths depending on the build target. Each dependency must have a single canonical source path irrespective of build target.

"#]])
        .run();
}

#[cargo_test]
fn duplicate_deps_diff_sources() {
    let p = project()
        .file("shim-bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("shim-bar/src/lib.rs", "pub fn a() {}")
        .file("linux-bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("linux-bar/src/lib.rs", "pub fn a() {}")
        .file(
            "Cargo.toml",
            r#"
               [package]
               name = "qqq"
               version = "0.0.1"
               edition = "2015"
               authors = []

               [target.i686-unknown-linux-gnu.dependencies]
               bar = { path = "shim-bar" }

               [target.x86_64-unknown-linux-gnu.dependencies]
               bar = { path = "linux-bar" }
            "#,
        )
        .file("src/main.rs", r#"fn main () {}"#)
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  Dependency 'bar' has different source paths depending on the build target. Each dependency must have a single canonical source path irrespective of build target.

"#]])
        .run();
}

#[cargo_test]
fn unused_keys() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
               [package]
               name = "foo"
               version = "0.1.0"
               edition = "2015"
               authors = []

               [target.foo]
               bar = "3"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] unused manifest key: target.foo.bar
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]
                bulid = "foo"
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] unused manifest key: package.bulid
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let p = project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [lib]
                build = "foo"
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] unused manifest key: lib.build
[CHECKING] foo v0.5.0 ([ROOT]/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_keys_in_virtual_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
                bulid = "foo"
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();
    p.cargo("check --workspace")
        .with_stderr_data(str![[r#"
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: workspace.bulid
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn empty_dependencies() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = {}
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  dependency (bar) specified without providing a local path, Git repository, version, or workspace dependency to use

"#]])
        .run();
}

#[cargo_test]
fn dev_dependencies2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dev_dependencies]
                a = {path = "a"}
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();
    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] `dev_dependencies` is deprecated in favor of `dev-dependencies` and will not work in the 2024 edition
(in the `foo` package)
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn dev_dependencies2_2024() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"

                [dev_dependencies]
                a = {path = "a"}
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `dev_dependencies` is unsupported as of the 2024 edition; instead use `dev-dependencies`
  (in the `foo` package)

"#]])
        .run();
}

#[cargo_test]
fn dev_dependencies2_conflict() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dev-dependencies]
                a = {path = "a"}
                [dev_dependencies]
                a = {path = "a"}
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();
    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] `dev_dependencies` is redundant with `dev-dependencies`, preferring `dev-dependencies` in the `foo` package
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn build_dependencies2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [build_dependencies]
                a = {path = "a"}
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();
    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] `build_dependencies` is deprecated in favor of `build-dependencies` and will not work in the 2024 edition
(in the `foo` package)
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn build_dependencies2_2024() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"

                [build_dependencies]
                a = {path = "a"}
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `build_dependencies` is unsupported as of the 2024 edition; instead use `build-dependencies`
  (in the `foo` package)

"#]])
        .run();
}

#[cargo_test]
fn build_dependencies2_conflict() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [build-dependencies]
                a = {path = "a"}
                [build_dependencies]
                a = {path = "a"}
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();
    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] `build_dependencies` is redundant with `build-dependencies`, preferring `build-dependencies` in the `foo` package
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn lib_crate_type2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [lib]
                name = "foo"
                crate_type = ["staticlib", "dylib"]
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `crate_type` is deprecated in favor of `crate-type` and will not work in the 2024 edition
(in the `foo` library target)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn lib_crate_type2_2024() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2024"
                authors = ["wycats@example.com"]

                [lib]
                name = "foo"
                crate_type = ["staticlib", "dylib"]
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `crate_type` is unsupported as of the 2024 edition; instead use `crate-type`
  (in the `foo` library target)

"#]])
        .run();
}

#[cargo_test]
fn lib_crate_type2_conflict() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [lib]
                name = "foo"
                crate-type = ["rlib", "dylib"]
                crate_type = ["staticlib", "dylib"]
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();
    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] `crate_type` is redundant with `crate-type`, preferring `crate-type` in the `foo` library target
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn bin_crate_type2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [[bin]]
                name = "foo"
                path = "src/main.rs"
                crate_type = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `crate_type` is deprecated in favor of `crate-type` and will not work in the 2024 edition
(in the `foo` binary target)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn bin_crate_type2_2024() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2024"
                authors = ["wycats@example.com"]

                [[bin]]
                name = "foo"
                path = "src/main.rs"
                crate_type = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `crate_type` is unsupported as of the 2024 edition; instead use `crate-type`
  (in the `foo` binary target)

"#]])
        .run();
}

#[cargo_test]
fn bin_crate_type2_conflict() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [[bin]]
                name = "foo"
                path = "src/main.rs"
                crate_type = []
                crate-type = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] `crate_type` is redundant with `crate-type`, preferring `crate-type` in the `foo` binary target
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn examples_crate_type2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [[example]]
                name = "ex"
                path = "examples/ex.rs"
                crate_type = ["proc_macro"]
                [[example]]
                name = "goodbye"
                path = "examples/ex-goodbye.rs"
                crate_type = ["rlib", "staticlib"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "examples/ex.rs",
            r#"
                fn main() { println!("ex"); }
            "#,
        )
        .file(
            "examples/ex-goodbye.rs",
            r#"
                fn main() { println!("goodbye"); }
            "#,
        )
        .build();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `crate_type` is deprecated in favor of `crate-type` and will not work in the 2024 edition
(in the `ex` example target)
[WARNING] `crate_type` is deprecated in favor of `crate-type` and will not work in the 2024 edition
(in the `goodbye` example target)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn examples_crate_type2_2024() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2024"
                authors = ["wycats@example.com"]

                [[example]]
                name = "ex"
                path = "examples/ex.rs"
                crate_type = ["proc_macro"]
                [[example]]
                name = "goodbye"
                path = "examples/ex-goodbye.rs"
                crate_type = ["rlib", "staticlib"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "examples/ex.rs",
            r#"
                fn main() { println!("ex"); }
            "#,
        )
        .file(
            "examples/ex-goodbye.rs",
            r#"
                fn main() { println!("goodbye"); }
            "#,
        )
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `crate_type` is unsupported as of the 2024 edition; instead use `crate-type`
  (in the `ex` example target)

"#]])
        .run();
}

#[cargo_test]
fn examples_crate_type2_conflict() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [[example]]
                name = "ex"
                path = "examples/ex.rs"
                crate-type = ["rlib", "dylib"]
                crate_type = ["proc_macro"]
                [[example]]
                name = "goodbye"
                path = "examples/ex-goodbye.rs"
                crate-type = ["rlib", "dylib"]
                crate_type = ["rlib", "staticlib"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "examples/ex.rs",
            r#"
                fn main() { println!("ex"); }
            "#,
        )
        .file(
            "examples/ex-goodbye.rs",
            r#"
                fn main() { println!("goodbye"); }
            "#,
        )
        .build();
    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] `crate_type` is redundant with `crate-type`, preferring `crate-type` in the `ex` example target
[WARNING] `crate_type` is redundant with `crate-type`, preferring `crate-type` in the `goodbye` example target
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn cargo_platform_build_dependencies2() {
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = ["wycats@example.com"]
                    build = "build.rs"

                    [target.{host}.build_dependencies]
                    build = {{ path = "build" }}
                "#,
                host = host
            ),
        )
        .file("src/main.rs", "fn main() { }")
        .file(
            "build.rs",
            "extern crate build; fn main() { build::build(); }",
        )
        .file("build/Cargo.toml", &basic_manifest("build", "0.5.0"))
        .file("build/src/lib.rs", "pub fn build() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `build_dependencies` is deprecated in favor of `build-dependencies` and will not work in the 2024 edition
(in the `[HOST_TARGET]` platform target)
[LOCKING] 1 package to latest compatible version
[COMPILING] build v0.5.0 ([ROOT]/foo/build)
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
        )
        .run();
}

#[cargo_test]
fn cargo_platform_build_dependencies2_2024() {
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2024"
                    authors = ["wycats@example.com"]
                    build = "build.rs"

                    [target.{host}.build_dependencies]
                    build = {{ path = "build" }}
                "#,
                host = host
            ),
        )
        .file("src/main.rs", "fn main() { }")
        .file(
            "build.rs",
            "extern crate build; fn main() { build::build(); }",
        )
        .file("build/Cargo.toml", &basic_manifest("build", "0.5.0"))
        .file("build/src/lib.rs", "pub fn build() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `build_dependencies` is unsupported as of the 2024 edition; instead use `build-dependencies`
  (in the `[HOST_TARGET]` platform target)

"#]])
        .run();
}

#[cargo_test]
fn cargo_platform_build_dependencies2_conflict() {
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = ["wycats@example.com"]
                    build = "build.rs"

                    [target.{host}.build-dependencies]
                    build = {{ path = "build" }}
                    [target.{host}.build_dependencies]
                    build = {{ path = "build" }}
                "#,
                host = host
            ),
        )
        .file("src/main.rs", "fn main() { }")
        .file(
            "build.rs",
            "extern crate build; fn main() { build::build(); }",
        )
        .file("build/Cargo.toml", &basic_manifest("build", "0.5.0"))
        .file("build/src/lib.rs", "pub fn build() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `build_dependencies` is redundant with `build-dependencies`, preferring `build-dependencies` in the `[HOST_TARGET]` platform target
[LOCKING] 1 package to latest compatible version
[COMPILING] build v0.5.0 ([ROOT]/foo/build)
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])

        .run();
}

#[cargo_test]
fn cargo_platform_dev_dependencies2() {
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = ["wycats@example.com"]

                    [target.{host}.dev_dependencies]
                    dev = {{ path = "dev" }}
                "#,
                host = host
            ),
        )
        .file("src/main.rs", "fn main() { }")
        .file(
            "tests/foo.rs",
            "extern crate dev; #[test] fn foo() { dev::dev() }",
        )
        .file("dev/Cargo.toml", &basic_manifest("dev", "0.5.0"))
        .file("dev/src/lib.rs", "pub fn dev() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `dev_dependencies` is deprecated in favor of `dev-dependencies` and will not work in the 2024 edition
(in the `[HOST_TARGET]` platform target)
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn cargo_platform_dev_dependencies2_2024() {
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2024"
                    authors = ["wycats@example.com"]

                    [target.{host}.dev_dependencies]
                    dev = {{ path = "dev" }}
                "#,
                host = host
            ),
        )
        .file("src/main.rs", "fn main() { }")
        .file(
            "tests/foo.rs",
            "extern crate dev; #[test] fn foo() { dev::dev() }",
        )
        .file("dev/Cargo.toml", &basic_manifest("dev", "0.5.0"))
        .file("dev/src/lib.rs", "pub fn dev() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `dev_dependencies` is unsupported as of the 2024 edition; instead use `dev-dependencies`
  (in the `[HOST_TARGET]` platform target)

"#]])
        .run();
}

#[cargo_test]
fn cargo_platform_dev_dependencies2_conflict() {
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = ["wycats@example.com"]

                    [target.{host}.dev-dependencies]
                    dev = {{ path = "dev" }}
                    [target.{host}.dev_dependencies]
                    dev = {{ path = "dev" }}
                "#,
                host = host
            ),
        )
        .file("src/main.rs", "fn main() { }")
        .file(
            "tests/foo.rs",
            "extern crate dev; #[test] fn foo() { dev::dev() }",
        )
        .file("dev/Cargo.toml", &basic_manifest("dev", "0.5.0"))
        .file("dev/src/lib.rs", "pub fn dev() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `dev_dependencies` is redundant with `dev-dependencies`, preferring `dev-dependencies` in the `[HOST_TARGET]` platform target
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn default_features2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                a = { path = "a", features = ["f1"], default_features = false }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [features]
                default = ["f1"]
                f1 = []
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] `default_features` is deprecated in favor of `default-features` and will not work in the 2024 edition
(in the `a` dependency)
[LOCKING] 1 package to latest compatible version
[CHECKING] a v0.1.0 ([ROOT]/foo/a)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn default_features2_2024() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"
                authors = []

                [dependencies]
                a = { path = "a", features = ["f1"], default_features = false }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [features]
                default = ["f1"]
                f1 = []
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `default_features` is unsupported as of the 2024 edition; instead use `default-features`
  (in the `a` dependency)

"#]])
        .run();
}

#[cargo_test]
fn default_features2_conflict() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                a = { path = "a", features = ["f1"], default-features = false, default_features = false }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [features]
                default = ["f1"]
                f1 = []
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] `default_features` is redundant with `default-features`, preferring `default-features` in the `a` dependency
[LOCKING] 1 package to latest compatible version
[CHECKING] a v0.1.0 ([ROOT]/foo/a)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn workspace_default_features2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["workspace_only", "dep_workspace_only", "package_only", "dep_package_only"]

                [workspace.dependencies]
                dep_workspace_only = { path = "dep_workspace_only", default_features = true }
                dep_package_only = { path = "dep_package_only" }
            "#,
        )
        .file(
            "workspace_only/Cargo.toml",
            r#"
                [package]
                name = "workspace_only"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                dep_workspace_only.workspace = true
            "#,
        )
        .file("workspace_only/src/lib.rs", "")
        .file(
            "dep_workspace_only/Cargo.toml",
            r#"
                [package]
                name = "dep_workspace_only"
                version = "0.1.0"
                edition = "2015"
                authors = []
            "#,
        )
        .file("dep_workspace_only/src/lib.rs", "")
        .file(
            "package_only/Cargo.toml",
            r#"
                [package]
                name = "package_only"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                dep_package_only = { workspace = true, default_features = true }
            "#,
        )
        .file("package_only/src/lib.rs", "")
        .file(
            "dep_package_only/Cargo.toml",
            r#"
                [package]
                name = "dep_package_only"
                version = "0.1.0"
                edition = "2015"
                authors = []
            "#,
        )
        .file("dep_package_only/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(
            str![[r#"
[CHECKING] dep_package_only v0.1.0 ([ROOT]/foo/dep_package_only)
[CHECKING] dep_workspace_only v0.1.0 ([ROOT]/foo/dep_workspace_only)
[CHECKING] package_only v0.1.0 ([ROOT]/foo/package_only)
[CHECKING] workspace_only v0.1.0 ([ROOT]/foo/workspace_only)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[WARNING] [ROOT]/foo/workspace_only/Cargo.toml: `default_features` is deprecated in favor of `default-features` and will not work in the 2024 edition
(in the `dep_workspace_only` dependency)

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn workspace_default_features2_2024() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["workspace_only", "dep_workspace_only", "package_only", "dep_package_only"]

                [workspace.dependencies]
                dep_workspace_only = { path = "dep_workspace_only", default_features = true }
                dep_package_only = { path = "dep_package_only" }
            "#,
        )
        .file(
            "workspace_only/Cargo.toml",
            r#"
                [package]
                name = "workspace_only"
                version = "0.1.0"
                edition = "2024"
                authors = []

                [dependencies]
                dep_workspace_only.workspace = true
            "#,
        )
        .file("workspace_only/src/lib.rs", "")
        .file(
            "dep_workspace_only/Cargo.toml",
            r#"
                [package]
                name = "dep_workspace_only"
                version = "0.1.0"
                edition = "2024"
                authors = []
            "#,
        )
        .file("dep_workspace_only/src/lib.rs", "")
        .file(
            "package_only/Cargo.toml",
            r#"
                [package]
                name = "package_only"
                version = "0.1.0"
                edition = "2024"
                authors = []

                [dependencies]
                dep_package_only = { workspace = true, default_features = true }
            "#,
        )
        .file("package_only/src/lib.rs", "")
        .file(
            "dep_package_only/Cargo.toml",
            r#"
                [package]
                name = "dep_package_only"
                version = "0.1.0"
                edition = "2024"
                authors = []
            "#,
        )
        .file("dep_package_only/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to load manifest for workspace member `[ROOT]/foo/workspace_only`
referenced by workspace at `[ROOT]/foo/Cargo.toml`

Caused by:
  failed to parse manifest at `[ROOT]/foo/workspace_only/Cargo.toml`

Caused by:
  `default_features` is unsupported as of the 2024 edition; instead use `default-features`
  (in the `dep_workspace_only` dependency)

"#]])
        .run();
}

#[cargo_test]
fn lib_proc_macro2() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                [lib]
                proc_macro = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `proc_macro` is deprecated in favor of `proc-macro` and will not work in the 2024 edition
(in the `foo` library target)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn lib_proc_macro2_2024() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"
                [lib]
                proc_macro = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `proc_macro` is unsupported as of the 2024 edition; instead use `proc-macro`
  (in the `foo` library target)

"#]])
        .run();
}

#[cargo_test]
fn lib_proc_macro2_conflict() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                [lib]
                proc-macro = false
                proc_macro = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check").with_stderr_data(str![[r#"
[WARNING] `proc_macro` is redundant with `proc-macro`, preferring `proc-macro` in the `foo` library target
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn bin_proc_macro2() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [[bin]]
                name = "foo"
                path = "src/main.rs"
                proc_macro = false
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    foo.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `proc_macro` is deprecated in favor of `proc-macro` and will not work in the 2024 edition
(in the `foo` binary target)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn bin_proc_macro2_2024() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2024"
                authors = ["wycats@example.com"]

                [[bin]]
                name = "foo"
                path = "src/main.rs"
                proc_macro = false
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `proc_macro` is unsupported as of the 2024 edition; instead use `proc-macro`
  (in the `foo` binary target)

"#]])
        .run();
}

#[cargo_test]
fn bin_proc_macro2_conflict() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [[bin]]
                name = "foo"
                path = "src/main.rs"
                proc-macro = false
                proc_macro = false
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    foo.cargo("check").with_stderr_data(str![[r#"
[WARNING] `proc_macro` is redundant with `proc-macro`, preferring `proc-macro` in the `foo` binary target
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn invalid_toml_historically_allowed_fails() {
    let p = project()
        .file(".cargo/config.toml", "[bar] baz = 2")
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not load Cargo configuration

Caused by:
  could not parse TOML configuration in `[ROOT]/foo/.cargo/config.toml`

Caused by:
  TOML parse error at line 1, column 7
    |
  1 | [bar] baz = 2
    |       ^
  unexpected key or value, expected newline, `#`

"#]])
        .run();
}

#[cargo_test]
fn ambiguous_git_reference() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1"
                branch = "master"
                tag = "some-tag"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  dependency (bar) specification is ambiguous. Only one of `branch`, `tag` or `rev` is allowed.

"#]])
        .run();
}

#[cargo_test]
fn fragment_in_git_url() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1#foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -v")
        .with_status(101)
        // the following is needed as gitoxide has a different error message
        // ...
        // [..]127.0.0.1[..]
        .with_stderr_data(str![[r#"
[WARNING] URL fragment `#foo` in git URL is ignored for dependency (bar). If you were trying to specify a specific git revision, use `rev = "foo"` in the dependency declaration.
[UPDATING] git repository `http://127.0.0.1/#foo`
...
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update http://127.0.0.1/#foo

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/_empty-[HASH]
...

"#]])
        .run();
}

#[cargo_test]
fn bad_source_config1() {
    let p = project()
        .file("src/lib.rs", "")
        .file(".cargo/config.toml", "[source.foo]")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no source location specified for `source.foo`, need `registry`, `local-registry`, `directory`, or `git` defined

"#]])
        .run();
}

#[cargo_test]
fn bad_source_config2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [source.crates-io]
                registry = 'http://example.com'
                replace-with = 'bar'
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update registry `crates-io`

Caused by:
  could not find a configured source with the name `bar` when attempting to lookup `crates-io` (configuration in `[ROOT]/foo/.cargo/config.toml`)

"#]])
        .run();
}

#[cargo_test]
fn bad_source_config3() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [source.crates-io]
                registry = 'https://example.com'
                replace-with = 'crates-io'
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update registry `crates-io`

Caused by:
  detected a cycle of `replace-with` sources, the source `crates-io` is eventually replaced with itself (configuration in `[ROOT]/foo/.cargo/config.toml`)

"#]])
        .run();
}

#[cargo_test]
fn bad_source_config4() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [source.crates-io]
                replace-with = 'bar'

                [source.bar]
                registry = 'https://example.com'
                replace-with = 'crates-io'
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update registry `crates-io`

Caused by:
  detected a cycle of `replace-with` sources, the source `crates-io` is eventually replaced with itself (configuration in `[ROOT]/foo/.cargo/config.toml`)

"#]])
        .run();
}

#[cargo_test]
fn bad_source_config5() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [source.crates-io]
                registry = 'https://example.com'
                replace-with = 'bar'

                [source.bar]
                registry = 'not a url'
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] configuration key `source.bar.registry` specified an invalid URL (in [ROOT]/foo/.cargo/config.toml)

Caused by:
  invalid url `not a url`: relative URL without a base

"#]])
        .run();
}

#[cargo_test]
fn both_git_and_path_specified() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1"
                path = "bar"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  dependency (bar) specification is ambiguous. Only one of `git` or `path` is allowed.

"#]])
        .run();
}

#[cargo_test]
fn bad_source_config6() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [source.crates-io]
                registry = 'https://example.com'
                replace-with = ['not', 'a', 'string']
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] error in [ROOT]/foo/.cargo/config.toml: could not load config key `source.crates-io.replace-with`

Caused by:
  error in [ROOT]/foo/.cargo/config.toml: `source.crates-io.replace-with` expected a string, but found a array

"#]])
        .run();
}

#[cargo_test]
fn ignored_git_revision() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
                branch = "spam"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  key `branch` is ignored for dependency (bar).

"#]])
        .run();

    // #11540, check that [target] dependencies fail the same way.
    foo.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.0.0"
            edition = "2015"

            [target.some-target.dependencies]
            bar = { path = "bar", branch = "spam" }
        "#,
    );
    foo.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  key `branch` is ignored for dependency (bar).

"#]])
        .run();
}

#[cargo_test]
fn bad_source_config7() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [source.foo]
                registry = 'https://example.com'
                local-registry = 'file:///another/file'
            "#,
        )
        .build();

    Package::new("bar", "0.1.0").publish();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] more than one source location specified for `source.foo`

"#]])
        .run();
}

#[cargo_test]
fn bad_source_config8() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [source.foo]
                branch = "somebranch"
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] source definition `source.foo` specifies `branch`, but that requires a `git` key to be specified (in [ROOT]/foo/.cargo/config.toml)

"#]])
        .run();
}

#[cargo_test]
fn bad_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = 3
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: integer `3`, expected a version string like "0.9.8" or a detailed dependency like { version = "0.9.8" }
 --> Cargo.toml:9:23
  |
9 |                 bar = 3
  |                       ^

"#]])
        .run();
}

#[cargo_test]
fn bad_dependency_true_literal() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: boolean `true`, expected a version string like "0.9.8" or a detailed dependency like { version = "0.9.8" }
       [NOTE] if you meant to use a workspace member, you can write
        dep.workspace = true
 --> Cargo.toml:9:23
  |
9 |                 bar = true
  |                       ^^^^

"#]])
        .run();
}

#[cargo_test]
fn bad_debuginfo() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [profile.dev]
                debug = 'a'
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid value: string "a", expected a boolean, 0, 1, 2, "none", "limited", "full", "line-tables-only", or "line-directives-only"
 --> Cargo.toml:9:25
  |
9 |                 debug = 'a'
  |                         ^^^

"#]])
        .run();
}

#[cargo_test]
fn bad_debuginfo2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [profile.dev]
                debug = 3.6
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: floating point `3.6`, expected a boolean, 0, 1, 2, "none", "limited", "full", "line-tables-only", or "line-directives-only"
 --> Cargo.toml:9:25
  |
9 |                 debug = 3.6
  |                         ^^^

"#]])
        .run();
}

#[cargo_test]
fn bad_opt_level() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = 3
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: integer `3`, expected a boolean, string or array
 --> Cargo.toml:7:25
  |
7 |                 build = 3
  |                         ^

"#]])
        .run();
}

#[cargo_test]
fn warn_semver_metadata() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            edition = "2015"

            [dependencies]
            bar = "1.0.0+1234"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] version requirement `1.0.0+1234` for dependency `bar` includes semver metadata which will be ignored, removing the metadata is recommended to avoid confusion
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[CHECKING] foo v1.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn bad_http_ssl_version() {
    // Invalid type in SslVersionConfig.
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [http]
            ssl-version = ["tlsv1.2", "tlsv1.3"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] error in [ROOT]/foo/.cargo/config.toml: could not load config key `http.ssl-version`

Caused by:
  invalid type: sequence, expected a string or map

"#]])
        .run();
}

#[cargo_test]
fn bad_http_ssl_version_range() {
    // Invalid type in SslVersionConfigRange.
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [http]
            ssl-version.min = false
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] error in [ROOT]/foo/.cargo/config.toml: could not load config key `http.ssl-version`

Caused by:
  error in [ROOT]/foo/.cargo/config.toml: `http.ssl-version.min` expected a string, but found a boolean

"#]])
        .run();
}

#[cargo_test]
fn bad_build_jobs() {
    // Invalid type in JobsConfig.
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            jobs = { default = true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] error in [ROOT]/foo/.cargo/config.toml: could not load config key `build.jobs`

Caused by:
  invalid type: map, expected an integer or string

"#]])
        .run();
}

#[cargo_test]
fn bad_build_target() {
    // Invalid type in BuildTargetConfig.
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target.'cfg(unix)' = "x86_64"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] error in [ROOT]/foo/.cargo/config.toml: could not load config key `build.target`

Caused by:
  error in [ROOT]/foo/.cargo/config.toml: could not load config key `build.target`

Caused by:
  invalid type: map, expected a string or array

"#]])
        .run();
}

#[cargo_test]
fn bad_target_cfg() {
    // Invalid type in a StringList.
    //
    // The error message is a bit unfortunate here. The type here ends up
    // being essentially Value<Value<StringList>>, and each layer of "Value"
    // adds some context to the error message. Also, untagged enums provide
    // strange error messages. Hopefully most users will be able to untangle
    // the message.
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [target.'cfg(not(target_os = "none"))']
            runner = false
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] error in [ROOT]/foo/.cargo/config.toml: could not load config key `target.'cfg(not(target_os = "none"))'.runner`

Caused by:
  error in [ROOT]/foo/.cargo/config.toml: could not load config key `target.'cfg(not(target_os = "none"))'.runner`

Caused by:
  invalid configuration for key `target.'cfg(not(target_os = "none"))'.runner`
  expected a string or array of strings, but found a boolean for `target.'cfg(not(target_os = "none"))'.runner` in [ROOT]/foo/.cargo/config.toml

"#]])
        .run();
}

#[cargo_test]
fn bad_target_links_overrides() {
    // Invalid parsing of links overrides.
    //
    // This error message is terrible. Nothing in the deserialization path is
    // using config::Value<>, so nothing is able to report the location. I
    // think this illustrates how the way things break down with how it
    // currently is designed with serde.
    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                [target.{}.somelib]
                rustc-flags = 'foo'
                "#,
                rustc_host()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r"
[ERROR] Only `-l` and `-L` flags are allowed in target config `target.[..].rustc-flags` (in [..]foo/.cargo/config.toml): `foo`

"]])
        .run();

    p.change_file(
        ".cargo/config.toml",
        &format!(
            "[target.{}.somelib]
            warning = \"foo\"
            ",
            rustc_host(),
        ),
    );
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `warning` is not supported in build script overrides

"#]])
        .run();
}

#[cargo_test]
fn redefined_sources() {
    // Cannot define a source multiple times.
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [source.foo]
            registry = "https://github.com/rust-lang/crates.io-index"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] source `foo` defines source registry `crates-io`, but that source is already defined by `crates-io`
[NOTE] Sources are not allowed to be defined multiple times.

"#]])
        .run();

    p.change_file(
        ".cargo/config.toml",
        r#"
        [source.one]
        directory = "index"

        [source.two]
        directory = "index"
        "#,
    );

    // Name is `[..]` because we can't guarantee the order.
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] source `[..]` defines source dir [ROOT]/foo/index, but that source is already defined by `[..]`
[NOTE] Sources are not allowed to be defined multiple times.

"#]])
        .run();
}

#[cargo_test]
fn bad_trim_paths() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"

                [profile.dev]
                trim-paths = "split-debuginfo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["trim-paths"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] expected a boolean, "none", "diagnostics", "macro", "object", "all", or an array with these options
 --> Cargo.toml:8:30
  |
8 |                 trim-paths = "split-debuginfo"
  |                              ^^^^^^^^^^^^^^^^^

"#]])
        .run();
}

fn bad_target_name_project(target: &str, path: &str, name: &str) -> Project {
    project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
[package]
name = "bad-{target}-name"
edition = "2024"

[[{target}]]
name = "{name}"
"#
            ),
        )
        .file("src/lib.rs", "")
        .file(format!("{path}/{name}"), "")
        .build()
}

#[cargo_test]
fn bad_bin_name() {
    bad_target_name_project("bin", "src/bin", "bin.rs")
        .cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `bin.rs` bin at `src/bin/bin.rs.rs` or `src/bin/bin.rs/main.rs`. Please specify bin.path if you want to use a non-default path.

  [HELP] a bin with a similar name exists: `bin`

"#]])
        .run();
}

#[cargo_test]
fn bad_example_name() {
    bad_target_name_project("example", "examples", "example.rs")
        .cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `example.rs` example at `examples/example.rs.rs` or `examples/example.rs/main.rs`. Please specify example.path if you want to use a non-default path.

  [HELP] a example with a similar name exists: `example`

"#]])
        .run();
}

#[cargo_test]
fn bad_test_name() {
    bad_target_name_project("test", "tests", "test.rs")
        .cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `test.rs` test at `tests/test.rs.rs` or `tests/test.rs/main.rs`. Please specify test.path if you want to use a non-default path.

  [HELP] a test with a similar name exists: `test`

"#]])
        .run();
}

#[cargo_test]
fn bad_bench_name() {
    bad_target_name_project("bench", "benches", "bench.rs")
        .cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `bench.rs` bench at `benches/bench.rs.rs` or `benches/bench.rs/main.rs`. Please specify bench.path if you want to use a non-default path.

  [HELP] a bench with a similar name exists: `bench`

"#]])
        .run();
}

#[cargo_test]
fn non_existing_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2015"

                [lib]
                name = "foo"
                path = "src/lib.rs"

                [[test]]
                name = "hello"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check --tests -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `hello` test at `tests/hello.rs` or `tests/hello/main.rs`. Please specify test.path if you want to use a non-default path.

"#]])
        .run();
}

#[cargo_test]
fn non_existing_example() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2015"

                [lib]
                name = "foo"
                path = "src/lib.rs"

                [[example]]
                name = "hello"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check --examples -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `hello` example at `examples/hello.rs` or `examples/hello/main.rs`. Please specify example.path if you want to use a non-default path.

"#]])
        .run();
}

#[cargo_test]
fn non_existing_benchmark() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2015"

                [lib]
                name = "foo"
                path = "src/lib.rs"

                [[bench]]
                name = "hello"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check --benches -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `hello` bench at `benches/hello.rs` or `benches/hello/main.rs`. Please specify bench.path if you want to use a non-default path.

"#]])
        .run();
}

#[cargo_test]
fn non_existing_binary() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/bin/ehlo.rs", "")
        .build();

    p.cargo("check -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `foo` bin at `src/bin/foo.rs` or `src/bin/foo/main.rs`. Please specify bin.path if you want to use a non-default path.

  [HELP] a bin with a similar name exists: `ehlo`

"#]])
        .run();
}

#[cargo_test]
fn commonly_wrong_path_of_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2015"

                [lib]
                name = "foo"
                path = "src/lib.rs"

                [[test]]
                name = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .file("test/foo.rs", "")
        .build();

    p.cargo("check --tests -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `foo` test at default paths, but found a file at `test/foo.rs`.
  Perhaps rename the file to `tests/foo.rs` for target auto-discovery, or specify test.path if you want to use a non-default path.

"#]])
        .run();
}

#[cargo_test]
fn commonly_wrong_path_of_example() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2015"

                [lib]
                name = "foo"
                path = "src/lib.rs"

                [[example]]
                name = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .file("example/foo.rs", "")
        .build();

    p.cargo("check --examples -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `foo` example at default paths, but found a file at `example/foo.rs`.
  Perhaps rename the file to `examples/foo.rs` for target auto-discovery, or specify example.path if you want to use a non-default path.

"#]])
        .run();
}

#[cargo_test]
fn commonly_wrong_path_of_benchmark() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2015"

                [lib]
                name = "foo"
                path = "src/lib.rs"

                [[bench]]
                name = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .file("bench/foo.rs", "")
        .build();

    p.cargo("check --benches -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `foo` bench at default paths, but found a file at `bench/foo.rs`.
  Perhaps rename the file to `benches/foo.rs` for target auto-discovery, or specify bench.path if you want to use a non-default path.

"#]])
        .run();
}

#[cargo_test]
fn commonly_wrong_path_binary() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/bins/foo.rs", "")
        .build();

    p.cargo("check -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `foo` bin at default paths, but found a file at `src/bins/foo.rs`.
  Perhaps rename the file to `src/bin/foo.rs` for target auto-discovery, or specify bin.path if you want to use a non-default path.

"#]])
        .run();
}

#[cargo_test]
fn commonly_wrong_path_subdir_binary() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/bins/foo/main.rs", "")
        .build();

    p.cargo("check -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `foo` bin at default paths, but found a file at `src/bins/foo/main.rs`.
  Perhaps rename the file to `src/bin/foo/main.rs` for target auto-discovery, or specify bin.path if you want to use a non-default path.

"#]])
        .run();
}

#[cargo_test]
fn found_multiple_target_files() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/bin/foo.rs", "")
        .file("src/bin/foo/main.rs", "")
        .build();

    p.cargo("check -v")
        .with_status(101)
        // Don't assert the inferred paths since the order is non-deterministic.
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  cannot infer path for `foo` bin
  Cargo doesn't know which to use because multiple target files found at `src/bin/foo[..]rs` and `src/bin/foo[..].rs`.

"#]])
        .run();
}

#[cargo_test]
fn legacy_binary_paths_warnings() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2015"
                authors = []

                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[WARNING] An explicit [[bin]] section is specified in Cargo.toml which currently
disables Cargo from automatically inferring other binary targets.
This inference behavior will change in the Rust 2018 edition and the following
files will be included as a binary target:

* src/main.rs

This is likely to break cargo build or cargo test as these files may not be
ready to be compiled as a binary target today. You can future-proof yourself
and disable this warning by adding `autobins = false` to your [package]
section. You may also move the files to a location where Cargo would not
automatically infer them to be a target, such as in subfolders.

For more information on this warning you can consult
https://github.com/rust-lang/cargo/issues/5330
[WARNING] path `src/main.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml
[CHECKING] foo v1.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2015"
                authors = []

                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[WARNING] An explicit [[bin]] section is specified in Cargo.toml which currently
disables Cargo from automatically inferring other binary targets.
This inference behavior will change in the Rust 2018 edition and the following
files will be included as a binary target:

* src/bin/main.rs

This is likely to break cargo build or cargo test as these files may not be
ready to be compiled as a binary target today. You can future-proof yourself
and disable this warning by adding `autobins = false` to your [package]
section. You may also move the files to a location where Cargo would not
automatically infer them to be a target, such as in subfolders.

For more information on this warning you can consult
https://github.com/rust-lang/cargo/issues/5330
[WARNING] path `src/bin/main.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml
[CHECKING] foo v1.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2015"
                authors = []

                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/bar.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[WARNING] path `src/bar.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml
[CHECKING] foo v1.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
