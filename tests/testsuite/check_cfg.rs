//! Tests for -Zcheck-cfg.

use cargo_test_support::{basic_manifest, project};

macro_rules! x {
    ($tool:tt => $what:tt $(of $who:tt)?) => {{
        #[cfg(windows)]
        {
            concat!("[RUNNING] [..]", $tool, "[..] --check-cfg ",
                    $what, '(', $($who,)* ')', "[..]")
        }
        #[cfg(not(windows))]
        {
            concat!("[RUNNING] [..]", $tool, "[..] --check-cfg '",
                    $what, '(', $($who,)* ')', "'", "[..]")
        }
    }};
    ($tool:tt => $what:tt of $who:tt with $($first_value:tt $($other_values:tt)*)?) => {{
        #[cfg(windows)]
        {
            concat!("[RUNNING] [..]", $tool, "[..] --check-cfg \"",
                    $what, '(', $who, ", values(", $("/\"", $first_value, "/\"", $(", ", "/\"", $other_values, "/\"",)*)* "))", '"', "[..]")
        }
        #[cfg(not(windows))]
        {
            concat!("[RUNNING] [..]", $tool, "[..] --check-cfg '",
                    $what, '(', $who, ", values(", $("\"", $first_value, "\"", $(", ", "\"", $other_values, "\"",)*)* "))", "'", "[..]")
        }
    }};
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [features]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v -Zcheck-cfg")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "f_a" "f_b"))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn features_with_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { path = "bar/" }

                [features]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[allow(dead_code)] fn bar() {}")
        .build();

    p.cargo("check -v -Zcheck-cfg")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "f_a" "f_b"))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn features_with_opt_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { path = "bar/", optional = true }

                [features]
                default = ["bar"]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[allow(dead_code)] fn bar() {}")
        .build();

    p.cargo("check -v -Zcheck-cfg")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "bar" "default" "f_a" "f_b"))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn features_with_namespaced_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { path = "bar/", optional = true }

                [features]
                f_a = ["dep:bar"]
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "#[allow(dead_code)] fn bar() {}")
        .build();

    p.cargo("check -v -Zcheck-cfg")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "f_a" "f_b"))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn well_known_names_values() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v -Zcheck-cfg")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn features_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [features]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("test -v -Zcheck-cfg")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "f_a" "f_b"))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn features_doctest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [features]
                default = ["f_a"]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/lib.rs", "#[allow(dead_code)] fn foo() {}")
        .build();

    p.cargo("test -v --doc -Zcheck-cfg")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "default" "f_a" "f_b"))
        .with_stderr_contains(x!("rustdoc" => "cfg" of "feature" with "default" "f_a" "f_b"))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn well_known_names_values_test() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("test -v -Zcheck-cfg")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn well_known_names_values_doctest() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "#[allow(dead_code)] fn foo() {}")
        .build();

    p.cargo("test -v --doc -Zcheck-cfg")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with))
        .with_stderr_contains(x!("rustdoc" => "cfg" of "feature" with))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn features_doc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [features]
                default = ["f_a"]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/lib.rs", "#[allow(dead_code)] fn foo() {}")
        .build();

    p.cargo("doc -v -Zcheck-cfg")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustdoc" => "cfg" of "feature" with "default" "f_a" "f_b"))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn build_script_feedback() {
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
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo:rustc-check-cfg=cfg(foo)"); }"#,
        )
        .build();

    p.cargo("check -v -Zcheck-cfg")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustc" => "cfg" of "foo"))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn build_script_doc() {
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
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo:rustc-check-cfg=cfg(foo)"); }"#,
        )
        .build();

    p.cargo("doc -v -Zcheck-cfg")
        .with_stderr_does_not_contain("rustc [..] --check-cfg [..]")
        .with_stderr_contains(x!("rustdoc" => "cfg" of "foo"))
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..] build.rs [..]`
[RUNNING] `[..]/build-script-build`
[DOCUMENTING] foo [..]
[RUNNING] `rustdoc [..] src/main.rs [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn build_script_override() {
    let target = cargo_test_support::rustc_host();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                authors = []
                links = "a"
                build = "build.rs"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", "")
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{}.a]
                    rustc-check-cfg = ["cfg(foo)"]
                "#,
                target
            ),
        )
        .build();

    p.cargo("check -v -Zcheck-cfg")
        .with_stderr_contains(x!("rustc" => "cfg" of "foo"))
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn build_script_test() {
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
            r#"fn main() { 
                println!("cargo:rustc-check-cfg=cfg(foo)");
                println!("cargo:rustc-cfg=foo");
            }"#,
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

    p.cargo("test -v -Zcheck-cfg")
        .with_stderr_contains(x!("rustc" => "cfg" of "foo"))
        .with_stderr_contains(x!("rustdoc" => "cfg" of "foo"))
        .with_stdout_contains("test test_foo ... ok")
        .with_stdout_contains("test test_bar ... ok")
        .with_stdout_contains_n("test [..] ... ok", 3)
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn config_valid() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [features]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
                [unstable]
                check-cfg = true
            "#,
        )
        .build();

    p.cargo("check -v")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "f_a" "f_b"))
        .run();
}

#[cargo_test(nightly, reason = "--check-cfg is unstable")]
fn config_invalid() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
                [unstable]
                check-cfg = ["va"]
            "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["check-cfg"])
        .with_stderr_contains("error:[..]`unstable.check-cfg` expected true/false[..]")
        .with_status(101)
        .run();
}
