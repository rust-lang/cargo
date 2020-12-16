//! Tests for NatVis support.
//!
//! Currently, there is no way to test for the presence (or absence)
//! of a specific item in a JSON array, so these tests verify all of
//! the arguments in the corresponding `rustc` calls. That's fragile.
//! Ideally, we would be able to test for the `-Clink-args=...` args
//! without caring about any other args.

use cargo_test_support::{project, rustc_host};

const NATVIS_CONTENT: &str = r#"
<?xml version="1.0" encoding="utf-8"?>
<AutoVisualizer xmlns="http://schemas.microsoft.com/vstudio/debugger/natvis/2010">
</<AutoVisualizer>
"#;

fn is_natvis_supported() -> bool {
    rustc_host().ends_with("-msvc")
}

/// Tests a project that contains a single NatVis file.
/// The file is discovered by Cargo, since it is in the `/natvis` directory,
/// and does not need to be explicitly specified in the manifest.
#[cargo_test]
fn natvis_autodiscovery() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "natvis_autodiscovery"
        version = "0.0.1"
        edition = "2018"
    "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("natvis/types.natvis", NATVIS_CONTENT)
        .build();

    let mut execs = p.cargo("build --build-plan -Zunstable-options");
    execs.masquerade_as_nightly_cargo();

    if is_natvis_supported() {
        execs.with_json(
            r#"
            {
                "inputs": [
                    "[..]/foo/Cargo.toml"
                ],
                "invocations": [
                    {
                        "args": [
                            "--crate-name",
                            "natvis_autodiscovery",
                            "--edition=2018",
                            "src/main.rs",
                            "--error-format=json",
                            "--json=[..]",
                            "--crate-type",
                            "bin",
                            "--emit=[..]",
                            "-C",
                            "embed-bitcode=[..]",
                            "-C",
                            "debuginfo=[..]",
                            "-C",
                            "metadata=[..]",
                            "--out-dir",
                            "[..]",
                            "-Clink-arg=/natvis:[..]/foo/natvis/types.natvis",
                            "-L",
                            "dependency=[..]"
                        ],
                        "cwd": "[..]/cit/[..]/foo",
                        "deps": [],
                        "env": "{...}",
                        "kind": null,
                        "links": "{...}",
                        "outputs": "{...}",
                        "package_name": "natvis_autodiscovery",
                        "package_version": "0.0.1",
                        "program": "rustc",
                        "target_kind": ["bin"],
                        "compile_mode": "build"
                    }
                ]
            }
            "#,
        );
    }

    execs.run();
}

/// Tests a project that contains a single NatVis file, which is explicitly
/// specified in the manifest file. Because it is explicitly specified, it
/// does not have to be in the `/natvis` subdirectory.
#[cargo_test]
fn natvis_explicit() {
    if !is_natvis_supported() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "natvis_explicit"
        version = "0.0.1"
        edition = "2018"
        natvis-files = ["types.natvis"]
    "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("types.natvis", NATVIS_CONTENT)
        .build();

    let mut execs = p.cargo("build --build-plan -Zunstable-options");
    execs.masquerade_as_nightly_cargo();

    if is_natvis_supported() {
        execs.with_json(
            r#"
            {
                "inputs": [
                    "[..]/foo/Cargo.toml"
                ],
                "invocations": [
                    {
                        "args": [
                            "--crate-name",
                            "natvis_explicit",
                            "--edition=2018",
                            "src/main.rs",
                            "--error-format=json",
                            "--json=[..]",
                            "--crate-type",
                            "bin",
                            "--emit=[..]",
                            "-C",
                            "embed-bitcode=[..]",
                            "-C",
                            "debuginfo=[..]",
                            "-C",
                            "metadata=[..]",
                            "--out-dir",
                            "[..]",
                            "-Clink-arg=/natvis:[..]/foo/types.natvis",
                            "-L",
                            "dependency=[..]"
                        ],
                        "cwd": "[..]/cit/[..]/foo",
                        "deps": [],
                        "env": "{...}",
                        "kind": null,
                        "links": "{...}",
                        "outputs": "{...}",
                        "package_name": "natvis_explicit",
                        "package_version": "0.0.1",
                        "program": "rustc",
                        "target_kind": ["bin"],
                        "compile_mode": "build"
                    }
                ]
            }
            "#,
        );
    }

    execs.run();
}

/// Tests a project that has a file in the `/natvis` directory, but which has
/// been disabled in the manifest. This is analogous to specifying `autobenches = false`.
#[cargo_test]
fn natvis_disabled() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "natvis_disabled"
        version = "0.0.1"
        edition = "2018"
        natvis-files = []
    "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("natvis/types.natvis", NATVIS_CONTENT)
        .build();

    let mut execs = p.cargo("build --build-plan -Zunstable-options");
    execs.masquerade_as_nightly_cargo();
    if is_natvis_supported() {
        execs.with_json(
            r#"
            {
                "inputs": [
                    "[..]/foo/Cargo.toml"
                ],
                "invocations": [
                    {
                        "args": [
                            "--crate-name",
                            "natvis_disabled",
                            "--edition=2018",
                            "src/main.rs",
                            "--error-format=json",
                            "--json=[..]",
                            "--crate-type",
                            "bin",
                            "--emit=[..]",
                            "-C",
                            "embed-bitcode=[..]",
                            "-C",
                            "debuginfo=[..]",
                            "-C",
                            "metadata=[..]",
                            "--out-dir",
                            "[..]",
                            "-L",
                            "dependency=[..]"
                        ],
                        "cwd": "[..]/cit/[..]/foo",
                        "deps": [],
                        "env": "{...}",
                        "kind": null,
                        "links": "{...}",
                        "outputs": "{...}",
                        "package_name": "natvis_disabled",
                        "package_version": "0.0.1",
                        "program": "rustc",
                        "target_kind": ["bin"],
                        "compile_mode": "build"
                    }
                ]
            }
            "#,
        );
    }
    execs.run();
}
