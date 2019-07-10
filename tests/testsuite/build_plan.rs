use crate::support::registry::Package;
use crate::support::{basic_bin_manifest, basic_manifest, main_file, project};

#[cargo_test]
fn cargo_build_plan_simple() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build --build-plan init -Zunstable-options")
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
                "target_kind": ["bin"],
                "compile_mode": "build"
            }
        ]
    }
    "#,
        )
        .run();
    assert!(!p.bin("foo").is_file());
}

#[cargo_test]
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
        )
        .file(
            "src/lib.rs",
            r#"
            extern crate bar;
            pub fn foo() { bar::bar(); }

            #[test]
            fn test() { foo(); }
        "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
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
                    "[..]/foo/target/debug/deps/libbar-[..].rlib",
                    "[..]/foo/target/debug/deps/libbar-[..].rmeta"
                ],
                "package_name": "bar",
                "package_version": "0.0.1",
                "program": "rustc",
                "target_kind": ["lib"],
                "compile_mode": "build"
            },
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [0],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "outputs": [
                    "[..]/foo/target/debug/deps/libfoo-[..].rlib",
                    "[..]/foo/target/debug/deps/libfoo-[..].rmeta"
                ],
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "rustc",
                "target_kind": ["lib"],
                "compile_mode": "build"
            }
        ]
    }
    "#,
        )
        .run();
}

#[cargo_test]
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
        )
        .file("src/main.rs", r#"fn main() {}"#)
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
                "target_kind": ["custom-build"],
                "compile_mode": "build"
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
                "target_kind": ["custom-build"],
                "compile_mode": "run-custom-build"
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
                "target_kind": ["bin"],
                "compile_mode": "build"
            }
        ]
    }
    "#,
        )
        .run();
}

// Check that (in contrast with `cargo_build_plan_build_script`) the custom
// build script invocations are gone (they've been compiled instead), as
// `foo`'s dependency on them also, and that their emitted information is passed
// to `foo`'s invocation: `-L bar` argument and `FOO=foo` environmental variable.
//
// FIXME: The JSON matching pattern in this test is very brittle, we just care
//  about the above *extra* items in the `"args"` and `"env"` arrays, but we need
//  to add *all* their elements for the pattern to match, there doesn't seem to
//  be a more flexible wildcard of the sort `{... "extra-item"}`.
#[cargo_test]
fn cargo_build_plan_post_build_script() {
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
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo:rustc-flags=-L bar");
                println!("cargo:rustc-env=FOO=foo");
            }
        "#,
        )
        .build();

    p.cargo("build --build-plan post-build-scripts -Z unstable-options")
        .masquerade_as_nightly_cargo()
        .with_json(
            r#"
    {
        "inputs": [
            "[..]/foo/Cargo.toml"
        ],
        "invocations": [
            {
                "args": [
                    "--crate-name",
                    "foo",
                    "src/main.rs",
                    "--color",
                    "never",
                    "--crate-type",
                    "bin",
                    "--emit=[..]",
                    "-C",
                    "debuginfo=[..]",
                    "-C",
                    "metadata=[..]",
                    "-C",
                    "extra-filename=[..]",
                    "--out-dir",
                    "[..]",
                    "-L",
                    "dependency=[..]",
                    "-L",
                    "bar"
                ],
                "cwd": "[..]/cit/[..]/foo",
                "deps": [],
                "env": {
                    "CARGO": "[..]",
                    "CARGO_MANIFEST_DIR": "[..]",
                    "CARGO_PKG_AUTHORS": "[..]",
                    "CARGO_PKG_DESCRIPTION": "",
                    "CARGO_PKG_HOMEPAGE": "",
                    "CARGO_PKG_NAME": "foo",
                    "CARGO_PKG_REPOSITORY": "",
                    "CARGO_PKG_VERSION": "0.5.0",
                    "CARGO_PKG_VERSION_MAJOR": "0",
                    "CARGO_PKG_VERSION_MINOR": "5",
                    "CARGO_PKG_VERSION_PATCH": "0",
                    "CARGO_PKG_VERSION_PRE": "",
                    "CARGO_PRIMARY_PACKAGE": "1",
                    "FOO": "foo",
                    "LD_LIBRARY_PATH": "[..]",
                    "OUT_DIR": "[..]"
                },
                "kind": "Host",
                "links": "{...}",
                "outputs": "{...}",
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "rustc",
                "target_kind": ["bin"],
                "compile_mode": "build"
            }
        ]
    }
    "#,
        )
        .run();
}

// Similar to `cargo_build_plan_single_dep`, this test adds `bar` *also* as a
// build dependency. The two `bar` dependencies should be processed separately
// while having the same metadata and fingerprint. Using the `post-build-scripts`
// option, `bar` should still be listed since we're only consuming the build
// dependency (the expected JSON is an exact copy of the other test).
//
// Note that the `bar` invocation is listed even though it *won't* be compiled
// (since it's already available from the compiled build dependency), the same
// way that in the default `init` option a dependency is listed even though it
// may be already fresh and not chosen for compilation.
#[cargo_test]
fn cargo_build_plan_post_build_script_with_build_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "build.rs"
            [dependencies]
            bar = { path = "bar" }
            [build-dependencies]
            bar = { path = "bar" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() { bar::bar(); }
            "#,
        )
        .file("build.rs", r#"fn main() {}"#)
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build --build-plan post-build-scripts -Z unstable-options")
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
                    "[..]/foo/target/debug/deps/libbar-[..].rlib",
                    "[..]/foo/target/debug/deps/libbar-[..].rmeta"
                ],
                "package_name": "bar",
                "package_version": "0.0.1",
                "program": "rustc",
                "target_kind": ["lib"],
                "compile_mode": "build"
            },
            {
                "args": "{...}",
                "cwd": "[..]/cit/[..]/foo",
                "deps": [0],
                "env": "{...}",
                "kind": "Host",
                "links": "{...}",
                "outputs": [
                    "[..]/foo/target/debug/deps/libfoo-[..].rlib",
                    "[..]/foo/target/debug/deps/libfoo-[..].rmeta"
                ],
                "package_name": "foo",
                "package_version": "0.5.0",
                "program": "rustc",
                "target_kind": ["lib"],
                "compile_mode": "build"
            }
        ]
    }
    "#,
        )
        .run();
}

#[cargo_test]
fn cargo_build_plan_with_dev_dep() {
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
