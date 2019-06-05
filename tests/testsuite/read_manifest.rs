use crate::support::{basic_bin_manifest, main_file, project};

static MANIFEST_OUTPUT: &'static str = r#"
{
    "authors": [
        "wycats@example.com"
    ],
    "categories": [],
    "name":"foo",
    "readme": null,
    "repository": null,
    "version":"0.5.0",
    "id":"foo[..]0.5.0[..](path+file://[..]/foo)",
    "keywords": [],
    "license": null,
    "license_file": null,
    "links": null,
    "description": null,
    "edition": "2015",
    "source":null,
    "dependencies":[],
    "targets":[{
        "kind":["bin"],
        "crate_types":["bin"],
        "doctest": false,
        "edition": "2015",
        "name":"foo",
        "src_path":"[..]/foo/src/foo.rs"
    }],
    "features":{},
    "manifest_path":"[..]Cargo.toml",
    "metadata": null
}"#;

#[cargo_test]
fn cargo_read_manifest_path_to_cargo_toml_relative() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest --manifest-path foo/Cargo.toml")
        .cwd(p.root().parent().unwrap())
        .with_json(MANIFEST_OUTPUT)
        .run();
}

#[cargo_test]
fn cargo_read_manifest_path_to_cargo_toml_absolute() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest --manifest-path")
        .arg(p.root().join("Cargo.toml"))
        .cwd(p.root().parent().unwrap())
        .with_json(MANIFEST_OUTPUT)
        .run();
}

#[cargo_test]
fn cargo_read_manifest_path_to_cargo_toml_parent_relative() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest --manifest-path foo")
        .cwd(p.root().parent().unwrap())
        .with_status(101)
        .with_stderr(
            "[ERROR] the manifest-path must be \
             a path to a Cargo.toml file",
        )
        .run();
}

#[cargo_test]
fn cargo_read_manifest_path_to_cargo_toml_parent_absolute() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest --manifest-path")
        .arg(p.root())
        .cwd(p.root().parent().unwrap())
        .with_status(101)
        .with_stderr(
            "[ERROR] the manifest-path must be \
             a path to a Cargo.toml file",
        )
        .run();
}

#[cargo_test]
fn cargo_read_manifest_cwd() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest").with_json(MANIFEST_OUTPUT).run();
}
