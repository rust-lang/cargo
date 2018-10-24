use support::registry::Package;
use support::{basic_bin_manifest, basic_manifest, main_file, project};

#[test]
fn cargo_build_plan_simple() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build --build-plan -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_json(
            r#"
    {
        "inputs": [
            "[..]/foo/Cargo.toml"
        ],
        "invocations": [
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "outputs": "{...}",
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "rustc",
                "target_kind": ["bin"]
            }
        ]
    }
    "#,
        ).run();
    assert!(!p.bin("foo").is_file());
}

#[test]
fn cargo_build_plan_single_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.5.0"

            [dependencies]
            bar = { path = "bar" }
        "#,
        ).file(
            "src/lib.rs",
            r#"
            extern crate bar;
            pub fn foo() { bar::bar(); }

            #[test]
            fn test() { foo(); }
        "#,
        ).file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();
    p.cargo("build --build-plan -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_json(
            r#"
    {
        "inputs": [
            "[..]/foo/Cargo.toml",
            "[..]/foo/bar/Cargo.toml"
        ],
        "invocations": [
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "outputs": [
                    "[..]/foo/target/debug/deps/libbar-[..].rlib"
                ],
                "package_name": "bar",
                "package_version": "0.0.1",
                "program": "rustc",
                "target_kind": ["lib"]
            },
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [0],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "outputs": [
                    "[..]/foo/target/debug/deps/libfoo-[..].rlib"
                ],
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "rustc",
                "target_kind": ["lib"]
            }
        ]
    }
    "#,
        ).run();
}

#[test]
fn cargo_build_plan_build_script() {
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
        ).file("src/main.rs", r#"fn main() {}"#)
        .file("build.rs", r#"fn main() {}"#)
        .build();

    p.cargo("build --build-plan -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_json(
            r#"
    {
        "inputs": [
            "[..]/foo/Cargo.toml"
        ],
        "invocations": [
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "outputs": [
                    "[..]/foo/target/debug/build/[..]/build_script_build-[..]"
                ],
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "rustc",
                "target_kind": ["custom-build"]
            },
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [0],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "outputs": [],
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "[..]/build-script-build",
                "target_kind": ["custom-build"]
            },
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [1],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "outputs": "{...}",
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "rustc",
                "target_kind": ["bin"]
            }
        ]
    }
    "#,
        ).run();
}

#[test]
fn build_plan_with_dev_dep() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dev-dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --build-plan -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .run();
}

#[test]
fn build_plan_detailed_with_inputs() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("data/some.txt", "o hai there")
        .file("src/foo.rs", "mod module1; mod module2; fn main() {}")
        .file("src/module1/mod.rs", "mod nested;")
        .file("src/module1/nested.rs", "")
        .file("src/module2/mod.rs", r#"
            const DATA: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/some.txt"));"#)
        .file("src/not_included_in_inputs.rs", "")
        .build();

    p.cargo("build --build-plan=detailed -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_json(
            r#"
    {
        "inputs": [
            "[..]/foo/Cargo.toml"
        ],
        "invocations": [
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "inputs": [
                    "[..]/foo/data/some.txt",
                    "[..]/foo/src/foo.rs",
                    "[..]/foo/src/module1/mod.rs",
                    "[..]/foo/src/module1/nested.rs",
                    "[..]/foo/src/module2/mod.rs"
                ],
                "outputs": "{...}",
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "rustc",
                "target_kind": ["bin"]
            }
        ]
    }
    "#,
        ).run();
}

#[test]
fn build_plan_detailed_build_script() {
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
        .file("src/main.rs", r#"fn main() {}"#)
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rerun-if-changed=build_script_rerun_dep.txt");
            }
        "#)
        .file("build_script_rerun_dep.txt", "")
        .build();

    p.cargo("build --build-plan=detailed -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_json(
            r#"
    {
        "inputs": [
            "[..]/foo/Cargo.toml"
        ],
        "invocations": [
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "inputs": [
                    "[..]/foo/build.rs"
                ],
                "outputs": [
                    "[..]/foo/target/debug/build/[..]/build_script_build-[..]"
                ],
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "rustc",
                "target_kind": ["custom-build"]
            },
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [0],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "inputs": [
                    "[..]/foo/build_script_rerun_dep.txt"
                ],
                "outputs": [],
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "[..]/build-script-build",
                "target_kind": ["custom-build"]
            },
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [1],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "inputs": [
                    "[..]/foo/src/main.rs"
                ],
                "outputs": "{...}",
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "rustc",
                "target_kind": ["bin"]
            }
        ]
    }
    "#,
        ).run();
}

#[test]
fn cargo_build_plan_detailed_path_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.5.0"

            [dependencies]
            bar = { path = "bar" }
        "#,
        ).file(
            "src/lib.rs",
            r#"
            extern crate bar;
            pub fn foo() { bar::bar(); }

            #[test]
            fn test() { foo(); }
        "#,
        ).file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();
    p.cargo("build --build-plan=detailed -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_json(
            r#"
    {
        "inputs": [
            "[..]/foo/Cargo.toml",
            "[..]/foo/bar/Cargo.toml"
        ],
        "invocations": [
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "inputs": [
                    "[..]/foo/bar/src/lib.rs"
                ],
                "outputs": [
                    "[..]/foo/target/debug/deps/libbar-[..].rlib"
                ],
                "package_name": "bar",
                "package_version": "0.0.1",
                "program": "rustc",
                "target_kind": ["lib"]
            },
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [0],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "inputs": [
                    "[..]/foo/src/lib.rs"
                ],
                "outputs": [
                    "[..]/foo/target/debug/deps/libfoo-[..].rlib"
                ],
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "rustc",
                "target_kind": ["lib"]
            }
        ]
    }
    "#,
        ).run();
}
