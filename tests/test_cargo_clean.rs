use support::{project, execs, main_file, basic_bin_manifest};
use hamcrest::{assert_that, existing_dir, is_not};

fn setup() {
}

test!(cargo_clean_simple {
    let p = project("foo")
              .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
              .file("src/foo.rs", main_file(r#""i am foo""#, []).as_slice());

    assert_that(p.cargo_process("cargo-build"), execs());
    assert_that(&p.build_dir(), existing_dir());

    assert_that(p.cargo_process("cargo-clean"), execs());
    assert_that(&p.build_dir(), is_not(existing_dir()));
})
