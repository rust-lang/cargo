//! Tests for -Zcheck-cfg.

use cargo_test_support::{basic_manifest, is_nightly, project};

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn features() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [features]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v -Zcheck-cfg=features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'values(feature, \"f_a\", \"f_b\")' [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn features_with_deps() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
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

    p.cargo("build -v -Zcheck-cfg=features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] bar v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'values(feature)' [..]
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc --crate-name foo [..] --check-cfg 'values(feature, \"f_a\", \"f_b\")' [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn features_with_opt_deps() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
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

    p.cargo("build -v -Zcheck-cfg=features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] bar v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'values(feature)' [..]
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc --crate-name foo [..] --check-cfg 'values(feature, \"bar\", \"default\", \"f_a\", \"f_b\")' [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn features_with_namespaced_features() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
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

    p.cargo("build -v -Zcheck-cfg=features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc --crate-name foo [..] --check-cfg 'values(feature, \"f_a\", \"f_b\")' [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn well_known_names() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v -Zcheck-cfg=names")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'names()' [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn well_known_values() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v -Zcheck-cfg=values")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'values()' [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn cli_all_options() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [features]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v -Zcheck-cfg=features,names,values")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn features_with_cargo_check() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [features]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v -Zcheck-cfg=features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'values(feature, \"f_a\", \"f_b\")' [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn well_known_names_with_check() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v -Zcheck-cfg=names")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'names()' [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn well_known_values_with_check() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v -Zcheck-cfg=values")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'values()' [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn features_test() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [features]
                f_a = []
                f_b = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("test -v -Zcheck-cfg=features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'values(feature, \"f_a\", \"f_b\")' [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn features_doctest() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc and rustdoc command line
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
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

    p.cargo("test -v --doc -Zcheck-cfg=features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'values(feature, \"default\", \"f_a\", \"f_b\")' [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[DOCTEST] foo
[RUNNING] `rustdoc [..] --check-cfg 'values(feature, \"default\", \"f_a\", \"f_b\")' [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn well_known_names_test() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("test -v -Zcheck-cfg=names")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'names()' [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn well_known_values_test() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc command line
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("test -v -Zcheck-cfg=values")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'values()' [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn well_known_names_doctest() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc and rustdoc command line
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "#[allow(dead_code)] fn foo() {}")
        .build();

    p.cargo("test -v --doc -Zcheck-cfg=names")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'names()' [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[DOCTEST] foo
[RUNNING] `rustdoc [..] --check-cfg 'names()' [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn well_known_values_doctest() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustc and rustdoc command line
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "#[allow(dead_code)] fn foo() {}")
        .build();

    p.cargo("test -v --doc -Zcheck-cfg=values")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc [..] --check-cfg 'values()' [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[DOCTEST] foo
[RUNNING] `rustdoc [..] --check-cfg 'values()' [..]
",
        )
        .run();
}

#[cfg_attr(windows, ignore)] // weird normalization issue with windows and cargo-test-support
#[cargo_test]
fn features_doc() {
    if !is_nightly() {
        // --check-cfg is a nightly only rustdoc command line
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
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

    p.cargo("doc -v -Zcheck-cfg=features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[DOCUMENTING] foo v0.1.0 [..]
[RUNNING] `rustdoc [..] --check-cfg 'values(feature, \"default\", \"f_a\", \"f_b\")' [..]
[FINISHED] [..]
",
        )
        .run();
}
