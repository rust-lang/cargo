extern crate cargotest;
extern crate hamcrest;
extern crate rustc_serialize;

use std::str;

use rustc_serialize::json;
use hamcrest::{assert_that, existing_file};
use cargotest::support::registry::Package;
use cargotest::support::{project, execs, basic_bin_manifest, basic_lib_manifest, main_file};

#[test]
fn cargo_metadata_simple() {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata"), execs().with_json(r#"
    {
        "packages": [
            {
                "name": "foo",
                "version": "0.5.0",
                "id": "foo[..]",
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "targets": [
                    {
                        "kind": [
                            "bin"
                        ],
                        "name": "foo",
                        "src_path": "src[..]foo.rs",
                        "metadata": null,
                        "filename": "foo"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml"
            }
        ],
        "workspace_members": ["foo 0.5.0 (path+file:[..]foo)"],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "id": "foo 0.5.0 (path+file:[..]foo)"
                }
            ],
            "root": "foo 0.5.0 (path+file:[..]foo)"
        },
        "version": 1
    }"#));
}


#[test]
fn cargo_metadata_with_deps_and_version() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            license = "MIT"
            description = "foo"

            [[bin]]
            name = "foo"

            [dependencies]
            bar = "*"
        "#);
    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1").dep("baz", "0.0.1").publish();

    assert_that(p.cargo_process("metadata")
                 .arg("-q")
                 .arg("--format-version").arg("1"),
                execs().with_json(r#"
    {
        "packages": [
            {
                "dependencies": [],
                "features": {},
                "id": "baz 0.0.1 (registry+[..])",
                "manifest_path": "[..]Cargo.toml",
                "name": "baz",
                "source": "registry+[..]",
                "license": null,
                "license_file": null,
                "targets": [
                    {
                        "kind": [
                            "lib"
                        ],
                        "name": "baz",
                        "src_path": "[..]lib.rs",
                        "metadata": "[..]",
                        "filename": "libbaz-[..].rlib"
                    }
                ],
                "version": "0.0.1"
            },
            {
                "dependencies": [
                    {
                        "features": [],
                        "kind": null,
                        "name": "baz",
                        "optional": false,
                        "req": "^0.0.1",
                        "source": "registry+[..]",
                        "target": null,
                        "uses_default_features": true
                    }
                ],
                "features": {},
                "id": "bar 0.0.1 (registry+[..])",
                "manifest_path": "[..]Cargo.toml",
                "name": "bar",
                "source": "registry+[..]",
                "license": null,
                "license_file": null,
                "targets": [
                    {
                        "kind": [
                            "lib"
                        ],
                        "name": "bar",
                        "src_path": "[..]lib.rs",
                        "metadata": "[..]",
                        "filename": "libbar-[..].rlib"
                    }
                ],
                "version": "0.0.1"
            },
            {
                "dependencies": [
                    {
                        "features": [],
                        "kind": null,
                        "name": "bar",
                        "optional": false,
                        "req": "*",
                        "source": "registry+[..]",
                        "target": null,
                        "uses_default_features": true
                    }
                ],
                "features": {},
                "id": "foo 0.5.0 (path+file:[..]foo)",
                "manifest_path": "[..]Cargo.toml",
                "name": "foo",
                "source": null,
                "license": "MIT",
                "license_file": null,
                "targets": [
                    {
                        "kind": [
                            "bin"
                        ],
                        "name": "foo",
                        "src_path": "[..]foo.rs",
                        "metadata": null,
                        "filename": "foo"
                    }
                ],
                "version": "0.5.0"
            }
        ],
        "workspace_members": ["foo 0.5.0 (path+file:[..]foo)"],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [
                        "bar 0.0.1 (registry+[..])"
                    ],
                    "id": "foo 0.5.0 (path+file:[..]foo)"
                },
                {
                    "dependencies": [
                        "baz 0.0.1 (registry+[..])"
                    ],
                    "id": "bar 0.0.1 (registry+[..])"
                },
                {
                    "dependencies": [],
                    "id": "baz 0.0.1 (registry+[..])"
                }
            ],
            "root": "foo 0.5.0 (path+file:[..]foo)"
        },
        "version": 1
    }"#));
}

#[test]
fn workspace_metadata() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["bar", "baz"]
        "#)
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file("baz/src/lib.rs", "");
    p.build();

    assert_that(p.cargo_process("metadata"), execs().with_status(0).with_json(r#"
    {
        "packages": [
            {
                "name": "bar",
                "version": "0.5.0",
                "id": "bar[..]",
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "name": "bar",
                        "src_path": "[..]bar[..]src[..]lib.rs",
                        "metadata": "[..]",
                        "filename": "libbar-[..].rlib"
                    }
                ],
                "features": {},
                "manifest_path": "[..]bar[..]Cargo.toml"
            },
            {
                "name": "baz",
                "version": "0.5.0",
                "id": "baz[..]",
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "name": "baz",
                        "src_path": "[..]baz[..]src[..]lib.rs",
                        "metadata": "[..]",
                        "filename": "libbaz-[..].rlib"
                    }
                ],
                "features": {},
                "manifest_path": "[..]baz[..]Cargo.toml"
            }
        ],
        "workspace_members": ["baz 0.5.0 (path+file:[..]baz)", "bar 0.5.0 (path+file:[..]bar)"],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "id": "baz 0.5.0 (path+file:[..]baz)"
                },
                {
                    "dependencies": [],
                    "id": "bar 0.5.0 (path+file:[..]bar)"
                }
            ],
            "root": null
        },
        "version": 1
    }"#))
}

#[test]
fn workspace_metadata_no_deps() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["bar", "baz"]
        "#)
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file("baz/src/lib.rs", "");
    p.build();

    assert_that(p.cargo_process("metadata").arg("--no-deps"), execs().with_status(0).with_json(r#"
    {
        "packages": [
            {
                "name": "bar",
                "version": "0.5.0",
                "id": "bar[..]",
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "name": "bar",
                        "src_path": "[..]bar[..]src[..]lib.rs",
                        "metadata": "[..]",
                        "filename": "libbar-[..].rlib"
                    }
                ],
                "features": {},
                "manifest_path": "[..]bar[..]Cargo.toml"
            },
            {
                "name": "baz",
                "version": "0.5.0",
                "id": "baz[..]",
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "name": "baz",
                        "src_path": "[..]baz[..]src[..]lib.rs",
                        "metadata": "[..]",
                        "filename": "libbaz-[..].rlib"
                    }
                ],
                "features": {},
                "manifest_path": "[..]baz[..]Cargo.toml"
            }
        ],
        "workspace_members": ["baz 0.5.0 (path+file:[..]baz)", "bar 0.5.0 (path+file:[..]bar)"],
        "resolve": null,
        "version": 1
    }"#))
}

#[test]
fn cargo_metadata_with_invalid_manifest() {
    let p = project("foo")
            .file("Cargo.toml", "");

    assert_that(p.cargo_process("metadata"), execs().with_status(101)
                                                    .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  no `package` or `project` section found."))
}

const MANIFEST_OUTPUT: &'static str=
    r#"
{
    "packages": [{
        "name":"foo",
        "version":"0.5.0",
        "id":"foo[..]0.5.0[..](path+file://[..]/foo)",
        "source":null,
        "dependencies":[],
        "license": null,
        "license_file": null,
        "targets":[{
            "kind":["bin"],
            "name":"foo",
            "src_path":"src[..]foo.rs",
            "metadata": null,
            "filename": "foo"
        }],
        "features":{},
        "manifest_path":"[..]Cargo.toml"
    }],
    "workspace_members": [ "foo 0.5.0 (path+file:[..]foo)" ],
    "resolve": null,
    "version": 1
}"#;

#[test]
fn cargo_metadata_no_deps_path_to_cargo_toml_relative() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

        assert_that(p.cargo_process("metadata").arg("--no-deps")
                     .arg("--manifest-path").arg("foo/Cargo.toml")
                     .cwd(p.root().parent().unwrap()),
                    execs().with_status(0)
                           .with_json(MANIFEST_OUTPUT));
}

#[test]
fn cargo_metadata_no_deps_path_to_cargo_toml_absolute() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("metadata").arg("--no-deps")
                 .arg("--manifest-path").arg(p.root().join("Cargo.toml"))
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(0)
                       .with_json(MANIFEST_OUTPUT));
}

#[test]
fn cargo_metadata_no_deps_path_to_cargo_toml_parent_relative() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("metadata").arg("--no-deps")
                 .arg("--manifest-path").arg("foo")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(101)
                       .with_stderr("[ERROR] the manifest-path must be \
                                             a path to a Cargo.toml file"));
}

#[test]
fn cargo_metadata_no_deps_path_to_cargo_toml_parent_absolute() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("metadata").arg("--no-deps")
                 .arg("--manifest-path").arg(p.root())
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(101)
                       .with_stderr("[ERROR] the manifest-path must be \
                                             a path to a Cargo.toml file"));
}

#[test]
fn cargo_metadata_no_deps_cwd() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("metadata").arg("--no-deps")
                 .cwd(p.root()),
                execs().with_status(0)
                       .with_json(MANIFEST_OUTPUT));
}

#[test]
fn carg_metadata_bad_version() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("metadata").arg("--no-deps")
                 .arg("--format-version").arg("2")
                 .cwd(p.root()),
                execs().with_status(101)
    .with_stderr("[ERROR] metadata version 2 not supported, only 1 is currently supported"));
}

#[test]
fn cargo_metadata_filename_bin_test_dash() {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo-bar"))
            .file("tests/foo-test.rs", "#[test]fn isok() {}");

    assert_that(p.cargo_process("metadata"), execs().with_json(r#"
    {
        "packages": [
            {
                "name": "foo-bar",
                "version": "0.5.0",
                "id": "foo-bar[..]",
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "targets": [
                    {
                        "kind": [
                            "bin"
                        ],
                        "name": "foo-bar",
                        "src_path": "src/foo-bar.rs",
                        "metadata": null,
                        "filename": "foo_bar"
                    },
                    {
                        "filename": "foo_test-[..]",
                        "kind": [
                            "test"
                        ],
                        "metadata": "[..]",
                        "name": "foo-test",
                        "src_path": "[..]/foo/tests/foo-test.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml"
            }
        ],
        "workspace_members": [
            "foo-bar 0.5.0 (path+file:[..]foo)"
        ],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "id": "foo-bar 0.5.0 (path+file:[..]foo)"
                }
            ],
            "root": "foo-bar 0.5.0 (path+file:[..]foo)"
        },
        "version": 1
    }"#));
}

#[test]
fn cargo_metadata_filename_bin_test_underbar() {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo_underbar"))
            .file("tests/foo_underbartest.rs", "#[test]fn isok() {}");

    assert_that(p.cargo_process("metadata"), execs().with_json(r#"
    {
        "packages": [
            {
                "name": "foo_underbar",
                "version": "0.5.0",
                "id": "foo_underbar[..]",
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "targets": [
                    {
                        "kind": [
                            "bin"
                        ],
                        "name": "foo_underbar",
                        "src_path": "src[..]foo_underbar.rs",
                        "metadata": null,
                        "filename": "foo_underbar"
                    },
                    {
                        "filename": "foo_underbartest-[..]",
                        "kind": [
                            "test"
                        ],
                        "metadata": "[..]",
                        "name": "foo_underbartest",
                        "src_path": "[..]/foo/tests/foo_underbartest.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml"
            }
        ],
        "workspace_members": [
            "foo_underbar 0.5.0 (path+file:[..]foo)"
        ],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "id": "foo_underbar 0.5.0 (path+file:[..]foo)"
                }
            ],
            "root": "foo_underbar 0.5.0 (path+file:[..]foo)"
        },
        "version": 1
    }"#));
}

#[test]
fn cargo_metadata_filename_lib_dash() {
    let p = project("foo")
            .file("Cargo.toml", r#"
                [package]
                name = "foo-bar"
                version = "0.5.0"
                authors = ["wycats@example.com"]
            "#)
            .file("src/lib.rs", "pub fn foo() {}");

    assert_that(p.cargo_process("metadata"), execs().with_json(r#"
    {
        "packages": [
            {
                "name": "foo-bar",
                "version": "0.5.0",
                "id": "foo-bar[..]",
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "targets": [
                    {
                        "kind": [
                            "lib"
                        ],
                        "name": "foo-bar",
                        "src_path": "[..]src/lib.rs",
                        "metadata": "[..]",
                        "filename": "libfoo_bar-[..].rlib"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml"
            }
        ],
        "workspace_members": [
            "foo-bar 0.5.0 (path+file:[..]foo)"
        ],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "id": "foo-bar 0.5.0 (path+file:[..]foo)"
                }
            ],
            "root": "foo-bar 0.5.0 (path+file:[..]foo)"
        },
        "version": 1
    }"#));
}

#[test]
fn cargo_metadata_filename_lib_underbar() {
    let p = project("foo")
            .file("Cargo.toml", &basic_lib_manifest("foo_underbar"));

    assert_that(p.cargo_process("metadata"), execs().with_json(r#"
    {
        "packages": [
            {
                "name": "foo_underbar",
                "version": "0.5.0",
                "id": "foo_underbar[..]",
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "targets": [
                    {
                        "kind": [
                            "lib"
                        ],
                        "name": "foo_underbar",
                        "src_path": "src[..]foo_underbar.rs",
                        "metadata": "[..]",
                        "filename": "libfoo_underbar-[..].rlib"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml"
            }
        ],
        "workspace_members": [
            "foo_underbar 0.5.0 (path+file:[..]foo)"
        ],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "id": "foo_underbar 0.5.0 (path+file:[..]foo)"
                }
            ],
            "root": "foo_underbar 0.5.0 (path+file:[..]foo)"
        },
        "version": 1
    }"#));
}

#[test]
fn cargo_metadata_filename_lib_pathdep_namematch() {
    // Construct a binary which depends on a library, so we can check the library
    // is built with the expected extra_filename
    let p = project("foo")
            .file("Cargo.toml", r#"
                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                foo-bar = { path = "foo-bar" }
            "#)
            .file("src/main.rs", &main_file(r#""calling dep""#, &["foo_bar"]))
            .file("foo-bar/Cargo.toml", r#"
                [project]
                name = "foo-bar"
                version = "0.1.0"
            "#)
            .file("foo-bar/src/lib.rs", "pub fn foo() {}");

    // get metadata
    let out = p.cargo_process("metadata").exec_with_output();
    let metadata: &str = match &out {
        &Ok(ref out) => str::from_utf8(&out.stdout).expect("bad output"),
        &Err(ref err) => panic!("cargo metadata failed {:?}", err),
    };

    // build things
    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());

    // Check the build matched the metadata
    #[derive(RustcDecodable)]
    struct DecodableTarget {
        name: String,
        filename: String,
    }

    #[derive(RustcDecodable)]
    struct DecodablePackage {
        name: String,
        targets: Vec<DecodableTarget>,
    }

    #[derive(RustcDecodable)]
    struct DecodableMetadata {
        packages: Vec<DecodablePackage>,
    }

    let metadata: DecodableMetadata = json::decode(&metadata).expect("can't decode metadata json");
    let pkg = metadata.packages.iter().find(|p| p.name == "foo-bar").expect("package not found");
    let target = pkg.targets.iter().find(|t| t.name == "foo-bar").expect("target not found");
    let dir = p.deps();
    let path = dir.join(&target.filename);

    println!("Dir {:?} path {:?}", p.debug_dir(), path);
    for f in p.debug_dir().read_dir().expect("read_dir") {
        let f = f.expect("dirent");
        println!("Found {:?}", f.path())
    }

    println!("Dir {:?} path {:?}", dir, path);
    for f in dir.read_dir().expect("read_dir") {
        let f = f.expect("dirent");
        println!("Found {:?}", f.path())
    }
    assert_that(&path, existing_file());
}

#[test]
fn cargo_metadata_filename_lib_registry_namematch() {
    // Construct a binary which depends on a library, so we can check the library
    // is built with the expected extra_filename
    let p = project("foo")
            .file("Cargo.toml", r#"
                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                foo-bar = { path = "foo-bar" }
                far = "*"
            "#)
            .file("src/main.rs", &main_file(r#""calling dep""#, &["foo_bar"]))
            .file("foo-bar/Cargo.toml", r#"
                [project]
                name = "foo-bar"
                version = "0.1.0"
            "#)
            .file("foo-bar/src/lib.rs", "pub fn foo() {}");

    Package::new("far", "0.0.1").publish();

    // get metadata
    let out = p.cargo_process("metadata").exec_with_output();
    let metadata: &str = match &out {
        &Ok(ref out) => str::from_utf8(&out.stdout).expect("bad output"),
        &Err(ref err) => panic!("cargo metadata failed {:?}", err),
    };

    // build things
    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());

    // Check the build matched the metadata
    #[derive(RustcDecodable)]
    struct DecodableTarget {
        name: String,
        filename: String,
    }

    #[derive(RustcDecodable)]
    struct DecodablePackage {
        name: String,
        targets: Vec<DecodableTarget>,
    }

    #[derive(RustcDecodable)]
    struct DecodableMetadata {
        packages: Vec<DecodablePackage>,
    }

    let metadata: DecodableMetadata = json::decode(&metadata).expect("can't decode metadata json");
    let pkg = metadata.packages.iter().find(|p| p.name == "far").expect("package not found");
    let target = pkg.targets.iter().find(|t| t.name == "far").expect("target not found");
    let dir = p.deps();
    let path = dir.join(&target.filename);

    println!("Dir {:?} path {:?}", p.debug_dir(), path);
    for f in p.debug_dir().read_dir().expect("read_dir") {
        let f = f.expect("dirent");
        println!("Found {:?}", f.path())
    }

    println!("Dir {:?} path {:?}", dir, path);
    for f in dir.read_dir().expect("read_dir") {
        let f = f.expect("dirent");
        println!("Found {:?}", f.path())
    }
    assert_that(&path, existing_file());
}
