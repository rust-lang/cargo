use support::{project, execs, main_file, basic_bin_manifest};
use hamcrest::{assert_that};

fn setup() {}

fn read_manifest_output() -> String {
    "\
{\
    \"name\":\"foo\",\
    \"version\":\"0.5.0\",\
    \"dependencies\":[],\
    \"targets\":[{\
        \"kind\":[\"bin\"],\
        \"name\":\"foo\",\
        \"src_path\":\"src[..]foo.rs\",\
        \"metadata\":null\
    }],\
    \"manifest_path\":\"[..]Cargo.toml\"\
}".into()
}

test!(cargo_read_manifest_path_to_cargo_toml_relative {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("read-manifest")
                 .arg("--manifest-path").arg("foo/Cargo.toml")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(0)
                       .with_stdout(read_manifest_output()));
});

test!(cargo_read_manifest_path_to_cargo_toml_absolute {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("read-manifest")
                 .arg("--manifest-path").arg(p.root().join("Cargo.toml"))
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(0)
                       .with_stdout(read_manifest_output()));
});

test!(cargo_read_manifest_path_to_cargo_toml_parent_relative {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("read-manifest")
                 .arg("--manifest-path").arg("foo")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(0)
                       .with_stdout(read_manifest_output()));
});

test!(cargo_read_manifest_path_to_cargo_toml_parent_absolute {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("read-manifest")
                 .arg("--manifest-path").arg(p.root())
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(0)
                       .with_stdout(read_manifest_output()));
});

test!(cargo_read_manifest_cwd {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("read-manifest")
                 .cwd(p.root()),
                execs().with_status(0)
                       .with_stdout(read_manifest_output()));
});
