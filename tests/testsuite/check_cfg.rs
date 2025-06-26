//! Tests for Cargo usage of rustc `--check-cfg`.

use crate::prelude::*;
use cargo_test_support::{basic_manifest, project, str};

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

#[cargo_test]
fn features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [features]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "f_a" "f_b"))
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn features_with_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

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

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "f_a" "f_b"))
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn features_with_opt_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

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

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "bar" "default" "f_a" "f_b"))
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn features_with_namespaced_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

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

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "f_a" "f_b"))
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn features_fingerprint() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [features]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/lib.rs", "#[cfg(feature = \"f_b\")] fn entry() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "f_a" "f_b"))
        .with_stderr_does_not_contain("[..]unexpected_cfgs[..]")
        .run();

    p.cargo("check -v")
        .with_stderr_does_not_contain("[..]rustc[..]")
        .run();

    // checking that re-ordering the features does not invalid the fingerprint
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [features]
            f_b = []
            f_a = []
        "#,
    );

    p.cargo("check -v")
        .with_stderr_does_not_contain("[..]rustc[..]")
        .run();

    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [features]
            f_a = []
        "#,
    );

    p.cargo("check -v")
        // we check that the fingerprint is indeed dirty
        // that is cause rustc to be called again with the new check-cfg args
        // and that we indeed found a new warning from the unexpected_cfgs lint
        .with_stderr_data(format!(
            "\
[DIRTY] foo v0.1.0 ([ROOT]/foo): the list of declared features changed
[CHECKING] foo v0.1.0 ([ROOT]/foo)
{running_rustc}
[WARNING] unexpected `cfg` condition value: `f_b`
...
",
            running_rustc = x!("rustc" => "cfg" of "feature" with "f_a")
        ))
        .run();
}

#[cargo_test]
fn well_known_names_values() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with))
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn features_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [features]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("test -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "f_a" "f_b"))
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn features_doctest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [features]
                default = ["f_a"]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/lib.rs", "#[allow(dead_code)] fn foo() {}")
        .build();

    p.cargo("test -v --doc")
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "default" "f_a" "f_b"))
        .with_stderr_contains(x!("rustdoc" => "cfg" of "feature" with "default" "f_a" "f_b"))
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test"))
        .with_stderr_contains(x!("rustdoc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn well_known_names_values_test() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("test -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with))
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn well_known_names_values_doctest() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "#[allow(dead_code)] fn foo() {}")
        .build();

    p.cargo("test -v --doc")
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with))
        .with_stderr_contains(x!("rustdoc" => "cfg" of "feature" with))
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test"))
        .with_stderr_contains(x!("rustdoc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn features_doc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [features]
                default = ["f_a"]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/lib.rs", "#[allow(dead_code)] fn foo() {}")
        .build();

    p.cargo("doc -v")
        .with_stderr_contains(x!("rustdoc" => "cfg" of "feature" with "default" "f_a" "f_b"))
        .with_stderr_contains(x!("rustdoc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn build_script_feedback() {
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
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo::rustc-check-cfg=cfg(foo)"); }"#,
        )
        .build();

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "foo"))
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn build_script_doc() {
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
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo::rustc-check-cfg=cfg(foo)"); }"#,
        )
        .build();

    p.cargo("doc -v")
        .with_stderr_does_not_contain("rustc [..] --check-cfg [..]")
        .with_stderr_data(format!(
            "\
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] build.rs [..]`
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
{running_rustdoc}
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html
",
            running_rustdoc = x!("rustdoc" => "cfg" of "foo")
        ))
        .run();
}

#[cargo_test]
fn build_script_override() {
    let target = cargo_test_support::rustc_host();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = []
                links = "a"
                build = "build.rs"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", "")
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [target.{}.a]
                    rustc-check-cfg = ["cfg(foo)"]
                "#,
                target
            ),
        )
        .build();

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "foo"))
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with))
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test"))
        .run();
}

#[cargo_test]
fn build_script_test() {
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
            "#,
        )
        .file(
            "build.rs",
            r#"fn main() {
                println!("cargo::rustc-check-cfg=cfg(foo)");
                println!("cargo::rustc-cfg=foo");
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

    p.cargo("test -v")
        .with_stderr_data(
            format!(
                "\
{running_rustc}
{running_rustdoc}
...
",
                running_rustc = x!("rustc" => "cfg" of "foo"),
                running_rustdoc = x!("rustdoc" => "cfg" of "foo")
            )
            .unordered(),
        )
        .with_stdout_data(
            str![[r#"
test test_foo ... ok
test test_bar ... ok
test [..] ... ok
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn config_simple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lints.rust]
                unexpected_cfgs = { level = "warn", check-cfg = ["cfg(has_foo)", "cfg(has_bar, values(\"yes\", \"no\"))"] }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "has_foo"))
        .with_stderr_contains(x!("rustc" => "cfg" of "has_bar" with "yes" "no"))
        .with_stderr_does_not_contain("[..]unused manifest key[..]")
        .run();
}

#[cargo_test]
fn config_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo/"]

                [workspace.lints.rust]
                unexpected_cfgs = { level = "warn", check-cfg = ["cfg(has_foo)"] }
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lints]
                workspace = true
            "#,
        )
        .file("foo/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_data(format!(
            "\
...
{running_rustc}
...
",
            running_rustc = x!("rustc" => "cfg" of "has_foo")
        ))
        .with_stderr_does_not_contain("unexpected_cfgs")
        .run();
}

#[cargo_test]
fn config_workspace_not_inherited() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo/"]

                [workspace.lints.rust]
                unexpected_cfgs = { level = "warn", check-cfg = ["cfg(has_foo)"] }
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
            "#,
        )
        .file("foo/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_does_not_contain(x!("rustc" => "cfg" of "has_foo"))
        .with_stderr_does_not_contain("unexpected_cfgs")
        .run();
}

#[cargo_test]
fn config_invalid_position() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lints.rust]
                use_bracket = { level = "warn", check-cfg = ["cfg(has_foo)"] }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[WARNING] unused manifest key: `lints.rust.use_bracket.check-cfg`
...
"#]])
        .with_stderr_does_not_contain(x!("rustc" => "cfg" of "has_foo"))
        .run();
}

#[cargo_test]
fn config_invalid_empty() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lints.rust]
                unexpected_cfgs = { }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] missing field `level`
...
"#]])
        .run();
}

#[cargo_test]
fn config_invalid_not_list() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lints.rust]
                unexpected_cfgs = { level = "warn", check-cfg = "cfg()" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `lints.rust.unexpected_cfgs.check-cfg` must be a list of string

"#]])
        .run();
}

#[cargo_test]
fn config_invalid_not_list_string() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lints.rust]
                unexpected_cfgs = { level = "warn", check-cfg = [12] }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `lints.rust.unexpected_cfgs.check-cfg` must be a list of string

"#]])
        .run();
}

#[cargo_test]
fn config_and_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [features]
                my_feature = []
                alloc = []

                [lints.rust]
                unexpected_cfgs = { level = "warn", check-cfg = ["cfg(has_foo)", "cfg(has_bar, values(\"yes\", \"no\"))"] }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "has_foo"))
        .with_stderr_contains(x!("rustc" => "cfg" of "has_bar" with "yes" "no"))
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "alloc" "my_feature"))
        .run();
}

#[cargo_test]
fn config_with_cargo_doc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lints.rust]
                unexpected_cfgs = { level = "warn", check-cfg = ["cfg(has_foo)"] }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("doc -v")
        .with_stderr_data(format!(
            "\
...
{running_rustdoc}
...
",
            running_rustdoc = x!("rustdoc" => "cfg" of "has_foo")
        ))
        .run();
}

#[cargo_test]
fn config_with_cargo_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lints.rust]
                unexpected_cfgs = { level = "warn", check-cfg = ["cfg(has_foo)"] }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("test -v")
        .with_stderr_data(format!(
            "\
...
{running_rustc}
...
",
            running_rustc = x!("rustc" => "cfg" of "has_foo")
        ))
        .run();
}

#[cargo_test]
fn config_and_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"
                build = "build.rs"

                [lints.rust]
                unexpected_cfgs = { level = "warn", check-cfg = ["cfg(bar)"] }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo::rustc-check-cfg=cfg(foo)"); }"#,
        )
        .build();

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "foo")) // from build.rs
        .with_stderr_contains(x!("rustc" => "cfg" of "bar")) // from config
        .run();
}

#[cargo_test]
fn config_features_and_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"
                build = "build.rs"

                [features]
                serde = []
                json = []

                [lints.rust]
                unexpected_cfgs = { level = "warn", check-cfg = ["cfg(bar)"] }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo::rustc-check-cfg=cfg(foo)"); }"#,
        )
        .build();

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "foo")) // from build.rs
        .with_stderr_contains(x!("rustc" => "cfg" of "bar")) // from config
        .with_stderr_contains(x!("rustc" => "cfg" of "feature" with "json" "serde")) // features
        .with_stderr_contains(x!("rustc" => "cfg" of "docsrs,test")) // Cargo well known
        .run();
}

#[cargo_test]
fn config_fingerprint() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lints.rust]
                unexpected_cfgs = { level = "warn", check-cfg = ["cfg(bar)"] }
            "#,
        )
        .file("src/lib.rs", "fn entry() {}")
        .build();

    p.cargo("check -v")
        .with_stderr_contains(x!("rustc" => "cfg" of "bar"))
        .run();

    p.cargo("check -v")
        .with_stderr_does_not_contain("[..]rustc[..]")
        .run();

    // checking that changing the `check-cfg` config does invalid the fingerprint
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [lints.rust]
            unexpected_cfgs = { level = "warn", check-cfg = ["cfg(bar)", "cfg(foo)"] }
        "#,
    );

    p.cargo("check -v")
        // we check that the fingerprint is indeed dirty
        .with_stderr_contains("[..][DIRTY][..]the profile configuration changed")
        // that cause rustc to be called again with the new check-cfg args
        .with_stderr_contains(x!("rustc" => "cfg" of "bar"))
        .with_stderr_contains(x!("rustc" => "cfg" of "foo"))
        .run();
}
