//! Tests for build.rs scripts.

use cargo_test_support::compare::assert_match_exact;
use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, cross_compile, is_coarse_mtime, project};
use cargo_test_support::{rustc_host, sleep_ms, slow_cpu_multiplier, symlink_supported};
use cargo_util::paths::remove_dir_all;
use std::env;
use std::fs;
use std::io;
use std::thread;

#[cargo_test]
fn custom_build_script_failed() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]
                build = "build.rs"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", "fn main() { std::process::exit(101); }")
        .build();
    p.cargo("build -v")
        .with_status(101)
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc --crate-name build_script_build build.rs [..]--crate-type bin [..]`
[RUNNING] `[..]/build-script-build`
[ERROR] failed to run custom build command for `foo v0.5.0 ([CWD])`

Caused by:
  process didn't exit successfully: `[..]/build-script-build` (exit [..]: 101)",
        )
        .run();
}

#[cargo_test]
fn custom_build_env_vars() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [features]
                bar_feat = ["bar/foo"]

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [project]

                name = "bar"
                version = "0.5.0"
                authors = ["wycats@example.com"]
                build = "build.rs"

                [features]
                foo = []
            "#,
        )
        .file("bar/src/lib.rs", "pub fn hello() {}");

    let file_content = format!(
        r#"
            use std::env;
            use std::io::prelude::*;
            use std::path::Path;

            fn main() {{
                let _target = env::var("TARGET").unwrap();
                let _ncpus = env::var("NUM_JOBS").unwrap();
                let _dir = env::var("CARGO_MANIFEST_DIR").unwrap();

                let opt = env::var("OPT_LEVEL").unwrap();
                assert_eq!(opt, "0");

                let opt = env::var("PROFILE").unwrap();
                assert_eq!(opt, "debug");

                let debug = env::var("DEBUG").unwrap();
                assert_eq!(debug, "true");

                let out = env::var("OUT_DIR").unwrap();
                assert!(out.starts_with(r"{0}"));
                assert!(Path::new(&out).is_dir());

                let _host = env::var("HOST").unwrap();

                let _feat = env::var("CARGO_FEATURE_FOO").unwrap();

                let _cargo = env::var("CARGO").unwrap();

                let rustc = env::var("RUSTC").unwrap();
                assert_eq!(rustc, "rustc");

                let rustdoc = env::var("RUSTDOC").unwrap();
                assert_eq!(rustdoc, "rustdoc");

                assert!(env::var("RUSTC_LINKER").is_err());
            }}
        "#,
        p.root()
            .join("target")
            .join(rustc_host())
            .join("debug")
            .join("build")
            .display()
    );

    let p = p.file("bar/build.rs", &file_content).build();

    p.cargo("build --features bar_feat").run();
}

#[cargo_test]
fn custom_build_env_var_rustc_linker() {
    if cross_compile::disabled() {
        return;
    }
    let target = cross_compile::alternate();
    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                [target.{}]
                linker = "/path/to/linker"
                "#,
                target
            ),
        )
        .file(
            "build.rs",
            r#"
            use std::env;

            fn main() {
                assert!(env::var("RUSTC_LINKER").unwrap().ends_with("/path/to/linker"));
            }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // no crate type set => linker never called => build succeeds if and
    // only if build.rs succeeds, despite linker binary not existing.
    p.cargo("build --target").arg(&target).run();
}

#[cargo_test]
fn custom_build_env_var_rustc_linker_bad_host_target() {
    let target = rustc_host();
    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                [target.{}]
                linker = "/path/to/linker"
                "#,
                target
            ),
        )
        .file(
            "build.rs",
            r#"
            use std::env;

            fn main() {
                assert!(env::var("RUSTC_LINKER").unwrap().ends_with("/path/to/linker"));
            }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // build.rs should fail since host == target when no target is set
    p.cargo("build --verbose")
        .with_status(101)
        .with_stderr_contains(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name build_script_build build.rs [..]--crate-type bin [..]-C linker=[..]/path/to/linker [..]`
[ERROR] linker `[..]/path/to/linker` not found
"
        )
        .run();
}

#[cargo_test]
fn custom_build_env_var_rustc_linker_host_target() {
    let target = rustc_host();
    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                target-applies-to-host = false
                [target.{}]
                linker = "/path/to/linker"
                "#,
                target
            ),
        )
        .file(
            "build.rs",
            r#"
            use std::env;

            fn main() {
                assert!(env::var("RUSTC_LINKER").unwrap().ends_with("/path/to/linker"));
            }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // no crate type set => linker never called => build succeeds if and
    // only if build.rs succeeds, despite linker binary not existing.
    if cargo_test_support::is_nightly() {
        p.cargo("build -Z target-applies-to-host --target")
            .arg(&target)
            .masquerade_as_nightly_cargo()
            .run();
    }
}

#[cargo_test]
fn custom_build_env_var_rustc_linker_host_target_env() {
    let target = rustc_host();
    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                [target.{}]
                linker = "/path/to/linker"
                "#,
                target
            ),
        )
        .file(
            "build.rs",
            r#"
            use std::env;

            fn main() {
                assert!(env::var("RUSTC_LINKER").unwrap().ends_with("/path/to/linker"));
            }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // no crate type set => linker never called => build succeeds if and
    // only if build.rs succeeds, despite linker binary not existing.
    if cargo_test_support::is_nightly() {
        p.cargo("build -Z target-applies-to-host --target")
            .env("CARGO_TARGET_APPLIES_TO_HOST", "false")
            .arg(&target)
            .masquerade_as_nightly_cargo()
            .run();
    }
}

#[cargo_test]
fn custom_build_invalid_host_config_feature_flag() {
    let target = rustc_host();
    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                [target.{}]
                linker = "/path/to/linker"
                "#,
                target
            ),
        )
        .file(
            "build.rs",
            r#"
            use std::env;

            fn main() {
                assert!(env::var("RUSTC_LINKER").unwrap().ends_with("/path/to/linker"));
            }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // build.rs should fail due to -Zhost-config being set without -Ztarget-applies-to-host
    if cargo_test_support::is_nightly() {
        p.cargo("build -Z host-config --target")
            .arg(&target)
            .masquerade_as_nightly_cargo()
            .with_status(101)
            .with_stderr_contains(
                "\
error: the -Zhost-config flag requires the -Ztarget-applies-to-host flag to be set
",
            )
            .run();
    }
}

#[cargo_test]
fn custom_build_env_var_rustc_linker_host_target_with_bad_host_config() {
    let target = rustc_host();
    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                target-applies-to-host = true
                [host]
                linker = "/path/to/host/linker"
                [target.{}]
                linker = "/path/to/target/linker"
                "#,
                target
            ),
        )
        .file(
            "build.rs",
            r#"
            use std::env;

            fn main() {
                assert!(env::var("RUSTC_LINKER").unwrap().ends_with("/path/to/target/linker"));
            }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // build.rs should fail due to bad target linker being set
    if cargo_test_support::is_nightly() {
        p.cargo("build -Z target-applies-to-host -Z host-config --verbose --target")
            .arg(&target)
            .masquerade_as_nightly_cargo()
            .with_status(101)
            .with_stderr_contains(
                "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name build_script_build build.rs [..]--crate-type bin [..]-C linker=[..]/path/to/target/linker [..]`
[ERROR] linker `[..]/path/to/target/linker` not found
"
            )
            .run();
    }
}

#[cargo_test]
fn custom_build_env_var_rustc_linker_bad_host() {
    let target = rustc_host();
    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                [host]
                linker = "/path/to/host/linker"
                [target.{}]
                linker = "/path/to/target/linker"
                "#,
                target
            ),
        )
        .file(
            "build.rs",
            r#"
            use std::env;

            fn main() {
                assert!(env::var("RUSTC_LINKER").unwrap().ends_with("/path/to/target/linker"));
            }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // build.rs should fail due to bad host linker being set
    if cargo_test_support::is_nightly() {
        p.cargo("build -Z target-applies-to-host -Z host-config --verbose --target")
            .arg(&target)
            .masquerade_as_nightly_cargo()
            .with_status(101)
            .with_stderr_contains(
                "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name build_script_build build.rs [..]--crate-type bin [..]-C linker=[..]/path/to/host/linker [..]`
[ERROR] linker `[..]/path/to/host/linker` not found
"
            )
            .run();
    }
}

#[cargo_test]
fn custom_build_env_var_rustc_linker_bad_host_with_arch() {
    let target = rustc_host();
    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                [host]
                linker = "/path/to/host/linker"
                [host.{}]
                linker = "/path/to/host/arch/linker"
                [target.{}]
                linker = "/path/to/target/linker"
                "#,
                target, target
            ),
        )
        .file(
            "build.rs",
            r#"
            use std::env;

            fn main() {
                assert!(env::var("RUSTC_LINKER").unwrap().ends_with("/path/to/target/linker"));
            }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // build.rs should fail due to bad host linker being set
    if cargo_test_support::is_nightly() {
        p.cargo("build -Z target-applies-to-host -Z host-config --verbose --target")
            .arg(&target)
            .masquerade_as_nightly_cargo()
            .with_status(101)
            .with_stderr_contains(
                "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name build_script_build build.rs [..]--crate-type bin [..]-C linker=[..]/path/to/host/arch/linker [..]`
[ERROR] linker `[..]/path/to/host/arch/linker` not found
"
            )
            .run();
    }
}

#[cargo_test]
fn custom_build_env_var_rustc_linker_cross_arch_host() {
    let target = rustc_host();
    let cross_target = cross_compile::alternate();
    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                [host.{}]
                linker = "/path/to/host/arch/linker"
                [target.{}]
                linker = "/path/to/target/linker"
                "#,
                cross_target, target
            ),
        )
        .file(
            "build.rs",
            r#"
            use std::env;

            fn main() {
                assert!(env::var("RUSTC_LINKER").unwrap().ends_with("/path/to/target/linker"));
            }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // build.rs should fail due to bad host linker being set
    if cargo_test_support::is_nightly() {
        p.cargo("build -Z target-applies-to-host -Z host-config --verbose --target")
            .arg(&target)
            .masquerade_as_nightly_cargo()
            .run();
    }
}

#[cargo_test]
fn custom_build_env_var_rustc_linker_bad_cross_arch_host() {
    let target = rustc_host();
    let cross_target = cross_compile::alternate();
    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                [host]
                linker = "/path/to/host/linker"
                [host.{}]
                linker = "/path/to/host/arch/linker"
                [target.{}]
                linker = "/path/to/target/linker"
                "#,
                cross_target, target
            ),
        )
        .file(
            "build.rs",
            r#"
            use std::env;

            fn main() {
                assert!(env::var("RUSTC_LINKER").unwrap().ends_with("/path/to/target/linker"));
            }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // build.rs should fail due to bad host linker being set
    if cargo_test_support::is_nightly() {
        p.cargo("build -Z target-applies-to-host -Z host-config --verbose --target")
            .arg(&target)
            .masquerade_as_nightly_cargo()
            .with_status(101)
            .with_stderr_contains(
                "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name build_script_build build.rs [..]--crate-type bin [..]-C linker=[..]/path/to/host/linker [..]`
[ERROR] linker `[..]/path/to/host/linker` not found
"
            )
            .run();
    }
}

#[cargo_test]
fn custom_build_script_wrong_rustc_flags() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]
                build = "build.rs"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo:rustc-flags=-aaa -bbb"); }"#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(
            "[ERROR] Only `-l` and `-L` flags are allowed in build script of `foo v0.5.0 ([CWD])`: \
             `-aaa -bbb`",
        )
        .run();
}

#[cargo_test]
fn custom_build_script_rustc_flags() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "bar"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.foo]
                path = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "foo/Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]
                build = "build.rs"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "foo/build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-flags=-l nonexistinglib -L /dummy/path1 -L /dummy/path2");
                }
            "#,
        )
        .build();

    p.cargo("build --verbose")
        .with_stderr(&format!(
            "\
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name build_script_build foo/build.rs [..]
[RUNNING] `[..]build-script-build`
[RUNNING] `rustc --crate-name foo foo/src/lib.rs [..]\
    -L dependency=[CWD]/target/{target}/debug/deps \
    -L dependency=[CWD]/target/host/{host}/debug/deps \
    -L /dummy/path1 -L /dummy/path2 -l nonexistinglib`
[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar src/main.rs [..]\
    -L dependency=[CWD]/target/{target}/debug/deps \
    -L dependency=[CWD]/target/host/{host}/debug/deps \
    --extern foo=[..]libfoo-[..] \
    -L /dummy/path1 -L /dummy/path2`
[FINISHED] dev [..]
",
            host = rustc_host(),
            target = rustc_host()
        ))
        .run();
}

#[cargo_test]
fn custom_build_script_rustc_flags_no_space() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "bar"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.foo]
                path = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "foo/Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]
                build = "build.rs"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "foo/build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-flags=-lnonexistinglib -L/dummy/path1 -L/dummy/path2");
                }
            "#,
        )
        .build();

    p.cargo("build --verbose")
        .with_stderr(&format!(
            "\
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name build_script_build foo/build.rs [..]
[RUNNING] `[..]build-script-build`
[RUNNING] `rustc --crate-name foo foo/src/lib.rs [..]\
    -L dependency=[CWD]/target/{target}/debug/deps \
    -L dependency=[CWD]/target/host/{host}/debug/deps \
    -L /dummy/path1 -L /dummy/path2 -l nonexistinglib`
[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar src/main.rs [..]\
    -L dependency=[CWD]/target/{target}/debug/deps \
    -L dependency=[CWD]/target/host/{host}/debug/deps \
    --extern foo=[..]libfoo-[..] \
    -L /dummy/path1 -L /dummy/path2`
[FINISHED] dev [..]
",
            host = rustc_host(),
            target = rustc_host()
        ))
        .run();
}

#[cargo_test]
fn links_no_build_cmd() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                links = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  package `foo v0.5.0 ([CWD])` specifies that it links to `a` but does \
not have a custom build script
",
        )
        .run();
}

#[cargo_test]
fn links_duplicates() {
    // this tests that the links_duplicates are caught at resolver time
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                links = "a"
                build = "build.rs"

                [dependencies.a-sys]
                path = "a-sys"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "")
        .file(
            "a-sys/Cargo.toml",
            r#"
                [project]
                name = "a-sys"
                version = "0.5.0"
                authors = []
                links = "a"
                build = "build.rs"
            "#,
        )
        .file("a-sys/src/lib.rs", "")
        .file("a-sys/build.rs", "")
        .build();

    p.cargo("build").with_status(101)
                       .with_stderr("\
error: failed to select a version for `a-sys`.
    ... required by package `foo v0.5.0 ([..])`
versions that meet the requirements `*` are: 0.5.0

the package `a-sys` links to the native library `a`, but it conflicts with a previous package which links to `a` as well:
package `foo v0.5.0 ([..])`
Only one package in the dependency graph may specify the same links value. This helps ensure that only one copy of a native library is linked in the final binary. Try to adjust your dependencies so that only one package uses the links ='a-sys' value. For more information, see https://doc.rust-lang.org/cargo/reference/resolver.html#links.

failed to select a version for `a-sys` which could resolve this conflict
").run();
}

#[cargo_test]
fn links_duplicates_old_registry() {
    // Test old links validator. See `validate_links`.
    Package::new("bar", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"
            links = "a"
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            links = "a"

            [dependencies]
            bar = "0.1"
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 ([..])
[ERROR] multiple packages link to native library `a`, \
    but a native library can be linked only once

package `bar v0.1.0`
    ... which is depended on by `foo v0.1.0 ([..]foo)`
links to native library `a`

package `foo v0.1.0 ([..]foo)`
also links to native library `a`
",
        )
        .run();
}

#[cargo_test]
fn links_duplicates_deep_dependency() {
    // this tests that the links_duplicates are caught at resolver time
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                links = "a"
                build = "build.rs"

                [dependencies.a]
                path = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                build = "build.rs"

                [dependencies.a-sys]
                path = "a-sys"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file("a/build.rs", "")
        .file(
            "a/a-sys/Cargo.toml",
            r#"
                [project]
                name = "a-sys"
                version = "0.5.0"
                authors = []
                links = "a"
                build = "build.rs"
            "#,
        )
        .file("a/a-sys/src/lib.rs", "")
        .file("a/a-sys/build.rs", "")
        .build();

    p.cargo("build").with_status(101)
                       .with_stderr("\
error: failed to select a version for `a-sys`.
    ... required by package `a v0.5.0 ([..])`
    ... which is depended on by `foo v0.5.0 ([..])`
versions that meet the requirements `*` are: 0.5.0

the package `a-sys` links to the native library `a`, but it conflicts with a previous package which links to `a` as well:
package `foo v0.5.0 ([..])`
Only one package in the dependency graph may specify the same links value. This helps ensure that only one copy of a native library is linked in the final binary. Try to adjust your dependencies so that only one package uses the links ='a-sys' value. For more information, see https://doc.rust-lang.org/cargo/reference/resolver.html#links.

failed to select a version for `a-sys` which could resolve this conflict
").run();
}

#[cargo_test]
fn overrides_and_links() {
    let target = rustc_host();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"

                [dependencies.a]
                path = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                use std::env;
                fn main() {
                    assert_eq!(env::var("DEP_FOO_FOO").ok().expect("FOO missing"),
                               "bar");
                    assert_eq!(env::var("DEP_FOO_BAR").ok().expect("BAR missing"),
                               "baz");
                }
            "#,
        )
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{}.foo]
                    rustc-flags = "-L foo -L bar"
                    foo = "bar"
                    bar = "baz"
                "#,
                target
            ),
        )
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                links = "foo"
                build = "build.rs"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file("a/build.rs", "not valid rust code")
        .build();

    p.cargo("build -v")
        .with_stderr(
            "\
[..]
[..]
[..]
[..]
[..]
[RUNNING] `rustc --crate-name foo [..] -L foo -L bar`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn unused_overrides() {
    let target = rustc_host();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{}.foo]
                    rustc-flags = "-L foo -L bar"
                    foo = "bar"
                    bar = "baz"
                "#,
                target
            ),
        )
        .build();

    p.cargo("build -v").run();
}

#[cargo_test]
fn links_passes_env_vars() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"

                [dependencies.a]
                path = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                use std::env;
                fn main() {
                    assert_eq!(env::var("DEP_FOO_FOO").unwrap(), "bar");
                    assert_eq!(env::var("DEP_FOO_BAR").unwrap(), "baz");
                }
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                links = "foo"
                build = "build.rs"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "a/build.rs",
            r#"
                use std::env;
                fn main() {
                    let lib = env::var("CARGO_MANIFEST_LINKS").unwrap();
                    assert_eq!(lib, "foo");

                    println!("cargo:foo=bar");
                    println!("cargo:bar=baz");
                }
            "#,
        )
        .build();

    p.cargo("build -v").run();
}

#[cargo_test]
fn only_rerun_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("build -v").run();
    p.root().move_into_the_past();

    p.change_file("some-new-file", "");
    p.root().move_into_the_past();

    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `[..]/build-script-build`
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn rebuild_continues_to_pass_env_vars() {
    let a = project()
        .at("a")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                links = "foo"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                use std::time::Duration;
                fn main() {
                    println!("cargo:foo=bar");
                    println!("cargo:bar=baz");
                    std::thread::sleep(Duration::from_millis(500));
                }
            "#,
        )
        .build();
    a.root().move_into_the_past();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = []
                    build = "build.rs"

                    [dependencies.a]
                    path = '{}'
                "#,
                a.root().display()
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                use std::env;
                fn main() {
                    assert_eq!(env::var("DEP_FOO_FOO").unwrap(), "bar");
                    assert_eq!(env::var("DEP_FOO_BAR").unwrap(), "baz");
                }
            "#,
        )
        .build();

    p.cargo("build -v").run();
    p.root().move_into_the_past();

    p.change_file("some-new-file", "");
    p.root().move_into_the_past();

    p.cargo("build -v").run();
}

#[cargo_test]
fn testing_and_such() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    println!("build");
    p.cargo("build -v").run();
    p.root().move_into_the_past();

    p.change_file("src/lib.rs", "");
    p.root().move_into_the_past();

    println!("test");
    p.cargo("test -vj1")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `[..]/build-script-build`
[RUNNING] `rustc --crate-name foo [..]`
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]/foo-[..][EXE]`
[DOCTEST] foo
[RUNNING] `rustdoc [..]--test [..]`",
        )
        .with_stdout_contains_n("running 0 tests", 2)
        .run();

    println!("doc");
    p.cargo("doc -v")
        .with_stderr(
            "\
[DOCUMENTING] foo v0.5.0 ([CWD])
[RUNNING] `rustdoc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.change_file("src/main.rs", "fn main() {}");
    println!("run");
    p.cargo("run")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/{}/debug/foo[EXE]`
",
            rustc_host()
        ))
        .run();
}

#[cargo_test]
fn propagation_of_l_flags() {
    let target = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                [dependencies.a]
                path = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                links = "bar"
                build = "build.rs"

                [dependencies.b]
                path = "../b"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "a/build.rs",
            r#"fn main() { println!("cargo:rustc-flags=-L bar"); }"#,
        )
        .file(
            "b/Cargo.toml",
            r#"
                [project]
                name = "b"
                version = "0.5.0"
                authors = []
                links = "foo"
                build = "build.rs"
            "#,
        )
        .file("b/src/lib.rs", "")
        .file("b/build.rs", "bad file")
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{}.foo]
                    rustc-flags = "-L foo"
                "#,
                target
            ),
        )
        .build();

    p.cargo("build -v -j1")
        .with_stderr_contains(
            "\
[RUNNING] `rustc --crate-name a [..] -L bar[..]-L foo[..]`
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc --crate-name foo [..] -L bar -L foo`
",
        )
        .run();
}

#[cargo_test]
fn propagation_of_l_flags_new() {
    let target = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                [dependencies.a]
                path = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                links = "bar"
                build = "build.rs"

                [dependencies.b]
                path = "../b"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "a/build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-search=bar");
                }
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
                [project]
                name = "b"
                version = "0.5.0"
                authors = []
                links = "foo"
                build = "build.rs"
            "#,
        )
        .file("b/src/lib.rs", "")
        .file("b/build.rs", "bad file")
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{}.foo]
                    rustc-link-search = ["foo"]
                "#,
                target
            ),
        )
        .build();

    p.cargo("build -v -j1")
        .with_stderr_contains(
            "\
[RUNNING] `rustc --crate-name a [..] -L bar[..]-L foo[..]`
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc --crate-name foo [..] -L bar -L foo`
",
        )
        .run();
}

#[cargo_test]
fn build_deps_simple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
                [build-dependencies.a]
                path = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            "
            #[allow(unused_extern_crates)]
            extern crate a;
            fn main() {}
        ",
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.5.0"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] a v0.5.0 ([CWD]/a)
[RUNNING] `rustc --crate-name a [..]`
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc [..] build.rs [..] --extern a=[..]`
[RUNNING] `[..]/foo-[..]/build-script-build`
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_deps_not_for_normal() {
    let target = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
                [build-dependencies.aaaaa]
                path = "a"
            "#,
        )
        .file(
            "src/lib.rs",
            "#[allow(unused_extern_crates)] extern crate aaaaa;",
        )
        .file(
            "build.rs",
            "
            #[allow(unused_extern_crates)]
            extern crate aaaaa;
            fn main() {}
        ",
        )
        .file("a/Cargo.toml", &basic_manifest("aaaaa", "0.5.0"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build -v --target")
        .arg(&target)
        .with_status(101)
        .with_stderr_contains("[..]can't find crate for `aaaaa`[..]")
        .with_stderr_contains(
            "\
[ERROR] could not compile `foo` due to previous error

Caused by:
  process didn't exit successfully: [..]
",
        )
        .run();
}

#[cargo_test]
fn build_cmd_with_a_build_cmd() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"

                [build-dependencies.a]
                path = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            "
            #[allow(unused_extern_crates)]
            extern crate a;
            fn main() {}
        ",
        )
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                build = "build.rs"

                [build-dependencies.b]
                path = "../b"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "a/build.rs",
            "#[allow(unused_extern_crates)] extern crate b; fn main() {}",
        )
        .file("b/Cargo.toml", &basic_manifest("b", "0.5.0"))
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr(format!(
            "\
[COMPILING] b v0.5.0 ([CWD]/b)
[RUNNING] `rustc --crate-name b [..]`
[COMPILING] a v0.5.0 ([CWD]/a)
[RUNNING] `rustc [..] a/build.rs [..] --extern b=[..]`
[RUNNING] `[..]/a-[..]/build-script-build`
[RUNNING] `rustc --crate-name a [..]lib.rs [..]--crate-type lib \
    --emit=[..]link[..]-C debuginfo=2 \
    -C metadata=[..] \
    --out-dir [..]host/{host}/debug/deps \
    -L [..]host/{host}/debug/deps`
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc --crate-name build_script_build build.rs [..]--crate-type bin \
    --emit=[..]link[..]\
    -C debuginfo=2 -C metadata=[..] --out-dir [..] \
    -L [..]host/{host}/debug/deps \
    --extern a=[..]liba[..].rlib`
[RUNNING] `[..]/foo-[..]/build-script-build`
[RUNNING] `rustc --crate-name foo [..]lib.rs [..]--crate-type lib \
    --emit=[..]link[..]-C debuginfo=2 \
    -C metadata=[..] \
    --out-dir [..] \
    -L [..]target/{target}/debug/deps \
    -L [..]host/{host}/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            host = rustc_host(),
            target = rustc_host(),
        ))
        .run();
}

#[cargo_test]
fn out_dir_is_preserved() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                use std::env;
                use std::fs::File;
                use std::path::Path;
                fn main() {
                    let out = env::var("OUT_DIR").unwrap();
                    File::create(Path::new(&out).join("foo")).unwrap();
                }
            "#,
        )
        .build();

    // Make the file
    p.cargo("build -v").run();

    // Change to asserting that it's there
    p.change_file(
        "build.rs",
        r#"
            use std::env;
            use std::fs::File;
            use std::path::Path;
            fn main() {
                let out = env::var("OUT_DIR").unwrap();
                File::open(&Path::new(&out).join("foo")).unwrap();
            }
        "#,
    );
    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name build_script_build [..]
[RUNNING] `[..]/build-script-build`
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] [..]
",
        )
        .run();

    // Run a fresh build where file should be preserved
    p.cargo("build -v")
        .with_stderr(
            "\
[FRESH] foo [..]
[FINISHED] [..]
",
        )
        .run();

    // One last time to make sure it's still there.
    p.change_file("foo", "");
    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo [..]
[RUNNING] `[..]build-script-build`
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn output_separate_lines() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-flags=-L foo");
                    println!("cargo:rustc-flags=-l static=foo");
                }
            "#,
        )
        .build();
    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc [..] build.rs [..]`
[RUNNING] `[..]/foo-[..]/build-script-build`
[RUNNING] `rustc --crate-name foo [..] -L foo -l static=foo`
[ERROR] could not find native static library [..]
",
        )
        .run();
}

#[cargo_test]
fn output_separate_lines_new() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-search=foo");
                    println!("cargo:rustc-link-lib=static=foo");
                }
            "#,
        )
        .build();
    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc [..] build.rs [..]`
[RUNNING] `[..]/foo-[..]/build-script-build`
[RUNNING] `rustc --crate-name foo [..] -L foo -l static=foo`
[ERROR] could not find native static library [..]
",
        )
        .run();
}

#[cfg(not(windows))] // FIXME(#867)
#[cargo_test]
fn code_generation() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                include!(concat!(env!("OUT_DIR"), "/hello.rs"));

                fn main() {
                    println!("{}", message());
                }
            "#,
        )
        .file(
            "build.rs",
            r#"
                use std::env;
                use std::fs;
                use std::path::PathBuf;

                fn main() {
                    let dst = PathBuf::from(env::var("OUT_DIR").unwrap());
                    fs::write(dst.join("hello.rs"),
                        "
                        pub fn message() -> &'static str {
                            \"Hello, World!\"
                        }
                        ")
                    .unwrap();
                }
            "#,
        )
        .build();

    p.cargo("run")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/{}/debug/foo`",
            rustc_host()
        ))
        .with_stdout("Hello, World!")
        .run();

    p.cargo("test").run();
}

#[cargo_test]
fn release_with_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {}
            "#,
        )
        .build();

    p.cargo("build -v --release").run();
}

#[cargo_test]
fn build_script_only() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                  [project]
                  name = "foo"
                  version = "0.0.0"
                  authors = []
                  build = "build.rs"
            "#,
        )
        .file("build.rs", r#"fn main() {}"#)
        .build();
    p.cargo("build -v")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  no targets specified in the manifest
  either src/lib.rs, src/main.rs, a [lib] section, or [[bin]] section must be present",
        )
        .run();
}

#[cargo_test]
fn shared_dep_with_a_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"

                [dependencies.a]
                path = "a"

                [build-dependencies.b]
                path = "b"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("a/build.rs", "fn main() {}")
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.5.0"
                authors = []

                [dependencies.a]
                path = "../a"
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();
    p.cargo("build -v").run();
}

#[cargo_test]
fn transitive_dep_host() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"

                [build-dependencies.b]
                path = "b"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.5.0"
                authors = []
                links = "foo"
                build = "build.rs"
            "#,
        )
        .file("a/build.rs", "fn main() {}")
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.5.0"
                authors = []

                [lib]
                name = "b"
                plugin = true

                [dependencies.a]
                path = "../a"
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();
    p.cargo("build").run();
}

#[cargo_test]
fn test_a_lib_with_a_build_command() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                include!(concat!(env!("OUT_DIR"), "/foo.rs"));

                /// ```
                /// foo::bar();
                /// ```
                pub fn bar() {
                    assert_eq!(foo(), 1);
                }
            "#,
        )
        .file(
            "build.rs",
            r#"
                use std::env;
                use std::fs;
                use std::path::PathBuf;

                fn main() {
                    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
                    fs::write(out.join("foo.rs"), "fn foo() -> i32 { 1 }").unwrap();
                }
            "#,
        )
        .build();
    p.cargo("test").run();
}

#[cargo_test]
fn test_dev_dep_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dev-dependencies.a]
                path = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("a/build.rs", "fn main() {}")
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("test").run();
}

#[cargo_test]
fn build_script_with_dynamic_native_dependency() {
    let build = project()
        .at("builder")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "builder"
                version = "0.0.1"
                authors = []

                [lib]
                name = "builder"
                crate-type = ["dylib"]
            "#,
        )
        .file("src/lib.rs", "#[no_mangle] pub extern fn foo() {}")
        .build();

    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"

                [build-dependencies.bar]
                path = "bar"
            "#,
        )
        .file("build.rs", "extern crate bar; fn main() { bar::bar() }")
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "bar/build.rs",
            r#"
                use std::env;
                use std::fs;
                use std::path::PathBuf;

                fn main() {
                    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
                    let root = PathBuf::from(env::var("BUILDER_ROOT").unwrap());
                    let file = format!("{}builder{}",
                        env::consts::DLL_PREFIX,
                        env::consts::DLL_SUFFIX);
                    let src = root.join(&file);
                    let dst = out_dir.join(&file);
                    fs::copy(src, dst).unwrap();
                    if cfg!(target_env = "msvc") {
                        fs::copy(root.join("builder.dll.lib"),
                                 out_dir.join("builder.dll.lib")).unwrap();
                    }
                    println!("cargo:rustc-link-search=native={}", out_dir.display());
                }
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                pub fn bar() {
                    #[cfg_attr(not(target_env = "msvc"), link(name = "builder"))]
                    #[cfg_attr(target_env = "msvc", link(name = "builder.dll"))]
                    extern { fn foo(); }
                    unsafe { foo() }
                }
            "#,
        )
        .build();

    build
        .cargo("build -v")
        .env("CARGO_LOG", "cargo::ops::cargo_rustc")
        .run();

    let root = build.root().join("target").join(rustc_host()).join("debug");
    foo.cargo("build -v")
        .env("BUILDER_ROOT", root)
        .env("CARGO_LOG", "cargo::ops::cargo_rustc")
        .run();
}

#[cargo_test]
fn profile_and_opt_level_set_correctly() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                  use std::env;

                  fn main() {
                      assert_eq!(env::var("OPT_LEVEL").unwrap(), "3");
                      assert_eq!(env::var("PROFILE").unwrap(), "release");
                      assert_eq!(env::var("DEBUG").unwrap(), "false");
                  }
            "#,
        )
        .build();
    p.cargo("bench").run();
}

#[cargo_test]
fn profile_debug_0() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [profile.dev]
                debug = 0
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                  use std::env;

                  fn main() {
                      assert_eq!(env::var("OPT_LEVEL").unwrap(), "0");
                      assert_eq!(env::var("PROFILE").unwrap(), "debug");
                      assert_eq!(env::var("DEBUG").unwrap(), "false");
                  }
            "#,
        )
        .build();
    p.cargo("build").run();
}

#[cargo_test]
fn build_script_with_lto() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"

                [profile.dev]
                lto = true
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();
    p.cargo("build").run();
}

#[cargo_test]
fn test_duplicate_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"
                authors = []
                build = "build.rs"

                [dependencies.bar]
                path = "bar"

                [build-dependencies.bar]
                path = "bar"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                extern crate bar;
                fn main() { bar::do_nothing() }
            "#,
        )
        .file(
            "build.rs",
            r#"
                extern crate bar;
                fn main() { bar::do_nothing() }
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn do_nothing() {}")
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn cfg_feedback() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/main.rs", "#[cfg(foo)] fn main() {}")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo:rustc-cfg=foo"); }"#,
        )
        .build();
    p.cargo("build -v").run();
}

#[cargo_test]
fn cfg_override() {
    let target = rustc_host();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                links = "a"
                build = "build.rs"
            "#,
        )
        .file("src/main.rs", "#[cfg(foo)] fn main() {}")
        .file("build.rs", "")
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{}.a]
                    rustc-cfg = ["foo"]
                "#,
                target
            ),
        )
        .build();

    p.cargo("build -v").run();
}

#[cargo_test]
fn cfg_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            r#"fn main() { println!("cargo:rustc-cfg=foo"); }"#,
        )
        .file(
            "src/lib.rs",
            r#"
                ///
                /// ```
                /// extern crate foo;
                ///
                /// fn main() {
                ///     foo::foo()
                /// }
                /// ```
                ///
                #[cfg(foo)]
                pub fn foo() {}

                #[cfg(foo)]
                #[test]
                fn test_foo() {
                    foo()
                }
            "#,
        )
        .file("tests/test.rs", "#[cfg(foo)] #[test] fn test_bar() {}")
        .build();
    p.cargo("test -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] [..] build.rs [..]
[RUNNING] `[..]/build-script-build`
[RUNNING] [..] --cfg foo[..]
[RUNNING] [..] --cfg foo[..]
[RUNNING] [..] --cfg foo[..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]/foo-[..][EXE]`
[RUNNING] `[..]/test-[..][EXE]`
[DOCTEST] foo
[RUNNING] [..] --cfg foo[..]",
        )
        .with_stdout_contains("test test_foo ... ok")
        .with_stdout_contains("test test_bar ... ok")
        .with_stdout_contains_n("test [..] ... ok", 3)
        .run();
}

#[cargo_test]
fn cfg_doc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file(
            "build.rs",
            r#"fn main() { println!("cargo:rustc-cfg=foo"); }"#,
        )
        .file("src/lib.rs", "#[cfg(foo)] pub fn foo() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "bar/build.rs",
            r#"fn main() { println!("cargo:rustc-cfg=bar"); }"#,
        )
        .file("bar/src/lib.rs", "#[cfg(bar)] pub fn bar() {}")
        .build();
    p.cargo("doc").run();
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc")
        .is_dir());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/fn.foo.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/fn.bar.html")
        .is_file());
}

#[cargo_test]
fn cfg_override_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
                links = "a"
            "#,
        )
        .file("build.rs", "")
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{}.a]
                    rustc-cfg = ["foo"]
                "#,
                rustc_host()
            ),
        )
        .file(
            "src/lib.rs",
            r#"
                ///
                /// ```
                /// extern crate foo;
                ///
                /// fn main() {
                ///     foo::foo()
                /// }
                /// ```
                ///
                #[cfg(foo)]
                pub fn foo() {}

                #[cfg(foo)]
                #[test]
                fn test_foo() {
                    foo()
                }
            "#,
        )
        .file("tests/test.rs", "#[cfg(foo)] #[test] fn test_bar() {}")
        .build();
    p.cargo("test -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `[..]`
[RUNNING] `[..]`
[RUNNING] `[..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]/foo-[..][EXE]`
[RUNNING] `[..]/test-[..][EXE]`
[DOCTEST] foo
[RUNNING] [..] --cfg foo[..]",
        )
        .with_stdout_contains("test test_foo ... ok")
        .with_stdout_contains("test test_bar ... ok")
        .with_stdout_contains_n("test [..] ... ok", 3)
        .run();
}

#[cargo_test]
fn cfg_override_doc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
                links = "a"

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{target}.a]
                    rustc-cfg = ["foo"]
                    [target.{target}.b]
                    rustc-cfg = ["bar"]
                "#,
                target = rustc_host()
            ),
        )
        .file("build.rs", "")
        .file("src/lib.rs", "#[cfg(foo)] pub fn foo() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []
                build = "build.rs"
                links = "b"
            "#,
        )
        .file("bar/build.rs", "")
        .file("bar/src/lib.rs", "#[cfg(bar)] pub fn bar() {}")
        .build();
    p.cargo("doc").run();
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc")
        .is_dir());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/fn.foo.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/fn.bar.html")
        .is_file());
}

#[cargo_test]
fn env_build() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                const FOO: &'static str = env!("FOO");
                fn main() {
                    println!("{}", FOO);
                }
            "#,
        )
        .file(
            "build.rs",
            r#"fn main() { println!("cargo:rustc-env=FOO=foo"); }"#,
        )
        .build();
    p.cargo("build -v").run();
    p.cargo("run -v").with_stdout("foo\n").run();
}

#[cargo_test]
fn env_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            r#"fn main() { println!("cargo:rustc-env=FOO=foo"); }"#,
        )
        .file(
            "src/lib.rs",
            r#"pub const FOO: &'static str = env!("FOO"); "#,
        )
        .file(
            "tests/test.rs",
            r#"
                extern crate foo;

                #[test]
                fn test_foo() {
                    assert_eq!("foo", foo::FOO);
                }
            "#,
        )
        .build();
    p.cargo("test -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] [..] build.rs [..]
[RUNNING] `[..]/build-script-build`
[RUNNING] [..] --crate-name foo[..]
[RUNNING] [..] --crate-name foo[..]
[RUNNING] [..] --crate-name test[..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]/foo-[..][EXE]`
[RUNNING] `[..]/test-[..][EXE]`
[DOCTEST] foo
[RUNNING] [..] --crate-name foo[..]",
        )
        .with_stdout_contains_n("running 0 tests", 2)
        .with_stdout_contains("test test_foo ... ok")
        .run();
}

#[cargo_test]
fn env_doc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                const FOO: &'static str = env!("FOO");
                fn main() {}
            "#,
        )
        .file(
            "build.rs",
            r#"fn main() { println!("cargo:rustc-env=FOO=foo"); }"#,
        )
        .build();
    p.cargo("doc -v").run();
}

#[cargo_test]
fn flags_go_into_tests() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                b = { path = "b" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("tests/foo.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [project]
                name = "b"
                version = "0.5.0"
                authors = []
                [dependencies]
                a = { path = "../a" }
            "#,
        )
        .file("b/src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "a/build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-search=test");
                }
            "#,
        )
        .build();

    p.cargo("test -v --test=foo")
        .with_stderr(
            "\
[COMPILING] a v0.5.0 ([..]
[RUNNING] `rustc [..] a/build.rs [..]`
[RUNNING] `[..]/build-script-build`
[RUNNING] `rustc [..] a/src/lib.rs [..] -L test[..]`
[COMPILING] b v0.5.0 ([..]
[RUNNING] `rustc [..] b/src/lib.rs [..] -L test[..]`
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..] src/lib.rs [..] -L test[..]`
[RUNNING] `rustc [..] tests/foo.rs [..] -L test[..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]/foo-[..][EXE]`",
        )
        .with_stdout_contains("running 0 tests")
        .run();

    p.cargo("test -v -pb --lib")
        .with_stderr(
            "\
[FRESH] a v0.5.0 ([..]
[COMPILING] b v0.5.0 ([..]
[RUNNING] `rustc [..] b/src/lib.rs [..] -L test[..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]/b-[..][EXE]`",
        )
        .with_stdout_contains("running 0 tests")
        .run();
}

#[cargo_test]
fn diamond_passes_args_only_once() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = { path = "a" }
                b = { path = "b" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("tests/foo.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                [dependencies]
                b = { path = "../b" }
                c = { path = "../c" }
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [project]
                name = "b"
                version = "0.5.0"
                authors = []
                [dependencies]
                c = { path = "../c" }
            "#,
        )
        .file("b/src/lib.rs", "")
        .file(
            "c/Cargo.toml",
            r#"
                [project]
                name = "c"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "c/build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-search=native=test");
                }
            "#,
        )
        .file("c/src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] c v0.5.0 ([..]
[RUNNING] `rustc [..]`
[RUNNING] `[..]`
[RUNNING] `rustc [..]`
[COMPILING] b v0.5.0 ([..]
[RUNNING] `rustc [..]`
[COMPILING] a v0.5.0 ([..]
[RUNNING] `rustc [..]`
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `[..]rmeta -L native=test`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn adding_an_override_invalidates() {
    let target = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                links = "foo"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(".cargo/config", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-search=native=foo");
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..]`
[RUNNING] `[..]`
[RUNNING] `rustc [..] -L native=foo`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.change_file(
        ".cargo/config",
        &format!(
            "
                [target.{}.foo]
                rustc-link-search = [\"native=bar\"]
            ",
            target
        ),
    );

    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..] -L native=bar`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn changing_an_override_invalidates() {
    let target = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                links = "foo"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            &format!(
                "
            [target.{}.foo]
            rustc-link-search = [\"native=foo\"]
        ",
                target
            ),
        )
        .file("build.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..] -L native=foo`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.change_file(
        ".cargo/config",
        &format!(
            "
                [target.{}.foo]
                rustc-link-search = [\"native=bar\"]
            ",
            target
        ),
    );

    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..] -L native=bar`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn fresh_builds_possible_with_link_libs() {
    // The bug is non-deterministic. Sometimes you can get a fresh build
    let target = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                links = "nativefoo"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            &format!(
                "
            [target.{}.nativefoo]
            rustc-link-lib = [\"a\"]
            rustc-link-search = [\"./b\"]
            rustc-flags = \"-l z -L ./\"
        ",
                target
            ),
        )
        .file("build.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("build -v")
        .with_stderr(
            "\
[FRESH] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn fresh_builds_possible_with_multiple_metadata_overrides() {
    // The bug is non-deterministic. Sometimes you can get a fresh build
    let target = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                links = "foo"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            &format!(
                "
            [target.{}.foo]
            a = \"\"
            b = \"\"
            c = \"\"
            d = \"\"
            e = \"\"
        ",
                target
            ),
        )
        .file("build.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("build -v")
        .env("CARGO_LOG", "cargo::ops::cargo_rustc::fingerprint=info")
        .with_stderr(
            "\
[FRESH] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn generate_good_d_files() {
    // this is here to stop regression on an issue where build.rs rerun-if-changed paths aren't
    // made absolute properly, which in turn interacts poorly with the dep-info-basedir setting,
    // and the dep-info files have other-crate-relative paths spat out in them
    let p = project()
        .file(
            "awoo/Cargo.toml",
            r#"
                [project]
                name = "awoo"
                version = "0.5.0"
                build = "build.rs"
            "#,
        )
        .file("awoo/src/lib.rs", "")
        .file(
            "awoo/build.rs",
            r#"
                fn main() {
                    println!("cargo:rerun-if-changed=build.rs");
                    println!("cargo:rerun-if-changed=barkbarkbark");
                }
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "meow"
                version = "0.5.0"
                [dependencies]
                awoo = { path = "awoo" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v").run();

    let dot_d_path = p.bin("meow").with_extension("d");
    println!("*meow at* {:?}", dot_d_path);
    let dot_d = fs::read_to_string(&dot_d_path).unwrap();

    println!("*.d file content*: {}", &dot_d);

    assert_match_exact(
        &format!(
            "[..]/target/{}/debug/meow[EXE]: [..]/awoo/barkbarkbark [..]/awoo/build.rs[..]",
            rustc_host()
        ),
        &dot_d,
    );

    // paths relative to dependency roots should not be allowed
    assert!(!dot_d
        .split_whitespace()
        .any(|v| v == "barkbarkbark" || v == "build.rs"));

    p.change_file(
        ".cargo/config.toml",
        r#"
        [build]
        dep-info-basedir="."
    "#,
    );
    p.cargo("build -v").run();

    let dot_d = fs::read_to_string(&dot_d_path).unwrap();

    println!("*.d file content with dep-info-basedir*: {}", &dot_d);

    assert_match_exact(
        &format!(
            "target/{}/debug/meow[EXE]: awoo/barkbarkbark awoo/build.rs[..]",
            rustc_host()
        ),
        &dot_d,
    );

    // paths relative to dependency roots should not be allowed
    assert!(!dot_d
        .split_whitespace()
        .any(|v| v == "barkbarkbark" || v == "build.rs"));
}

#[ignore]
#[cargo_test]
fn rebuild_only_on_explicit_paths() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rerun-if-changed=foo");
                    println!("cargo:rerun-if-changed=bar");
                }
            "#,
        )
        .build();

    p.cargo("build -v").run();

    // files don't exist, so should always rerun if they don't exist
    println!("run without");
    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `[..]/build-script-build`
[RUNNING] `rustc [..] src/lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    sleep_ms(1000);
    p.change_file("foo", "");
    p.change_file("bar", "");
    sleep_ms(1000); // make sure the to-be-created outfile has a timestamp distinct from the infiles

    // now the exist, so run once, catch the mtime, then shouldn't run again
    println!("run with");
    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `[..]/build-script-build`
[RUNNING] `rustc [..] src/lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    println!("run with2");
    p.cargo("build -v")
        .with_stderr(
            "\
[FRESH] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    sleep_ms(1000);

    // random other files do not affect freshness
    println!("run baz");
    p.change_file("baz", "");
    p.cargo("build -v")
        .with_stderr(
            "\
[FRESH] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    // but changing dependent files does
    println!("run foo change");
    p.change_file("foo", "");
    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `[..]/build-script-build`
[RUNNING] `rustc [..] src/lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    // .. as does deleting a file
    println!("run foo delete");
    fs::remove_file(p.root().join("bar")).unwrap();
    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `[..]/build-script-build`
[RUNNING] `rustc [..] src/lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn doctest_receives_build_link_args() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                [dependencies.a]
                path = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                links = "bar"
                build = "build.rs"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "a/build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-search=native=bar");
                }
            "#,
        )
        .build();

    p.cargo("test -v")
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..]--crate-name foo --test [..]-L native=bar[..]`",
        )
        .run();
}

#[cargo_test]
fn please_respect_the_dag() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"

                [dependencies]
                a = { path = 'a' }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-search=native=foo");
                }
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                links = "bar"
                build = "build.rs"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "a/build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-search=native=bar");
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .with_stderr_contains("[RUNNING] `rustc [..] -L native=foo -L native=bar[..]`")
        .run();
}

#[cargo_test]
fn non_utf8_output() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            r#"
                use std::io::prelude::*;

                fn main() {
                    let mut out = std::io::stdout();
                    // print something that's not utf8
                    out.write_all(b"\xff\xff\n").unwrap();

                    // now print some cargo metadata that's utf8
                    println!("cargo:rustc-cfg=foo");

                    // now print more non-utf8
                    out.write_all(b"\xff\xff\n").unwrap();
                }
            "#,
        )
        .file("src/main.rs", "#[cfg(foo)] fn main() {}")
        .build();

    p.cargo("build -v").run();
}

#[cargo_test]
fn custom_target_dir() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = { path = "a" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
                [build]
                target-dir = 'test'
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("a/build.rs", "fn main() {}")
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build -v").run();
}

#[cargo_test]
fn panic_abort_with_build_scripts() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [profile.release]
                panic = 'abort'

                [dependencies]
                a = { path = "a" }
            "#,
        )
        .file(
            "src/lib.rs",
            "#[allow(unused_extern_crates)] extern crate a;",
        )
        .file("build.rs", "fn main() {}")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                build = "build.rs"

                [build-dependencies]
                b = { path = "../b" }
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "a/build.rs",
            "#[allow(unused_extern_crates)] extern crate b; fn main() {}",
        )
        .file(
            "b/Cargo.toml",
            r#"
                [project]
                name = "b"
                version = "0.5.0"
                authors = []
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("build -v --release").run();

    p.root().join("target").rm_rf();

    p.cargo("test --release -v")
        .with_stderr_does_not_contain("[..]panic[..]")
        .run();
}

#[cargo_test]
fn warnings_emitted() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:warning=foo");
                    println!("cargo:warning=bar");
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `rustc [..]`
[RUNNING] `[..]`
warning: foo
warning: bar
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn warnings_emitted_when_build_script_panics() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:warning=foo");
                    println!("cargo:warning=bar");
                    panic!();
                }
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stdout("")
        .with_stderr_contains("warning: foo\nwarning: bar")
        .run();
}

#[cargo_test]
fn warnings_hidden_for_upstream() {
    Package::new("bar", "0.1.0")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:warning=foo");
                    println!("cargo:warning=bar");
                }
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 ([..])
[COMPILING] bar v0.1.0
[RUNNING] `rustc [..]`
[RUNNING] `[..]`
[RUNNING] `rustc [..]`
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn warnings_printed_on_vv() {
    Package::new("bar", "0.1.0")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:warning=foo");
                    println!("cargo:warning=bar");
                }
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -vv")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 ([..])
[COMPILING] bar v0.1.0
[RUNNING] `[..] rustc [..]`
[RUNNING] `[..]`
warning: foo
warning: bar
[RUNNING] `[..] rustc [..]`
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `[..] rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn output_shows_on_vv() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                use std::io::prelude::*;

                fn main() {
                    std::io::stderr().write_all(b"stderr\n").unwrap();
                    std::io::stdout().write_all(b"stdout\n").unwrap();
                }
            "#,
        )
        .build();

    p.cargo("build -vv")
        .with_stdout("[foo 0.5.0] stdout")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `[..] rustc [..]`
[RUNNING] `[..]`
[foo 0.5.0] stderr
[RUNNING] `[..] rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn links_with_dots() {
    let target = rustc_host();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
                links = "a.b"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-search=bar")
                }
            "#,
        )
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{}.'a.b']
                    rustc-link-search = ["foo"]
                "#,
                target
            ),
        )
        .build();

    p.cargo("build -v")
        .with_stderr_contains("[RUNNING] `rustc --crate-name foo [..] [..] -L foo[..]`")
        .run();
}

#[cargo_test]
fn rustc_and_rustdoc_set_correctly() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                  use std::env;

                  fn main() {
                      assert_eq!(env::var("RUSTC").unwrap(), "rustc");
                      assert_eq!(env::var("RUSTDOC").unwrap(), "rustdoc");
                  }
            "#,
        )
        .build();
    p.cargo("bench").run();
}

#[cargo_test]
fn cfg_env_vars_available() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                use std::env;

                fn main() {
                    let fam = env::var("CARGO_CFG_TARGET_FAMILY").unwrap();
                    if cfg!(unix) {
                        assert_eq!(fam, "unix");
                    } else {
                        assert_eq!(fam, "windows");
                    }
                }
            "#,
        )
        .build();
    p.cargo("bench").run();
}

#[cargo_test]
fn switch_features_rerun() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"

                [features]
                foo = []
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    println!(include_str!(concat!(env!("OUT_DIR"), "/output")));
                }
            "#,
        )
        .file(
            "build.rs",
            r#"
                use std::env;
                use std::fs;
                use std::path::Path;

                fn main() {
                    let out_dir = env::var_os("OUT_DIR").unwrap();
                    let output = Path::new(&out_dir).join("output");

                    if env::var_os("CARGO_FEATURE_FOO").is_some() {
                        fs::write(output, "foo").unwrap();
                    } else {
                        fs::write(output, "bar").unwrap();
                    }
                }
            "#,
        )
        .build();

    p.cargo("build -v --features=foo").run();
    p.rename_run("foo", "with_foo").with_stdout("foo\n").run();
    p.cargo("build -v").run();
    p.rename_run("foo", "without_foo")
        .with_stdout("bar\n")
        .run();
    p.cargo("build -v --features=foo").run();
    p.rename_run("foo", "with_foo2").with_stdout("foo\n").run();
}

#[cargo_test]
fn assume_build_script_when_build_rs_present() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    if ! cfg!(foo) {
                        panic!("the build script was not run");
                    }
                }
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-cfg=foo");
                }
            "#,
        )
        .build();

    p.cargo("run -v").run();
}

#[cargo_test]
fn if_build_set_to_false_dont_treat_build_rs_as_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = false
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    if cfg!(foo) {
                        panic!("the build script was run");
                    }
                }
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-cfg=foo");
                }
            "#,
        )
        .build();

    p.cargo("run -v").run();
}

#[cargo_test]
fn deterministic_rustc_dependency_flags() {
    // This bug is non-deterministic hence the large number of dependencies
    // in the hopes it will have a much higher chance of triggering it.

    Package::new("dep1", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "dep1"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-flags=-L native=test1");
                }
            "#,
        )
        .file("src/lib.rs", "")
        .publish();
    Package::new("dep2", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "dep2"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-flags=-L native=test2");
                }
            "#,
        )
        .file("src/lib.rs", "")
        .publish();
    Package::new("dep3", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "dep3"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-flags=-L native=test3");
                }
            "#,
        )
        .file("src/lib.rs", "")
        .publish();
    Package::new("dep4", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "dep4"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-flags=-L native=test4");
                }
            "#,
        )
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                dep1 = "*"
                dep2 = "*"
                dep3 = "*"
                dep4 = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .with_stderr_contains(
            "\
[RUNNING] `rustc --crate-name foo [..] -L native=test1 -L native=test2 \
-L native=test3 -L native=test4`
",
        )
        .run();
}

#[cargo_test]
fn links_duplicates_with_cycle() {
    // this tests that the links_duplicates are caught at resolver time
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                links = "a"
                build = "build.rs"

                [dependencies.a]
                path = "a"

                [dev-dependencies]
                b = { path = "b" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                links = "a"
                build = "build.rs"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file("a/build.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [project]
                name = "b"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = { path = ".." }
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("build").with_status(101)
                       .with_stderr("\
error: failed to select a version for `a`.
    ... required by package `foo v0.5.0 ([..])`
versions that meet the requirements `*` are: 0.5.0

the package `a` links to the native library `a`, but it conflicts with a previous package which links to `a` as well:
package `foo v0.5.0 ([..])`
Only one package in the dependency graph may specify the same links value. This helps ensure that only one copy of a native library is linked in the final binary. Try to adjust your dependencies so that only one package uses the links ='a' value. For more information, see https://doc.rust-lang.org/cargo/reference/resolver.html#links.

failed to select a version for `a` which could resolve this conflict
").run();
}

#[ignore]
#[cargo_test]
fn rename_with_link_search_path() {
    _rename_with_link_search_path(false);
}

#[ignore]
#[cargo_test]
// Don't have a cdylib cross target on macos.
// #[cfg_attr(target_os = "macos", ignore)]
fn rename_with_link_search_path_cross() {
    if cross_compile::disabled() {
        return;
    }

    _rename_with_link_search_path(true);
}

fn _rename_with_link_search_path(cross: bool) {
    let target_arg = if cross {
        format!(" --target={}", cross_compile::alternate())
    } else {
        "".to_string()
    };
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [lib]
                crate-type = ["cdylib"]
            "#,
        )
        .file(
            "src/lib.rs",
            "#[no_mangle] pub extern fn cargo_test_foo() {}",
        );
    let p = p.build();

    p.cargo(&format!("build{}", target_arg)).run();

    let p2 = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file(
            "build.rs",
            r#"
                use std::env;
                use std::fs;
                use std::path::PathBuf;

                fn main() {
                    // Move the `libfoo.so` from the root of our project into the
                    // build directory. This way Cargo should automatically manage
                    // `LD_LIBRARY_PATH` and such.
                    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
                    let file = format!("{}foo{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX);
                    let src = root.join(&file);

                    let dst_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
                    let dst = dst_dir.join(&file);

                    fs::copy(&src, &dst).unwrap();
                    // handle windows, like below
                    drop(fs::copy(root.join("foo.dll.lib"), dst_dir.join("foo.dll.lib")));

                    println!("cargo:rerun-if-changed=build.rs");
                    if cfg!(target_env = "msvc") {
                        println!("cargo:rustc-link-lib=foo.dll");
                    } else {
                        println!("cargo:rustc-link-lib=foo");
                    }
                    println!("cargo:rustc-link-search=all={}",
                             dst.parent().unwrap().display());
                }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                extern {
                    #[link_name = "cargo_test_foo"]
                    fn foo();
                }

                fn main() {
                    unsafe { foo(); }
                }
            "#,
        );
    let p2 = p2.build();

    // Move the output `libfoo.so` into the directory of `p2`, and then delete
    // the `p` project. On macOS, the `libfoo.dylib` artifact references the
    // original path in `p` so we want to make sure that it can't find it (hence
    // the deletion).
    let root = if cross {
        p.root()
            .join("target")
            .join(cross_compile::alternate())
            .join("debug")
            .join("deps")
    } else {
        p.root()
            .join("target")
            .join(rustc_host())
            .join("debug")
            .join("deps")
    };
    let file = format!("{}foo{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX);
    let src = root.join(&file);

    let dst = p2.root().join(&file);

    fs::copy(&src, &dst).unwrap();
    // copy the import library for windows, if it exists
    drop(fs::copy(
        &root.join("foo.dll.lib"),
        p2.root().join("foo.dll.lib"),
    ));
    remove_dir_all(p.root()).unwrap();

    // Everything should work the first time
    p2.cargo(&format!("run{}", target_arg)).run();

    // Now rename the root directory and rerun `cargo run`. Not only should we
    // not build anything but we also shouldn't crash.
    let mut new = p2.root();
    new.pop();
    new.push("bar2");

    // For whatever reason on Windows right after we execute a binary it's very
    // unlikely that we're able to successfully delete or rename that binary.
    // It's not really clear why this is the case or if it's a bug in Cargo
    // holding a handle open too long. In an effort to reduce the flakiness of
    // this test though we throw this in a loop
    //
    // For some more information see #5481 and rust-lang/rust#48775
    let mut i = 0;
    loop {
        let error = match fs::rename(p2.root(), &new) {
            Ok(()) => break,
            Err(e) => e,
        };
        i += 1;
        if !cfg!(windows) || error.kind() != io::ErrorKind::PermissionDenied || i > 10 {
            panic!("failed to rename: {}", error);
        }
        println!("assuming {} is spurious, waiting to try again", error);
        thread::sleep(slow_cpu_multiplier(100));
    }

    p2.cargo(&format!("run{}", target_arg))
        .cwd(&new)
        .with_stderr(
            "\
[FINISHED] [..]
[RUNNING] [..]
",
        )
        .run();
}

#[cargo_test]
fn optional_build_script_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                bar = { path = "bar", optional = true }

                [build-dependencies]
                bar = { path = "bar", optional = true }
            "#,
        )
        .file(
            "build.rs",
            r#"
                #[cfg(feature = "bar")]
                extern crate bar;

                fn main() {
                    #[cfg(feature = "bar")] {
                        println!("cargo:rustc-env=FOO={}", bar::bar());
                        return
                    }
                    println!("cargo:rustc-env=FOO=0");
                }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[cfg(feature = "bar")]
                extern crate bar;

                fn main() {
                    println!("{}", env!("FOO"));
                }
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("bar/src/lib.rs", "pub fn bar() -> u32 { 1 }");
    let p = p.build();

    p.cargo("run").with_stdout("0\n").run();
    p.cargo("run --features bar").with_stdout("1\n").run();
}

#[cargo_test]
fn optional_build_dep_and_required_normal_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [dependencies]
            bar = { path = "./bar", optional = true }

            [build-dependencies]
            bar = { path = "./bar" }
            "#,
        )
        .file("build.rs", "extern crate bar; fn main() { bar::bar(); }")
        .file(
            "src/main.rs",
            r#"
                #[cfg(feature = "bar")]
                extern crate bar;

                fn main() {
                    #[cfg(feature = "bar")] {
                        println!("{}", bar::bar());
                    }
                    #[cfg(not(feature = "bar"))] {
                        println!("0");
                    }
                }
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("bar/src/lib.rs", "pub fn bar() -> u32 { 1 }");
    let p = p.build();

    p.cargo("run")
        .with_stdout("0")
        .with_stderr(
            "\
[COMPILING] bar v0.5.0 ([..])
[COMPILING] foo v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]foo[EXE]`",
        )
        .run();

    p.cargo("run --all-features")
        .with_stdout("1")
        .with_stderr(
            "\
[COMPILING] bar v0.5.0 ([..])
[COMPILING] foo v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]foo[EXE]`",
        )
        .run();
}

#[ignore]
#[cargo_test]
fn using_rerun_if_changed_does_not_rebuild() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rerun-if-changed=build.rs");
                }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
}

#[cargo_test]
fn links_interrupted_can_restart() {
    // Test for a `links` dependent build script getting canceled and then
    // restarted. Steps:
    // 1. Build to establish fingerprints.
    // 2. Change something (an env var in this case) that triggers the
    //    dependent build script to run again. Kill the top-level build script
    //    while it is running (such as hitting Ctrl-C).
    // 3. Run the build again, it should re-run the build script.
    let bar = project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.5.0"
            authors = []
            links = "foo"
            build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo:rerun-if-env-changed=SOMEVAR");
            }
            "#,
        )
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"

                [dependencies.bar]
                path = '{}'
                "#,
                bar.root().display()
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
            use std::env;
            fn main() {
                println!("cargo:rebuild-if-changed=build.rs");
                if std::path::Path::new("abort").exists() {
                    panic!("Crash!");
                }
            }
            "#,
        )
        .build();

    p.cargo("build").run();
    // Simulate the user hitting Ctrl-C during a build.
    p.change_file("abort", "");
    // Set SOMEVAR to trigger a rebuild.
    p.cargo("build")
        .env("SOMEVAR", "1")
        .with_stderr_contains("[..]Crash![..]")
        .with_status(101)
        .run();
    fs::remove_file(p.root().join("abort")).unwrap();
    // Try again without aborting the script.
    // ***This is currently broken, the script does not re-run.
    p.cargo("build -v")
        .env("SOMEVAR", "1")
        .with_stderr_contains("[RUNNING] [..]/foo-[..]/build-script-build[..]")
        .run();
}

#[cargo_test]
#[cfg(unix)]
fn build_script_scan_eacces() {
    // build.rs causes a scan of the whole project, which can be a problem if
    // a directory is not accessible.
    use cargo_test_support::git;
    use std::os::unix::fs::PermissionsExt;

    let p = project()
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file("secrets/stuff", "")
        .build();
    let path = p.root().join("secrets");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
    // The last "Caused by" is a string from libc such as the following:
    //   Permission denied (os error 13)
    p.cargo("build")
        .with_stderr(
            "\
[ERROR] failed to determine package fingerprint for build script for foo v0.0.1 ([..]/foo)

Caused by:
  failed to determine the most recently modified file in [..]/foo

Caused by:
  failed to determine list of files in [..]/foo

Caused by:
  cannot read \"[..]/foo/secrets\"

Caused by:
  [..]
",
        )
        .with_status(101)
        .run();

    // Try `package.exclude` to skip a directory.
    p.change_file(
        "Cargo.toml",
        r#"
        [package]
        name = "foo"
        version = "0.0.1"
        exclude = ["secrets"]
        "#,
    );
    p.cargo("build").run();

    // Try with git. This succeeds because the git status walker ignores
    // directories it can't access.
    p.change_file("Cargo.toml", &basic_manifest("foo", "0.0.1"));
    p.build_dir().rm_rf();
    let repo = git::init(&p.root());
    git::add(&repo);
    git::commit(&repo);
    p.cargo("build").run();

    // Restore permissions so that the directory can be deleted.
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[cargo_test]
fn dev_dep_with_links() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
                links = "x"

                [dev-dependencies]
                bar = { path = "./bar" }
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
                links = "y"

                [dependencies]
                foo = { path = ".." }
            "#,
        )
        .file("bar/build.rs", "fn main() {}")
        .file("bar/src/lib.rs", "")
        .build();
    p.cargo("check --tests").run()
}

#[ignore]
#[cargo_test]
fn rerun_if_directory() {
    if !symlink_supported() {
        return;
    }

    // rerun-if-changed of a directory should rerun if any file in the directory changes.
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rerun-if-changed=somedir");
                }
            "#,
        )
        .build();

    let dirty = || {
        p.cargo("check")
            .with_stderr(
                "[COMPILING] foo [..]\n\
                 [FINISHED] [..]",
            )
            .run();
    };

    let fresh = || {
        p.cargo("check").with_stderr("[FINISHED] [..]").run();
    };

    // Start with a missing directory.
    dirty();
    // Because the directory doesn't exist, it will trigger a rebuild every time.
    // https://github.com/rust-lang/cargo/issues/6003
    dirty();

    if is_coarse_mtime() {
        sleep_ms(1000);
    }

    // Empty directory.
    fs::create_dir(p.root().join("somedir")).unwrap();
    dirty();
    fresh();

    if is_coarse_mtime() {
        sleep_ms(1000);
    }

    // Add a file.
    p.change_file("somedir/foo", "");
    p.change_file("somedir/bar", "");
    dirty();
    fresh();

    if is_coarse_mtime() {
        sleep_ms(1000);
    }

    // Add a symlink.
    p.symlink("foo", "somedir/link");
    dirty();
    fresh();

    if is_coarse_mtime() {
        sleep_ms(1000);
    }

    // Move the symlink.
    fs::remove_file(p.root().join("somedir/link")).unwrap();
    p.symlink("bar", "somedir/link");
    dirty();
    fresh();

    if is_coarse_mtime() {
        sleep_ms(1000);
    }

    // Remove a file.
    fs::remove_file(p.root().join("somedir/foo")).unwrap();
    dirty();
    fresh();
}

#[cargo_test]
fn test_with_dep_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { path = 'bar' }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    assert_eq!(std::env::var("DEP_BAR_FOO").unwrap(), "bar");
                }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                links = 'bar'
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "bar/build.rs",
            r#"
                fn main() {
                    println!("cargo:foo=bar");
                }
            "#,
        )
        .build();
    p.cargo("test --lib").run();
}

#[cargo_test]
fn duplicate_script_with_extra_env() {
    // Test where a build script is run twice, that emits different rustc-env
    // and rustc-cfg values. In this case, one is run for host, the other for
    // target.
    if !cross_compile::can_run_on_host() {
        return;
    }

    let target = cross_compile::alternate();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "pm"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                pm = { path = "../pm" }
            "#,
        )
        .file(
            "foo/src/lib.rs",
            &r#"
                //! ```rust
                //! #[cfg(not(mycfg="{target}"))]
                //! compile_error!{"expected mycfg set"}
                //! assert_eq!(env!("CRATE_TARGET"), "{target}");
                //! assert_eq!(std::env::var("CRATE_TARGET").unwrap(), "{target}");
                //! ```

                #[test]
                fn check_target() {
                    #[cfg(not(mycfg="{target}"))]
                    compile_error!{"expected mycfg set"}
                    // Compile-time assertion.
                    assert_eq!(env!("CRATE_TARGET"), "{target}");
                    // Run-time assertion.
                    assert_eq!(std::env::var("CRATE_TARGET").unwrap(), "{target}");
                }
            "#
            .replace("{target}", target),
        )
        .file(
            "foo/build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-env=CRATE_TARGET={}", std::env::var("TARGET").unwrap());
                    println!("cargo:rustc-cfg=mycfg=\"{}\"", std::env::var("TARGET").unwrap());
                }
            "#,
        )
        .file(
            "pm/Cargo.toml",
            r#"
                [package]
                name = "pm"
                version = "0.1.0"

                [lib]
                proc-macro = true
                # This is just here to speed things up.
                doctest = false

                [dev-dependencies]
                foo = { path = "../foo" }
            "#,
        )
        .file("pm/src/lib.rs", "")
        .build();

    p.cargo("test --workspace --target")
        .arg(&target)
        .with_stdout_contains("test check_target ... ok")
        .run();

    if cargo_test_support::is_nightly() {
        p.cargo("test --workspace -Z doctest-xcompile --doc --target")
            .arg(&target)
            .masquerade_as_nightly_cargo()
            .with_stdout_contains("test src/lib.rs - (line 2) ... ok")
            .run();
    }
}
