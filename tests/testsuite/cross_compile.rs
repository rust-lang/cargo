//! Tests for cross compiling with --target.
//!
//! See `cargo_test_support::cross_compile` for more detail.

use crate::prelude::*;
use cargo_test_support::rustc_host;
use cargo_test_support::str;
use cargo_test_support::{basic_bin_manifest, basic_manifest, cross_compile, project};

use crate::utils::cross_compile::{
    can_run_on_host as cross_compile_can_run_on_host, disabled as cross_compile_disabled,
};

#[cargo_test]
fn simple_cross() {
    if cross_compile_disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            &format!(
                r#"
                    fn main() {{
                        assert_eq!(std::env::var("TARGET").unwrap(), "{}");
                    }}
                "#,
                cross_compile::alternate()
            ),
        )
        .file(
            "src/main.rs",
            &format!(
                r#"
                    use std::env;
                    fn main() {{
                        assert_eq!(env::consts::ARCH, "{}");
                    }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let target = cross_compile::alternate();
    p.cargo("build -v --target").arg(&target).run();
    assert!(p.target_bin(target, "foo").is_file());

    if cross_compile_can_run_on_host() {
        p.process(&p.target_bin(target, "foo")).run();
    }
}

#[cargo_test]
fn simple_cross_config() {
    if cross_compile_disabled() {
        return;
    }

    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [build]
                    target = "{}"
                "#,
                cross_compile::alternate()
            ),
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            &format!(
                r#"
                    fn main() {{
                        assert_eq!(std::env::var("TARGET").unwrap(), "{}");
                    }}
                "#,
                cross_compile::alternate()
            ),
        )
        .file(
            "src/main.rs",
            &format!(
                r#"
                    use std::env;
                    fn main() {{
                        assert_eq!(env::consts::ARCH, "{}");
                    }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let target = cross_compile::alternate();
    p.cargo("build -v").run();
    assert!(p.target_bin(target, "foo").is_file());

    if cross_compile_can_run_on_host() {
        p.process(&p.target_bin(target, "foo")).run();
    }
}

#[cargo_test]
fn target_host_arg() {
    if cross_compile_disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            &format!(
                r#"
                    fn main() {{
                        assert_eq!(std::env::var("TARGET").unwrap(), "{}");
                    }}
                "#,
                rustc_host()
            ),
        )
        .file("src/lib.rs", r#""#)
        .build();

    p.cargo("build -v --target host-tuple")
        .with_stderr_contains("[RUNNING] `rustc [..] --target [HOST_TARGET] [..]`")
        .run();
}

#[cargo_test]
fn target_host_config() {
    if cross_compile_disabled() {
        return;
    }

    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [build]
                    target = "host-tuple"
                "#,
            ),
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            &format!(
                r#"
                    fn main() {{
                        assert_eq!(std::env::var("TARGET").unwrap(), "{}");
                    }}
                "#,
                rustc_host()
            ),
        )
        .file("src/lib.rs", r#""#)
        .build();

    p.cargo("build -v")
        .with_stderr_contains("[RUNNING] `rustc [..] --target [HOST_TARGET] [..]`")
        .run();
}

#[cargo_test]
fn simple_deps() {
    if cross_compile_disabled() {
        return;
    }

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
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() { bar::bar(); }")
        .build();
    let _p2 = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let target = cross_compile::alternate();
    p.cargo("build --target").arg(&target).run();
    assert!(p.target_bin(target, "foo").is_file());

    if cross_compile_can_run_on_host() {
        p.process(&p.target_bin(target, "foo")).run();
    }
}

/// Always take care of setting these so that
/// `cross_compile::alternate()` is the actually-picked target
fn per_crate_target_test(
    default_target: Option<&'static str>,
    forced_target: Option<&'static str>,
    arg_target: Option<&'static str>,
) {
    if cross_compile_disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    cargo-features = ["per-package-target"]

                    [package]
                    name = "foo"
                    version = "0.0.0"
                    edition = "2015"
                    authors = []
                    build = "build.rs"
                    {}
                    {}
                "#,
                default_target
                    .map(|t| format!(r#"default-target = "{}""#, t))
                    .unwrap_or(String::new()),
                forced_target
                    .map(|t| format!(r#"forced-target = "{}""#, t))
                    .unwrap_or(String::new()),
            ),
        )
        .file(
            "build.rs",
            &format!(
                r#"
                    fn main() {{
                        assert_eq!(std::env::var("TARGET").unwrap(), "{}");
                    }}
                "#,
                cross_compile::alternate()
            ),
        )
        .file(
            "src/main.rs",
            &format!(
                r#"
                    use std::env;
                    fn main() {{
                        assert_eq!(env::consts::ARCH, "{}");
                    }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let mut cmd = p.cargo("build -v");
    if let Some(t) = arg_target {
        cmd.arg("--target").arg(&t);
    }
    cmd.masquerade_as_nightly_cargo(&["per-package-target"])
        .run();
    assert!(p.target_bin(cross_compile::alternate(), "foo").is_file());

    if cross_compile_can_run_on_host() {
        p.process(&p.target_bin(cross_compile::alternate(), "foo"))
            .run();
    }
}

#[cargo_test]
fn per_crate_default_target_is_default() {
    per_crate_target_test(Some(cross_compile::alternate()), None, None);
}

#[cargo_test]
fn per_crate_default_target_gets_overridden() {
    per_crate_target_test(
        Some(cross_compile::unused()),
        None,
        Some(cross_compile::alternate()),
    );
}

#[cargo_test]
fn per_crate_forced_target_is_default() {
    per_crate_target_test(None, Some(cross_compile::alternate()), None);
}

#[cargo_test]
fn per_crate_forced_target_does_not_get_overridden() {
    per_crate_target_test(
        None,
        Some(cross_compile::alternate()),
        Some(cross_compile::unused()),
    );
}

#[cargo_test]
fn workspace_with_multiple_targets() {
    if cross_compile_disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["native", "cross"]
            "#,
        )
        .file(
            "native/Cargo.toml",
            r#"
                cargo-features = ["per-package-target"]

                [package]
                name = "native"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "native/build.rs",
            &format!(
                r#"
                    fn main() {{
                        assert_eq!(std::env::var("TARGET").unwrap(), "{}");
                    }}
                "#,
                cross_compile::native()
            ),
        )
        .file(
            "native/src/main.rs",
            &format!(
                r#"
                    use std::env;
                    fn main() {{
                        assert_eq!(env::consts::ARCH, "{}");
                    }}
                "#,
                cross_compile::native_arch()
            ),
        )
        .file(
            "cross/Cargo.toml",
            &format!(
                r#"
                    cargo-features = ["per-package-target"]

                    [package]
                    name = "cross"
                    version = "0.0.0"
                    edition = "2015"
                    authors = []
                    build = "build.rs"
                    default-target = "{}"
                "#,
                cross_compile::alternate(),
            ),
        )
        .file(
            "cross/build.rs",
            &format!(
                r#"
                    fn main() {{
                        assert_eq!(std::env::var("TARGET").unwrap(), "{}");
                    }}
                "#,
                cross_compile::alternate()
            ),
        )
        .file(
            "cross/src/main.rs",
            &format!(
                r#"
                    use std::env;
                    fn main() {{
                        assert_eq!(env::consts::ARCH, "{}");
                    }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let mut cmd = p.cargo("build -v");
    cmd.masquerade_as_nightly_cargo(&["per-package-target"])
        .run();

    assert!(p.bin("native").is_file());
    assert!(p.target_bin(cross_compile::alternate(), "cross").is_file());

    p.process(&p.bin("native")).run();
    if cross_compile_can_run_on_host() {
        p.process(&p.target_bin(cross_compile::alternate(), "cross"))
            .run();
    }
}

#[cargo_test]
fn linker() {
    if cross_compile_disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [target.{}]
                    linker = "my-linker-tool"
                "#,
                target
            ),
        )
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/foo.rs",
            &format!(
                r#"
                    use std::env;
                    fn main() {{
                        assert_eq!(env::consts::ARCH, "{}");
                    }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    p.cargo("build -v --target")
        .arg(&target)
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] path `src/foo.rs` was erroneously implicitly accepted for binary `foo`,
please set bin.path in Cargo.toml
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/foo.rs [..]--crate-type bin --emit=[..]link[..]-C debuginfo=2 [..] -C metadata=[..] --out-dir [ROOT]/foo/target/[ALT_TARGET]/debug/deps --target [ALT_TARGET] -C linker=my-linker-tool -L dependency=[ROOT]/foo/target/[ALT_TARGET]/debug/deps -L dependency=[ROOT]/foo/target/debug/deps`
[ERROR] linker `my-linker-tool` not found
...
"#]])
        .run();
}

#[cargo_test]
fn cross_tests() {
    if !cross_compile_can_run_on_host() {
        return;
    }

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
                name = "bar"
            "#,
        )
        .file(
            "src/bin/bar.rs",
            &format!(
                r#"
                    #[allow(unused_extern_crates)]
                    extern crate foo;
                    use std::env;
                    fn main() {{
                        assert_eq!(env::consts::ARCH, "{}");
                    }}
                    #[test] fn test() {{ main() }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .file(
            "src/lib.rs",
            &format!(
                r#"
                    //! ```
                    //! extern crate foo;
                    //! assert!(true);
                    //! ```

                    use std::env;
                    pub fn foo() {{ assert_eq!(env::consts::ARCH, "{}"); }}
                    #[test] fn test_foo() {{ foo() }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let target = cross_compile::alternate();
    p.cargo("test --target")
        .arg(&target)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/[ALT_TARGET]/debug/deps/foo-[HASH][EXE])
[RUNNING] unittests src/bin/bar.rs (target/[ALT_TARGET]/debug/deps/bar-[HASH][EXE])
[DOCTEST] foo

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test test_foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test src/lib.rs - (line 2) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test]
fn simple_cargo_run() {
    if !cross_compile_can_run_on_host() {
        return;
    }

    let p = project()
        .file(
            "src/main.rs",
            &format!(
                r#"
                    use std::env;
                    fn main() {{
                        assert_eq!(env::consts::ARCH, "{}");
                    }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let target = cross_compile::alternate();
    p.cargo("run --target").arg(&target).run();
}

#[cargo_test]
fn cross_with_a_build_script() {
    if cross_compile_disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let p = project()
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
            &format!(
                r#"
                    use std::env;
                    use std::path::PathBuf;
                    fn main() {{
                        assert_eq!(env::var("TARGET").unwrap(), "{0}");
                        let mut path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
                        assert_eq!(path.file_name().unwrap().to_str().unwrap(), "out");
                        path.pop();
                        assert!(path.file_name().unwrap().to_str().unwrap()
                                    .starts_with("foo-"));
                        path.pop();
                        assert_eq!(path.file_name().unwrap().to_str().unwrap(), "build");
                        path.pop();
                        assert_eq!(path.file_name().unwrap().to_str().unwrap(), "debug");
                        path.pop();
                        assert_eq!(path.file_name().unwrap().to_str().unwrap(), "{0}");
                        path.pop();
                        assert_eq!(path.file_name().unwrap().to_str().unwrap(), "target");
                    }}
                "#,
                target
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v --target")
        .arg(&target)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..] build.rs [..] --out-dir [ROOT]/foo/target/debug/build/foo-[HASH] [..]
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[RUNNING] `rustc [..] src/main.rs [..] --target [ALT_TARGET] [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_script_needed_for_host_and_target() {
    if cross_compile_disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = 'build.rs'

                [dependencies.d1]
                path = "d1"
                [build-dependencies.d2]
                path = "d2"
            "#,
        )
        .file(
            "build.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate d2;
                fn main() { d2::d2(); }
            "#,
        )
        .file(
            "src/main.rs",
            "
            #[allow(unused_extern_crates)]
            extern crate d1;
            fn main() { d1::d1(); }
        ",
        )
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = 'build.rs'
            "#,
        )
        .file("d1/src/lib.rs", "pub fn d1() {}")
        .file(
            "d1/build.rs",
            r#"
                use std::env;
                fn main() {
                    let target = env::var("TARGET").unwrap();
                    let root = std::env::current_dir().unwrap();
                    let root = root.parent().unwrap().join(format!("link-{target}"));
                    println!("cargo::rustc-flags=-L {}", root.display());
                }
            "#,
        )
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies.d1]
                path = "../d1"
            "#,
        )
        .file(
            "d2/src/lib.rs",
            "
            #[allow(unused_extern_crates)]
            extern crate d1;
            pub fn d2() { d1::d1(); }
        ",
        )
        .build();
    p.root().join(format!("link-{target}")).mkdir_p();
    p.root().join(format!("link-{}", rustc_host())).mkdir_p();

    p.cargo("build -v --target")
        .arg(&target)
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] d1 v0.0.0 ([ROOT]/foo/d1)
[RUNNING] `rustc [..] d1/build.rs [..] --out-dir [ROOT]/foo/target/debug/build/d1-[HASH] [..]
[RUNNING] `[ROOT]/foo/target/debug/build/d1-[HASH]/build-script-build`
[RUNNING] `[ROOT]/foo/target/debug/build/d1-[HASH]/build-script-build`
[RUNNING] `rustc [..] d1/src/lib.rs [..] --out-dir [ROOT]/foo/target/debug/deps [..]
[RUNNING] `rustc [..] d1/src/lib.rs [..] --out-dir [ROOT]/foo/target/[ALT_TARGET]/debug/deps [..]
[COMPILING] d2 v0.0.0 ([ROOT]/foo/d2)
[RUNNING] `rustc [..] d2/src/lib.rs [..] --out-dir [ROOT]/foo/target/debug/deps [..]-L [ROOT]/foo/link-[HOST_TARGET]`
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..] build.rs [..] --out-dir [ROOT]/foo/target/debug/build/foo-[HASH] [..]-L [ROOT]/foo/link-[HOST_TARGET]`
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[RUNNING] `rustc [..] src/main.rs [..] --out-dir [ROOT]/foo/target/[ALT_TARGET]/debug/deps --target [ALT_TARGET] [..]-L [ROOT]/foo/link-[ALT_TARGET]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered())
        .run();
}

#[cargo_test]
fn build_deps_for_the_right_arch() {
    if cross_compile_disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []

                [dependencies.d2]
                path = "d2"
            "#,
        )
        .file("src/main.rs", "extern crate d2; fn main() {}")
        .file("d1/Cargo.toml", &basic_manifest("d1", "0.0.0"))
        .file("d1/src/lib.rs", "pub fn d1() {}")
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = "build.rs"

                [build-dependencies.d1]
                path = "../d1"
            "#,
        )
        .file("d2/build.rs", "extern crate d1; fn main() {}")
        .file("d2/src/lib.rs", "")
        .build();

    let target = cross_compile::alternate();
    p.cargo("build -v --target").arg(&target).run();
}

#[cargo_test]
fn build_script_only_host() {
    if cross_compile_disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = "build.rs"

                [build-dependencies.d1]
                path = "d1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", "extern crate d1; fn main() {}")
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.0"
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("d1/src/lib.rs", "pub fn d1() {}")
        .file(
            "d1/build.rs",
            r#"
                use std::env;

                fn main() {
                    assert!(env::var("OUT_DIR").unwrap().replace("\\", "/")
                                               .contains("target/debug/build/d1-"),
                            "bad: {:?}", env::var("OUT_DIR"));
                }
            "#,
        )
        .build();

    let target = cross_compile::alternate();
    p.cargo("build -v --target").arg(&target).run();
}

#[cargo_test]
fn build_script_with_platform_specific_dependencies() {
    if cross_compile_disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                build = "build.rs"

                [build-dependencies.d1]
                path = "d1"
            "#,
        )
        .file(
            "build.rs",
            "
            #[allow(unused_extern_crates)]
            extern crate d1;
            fn main() {}
        ",
        )
        .file("src/lib.rs", "")
        .file(
            "d1/Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "d1"
                    version = "0.0.0"
                    edition = "2015"
                    authors = []

                    [target.{}.dependencies]
                    d2 = {{ path = "../d2" }}
                "#,
                host
            ),
        )
        .file(
            "d1/src/lib.rs",
            "#[allow(unused_extern_crates)] extern crate d2;",
        )
        .file("d2/Cargo.toml", &basic_manifest("d2", "0.0.0"))
        .file("d2/src/lib.rs", "")
        .build();

    p.cargo("build -v --target")
        .arg(&target)
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] d2 v0.0.0 ([ROOT]/foo/d2)
[RUNNING] `rustc [..] d2/src/lib.rs [..]`
[COMPILING] d1 v0.0.0 ([ROOT]/foo/d1)
[RUNNING] `rustc [..] d1/src/lib.rs [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] build.rs [..]`
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[RUNNING] `rustc [..] src/lib.rs [..] --target [ALT_TARGET] [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn platform_specific_dependencies_do_not_leak() {
    if cross_compile_disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                build = "build.rs"

                [dependencies.d1]
                path = "d1"

                [build-dependencies.d1]
                path = "d1"
            "#,
        )
        .file("build.rs", "extern crate d1; fn main() {}")
        .file("src/lib.rs", "")
        .file(
            "d1/Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "d1"
                    version = "0.0.0"
                    edition = "2015"
                    authors = []

                    [target.{}.dependencies]
                    d2 = {{ path = "../d2" }}
                "#,
                host
            ),
        )
        .file("d1/src/lib.rs", "extern crate d2;")
        .file("d1/Cargo.toml", &basic_manifest("d1", "0.0.0"))
        .file("d2/src/lib.rs", "")
        .build();

    p.cargo("build -v --target")
        .arg(&target)
        .with_status(101)
        .with_stderr_data(str![[r#"
...
error[E0463]: can't find crate for `d2`
...
"#]])
        .run();
}

#[cargo_test]
fn platform_specific_variables_reflected_in_build_scripts() {
    if cross_compile_disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []
                    build = "build.rs"

                    [target.{host}.dependencies]
                    d1 = {{ path = "d1" }}

                    [target.{target}.dependencies]
                    d2 = {{ path = "d2" }}
                "#,
                host = host,
                target = target
            ),
        )
        .file(
            "build.rs",
            &format!(
                r#"
                    use std::env;

                    fn main() {{
                        let platform = env::var("TARGET").unwrap();
                        let (expected, not_expected) = match &platform[..] {{
                            "{host}" => ("DEP_D1_VAL", "DEP_D2_VAL"),
                            "{target}" => ("DEP_D2_VAL", "DEP_D1_VAL"),
                            _ => panic!("unknown platform")
                        }};

                        env::var(expected).ok()
                            .expect(&format!("missing {{}}", expected));
                        env::var(not_expected).err()
                            .expect(&format!("found {{}}", not_expected));
                    }}
                "#,
                host = host,
                target = target
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.0"
                edition = "2015"
                authors = []
                links = "d1"
                build = "build.rs"
            "#,
        )
        .file(
            "d1/build.rs",
            r#"fn main() { println!("cargo::metadata=val=1") }"#,
        )
        .file("d1/src/lib.rs", "")
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.0"
                edition = "2015"
                authors = []
                links = "d2"
                build = "build.rs"
            "#,
        )
        .file(
            "d2/build.rs",
            r#"fn main() { println!("cargo::metadata=val=1") }"#,
        )
        .file("d2/src/lib.rs", "")
        .build();

    p.cargo("build -v").run();
    p.cargo("build -v --target").arg(&target).run();
}

#[cargo_test]
#[cfg_attr(
    target_os = "macos",
    ignore = "don't have a dylib cross target on macos"
)]
fn cross_test_dylib() {
    if cross_compile_disabled() {
        return;
    }

    let target = cross_compile::alternate();

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
                crate-type = ["dylib"]

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate bar as the_bar;

                pub fn bar() { the_bar::baz(); }

                #[test]
                fn foo() { bar(); }
            "#,
        )
        .file(
            "tests/test.rs",
            r#"
                extern crate foo as the_foo;

                #[test]
                fn foo() { the_foo::bar(); }
            "#,
        )
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
        .file(
            "bar/src/lib.rs",
            &format!(
                r#"
                     use std::env;
                     pub fn baz() {{
                        assert_eq!(env::consts::ARCH, "{}");
                    }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    p.cargo("test --target")
        .arg(&target)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/[ALT_TARGET]/debug/deps/foo-[HASH][EXE])
[RUNNING] tests/test.rs (target/[ALT_TARGET]/debug/deps/test-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


running 1 test
test foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}

#[cargo_test]
fn doctest_xcompile_linker() {
    if cross_compile_disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let p = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [target.{}]
                    linker = "my-linker-tool"
                "#,
                target
            ),
        )
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file(
            "src/lib.rs",
            r#"
                /// ```
                /// assert_eq!(1, 1);
                /// ```
                pub fn foo() {}
            "#,
        )
        .build();

    // Fails because `my-linker-tool` doesn't actually exist.
    p.cargo("test --doc -v --target")
        .arg(&target)
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..] --out-dir [ROOT]/foo/target/[ALT_TARGET]/debug/deps --target [ALT_TARGET] [..]
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[DOCTEST] foo
[RUNNING] `rustdoc [..] src/lib.rs [..]
[ERROR] doctest failed, to rerun pass `--doc`

"#]])
        .run();
}

#[cargo_test]
fn always_emit_warnings_as_warnings_when_learning_target_info() {
    if cross_compile_disabled() {
        return;
    }

    let target = "wasm32-unknown-unknown";
    if !cross_compile::requires_target_installed(target) {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v --target")
        .env("RUSTFLAGS", "-Awarnings")
        .arg(target)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]-Awarnings[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
