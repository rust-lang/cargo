use cargotest::support::{execs, project};
use cargotest::support::registry::Package;
use hamcrest::assert_that;

#[test]
fn bad1() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
              [target]
              nonexistent-target = "foo"
        "#,
        )
        .build();
    assert_that(
        p.cargo("build")
            .arg("-v")
            .arg("--target=nonexistent-target"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] expected table for configuration key `target.nonexistent-target`, \
but found string in [..]config
",
        ),
    );
}

#[test]
fn bad2() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
              [http]
                proxy = 3.0
        "#,
        )
        .build();
    assert_that(
        p.cargo("publish").arg("-v"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] Couldn't load Cargo configuration

Caused by:
  failed to load TOML configuration from `[..]config`

Caused by:
  failed to parse key `http`

Caused by:
  failed to parse key `proxy`

Caused by:
  found TOML configuration value of unknown type `float`
",
        ),
    );
}

#[test]
fn bad3() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [http]
              proxy = true
        "#,
        )
        .build();
    Package::new("foo", "1.0.0").publish();

    assert_that(
        p.cargo("publish").arg("-v"),
        execs().with_status(101).with_stderr(
            "\
error: failed to update registry [..]

Caused by:
  invalid configuration for key `http.proxy`
expected a string, but found a boolean for `http.proxy` in [..]config
",
        ),
    );
}

#[test]
fn bad4() {
    let p = project("foo")
        .file(
            ".cargo/config",
            r#"
            [cargo-new]
              name = false
        "#,
        )
        .build();
    assert_that(
        p.cargo("new").arg("-v").arg("foo"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] Failed to create project `foo` at `[..]`

Caused by:
  invalid configuration for key `cargo-new.name`
expected a string, but found a boolean for `cargo-new.name` in [..]config
",
        ),
    );
}

#[test]
fn bad5() {
    let p = project("foo")
        .file(
            ".cargo/config",
            r#"
            foo = ""
        "#,
        )
        .file(
            "foo/.cargo/config",
            r#"
            foo = 2
        "#,
        )
        .build();
    assert_that(
        p.cargo("new")
            .arg("-v")
            .arg("foo")
            .cwd(&p.root().join("foo")),
        execs().with_status(101).with_stderr(
            "\
[ERROR] Failed to create project `foo` at `[..]`

Caused by:
  Couldn't load Cargo configuration

Caused by:
  failed to merge configuration at `[..]`

Caused by:
  failed to merge key `foo` between files:
  file 1: [..]foo[..]foo[..]config
  file 2: [..]foo[..]config

Caused by:
  expected integer, but found string
",
        ),
    );
}

#[test]
fn bad_cargo_config_jobs() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [build]
            jobs = -1
        "#,
        )
        .build();
    assert_that(
        p.cargo("build").arg("-v"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] build.jobs must be positive, but found -1 in [..]"),
    );
}

#[test]
fn default_cargo_config_jobs() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [build]
            jobs = 1
        "#,
        )
        .build();
    assert_that(p.cargo("build").arg("-v"), execs().with_status(0));
}

#[test]
fn good_cargo_config_jobs() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [build]
            jobs = 4
        "#,
        )
        .build();
    assert_that(p.cargo("build").arg("-v"), execs().with_status(0));
}

#[test]
fn invalid_global_config() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#,
        )
        .file(".cargo/config", "4")
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").arg("-v"),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  Couldn't load Cargo configuration

Caused by:
  could not parse TOML configuration in `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  expected an equals, found eof at line 1
",
        ),
    );
}

#[test]
fn bad_cargo_lock() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#,
        )
        .file("Cargo.lock", "[[package]]\nfoo = 92")
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").arg("-v"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse lock file at: [..]Cargo.lock

Caused by:
  missing field `name` for key `package`
",
        ),
    );
}

#[test]
fn duplicate_packages_in_cargo_lock() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
            [[package]]
            name = "bar"
            version = "0.0.1"
            dependencies = [
             "foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "foo"
            version = "0.1.0"
            source = "registry+https://github.com/rust-lang/crates.io-index"

            [[package]]
            name = "foo"
            version = "0.1.0"
            source = "registry+https://github.com/rust-lang/crates.io-index"
        "#,
        )
        .build();

    assert_that(
        p.cargo("build").arg("--verbose"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse lock file at: [..]

Caused by:
  package `foo` is specified twice in the lockfile
",
        ),
    );
}

#[test]
fn bad_source_in_cargo_lock() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
            [[package]]
            name = "bar"
            version = "0.0.1"
            dependencies = [
             "foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "foo"
            version = "0.1.0"
            source = "You shall not parse"
        "#,
        )
        .build();

    assert_that(
        p.cargo("build").arg("--verbose"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse lock file at: [..]

Caused by:
  invalid source `You shall not parse` for key `package.source`
",
        ),
    );
}

#[test]
fn bad_dependency_in_lockfile() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
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
        "#,
        )
        .build();

    assert_that(
        p.cargo("build").arg("--verbose"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse lock file at: [..]

Caused by:
  package `bar 0.1.0 ([..])` is specified as a dependency, but is missing from the package list
",
        ),
    );
}

#[test]
fn bad_git_dependency() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies]
            foo = { git = "file:.." }
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").arg("-v"),
        execs().with_status(101).with_stderr(
            "\
[UPDATING] git repository `file:///`
[ERROR] failed to load source for a dependency on `foo`

Caused by:
  Unable to update file:///

Caused by:
  failed to clone into: [..]

Caused by:
  [..]'file:///' is not a valid local file URI[..]
",
        ),
    );
}

#[test]
fn bad_crate_type() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [lib]
            crate-type = ["bad_type", "rlib"]
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").arg("-v"),
        execs().with_status(101).with_stderr_contains(
            "error: failed to run `rustc` to learn about crate-type bad_type information",
        ),
    );
}

#[test]
fn malformed_override() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [target.x86_64-apple-darwin.freetype]
            native = {
              foo: "bar"
            }
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  expected a table key, found a newline at line 8
",
        ),
    );
}

#[test]
fn duplicate_binary_names() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
           [package]
           name = "qqq"
           version = "0.1.0"
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

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  found duplicate binary name e, but all binary targets must have a unique name
",
        ),
    );
}

#[test]
fn duplicate_example_names() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
           [package]
           name = "qqq"
           version = "0.1.0"
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

    assert_that(
        p.cargo("build").arg("--example").arg("ex"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  found duplicate example name ex, but all example targets must have a unique name
",
        ),
    );
}

#[test]
fn duplicate_bench_names() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
           [package]
           name = "qqq"
           version = "0.1.0"
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

    assert_that(
        p.cargo("bench"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  found duplicate bench name ex, but all bench targets must have a unique name
",
        ),
    );
}

#[test]
fn duplicate_deps() {
    let p = project("foo")
        .file(
            "shim-bar/Cargo.toml",
            r#"
           [package]
           name = "bar"
           version = "0.0.1"
           authors = []
        "#,
        )
        .file(
            "shim-bar/src/lib.rs",
            r#"
                pub fn a() {}
        "#,
        )
        .file(
            "linux-bar/Cargo.toml",
            r#"
           [package]
           name = "bar"
           version = "0.0.1"
           authors = []
        "#,
        )
        .file(
            "linux-bar/src/lib.rs",
            r#"
                pub fn a() {}
        "#,
        )
        .file(
            "Cargo.toml",
            r#"
           [package]
           name = "qqq"
           version = "0.0.1"
           authors = []

           [dependencies]
           bar = { path = "shim-bar" }

           [target.x86_64-unknown-linux-gnu.dependencies]
           bar = { path = "linux-bar" }
        "#,
        )
        .file("src/main.rs", r#"fn main () {}"#)
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Dependency 'bar' has different source paths depending on the build target. Each dependency must \
have a single canonical source path irrespective of build target.
",
        ),
    );
}

#[test]
fn duplicate_deps_diff_sources() {
    let p = project("foo")
        .file(
            "shim-bar/Cargo.toml",
            r#"
           [package]
           name = "bar"
           version = "0.0.1"
           authors = []
        "#,
        )
        .file(
            "shim-bar/src/lib.rs",
            r#"
                pub fn a() {}
        "#,
        )
        .file(
            "linux-bar/Cargo.toml",
            r#"
           [package]
           name = "bar"
           version = "0.0.1"
           authors = []
        "#,
        )
        .file(
            "linux-bar/src/lib.rs",
            r#"
                pub fn a() {}
        "#,
        )
        .file(
            "Cargo.toml",
            r#"
           [package]
           name = "qqq"
           version = "0.0.1"
           authors = []

           [target.i686-unknown-linux-gnu.dependencies]
           bar = { path = "shim-bar" }

           [target.x86_64-unknown-linux-gnu.dependencies]
           bar = { path = "linux-bar" }
        "#,
        )
        .file("src/main.rs", r#"fn main () {}"#)
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Dependency 'bar' has different source paths depending on the build target. Each dependency must \
have a single canonical source path irrespective of build target.
",
        ),
    );
}

#[test]
fn unused_keys() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
           [package]
           name = "foo"
           version = "0.1.0"
           authors = []

           [target.foo]
           bar = "3"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(
            "\
warning: unused manifest key: target.foo.bar
[COMPILING] foo v0.1.0 (file:///[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );

    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            bulid = "foo"
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() {}
        "#,
        )
        .build();
    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(
            "\
warning: unused manifest key: project.bulid
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );

    let p = project("bar")
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]
            build = "foo"
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() {}
        "#,
        )
        .build();
    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(
            "\
warning: unused manifest key: lib.build
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn empty_dependencies() {
    let p = project("empty_deps")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "empty_deps"
            version = "0.0.0"
            authors = []

            [dependencies]
            foo = {}
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "0.0.1").publish();

    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr_contains(
            "\
warning: dependency (foo) specified without providing a local path, Git repository, or version \
to use. This will be considered an error in future versions
",
        ),
    );
}

#[test]
fn invalid_toml_historically_allowed_is_warned() {
    let p = project("empty_deps")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "empty_deps"
            version = "0.0.0"
            authors = []
        "#,
        )
        .file(
            ".cargo/config",
            r#"
            [foo] bar = 2
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(
            "\
warning: TOML file found which contains invalid syntax and will soon not parse
at `[..]config`.

The TOML spec requires newlines after table definitions (e.g. `[a] b = 1` is
invalid), but this file has a table header which does not have a newline after
it. A newline needs to be added and this warning will soon become a hard error
in the future.
[COMPILING] empty_deps v0.0.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn ambiguous_git_reference() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies.bar]
            git = "https://127.0.0.1"
            branch = "master"
            tag = "some-tag"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").arg("-v"),
        execs().with_stderr_contains(
            "\
[WARNING] dependency (bar) specification is ambiguous. \
Only one of `branch`, `tag` or `rev` is allowed. \
This will be considered an error in future versions
",
        ),
    );
}

#[test]
fn bad_source_config1() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [source.foo]
        "#,
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(101)
            .with_stderr("error: no source URL specified for `source.foo`, need [..]"),
    );
}

#[test]
fn bad_source_config2() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies]
            bar = "*"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [source.crates-io]
            registry = 'http://example.com'
            replace-with = 'bar'
        "#,
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
error: failed to load source for a dependency on `bar`

Caused by:
  Unable to update registry `https://[..]`

Caused by:
  could not find a configured source with the name `bar` \
    when attempting to lookup `crates-io` (configuration in [..])
",
        ),
    );
}

#[test]
fn bad_source_config3() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies]
            bar = "*"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [source.crates-io]
            registry = 'http://example.com'
            replace-with = 'crates-io'
        "#,
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
error: failed to load source for a dependency on `bar`

Caused by:
  Unable to update registry `https://[..]`

Caused by:
  detected a cycle of `replace-with` sources, [..]
",
        ),
    );
}

#[test]
fn bad_source_config4() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies]
            bar = "*"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [source.crates-io]
            registry = 'http://example.com'
            replace-with = 'bar'

            [source.bar]
            registry = 'http://example.com'
            replace-with = 'crates-io'
        "#,
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
error: failed to load source for a dependency on `bar`

Caused by:
  Unable to update registry `https://[..]`

Caused by:
  detected a cycle of `replace-with` sources, the source `crates-io` is \
    eventually replaced with itself (configuration in [..])
",
        ),
    );
}

#[test]
fn bad_source_config5() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies]
            bar = "*"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [source.crates-io]
            registry = 'http://example.com'
            replace-with = 'bar'

            [source.bar]
            registry = 'not a url'
        "#,
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
error: configuration key `source.bar.registry` specified an invalid URL (in [..])

Caused by:
  invalid url `not a url`: [..]
",
        ),
    );
}

#[test]
fn both_git_and_path_specified() {
    let foo = project("foo")
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []

        [dependencies.bar]
        git = "https://127.0.0.1"
        path = "bar"
    "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        foo.cargo("build").arg("-v"),
        execs().with_stderr_contains(
            "\
[WARNING] dependency (bar) specification is ambiguous. \
Only one of `git` or `path` is allowed. \
This will be considered an error in future versions
",
        ),
    );
}

#[test]
fn bad_source_config6() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies]
            bar = "*"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [source.crates-io]
            registry = 'http://example.com'
            replace-with = ['not', 'a', 'string']
        "#,
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "error: expected a string, but found a array for `source.crates-io.replace-with` in [..]",
        ),
    );
}

#[test]
fn ignored_git_revision() {
    let foo = project("foo")
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []

        [dependencies.bar]
        path = "bar"
        branch = "spam"
    "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        foo.cargo("build").arg("-v"),
        execs().with_stderr_contains(
            "\
             [WARNING] key `branch` is ignored for dependency (bar). \
             This will be considered an error in future versions",
        ),
    );
}

#[test]
fn bad_source_config7() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies]
            bar = "*"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [source.foo]
            registry = 'http://example.com'
            local-registry = 'file:///another/file'
        "#,
        )
        .build();

    Package::new("bar", "0.1.0").publish();

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(101)
            .with_stderr("error: more than one source URL specified for `source.foo`"),
    );
}

#[test]
fn bad_dependency() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies]
            bar = 3
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  invalid type: integer `3`, expected a version string like [..]
",
        ),
    );
}

#[test]
fn bad_debuginfo() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [profile.dev]
            debug = 'a'
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  invalid type: string \"a\", expected a boolean or an integer for [..]
",
        ),
    );
}

#[test]
fn bad_opt_level() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = 3
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  invalid type: integer `3`, expected a boolean or a string for key [..]
",
        ),
    );
}
