use hamcrest::assert_that;
use support::registry::Package;
use support::{project, execs, basic_bin_manifest, main_file};


fn setup() {}

test!(cargo_metadata_simple {
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
                "targets": [
                    {
                        "kind": [
                            "bin"
                        ],
                        "name": "foo",
                        "src_path": "src[..]foo.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml"
            }
        ],
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
});


test!(cargo_metadata_with_deps_and_version {
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
                "id": "baz 0.0.1 (registry+file:[..])",
                "manifest_path": "[..]Cargo.toml",
                "name": "baz",
                "source": "registry+file:[..]",
                "targets": [
                    {
                        "kind": [
                            "lib"
                        ],
                        "name": "baz",
                        "src_path": "[..]lib.rs"
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
                        "source": "registry+file:[..]",
                        "target": null,
                        "uses_default_features": true
                    }
                ],
                "features": {},
                "id": "bar 0.0.1 (registry+file:[..])",
                "manifest_path": "[..]Cargo.toml",
                "name": "bar",
                "source": "registry+file:[..]",
                "targets": [
                    {
                        "kind": [
                            "lib"
                        ],
                        "name": "bar",
                        "src_path": "[..]lib.rs"
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
                        "source": "registry+file:[..]",
                        "target": null,
                        "uses_default_features": true
                    }
                ],
                "features": {},
                "id": "foo 0.5.0 (path+file:[..]foo)",
                "manifest_path": "[..]Cargo.toml",
                "name": "foo",
                "source": null,
                "targets": [
                    {
                        "kind": [
                            "bin"
                        ],
                        "name": "foo",
                        "src_path": "[..]foo.rs"
                    }
                ],
                "version": "0.5.0"
            }
        ],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [
                        "bar 0.0.1 (registry+file:[..])"
                    ],
                    "id": "foo 0.5.0 (path+file:[..]foo)"
                },
                {
                    "dependencies": [
                        "baz 0.0.1 (registry+file:[..])"
                    ],
                    "id": "bar 0.0.1 (registry+file:[..])"
                },
                {
                    "dependencies": [],
                    "id": "baz 0.0.1 (registry+file:[..])"
                }
            ],
            "root": "foo 0.5.0 (path+file:[..]foo)"
        },
        "version": 1
    }"#));
});

test!(cargo_metadata_with_invalid_manifest {
    let p = project("foo")
            .file("Cargo.toml", "");

    assert_that(p.cargo_process("metadata"), execs().with_status(101)
                                                    .with_stderr("\
failed to parse manifest at `[..]`

Caused by:
  no `package` or `project` section found."))
});

const MANIFEST_OUTPUT: &'static str=
    r#"
{
    "packages": [{
        "name":"foo",
        "version":"0.5.0",
        "id":"foo[..]0.5.0[..](path+file://[..]/foo)",
        "source":null,
        "dependencies":[],
        "targets":[{
            "kind":["bin"],
            "name":"foo",
            "src_path":"src[..]foo.rs"
        }],
        "features":{},
        "manifest_path":"[..]Cargo.toml"
    }],
    "resolve": null,
    "version": 1
}"#;

test!(cargo_metadata_no_deps_path_to_cargo_toml_relative {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

        assert_that(p.cargo_process("metadata").arg("--no-deps")
                     .arg("--manifest-path").arg("foo/Cargo.toml")
                     .cwd(p.root().parent().unwrap()),
                    execs().with_status(0)
                           .with_json(MANIFEST_OUTPUT));
});

test!(cargo_metadata_no_deps_path_to_cargo_toml_absolute {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("metadata").arg("--no-deps")
                 .arg("--manifest-path").arg(p.root().join("Cargo.toml"))
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(0)
                       .with_json(MANIFEST_OUTPUT));
});

test!(cargo_metadata_no_deps_path_to_cargo_toml_parent_relative {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("metadata").arg("--no-deps")
                 .arg("--manifest-path").arg("foo")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(101)
                       .with_stderr("the manifest-path must be a path to a Cargo.toml file"));
});

test!(cargo_metadata_no_deps_path_to_cargo_toml_parent_absolute {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("metadata").arg("--no-deps")
                 .arg("--manifest-path").arg(p.root())
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(101)
                       .with_stderr("the manifest-path must be a path to a Cargo.toml file"));
});

test!(cargo_metadata_no_deps_cwd {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("metadata").arg("--no-deps")
                 .cwd(p.root()),
                execs().with_status(0)
                       .with_json(MANIFEST_OUTPUT));
});

test!(carg_metadata_bad_version {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("metadata").arg("--no-deps")
                 .arg("--format-version").arg("2")
                 .cwd(p.root()),
                execs().with_status(101)
    .with_stderr("metadata version 2 not supported, only 1 is currently supported"));
});
