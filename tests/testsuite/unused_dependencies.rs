//! A test suite for `-Zwarn-unused-dependencies`
//!
//! All tests here should use `#[cargo_test(unused_dependencies)]` to indicate that
//! boilerplate should be generated to require the nightly toolchain.
//! Otherwise the tests are skipped.
//!
//! In order to debug a test, you can add an env var like:
//! .env("CARGO_LOG", "cargo::core::compiler::unused_dependencies=trace")

use cargo_test_support::project;
use cargo_test_support::registry::Package;

// TODO more commands for the tests to test the allowed kinds logic
// TODO document the tests

#[cargo_test(unused_dependencies)]
fn unused_proper_dep() {
    // The most basic case where there is an unused dependency
    Package::new("bar", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            bar = "0.1.0"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {}
            "#,
        )
        .build();

    p.cargo("build -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo [..]
[WARNING] unused dependency bar in package foo v0.1.0
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
}

#[cargo_test(unused_dependencies)]
fn unused_build_dep() {
    // A build dependency is unused
    Package::new("bar", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [build-dependencies]
            bar = "0.1.0"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {}
            "#,
        )
        .file(
            "build.rs",
            r#"
            fn main() {}
            "#,
        )
        .build();

    p.cargo("build -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo [..]
[WARNING] unused build-dependency bar in package foo v0.1.0
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
}

#[cargo_test(unused_dependencies)]
fn unused_deps_multiple() {
    // Multiple dependencies are unused,
    // also test that re-using dependencies
    // between proper and build deps doesn't
    // confuse the lint
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();
    Package::new("qux", "0.1.0").publish();
    Package::new("quux", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            bar = "0.1.0"
            baz = "0.1.0"
            qux = "0.1.0"
            quux = "0.1.0"

            [build-dependencies]
            bar = "0.1.0"
            baz = "0.1.0"
            qux = "0.1.0"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            use qux as _;
            fn main() {}
            "#,
        )
        .file(
            "build.rs",
            r#"
            use baz as _;
            fn main() {}
            "#,
        )
        .build();

    p.cargo("build -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] qux v0.1.0 [..]
[DOWNLOADED] quux v0.1.0 [..]
[DOWNLOADED] baz v0.1.0 [..]
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] foo [..]
[WARNING] unused dependency bar in package foo v0.1.0
[WARNING] unused dependency baz in package foo v0.1.0
[WARNING] unused dependency quux in package foo v0.1.0
[WARNING] unused build-dependency bar in package foo v0.1.0
[WARNING] unused build-dependency qux in package foo v0.1.0
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
}

#[cargo_test(unused_dependencies)]
fn unused_build_dep_used_proper() {
    // Check sharingof a dependency
    // between build and proper deps
    Package::new("bar", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            bar = "0.1.0"

            [build-dependencies]
            bar = "0.1.0"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            use bar as _;
            fn main() {}
            "#,
        )
        .file(
            "build.rs",
            r#"
            fn main() {}
            "#,
        )
        .build();

    p.cargo("build -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo [..]
[WARNING] unused build-dependency bar in package foo v0.1.0
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
}

#[cargo_test(unused_dependencies)]
fn unused_dep_renamed() {
    // Make sure that package renaming works
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.2.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            baz = { package = "bar", version = "0.1.0" }
            bar = { package = "baz", version = "0.2.0" }
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            use bar as _;
            fn main() {}
            "#,
        )
        .build();

    p.cargo("build -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] baz v0.2.0 [..]
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] foo [..]
[WARNING] unused dependency baz in package foo v0.1.0
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
}

#[cargo_test(unused_dependencies)]
fn unused_proper_dep_allowed() {
    // There is an unused dependency but it's marked as
    // allowed due to the leading underscore
    Package::new("bar", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            _bar = { package = "bar", version = "0.1.0" }
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {}
            "#,
        )
        .build();

    p.cargo("build -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
}

#[cargo_test(unused_dependencies)]
fn unused_dep_lib_bin() {
    // Make sure that dependency uses by both binaries and libraries
    // are being registered as uses.
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();
    Package::new("qux", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            bar = "0.1.0"
            baz = "0.1.0"
            qux = "0.1.0"
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            use baz as _;
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            use bar as _;
            fn main() {}
            "#,
        )
        .build();

    p.cargo("build -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] qux v0.1.0 [..]
[DOWNLOADED] baz v0.1.0 [..]
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] foo [..]
[WARNING] unused dependency qux in package foo v0.1.0
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
}

#[cargo_test(unused_dependencies)]
fn should_be_dev() {
    // Test the warning that a dependency should be a dev dep.
    // Sometimes, a cargo command doesn't compile the dev unit
    // that would use the dependency and thus will claim that
    // the dependency is unused while it actually is used.
    // However, this behaviour is common in unused lints:
    // e.g. when you cfg-gate a public function foo that uses
    // a function bar, the bar function will be marked as
    // unused even though there is a mode that uses bar.
    //
    // So the "should be dev" lint should be seen as a
    // best-effort improvement over the "unused dep" lint.
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();
    Package::new("qux", "0.1.0").publish();
    Package::new("quux", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            bar = "0.1.0"
            baz = "0.1.0"
            qux = "0.1.0"
            quux = "0.1.0" # only genuinely unused dep
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "tests/hello.rs",
            r#"
            use bar as _;
            "#,
        )
        .file(
            "examples/hello.rs",
            r#"
            use baz as _;
            fn main() {}
            "#,
        )
        .file(
            "benches/hello.rs",
            r#"
            use qux as _;
            fn main() {}
            "#,
        )
        .build();

    #[rustfmt::skip]
/*
    // Currently disabled because of a bug: test --no-run witholds unused dep warnings
    // for doctests that never happen
    p.cargo("test --no-run -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] qux v0.1.0 [..]
[DOWNLOADED] quux v0.1.0 [..]
[DOWNLOADED] baz v0.1.0 [..]
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] foo [..]
[WARNING] dependency bar in package foo v0.1.0 is only used by dev targets
[WARNING] dependency baz in package foo v0.1.0 is only used by dev targets
[WARNING] unused dependency quux in package foo v0.1.0
[WARNING] unused dependency qux in package foo v0.1.0
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
*/
    p.cargo("test --no-run -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] qux v0.1.0 [..]
[DOWNLOADED] quux v0.1.0 [..]
[DOWNLOADED] baz v0.1.0 [..]
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] foo [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();

    p.cargo("test --no-run --all-targets -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo [..]
[WARNING] dependency bar in package foo v0.1.0 is only used by dev targets
[WARNING] dependency baz in package foo v0.1.0 is only used by dev targets
[WARNING] unused dependency quux in package foo v0.1.0
[WARNING] dependency qux in package foo v0.1.0 is only used by dev targets
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();

    p.cargo("build -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[WARNING] unused dependency bar in package foo v0.1.0
[WARNING] unused dependency baz in package foo v0.1.0
[WARNING] unused dependency quux in package foo v0.1.0
[WARNING] unused dependency qux in package foo v0.1.0
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();

    p.cargo("check --all-targets -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] [..]
[CHECKING] [..]
[CHECKING] [..]
[CHECKING] [..]
[CHECKING] foo [..]
[WARNING] dependency bar in package foo v0.1.0 is only used by dev targets
[WARNING] dependency baz in package foo v0.1.0 is only used by dev targets
[WARNING] unused dependency quux in package foo v0.1.0
[WARNING] dependency qux in package foo v0.1.0 is only used by dev targets
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
}

#[cargo_test(unused_dependencies)]
fn dev_deps() {
    // Test for unused dev dependencies
    // In this instance,
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();
    Package::new("baz2", "0.1.0").publish();
    Package::new("qux", "0.1.0").publish();
    Package::new("quux", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dev-dependencies]
            bar = "0.1.0"
            baz = "0.1.0"
            baz2 = "0.1.0"
            qux = "0.1.0"
            quux = "0.1.0" # only genuinely unused dep
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            /// ```
            /// use baz2 as _; extern crate baz2;
            /// ```
            pub fn foo() {}
        "#,
        )
        .file(
            "tests/hello.rs",
            r#"
            use bar as _;
            "#,
        )
        .file(
            "examples/hello.rs",
            r#"
            use baz as _;
            fn main() {}
            "#,
        )
        .file(
            "benches/hello.rs",
            r#"
            use qux as _;
            fn main() {}
            "#,
        )
        .build();

    // cargo test --no-run doesn't test doctests and benches
    // and thus doesn't create unused dev dep warnings
    p.cargo("test --no-run -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] qux v0.1.0 [..]
[DOWNLOADED] quux v0.1.0 [..]
[DOWNLOADED] baz2 v0.1.0 [..]
[DOWNLOADED] baz v0.1.0 [..]
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] foo [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();

    // cargo test --no-run --all-targets
    // doesn't test doctests, still no unused dev dep warnings
    p.cargo("test --no-run --all-targets -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();

    // cargo test tests
    // everything including doctests, but not
    // the benches
    p.cargo("test -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..]
[RUNNING] [..]
[..]
[WARNING] unused dev-dependency quux in package foo v0.1.0
[WARNING] unused dev-dependency qux in package foo v0.1.0\
            ",
        )
        .run();

    // Check that cargo build doesn't check for unused dev-deps
    p.cargo("build -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();

    // cargo check --all-targets doesn't check for unused dev-deps (no doctests ran)
    p.cargo("check --all-targets -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] [..]
[CHECKING] [..]
[CHECKING] [..]
[CHECKING] [..]
[CHECKING] [..]
[CHECKING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
}

#[cargo_test(unused_dependencies)]
fn cfg_test_used() {
    // Ensure that a dependency only used from #[cfg(test)] code
    // is still registered as used.

    // TODO: this test currently doesn't actually test that bar is used
    // because the warning is witheld, waiting for doctests that never happen.
    // It's due to a bug in cargo test --no-run
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dev-dependencies]
            bar = "0.1.0"
            #baz = "0.1.0"
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #[cfg(test)]
            mod tests {
                use bar as _;
            }
            "#,
        )
        .build();

    p.cargo("test --no-run -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] [..]
[COMPILING] [..]
[COMPILING] foo [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
}

#[cargo_test(unused_dependencies)]
fn cfg_test_workspace() {
    // Make sure that workspaces are supported,
    // --all params, -p params, etc.
    Package::new("baz", "0.1.0").publish();
    Package::new("qux", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            bar = { path = "bar" }
            baz = "0.1.0"

            [workspace]
            members = ["bar"]
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            use bar as _;
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            baz = "0.1.0"
            qux = "0.1.0"
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
            use baz as _;
            "#,
        )
        .build();

    p.cargo("check --all -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] [..]
[DOWNLOADED] [..]
[CHECKING] [..]
[CHECKING] [..]
[CHECKING] bar [..]
[CHECKING] foo [..]
[WARNING] unused dependency qux in package bar v0.1.0
[WARNING] unused dependency baz in package foo v0.1.0
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();

    p.cargo("check -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[WARNING] unused dependency baz in package foo v0.1.0
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();

    p.cargo("check -p bar -Zwarn-unused-deps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[WARNING] unused dependency qux in package bar v0.1.0
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\
            ",
        )
        .run();
}
