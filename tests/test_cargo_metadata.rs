use std::fs::File;
use std::io::prelude::*;

use support::{project, execs, basic_bin_manifest};
use hamcrest::{assert_that, existing_file, is, equal_to};


fn setup() {
}

test!(cargo_metadata_simple {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata"), execs().with_stdout(format!(r#"
[[packages]]
dependencies = []
manifest_path = "{}/Cargo.toml"
name = "foo"
version = "0.5.0"

[[packages.targets]]
kind = ["bin"]
name = "foo"
src_path = "src/foo.rs"

[root]
name = "foo"
version = "0.5.0"

"#, p.root().to_str().unwrap())));
});

test!(cargo_metadata_simple_json {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata").arg("-f").arg("json"), execs().with_stdout(format!(r#"{{"root":{{"name":"foo","version":"0.5.0","features":null}},"packages":[{{"name":"foo","version":"0.5.0","dependencies":[],"targets":[{{"kind":["bin"],"name":"foo","src_path":"src/foo.rs","metadata":null}}],"manifest_path":"{}/Cargo.toml"}}]}}
"#, p.root().to_str().unwrap())));
});

test!(cargo_metadata_with_invalid_manifest {
    let p = project("foo")
            .file("Cargo.toml", "");

    assert_that(p.cargo_process("metadata"),
    execs()
            .with_status(101)
            .with_stderr("\
failed to parse manifest at `[..]`

Caused by:
  No `package` or `project` section found.
"))
});

test!(cargo_metadata_simple_file {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata").arg("--output-path").arg("metadata.toml"), execs().with_status(0));

    let outputfile = p.root().join("metadata.toml");
    assert_that(&outputfile, existing_file());

    let mut output = String::new();
    File::open(&outputfile).unwrap().read_to_string(&mut output).unwrap();

    assert_that(output, is(equal_to(format!(r#"
[[packages]]
dependencies = []
manifest_path = "{}/Cargo.toml"
name = "foo"
version = "0.5.0"

[[packages.targets]]
kind = ["bin"]
name = "foo"
src_path = "src/foo.rs"

[root]
name = "foo"
version = "0.5.0"
"#, p.root().to_str().unwrap()))));
});