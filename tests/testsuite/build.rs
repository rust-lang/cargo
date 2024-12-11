//! Tests for the `cargo build` command.

use std::env;
use std::fs;
use std::io::Read;
use std::process::Stdio;

use cargo::{
    core::compiler::CompileMode,
    core::{Shell, Workspace},
    ops::CompileOptions,
    GlobalContext,
};
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::paths::root;
use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{
    basic_bin_manifest, basic_lib_manifest, basic_manifest, cargo_exe, cargo_process, git,
    is_nightly, main_file, paths, process, project, rustc_host, sleep_ms, symlink_supported, t,
    tools, Execs, ProjectBuilder,
};
use cargo_util::paths::dylib_path_envvar;

#[cargo_test]
fn cargo_compile_simple() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
i am foo

"#]])
        .run();
}

#[cargo_test]
fn build_with_symlink_to_path_dependency_with_build_script_in_git() {
    if !symlink_supported() {
        return;
    }

    let root = paths::root();
    git::repo(&root)
        .nocommit_file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

               [dependencies]
               # the path leads through a symlink, 'symlink-to-original' is a worktree root,
               # and symlink-to-dir/ is a symlink to a sub-directory to be stepped through.
               lib = { version = "0.1.0", path = "symlink-to-original/symlink-to-dir/lib" }
            "#,
        )
        .nocommit_file("src/main.rs", "fn main() { }")
        .nocommit_file("original/dir/lib/build.rs", "fn main() {}")
        .nocommit_file(
            "original/dir/lib/Cargo.toml",
            r#"
                [package]
                name = "lib"
                version = "0.1.0"
                edition = "2021"
              "#,
        )
        .nocommit_file("original/dir/lib/src/lib.rs", "")
        .nocommit_symlink_dir("original", "symlink-to-original")
        .nocommit_symlink_dir("original/dir", "original/symlink-to-dir")
        .build();

    // It is necessary to have a sub-repository and to add files so there is an index.
    let repo = git::init(&root.join("original"));
    git::add(&repo);
    cargo_process("build").run();
}

#[cargo_test]
fn cargo_fail_with_no_stderr() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", &String::from("refusal"))
        .build();
    p.cargo("build --message-format=json")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error

"#]])
        .run();
}

/// Checks that the `CARGO_INCREMENTAL` environment variable results in
/// `rustc` getting `-C incremental` passed to it.
#[cargo_test]
fn cargo_compile_incremental() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build -v")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc [..] -C incremental=[ROOT]/foo/target/debug/incremental[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("test -v")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc [..] -C incremental=[ROOT]/foo/target/debug/incremental[..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`

"#]])
        .run();
}

#[cargo_test]
fn incremental_profile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [profile.dev]
                incremental = false

                [profile.release]
                incremental = true
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .env_remove("CARGO_INCREMENTAL")
        .with_stderr_does_not_contain("[..]C incremental=[..]")
        .run();

    p.cargo("build -v")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_contains("[..]C incremental=[..]")
        .run();

    p.cargo("build --release -v")
        .env_remove("CARGO_INCREMENTAL")
        .with_stderr_contains("[..]C incremental=[..]")
        .run();

    p.cargo("build --release -v")
        .env("CARGO_INCREMENTAL", "0")
        .with_stderr_does_not_contain("[..]C incremental=[..]")
        .run();
}

#[cargo_test]
fn incremental_config() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                incremental = false
            "#,
        )
        .build();

    p.cargo("build -v")
        .env_remove("CARGO_INCREMENTAL")
        .with_stderr_does_not_contain("[..]C incremental=[..]")
        .run();

    p.cargo("build -v")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_contains("[..]C incremental=[..]")
        .run();
}

#[cargo_test]
fn cargo_compile_with_redundant_default_mode() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build --debug")
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '--debug' found

  tip: `--debug` is the default for `cargo build`; instead `--release` is supported

Usage: cargo[EXE] build [OPTIONS]

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn cargo_compile_with_unsupported_short_config_flag() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build -c net.git-fetch-with-cli=true")
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '-c' found

  tip: a similar argument exists: '--config'

Usage: cargo[EXE] build [OPTIONS]

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn cargo_compile_with_workspace_excluded() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("build --workspace --exclude foo")
        .with_stderr_data(str![[r#"
[ERROR] no packages to compile

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn cargo_compile_manifest_path() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build --manifest-path foo/Cargo.toml")
        .cwd(p.root().parent().unwrap())
        .run();
    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn cargo_compile_with_wrong_manifest_path_flag() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build --path foo/Cargo.toml")
        .cwd(p.root().parent().unwrap())
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '--path' found

  tip: a similar argument exists: '--manifest-path'

Usage: cargo[EXE] build [OPTIONS]

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn chdir_gated() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .build();
    p.cargo("-C foo build")
        .cwd(p.root().parent().unwrap())
        .with_stderr_data(str![[r#"
[ERROR] the `-C` flag is unstable, pass `-Z unstable-options` on the nightly channel to enable it

"#]])
        .with_status(101)
        .run();
    // No masquerade should also fail.
    p.cargo("-C foo -Z unstable-options build")
        .cwd(p.root().parent().unwrap())
        .with_stderr_data(str![[r#"
[ERROR] the `-C` flag is unstable, pass `-Z unstable-options` on the nightly channel to enable it

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn cargo_compile_directory_not_cwd() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file(".cargo/config.toml", &"")
        .build();

    p.cargo("-Zunstable-options -C foo build")
        .masquerade_as_nightly_cargo(&["chdir"])
        .cwd(p.root().parent().unwrap())
        .run();
    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn cargo_compile_with_unsupported_short_unstable_feature_flag() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file(".cargo/config.toml", &"")
        .build();

    p.cargo("-zunstable-options -C foo build")
        .masquerade_as_nightly_cargo(&["chdir"])
        .cwd(p.root().parent().unwrap())
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '-z' found

  tip: a similar argument exists: '-Z'

Usage: cargo [..][OPTIONS] [COMMAND]
       cargo [..][OPTIONS] -Zscript <MANIFEST_RS> [ARGS]...

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn cargo_compile_directory_not_cwd_with_invalid_config() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file(".cargo/config.toml", &"!")
        .build();

    p.cargo("-Zunstable-options -C foo build")
        .masquerade_as_nightly_cargo(&["chdir"])
        .cwd(p.root().parent().unwrap())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not load Cargo configuration

Caused by:
  could not parse TOML configuration in `[ROOT]/foo/.cargo/config.toml`

Caused by:
  TOML parse error at line 1, column 1
    |
  1 | !
    | ^
  invalid key

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_manifest() {
    let p = project().file("Cargo.toml", "").build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  manifest is missing either a `[package]` or a `[workspace]`

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_manifest2() {
    let p = project()
        .file(
            "Cargo.toml",
            "
                [package]
                foo = bar
            ",
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid string
expected `"`, `'`
 --> Cargo.toml:3:23
  |
3 |                 foo = bar
  |                       ^
  |

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_manifest3() {
    let p = project().file("src/Cargo.toml", "a = bar").build();

    p.cargo("build --manifest-path src/Cargo.toml")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid string
expected `"`, `'`
 --> src/Cargo.toml:1:5
  |
1 | a = bar
  |     ^
  |

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_duplicate_build_targets() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lib]
                name = "main"
                path = "src/main.rs"
                crate-type = ["dylib"]

                [dependencies]
            "#,
        )
        .file("src/main.rs", "#![allow(warnings)] fn main() {}")
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[WARNING] file `[ROOT]/foo/src/main.rs` found to be present in multiple build targets:
  * `lib` target `main`
  * `bin` target `foo`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_version() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0"))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unexpected end of input while parsing minor version number
 --> Cargo.toml:4:19
  |
4 |         version = "1.0"
  |                   ^^^^^
  |

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_empty_package_name() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("", "0.0.0"))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package name cannot be empty
 --> Cargo.toml:3:16
  |
3 |         name = ""
  |                ^^
  |

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_package_name() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo@bar", "0.0.0"))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid character `@` in package name: `foo@bar`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
 --> Cargo.toml:3:16
  |
3 |         name = "foo@bar"
  |                ^^^^^^^^^
  |

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_bin_target_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                edition = "2015"

                [[bin]]
                name = ""
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  binary target names cannot be empty

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_forbidden_bin_target_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                edition = "2015"

                [[bin]]
                name = "build"
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the binary target name `build` is forbidden, it conflicts with cargo's build directory names

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_bin_and_crate_type() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                edition = "2015"

                [[bin]]
                name = "the_foo_bin"
                path = "src/foo.rs"
                crate-type = ["cdylib", "rlib"]
            "#,
        )
        .file("src/foo.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the target `the_foo_bin` is a binary and can't have any crate-types set (currently "cdylib, rlib")

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_api_exposes_artifact_paths() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                edition = "2015"

                [[bin]]
                name = "the_foo_bin"
                path = "src/bin.rs"

                [lib]
                name = "the_foo_lib"
                path = "src/foo.rs"
                crate-type = ["cdylib", "rlib"]
            "#,
        )
        .file("src/foo.rs", "pub fn bar() {}")
        .file("src/bin.rs", "pub fn main() {}")
        .build();

    let shell = Shell::from_write(Box::new(Vec::new()));
    let gctx = GlobalContext::new(shell, env::current_dir().unwrap(), paths::home());
    let ws = Workspace::new(&p.root().join("Cargo.toml"), &gctx).unwrap();
    let compile_options = CompileOptions::new(ws.gctx(), CompileMode::Build).unwrap();

    let result = cargo::ops::compile(&ws, &compile_options).unwrap();

    assert_eq!(1, result.binaries.len());
    assert!(result.binaries[0].path.exists());
    assert!(result.binaries[0]
        .path
        .to_str()
        .unwrap()
        .contains("the_foo_bin"));

    assert_eq!(1, result.cdylibs.len());
    // The exact library path varies by platform, but should certainly exist at least
    assert!(result.cdylibs[0].path.exists());
    assert!(result.cdylibs[0]
        .path
        .to_str()
        .unwrap()
        .contains("the_foo_lib"));
}

#[cargo_test]
fn cargo_compile_with_bin_and_proc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                edition = "2015"

                [[bin]]
                name = "the_foo_bin"
                path = "src/foo.rs"
                proc-macro = true
            "#,
        )
        .file("src/foo.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the target `the_foo_bin` is a binary and can't have `proc-macro` set `true`

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_lib_target_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                edition = "2015"

                [lib]
                name = ""
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  library target names cannot be empty

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_non_numeric_dep_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                crossbeam = "y"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  failed to parse the version requirement `y` for dependency `crossbeam`

Caused by:
  unexpected character 'y' while parsing major version number

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_without_manifest() {
    let p = project().no_manifest().build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not find `Cargo.toml` in `[ROOT]/foo` or any parent directory

"#]])
        .run();
}

#[cargo_test]
#[cfg(target_os = "linux")]
fn cargo_compile_with_lowercase_cargo_toml() {
    let p = project()
        .no_manifest()
        .file("cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not find `Cargo.toml` in `[ROOT]/foo` or any parent directory, but found cargo.toml please try to rename it to Cargo.toml

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_code() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "invalid rust code!")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[ERROR] [..]
...
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error

"#]])
        .run();
    assert!(p.root().join("Cargo.lock").is_file());
}

#[cargo_test]
fn cargo_compile_with_invalid_code_in_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"
                [dependencies.baz]
                path = "../baz"
            "#,
        )
        .file("src/main.rs", "invalid rust code!")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "invalid rust code!")
        .build();
    let _baz = project()
        .at("baz")
        .file("Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("src/lib.rs", "invalid rust code!")
        .build();
    p.cargo("build")
        .with_status(101)
        .with_stderr_data(
            str![[r#"
[COMPILING] bar v0.1.0 ([ROOT]/bar)
[COMPILING] baz v0.1.0 ([ROOT]/baz)
[ERROR] could not compile `bar` (lib) due to 1 previous error
[ERROR] could not compile `baz` (lib) due to 1 previous error
...

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_warnings_in_the_root_package() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {} fn dead() {}")
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[WARNING] [..]dead[..]
...
[WARNING] `foo` (bin "foo") generated 1 warning
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_warnings_in_a_dep_package() {
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

                [[bin]]

                name = "foo"
            "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file(
            "bar/src/bar.rs",
            r#"
                pub fn gimme() -> &'static str {
                    "test passed"
                }

                fn dead() {}
            "#,
        )
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[WARNING] [..]dead[..]
...
[WARNING] `bar` (lib) generated 1 warning
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
}

#[cargo_test]
fn cargo_compile_with_nested_deps_inferred() {
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
                path = 'bar'

                [[bin]]
                name = "foo"
            "#,
        )
        .file("src/foo.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
                [package]

                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies.baz]
                path = "../baz"
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                extern crate baz;

                pub fn gimme() -> String {
                    baz::gimme()
                }
            "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.5.0"))
        .file(
            "baz/src/lib.rs",
            r#"
                pub fn gimme() -> String {
                    "test passed".to_string()
                }
            "#,
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    assert!(!p.bin("libbar.rlib").is_file());
    assert!(!p.bin("libbaz.rlib").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
test passed

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_nested_deps_correct_bin() {
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

                [[bin]]
                name = "foo"
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
                path = "../baz"
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                extern crate baz;

                pub fn gimme() -> String {
                    baz::gimme()
                }
            "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.5.0"))
        .file(
            "baz/src/lib.rs",
            r#"
                pub fn gimme() -> String {
                    "test passed".to_string()
                }
            "#,
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    assert!(!p.bin("libbar.rlib").is_file());
    assert!(!p.bin("libbaz.rlib").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
test passed

"#]])
        .run();
}

#[cargo_test]
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
                path = "../baz"

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
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file(
            "baz/src/baz.rs",
            r#"
                pub fn gimme() -> String {
                    "test passed".to_string()
                }
            "#,
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    assert!(!p.bin("libbar.rlib").is_file());
    assert!(!p.bin("libbaz.rlib").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
test passed

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_nested_deps_longhand() {
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
                version = "0.5.0"
                edition = "2015"

                [[bin]]

                name = "foo"
            "#,
        )
        .file("src/foo.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
                [package]

                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies.baz]
                path = "../baz"
                version = "0.5.0"
                edition = "2015"

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
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file(
            "baz/src/baz.rs",
            r#"
                pub fn gimme() -> String {
                    "test passed".to_string()
                }
            "#,
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    assert!(!p.bin("libbar.rlib").is_file());
    assert!(!p.bin("libbaz.rlib").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
test passed

"#]])
        .run();
}

// Check that Cargo gives a sensible error if a dependency can't be found
// because of a name mismatch.
#[cargo_test]
fn cargo_compile_with_dep_name_mismatch() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = ["wycats@example.com"]

                [[bin]]

                name = "foo"

                [dependencies.notquitebar]

                path = "bar"
            "#,
        )
        .file("src/bin/foo.rs", &main_file(r#""i am foo""#, &["bar"]))
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/bar.rs", &main_file(r#""i am bar""#, &[]))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no matching package named `notquitebar` found
location searched: [ROOT]/foo/bar
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]])
        .run();
}

// Ensure that renamed deps have a valid name
#[cargo_test]
fn cargo_compile_with_invalid_dep_rename() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "buggin"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                "haha this isn't a valid name 🐛" = { package = "libc", version = "0.1" }
            "#,
        )
        .file("src/main.rs", &main_file(r#""What's good?""#, &[]))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid character ` ` in package name: `haha this isn't a valid name 🐛`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
 --> Cargo.toml:8:17
  |
8 |                 "haha this isn't a valid name 🐛" = { package = "libc", version = "0.1" }
  |                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_filename() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "src/bin/a.rs",
            r#"
                extern crate foo;
                fn main() { println!("hello a.rs"); }
            "#,
        )
        .file("examples/a.rs", r#"fn main() { println!("example"); }"#)
        .build();

    p.cargo("build --bin bin.rs")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target named `bin.rs`.
Available bin targets:
    a


"#]])
        .run();

    p.cargo("build --bin a.rs")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target named `a.rs`

	Did you mean `a`?

"#]])
        .run();

    p.cargo("build --example example.rs")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no example target named `example.rs`.
Available example targets:
    a


"#]])
        .run();

    p.cargo("build --example a.rs")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no example target named `a.rs`

	Did you mean `a`?

"#]])
        .run();
}

#[cargo_test]
fn incompatible_dependencies() {
    Package::new("bad", "0.1.0").publish();
    Package::new("bad", "1.0.0").publish();
    Package::new("bad", "1.0.1").publish();
    Package::new("bad", "1.0.2").publish();
    Package::new("bar", "0.1.0").dep("bad", "0.1.0").publish();
    Package::new("baz", "0.1.1").dep("bad", "=1.0.0").publish();
    Package::new("baz", "0.1.0").dep("bad", "=1.0.0").publish();
    Package::new("qux", "0.1.2").dep("bad", ">=1.0.1").publish();
    Package::new("qux", "0.1.1").dep("bad", ">=1.0.1").publish();
    Package::new("qux", "0.1.0").dep("bad", ">=1.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = "0.1.0"
                baz = "0.1.0"
                qux = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for `bad`.
    ... required by package `qux v0.1.0`
    ... which satisfies dependency `qux = "^0.1.0"` of package `foo v0.0.1 ([ROOT]/foo)`
versions that meet the requirements `>=1.0.1` are: 1.0.2, 1.0.1

all possible versions conflict with previously selected packages.

  previously selected package `bad v1.0.0`
    ... which satisfies dependency `bad = "=1.0.0"` of package `baz v0.1.0`
    ... which satisfies dependency `baz = "^0.1.0"` of package `foo v0.0.1 ([ROOT]/foo)`

failed to select a version for `bad` which could resolve this conflict

"#]])
        .run();
}

#[cargo_test]
fn incompatible_dependencies_with_multi_semver() {
    Package::new("bad", "1.0.0").publish();
    Package::new("bad", "1.0.1").publish();
    Package::new("bad", "2.0.0").publish();
    Package::new("bad", "2.0.1").publish();
    Package::new("bar", "0.1.0").dep("bad", "=1.0.0").publish();
    Package::new("baz", "0.1.0").dep("bad", ">=2.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = "0.1.0"
                baz = "0.1.0"
                bad = ">=1.0.1, <=2.0.0"
            "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for `bad`.
    ... required by package `foo v0.0.1 ([ROOT]/foo)`
versions that meet the requirements `>=1.0.1, <=2.0.0` are: 2.0.0, 1.0.1

all possible versions conflict with previously selected packages.

  previously selected package `bad v2.0.1`
    ... which satisfies dependency `bad = ">=2.0.1"` of package `baz v0.1.0`
    ... which satisfies dependency `baz = "^0.1.0"` of package `foo v0.0.1 ([ROOT]/foo)`

  previously selected package `bad v1.0.0`
    ... which satisfies dependency `bad = "=1.0.0"` of package `bar v0.1.0`
    ... which satisfies dependency `bar = "^0.1.0"` of package `foo v0.0.1 ([ROOT]/foo)`

failed to select a version for `bad` which could resolve this conflict

"#]])
        .run();
}

#[cargo_test]
fn compile_path_dep_then_change_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build").run();

    p.change_file("bar/Cargo.toml", &basic_manifest("bar", "0.0.2"));

    p.cargo("build").run();
}

#[cargo_test]
fn ignores_carriage_return_in_lockfile() {
    let p = project()
        .file("src/main.rs", "mod a; fn main() {}")
        .file("src/a.rs", "")
        .build();

    p.cargo("build").run();

    let lock = p.read_lockfile();
    p.change_file("Cargo.lock", &lock.replace("\n", "\r\n"));
    p.cargo("build").run();
}

#[cargo_test]
fn cargo_default_env_metadata_env_var() {
    // Ensure that path dep + dylib + env_var get metadata
    // (even though path_dep + dylib should not)
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/lib.rs", "// hi")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lib]
                name = "bar"
                crate-type = ["dylib"]
            "#,
        )
        .file("bar/src/lib.rs", "// hello")
        .build();

    let dll_prefix = env::consts::DLL_PREFIX;
    let dll_suffix = env::consts::DLL_SUFFIX;

    // No metadata on libbar since it's a dylib path dependency
    p.cargo("build -v")
        .with_stderr_data(format!(
            "\
...
[RUNNING] `rustc --crate-name foo [..]--extern bar=[ROOT]/foo/target/debug/deps/{dll_prefix}bar{dll_suffix}`
...
"))
        .run();

    p.cargo("clean").run();

    // If you set the env-var, then we expect metadata on libbar
    p.cargo("build -v")
        .env("__CARGO_DEFAULT_LIB_METADATA", "stable")
        .with_stderr_data(format!(
            "\
...
[RUNNING] `rustc --crate-name foo [..]--extern bar=[ROOT]/foo/target/debug/deps/{dll_prefix}bar-[..]{dll_suffix}`
...
"))
        .run();
}

#[cargo_test]
fn crate_env_vars() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.5.1-alpha.1"
            edition = "2015"
            description = "This is foo"
            homepage = "https://example.com"
            repository = "https://example.com/repo.git"
            authors = ["wycats@example.com"]
            license = "MIT OR Apache-2.0"
            license-file = "license.txt"
            rust-version = "1.61.0"
            readme = "../../README.md"

            [[bin]]
            name = "foo-bar"
            path = "src/main.rs"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                extern crate foo;


                static VERSION_MAJOR: &'static str = env!("CARGO_PKG_VERSION_MAJOR");
                static VERSION_MINOR: &'static str = env!("CARGO_PKG_VERSION_MINOR");
                static VERSION_PATCH: &'static str = env!("CARGO_PKG_VERSION_PATCH");
                static VERSION_PRE: &'static str = env!("CARGO_PKG_VERSION_PRE");
                static VERSION: &'static str = env!("CARGO_PKG_VERSION");
                static CARGO_MANIFEST_DIR: &'static str = env!("CARGO_MANIFEST_DIR");
                static CARGO_MANIFEST_PATH: &'static str = env!("CARGO_MANIFEST_PATH");
                static PKG_NAME: &'static str = env!("CARGO_PKG_NAME");
                static HOMEPAGE: &'static str = env!("CARGO_PKG_HOMEPAGE");
                static REPOSITORY: &'static str = env!("CARGO_PKG_REPOSITORY");
                static LICENSE: &'static str = env!("CARGO_PKG_LICENSE");
                static LICENSE_FILE: &'static str = env!("CARGO_PKG_LICENSE_FILE");
                static DESCRIPTION: &'static str = env!("CARGO_PKG_DESCRIPTION");
                static RUST_VERSION: &'static str = env!("CARGO_PKG_RUST_VERSION");
                static README: &'static str = env!("CARGO_PKG_README");
                static BIN_NAME: &'static str = env!("CARGO_BIN_NAME");
                static CRATE_NAME: &'static str = env!("CARGO_CRATE_NAME");


                fn main() {
                    let s = format!("{}-{}-{} @ {} in {} file {}", VERSION_MAJOR,
                                    VERSION_MINOR, VERSION_PATCH, VERSION_PRE,
                                    CARGO_MANIFEST_DIR, CARGO_MANIFEST_PATH);
                     assert_eq!(s, foo::version());
                     println!("{}", s);
                     assert_eq!("foo", PKG_NAME);
                     assert_eq!("foo-bar", BIN_NAME);
                     assert_eq!("foo_bar", CRATE_NAME);
                     assert_eq!("https://example.com", HOMEPAGE);
                     assert_eq!("https://example.com/repo.git", REPOSITORY);
                     assert_eq!("MIT OR Apache-2.0", LICENSE);
                     assert_eq!("license.txt", LICENSE_FILE);
                     assert_eq!("This is foo", DESCRIPTION);
                     assert_eq!("1.61.0", RUST_VERSION);
                     assert_eq!("../../README.md", README);
                    let s = format!("{}.{}.{}-{}", VERSION_MAJOR,
                                    VERSION_MINOR, VERSION_PATCH, VERSION_PRE);
                    assert_eq!(s, VERSION);

                    // Verify CARGO_TARGET_TMPDIR isn't set for bins
                    assert!(option_env!("CARGO_TARGET_TMPDIR").is_none());
                }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                use std::env;
                use std::path::PathBuf;

                pub fn version() -> String {
                    format!("{}-{}-{} @ {} in {} file {}",
                            env!("CARGO_PKG_VERSION_MAJOR"),
                            env!("CARGO_PKG_VERSION_MINOR"),
                            env!("CARGO_PKG_VERSION_PATCH"),
                            env!("CARGO_PKG_VERSION_PRE"),
                            env!("CARGO_MANIFEST_DIR"),
                            env!("CARGO_MANIFEST_PATH"))
                }

                pub fn check_no_int_test_env() {
                    env::var("CARGO_TARGET_DIR").unwrap_err();
                }

                pub fn check_tmpdir(tmp: Option<&'static str>) {
                    let tmpdir: PathBuf = tmp.unwrap().into();

                    let exe: PathBuf = env::current_exe().unwrap().into();
                    let mut expected: PathBuf = exe.parent().unwrap()
                        .parent().unwrap()
                        .parent().unwrap()
                        .into();
                    expected.push("tmp");
                    assert_eq!(tmpdir, expected);

                    // Check that CARGO_TARGET_TMPDIR isn't set for lib code
                    assert!(option_env!("CARGO_TARGET_TMPDIR").is_none());
                    env::var("CARGO_TARGET_TMPDIR").unwrap_err();
                }

                #[test]
                fn unit_env_cargo_target_tmpdir() {
                    // Check that CARGO_TARGET_TMPDIR isn't set for unit tests
                    assert!(option_env!("CARGO_TARGET_TMPDIR").is_none());
                    env::var("CARGO_TARGET_TMPDIR").unwrap_err();
                }
            "#,
        )
        .file(
            "examples/ex-env-vars.rs",
            r#"
                static PKG_NAME: &'static str = env!("CARGO_PKG_NAME");
                static BIN_NAME: &'static str = env!("CARGO_BIN_NAME");
                static CRATE_NAME: &'static str = env!("CARGO_CRATE_NAME");

                fn main() {
                    assert_eq!("foo", PKG_NAME);
                    assert_eq!("ex-env-vars", BIN_NAME);
                    assert_eq!("ex_env_vars", CRATE_NAME);

                    // Verify CARGO_TARGET_TMPDIR isn't set for examples
                    assert!(option_env!("CARGO_TARGET_TMPDIR").is_none());
                }
            "#,
        )
        .file(
            "tests/env.rs",
            r#"
                #[test]
                fn integration_env_cargo_target_tmpdir() {
                    foo::check_tmpdir(option_env!("CARGO_TARGET_TMPDIR"));
                }
            "#,
        );

    let p = if is_nightly() {
        p.file(
            "benches/env.rs",
            r#"
                #![feature(test)]
                extern crate test;
                use test::Bencher;

                #[bench]
                fn bench_env_cargo_target_tmpdir(_: &mut Bencher) {
                    foo::check_tmpdir(option_env!("CARGO_TARGET_TMPDIR"));
                }
            "#,
        )
        .build()
    } else {
        p.build()
    };

    println!("build");
    p.cargo("build -v").run();

    println!("bin");
    p.process(&p.bin("foo-bar"))
        .with_stdout_data(str![[r#"
0-5-1 @ alpha.1 in [ROOT]/foo file [ROOT]/foo/Cargo.toml

"#]])
        .run();

    println!("example");
    p.cargo("run --example ex-env-vars -v").run();

    println!("test");
    p.cargo("test -v").run();

    if is_nightly() {
        println!("bench");
        p.cargo("bench -v").run();
    }
}

#[cargo_test]
fn crate_authors_env_vars() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.1-alpha.1"
                edition = "2015"
                authors = ["wycats@example.com", "neikos@example.com"]
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                extern crate foo;

                static AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");

                fn main() {
                    let s = "wycats@example.com:neikos@example.com";
                    assert_eq!(AUTHORS, foo::authors());
                    println!("{}", AUTHORS);
                    assert_eq!(s, AUTHORS);
                }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn authors() -> String {
                    format!("{}", env!("CARGO_PKG_AUTHORS"))
                }
            "#,
        )
        .build();

    println!("build");
    p.cargo("build -v").run();

    println!("bin");
    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
wycats@example.com:neikos@example.com

"#]])
        .run();

    println!("test");
    p.cargo("test -v").run();
}

#[cargo_test]
fn vv_prints_rustc_env_vars() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = ["escape='\"@example.com"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let mut b = p.cargo("build -vv");

    #[cfg(windows)]
    {
        b.with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] CARGO_PKG_AUTHORS="escape='/"@example.com"&& [..] set CARGO_PKG_NAME=foo&& [..] rustc [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
    }

    #[cfg(not(windows))]
    {
        b.with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] CARGO_PKG_AUTHORS='escape='/''"@example.com' [..] CARGO_PKG_NAME=foo [..] rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
    }

    b.run();
}

// The tester may already have LD_LIBRARY_PATH=::/foo/bar which leads to a false positive error
fn setenv_for_removing_empty_component(mut execs: Execs) -> Execs {
    let v = dylib_path_envvar();
    if let Ok(search_path) = env::var(v) {
        let new_search_path =
            env::join_paths(env::split_paths(&search_path).filter(|e| !e.as_os_str().is_empty()))
                .expect("join_paths");
        execs.env(v, new_search_path); // build_command() will override LD_LIBRARY_PATH accordingly
    }
    execs
}

// Regression test for #4277
#[cargo_test]
fn crate_library_path_env_var() {
    let p = project()
        .file(
            "src/main.rs",
            &format!(
                r#"
                    fn main() {{
                        let search_path = env!("{}");
                        let paths = std::env::split_paths(&search_path).collect::<Vec<_>>();
                        assert!(!paths.contains(&"".into()));
                    }}
                "#,
                dylib_path_envvar()
            ),
        )
        .build();

    setenv_for_removing_empty_component(p.cargo("run")).run();
}

// See https://github.com/rust-lang/cargo/issues/14194
#[cargo_test]
fn issue_14194_deduplicate_library_path_env_var() {
    let p = project()
        .file(
            "src/main.rs",
            &format!(
                r#"
                    use std::process::Command;
                    fn main() {{
                        let level: i32 = std::env::args().nth(1).unwrap().parse().unwrap();
                        let txt = "var.txt";
                        let lib_path = std::env::var("{}").unwrap();

                        // Make sure we really have something in dylib search path.
                        let count = std::env::split_paths(&lib_path).count();
                        assert!(count > 0);

                        if level >= 3 {{
                            std::fs::write(txt, &lib_path).unwrap();
                        }} else {{
                            let prev_lib_path = std::fs::read_to_string(txt).unwrap();
                            // Ensure no duplicate insertion to dylib search paths
                            // when calling `cargo run` recursively.
                            assert_eq!(lib_path, prev_lib_path);
                        }}

                        if level == 0 {{
                            return;
                        }}
                        
                        let _  = Command::new(std::env!("CARGO"))
                        .arg("run")
                        .arg("--")
                        .arg((level - 1).to_string())
                        .status()
                        .unwrap();
                    }}
                "#,
                dylib_path_envvar(),
            ),
        )
        .build();

    setenv_for_removing_empty_component(p.cargo("run -- 3"))
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE] 3`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE] 2`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE] 1`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE] 0`

"#]])
        .run();
}

// Regression test for #4277
#[cargo_test]
fn build_with_fake_libc_not_loading() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .file("libc.so.6", r#""#)
        .build();

    setenv_for_removing_empty_component(p.cargo("build")).run();
}

// this is testing that src/<pkg-name>.rs still works (for now)
#[cargo_test]
fn many_crate_types_old_style_lib_location() {
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
            "#,
        )
        .file("src/foo.rs", "pub fn foo() {}")
        .build();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[WARNING] path `src/foo.rs` was erroneously implicitly accepted for library `foo`,
please rename the file to `src/lib.rs` or set lib.path in Cargo.toml
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert!(p.root().join("target/debug/libfoo.rlib").is_file());
    let fname = format!("{}foo{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX);
    assert!(p.root().join("target/debug").join(&fname).is_file());
}

#[cargo_test]
fn many_crate_types_correct() {
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
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();
    p.cargo("build").run();

    assert!(p.root().join("target/debug/libfoo.rlib").is_file());
    let fname = format!("{}foo{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX);
    assert!(p.root().join("target/debug").join(&fname).is_file());
}

#[cargo_test]
fn set_both_dylib_and_cdylib_crate_types() {
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
                crate-type = ["cdylib", "dylib"]
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();
    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  library `foo` cannot set the crate type of both `dylib` and `cdylib`

"#]])
        .run();
}

#[cargo_test]
fn self_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "test"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies.test]

                path = "."

                [lib]
                name = "test"
                path = "src/test.rs"
            "#,
        )
        .file("src/test.rs", "fn main() {}")
        .build();
    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cyclic package dependency: package `test v0.0.0 ([ROOT]/foo)` depends on itself. Cycle:
package `test v0.0.0 ([ROOT]/foo)`
    ... which satisfies path dependency `test` of package `test v0.0.0 ([ROOT]/foo)`

"#]])
        .run();
}

#[cargo_test]
/// Make sure broken and loop symlinks don't break the build
///
/// This test requires you to be able to make symlinks.
/// For windows, this may require you to enable developer mode.
fn ignore_broken_symlinks() {
    if !symlink_supported() {
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .symlink("Notafile", "bar")
        // To hit the symlink directory, we need a build script
        // to trigger a full scan of package files.
        .file("build.rs", &main_file(r#""build script""#, &[]))
        .symlink_dir("a/b", "a/b/c/d/foo")
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[WARNING] File system loop found: [ROOT]/foo/a/b/c/d/foo points to an ancestor [ROOT]/foo/a/b
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
i am foo

"#]])
        .run();
}

#[cargo_test]
fn missing_lib_and_bin() {
    let p = project().build();
    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  no targets specified in the manifest
  either src/lib.rs, src/main.rs, a [lib] section, or [[bin]] section must be present

"#]])
        .run();
}

#[cargo_test]
fn lto_build() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "test"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [profile.release]
                lto = true
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("build -v --release")
        .with_stderr_data(str![[r#"
[COMPILING] test v0.0.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name test --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link -C opt-level=3 -C lto [..]`
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn explicit_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                edition = "2015"
                authors = []

                [lib]
                name = "foo"
                path = "src/lib.rs"

                [[example]]
                name = "hello"
                path = "examples/ex-hello.rs"

                [[example]]
                name = "goodbye"
                path = "examples/ex-goodbye.rs"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn get_hello() -> &'static str { "Hello" }
                pub fn get_goodbye() -> &'static str { "Goodbye" }
                pub fn get_world() -> &'static str { "World" }
            "#,
        )
        .file(
            "examples/ex-hello.rs",
            r#"
                extern crate foo;
                fn main() { println!("{}, {}!", foo::get_hello(), foo::get_world()); }
            "#,
        )
        .file(
            "examples/ex-goodbye.rs",
            r#"
                extern crate foo;
                fn main() { println!("{}, {}!", foo::get_goodbye(), foo::get_world()); }
            "#,
        )
        .build();

    p.cargo("build --examples").run();
    p.process(&p.bin("examples/hello"))
        .with_stdout_data(str![[r#"
Hello, World!

"#]])
        .run();
    p.process(&p.bin("examples/goodbye"))
        .with_stdout_data(str![[r#"
Goodbye, World!

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

    p.cargo("build --tests -v")
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

    p.cargo("build --examples -v")
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

    p.cargo("build --benches -v")
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

    p.cargo("build -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `foo` bin at `src/bin/foo.rs` or `src/bin/foo/main.rs`. Please specify bin.path if you want to use a non-default path.

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

    p.cargo("build --tests -v")
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

    p.cargo("build --examples -v")
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

    p.cargo("build --benches -v")
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

    p.cargo("build -v")
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

    p.cargo("build -v")
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

    p.cargo("build -v")
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

    p.cargo("build -v")
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
[COMPILING] foo v1.0.0 ([ROOT]/foo)
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

    p.cargo("build -v")
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
[COMPILING] foo v1.0.0 ([ROOT]/foo)
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

    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[WARNING] path `src/bar.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml
[COMPILING] foo v1.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn implicit_examples() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn get_hello() -> &'static str { "Hello" }
                pub fn get_goodbye() -> &'static str { "Goodbye" }
                pub fn get_world() -> &'static str { "World" }
            "#,
        )
        .file(
            "examples/hello.rs",
            r#"
                extern crate foo;
                fn main() {
                    println!("{}, {}!", foo::get_hello(), foo::get_world());
                }
            "#,
        )
        .file(
            "examples/goodbye.rs",
            r#"
                extern crate foo;
                fn main() {
                    println!("{}, {}!", foo::get_goodbye(), foo::get_world());
                }
            "#,
        )
        .build();

    p.cargo("build --examples").run();
    p.process(&p.bin("examples/hello"))
        .with_stdout_data(str![[r#"
Hello, World!

"#]])
        .run();
    p.process(&p.bin("examples/goodbye"))
        .with_stdout_data(str![[r#"
Goodbye, World!

"#]])
        .run();
}

#[cargo_test]
fn standard_build_no_ndebug() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/foo.rs",
            r#"
                fn main() {
                    if cfg!(debug_assertions) {
                        println!("slow")
                    } else {
                        println!("fast")
                    }
                }
            "#,
        )
        .build();

    p.cargo("build").run();
    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
slow

"#]])
        .run();
}

#[cargo_test]
fn release_build_ndebug() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/foo.rs",
            r#"
                fn main() {
                    if cfg!(debug_assertions) {
                        println!("slow")
                    } else {
                        println!("fast")
                    }
                }
            "#,
        )
        .build();

    p.cargo("build --release").run();
    p.process(&p.release_bin("foo"))
        .with_stdout_data(str![[r#"
fast

"#]])
        .run();
}

#[cargo_test]
fn inferred_main_bin() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("build").run();
    p.process(&p.bin("foo")).run();
}

#[cargo_test]
fn deletion_causes_failure() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.change_file("Cargo.toml", &basic_manifest("foo", "0.0.1"));
    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
error[E0463]: can't find crate for `bar`
...
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error

"#]])
        .run();
}

#[cargo_test]
fn bad_cargo_toml_in_target_dir() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("target/Cargo.toml", "bad-toml")
        .build();

    p.cargo("build").run();
    p.process(&p.bin("foo")).run();
}

#[cargo_test]
fn lib_with_standard_name() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("syntax", "0.0.1"))
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "src/main.rs",
            "extern crate syntax; fn main() { syntax::foo() }",
        )
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] syntax v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn simple_staticlib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                  [package]
                  name = "foo"
                  authors = []
                  version = "0.0.1"
                  edition = "2015"

                  [lib]
                  name = "foo"
                  crate-type = ["staticlib"]
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();

    // env var is a test for #1381
    p.cargo("build").env("CARGO_LOG", "nekoneko=trace").run();
}

#[cargo_test]
fn staticlib_rlib_and_bin() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                  [package]
                  name = "foo"
                  authors = []
                  version = "0.0.1"
                  edition = "2015"

                  [lib]
                  name = "foo"
                  crate-type = ["staticlib", "rlib"]
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file("src/main.rs", "extern crate foo; fn main() { foo::foo(); }")
        .build();

    p.cargo("build -v").run();
}

#[cargo_test]
fn suggested_pkg_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "ver"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                static VERSION: &'static str = env!("CARGO_PKG_VERSION");

                fn main() {
                    println!("{}", VERSION);
                }
            "#,
        )
        .build();

    p.cargo("build -v").run();
    p.process(&p.bin("ver"))
        .with_stdout_data(str![[r#"
0.0.0

"#]])
        .run();

    p.cargo("build -v")
        .env("CARGO_SUGGESTED_PKG_VERSION", "X.Y.Z")
        .run();
    p.process(&p.bin("ver"))
        .with_stdout_data(str![[r#"
0.0.0

"#]])
        .run();

    p.cargo("build -v")
        .env("CARGO_SUGGESTED_PKG_VERSION", "1.2.3")
        .run();
    p.process(&p.bin("ver"))
        .with_stdout_data(str![[r#"
1.2.3

"#]])
        .run();
}

#[cargo_test]
fn opt_out_of_bin() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                  bin = []

                  [package]
                  name = "foo"
                  authors = []
                  version = "0.0.1"
                  edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "bad syntax")
        .build();
    p.cargo("build").run();
}

#[cargo_test]
fn single_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                  [package]
                  name = "foo"
                  authors = []
                  version = "0.0.1"
                  edition = "2015"

                  [lib]
                  name = "foo"
                  path = "src/bar.rs"
            "#,
        )
        .file("src/bar.rs", "")
        .build();
    p.cargo("build").run();
}

#[cargo_test]
fn freshness_ignores_excluded() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = "build.rs"
                exclude = ["src/b*.rs"]
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
        .build();
    foo.root().move_into_the_past();

    foo.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Smoke test to make sure it doesn't compile again
    println!("first pass");
    foo.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Modify an ignored file and make sure we don't rebuild
    println!("second pass");
    foo.change_file("src/bar.rs", "");
    foo.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn rebuild_preserves_out_dir() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = 'build.rs'
            "#,
        )
        .file(
            "build.rs",
            r#"
                use std::env;
                use std::fs::File;
                use std::path::Path;

                fn main() {
                    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("foo");
                    if env::var_os("FIRST").is_some() {
                        File::create(&path).unwrap();
                    } else {
                        File::create(&path).unwrap();
                    }
                }
            "#,
        )
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
        .build();
    foo.root().move_into_the_past();

    foo.cargo("build")
        .env("FIRST", "1")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    foo.change_file("src/bar.rs", "");
    foo.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn dep_no_libs() {
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
            "#,
        )
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.0"))
        .file("bar/src/main.rs", "")
        .build();
    foo.cargo("build").run();
}

#[cargo_test]
fn recompile_space_in_name() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [lib]
                name = "foo"
                path = "src/my lib.rs"
            "#,
        )
        .file("src/my lib.rs", "")
        .build();
    foo.cargo("build").run();
    foo.root().move_into_the_past();
    foo.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cfg(unix)]
#[cargo_test]
fn credentials_is_unreadable() {
    use cargo_test_support::paths::home;
    use std::os::unix::prelude::*;
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "")
        .build();

    let credentials = home().join(".cargo/credentials.toml");
    t!(fs::create_dir_all(credentials.parent().unwrap()));
    t!(fs::write(
        &credentials,
        r#"
            [registry]
            token = "api-token"
        "#
    ));
    let stat = fs::metadata(credentials.as_path()).unwrap();
    let mut perms = stat.permissions();
    perms.set_mode(0o000);
    fs::set_permissions(credentials, perms).unwrap();

    p.cargo("build").run();
}

#[cfg(unix)]
#[cargo_test]
fn ignore_bad_directories() {
    use std::os::unix::prelude::*;
    let foo = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();
    let dir = foo.root().join("tmp");
    fs::create_dir(&dir).unwrap();
    let stat = fs::metadata(&dir).unwrap();
    let mut perms = stat.permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&dir, perms.clone()).unwrap();
    foo.cargo("build").run();
    perms.set_mode(0o755);
    fs::set_permissions(&dir, perms).unwrap();
}

#[cargo_test]
fn bad_cargo_config() {
    let foo = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .file(".cargo/config.toml", "this is not valid toml")
        .build();
    foo.cargo("build -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not load Cargo configuration

Caused by:
  could not parse TOML configuration in `[ROOT]/foo/.cargo/config.toml`

Caused by:
  TOML parse error at line 1, column 6
    |
  1 | this is not valid toml
    |      ^
  expected `.`, `=`

"#]])
        .run();
}

#[cargo_test]
fn cargo_platform_specific_dependency() {
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

                    [target.{host}.dependencies]
                    dep = {{ path = "dep" }}
                    [target.{host}.build-dependencies]
                    build = {{ path = "build" }}
                    [target.{host}.dev-dependencies]
                    dev = {{ path = "dev" }}
                "#,
                host = host
            ),
        )
        .file("src/main.rs", "extern crate dep; fn main() { dep::dep() }")
        .file(
            "tests/foo.rs",
            "extern crate dev; #[test] fn foo() { dev::dev() }",
        )
        .file(
            "build.rs",
            "extern crate build; fn main() { build::build(); }",
        )
        .file("dep/Cargo.toml", &basic_manifest("dep", "0.5.0"))
        .file("dep/src/lib.rs", "pub fn dep() {}")
        .file("build/Cargo.toml", &basic_manifest("build", "0.5.0"))
        .file("build/src/lib.rs", "pub fn build() {}")
        .file("dev/Cargo.toml", &basic_manifest("dev", "0.5.0"))
        .file("dev/src/lib.rs", "pub fn dev() {}")
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    p.cargo("test").run();
}

#[cargo_test]
fn bad_platform_specific_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [target.wrong-target.dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file(
            "bar/src/lib.rs",
            r#"pub fn gimme() -> String { format!("") }"#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] foo v0.5.0 ([ROOT]/foo)
error[E0463]: can't find crate for `bar`
...
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error

"#]])
        .run();
}

#[cargo_test]
fn cargo_platform_specific_dependency_wrong_platform() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = ["wycats@example.com"]

                [target.non-existing-triplet.dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file(
            "bar/src/lib.rs",
            "invalid rust file, should not be compiled",
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    p.process(&p.bin("foo")).run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("bar"));
}

#[cargo_test]
fn example_as_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[example]]
                name = "ex"
                crate-type = ["lib"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    p.cargo("build --example=ex").run();
    assert!(p.example_lib("ex", "lib").is_file());
}

#[cargo_test]
fn example_as_rlib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[example]]
                name = "ex"
                crate-type = ["rlib"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    p.cargo("build --example=ex").run();
    assert!(p.example_lib("ex", "rlib").is_file());
}

#[cargo_test]
fn example_as_dylib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[example]]
                name = "ex"
                crate-type = ["dylib"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    p.cargo("build --example=ex").run();
    assert!(p.example_lib("ex", "dylib").is_file());
}

#[cargo_test]
fn example_as_proc_macro() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[example]]
                name = "ex"
                crate-type = ["proc-macro"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "examples/ex.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro]
            pub fn eat(_item: TokenStream) -> TokenStream {
                "".parse().unwrap()
            }
            "#,
        )
        .build();

    p.cargo("build --example=ex").run();
    assert!(p.example_lib("ex", "proc-macro").is_file());
}

#[cargo_test]
fn example_bin_same_name() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("examples/foo.rs", "fn main() {}")
        .build();

    p.cargo("build --examples").run();

    assert!(!p.bin("foo").is_file());
    // We expect a file of the form bin/foo-{metadata_hash}
    assert!(p.bin("examples/foo").is_file());

    p.cargo("build --examples").run();

    assert!(!p.bin("foo").is_file());
    // We expect a file of the form bin/foo-{metadata_hash}
    assert!(p.bin("examples/foo").is_file());
}

#[cargo_test]
fn compile_then_delete() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("run -v").run();
    assert!(p.bin("foo").is_file());
    if cfg!(windows) {
        // On windows unlinking immediately after running often fails, so sleep
        sleep_ms(100);
    }
    fs::remove_file(&p.bin("foo")).unwrap();
    p.cargo("run -v").run();
}

#[cargo_test]
fn transitive_dependencies_not_available() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.aaaaa]
                path = "a"
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate bbbbb; extern crate aaaaa; fn main() {}",
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "aaaaa"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bbbbb]
                path = "../b"
            "#,
        )
        .file("a/src/lib.rs", "extern crate bbbbb;")
        .file("b/Cargo.toml", &basic_manifest("bbbbb", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bbbbb v0.0.1 ([ROOT]/foo/b)
[RUNNING] `rustc [..]
[COMPILING] aaaaa v0.0.1 ([ROOT]/foo/a)
[RUNNING] `rustc [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]`
error[E0463]: can't find crate for `bbbbb`
...
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error

Caused by:
  process didn't exit successfully: `rustc [..]` ([EXIT_STATUS]: 1)

"#]])
        .run();
}

#[cargo_test]
fn cyclic_deps_rejected() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.a]
                path = "a"
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
                authors = []

                [dependencies.foo]
                path = ".."
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cyclic package dependency: package `a v0.0.1 ([ROOT]/foo/a)` depends on itself. Cycle:
package `a v0.0.1 ([ROOT]/foo/a)`
    ... which satisfies path dependency `a` of package `foo v0.0.1 ([ROOT]/foo)`
    ... which satisfies path dependency `foo` of package `a v0.0.1 ([ROOT]/foo/a)`

"#]])
        .run();
}

#[cargo_test]
fn predictable_filenames() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lib]
                name = "foo"
                crate-type = ["dylib", "rlib"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v").run();
    assert!(p.root().join("target/debug/libfoo.rlib").is_file());
    let dylib_name = format!("{}foo{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX);
    assert!(p.root().join("target/debug").join(dylib_name).is_file());
}

#[cargo_test]
fn dashes_to_underscores() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo-bar", "0.0.1"))
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo_bar; fn main() {}")
        .build();

    p.cargo("build -v").run();
    assert!(p.bin("foo-bar").is_file());
}

#[cargo_test]
fn dashes_in_crate_name_bad() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lib]
                name = "foo-bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo_bar; fn main() {}")
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  library target names cannot contain hyphens: foo-bar

"#]])
        .run();
}

#[cargo_test]
fn rustc_env_var() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build -v")
        .env("RUSTC", "rustc-that-does-not-exist")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not execute process `rustc-that-does-not-exist -vV` (never executed)

Caused by:
  [..]

"#]])
        .run();
    assert!(!p.bin("a").is_file());
}

#[cargo_test]
fn filtering() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .build();

    p.cargo("build --lib").run();
    assert!(!p.bin("a").is_file());

    p.cargo("build --bin=a --example=a").run();
    assert!(p.bin("a").is_file());
    assert!(!p.bin("b").is_file());
    assert!(p.bin("examples/a").is_file());
    assert!(!p.bin("examples/b").is_file());
}

#[cargo_test]
fn filtering_implicit_bins() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .build();

    p.cargo("build --bins").run();
    assert!(p.bin("a").is_file());
    assert!(p.bin("b").is_file());
    assert!(!p.bin("examples/a").is_file());
    assert!(!p.bin("examples/b").is_file());
}

#[cargo_test]
fn filtering_implicit_examples() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .build();

    p.cargo("build --examples").run();
    assert!(!p.bin("a").is_file());
    assert!(!p.bin("b").is_file());
    assert!(p.bin("examples/a").is_file());
    assert!(p.bin("examples/b").is_file());
}

#[cargo_test]
fn ignore_dotfile() {
    let p = project()
        .file("src/bin/.a.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn ignore_dotdirs() {
    let p = project()
        .file("src/bin/a.rs", "fn main() {}")
        .file(".git/Cargo.toml", "")
        .file(".pc/dummy-fix.patch/Cargo.toml", "")
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn dotdir_root() {
    let p = ProjectBuilder::new(root().join(".foo"))
        .file("src/bin/a.rs", "fn main() {}")
        .build();
    p.cargo("build").run();
}

#[cargo_test]
fn custom_target_dir_env() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let exe_name = format!("foo{}", env::consts::EXE_SUFFIX);

    p.cargo("build").env("CARGO_TARGET_DIR", "foo/target").run();
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(!p.root().join("target/debug").join(&exe_name).is_file());

    p.cargo("build").run();
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("target/debug").join(&exe_name).is_file());

    p.cargo("build")
        .env("CARGO_BUILD_TARGET_DIR", "foo2/target")
        .run();
    assert!(p.root().join("foo2/target/debug").join(&exe_name).is_file());

    p.change_file(
        ".cargo/config.toml",
        r#"
            [build]
            target-dir = "foo/target"
        "#,
    );
    p.cargo("build").env("CARGO_TARGET_DIR", "bar/target").run();
    assert!(p.root().join("bar/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("target/debug").join(&exe_name).is_file());
}

#[cargo_test]
fn custom_target_dir_line_parameter() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let exe_name = format!("foo{}", env::consts::EXE_SUFFIX);

    p.cargo("build --target-dir foo/target").run();
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(!p.root().join("target/debug").join(&exe_name).is_file());

    p.cargo("build").run();
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("target/debug").join(&exe_name).is_file());

    p.change_file(
        ".cargo/config.toml",
        r#"
            [build]
            target-dir = "foo/target"
        "#,
    );
    p.cargo("build --target-dir bar/target").run();
    assert!(p.root().join("bar/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("target/debug").join(&exe_name).is_file());

    p.cargo("build --target-dir foobar/target")
        .env("CARGO_TARGET_DIR", "bar/target")
        .run();
    assert!(p
        .root()
        .join("foobar/target/debug")
        .join(&exe_name)
        .is_file());
    assert!(p.root().join("bar/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("target/debug").join(&exe_name).is_file());
}

#[cargo_test]
fn build_multiple_packages() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.d1]
                    path = "d1"
                [dependencies.d2]
                    path = "d2"

                [[bin]]
                    name = "foo"
            "#,
        )
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("d1/Cargo.toml", &basic_bin_manifest("d1"))
        .file("d1/src/lib.rs", "")
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[bin]]
                    name = "d2"
                    doctest = false
            "#,
        )
        .file("d2/src/main.rs", "fn main() { println!(\"d2\"); }")
        .build();

    p.cargo("build -p d1 -p d2 -p foo").run();

    assert!(p.bin("foo").is_file());
    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
i am foo

"#]])
        .run();

    let d1_path = &p
        .build_dir()
        .join("debug")
        .join(format!("d1{}", env::consts::EXE_SUFFIX));
    let d2_path = &p
        .build_dir()
        .join("debug")
        .join(format!("d2{}", env::consts::EXE_SUFFIX));

    assert!(d1_path.is_file());
    p.process(d1_path)
        .with_stdout_data(str![[r#"
d1

"#]])
        .run();

    assert!(d2_path.is_file());
    p.process(d2_path)
        .with_stdout_data(str![[r#"
d2

"#]])
        .run();
}

#[cargo_test]
fn invalid_spec() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.d1]
                    path = "d1"

                [[bin]]
                    name = "foo"
            "#,
        )
        .file("src/bin/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("d1/Cargo.toml", &basic_bin_manifest("d1"))
        .file("d1/src/lib.rs", "")
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .build();

    p.cargo("build -p notAValidDep")
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[ERROR] package ID specification `notAValidDep` did not match any packages

"#]])
        .run();

    p.cargo("build -p d1 -p notAValidDep")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `notAValidDep` did not match any packages

"#]])
        .run();
}

#[cargo_test]
fn manifest_with_bom_is_ok() {
    let p = project()
        .file(
            "Cargo.toml",
            "\u{FEFF}
            [package]
            name = \"foo\"
            version = \"0.0.1\"
            edition = \"2015\"
            authors = []
        ",
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build -v").run();
}

#[cargo_test]
fn panic_abort_compiles_with_panic_abort() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [profile.dev]
                panic = 'abort'
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] -C panic=abort [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn compiler_json_error_format() {
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
        .file(
            "build.rs",
            "fn main() { println!(\"cargo::rustc-cfg=xyz\") }",
        )
        .file("src/main.rs", "fn main() { let unused = 92; }")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("bar/src/lib.rs", r#"fn dead() {}"#)
        .build();

    let output = |fresh| {
        r#"
[
  {
    "executable": null,
    "features": [],
    "fresh": $FRESH,
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo#0.5.0",
    "reason": "compiler-artifact",
    "target": {
      "kind": ["custom-build"],
      "...": "{...}"
    },
    "...": "{...}"
  },
  {
    "manifest_path": "[ROOT]/foo/bar/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo/bar#0.5.0",
    "reason": "compiler-message",
    "target": {
      "kind": ["lib"],
      "...": "{...}"
    },
    "...": "{...}"
  },
  {
    "executable": null,
    "features": [],
    "fresh": $FRESH,
    "manifest_path": "[ROOT]/foo/bar/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo/bar#0.5.0",
    "reason": "compiler-artifact",
    "target": {
      "kind": ["lib"],
      "...": "{...}"
    },
    "...": "{...}"
  },
  {
    "cfgs": [
      "xyz"
    ],
    "env": [],
    "linked_libs": [],
    "linked_paths": [],
    "out_dir": "[ROOT]/foo/target/debug/build/foo-[HASH]/out",
    "package_id": "path+[ROOTURL]/foo#0.5.0",
    "reason": "build-script-executed"
  },
  {
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo#0.5.0",
    "reason": "compiler-message",
    "target": {
      "kind": ["bin"],
      "...": "{...}"
    },
    "...": "{...}"
  },
  {
    "features": [],
    "fresh": $FRESH,
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo#0.5.0",
    "reason": "compiler-artifact",
    "target": {
      "kind": ["bin"],
      "...": "{...}"
    },
    "...": "{...}"
  },
  {
    "reason": "build-finished",
    "success": true
  },
  "{...}"
]
"#
        .replace("$FRESH", fresh)
        .is_json()
        .against_jsonlines()
        .unordered()
    };

    // Use `jobs=1` to ensure that the order of messages is consistent.
    p.cargo("build -v --message-format=json --jobs=1")
        .with_stdout_data(output("false"))
        .run();

    // With fresh build, we should repeat the artifacts,
    // and replay the cached compiler warnings.
    p.cargo("build -v --message-format=json --jobs=1")
        .with_stdout_data(output("true"))
        .run();
}

#[cargo_test]
fn wrong_message_format_option() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --message-format XML")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid message format specifier: `xml`

"#]])
        .run();
}

#[cargo_test]
fn message_format_json_forward_stderr() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() { let unused = 0; }")
        .build();

    p.cargo("rustc --release --bin foo --message-format JSON")
        .with_stdout_data(
            str![[r#"
[
  {
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "message": "{...}",
    "package_id": "path+[ROOTURL]/foo#0.5.0",
    "reason": "compiler-message",
    "target": {
      "kind": ["bin"],
      "...": "{...}"
    },
    "...": "{...}"
  },
  {
    "features": [],
    "fresh": false,
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo#0.5.0",
    "reason": "compiler-artifact",
    "target": {
      "kind": ["bin"],
      "...": "{...}"
    },
    "...": "{...}"
  },
  {
    "reason": "build-finished",
    "success": true
  },
  "{...}"
]
"#]]
            .is_json()
            .against_jsonlines()
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn no_warn_about_package_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [package.metadata]
                foo = "bar"
                a = true
                b = 3

                [package.metadata.another]
                bar = 3
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn no_warn_about_workspace_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["foo"]

            [workspace.metadata]
            something = "something_else"
            x = 1
            y = 2

            [workspace.metadata.another]
            bar = 12
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn cargo_build_empty_target() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --target")
        .arg("")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] target was empty

"#]])
        .run();
}

#[cargo_test]
fn cargo_build_with_unsupported_short_target_flag() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -t")
        .arg("")
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '-t' found

  tip: a similar argument exists: '--target'

Usage: cargo[EXE] build [OPTIONS]

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn build_all_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = { path = "bar" }

                [workspace]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build --workspace")
        .with_stderr_data(str![[r#"
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_all_exclude() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("build --workspace --exclude baz")
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn cargo_build_with_unsupported_short_exclude_flag() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("build --workspace -x baz")
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '-x' found

  tip: a similar argument exists: '--exclude'

Usage: cargo[EXE] build [OPTIONS]

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn build_all_exclude_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build --workspace --exclude baz")
        .with_stderr_data(
            str![[r#"
[WARNING] excluded package(s) `baz` not found in workspace `[ROOT]/foo`
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn build_all_exclude_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("build --workspace --exclude '*z'")
        .with_stderr_data(
            str![[r#"
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn build_all_exclude_glob_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build --workspace --exclude '*z'")
        .with_stderr_data(
            str![[r#"
[WARNING] excluded package pattern(s) `*z` not found in workspace `[ROOT]/foo`
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn build_all_exclude_broken_glob() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("build --workspace --exclude '[*z'")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot build glob pattern from `[*z`

Caused by:
...

"#]])
        .run();
}

#[cargo_test]
fn build_all_workspace_implicit_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = { path = "bar" }

                [workspace]
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/c.rs", "fn main() {}")
        .file("examples/d.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .file("bar/src/bin/e.rs", "fn main() {}")
        .file("bar/src/bin/f.rs", "fn main() {}")
        .file("bar/examples/g.rs", "fn main() {}")
        .file("bar/examples/h.rs", "fn main() {}")
        .build();

    p.cargo("build --workspace --examples")
        .with_stderr_data(str![[r#"
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert!(!p.bin("a").is_file());
    assert!(!p.bin("b").is_file());
    assert!(p.bin("examples/c").is_file());
    assert!(p.bin("examples/d").is_file());
    assert!(!p.bin("e").is_file());
    assert!(!p.bin("f").is_file());
    assert!(p.bin("examples/g").is_file());
    assert!(p.bin("examples/h").is_file());
}

#[cargo_test]
fn build_all_virtual_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    // The order in which bar and baz are built is not guaranteed
    p.cargo("build --workspace")
        .with_stderr_data(
            str![[r#"
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] baz v0.1.0 ([ROOT]/foo/baz)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn build_virtual_manifest_all_implied() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    // The order in which `bar` and `baz` are built is not guaranteed.
    p.cargo("build")
        .with_stderr_data(
            str![[r#"
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] baz v0.1.0 ([ROOT]/foo/baz)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn build_virtual_manifest_one_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("build -p bar")
        .with_stderr_data(str![[r#"
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_virtual_manifest_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() { break_the_build(); }")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("build -p '*z'")
        .with_stderr_data(str![[r#"
[COMPILING] baz v0.1.0 ([ROOT]/foo/baz)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_virtual_manifest_glob_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build -p bar -p '*z'")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package pattern(s) `*z` not found in workspace `[ROOT]/foo`

"#]])
        .run();
}

#[cargo_test]
fn build_virtual_manifest_broken_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build -p '[*z'")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot build glob pattern from `[*z`

Caused by:
...

"#]])
        .run();
}

#[cargo_test]
fn build_all_virtual_manifest_implicit_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .file("bar/src/bin/a.rs", "fn main() {}")
        .file("bar/src/bin/b.rs", "fn main() {}")
        .file("bar/examples/c.rs", "fn main() {}")
        .file("bar/examples/d.rs", "fn main() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "")
        .file("baz/src/bin/e.rs", "fn main() {}")
        .file("baz/src/bin/f.rs", "fn main() {}")
        .file("baz/examples/g.rs", "fn main() {}")
        .file("baz/examples/h.rs", "fn main() {}")
        .build();

    // The order in which bar and baz are built is not guaranteed
    p.cargo("build --workspace --examples")
        .with_stderr_data(
            str![[r#"
[COMPILING] baz v0.1.0 ([ROOT]/foo/baz)
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
    assert!(!p.bin("a").is_file());
    assert!(!p.bin("b").is_file());
    assert!(p.bin("examples/c").is_file());
    assert!(p.bin("examples/d").is_file());
    assert!(!p.bin("e").is_file());
    assert!(!p.bin("f").is_file());
    assert!(p.bin("examples/g").is_file());
    assert!(p.bin("examples/h").is_file());
}

#[cargo_test]
fn build_all_member_dependency_same_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                a = "0.1.0"
            "#,
        )
        .file("a/src/lib.rs", "pub fn a() {}")
        .build();

    Package::new("a", "0.1.0").publish();

    p.cargo("build --workspace")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.0 (registry `dummy-registry`)
[COMPILING] a v0.1.0
[COMPILING] a v0.1.0 ([ROOT]/foo/a)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn run_proper_binary() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                edition = "2015"
                [[bin]]
                name = "main"
                [[bin]]
                name = "other"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "src/bin/main.rs",
            r#"fn main() { panic!("This should never be run."); }"#,
        )
        .file("src/bin/other.rs", "fn main() {}")
        .build();

    p.cargo("run --bin other").run();
}

#[cargo_test]
fn run_proper_binary_main_rs() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", "fn main() {}")
        .build();

    p.cargo("run --bin foo").run();
}

#[cargo_test]
fn run_proper_alias_binary_from_src() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                edition = "2015"
                [[bin]]
                name = "foo"
                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/foo.rs", r#"fn main() { println!("foo"); }"#)
        .file("src/bar.rs", r#"fn main() { println!("bar"); }"#)
        .build();

    p.cargo("build --workspace").run();
    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
foo

"#]])
        .run();
    p.process(&p.bin("bar"))
        .with_stdout_data(str![[r#"
bar

"#]])
        .run();
}

#[cargo_test]
fn run_proper_alias_binary_main_rs() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                edition = "2015"
                [[bin]]
                name = "foo"
                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("main"); }"#)
        .build();

    p.cargo("build --workspace").run();
    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
main

"#]])
        .run();
    p.process(&p.bin("bar"))
        .with_stdout_data(str![[r#"
main

"#]])
        .run();
}

#[cargo_test]
fn run_proper_binary_main_rs_as_foo() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/foo.rs",
            r#" fn main() { panic!("This should never be run."); }"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("run --bin foo").run();
}

#[cargo_test]
fn rustc_wrapper() {
    let p = project().file("src/lib.rs", "").build();
    let wrapper = tools::echo_wrapper();
    p.cargo("build -v")
        .env("RUSTC_WRAPPER", &wrapper)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..]/rustc-echo-wrapper[EXE] rustc --crate-name foo [..]`
WRAPPER CALLED: rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.build_dir().rm_rf();
    p.cargo("build -v")
        .env("RUSTC_WORKSPACE_WRAPPER", &wrapper)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..]/rustc-echo-wrapper[EXE] rustc --crate-name foo [..]`
WRAPPER CALLED: rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

/// Checks what happens when both rust-wrapper and rustc-workspace-wrapper are set.
#[cargo_test]
fn rustc_wrapper_precendence() {
    let p = project().file("src/lib.rs", "").build();
    let rustc_wrapper = tools::echo_wrapper();
    let ws_wrapper = rustc_wrapper.with_file_name("rustc-ws-wrapper");
    assert_ne!(rustc_wrapper, ws_wrapper);
    std::fs::hard_link(&rustc_wrapper, &ws_wrapper).unwrap();

    p.cargo("build -v")
        .env("RUSTC_WRAPPER", &rustc_wrapper)
        .env("RUSTC_WORKSPACE_WRAPPER", &ws_wrapper)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..]/rustc-echo-wrapper[EXE] [..]/rustc-ws-wrapper rustc --crate-name foo [..]`
WRAPPER CALLED: [..]/rustc-ws-wrapper rustc --crate-name foo [..]
WRAPPER CALLED: rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn rustc_wrapper_queries() {
    // Check that the invocations querying rustc for information are done with the wrapper.
    let p = project().file("src/lib.rs", "").build();
    let wrapper = tools::echo_wrapper();
    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("RUSTC_WRAPPER", &wrapper)
        .with_stderr_contains("[..]running [..]rustc-echo-wrapper[EXE] rustc -vV[..]")
        .with_stderr_contains(
            "[..]running [..]rustc-echo-wrapper[EXE] rustc - --crate-name ___ --print[..]",
        )
        .run();
    p.build_dir().rm_rf();
    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("RUSTC_WORKSPACE_WRAPPER", &wrapper)
        .with_stderr_contains("[..]running [..]rustc-echo-wrapper[EXE] rustc -vV[..]")
        .with_stderr_contains(
            "[..]running [..]rustc-echo-wrapper[EXE] rustc - --crate-name ___ --print[..]",
        )
        .run();
}

#[cargo_test]
fn rustc_wrapper_relative() {
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
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    let wrapper = tools::echo_wrapper();
    let exe_name = wrapper.file_name().unwrap().to_str().unwrap();
    let relative_path = format!("./{}", exe_name);
    fs::hard_link(&wrapper, p.root().join(exe_name)).unwrap();
    p.cargo("build -v")
        .env("RUSTC_WRAPPER", &relative_path)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[COMPILING] bar v1.0.0
[RUNNING] `[ROOT]/foo/./rustc-echo-wrapper[EXE] rustc --crate-name bar [..]`
WRAPPER CALLED: rustc --crate-name bar [..]
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `[ROOT]/foo/./rustc-echo-wrapper[EXE] rustc --crate-name foo [..]`
WRAPPER CALLED: rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.build_dir().rm_rf();
    p.cargo("build -v")
        .env("RUSTC_WORKSPACE_WRAPPER", &relative_path)
        .with_stderr_data(str![[r#"
[COMPILING] bar v1.0.0
[RUNNING] `rustc --crate-name bar [..]`
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `[ROOT]/foo/./rustc-echo-wrapper[EXE] rustc --crate-name foo [..]`
WRAPPER CALLED: rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.build_dir().rm_rf();
    p.change_file(
        ".cargo/config.toml",
        &format!(
            r#"
                build.rustc-wrapper = "./{}"
            "#,
            exe_name
        ),
    );
    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[COMPILING] bar v1.0.0
[RUNNING] `[ROOT]/foo/./rustc-echo-wrapper[EXE] rustc --crate-name bar [..]`
WRAPPER CALLED: rustc --crate-name bar [..]
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `[ROOT]/foo/./rustc-echo-wrapper[EXE] rustc --crate-name foo [..]`
WRAPPER CALLED: rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn rustc_wrapper_from_path() {
    let p = project().file("src/lib.rs", "").build();
    p.cargo("build -v")
        .env("RUSTC_WRAPPER", "wannabe_sccache")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not execute process `wannabe_sccache rustc -vV` (never executed)

Caused by:
  [..]

"#]])
        .run();
    p.build_dir().rm_rf();
    p.cargo("build -v")
        .env("RUSTC_WORKSPACE_WRAPPER", "wannabe_sccache")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not execute process `wannabe_sccache rustc -vV` (never executed)

Caused by:
  [..]

"#]])
        .run();
}

#[cargo_test]
fn cdylib_not_lifted() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.1.0"
                edition = "2015"

                [lib]
                crate-type = ["cdylib"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();

    let files = if cfg!(windows) {
        if cfg!(target_env = "msvc") {
            vec!["foo.dll.lib", "foo.dll.exp", "foo.dll"]
        } else {
            vec!["libfoo.dll.a", "foo.dll"]
        }
    } else if cfg!(target_os = "macos") {
        vec!["libfoo.dylib"]
    } else {
        vec!["libfoo.so"]
    };

    for file in files {
        println!("checking: {}", file);
        assert!(p.root().join("target/debug/deps").join(&file).is_file());
    }
}

#[cargo_test]
fn cdylib_final_outputs() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo-bar"
                authors = []
                version = "0.1.0"
                edition = "2015"

                [lib]
                crate-type = ["cdylib"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();

    let files = if cfg!(windows) {
        if cfg!(target_env = "msvc") {
            vec!["foo_bar.dll.lib", "foo_bar.dll"]
        } else {
            vec!["foo_bar.dll", "libfoo_bar.dll.a"]
        }
    } else if cfg!(target_os = "macos") {
        vec!["libfoo_bar.dylib"]
    } else {
        vec!["libfoo_bar.so"]
    };

    for file in files {
        println!("checking: {}", file);
        assert!(p.root().join("target/debug").join(&file).is_file());
    }
}

#[cargo_test]
// NOTE: Windows MSVC and wasm32-unknown-emscripten do not use metadata. Skip them.
// See <https://github.com/rust-lang/cargo/issues/9325#issuecomment-1030662699>
#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
fn no_dep_info_collision_when_cdylib_and_bin_coexist() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            edition = "2015"

            [lib]
            crate-type = ["cdylib"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v1.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..] --crate-type bin [..] -C metadata=[..]`
[RUNNING] `rustc [..] --crate-type cdylib [..] -C metadata=[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    let deps_dir = p.target_debug_dir().join("deps");
    assert!(deps_dir.join("foo.d").exists());
    let dep_info_count = deps_dir
        .read_dir()
        .unwrap()
        .filter(|e| {
            let filename = e.as_ref().unwrap().file_name();
            let filename = filename.to_str().unwrap();
            filename.starts_with("foo") && filename.ends_with(".d")
        })
        .count();
    // cdylib -> foo.d
    // bin -> foo-<meta>.d
    assert_eq!(dep_info_count, 2);
}

#[cargo_test]
fn deterministic_cfg_flags() {
    // This bug is non-deterministic.

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []
                build = "build.rs"

                [features]
                default = ["f_a", "f_b", "f_c", "f_d"]
                f_a = []
                f_b = []
                f_c = []
                f_d = []
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-cfg=cfg_a");
                    println!("cargo::rustc-cfg=cfg_b");
                    println!("cargo::rustc-cfg=cfg_c");
                    println!("cargo::rustc-cfg=cfg_d");
                    println!("cargo::rustc-cfg=cfg_e");
                }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[RUNNING] `rustc --crate-name foo [..] --cfg[..]default[..]--cfg[..]f_a[..]--cfg[..]f_b[..] --cfg[..]f_c[..]--cfg[..]f_d[..] --cfg cfg_a --cfg cfg_b --cfg cfg_c --cfg cfg_d --cfg cfg_e`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn explicit_bins_without_paths() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [[bin]]
                name = "foo"

                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn no_bin_in_src_with_lib() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/foo.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  can't find `foo` bin at `src/bin/foo.rs` or `src/bin/foo/main.rs`. Please specify bin.path if you want to use a non-default path.

"#]])
        .run();
}

#[cargo_test]
fn inferred_bins() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .file("src/bin/baz/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());
    assert!(p.bin("bar").is_file());
    assert!(p.bin("baz").is_file());
}

#[cargo_test]
fn inferred_bins_duplicate_name() {
    // this should fail, because we have two binaries with the same name
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .file("src/bin/bar/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  found duplicate binary name bar, but all binary targets must have a unique name

"#]])
        .run();
}

#[cargo_test]
fn inferred_bin_path() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"
            authors = []

            [[bin]]
            name = "bar"
            # Note, no `path` key!
            "#,
        )
        .file("src/bin/bar/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
    assert!(p.bin("bar").is_file());
}

#[cargo_test]
fn inferred_examples() {
    let p = project()
        .file("src/lib.rs", "fn main() {}")
        .file("examples/bar.rs", "fn main() {}")
        .file("examples/baz/main.rs", "fn main() {}")
        .build();

    p.cargo("build --examples").run();
    assert!(p.bin("examples/bar").is_file());
    assert!(p.bin("examples/baz").is_file());
}

#[cargo_test]
fn inferred_tests() {
    let p = project()
        .file("src/lib.rs", "fn main() {}")
        .file("tests/bar.rs", "fn main() {}")
        .file("tests/baz/main.rs", "fn main() {}")
        .build();

    p.cargo("test --test=bar --test=baz").run();
}

#[cargo_test]
fn inferred_benchmarks() {
    let p = project()
        .file("src/lib.rs", "fn main() {}")
        .file("benches/bar.rs", "fn main() {}")
        .file("benches/baz/main.rs", "fn main() {}")
        .build();

    p.cargo("bench --bench=bar --bench=baz").run();
}

#[cargo_test]
fn no_infer_dirs() {
    let p = project()
        .file("src/lib.rs", "fn main() {}")
        .file("examples/dir.rs/dummy", "")
        .file("benches/dir.rs/dummy", "")
        .file("tests/dir.rs/dummy", "")
        .build();

    p.cargo("build --examples --benches --tests").run(); // should not fail with "is a directory"
}

#[cargo_test]
fn target_edition() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [lib]
                edition = "2018"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--edition=2018 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn target_edition_override() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2018"

                [lib]
                edition = "2015"
            "#,
        )
        .file(
            "src/lib.rs",
            "
                pub fn async() {}
                pub fn try() {}
                pub fn await() {}
            ",
        )
        .build();

    p.cargo("build -v").run();
}

#[cargo_test]
fn same_metadata_different_directory() {
    // A top-level crate built in two different workspaces should have the
    // same metadata hash.
    let p = project()
        .at("foo1")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();
    let output = t!(String::from_utf8(p.cargo("build -v").run().stderr,));
    let metadata = output
        .split_whitespace()
        .find(|arg| arg.starts_with("metadata="))
        .unwrap();

    let p = project()
        .at("foo2")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build -v")
        .with_stderr_data(format!("...\n[..]{metadata}[..]\n..."))
        .run();
}

#[cargo_test]
fn building_a_dependent_crate_without_bin_should_fail() {
    Package::new("testless", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "testless"
                version = "0.1.0"
                edition = "2015"

                [[bin]]
                name = "a_bin"
            "#,
        )
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                testless = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] testless v0.1.0 (registry `dummy-registry`)
[ERROR] failed to download replaced source registry `crates-io`

Caused by:
  failed to parse manifest at `[ROOT]/home/.cargo/registry/src/-[HASH]/testless-0.1.0/Cargo.toml`

Caused by:
  can't find `a_bin` bin at `src/bin/a_bin.rs` or `src/bin/a_bin/main.rs`. Please specify bin.path if you want to use a non-default path.

"#]])
        .run();
}

#[cargo_test]
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn uplift_dsym_of_bin_on_mac() {
    let p = project()
        .file("src/main.rs", "fn main() { panic!(); }")
        .file("src/bin/b.rs", "fn main() { panic!(); }")
        .file("examples/c.rs", "fn main() { panic!(); }")
        .file("tests/d.rs", "fn main() { panic!(); }")
        .build();

    p.cargo("build --bins --examples --tests")
        .enable_mac_dsym()
        .run();
    assert!(p.target_debug_dir().join("foo.dSYM").is_dir());
    assert!(p.target_debug_dir().join("b.dSYM").is_dir());
    assert!(p.target_debug_dir().join("b.dSYM").is_symlink());
    assert!(p.target_debug_dir().join("examples/c.dSYM").is_dir());
    assert!(!p.target_debug_dir().join("c.dSYM").exists());
    assert!(!p.target_debug_dir().join("d.dSYM").exists());
}

#[cargo_test]
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn uplift_dsym_of_bin_on_mac_when_broken_link_exists() {
    let p = project()
        .file("src/main.rs", "fn main() { panic!(); }")
        .build();
    let dsym = p.target_debug_dir().join("foo.dSYM");

    p.cargo("build").enable_mac_dsym().run();
    assert!(dsym.is_dir());

    // Simulate the situation where the underlying dSYM bundle goes missing
    // but the uplifted symlink to it remains. This would previously cause
    // builds to permanently fail until the bad symlink was manually removed.
    dsym.rm_rf();
    p.symlink(
        p.target_debug_dir()
            .join("deps")
            .join("foo-baaaaaadbaaaaaad.dSYM"),
        &dsym,
    );
    assert!(dsym.is_symlink());
    assert!(!dsym.exists());

    p.cargo("build").enable_mac_dsym().run();
    assert!(dsym.is_dir());
}

#[cargo_test]
#[cfg(all(target_os = "windows", target_env = "msvc"))]
fn uplift_pdb_of_bin_on_windows() {
    let p = project()
        .file("src/main.rs", "fn main() { panic!(); }")
        .file("src/bin/b.rs", "fn main() { panic!(); }")
        .file("src/bin/foo-bar.rs", "fn main() { panic!(); }")
        .file("examples/c.rs", "fn main() { panic!(); }")
        .file("tests/d.rs", "fn main() { panic!(); }")
        .build();

    p.cargo("build --bins --examples --tests").run();
    assert!(p.target_debug_dir().join("foo.pdb").is_file());
    assert!(p.target_debug_dir().join("b.pdb").is_file());
    assert!(p.target_debug_dir().join("examples/c.pdb").exists());
    assert!(p.target_debug_dir().join("foo-bar.exe").is_file());
    assert!(p.target_debug_dir().join("foo_bar.pdb").is_file());
    assert!(!p.target_debug_dir().join("c.pdb").exists());
    assert!(!p.target_debug_dir().join("d.pdb").exists());
}

#[cargo_test]
#[cfg(target_os = "linux")]
fn uplift_dwp_of_bin_on_linux() {
    let p = project()
        .file("src/main.rs", "fn main() { panic!(); }")
        .file("src/bin/b.rs", "fn main() { panic!(); }")
        .file("src/bin/foo-bar.rs", "fn main() { panic!(); }")
        .file("examples/c.rs", "fn main() { panic!(); }")
        .file("tests/d.rs", "fn main() { panic!(); }")
        .build();

    p.cargo("build --bins --examples --tests")
        .enable_split_debuginfo_packed()
        .run();
    assert!(p.target_debug_dir().join("foo.dwp").is_file());
    assert!(p.target_debug_dir().join("b.dwp").is_file());
    assert!(p.target_debug_dir().join("examples/c.dwp").exists());
    assert!(p.target_debug_dir().join("foo-bar").is_file());
    assert!(p.target_debug_dir().join("foo-bar.dwp").is_file());
    assert!(!p.target_debug_dir().join("c.dwp").exists());
    assert!(!p.target_debug_dir().join("d.dwp").exists());
}

// Ensure that `cargo build` chooses the correct profile for building
// targets based on filters (assuming `--profile` is not specified).
#[cargo_test]
fn build_filter_infer_profile() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/t1.rs", "")
        .file("benches/b1.rs", "")
        .file("examples/ex1.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link[..]`
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered())
        .run();

    p.root().join("target").rm_rf();
    p.cargo("build -v --test=t1")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link[..] -C debuginfo=2 [..]`
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link[..] -C debuginfo=2 [..]`
[RUNNING] `rustc --crate-name t1 --edition=2015 tests/t1.rs [..]--emit=[..]link[..] -C debuginfo=2 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered())
        .run();

    p.root().join("target").rm_rf();
    // Bench uses test profile without `--release`.
    p.cargo("build -v --bench=b1")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link [..]-C debuginfo=2 [..]`
[RUNNING] `rustc --crate-name b1 --edition=2015 benches/b1.rs [..]--emit=[..]link[..] -C embed-bitcode=no -C debuginfo=2 [..]--test [..]`
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link [..]-C debuginfo=2 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered())
        .run();
}

#[cargo_test]
fn targets_selected_default() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("build -v")
        // Binaries.
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin \
             --emit=[..]link[..]",
        )
        // Benchmarks.
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]link \
             -C opt-level=3 --test [..]",
        )
        // Unit tests.
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]link[..]\
             -C debuginfo=2 --test [..]",
        )
        .run();
}

#[cargo_test]
fn targets_selected_all() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    // The first RUNNING is for unit tests
    // The second RUNNING is for binaries
    p.cargo("build -v --all-targets")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]link [..]-C debuginfo=2 -[..]-test[..]`
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered())
        .run();
}

#[cargo_test]
fn all_targets_no_lib() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    // The first RUNNING is for unit tests
    // The second RUNNING is for binaries
    p.cargo("build -v --all-targets")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--emit=[..]link[..] -C debuginfo=2 [..]--test [..]`
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]--crate-type bin --emit=[..]link[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered())
        .run();
}

#[cargo_test]
fn no_linkable_target() {
    // Issue 3169: this is currently not an error as per discussion in PR #4797.
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
                the_lib = { path = "the_lib" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "the_lib/Cargo.toml",
            r#"
                [package]
                name = "the_lib"
                version = "0.1.0"
                edition = "2015"
                [lib]
                name = "the_lib"
                crate-type = ["staticlib"]
            "#,
        )
        .file("the_lib/src/lib.rs", "pub fn foo() {}")
        .build();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[WARNING] The package `the_lib` provides no linkable target. The compiler might raise an error while compiling `foo`. Consider adding 'dylib' or 'rlib' to key `crate-type` in `the_lib`'s Cargo.toml. This warning might turn into a hard error in the future.
[COMPILING] the_lib v0.1.0 ([ROOT]/foo/the_lib)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn avoid_dev_deps() {
    Package::new("foo", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dev-dependencies]
                baz = "1.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `baz` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `bar v0.1.0 ([ROOT]/foo)`

"#]])
        .run();
    p.cargo("build -Zavoid-dev-deps")
        .masquerade_as_nightly_cargo(&["avoid-dev-deps"])
        .run();
}

#[cargo_test]
fn default_cargo_config_jobs() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                jobs = 1
            "#,
        )
        .build();
    p.cargo("build -v").run();
}

#[cargo_test]
fn good_cargo_config_jobs() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                jobs = 4
            "#,
        )
        .build();
    p.cargo("build -v").run();
}

#[cargo_test]
fn good_jobs() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build --jobs 1").run();

    p.cargo("build --jobs -1").run();

    p.cargo("build --jobs default").run();
}

#[cargo_test]
fn invalid_cargo_config_jobs() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                jobs = 0
            "#,
        )
        .build();
    p.cargo("build -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] jobs may not be 0

"#]])
        .run();
}

#[cargo_test]
fn invalid_jobs() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build --jobs 0")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] jobs may not be 0

"#]])
        .run();

    p.cargo("build --jobs over9000")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not parse `over9000`. Number of parallel jobs should be `default` or a number.

"#]])
        .run();
}

#[cargo_test]
fn target_filters_workspace() {
    let ws = project()
        .at("ws")
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_lib_manifest("a"))
        .file("a/src/lib.rs", "")
        .file("a/examples/ex1.rs", "fn main() {}")
        .file("b/Cargo.toml", &basic_bin_manifest("b"))
        .file("b/src/lib.rs", "")
        .file("b/src/main.rs", "fn main() {}")
        .build();

    ws.cargo("build -v --example ex")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no example target named `ex`

	Did you mean `ex1`?

"#]])
        .run();

    ws.cargo("build -v --example 'ex??'")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no example target matches pattern `ex??`

	Did you mean `ex1`?

"#]])
        .run();

    ws.cargo("build -v --lib")
        .with_stderr_data(
            str![[r#"
[COMPILING] a v0.5.0 ([ROOT]/ws/a)
[COMPILING] b v0.5.0 ([ROOT]/ws/b)
[RUNNING] `rustc [..]a/src/lib.rs[..]`
[RUNNING] `rustc [..]b/src/lib.rs[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    ws.cargo("build -v --example ex1")
        .with_stderr_data(str![[r#"
[COMPILING] a v0.5.0 ([ROOT]/ws/a)
[RUNNING] `rustc [..]a/examples/ex1.rs[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn target_filters_workspace_not_found() {
    let ws = project()
        .at("ws")
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_bin_manifest("a"))
        .file("a/src/main.rs", "fn main() {}")
        .file("b/Cargo.toml", &basic_bin_manifest("b"))
        .file("b/src/main.rs", "fn main() {}")
        .build();

    ws.cargo("build -v --lib")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no library targets found in packages: a, b

"#]])
        .run();
}

#[cfg(unix)]
#[cargo_test]
fn signal_display() {
    // Cause the compiler to crash with a signal.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                [dependencies]
                pm = { path = "pm" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[macro_use]
                extern crate pm;

                #[derive(Foo)]
                pub struct S;
            "#,
        )
        .file(
            "pm/Cargo.toml",
            r#"
                [package]
                name = "pm"
                version = "0.1.0"
                edition = "2015"
                [lib]
                proc-macro = true
            "#,
        )
        .file(
            "pm/src/lib.rs",
            r#"
                extern crate proc_macro;
                use proc_macro::TokenStream;

                #[proc_macro_derive(Foo)]
                pub fn derive(_input: TokenStream) -> TokenStream {
                    std::process::abort()
                }
            "#,
        )
        .build();

    foo.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] pm v0.1.0 ([ROOT]/foo/pm)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[ERROR] could not compile `foo` (lib)

Caused by:
  process didn't exit successfully: `rustc [..]` (signal: 6, SIGABRT: process abort signal)

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn tricky_pipelining() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    foo.cargo("build -p bar").run();
    foo.cargo("build -p foo").run();
}

#[cargo_test]
fn pipelining_works() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    foo.cargo("build")
        .with_stdout_data(str![])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn pipelining_big_graph() {
    // Create a crate graph of the form {a,b}{0..29}, where {a,b}(n) depend on {a,b}(n+1)
    // Then have `foo`, a binary crate, depend on the whole thing.
    let mut project = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                [dependencies]
                a1 = { path = "a1" }
                b1 = { path = "b1" }
            "#,
        )
        .file("src/main.rs", "fn main(){}");

    for n in 0..30 {
        for x in &["a", "b"] {
            project = project
                .file(
                    &format!("{x}{n}/Cargo.toml", x = x, n = n),
                    &format!(
                        r#"
                            [package]
                            name = "{x}{n}"
                            version = "0.1.0"
                            edition = "2015"
                            [dependencies]
                            a{np1} = {{ path = "../a{np1}" }}
                            b{np1} = {{ path = "../b{np1}" }}
                        "#,
                        x = x,
                        n = n,
                        np1 = n + 1
                    ),
                )
                .file(&format!("{x}{n}/src/lib.rs", x = x, n = n), "");
        }
    }

    let foo = project
        .file("a30/Cargo.toml", &basic_lib_manifest("a30"))
        .file(
            "a30/src/lib.rs",
            r#"compile_error!("don't actually build me");"#,
        )
        .file("b30/Cargo.toml", &basic_lib_manifest("b30"))
        .file("b30/src/lib.rs", "")
        .build();
    foo.cargo("build -p foo")
        .with_status(101)
        .with_stderr_data(
            str![[r#"
[COMPILING] a30 v0.5.0 ([ROOT]/foo/a30)
[ERROR] don't actually build me
...
[ERROR] could not compile `a30` (lib) due to 1 previous error
...

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn forward_rustc_output() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = '2018'
                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "bar::foo!();")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                [lib]
                proc-macro = true
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                extern crate proc_macro;
                use proc_macro::*;

                #[proc_macro]
                pub fn foo(input: TokenStream) -> TokenStream {
                    println!("a");
                    println!("b");
                    println!("{{}}");
                    eprintln!("c");
                    eprintln!("d");
                    eprintln!("{{a"); // "malformed json"
                    input
                }
            "#,
        )
        .build();

    foo.cargo("build")
        .with_stdout_data(str![[r#"
a
b
{}

"#]])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
c
d
{a
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_lib_only() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("build --lib -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]--crate-type lib --emit=[..]link[..] -L dependency=[ROOT]/foo/target/debug/deps`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_with_no_lib() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --lib")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no library targets found in package `foo`

"#]])
        .run();
}

#[cargo_test]
fn build_with_relative_cargo_home_path() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = ["wycats@example.com"]

                [dependencies]

                "test-dependency" = { path = "src/test_dependency" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("src/test_dependency/src/lib.rs", r#" "#)
        .file(
            "src/test_dependency/Cargo.toml",
            &basic_manifest("test-dependency", "0.0.1"),
        )
        .build();

    p.cargo("build").env("CARGO_HOME", "./cargo_home/").run();
}

#[cargo_test]
fn user_specific_cfgs_are_filtered_out() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"fn main() {}"#)
        .file(
            "build.rs",
            r#"
            fn main() {
                assert!(std::env::var_os("CARGO_CFG_PROC_MACRO").is_none());
                assert!(std::env::var_os("CARGO_CFG_DEBUG_ASSERTIONS").is_none());
            }
            "#,
        )
        .build();

    p.cargo("rustc -- --cfg debug_assertions --cfg proc_macro -Aunknown_lints -Aexplicit_builtin_cfgs_in_flags")
        .run();
    p.process(&p.bin("foo")).run();
}

#[cargo_test]
fn close_output() {
    // What happens when stdout or stderr is closed during a build.

    // Server to know when rustc has spawned.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [lib]
                proc-macro = true

                [[bin]]
                name = "foobar"
            "#,
        )
        .file(
            "src/lib.rs",
            &r#"
                use proc_macro::TokenStream;
                use std::io::Read;

                #[proc_macro]
                pub fn repro(_input: TokenStream) -> TokenStream {
                    println!("hello stdout!");
                    eprintln!("hello stderr!");
                    // Tell the test we have started.
                    let mut socket = std::net::TcpStream::connect("__ADDR__").unwrap();
                    // Wait for the test to tell us to start printing.
                    let mut buf = [0];
                    drop(socket.read_exact(&mut buf));
                    let use_stderr = std::env::var("__CARGO_REPRO_STDERR").is_ok();
                    // Emit at least 1MB of data.
                    // Linux pipes can buffer up to 64KB.
                    // This test seems to be sensitive to having other threads
                    // calling fork. My hypothesis is that the stdout/stderr
                    // file descriptors are duplicated into the child process,
                    // and during the short window between fork and exec, the
                    // file descriptor is kept alive long enough for the
                    // build to finish. It's a half-baked theory, but this
                    // seems to prevent the spurious errors in CI.
                    // An alternative solution is to run this test in
                    // a single-threaded environment.
                    for i in 0..100000 {
                        if use_stderr {
                            eprintln!("0123456789{}", i);
                        } else {
                            println!("0123456789{}", i);
                        }
                    }
                    TokenStream::new()
                }
            "#
            .replace("__ADDR__", &addr.to_string()),
        )
        .file(
            "src/bin/foobar.rs",
            r#"
                foo::repro!();

                fn main() {}
            "#,
        )
        .build();

    // The `stderr` flag here indicates if this should forcefully close stderr or stdout.
    let spawn = |stderr: bool| {
        let mut cmd = p.cargo("build").build_command();
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        if stderr {
            cmd.env("__CARGO_REPRO_STDERR", "1");
        }
        let mut child = cmd.spawn().unwrap();
        // Wait for proc macro to start.
        let pm_conn = listener.accept().unwrap().0;
        // Close stderr or stdout.
        if stderr {
            drop(child.stderr.take());
        } else {
            drop(child.stdout.take());
        }
        // Tell the proc-macro to continue;
        drop(pm_conn);
        // Read the output from the other channel.
        let out: &mut dyn Read = if stderr {
            child.stdout.as_mut().unwrap()
        } else {
            child.stderr.as_mut().unwrap()
        };
        let mut result = String::new();
        out.read_to_string(&mut result).unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
        result
    };

    let stderr = spawn(false);
    assert_e2e().eq(
        &stderr,
        str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
hello stderr!
[ERROR] [BROKEN_PIPE]
[WARNING] build failed, waiting for other jobs to finish...

"#]]
        .unordered(),
    );

    // Try again with stderr.
    p.build_dir().rm_rf();
    let stdout = spawn(true);
    assert_eq!(stdout, "hello stdout!\n");
}

#[cargo_test]
fn close_output_during_drain() {
    // Test to close the output during the build phase (drain_the_queue).
    // There was a bug where it would hang.

    // Server to know when rustc has spawned.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // Create a wrapper so the test can know when compiling has started.
    let rustc_wrapper = {
        let p = project()
            .at("compiler")
            .file("Cargo.toml", &basic_manifest("compiler", "1.0.0"))
            .file(
                "src/main.rs",
                &r#"
                    use std::process::Command;
                    use std::env;
                    use std::io::Read;

                    fn main() {
                        // Only wait on the first dependency.
                        if matches!(env::var("CARGO_PKG_NAME").as_deref(), Ok("dep")) {
                            let mut socket = std::net::TcpStream::connect("__ADDR__").unwrap();
                            // Wait for the test to tell us to start printing.
                            let mut buf = [0];
                            drop(socket.read_exact(&mut buf));
                        }
                        let mut cmd = Command::new("rustc");
                        for arg in env::args_os().skip(1) {
                            cmd.arg(arg);
                        }
                        std::process::exit(cmd.status().unwrap().code().unwrap());
                    }
                "#
                .replace("__ADDR__", &addr.to_string()),
            )
            .build();
        p.cargo("build").run();
        p.bin("compiler")
    };

    Package::new("dep", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                dep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Spawn cargo, wait for the first rustc to start, and then close stderr.
    let mut cmd = process(&cargo_exe())
        .arg("check")
        .cwd(p.root())
        .env("RUSTC", rustc_wrapper)
        .build_command();
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("cargo should spawn");
    // Wait for the rustc wrapper to start.
    let rustc_conn = listener.accept().unwrap().0;
    // Close stderr to force an error.
    drop(child.stderr.take());
    // Tell the wrapper to continue.
    drop(rustc_conn);
    match child.wait() {
        Ok(status) => assert!(!status.success()),
        Err(e) => panic!("child wait failed: {}", e),
    }
}

use cargo_test_support::registry::Dependency;

#[cargo_test]
fn reduced_reproduction_8249() {
    // https://github.com/rust-lang/cargo/issues/8249
    Package::new("a-src", "0.1.0").links("a").publish();
    Package::new("a-src", "0.2.0").links("a").publish();

    Package::new("b", "0.1.0")
        .add_dep(Dependency::new("a-src", "0.1").optional(true))
        .publish();
    Package::new("b", "0.2.0")
        .add_dep(Dependency::new("a-src", "0.2").optional(true))
        .publish();

    Package::new("c", "1.0.0")
        .add_dep(&Dependency::new("b", "0.1.0"))
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                b = { version = "*", features = ["a-src"] }
                a-src = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    cargo_util::paths::append(&p.root().join("Cargo.toml"), b"c = \"*\"").unwrap();
    p.cargo("check").run();
    p.cargo("check").run();
}

#[cargo_test]
fn target_directory_backup_exclusion() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    // Newly created target/ should have CACHEDIR.TAG inside...
    p.cargo("build").run();
    let cachedir_tag = p.build_dir().join("CACHEDIR.TAG");
    assert!(cachedir_tag.is_file());
    assert!(fs::read_to_string(&cachedir_tag)
        .unwrap()
        .starts_with("Signature: 8a477f597d28d172789f06886806bc55"));
    // ...but if target/ already exists CACHEDIR.TAG should not be created in it.
    fs::remove_file(&cachedir_tag).unwrap();
    p.cargo("build").run();
    assert!(!&cachedir_tag.is_file());
}

#[cargo_test]
fn simple_terminal_width() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() {
                    let _: () = 42;
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .env("__CARGO_TEST_TTY_WIDTH_DO_NOT_USE_THIS", "20")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--diagnostic-width=20[..]
error[E0308][..]
...
[ERROR] could not compile `foo` (lib) due to 1 previous error

Caused by:
  process didn't exit successfully: `rustc [..]` ([EXIT_STATUS]: 1)

"#]])
        .run();

    p.cargo("doc -v")
        .env("__CARGO_TEST_TTY_WIDTH_DO_NOT_USE_THIS", "20")
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc [..]--diagnostic-width=20[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]])
        .run();
}

#[cargo_test]
fn build_script_o0_default() {
    let p = project()
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("build -v --release")
        .with_stderr_does_not_contain("[..]build_script_build[..]opt-level[..]")
        .run();
}

#[cargo_test]
fn build_script_o0_default_even_with_release() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [profile.release]
                opt-level = 1
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("build -v --release")
        .with_stderr_does_not_contain("[..]build_script_build[..]opt-level[..]")
        .run();
}

#[cargo_test]
fn primary_package_env_var() {
    // Test that CARGO_PRIMARY_PACKAGE is enabled only for "foo" and not for any dependency.

    let is_primary_package = r#"
        pub fn is_primary_package() -> bool {{
            option_env!("CARGO_PRIMARY_PACKAGE").is_some()
        }}
    "#;

    Package::new("qux", "0.1.0")
        .file("src/lib.rs", is_primary_package)
        .publish();

    let baz = git::new("baz", |project| {
        project
            .file("Cargo.toml", &basic_manifest("baz", "0.1.0"))
            .file("src/lib.rs", is_primary_package)
    });

    let foo = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"
                    edition = "2015"

                    [dependencies]
                    bar = {{ path = "bar" }}
                    baz = {{ git = '{}' }}
                    qux = "0.1"
                "#,
                baz.url()
            ),
        )
        .file(
            "src/lib.rs",
            &format!(
                r#"
                    extern crate bar;
                    extern crate baz;
                    extern crate qux;

                    {}

                    #[test]
                    fn verify_primary_package() {{
                        assert!(!bar::is_primary_package());
                        assert!(!baz::is_primary_package());
                        assert!(!qux::is_primary_package());
                        assert!(is_primary_package());
                    }}
                "#,
                is_primary_package
            ),
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", is_primary_package)
        .build();

    foo.cargo("test").run();
}

#[cargo_test]
fn renamed_uplifted_artifact_remains_unmodified_after_rebuild() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                edition = "2015"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();

    let bin = p.bin("foo");
    let renamed_bin = p.bin("foo-renamed");

    fs::rename(&bin, &renamed_bin).unwrap();

    p.change_file("src/main.rs", "fn main() { eprintln!(\"hello, world\"); }");
    p.cargo("build").run();

    let not_the_same = !same_file::is_same_file(bin, renamed_bin).unwrap();
    assert!(not_the_same, "renamed uplifted artifact must be unmodified");
}
