extern crate cargotest;
extern crate hamcrest;

use cargotest::support::{project, execs};
use cargotest::support::registry::Package;
use hamcrest::assert_that;

#[test]
fn bad1() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
              [target]
              nonexistent-target = "foo"
        "#);
    assert_that(foo.cargo_process("build").arg("-v")
                   .arg("--target=nonexistent-target"),
                execs().with_status(101).with_stderr("\
[ERROR] expected table for configuration key `target.nonexistent-target`, \
but found string in [..]config
"));
}

#[test]
fn bad2() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
              [http]
                proxy = 3.0
        "#);
    assert_that(foo.cargo_process("publish").arg("-v"),
                execs().with_status(101).with_stderr("\
[ERROR] Couldn't load Cargo configuration

Caused by:
  failed to load TOML configuration from `[..]config`

Caused by:
  failed to parse key `http`

Caused by:
  failed to parse key `proxy`

Caused by:
  found TOML configuration value of unknown type `float`
"));
}

#[test]
fn bad3() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
            [http]
              proxy = true
        "#);
    Package::new("foo", "1.0.0").publish();
    assert_that(foo.cargo_process("publish").arg("-v"),
                execs().with_status(101).with_stderr("\
[UPDATING] registry `[..]`
[ERROR] invalid configuration for key `http.proxy`
expected a string, but found a boolean in [..]config
"));
}

#[test]
fn bad4() {
    let foo = project("foo")
        .file(".cargo/config", r#"
            [cargo-new]
              name = false
        "#);
    assert_that(foo.cargo_process("new").arg("-v").arg("foo"),
                execs().with_status(101).with_stderr("\
[ERROR] Failed to create project `foo` at `[..]`

Caused by:
  invalid configuration for key `cargo-new.name`
expected a string, but found a boolean in [..]config
"));
}

#[test]
fn bad5() {
    let foo = project("foo")
        .file(".cargo/config", r#"
            foo = ""
        "#)
        .file("foo/.cargo/config", r#"
            foo = 2
        "#);
    foo.build();
    assert_that(foo.cargo("new")
                   .arg("-v").arg("foo").cwd(&foo.root().join("foo")),
                execs().with_status(101).with_stderr("\
[ERROR] Failed to create project `foo` at `[..]`

Caused by:
  Couldn't load Cargo configuration

Caused by:
  failed to merge key `foo` between files:
  file 1: [..]foo[..]foo[..]config
  file 2: [..]foo[..]config

Caused by:
  expected integer, but found string
"));
}

#[test]
fn bad_cargo_config_jobs() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []
    "#)
    .file("src/lib.rs", "")
    .file(".cargo/config", r#"
        [build]
        jobs = -1
    "#);
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
[ERROR] build.jobs must be positive, but found -1 in [..]
"));
}

#[test]
fn default_cargo_config_jobs() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []
    "#)
    .file("src/lib.rs", "")
    .file(".cargo/config", r#"
        [build]
        jobs = 1
    "#);
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(0));
}

#[test]
fn good_cargo_config_jobs() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []
    "#)
    .file("src/lib.rs", "")
    .file(".cargo/config", r#"
        [build]
        jobs = 4
    "#);
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(0));
}

#[test]
fn invalid_global_config() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []

        [dependencies]
        foo = "0.1.0"
    "#)
    .file(".cargo/config", "4")
    .file("src/lib.rs", "");

    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
[ERROR] Couldn't load Cargo configuration

Caused by:
  could not parse TOML configuration in `[..]config`

Caused by:
  could not parse input as TOML
[..]config:1:2 expected `=`, but found eof

"));
}

#[test]
fn bad_cargo_lock() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []
    "#)
    .file("Cargo.lock", "")
    .file("src/lib.rs", "");

    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse lock file at: [..]Cargo.lock

Caused by:
  expected a section for the key `root`
"));
}

#[test]
fn bad_git_dependency() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []

        [dependencies]
        foo = { git = "file:.." }
    "#)
    .file("src/lib.rs", "");

    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
[UPDATING] git repository `file:///`
[ERROR] failed to load source for a dependency on `foo`

Caused by:
  Unable to update file:///

Caused by:
  failed to clone into: [..]

Caused by:
  [[..]] 'file:///' is not a valid local file URI
"));
}

#[test]
fn bad_crate_type() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []

        [lib]
        crate-type = ["bad_type", "rlib"]
    "#)
    .file("src/lib.rs", "");

    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr_contains("\
error: failed to run `rustc` to learn about target-specific information
"));
}

#[test]
fn malformed_override() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []

        [target.x86_64-apple-darwin.freetype]
        native = {
          foo: "bar"
        }
    "#)
    .file("src/lib.rs", "");

    assert_that(foo.cargo_process("build"),
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML
Cargo.toml:[..]

"));
}

#[test]
fn duplicate_binary_names() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
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
    "#)
    .file("a.rs", r#"fn main() -> () {}"#)
    .file("b.rs", r#"fn main() -> () {}"#);

    assert_that(foo.cargo_process("build"),
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  found duplicate binary name e, but all binary targets must have a unique name
"));
}

#[test]
fn duplicate_example_names() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
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
    "#)
    .file("examples/ex.rs", r#"fn main () -> () {}"#)
    .file("examples/ex2.rs", r#"fn main () -> () {}"#);

    assert_that(foo.cargo_process("build").arg("--example").arg("ex"),
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  found duplicate example name ex, but all binary targets must have a unique name
"));
}

#[test]
fn duplicate_bench_names() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
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
    "#)
    .file("benches/ex.rs", r#"fn main () {}"#)
    .file("benches/ex2.rs", r#"fn main () {}"#);

    assert_that(foo.cargo_process("bench"),
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  found duplicate bench name ex, but all binary targets must have a unique name
"));
}

#[test]
fn duplicate_deps() {
    let foo = project("foo")
    .file("shim-bar/Cargo.toml", r#"
       [package]
       name = "bar"
       version = "0.0.1"
       authors = []
    "#)
    .file("shim-bar/src/lib.rs", r#"
            pub fn a() {}
    "#)
    .file("linux-bar/Cargo.toml", r#"
       [package]
       name = "bar"
       version = "0.0.1"
       authors = []
    "#)
    .file("linux-bar/src/lib.rs", r#"
            pub fn a() {}
    "#)
    .file("Cargo.toml", r#"
       [package]
       name = "qqq"
       version = "0.0.1"
       authors = []

       [dependencies]
       bar = { path = "shim-bar" }

       [target.x86_64-unknown-linux-gnu.dependencies]
       bar = { path = "linux-bar" }
    "#)
    .file("src/main.rs", r#"fn main () {}"#);

    assert_that(foo.cargo_process("build"),
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  found duplicate dependency name bar, but all dependencies must have a unique name
"));
}

#[test]
fn unused_keys() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
           [package]
           name = "foo"
           version = "0.1.0"
           authors = []

           [target.foo]
           bar = "3"
        "#)
        .file("src/lib.rs", "");

    assert_that(foo.cargo_process("build"),
                execs().with_status(0).with_stderr("\
warning: unused manifest key: target.foo.bar
[COMPILING] foo v0.1.0 (file:///[..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn empty_dependencies() {
    let p = project("empty_deps")
    .file("Cargo.toml", r#"
        [package]
        name = "empty_deps"
        version = "0.0.0"
        authors = []

        [dependencies]
        foo = {}
    "#)
    .file("src/main.rs", "fn main() {}");

    Package::new("foo", "0.0.1").publish();

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr_contains("\
warning: dependency (foo) specified without providing a local path, Git repository, or version \
to use. This will be considered an error in future versions
"));
}

#[test]
fn invalid_toml_historically_allowed_is_warned() {
    let p = project("empty_deps")
    .file("Cargo.toml", r#"
        [package]
        name = "empty_deps"
        version = "0.0.0"
        authors = []
    "#)
    .file(".cargo/config", r#"
        [foo] bar = 2
    "#)
    .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
warning: TOML file found which contains invalid syntax and will soon not parse
at `[..]config`.

The TOML spec requires newlines after table definitions (e.g. `[a] b = 1` is
invalid), but this file has a table header which does not have a newline after
it. A newline needs to be added and this warning will soon become a hard error
in the future.
[COMPILING] empty_deps v0.0.0 ([..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));
}
