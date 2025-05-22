//! Tests for checking exactly how profiles correspond with each unit. For
//! example, the `test` profile applying to test targets, but not other
//! targets, etc.

use cargo_test_support::prelude::*;
use cargo_test_support::{basic_manifest, project, str, Project};

fn all_target_project() -> Project {
    // This abuses the `codegen-units` setting so that we can verify exactly
    // which profile is used for each compiler invocation.
    project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = { path = "bar" }

                [build-dependencies]
                bdep = { path = "bdep" }

                [profile.dev]
                codegen-units = 1
                panic = "abort"
                [profile.release]
                codegen-units = 2
                panic = "abort"
                [profile.test]
                codegen-units = 3
                [profile.bench]
                codegen-units = 4
                [profile.dev.build-override]
                codegen-units = 5
                [profile.release.build-override]
                codegen-units = 6
            "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .file("src/main.rs", "extern crate foo; fn main() {}")
        .file("examples/ex1.rs", "extern crate foo; fn main() {}")
        .file("tests/test1.rs", "extern crate foo;")
        .file("benches/bench1.rs", "extern crate foo;")
        .file(
            "build.rs",
            r#"
                extern crate bdep;
                fn main() {
                    eprintln!("foo custom build PROFILE={} DEBUG={} OPT_LEVEL={}",
                        std::env::var("PROFILE").unwrap(),
                        std::env::var("DEBUG").unwrap(),
                        std::env::var("OPT_LEVEL").unwrap(),
                    );
                }
            "#,
        )
        // `bar` package.
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        // `bdep` package.
        .file(
            "bdep/Cargo.toml",
            r#"
                [package]
                name = "bdep"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = { path = "../bar" }
            "#,
        )
        .file("bdep/src/lib.rs", "extern crate bar;")
        .build()
}

#[cargo_test]
fn profile_selection_build() {
    let p = all_target_project();

    // `build`
    // NOTES:
    // - bdep `panic` is not set because it thinks `build.rs` is a plugin.
    // - build_script_build is built without panic because it thinks `build.rs` is a plugin.
    // - We make sure that the build dependencies bar, bdep, and build.rs
    //   are built with debuginfo=0.
    p.cargo("build -vv")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C panic=abort[..]-C codegen-units=1 -C debuginfo=2 [..]
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=5 [..]
[COMPILING] bdep v0.0.1 ([ROOT]/foo/bdep)
[RUNNING] `[..] rustc --crate-name bdep --edition=2015 bdep/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=5 [..]
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name build_script_build --edition=2015 build.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=5 [..]
[RUNNING] `[..][ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[foo 0.0.1] foo custom build PROFILE=debug DEBUG=true OPT_LEVEL=0
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link -C panic=abort[..]-C codegen-units=1 -C debuginfo=2 [..]
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link -C panic=abort[..]-C codegen-units=1 -C debuginfo=2 [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered())
        .with_stderr_does_not_contain("[..] -C debuginfo=0[..]")
        .run();
    p.cargo("build -vv")
        .with_stderr_data(
            str![[r#"
[FRESH] bdep v0.0.1 ([ROOT]/foo/bdep)
[FRESH] bar v0.0.1 ([ROOT]/foo/bar)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn profile_selection_build_release() {
    let p = all_target_project();

    // `build --release`
    p.cargo("build --release -vv")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=6 [..]
[COMPILING] bdep v0.0.1 ([ROOT]/foo/bdep)
[RUNNING] `[..] rustc --crate-name bdep --edition=2015 bdep/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=6 [..]
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name build_script_build --edition=2015 build.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=6 [..]
[RUNNING] `[..][ROOT]/foo/target/release/build/foo-[HASH]/build-script-build`
[foo 0.0.1] foo custom build PROFILE=release DEBUG=false OPT_LEVEL=3
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]].unordered())
        .run();
    p.cargo("build --release -vv")
        .with_stderr_data(
            str![[r#"
[FRESH] bar v0.0.1 ([ROOT]/foo/bar)
[FRESH] bdep v0.0.1 ([ROOT]/foo/bdep)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn profile_selection_build_all_targets() {
    let p = all_target_project();
    // `build`
    // NOTES:
    // - bdep `panic` is not set because it thinks `build.rs` is a plugin.
    // - build_script_build is built without panic because it thinks
    //   `build.rs` is a plugin.
    // - Benchmark dependencies are compiled in `dev` mode, which may be
    //   surprising. See issue rust-lang/cargo#4929.
    // - We make sure that the build dependencies bar, bdep, and build.rs are built with
    //   debuginfo=0; but since we don't pass `-C debuginfo` when it's set to 0, we have to test
    //   explicitly that there's no `-C debuginfo` flag.
    //
    // - Dependency profiles:
    //   Pkg  Target  Profile     Reason
    //   ---  ------  -------     ------
    //   bar  lib     dev         For foo-bin
    //   bar  lib     dev-panic   For tests/benches and bdep
    //   bdep lib     dev-panic   For foo build.rs
    //   foo  custom  dev-panic
    //
    // - `foo` target list is:
    //   Target   Profile    Mode
    //   ------   -------    ----
    //   lib      dev+panic  build  (a normal lib target)
    //   lib      dev-panic  build  (used by tests/benches)
    //   lib      dev        dev
    //   test     dev        dev
    //   bench    dev        dev
    //   bin      dev        dev
    //   bin      dev        build
    //   example  dev        build
    p.cargo("build --all-targets -vv")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C embed-bitcode=[..]-C codegen-units=1 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C panic=abort -C embed-bitcode=[..]-C codegen-units=1 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C embed-bitcode=[..]-C codegen-units=5 [..]`
[COMPILING] bdep v0.0.1 ([ROOT]/foo/bdep)
[RUNNING] `[..] rustc --crate-name bdep --edition=2015 bdep/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=5 [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name build_script_build --edition=2015 build.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=5 [..]`
[RUNNING] `[..][ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[foo 0.0.1] foo custom build PROFILE=debug DEBUG=true OPT_LEVEL=0
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link -C panic=abort[..]-C codegen-units=1 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--emit=[..]link[..]-C codegen-units=1 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=1 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]link[..]-C codegen-units=1 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name test1 --edition=2015 tests/test1.rs [..]--emit=[..]link[..]-C codegen-units=1 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name bench1 --edition=2015 benches/bench1.rs [..]--emit=[..]link[..]-C codegen-units=1 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link -C panic=abort[..]-C codegen-units=1 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name ex1 --edition=2015 examples/ex1.rs [..]--crate-type bin --emit=[..]link -C panic=abort[..]-C codegen-units=1 -C debuginfo=2 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered())
        .with_stderr_does_not_contain("[..] -C debuginfo=0[..]")
        .run();
    p.cargo("build -vv")
        .with_stderr_data(
            str![[r#"
[FRESH] bar v0.0.1 ([ROOT]/foo/bar)
[FRESH] bdep v0.0.1 ([ROOT]/foo/bdep)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn profile_selection_build_all_targets_release() {
    let p = all_target_project();
    // `build --all-targets --release`
    // NOTES:
    // - bdep `panic` is not set because it thinks `build.rs` is a plugin.
    // - bar compiled twice. It tries with and without panic, but the "is a
    //   plugin" logic is forcing it to be cleared.
    // - build_script_build is built without panic because it thinks
    //   `build.rs` is a plugin.
    // - build_script_build is being run two times. Once for the `dev` and
    //   `test` targets, once for the `bench` targets.
    //   TODO: "PROFILE" says debug both times, though!
    //
    // - Dependency profiles:
    //   Pkg  Target  Profile        Reason
    //   ---  ------  -------        ------
    //   bar  lib     release        For foo-bin
    //   bar  lib     release-panic  For tests/benches and bdep
    //   bdep lib     release-panic  For foo build.rs
    //   foo  custom  release-panic
    //
    // - `foo` target list is:
    //   Target   Profile        Mode
    //   ------   -------        ----
    //   lib      release+panic  build  (a normal lib target)
    //   lib      release-panic  build  (used by tests/benches)
    //   lib      release        test   (bench/test de-duped)
    //   test     release        test
    //   bench    release        test
    //   bin      release        test   (bench/test de-duped)
    //   bin      release        build
    //   example  release        build
    p.cargo("build --all-targets --release -vv")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3 -C embed-bitcode=[..]-C codegen-units=2 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3 -C panic=abort -C embed-bitcode=[..]-C codegen-units=2 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C embed-bitcode=[..]-C codegen-units=6 [..]`
[COMPILING] bdep v0.0.1 ([ROOT]/foo/bdep)
[RUNNING] `[..] rustc --crate-name bdep --edition=2015 bdep/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=6 [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name build_script_build --edition=2015 build.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=6 [..]`
[RUNNING] `[..][ROOT]/foo/target/release/build/foo-[HASH]/build-script-build`
[foo 0.0.1] foo custom build PROFILE=release DEBUG=false OPT_LEVEL=3
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--emit=[..]link -C opt-level=3[..]-C codegen-units=2 --test [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3[..]-C codegen-units=2 [..]`
[RUNNING] `[..] rustc --crate-name test1 --edition=2015 tests/test1.rs [..]--emit=[..]link -C opt-level=3[..]-C codegen-units=2 --test [..]`
[RUNNING] `[..] rustc --crate-name bench1 --edition=2015 benches/bench1.rs [..]--emit=[..]link -C opt-level=3[..]-C codegen-units=2 --test [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]link -C opt-level=3[..]-C codegen-units=2 --test [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]`
[RUNNING] `[..] rustc --crate-name ex1 --edition=2015 examples/ex1.rs [..]--crate-type bin --emit=[..]link -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]`
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]].unordered())
        .run();
    p.cargo("build --all-targets --release -vv")
        .with_stderr_data(
            str![[r#"
[FRESH] bar v0.0.1 ([ROOT]/foo/bar)
[FRESH] bdep v0.0.1 ([ROOT]/foo/bdep)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn profile_selection_test() {
    let p = all_target_project();
    // `test`
    // NOTES:
    // - Dependency profiles:
    //   Pkg  Target  Profile    Reason
    //   ---  ------  -------    ------
    //   bar  lib     test        For foo-bin
    //   bar  lib     test-panic  For tests/benches and bdep
    //   bdep lib     test-panic  For foo build.rs
    //   foo  custom  test-panic
    //
    // - `foo` target list is:
    //   Target   Profile        Mode
    //   ------   -------        ----
    //   lib      test-panic     build (for tests)
    //   lib      test           build (for bins)
    //   lib      test           test
    //   test     test           test
    //   example  test-panic     build
    //   bin      test           test
    //   bin      test           build
    //
    p.cargo("test -vv")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C embed-bitcode=[..]-C codegen-units=3 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C embed-bitcode=[..]-C codegen-units=5 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C panic=abort[..]-C embed-bitcode=[..]-C codegen-units=3 -C debuginfo=2 [..]`
[COMPILING] bdep v0.0.1 ([ROOT]/foo/bdep)
[RUNNING] `[..] rustc --crate-name bdep --edition=2015 bdep/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=5 [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name build_script_build --edition=2015 build.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=5 [..]`
[RUNNING] `[..][ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[foo 0.0.1] foo custom build PROFILE=debug DEBUG=true OPT_LEVEL=0
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link -C panic=abort[..]-C codegen-units=3 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=3 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--emit=[..]link[..]-C codegen-units=3 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name test1 --edition=2015 tests/test1.rs [..]--emit=[..]link[..]-C codegen-units=3 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name ex1 --edition=2015 examples/ex1.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=3 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]link[..]-C codegen-units=3 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link -C panic=abort[..]-C codegen-units=3 -C debuginfo=2 [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[..][ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`
[RUNNING] `[..][ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`
[RUNNING] `[..][ROOT]/foo/target/debug/deps/test1-[HASH][EXE]`
[DOCTEST] foo
[RUNNING] `[..] rustdoc [..]--test [..]

"#]].unordered())
        .run();
    p.cargo("test -vv")
        .with_stderr_data(
            str![[r#"
[FRESH] bdep v0.0.1 ([ROOT]/foo/bdep)
[FRESH] bar v0.0.1 ([ROOT]/foo/bar)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[..][ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`
[RUNNING] `[..][ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`
[RUNNING] `[..][ROOT]/foo/target/debug/deps/test1-[HASH][EXE]`
[DOCTEST] foo
[RUNNING] `[..] rustdoc [..]--test [..]

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn profile_selection_test_release() {
    let p = all_target_project();

    // `test --release`
    // NOTES:
    // - Dependency profiles:
    //   Pkg  Target  Profile        Reason
    //   ---  ------  -------        ------
    //   bar  lib     release        For foo-bin
    //   bar  lib     release-panic  For tests/benches and bdep
    //   bdep lib     release-panic  For foo build.rs
    //   foo  custom  release-panic
    //
    // - `foo` target list is:
    //   Target   Profile        Mode
    //   ------   -------        ----
    //   lib      release-panic  build  (for tests)
    //   lib      release        build  (for bins)
    //   lib      release        test
    //   test     release        test
    //   example  release-panic  build
    //   bin      release        test
    //   bin      release        build
    //
    p.cargo("test --release -vv")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=6 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C opt-level=3[..]-C codegen-units=2[..]`
[COMPILING] bdep v0.0.1 ([ROOT]/foo/bdep)
[RUNNING] `[..] rustc --crate-name bdep --edition=2015 bdep/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=6 [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name build_script_build --edition=2015 build.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=6 [..]`
[RUNNING] `[..][ROOT]/foo/target/release/build/foo-[HASH]/build-script-build`
[foo 0.0.1] foo custom build PROFILE=release DEBUG=false OPT_LEVEL=3
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3[..]-C codegen-units=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--emit=[..]link -C opt-level=3[..]-C codegen-units=2 --test [..]`
[RUNNING] `[..] rustc --crate-name test1 --edition=2015 tests/test1.rs [..]--emit=[..]link -C opt-level=3[..]-C codegen-units=2 --test [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]link -C opt-level=3[..]-C codegen-units=2 --test [..]`
[RUNNING] `[..] rustc --crate-name ex1 --edition=2015 examples/ex1.rs [..]--crate-type bin --emit=[..]link -C opt-level=3[..]-C codegen-units=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]`
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[..][ROOT]/foo/target/release/deps/foo-[HASH][EXE]`
[RUNNING] `[..][ROOT]/foo/target/release/deps/foo-[HASH][EXE]`
[RUNNING] `[..][ROOT]/foo/target/release/deps/test1-[HASH][EXE]`
[DOCTEST] foo
[RUNNING] `[..] rustdoc [..]--test [..]`

"#]].unordered())
        .run();
    p.cargo("test --release -vv")
        .with_stderr_data(
            str![[r#"
[FRESH] bdep v0.0.1 ([ROOT]/foo/bdep)
[FRESH] bar v0.0.1 ([ROOT]/foo/bar)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[..][ROOT]/foo/target/release/deps/foo-[HASH][EXE]`
[RUNNING] `[..][ROOT]/foo/target/release/deps/foo-[HASH][EXE]`
[RUNNING] `[..][ROOT]/foo/target/release/deps/test1-[HASH][EXE]`
[DOCTEST] foo
[RUNNING] `[..] rustdoc [..]--test [..]

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn profile_selection_bench() {
    let p = all_target_project();

    // `bench`
    // NOTES:
    // - Dependency profiles:
    //   Pkg  Target  Profile        Reason
    //   ---  ------  -------        ------
    //   bar  lib     bench          For foo-bin
    //   bar  lib     bench-panic    For tests/benches and bdep
    //   bdep lib     bench-panic    For foo build.rs
    //   foo  custom  bench-panic
    //
    // - `foo` target list is:
    //   Target   Profile        Mode
    //   ------   -------        ----
    //   lib      bench-panic    build (for benches)
    //   lib      bench          build (for bins)
    //   lib      bench          test(bench)
    //   bench    bench          test(bench)
    //   bin      bench          test(bench)
    //   bin      bench          build
    //
    p.cargo("bench -vv")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3 -C embed-bitcode=[..]-C codegen-units=4 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3 -C panic=abort -C embed-bitcode=[..]-C codegen-units=4 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C embed-bitcode=[..]-C codegen-units=6 [..]`
[COMPILING] bdep v0.0.1 ([ROOT]/foo/bdep)
[RUNNING] `[..] rustc --crate-name bdep --edition=2015 bdep/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=6 [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name build_script_build --edition=2015 build.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=6 [..]`
[RUNNING] `[..][ROOT]/foo/target/release/build/foo-[HASH]/build-script-build`
[foo 0.0.1] foo custom build PROFILE=release DEBUG=false OPT_LEVEL=3
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3 -C panic=abort[..]-C codegen-units=4 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link -C opt-level=3[..]-C codegen-units=4 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--emit=[..]link -C opt-level=3[..]-C codegen-units=4 --test [..]`
[RUNNING] `[..] rustc --crate-name bench1 --edition=2015 benches/bench1.rs [..]--emit=[..]link -C opt-level=3[..]-C codegen-units=4 --test [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]link -C opt-level=3[..]-C codegen-units=4 --test [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link -C opt-level=3 -C panic=abort[..]-C codegen-units=4 [..]`
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[..][ROOT]/foo/target/release/deps/foo-[HASH][EXE] --bench`
[RUNNING] `[..][ROOT]/foo/target/release/deps/foo-[HASH][EXE] --bench`
[RUNNING] `[..][ROOT]/foo/target/release/deps/bench1-[HASH][EXE] --bench`

"#]].unordered())
        .run();
    p.cargo("bench -vv")
        .with_stderr_data(
            str![[r#"
[FRESH] bdep v0.0.1 ([ROOT]/foo/bdep)
[FRESH] bar v0.0.1 ([ROOT]/foo/bar)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[..][ROOT]/foo/target/release/deps/foo-[HASH][EXE] --bench`
[RUNNING] `[..][ROOT]/foo/target/release/deps/foo-[HASH][EXE] --bench`
[RUNNING] `[..][ROOT]/foo/target/release/deps/bench1-[HASH][EXE] --bench`

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn profile_selection_check_all_targets() {
    let p = all_target_project();
    // `check`
    // NOTES:
    // - Dependency profiles:
    //   Pkg  Target  Profile    Action   Reason
    //   ---  ------  -------    ------   ------
    //   bar  lib     dev*       link     For bdep
    //   bar  lib     dev-panic  metadata For tests/benches
    //   bar  lib     dev        metadata For lib/bins
    //   bdep lib     dev*       link     For foo build.rs
    //   foo  custom  dev*       link     For build.rs
    //
    //   `*` = wants panic, but it is cleared when args are built.
    //
    // - foo target list is:
    //   Target   Profile        Mode
    //   ------   -------        ----
    //   lib      dev            check
    //   lib      dev-panic      check (for tests/benches)
    //   lib      dev-panic      check-test (checking lib as a unittest)
    //   example  dev            check
    //   test     dev-panic      check-test
    //   bench    dev-panic      check-test
    //   bin      dev            check
    //   bin      dev-panic      check-test (checking bin as a unittest)
    //
    p.cargo("check --all-targets -vv")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C embed-bitcode=[..]-C codegen-units=5 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]metadata -C embed-bitcode=[..]-C codegen-units=1 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]metadata -C panic=abort -C embed-bitcode=[..]-C codegen-units=1 -C debuginfo=2 [..]`
[COMPILING] bdep v0.0.1 ([ROOT]/foo/bdep)
[RUNNING] `[..] rustc --crate-name bdep --edition=2015 bdep/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=5 [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name build_script_build --edition=2015 build.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=5 [..]`
[RUNNING] `[..][ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[foo 0.0.1] foo custom build PROFILE=debug DEBUG=true OPT_LEVEL=0
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]metadata -C panic=abort[..]-C codegen-units=1 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]metadata[..]-C codegen-units=1 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--emit=[..]metadata[..]-C codegen-units=1 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name test1 --edition=2015 tests/test1.rs [..]--emit=[..]metadata[..]-C codegen-units=1 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]metadata[..]-C codegen-units=1 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name bench1 --edition=2015 benches/bench1.rs [..]--emit=[..]metadata[..]-C codegen-units=1 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name ex1 --edition=2015 examples/ex1.rs [..]--crate-type bin --emit=[..]metadata -C panic=abort[..]-C codegen-units=1 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]metadata -C panic=abort[..]-C codegen-units=1 -C debuginfo=2 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered())
        .run();
    // Starting with Rust 1.27, rustc emits `rmeta` files for bins, so
    // everything should be completely fresh. Previously, bins were being
    // rechecked.
    // See PR rust-lang/rust#49289 and issue rust-lang/cargo#3624.
    p.cargo("check --all-targets -vv")
        .with_stderr_data(
            str![[r#"
[FRESH] bdep v0.0.1 ([ROOT]/foo/bdep)
[FRESH] bar v0.0.1 ([ROOT]/foo/bar)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn profile_selection_check_all_targets_release() {
    let p = all_target_project();
    // `check --release`
    // See issue rust-lang/cargo#5218.
    // This is a pretty straightforward variant of
    // `profile_selection_check_all_targets` that uses `release` instead of
    // `dev` for all targets.
    p.cargo("check --all-targets --release -vv")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=6 [..]
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]metadata -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]metadata -C opt-level=3[..]-C codegen-units=2 [..]
[COMPILING] bdep v0.0.1 ([ROOT]/foo/bdep)
[RUNNING] `[..] rustc --crate-name bdep --edition=2015 bdep/src/lib.rs [..]--crate-type lib --emit=[..]link [..]-C codegen-units=6 [..]
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name build_script_build --edition=2015 build.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=6 [..]
[RUNNING] `[..][ROOT]/foo/target/release/build/foo-[HASH]/build-script-build`
[foo 0.0.1] foo custom build PROFILE=release DEBUG=false OPT_LEVEL=3
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]metadata -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]metadata -C opt-level=3[..]-C codegen-units=2 [..]
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--emit=[..]metadata -C opt-level=3[..]-C codegen-units=2 --test [..]
[RUNNING] `[..] rustc --crate-name test1 --edition=2015 tests/test1.rs [..]--emit=[..]metadata -C opt-level=3[..]-C codegen-units=2 --test [..]
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]metadata -C opt-level=3[..]-C codegen-units=2 --test [..]
[RUNNING] `[..] rustc --crate-name bench1 --edition=2015 benches/bench1.rs [..]--emit=[..]metadata -C opt-level=3[..]-C codegen-units=2 --test [..]
[RUNNING] `[..] rustc --crate-name ex1 --edition=2015 examples/ex1.rs [..]--crate-type bin --emit=[..]metadata -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]metadata -C opt-level=3 -C panic=abort[..]-C codegen-units=2 [..]
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]].unordered())
        .run();

    p.cargo("check --all-targets --release -vv")
        .with_stderr_data(
            str![[r#"
[FRESH] bar v0.0.1 ([ROOT]/foo/bar)
[FRESH] bdep v0.0.1 ([ROOT]/foo/bdep)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn profile_selection_check_all_targets_test() {
    let p = all_target_project();

    // `check --profile=test`
    // - Dependency profiles:
    //   Pkg  Target  Profile    Action   Reason
    //   ---  ------  -------    ------   ------
    //   bar  lib     test*       link     For bdep
    //   bar  lib     test-panic  metadata  For tests/benches
    //   bdep lib     test*       link     For foo build.rs
    //   foo  custom  test*       link     For build.rs
    //
    //   `*` = wants panic, but it is cleared when args are built.
    //
    // - foo target list is:
    //   Target   Profile     Mode
    //   ------   -------     ----
    //   lib      test-panic  check-test (for tests/benches)
    //   lib      test-panic  check-test (checking lib as a unittest)
    //   example  test-panic  check-test
    //   test     test-panic  check-test
    //   bench    test-panic  check-test
    //   bin      test-panic  check-test
    //
    p.cargo("check --all-targets --profile=test -vv")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=5 [..]`
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]metadata[..]-C codegen-units=3 -C debuginfo=2 [..]`
[COMPILING] bdep v0.0.1 ([ROOT]/foo/bdep)
[RUNNING] `[..] rustc --crate-name bdep --edition=2015 bdep/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=5 [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name build_script_build --edition=2015 build.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=5 [..]`
[RUNNING] `[..][ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[foo 0.0.1] foo custom build PROFILE=debug DEBUG=true OPT_LEVEL=0
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]metadata[..]-C codegen-units=3 -C debuginfo=2 [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/lib.rs [..]--emit=[..]metadata[..]-C codegen-units=3 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name test1 --edition=2015 tests/test1.rs [..]--emit=[..]metadata[..]-C codegen-units=3 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]metadata[..]-C codegen-units=3 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name bench1 --edition=2015 benches/bench1.rs [..]--emit=[..]metadata[..]-C codegen-units=3 -C debuginfo=2 [..]--test [..]`
[RUNNING] `[..] rustc --crate-name ex1 --edition=2015 examples/ex1.rs [..]--emit=[..]metadata[..]-C codegen-units=3 -C debuginfo=2 [..]--test [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered())
        .run();

    p.cargo("check --all-targets --profile=test -vv")
        .with_stderr_data(
            str![[r#"
[FRESH] bdep v0.0.1 ([ROOT]/foo/bdep)
[FRESH] bar v0.0.1 ([ROOT]/foo/bar)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn profile_selection_doc() {
    let p = all_target_project();
    // `doc`
    // NOTES:
    // - Dependency profiles:
    //   Pkg  Target  Profile    Action   Reason
    //   ---  ------  -------    ------   ------
    //   bar  lib     dev*       link     For bdep
    //   bar  lib     dev        metadata For rustdoc
    //   bdep lib     dev*       link     For foo build.rs
    //   foo  custom  dev*       link     For build.rs
    //
    //   `*` = wants panic, but it is cleared when args are built.
    p.cargo("doc -vv")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[DOCUMENTING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=5 [..]`
[RUNNING] `[..] rustdoc [..]--crate-name bar bar/src/lib.rs [..]
[RUNNING] `[..] rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--crate-type lib --emit=[..]metadata -C panic=abort[..]-C codegen-units=1 -C debuginfo=2 [..]`
[COMPILING] bdep v0.0.1 ([ROOT]/foo/bdep)
[RUNNING] `[..] rustc --crate-name bdep --edition=2015 bdep/src/lib.rs [..]--crate-type lib --emit=[..]link[..]-C codegen-units=5 [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name build_script_build --edition=2015 build.rs [..]--crate-type bin --emit=[..]link[..]-C codegen-units=5 [..]`
[RUNNING] `[..][ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[foo 0.0.1] foo custom build PROFILE=debug DEBUG=true OPT_LEVEL=0
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustdoc [..]--crate-name foo src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]].unordered())
        .run();
}
