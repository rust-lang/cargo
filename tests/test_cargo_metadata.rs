use std::fs::File;
use std::io::prelude::*;

use hamcrest::{assert_that, existing_file, is, equal_to};
use support::{project, execs, basic_bin_manifest};


fn setup() {
}

test!(cargo_metadata_simple {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata"), execs().with_stdout(r#"
[[packages]]
dependencies = []
id = "foo 0.5.0 [..]"
manifest_path = "[..]Cargo.toml"
name = "foo"
version = "0.5.0"

[packages.features]

[[packages.targets]]
kind = ["bin"]
name = "foo"
src_path = "src[..]foo.rs"

[root]
name = "foo"
version = "0.5.0"

"#));
});


test!(cargo_metadata_simple_json {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata").arg("-f").arg("json"), execs().with_stdout(r#"
        {
            "root": {
                "name": "foo",
                "version": "0.5.0",
                "features": null
            },
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
            ]
        }"#.split_whitespace().collect::<String>()));
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

test!(cargo_metadata_with_invalid_output_format {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata").arg("--output-format").arg("XML"),
                execs().with_status(101)
                       .with_stderr("unknown format: XML, supported formats are TOML, JSON."))
});

test!(cargo_metadata_simple_file {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata").arg("--output-path").arg("metadata.toml"),
        execs().with_status(0));

    let outputfile = p.root().join("metadata.toml");
    assert_that(&outputfile, existing_file());

    let mut output = String::new();
    File::open(&outputfile).unwrap().read_to_string(&mut output).unwrap();

    assert_that(output[..].contains(r#"name = "foo""#), is(equal_to(true)));
});
