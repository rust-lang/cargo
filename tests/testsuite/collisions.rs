//! Tests for when multiple artifacts have the same output filename.
//! See <https://github.com/rust-lang/cargo/issues/6313> for more details.
//! Ideally these should never happen, but I don't think we'll ever be able to
//! prevent all collisions.

use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{basic_manifest, cross_compile, project};
use std::env;

#[cargo_test]
fn collision_dylib() {
    // Path dependencies don't include metadata hash in filename for dylibs.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "1.0.0"
            edition = "2015"

            [lib]
            crate-type = ["dylib"]
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "1.0.0"
            edition = "2015"

            [lib]
            crate-type = ["dylib"]
            name = "a"
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    // `j=1` is required because on Windows you'll get an error due to
    // two processes writing to the file at the same time.
    p.cargo("build -j=1")
        .with_stderr_data(&format!("\
...
[WARNING] output filename collision.
The lib target `a` in package `b v1.0.0 ([ROOT]/foo/b)` has the same output filename as the lib target `a` in package `a v1.0.0 ([ROOT]/foo/a)`.
Colliding filename is: [ROOT]/foo/target/debug/deps/{}a{}
The targets should have unique names.
Consider changing their names to be unique or compiling them separately.
This may become a hard error in the future; see <https://github.com/rust-lang/cargo/issues/6313>.
...
", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX))
        .run();
}

#[cargo_test]
fn collision_example() {
    // Examples in a workspace can easily collide.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "1.0.0"))
        .file("a/examples/ex1.rs", "fn main() {}")
        .file("b/Cargo.toml", &basic_manifest("b", "1.0.0"))
        .file("b/examples/ex1.rs", "fn main() {}")
        .build();

    // `j=1` is required because on Windows you'll get an error due to
    // two processes writing to the file at the same time.
    p.cargo("build --examples -j=1")
        .with_stderr_data(str![[r#"
...
[WARNING] output filename collision.
The example target `ex1` in package `b v1.0.0 ([ROOT]/foo/b)` has the same output filename as the example target `ex1` in package `a v1.0.0 ([ROOT]/foo/a)`.
Colliding filename is: [ROOT]/foo/target/debug/examples/ex1[EXE]
The targets should have unique names.
Consider changing their names to be unique or compiling them separately.
This may become a hard error in the future; see <https://github.com/rust-lang/cargo/issues/6313>.
...

"#]])
        .run();
}

#[cargo_test]
// See https://github.com/rust-lang/cargo/issues/7493
#[cfg_attr(
    any(target_env = "msvc", target_vendor = "apple"),
    ignore = "--artifact-dir and examples are currently broken on MSVC and apple"
)]
fn collision_export() {
    // `--artifact-dir` combines some things which can cause conflicts.
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("examples/foo.rs", "fn main() {}")
        .file("src/main.rs", "fn main() {}")
        .build();

    // -j1 to avoid issues with two processes writing to the same file at the
    // same time.
    p.cargo("build -j1 --artifact-dir=out -Z unstable-options --bins --examples")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .with_stderr_data(str![[r#"
[WARNING] `--artifact-dir` filename collision.
The example target `foo` in package `foo v1.0.0 ([ROOT]/foo)` has the same output filename as the bin target `foo` in package `foo v1.0.0 ([ROOT]/foo)`.
Colliding filename is: [ROOT]/foo/out/foo[EXE]
The exported filenames should be unique.
Consider changing their names to be unique or compiling them separately.
This may become a hard error in the future; see <https://github.com/rust-lang/cargo/issues/6313>.
...

"#]])
        .run();
}

#[cargo_test]
fn collision_doc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            foo2 = { path = "foo2" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo2/Cargo.toml",
            r#"
            [package]
            name = "foo2"
            version = "0.1.0"
            edition = "2015"

            [lib]
            name = "foo"
            "#,
        )
        .file("foo2/src/lib.rs", "")
        .build();

    p.cargo("doc -j=1")
        .with_stderr_data(str![[r#"
...
[WARNING] output filename collision.
The lib target `foo` in package `foo2 v0.1.0 ([ROOT]/foo/foo2)` has the same output filename as the lib target `foo` in package `foo v0.1.0 ([ROOT]/foo)`.
Colliding filename is: [ROOT]/foo/target/doc/foo/index.html
The targets should have unique names.
This is a known bug where multiple crates with the same name use
the same path; see <https://github.com/rust-lang/cargo/issues/6313>.
...

"#]])
        .run();
}

#[cargo_test]
fn collision_doc_multiple_versions() {
    // Multiple versions of the same package.
    Package::new("old-dep", "1.0.0").publish();
    Package::new("bar", "1.0.0").dep("old-dep", "1.0").publish();
    // Note that this removes "old-dep". Just checking what happens when there
    // are orphans.
    Package::new("bar", "2.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
                bar2 = { package="bar", version="2.0" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Should only document bar 2.0, should not document old-dep.
    p.cargo("doc")
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[ADDING] bar v1.0.0 (available: v2.0.0)
[DOWNLOADING] crates ...
[DOWNLOADED] bar v2.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] old-dep v1.0.0 (registry `dummy-registry`)
[CHECKING] old-dep v1.0.0
[CHECKING] bar v2.0.0
[CHECKING] bar v1.0.0
[DOCUMENTING] bar v2.0.0
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[DOCUMENTING] foo v0.1.0 ([ROOT]/foo)
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn collision_doc_host_target_feature_split() {
    // Same dependency built twice due to different features.
    //
    // foo v0.1.0
    // ├── common v1.0.0
    // │   └── common-dep v1.0.0
    // └── pm v0.1.0 (proc-macro)
    //     └── common v1.0.0
    //         └── common-dep v1.0.0
    // [build-dependencies]
    // └── common-dep v1.0.0
    //
    // Here `common` and `common-dep` are built twice. `common-dep` has
    // different features for host versus target.
    Package::new("common-dep", "1.0.0")
        .feature("bdep-feat", &[])
        .file(
            "src/lib.rs",
            r#"
                /// Some doc
                pub fn f() {}

                /// Another doc
                #[cfg(feature = "bdep-feat")]
                pub fn bdep_func() {}
            "#,
        )
        .publish();
    Package::new("common", "1.0.0")
        .dep("common-dep", "1.0")
        .file(
            "src/lib.rs",
            r#"
                /// Some doc
                pub fn f() {}
            "#,
        )
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                resolver = "2"

                [dependencies]
                pm = { path = "pm" }
                common = "1.0"

                [build-dependencies]
                common-dep = { version = "1.0", features = ["bdep-feat"] }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                /// Some doc
                pub fn f() {}
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file(
            "pm/Cargo.toml",
            r#"
                [package]
                name = "pm"
                version = "0.1.0"
                edition = "2018"

                [lib]
                proc-macro = true

                [dependencies]
                common = "1.0"
            "#,
        )
        .file(
            "pm/src/lib.rs",
            r#"
                use proc_macro::TokenStream;

                /// Some doc
                #[proc_macro]
                pub fn pm(_input: TokenStream) -> TokenStream {
                    "".parse().unwrap()
                }
            "#,
        )
        .build();

    // No warnings, no duplicates, common and common-dep only documented once.
    p.cargo("doc")
        // Cannot check full output due to https://github.com/rust-lang/cargo/issues/9076
        .with_stderr_does_not_contain("[WARNING][..]")
        .run();

    assert!(p.build_dir().join("doc/common_dep/fn.f.html").exists());
    assert!(!p
        .build_dir()
        .join("doc/common_dep/fn.bdep_func.html")
        .exists());
    assert!(p.build_dir().join("doc/common/fn.f.html").exists());
    assert!(p.build_dir().join("doc/pm/macro.pm.html").exists());
    assert!(p.build_dir().join("doc/foo/fn.f.html").exists());
}

#[cargo_test]
fn collision_doc_profile_split() {
    // Same dependency built twice due to different profile settings.
    Package::new("common", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                pm = { path = "pm" }
                common = "1.0"

                [profile.dev]
                opt-level = 2
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "pm/Cargo.toml",
            r#"
                [package]
                name = "pm"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                common = "1.0"

                [lib]
                proc-macro = true
            "#,
        )
        .file("pm/src/lib.rs", "")
        .build();

    // Just to verify that common is normally built twice.
    // This is unordered because in rare cases `pm` may start
    // building in-between the two `common`.
    p.cargo("build -v")
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] common v1.0.0 (registry `dummy-registry`)
[COMPILING] common v1.0.0
[RUNNING] `rustc --crate-name common [..]
[RUNNING] `rustc --crate-name common [..]
[COMPILING] pm v0.1.0 ([ROOT]/foo/pm)
[RUNNING] `rustc --crate-name pm [..]
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `dev` profile [optimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    // Should only document common once, no warnings.
    p.cargo("doc")
        .with_stderr_data(
            str![[r#"
[CHECKING] common v1.0.0
[DOCUMENTING] common v1.0.0
[DOCUMENTING] pm v0.1.0 ([ROOT]/foo/pm)
[DOCUMENTING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [optimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn collision_doc_sources() {
    // Different sources with the same package.
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
                bar2 = { path = "bar", package = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "1.0.0"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("doc -j=1")
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[WARNING] output filename collision.
The lib target `bar` in package `bar v1.0.0` has the same output filename as the lib target `bar` in package `bar v1.0.0 ([ROOT]/foo/bar)`.
Colliding filename is: [ROOT]/foo/target/doc/bar/index.html
The targets should have unique names.
This is a known bug where multiple crates with the same name use
the same path; see <https://github.com/rust-lang/cargo/issues/6313>.
[CHECKING] bar v1.0.0 ([ROOT]/foo/bar)
[DOCUMENTING] bar v1.0.0 ([ROOT]/foo/bar)
[DOCUMENTING] bar v1.0.0
[CHECKING] bar v1.0.0
[DOCUMENTING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn collision_doc_target() {
    // collision in doc with --target, doesn't fail due to orphans
    if cross_compile::disabled() {
        return;
    }

    Package::new("orphaned", "1.0.0").publish();
    Package::new("bar", "1.0.0")
        .dep("orphaned", "1.0")
        .publish();
    Package::new("bar", "2.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar2 = { version = "2.0", package="bar" }
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("doc --target")
        .arg(cross_compile::alternate())
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[ADDING] bar v1.0.0 (available: v2.0.0)
[DOWNLOADING] crates ...
[DOWNLOADED] orphaned v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar v2.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] orphaned v1.0.0
[DOCUMENTING] bar v2.0.0
[CHECKING] bar v2.0.0
[CHECKING] bar v1.0.0
[DOCUMENTING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/[ALT_TARGET]/doc/foo/index.html

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn collision_with_root() {
    // Check for a doc collision between a root package and a dependency.
    // In this case, `foo-macro` comes from both the workspace and crates.io.
    // This checks that the duplicate correction code doesn't choke on this
    // by removing the root unit.
    Package::new("foo-macro", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["abc", "foo-macro"]
            "#,
        )
        .file(
            "abc/Cargo.toml",
            r#"
                [package]
                name = "abc"
                version = "1.0.0"
                edition = "2015"

                [dependencies]
                foo-macro = "1.0"
            "#,
        )
        .file("abc/src/lib.rs", "")
        .file(
            "foo-macro/Cargo.toml",
            r#"
                [package]
                name = "foo-macro"
                version = "1.0.0"
                edition = "2015"

                [lib]
                proc-macro = true

                [dependencies]
                abc = {path="../abc"}
            "#,
        )
        .file("foo-macro/src/lib.rs", "")
        .build();

    p.cargo("doc -j=1")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] foo-macro v1.0.0 (registry `dummy-registry`)
[WARNING] output filename collision.
The lib target `foo_macro` in package `foo-macro v1.0.0` has the same output filename as the lib target `foo_macro` in package `foo-macro v1.0.0 ([ROOT]/foo/foo-macro)`.
Colliding filename is: [ROOT]/foo/target/doc/foo_macro/index.html
The targets should have unique names.
This is a known bug where multiple crates with the same name use
the same path; see <https://github.com/rust-lang/cargo/issues/6313>.
[CHECKING] foo-macro v1.0.0
[DOCUMENTING] foo-macro v1.0.0
[CHECKING] abc v1.0.0 ([ROOT]/foo/abc)
[DOCUMENTING] foo-macro v1.0.0 ([ROOT]/foo/foo-macro)
[DOCUMENTING] abc v1.0.0 ([ROOT]/foo/abc)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/abc/index.html and 1 other file

"#]].unordered())
        .run();
}
