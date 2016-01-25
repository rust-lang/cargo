use hamcrest::{assert_that, existing_file};
use support::registry::Package;
use support::{project, execs, basic_bin_manifest};


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
            "package": [],
            "root": {
                "name": "foo",
                "version": "0.5.0",
                "source": null,
                "dependencies" : []
            },
            "metadata": null
        },
        "version": 1
    }"#));
});


test!(cargo_metadata_with_deps {
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

    assert_that(p.cargo_process("metadata").arg("-q"), execs().with_json(r#"
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
            "metadata": null,
            "package": [
                {
                    "dependencies": [
                        "baz 0.0.1 (registry+file:[..])"
                    ],
                    "name": "bar",
                    "source": "registry+file:[..]",
                    "version": "0.0.1"
                },
                {
                    "dependencies": [],
                    "name": "baz",
                    "source": "registry+file:[..]",
                    "version": "0.0.1"
                }
            ],
            "root": {
                "dependencies": [
                    "bar 0.0.1 (registry+file:[..])"
                ],
                "name": "foo",
                "source": null,
                "version": "0.5.0"
            }
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
