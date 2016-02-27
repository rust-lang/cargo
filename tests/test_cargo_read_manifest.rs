use support::{project, execs, main_file, basic_bin_manifest, ERROR};
use hamcrest::{assert_that};

fn setup() {}

fn remove_all_whitespace(s: &str) -> String {
    s.split_whitespace().collect()
}

fn read_manifest_output() -> String {
    remove_all_whitespace(r#"
{
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
}"#)
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
                execs().with_status(101)
                       .with_stderr(&format!("{error} the manifest-path must be \
                                             a path to a Cargo.toml file", error = ERROR)));
});

test!(cargo_read_manifest_path_to_cargo_toml_parent_absolute {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("read-manifest")
                 .arg("--manifest-path").arg(p.root())
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(101)
                       .with_stderr(&format!("{error} the manifest-path must be \
                                             a path to a Cargo.toml file", error = ERROR)));
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
