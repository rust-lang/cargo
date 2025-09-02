//! Tests for `path` dependencies.

use std::fs;

use crate::prelude::*;
use cargo_test_support::paths;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{basic_lib_manifest, basic_manifest, main_file, project};
use cargo_test_support::{sleep_ms, t};

#[cargo_test]
// I have no idea why this is failing spuriously on Windows;
// for more info, see #3466.
#[cfg(not(windows))]
fn cargo_compile_with_nested_deps_shorthand() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies.bar]

                version = "0.5.0"
                path = "bar"
            "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
                [package]

                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies.baz]

                version = "0.5.0"
                path = "baz"

                [lib]

                name = "bar"
            "#,
        )
        .file(
            "bar/src/bar.rs",
            r#"
                extern crate baz;

                pub fn gimme() -> String {
                    baz::gimme()
                }
            "#,
        )
        .file("bar/baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file(
            "bar/baz/src/baz.rs",
            r#"
                pub fn gimme() -> String {
                    "test passed".to_string()
                }
            "#,
        )
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] baz v0.5.0 ([ROOT]/foo/bar/baz)
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
test passed

"#]])
        .run();

    println!("cleaning");
    p.cargo("clean -v")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/foo/target
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
    println!("building baz");
    p.cargo("build -p baz")
        .with_stderr_data(str![[r#"
[COMPILING] baz v0.5.0 ([ROOT]/foo/bar/baz)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    println!("building foo");
    p.cargo("build -p foo")
        .with_stderr_data(str![[r#"
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_root_dev_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dev-dependencies.bar]

                version = "0.5.0"
                path = "../bar"

                [[bin]]
                name = "foo"
            "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .build();
    let _p2 = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file(
            "src/lib.rs",
            r#"
                pub fn gimme() -> &'static str {
                    "zoidberg"
                }
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.5.0 ([ROOT]/foo)
error[E0463]: can't find crate for `bar`
...
"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_root_dev_deps_with_testing() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dev-dependencies.bar]

                version = "0.5.0"
                path = "../bar"

                [[bin]]
                name = "foo"
            "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .build();
    let _p2 = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file(
            "src/lib.rs",
            r#"
                pub fn gimme() -> &'static str {
                    "zoidberg"
                }
            "#,
        )
        .build();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/bar)
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/main.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_transitive_dev_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies.bar]

                version = "0.5.0"
                path = "bar"
            "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
                [package]

                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dev-dependencies.baz]

                git = "git://example.com/path/to/nowhere"

                [lib]

                name = "bar"
            "#,
        )
        .file(
            "bar/src/bar.rs",
            r#"
                pub fn gimme() -> &'static str {
                    "zoidberg"
                }
            "#,
        )
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
zoidberg

"#]])
        .run();
}

#[cargo_test]
fn no_rebuild_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() { bar::bar() }")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/bar.rs", "pub fn bar() {}")
        .build();
    // First time around we should compile both foo and bar
    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.5.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    sleep_ms(1000);
    p.change_file(
        "src/main.rs",
        r#"
            extern crate bar;
            fn main() { bar::bar(); }
        "#,
    );
    // Don't compile bar, but do recompile foo.
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn deep_dependencies_trigger_rebuild() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() { bar::bar() }")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]

                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [lib]
                name = "bar"
                [dependencies.baz]
                path = "../baz"
            "#,
        )
        .file(
            "bar/src/bar.rs",
            "extern crate baz; pub fn bar() { baz::baz() }",
        )
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file("baz/src/baz.rs", "pub fn baz() {}")
        .build();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[CHECKING] baz v0.5.0 ([ROOT]/foo/baz)
[CHECKING] bar v0.5.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Make sure an update to baz triggers a rebuild of bar
    //
    // We base recompilation off mtime, so sleep for at least a second to ensure
    // that this write will change the mtime.
    sleep_ms(1000);
    p.change_file("baz/src/baz.rs", r#"pub fn baz() { println!("hello!"); }"#);
    sleep_ms(1000);
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] baz v0.5.0 ([ROOT]/foo/baz)
[CHECKING] bar v0.5.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Make sure an update to bar doesn't trigger baz
    sleep_ms(1000);
    p.change_file(
        "bar/src/bar.rs",
        r#"
            extern crate baz;
            pub fn bar() { println!("hello!"); baz::baz(); }
        "#,
    );
    sleep_ms(1000);
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.5.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn no_rebuild_two_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies.bar]
                path = "bar"
                [dependencies.baz]
                path = "baz"
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() { bar::bar() }")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]

                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [lib]
                name = "bar"
                [dependencies.baz]
                path = "../baz"
            "#,
        )
        .file("bar/src/bar.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file("baz/src/baz.rs", "pub fn baz() {}")
        .build();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] baz v0.5.0 ([ROOT]/foo/baz)
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert!(p.bin("foo").is_file());
    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn nested_deps_recompile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies.bar]

                version = "0.5.0"
                path = "src/bar"
            "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("src/bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("src/bar/src/bar.rs", "pub fn gimme() -> i32 { 92 }")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.5.0 ([ROOT]/foo/src/bar)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    sleep_ms(1000);

    p.change_file("src/main.rs", r#"fn main() {}"#);

    // This shouldn't recompile `bar`
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn error_message_for_missing_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies.bar]

                path = "src/bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/bar/not-a-manifest", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to get `bar` as a dependency of package `foo v0.5.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update [ROOT]/foo/src/bar

Caused by:
  failed to read `[ROOT]/foo/src/bar/Cargo.toml`

Caused by:
  [NOT_FOUND]

"#]])
        .run();
}

#[cargo_test]
fn path_bases_not_stable() {
    let bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [path-bases]
                    test = '{}'
                "#,
                bar.root().parent().unwrap().display()
            ),
        )
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                bar = { base = 'test', path = 'bar' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(
            "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  resolving path dependency bar

Caused by:
  feature `path-bases` is required

  The package requires the Cargo feature called `path-bases`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider trying a newer version of Cargo (this may require the nightly release).
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#path-bases for more information about the status of this feature.
",
        )
        .run();
}

#[cargo_test]
fn path_with_base() {
    let bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [path-bases]
                    test = '{}'
                "#,
                bar.root().parent().unwrap().display()
            ),
        )
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                bar = { base = 'test', path = 'bar' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .run();
}

#[cargo_test]
fn current_dir_with_base() {
    let bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [path-bases]
                    test = '{}'
                "#,
                bar.root().display()
            ),
        )
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                bar = { base = 'test', path = '.' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .run();
}

#[cargo_test]
fn parent_dir_with_base() {
    let bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [path-bases]
                    test = '{}/subdir'
                "#,
                bar.root().display()
            ),
        )
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                bar = { base = 'test', path = '..' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .run();
}

#[cargo_test]
fn inherit_dependency_using_base() {
    let bar = project()
        .at("dep_with_base")
        .file("Cargo.toml", &basic_manifest("dep_with_base", "0.5.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [path-bases]
                    test = '{}'
                "#,
                bar.root().parent().unwrap().display()
            ),
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "parent"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["child"]

                [workspace.dependencies]
                dep_with_base = { base = 'test', path = 'dep_with_base' }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "child/Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "child"
                version = "0.1.0"
                authors = []

                [dependencies]
                dep_with_base = { workspace = true }
            "#,
        )
        .file("child/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .run();
}

#[cargo_test]
fn path_with_relative_base() {
    project()
        .at("shared_proj/bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            "../.cargo/config.toml",
            r#"
                [path-bases]
                test = 'shared_proj'
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                bar = { base = 'test', path = 'bar' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .run();
}

#[cargo_test]
fn workspace_builtin_base() {
    project()
        .at("dep_with_base")
        .file("Cargo.toml", &basic_manifest("dep_with_base", "0.5.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "parent"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["child"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "child/Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "child"
                version = "0.1.0"
                authors = []

                [dependencies]
                dep_with_base = { base = 'workspace', path = '../dep_with_base' }
            "#,
        )
        .file("child/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .run();
}

#[cargo_test]
fn workspace_builtin_base_not_a_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { base = 'workspace', path = 'bar' }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .with_status(101)
        .with_stderr_data(
            "\
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  resolving path dependency bar

Caused by:
  failed to find a workspace root
",
        )
        .run();
}

#[cargo_test]
fn shadow_workspace_builtin_base() {
    let bar = project()
        .at("dep_with_base")
        .file("Cargo.toml", &basic_manifest("dep_with_base", "0.5.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [path-bases]
                    workspace = '{}/subdir/anotherdir'
                "#,
                bar.root().parent().unwrap().display()
            ),
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "parent"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["child"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "child/Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "child"
                version = "0.1.0"
                authors = []

                [dependencies]
                dep_with_base = { base = 'workspace', path = '../../dep_with_base' }
            "#,
        )
        .file("child/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .run();
}

#[cargo_test]
fn unknown_base() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                bar = { base = 'test', path = 'bar' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .with_status(101)
        .with_stderr_data(
            "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  resolving path dependency bar

Caused by:
  path base `test` is undefined. You must add an entry for `test` in the Cargo configuration [path-bases] table.
",
        )
        .run();
}

#[cargo_test]
fn base_without_path() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                bar = { version = '1.0.0', base = 'test' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .with_status(101)
        .with_stderr_data(
            "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  resolving path dependency bar

Caused by:
  `base` can only be used with path dependencies
",
        )
        .run();
}

#[cargo_test]
fn invalid_base() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                bar = { base = '^^not-valid^^', path = 'bar' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .with_status(101)
        .with_stderr_data(
            "\
[ERROR] invalid character `^` in path base name: `^^not-valid^^`, the first character must be a Unicode XID start character (most letters or `_`)
       
       
  --> Cargo.toml:10:23
   |
10 |                 bar = { base = '^^not-valid^^', path = 'bar' }
   |                       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
",
        )
        .run();
}

#[cargo_test]
fn invalid_path_with_base() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [path-bases]
            test = 'shared_proj'
        "#,
        )
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]
                edition = "2015"

                [dependencies]
                bar = { base = 'test', path = '"' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .with_status(101)
        .with_stderr_data(
            "\
[ERROR] failed to get `bar` as a dependency of package `foo v0.5.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update [ROOT]/foo/shared_proj/\"

Caused by:
  failed to read `[ROOT]/foo/shared_proj/\"/Cargo.toml`

Caused by:
  [NOT_FOUND]
",
        )
        .run();
}

#[cargo_test]
fn self_dependency_using_base() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [path-bases]
            test = '.'
        "#,
        )
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["path-bases"]

                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
                edition = "2015"

                [dependencies]
                foo = { base = 'test', path = '.' }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo(&["path-bases"])
        .with_status(101)
        .with_stderr_data(
            "\
[ERROR] cyclic package dependency: package `foo v0.1.0 ([ROOT]/foo)` depends on itself. Cycle:
package `foo v0.1.0 ([ROOT]/foo)`
    ... which satisfies path dependency `foo` of package `foo v0.1.0 ([ROOT]/foo)`
",
        )
        .run();
}

#[cargo_test]
fn override_relative() {
    let bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("src/lib.rs", "")
        .build();

    fs::create_dir(&paths::root().join(".cargo")).unwrap();
    fs::write(
        &paths::root().join(".cargo/config.toml"),
        r#"paths = ["bar"]"#,
    )
    .unwrap();

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

                    [dependencies.bar]
                    path = '{}'
                "#,
                bar.root().display()
            ),
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check -v").run();
}

#[cargo_test]
fn override_self() {
    let bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project();
    let root = p.root();
    let p = p
        .file(
            ".cargo/config.toml",
            &format!("paths = ['{}']", root.display()),
        )
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = ["wycats@example.com"]

                    [dependencies.bar]
                    path = '{}'

                "#,
                bar.root().display()
            ),
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn override_path_dep() {
    let bar = project()
        .at("bar")
        .file(
            "p1/Cargo.toml",
            r#"
                 [package]
                 name = "p1"
                 version = "0.5.0"
                 edition = "2015"
                 authors = []

                 [dependencies.p2]
                 path = "../p2"
            "#,
        )
        .file("p1/src/lib.rs", "")
        .file("p2/Cargo.toml", &basic_manifest("p2", "0.5.0"))
        .file("p2/src/lib.rs", "")
        .build();

    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                "paths = ['{}', '{}']",
                bar.root().join("p1").display(),
                bar.root().join("p2").display()
            ),
        )
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = ["wycats@example.com"]

                    [dependencies.p2]
                    path = '{}'

                "#,
                bar.root().join("p2").display()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -v").run();
}

#[cargo_test]
fn path_dep_build_cmd() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies.bar]

                version = "0.5.0"
                path = "bar"
            "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
                [package]

                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]
                build = "build.rs"

                [lib]
                name = "bar"
                path = "src/bar.rs"
            "#,
        )
        .file(
            "bar/build.rs",
            r#"
                use std::fs;
                fn main() {
                    fs::copy("src/bar.rs.in", "src/bar.rs").unwrap();
                }
            "#,
        )
        .file("bar/src/bar.rs.in", "pub fn gimme() -> i32 { 0 }")
        .build();
    p.root().join("bar").move_into_the_past();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
0

"#]])
        .run();

    // Touching bar.rs.in should cause the `build` command to run again.
    p.change_file("bar/src/bar.rs.in", "pub fn gimme() -> i32 { 1 }");

    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
1

"#]])
        .run();
}

#[cargo_test]
fn dev_deps_no_rebuild_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = []

                [dev-dependencies.bar]
                    path = "bar"

                [lib]
                    name = "foo"
                    doctest = false
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[cfg(test)] #[allow(unused_extern_crates)] extern crate bar;
                #[cfg(not(test))] pub fn foo() { env!("FOO"); }
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();
    p.cargo("build")
        .env("FOO", "bar")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test]
fn custom_target_no_rebuild() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = []
                [dependencies]
                a = { path = "a" }
                [workspace]
                members = ["a", "b"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.5.0"))
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.5.0"
                edition = "2015"
                authors = []
                [dependencies]
                a = { path = "../a" }
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] a v0.5.0 ([ROOT]/foo/a)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    t!(fs::rename(
        p.root().join("target"),
        p.root().join("target_moved")
    ));
    p.cargo("check --manifest-path=b/Cargo.toml")
        .env("CARGO_TARGET_DIR", "target_moved")
        .with_stderr_data(str![[r#"
[CHECKING] b v0.5.0 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn override_and_depend() {
    let p = project()
        .no_manifest()
        .file(
            "a/a1/Cargo.toml",
            r#"
                [package]
                name = "a1"
                version = "0.5.0"
                edition = "2015"
                authors = []
                [dependencies]
                a2 = { path = "../a2" }
            "#,
        )
        .file("a/a1/src/lib.rs", "")
        .file("a/a2/Cargo.toml", &basic_manifest("a2", "0.5.0"))
        .file("a/a2/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.5.0"
                edition = "2015"
                authors = []
                [dependencies]
                a1 = { path = "../a/a1" }
                a2 = { path = "../a/a2" }
            "#,
        )
        .file("b/src/lib.rs", "")
        .file("b/.cargo/config.toml", r#"paths = ["../a"]"#)
        .build();
    p.cargo("check")
        .cwd("b")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[CHECKING] a2 v0.5.0 ([ROOT]/foo/a)
[CHECKING] a1 v0.5.0 ([ROOT]/foo/a)
[CHECKING] b v0.5.0 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn missing_path_dependency() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("a", "0.5.0"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"paths = ["../whoa-this-does-not-exist"]"#,
        )
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to update path override `[ROOT]/whoa-this-does-not-exist` (defined in `[ROOT]/foo/.cargo/config.toml`)

Caused by:
  failed to read directory `[ROOT]/whoa-this-does-not-exist`

Caused by:
  [NOT_FOUND]

"#]])
        .run();
}

#[cargo_test]
fn invalid_path_dep_in_workspace_with_lockfile() {
    Package::new("bar", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "top"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [workspace]

                [dependencies]
                foo = { path = "foo" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    // Generate a lock file
    p.cargo("check").run();

    // Change the dependency on `bar` to an invalid path
    p.change_file(
        "foo/Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.5.0"
            edition = "2015"
            authors = []

            [dependencies]
            bar = { path = "" }
        "#,
    );

    // Make sure we get a nice error. In the past this actually stack
    // overflowed!
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no matching package found
searched package name: `bar`
perhaps you meant:      foo
location searched: [ROOT]/foo/foo
required by package `foo v0.5.0 ([ROOT]/foo/foo)`

"#]])
        .run();
}

#[cargo_test]
fn workspace_produces_rlib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "top"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [workspace]

                [dependencies]
                foo = { path = "foo" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.5.0"))
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("build").run();

    assert!(p.root().join("target/debug/libtop.rlib").is_file());
    assert!(!p.root().join("target/debug/libfoo.rlib").is_file());
}

#[cargo_test]
fn deep_path_error() {
    // Test for an error loading a path deep in the dependency graph.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"
            [dependencies]
            a = {path="a"}
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
             [dependencies]
             b = {path="../b"}
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
             [package]
             name = "b"
             version = "0.1.0"
             edition = "2015"
             [dependencies]
             c = {path="../c"}
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to get `c` as a dependency of package `b v0.1.0 ([ROOT]/foo/b)`
    ... which satisfies path dependency `b` of package `a v0.1.0 ([ROOT]/foo/a)`
    ... which satisfies path dependency `a` of package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `c`

Caused by:
  Unable to update [ROOT]/foo/c

Caused by:
  failed to read `[ROOT]/foo/c/Cargo.toml`

Caused by:
  [NOT_FOUND]

"#]])
        .run();
}

#[cargo_test]
fn catch_tricky_cycle() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "message"
                version = "0.1.0"
                edition = "2015"

                [dev-dependencies]
                test = { path = "test" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "tangle/Cargo.toml",
            r#"
                [package]
                name = "tangle"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                message = { path = ".." }
                snapshot = { path = "../snapshot" }
            "#,
        )
        .file("tangle/src/lib.rs", "")
        .file(
            "snapshot/Cargo.toml",
            r#"
                [package]
                name = "snapshot"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                ledger = { path = "../ledger" }
            "#,
        )
        .file("snapshot/src/lib.rs", "")
        .file(
            "ledger/Cargo.toml",
            r#"
                [package]
                name = "ledger"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                tangle = { path = "../tangle" }
            "#,
        )
        .file("ledger/src/lib.rs", "")
        .file(
            "test/Cargo.toml",
            r#"
                [package]
                name = "test"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                snapshot = { path = "../snapshot" }
            "#,
        )
        .file("test/src/lib.rs", "")
        .build();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[ERROR] cyclic package dependency: package `ledger v0.1.0 ([ROOT]/foo/ledger)` depends on itself. Cycle:
package `ledger v0.1.0 ([ROOT]/foo/ledger)`
...
"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn same_name_version_changed() {
    // Illustrates having two path packages with the same name, but different versions.
    // Verifies it works correctly when one of the versions is changed.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2021"

                [dependencies]
                foo2 = { path = "foo2", package = "foo" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo2/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "2.0.0"
                edition = "2021"
            "#,
        )
        .file("foo2/src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version

"#]])
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
└── foo v2.0.0 ([ROOT]/foo/foo2)

"#]])
        .run();

    p.change_file(
        "foo2/Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "2.0.1"
            edition = "2021"
        "#,
    );
    p.cargo("tree")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[ADDING] foo v2.0.1 ([ROOT]/foo/foo2)

"#]])
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
└── foo v2.0.1 ([ROOT]/foo/foo2)

"#]])
        .run();
}
