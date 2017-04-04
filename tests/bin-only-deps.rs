extern crate cargo;
extern crate cargotest;
extern crate hamcrest;
extern crate tempdir;

use std::env;
use std::fs::{self, File};
use std::io::prelude::*;

use cargo::util::process;
use cargotest::{is_nightly, rustc_host, sleep_ms};
use cargotest::support::paths::{CargoPathExt,root};
use cargotest::support::{ProjectBuilder};
use cargotest::support::{project, execs, main_file, basic_bin_manifest};
use cargotest::support::registry::Package;
use hamcrest::{assert_that, existing_file, is_not};
use tempdir::TempDir;

#[test]
fn can_use_bin_deps() {
    // Check that the basic functionality works as expected

    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[[bin]]
name = "binary"

[bin.dependencies]
testdep = { path = "testdep" }
"#)
        .file("src/lib.rs", "pub fn bla() {}")
        .file("src/bin/binary.rs", r#"
extern crate foo;
extern crate testdep;

fn main() {
    foo::bla();
    testdep::bar();
}"#)
        .file("testdep/Cargo.toml", r#"
[package]
name = "testdep"
version = "0.5.0"
authors = ["wycats@example.com"]
        "#)
        .file("testdep/src/lib.rs", "pub fn bar() {}");

    assert_that(p.cargo_process("build"), execs()
        .with_status(0)
        .with_stderr_contains("[COMPILING] testdep v0.5.0 ([..])"));
}

#[test]
fn can_use_long_bin_deps() {
    // Long dependency specifications

    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[[bin]]
name = "binary"

[bin.dependencies.testdep]
path = "testdep"
version = "=0.5.0"
"#)
        .file("src/lib.rs", "pub fn bla() {}")
        .file("src/bin/binary.rs", r#"
extern crate testdep;
fn main() {
    testdep::bar();
}
"#)
        .file("testdep/Cargo.toml", r#"
[package]
name = "testdep"
version = "0.5.0"
authors = ["wycats@example.com"]
        "#)
        .file("testdep/src/lib.rs", "pub fn bar() {}");

    assert_that(p.cargo_process("build").arg("--bin").arg("binary"), execs()
        .with_status(0)
        .with_stderr_contains("[COMPILING] testdep v0.5.0 ([..])"));
}

#[test]
fn multiple_bins_different_deps() {
    // Check that multiple bins with different dependencies work as expected

    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[[bin]]
name = "bin1"

[bin.dependencies]
dep1 = { path = "dep1" }

[[bin]]
name = "bin2"

[bin.dependencies]
dep2 = { path = "dep2" }
"#)
        .file("src/lib.rs", "pub fn bla() {}")
        .file("src/bin/bin1.rs", r#"
extern crate foo;
extern crate dep1;

fn main() {
    foo::bla();
    dep1::bar1();
}"#)
        .file("src/bin/bin2.rs", r#"
extern crate foo;
extern crate dep2;

fn main() {
    foo::bla();
    dep2::bar2();
}"#)
        .file("dep1/Cargo.toml", r#"
[package]
name = "dep1"
version = "0.5.0"
authors = ["wycats@example.com"]
        "#)
        .file("dep1/src/lib.rs", "pub fn bar1() {}")
        .file("dep2/Cargo.toml", r#"
[package]
name = "dep2"
version = "0.2.0"
authors = ["wycats@example.com"]
        "#)
        .file("dep2/src/lib.rs", "pub fn bar2() {}");

    p.build();

    assert_that(p.cargo("build"), execs()
        .with_status(0)
        .with_stderr_contains("[COMPILING] dep1 v0.5.0 ([..])")
        .with_stderr_contains("[COMPILING] dep2 v0.2.0 ([..])"));

    assert_that(p.cargo("clean"), execs()
        .with_status(0));

    assert_that(p.cargo("build").arg("--bin").arg("bin1"), execs()
        .with_status(0)
        .with_stderr_contains("[COMPILING] dep1 v0.5.0 ([..])")
        .with_stderr_does_not_contain("[COMPILING] dep2 v0.2.0 ([..])"));

    assert_that(p.cargo("build").arg("--bin").arg("bin2"), execs()
        .with_status(0)
        .with_stderr_contains("[COMPILING] dep2 v0.2.0 ([..])")
        .with_stderr_does_not_contain("[COMPILING] dep1 v0.5.0 ([..])"));
}

#[test]
fn multiple_bins_common_deps() {
    // Check that multiple bins with common dependencies work as expected

    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[[bin]]
name = "bin1"

[bin.dependencies]
testdep = { path = "testdep" }

[[bin]]
name = "bin2"

[bin.dependencies]
testdep = { path = "testdep" }
"#)
        .file("src/lib.rs", "pub fn bla() {}")
        .file("src/bin/bin1.rs", r#"
extern crate foo;
extern crate testdep;

fn main() {
    foo::bla();
    testdep::bar();
}"#)
        .file("src/bin/bin2.rs", r#"
extern crate foo;
extern crate testdep;

fn main() {
    foo::bla();
    testdep::bar();
}"#)
        .file("testdep/Cargo.toml", r#"
[package]
name = "testdep"
version = "0.5.0"
authors = ["wycats@example.com"]
        "#)
        .file("testdep/src/lib.rs", "pub fn bar() {}");
    p.build();

    assert_that(p.cargo("build"), execs()
        .with_status(0)
        .with_stderr_contains("[COMPILING] testdep v0.5.0 ([..])"));

    assert_that(p.cargo("clean"), execs()
        .with_status(0));

    assert_that(p.cargo("build").arg("--bin").arg("bin1"), execs()
        .with_status(0)
        .with_stderr_contains("[COMPILING] testdep v0.5.0 ([..])"));

    assert_that(p.cargo("build").arg("--bin").arg("bin2"), execs()
        .with_status(0)
        .with_stderr_does_not_contain("[COMPILING] testdep v0.5.0 ([..])"));
}

#[test]
fn denies_deps_on_examples() {
    // We only want binary-specific dependencies for now

    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[[example]]
name = "ex1"

[example.dependencies]
libc = "*"
"#)
        .file("src/lib.rs", "pub fn bla() {}")
        .file("examples/ex1.rs", "fn main() {}");

    assert_that(p.cargo_process("build"), execs()
        .with_status(101)
        .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Target-specific dependencies are only supported for binary targets.
"));
}

#[test]
fn denies_deps_on_tests() {
    // We only want binary-specific dependencies for now

    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[[test]]
name = "test1"

[test.dependencies]
libc = "*"
"#)
        .file("src/lib.rs", "pub fn bla() {}")
        .file("tests/test1.rs", "#[test] fn stuff() {}");

    assert_that(p.cargo_process("build"), execs()
        .with_status(101)
        .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Target-specific dependencies are only supported for binary targets.
"));
}

#[test]
fn denies_deps_on_benches() {
    // We only want binary-specific dependencies for now

    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[[bench]]
name = "bench1"

[bench.dependencies]
libc = "*"
"#)
        .file("src/lib.rs", "pub fn bla() {}")
        .file("benches/bench1.rs", "#[test] fn stuff() {}");

    assert_that(p.cargo_process("build"), execs()
        .with_status(101)
        .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Target-specific dependencies are only supported for binary targets.
"));
}

#[test]
fn doesnt_add_dep_for_lib() {
    // Make sure the package's lib target doesn't require the dependency to be built

    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[[bin]]
name = "binary"

[bin.dependencies]
testdep = { path = "testdep" }
"#)
        .file("src/lib.rs", "pub fn bla() {}")
        .file("src/bin/binary.rs", "fn main() {}")
        .file("testdep/Cargo.toml", r#"
[package]
name = "testdep"
version = "0.5.0"
authors = ["wycats@example.com"]
"#)
        .file("testdep/src/lib.rs", "pub fn bar() {}");

    assert_that(p.cargo_process("build").arg("--lib"), execs()
        .with_status(0)
        .with_stderr("\
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn doesnt_add_downstream_deps() {
    // Make sure dependent packages don't depend on bin-only deps

    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[dependencies]
library = { path = "library" }
"#)
        .file("src/lib.rs", r#"
extern crate library;
pub fn bla() {
    library::foo();
}
"#)
        .file("library/src/bin/binary.rs", "fn main() {}")
        .file("library/Cargo.toml", r#"
[package]
name = "library"
version = "0.5.0"
authors = ["wycats@example.com"]

[[bin]]
name = "binary"

[bin.dependencies]
testdep = { path = "testdep" }
"#)
        .file("library/src/lib.rs", "pub fn foo() {}")
        .file("library/testdep/Cargo.toml", r#"
[package]
name = "testdep"
version = "0.5.0"
authors = ["wycats@example.com"]
"#)
        .file("library/testdep/src/lib.rs", "pub fn bar() {}");
    p.build();

    // Just building the libs shouldn't build testdep
    assert_that(p.cargo("build"), execs()
        .with_status(0)
        .with_stderr_does_not_contain("testdep"));

    // Building `binary` should build testdep
    assert_that(p.cargo("build").arg("-p").arg("library").arg("--bin").arg("binary"), execs()
        .with_status(0)
        .with_stderr_contains("[COMPILING] testdep v0.5.0 ([..])"));
}

#[test]
fn optional_deps() {
    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[[bin]]
name = "binary"

[bin.dependencies]
testdep = { path = "testdep", optional = true }
"#)
        .file("src/lib.rs", "pub fn bla() {}")
        .file("src/bin/binary.rs", "fn main() {}")
        .file("testdep/Cargo.toml", r#"
[package]
name = "testdep"
version = "0.5.0"
authors = ["wycats@example.com"]
"#)
        .file("testdep/src/lib.rs", "pub fn bar() {}");
    p.build();

    // Don't compile optional dep
    assert_that(p.cargo("build").arg("--bin").arg("binary"), execs()
        .with_status(0)
        .with_stderr_does_not_contain("testdep"));

    // Enabling optional dep feature builds testdep
    assert_that(p.cargo("build").args(&["--bin", "binary", "--features", "testdep"]),
        execs()
        .with_status(0)
        .with_stderr_contains("[COMPILING] testdep v0.5.0 ([..])"));
}

#[test]
fn optional_dep_collides_with_feature() {
    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[[bin]]
name = "binary"

[bin.dependencies]
testdep = { path = "testdep", optional = true }

[features]
testdep = []
"#)
        .file("src/lib.rs", "pub fn bla() {}")
        .file("src/bin/binary.rs", "fn main() {}")
        .file("testdep/Cargo.toml", r#"
[package]
name = "testdep"
version = "0.5.0"
authors = ["wycats@example.com"]
"#)
        .file("testdep/src/lib.rs", "pub fn bar() {}");

    assert_that(p.cargo_process("build"), execs()
        .with_status(101)
        .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Features and dependencies cannot have the same name: `testdep`
"));
}

#[test]
fn deps_collide() {
    // Having the same dep as the main package is allowed, with the same semantics as duplicate
    // [dev-dependencies] and [build-dependencies] entries.

    let p = project("foo")
        .file("Cargo.toml", r#"
[package]
name = "foo"
version = "0.5.0"
authors = ["wycats@example.com"]

[[bin]]
name = "binary"

[bin.dependencies]
testdep = { path = "testdep" }

[dependencies]
testdep = { path = "testdep" }
"#)
        .file("src/lib.rs", "pub fn bla() {}")
        .file("src/bin/binary.rs", "fn main() {}")
        .file("testdep/Cargo.toml", r#"
[package]
name = "testdep"
version = "0.5.0"
authors = ["wycats@example.com"]
"#)
        .file("testdep/src/lib.rs", "pub fn bar() {}");

    assert_that(p.cargo_process("build"), execs().with_status(0));
}

// TODO: Changes to doc(tests) I'm unsure about (+ add tests)
// TODO: Document this feature
