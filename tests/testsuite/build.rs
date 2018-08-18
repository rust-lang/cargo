use std::env;
use std::fs::{self, File};
use std::io::prelude::*;

use cargo::util::paths::dylib_path_envvar;
use cargo::util::{process, ProcessBuilder};
use support::{basic_manifest, basic_bin_manifest, basic_lib_manifest, is_nightly, rustc_host, sleep_ms};
use support::paths::{root, CargoPathExt};
use support::ProjectBuilder;
use support::{execs, main_file, project};
use support::registry::Package;
use support::ChannelChanger;
use support::hamcrest::{assert_that, existing_dir, existing_file, is_not};

#[test]
fn cargo_compile_simple() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("i am foo\n"),
    );
}

#[test]
fn cargo_fail_with_no_stderr() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &String::from("refusal"))
        .build();
    assert_that(
        p.cargo("build --message-format=json"),
        execs()
            .with_status(101)
            .with_stderr_does_not_contain("--- stderr"),
    );
}

/// Check that the `CARGO_INCREMENTAL` environment variable results in
/// `rustc` getting `-Zincremental` passed to it.
#[test]
fn cargo_compile_incremental() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    assert_that(
        p.cargo("build -v").env("CARGO_INCREMENTAL", "1"),
        execs().with_stderr_contains(
            "[RUNNING] `rustc [..] -C incremental=[..]/target/debug/incremental[..]`\n",
        )
    );

    assert_that(
        p.cargo("test -v").env("CARGO_INCREMENTAL", "1"),
        execs().with_stderr_contains(
            "[RUNNING] `rustc [..] -C incremental=[..]/target/debug/incremental[..]`\n",
        )
    );
}

#[test]
fn incremental_profile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [profile.dev]
            incremental = false

            [profile.release]
            incremental = true
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build -v").env_remove("CARGO_INCREMENTAL"),
        execs().with_stderr_does_not_contain("[..]C incremental=[..]"),
    );

    assert_that(
        p.cargo("build -v").env("CARGO_INCREMENTAL", "1"),
        execs().with_stderr_contains("[..]C incremental=[..]"),
    );

    assert_that(
        p.cargo("build --release -v")
            .env_remove("CARGO_INCREMENTAL"),
        execs().with_stderr_contains("[..]C incremental=[..]"),
    );

    assert_that(
        p.cargo("build --release -v")
            .env("CARGO_INCREMENTAL", "0"),
        execs().with_stderr_does_not_contain("[..]C incremental=[..]"),
    );
}

#[test]
fn incremental_config() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [build]
            incremental = false
        "#,
        )
        .build();

    assert_that(
        p.cargo("build -v").env_remove("CARGO_INCREMENTAL"),
        execs()
            .with_stderr_does_not_contain("[..]C incremental=[..]"),
    );

    assert_that(
        p.cargo("build -v").env("CARGO_INCREMENTAL", "1"),
        execs().with_stderr_contains("[..]C incremental=[..]"),
    );
}

#[test]
fn cargo_compile_with_workspace_excluded() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build --all --exclude foo"),
        execs()
            .with_stderr_does_not_contain("[..]virtual[..]")
            .with_stderr_contains("[..]no packages to compile")
            .with_status(101),
    );
}

#[test]
fn cargo_compile_manifest_path() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    assert_that(
        p.cargo("build --manifest-path foo/Cargo.toml")
            .cwd(p.root().parent().unwrap()),
        execs(),
    );
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn cargo_compile_with_invalid_manifest() {
    let p = project().file("Cargo.toml", "").build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  virtual manifests must be configured with [workspace]
",
        ),
    )
}

#[test]
fn cargo_compile_with_invalid_manifest2() {
    let p = project()
        .file(
            "Cargo.toml",
            r"
            [project]
            foo = bar
        ",
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  invalid number at line 3
",
        ),
    )
}

#[test]
fn cargo_compile_with_invalid_manifest3() {
    let p = project()
        .file("src/Cargo.toml", "a = bar")
        .build();

    assert_that(
        p.cargo("build --manifest-path src/Cargo.toml"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  invalid number at line 1
",
        ),
    )
}

#[test]
fn cargo_compile_duplicate_build_targets() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "main"
            path = "src/main.rs"
            crate-type = ["dylib"]

            [dependencies]
        "#,
        )
        .file("src/main.rs", "#![allow(warnings)] fn main() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
warning: file found to be present in multiple build targets: [..]main.rs
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        ),
    );
}

#[test]
fn cargo_compile_with_invalid_version() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0"))
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Expected dot for key `package.version`
",
        ),
    )
}

#[test]
fn cargo_compile_with_invalid_package_name() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("", "0.0.0"))
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  package name cannot be an empty string
",
        ),
    )
}

#[test]
fn cargo_compile_with_invalid_bin_target_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"

            [[bin]]
            name = ""
        "#,
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  binary target names cannot be empty
",
        ),
    )
}

#[test]
fn cargo_compile_with_forbidden_bin_target_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"

            [[bin]]
            name = "build"
        "#,
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  the binary target name `build` is forbidden
",
        ),
    )
}

#[test]
fn cargo_compile_with_bin_and_crate_type() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"

            [[bin]]
            name = "the_foo_bin"
            path = "src/foo.rs"
            crate-type = ["cdylib", "rlib"]
        "#,
        )
        .file("src/foo.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  the target `the_foo_bin` is a binary and can't have any crate-types set \
(currently \"cdylib, rlib\")",
        ),
    )
}

#[test]
fn cargo_compile_with_bin_and_proc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"

            [[bin]]
            name = "the_foo_bin"
            path = "src/foo.rs"
            proc-macro = true
        "#,
        )
        .file("src/foo.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  the target `the_foo_bin` is a binary and can't have `proc-macro` set `true`",
        ),
    )
}

#[test]
fn cargo_compile_with_invalid_lib_target_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"

            [lib]
            name = ""
        "#,
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  library target names cannot be empty
",
        ),
    )
}

#[test]
fn cargo_compile_without_manifest() {
    let p = project().no_manifest().build();

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] could not find `Cargo.toml` in `[..]` or any parent directory"),
    );
}

#[test]
fn cargo_compile_with_invalid_code() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", "invalid rust code!")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr_contains(
            "\
[ERROR] Could not compile `foo`.

To learn more, run the command again with --verbose.\n",
        ),
    );
    assert_that(&p.root().join("Cargo.lock"), existing_file());
}

#[test]
fn cargo_compile_with_invalid_code_in_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
            [dependencies.baz]
            path = "../baz"
        "#,
        )
        .file("src/main.rs", "invalid rust code!")
        .build();
    let _bar = project().at("bar")
        .file("Cargo.toml", &basic_bin_manifest("bar"))
        .file("src/lib.rs", "invalid rust code!")
        .build();
    let _baz = project().at("baz")
        .file("Cargo.toml", &basic_bin_manifest("baz"))
        .file("src/lib.rs", "invalid rust code!")
        .build();
    assert_that(p.cargo("build"), execs().with_status(101));
}

#[test]
fn cargo_compile_with_warnings_in_the_root_package() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", "fn main() {} fn dead() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs()
            .with_stderr_contains("[..]function is never used: `dead`[..]"),
    );
}

#[test]
fn cargo_compile_with_warnings_in_a_dep_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"

            [[bin]]

            name = "foo"
        "#,
        )
        .file("src/foo.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file(
            "bar/src/bar.rs",
            r#"
            pub fn gimme() -> &'static str {
                "test passed"
            }

            fn dead() {}
        "#,
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs()
            .with_stderr_contains("[..]function is never used: `dead`[..]"),
    );

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("test passed\n"),
    );
}

#[test]
fn cargo_compile_with_nested_deps_inferred() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = 'bar'

            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/foo.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]
            path = "../baz"
        "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
            extern crate baz;

            pub fn gimme() -> String {
                baz::gimme()
            }
        "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.5.0"))
        .file(
            "baz/src/lib.rs",
            r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#,
        )
        .build();

    p.cargo("build").exec_with_output().unwrap();

    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("libbar.rlib"), is_not(existing_file()));
    assert_that(&p.bin("libbaz.rlib"), is_not(existing_file()));

    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("test passed\n"),
    );
}

#[test]
fn cargo_compile_with_nested_deps_correct_bin() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"

            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]
            path = "../baz"
        "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
            extern crate baz;

            pub fn gimme() -> String {
                baz::gimme()
            }
        "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.5.0"))
        .file(
            "baz/src/lib.rs",
            r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#,
        )
        .build();

    p.cargo("build").exec_with_output().unwrap();

    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("libbar.rlib"), is_not(existing_file()));
    assert_that(&p.bin("libbaz.rlib"), is_not(existing_file()));

    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("test passed\n"),
    );
}

#[test]
fn cargo_compile_with_nested_deps_shorthand() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]
            path = "../baz"

            [lib]

            name = "bar"
        "#,
        )
        .file(
            "bar/src/bar.rs",
            r#"
            extern crate baz;

            pub fn gimme() -> String {
                baz::gimme()
            }
        "#,
        )
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file(
            "baz/src/baz.rs",
            r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#,
        )
        .build();

    p.cargo("build").exec_with_output().unwrap();

    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("libbar.rlib"), is_not(existing_file()));
    assert_that(&p.bin("libbaz.rlib"), is_not(existing_file()));

    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("test passed\n"),
    );
}

#[test]
fn cargo_compile_with_nested_deps_longhand() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"
            version = "0.5.0"

            [[bin]]

            name = "foo"
        "#,
        )
        .file("src/foo.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]
            path = "../baz"
            version = "0.5.0"

            [lib]

            name = "bar"
        "#,
        )
        .file(
            "bar/src/bar.rs",
            r#"
            extern crate baz;

            pub fn gimme() -> String {
                baz::gimme()
            }
        "#,
        )
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file(
            "baz/src/baz.rs",
            r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#,
        )
        .build();

    assert_that(p.cargo("build"), execs());

    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("libbar.rlib"), is_not(existing_file()));
    assert_that(&p.bin("libbaz.rlib"), is_not(existing_file()));

    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("test passed\n"),
    );
}

// Check that Cargo gives a sensible error if a dependency can't be found
// because of a name mismatch.
#[test]
fn cargo_compile_with_dep_name_mismatch() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]

            name = "foo"
            version = "0.0.1"
            authors = ["wycats@example.com"]

            [[bin]]

            name = "foo"

            [dependencies.notquitebar]

            path = "bar"
        "#,
        )
        .file("src/bin/foo.rs", &main_file(r#""i am foo""#, &["bar"]))
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/bar.rs", &main_file(r#""i am bar""#, &[]))
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(&format!(
            r#"error: no matching package named `notquitebar` found
location searched: {proj_dir}/bar
required by package `foo v0.0.1 ({proj_dir})`
"#,
            proj_dir = p.url()
        )),
    );
}

#[test]
fn cargo_compile_with_filename() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "src/bin/a.rs",
            r#"
            extern crate foo;
            fn main() { println!("hello a.rs"); }
        "#,
        )
        .file("examples/a.rs", r#"fn main() { println!("example"); }"#)
        .build();

    assert_that(
        p.cargo("build --bin bin.rs"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] no bin target named `bin.rs`"),
    );

    assert_that(
        p.cargo("build --bin a.rs"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] no bin target named `a.rs`

Did you mean `a`?",
        ),
    );

    assert_that(
        p.cargo("build --example example.rs"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] no example target named `example.rs`"),
    );

    assert_that(
        p.cargo("build --example a.rs"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] no example target named `a.rs`

Did you mean `a`?",
        ),
    );
}

#[test]
fn cargo_compile_path_with_offline() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -Zoffline")
            .masquerade_as_nightly_cargo(),
        execs(),
    );
}

#[test]
fn cargo_compile_with_downloaded_dependency_with_offline() {
    Package::new("present_dep", "1.2.3")
        .file("Cargo.toml", &basic_manifest("present_dep", "1.2.3"))
        .file("src/lib.rs", "")
        .publish();

    {
        // make package downloaded
        let p = project()
            .file(
                "Cargo.toml",
                r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            present_dep = "1.2.3"
        "#,
            )
            .file("src/lib.rs", "")
            .build();
        assert_that(p.cargo("build"), execs());
    }

    let p2 = project().at("bar")
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.1.0"

            [dependencies]
            present_dep = "1.2.3"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p2.cargo("build -Zoffline")
            .masquerade_as_nightly_cargo(),
        execs().with_stderr(
            "\
[COMPILING] present_dep v1.2.3
[COMPILING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"
    ),
    );
}

#[test]
fn cargo_compile_offline_not_try_update() {
    let p = project().at("bar")
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.1.0"

            [dependencies]
            not_cached_dep = "1.2.5"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -Zoffline")
            .masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
error: no matching package named `not_cached_dep` found
location searched: registry `[..]`
required by package `bar v0.1.0 ([..])`
As a reminder, you're using offline mode (-Z offline) \
which can sometimes cause surprising resolution failures, \
if this error is too confusing you may with to retry \
without the offline flag.",
        ),
    );
}

#[test]
fn compile_offline_without_maxvers_cached() {
    Package::new("present_dep", "1.2.1").publish();
    Package::new("present_dep", "1.2.2").publish();

    Package::new("present_dep", "1.2.3")
        .file("Cargo.toml", &basic_manifest("present_dep", "1.2.3"))
        .file("src/lib.rs", r#"pub fn get_version()->&'static str {"1.2.3"}"#)
        .publish();

    Package::new("present_dep", "1.2.5")
        .file("Cargo.toml", &basic_manifest("present_dep", "1.2.5"))
        .file("src/lib.rs", r#"pub fn get_version(){"1.2.5"}"#)
        .publish();

    {
        // make package cached
        let p = project()
            .file(
                "Cargo.toml",
                r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            present_dep = "=1.2.3"
        "#,
            )
            .file("src/lib.rs", "")
            .build();
        assert_that(p.cargo("build"), execs());
    }

    let p2 = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            present_dep = "1.2"
        "#,
        )
        .file(
            "src/main.rs",
            "\
extern crate present_dep;
fn main(){
    println!(\"{}\", present_dep::get_version());
}",
        )
        .build();

    assert_that(
        p2.cargo("run -Zoffline")
            .masquerade_as_nightly_cargo(),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] present_dep v1.2.3
[COMPILING] foo v0.1.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
     Running `[..]`",
                url = p2.url()
            ))
            .with_stdout("1.2.3"),
    );
}

#[test]
fn incompatible_dependencies() {
    Package::new("bad", "0.1.0").publish();
    Package::new("bad", "1.0.0").publish();
    Package::new("bad", "1.0.1").publish();
    Package::new("bad", "1.0.2").publish();
    Package::new("bar", "0.1.0").dep("bad", "0.1.0").publish();
    Package::new("baz", "0.1.1").dep("bad", "=1.0.0").publish();
    Package::new("baz", "0.1.0").dep("bad", "=1.0.0").publish();
    Package::new("qux", "0.1.2").dep("bad", ">=1.0.1").publish();
    Package::new("qux", "0.1.1").dep("bad", ">=1.0.1").publish();
    Package::new("qux", "0.1.0").dep("bad", ">=1.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = "0.1.0"
            baz = "0.1.0"
            qux = "0.1.0"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr_contains(
            "\
error: failed to select a version for `bad`.
    ... required by package `qux v0.1.0`
    ... which is depended on by `foo v0.0.1 ([..])`
versions that meet the requirements `>= 1.0.1` are: 1.0.2, 1.0.1

all possible versions conflict with previously selected packages.

  previously selected package `bad v1.0.0`
    ... which is depended on by `baz v0.1.0`
    ... which is depended on by `foo v0.0.1 ([..])`

failed to select a version for `bad` which could resolve this conflict",
        ),
    );
}

#[test]
fn incompatible_dependencies_with_multi_semver() {
    Package::new("bad", "1.0.0").publish();
    Package::new("bad", "1.0.1").publish();
    Package::new("bad", "2.0.0").publish();
    Package::new("bad", "2.0.1").publish();
    Package::new("bar", "0.1.0").dep("bad", "=1.0.0").publish();
    Package::new("baz", "0.1.0").dep("bad", ">=2.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = "0.1.0"
            baz = "0.1.0"
            bad = ">=1.0.1, <=2.0.0"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr_contains(
            "\
error: failed to select a version for `bad`.
    ... required by package `foo v0.0.1 ([..])`
versions that meet the requirements `>= 1.0.1, <= 2.0.0` are: 2.0.0, 1.0.1

all possible versions conflict with previously selected packages.

  previously selected package `bad v2.0.1`
    ... which is depended on by `baz v0.1.0`
    ... which is depended on by `foo v0.0.1 ([..])`

  previously selected package `bad v1.0.0`
    ... which is depended on by `bar v0.1.0`
    ... which is depended on by `foo v0.0.1 ([..])`

failed to select a version for `bad` which could resolve this conflict",
        ),
    );
}

#[test]
fn compile_offline_while_transitive_dep_not_cached() {
    let baz = Package::new("baz", "1.0.0");
    let baz_path = baz.archive_dst();
    baz.publish();

    let mut content = Vec::new();

    let mut file = File::open(baz_path.clone()).ok().unwrap();
    let _ok = file.read_to_end(&mut content).ok().unwrap();
    drop(file);
    drop(File::create(baz_path.clone()).ok().unwrap());

    Package::new("bar", "0.1.0").dep("baz", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = "0.1.0"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    // simulate download bar, but fail to download baz
    let _out = p.cargo("build").exec_with_output();

    drop(File::create(baz_path).ok().unwrap().write_all(&content));

    assert_that(
        p.cargo("build -Zoffline")
            .masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
error: no matching package named `baz` found
location searched: registry `[..]`
required by package `bar v0.1.0`
    ... which is depended on by `foo v0.0.1 ([..]/foo)`
As a reminder, you're using offline mode (-Z offline) \
which can sometimes cause surprising resolution failures, \
if this error is too confusing you may with to retry \
without the offline flag.",
        ),
    );
}

#[test]
fn compile_path_dep_then_change_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs());

    File::create(&p.root().join("bar/Cargo.toml"))
        .unwrap()
        .write_all(basic_manifest("bar", "0.0.2").as_bytes())
        .unwrap();

    assert_that(p.cargo("build"), execs());
}

#[test]
fn ignores_carriage_return_in_lockfile() {
    let p = project()
        .file("src/main.rs", r"mod a; fn main() {}")
        .file("src/a.rs", "")
        .build();

    assert_that(p.cargo("build"), execs());

    let lockfile = p.root().join("Cargo.lock");
    let mut lock = String::new();
    File::open(&lockfile)
        .unwrap()
        .read_to_string(&mut lock)
        .unwrap();
    let lock = lock.replace("\n", "\r\n");
    File::create(&lockfile)
        .unwrap()
        .write_all(lock.as_bytes())
        .unwrap();
    assert_that(p.cargo("build"), execs());
}

#[test]
fn cargo_default_env_metadata_env_var() {
    // Ensure that path dep + dylib + env_var get metadata
    // (even though path_dep + dylib should not)
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/lib.rs", "// hi")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            crate_type = ["dylib"]
        "#,
        )
        .file("bar/src/lib.rs", "// hello")
        .build();

    // No metadata on libbar since it's a dylib path dependency
    assert_that(
        p.cargo("build -v"),
        execs().with_stderr(&format!(
            "\
[COMPILING] bar v0.0.1 ({url}/bar)
[RUNNING] `rustc --crate-name bar bar/src/lib.rs --crate-type dylib \
        --emit=dep-info,link \
        -C prefer-dynamic -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}/target/debug/deps`
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc --crate-name foo src/lib.rs --crate-type lib \
        --emit=dep-info,link -C debuginfo=2 \
        -C metadata=[..] \
        -C extra-filename=[..] \
        --out-dir [..] \
        -L dependency={dir}/target/debug/deps \
        --extern bar={dir}/target/debug/deps/{prefix}bar{suffix}`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
            dir = p.root().display(),
            url = p.url(),
            prefix = env::consts::DLL_PREFIX,
            suffix = env::consts::DLL_SUFFIX,
        )),
    );

    assert_that(p.cargo("clean"), execs());

    // If you set the env-var, then we expect metadata on libbar
    assert_that(
        p.cargo("build -v")
            .env("__CARGO_DEFAULT_LIB_METADATA", "stable"),
        execs().with_stderr(&format!(
            "\
[COMPILING] bar v0.0.1 ({url}/bar)
[RUNNING] `rustc --crate-name bar bar/src/lib.rs --crate-type dylib \
        --emit=dep-info,link \
        -C prefer-dynamic -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}/target/debug/deps`
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc --crate-name foo src/lib.rs --crate-type lib \
        --emit=dep-info,link -C debuginfo=2 \
        -C metadata=[..] \
        -C extra-filename=[..] \
        --out-dir [..] \
        -L dependency={dir}/target/debug/deps \
        --extern bar={dir}/target/debug/deps/{prefix}bar-[..]{suffix}`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url(),
            prefix = env::consts::DLL_PREFIX,
            suffix = env::consts::DLL_SUFFIX,
        )),
    );
}

#[test]
fn crate_env_vars() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [project]
        name = "foo"
        version = "0.5.1-alpha.1"
        description = "This is foo"
        homepage = "http://example.com"
        authors = ["wycats@example.com"]
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            extern crate foo;


            static VERSION_MAJOR: &'static str = env!("CARGO_PKG_VERSION_MAJOR");
            static VERSION_MINOR: &'static str = env!("CARGO_PKG_VERSION_MINOR");
            static VERSION_PATCH: &'static str = env!("CARGO_PKG_VERSION_PATCH");
            static VERSION_PRE: &'static str = env!("CARGO_PKG_VERSION_PRE");
            static VERSION: &'static str = env!("CARGO_PKG_VERSION");
            static CARGO_MANIFEST_DIR: &'static str = env!("CARGO_MANIFEST_DIR");
            static PKG_NAME: &'static str = env!("CARGO_PKG_NAME");
            static HOMEPAGE: &'static str = env!("CARGO_PKG_HOMEPAGE");
            static DESCRIPTION: &'static str = env!("CARGO_PKG_DESCRIPTION");

            fn main() {
                let s = format!("{}-{}-{} @ {} in {}", VERSION_MAJOR,
                                VERSION_MINOR, VERSION_PATCH, VERSION_PRE,
                                CARGO_MANIFEST_DIR);
                 assert_eq!(s, foo::version());
                 println!("{}", s);
                 assert_eq!("foo", PKG_NAME);
                 assert_eq!("http://example.com", HOMEPAGE);
                 assert_eq!("This is foo", DESCRIPTION);
                let s = format!("{}.{}.{}-{}", VERSION_MAJOR,
                                VERSION_MINOR, VERSION_PATCH, VERSION_PRE);
                assert_eq!(s, VERSION);
            }
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn version() -> String {
                format!("{}-{}-{} @ {} in {}",
                        env!("CARGO_PKG_VERSION_MAJOR"),
                        env!("CARGO_PKG_VERSION_MINOR"),
                        env!("CARGO_PKG_VERSION_PATCH"),
                        env!("CARGO_PKG_VERSION_PRE"),
                        env!("CARGO_MANIFEST_DIR"))
            }
        "#,
        )
        .build();

    println!("build");
    assert_that(p.cargo("build -v"), execs());

    println!("bin");
    assert_that(
        process(&p.bin("foo")),
        execs()
            .with_stdout(&format!("0-5-1 @ alpha.1 in {}\n", p.root().display())),
    );

    println!("test");
    assert_that(p.cargo("test -v"), execs());
}

#[test]
fn crate_authors_env_vars() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.5.1-alpha.1"
            authors = ["wycats@example.com", "neikos@example.com"]
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            extern crate foo;

            static AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");

            fn main() {
                let s = "wycats@example.com:neikos@example.com";
                assert_eq!(AUTHORS, foo::authors());
                println!("{}", AUTHORS);
                assert_eq!(s, AUTHORS);
            }
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn authors() -> String {
                format!("{}", env!("CARGO_PKG_AUTHORS"))
            }
        "#,
        )
        .build();

    println!("build");
    assert_that(p.cargo("build -v"), execs());

    println!("bin");
    assert_that(
        process(&p.bin("foo")),
        execs()
            .with_stdout("wycats@example.com:neikos@example.com"),
    );

    println!("test");
    assert_that(p.cargo("test -v"), execs());
}

// The tester may already have LD_LIBRARY_PATH=::/foo/bar which leads to a false positive error
fn setenv_for_removing_empty_component(mut p: ProcessBuilder) -> ProcessBuilder {
    let v = dylib_path_envvar();
    if let Ok(search_path) = env::var(v) {
        let new_search_path = env::join_paths(
            env::split_paths(&search_path).filter(|e| !e.as_os_str().is_empty()),
        ).expect("join_paths");
        p.env(v, new_search_path); // build_command() will override LD_LIBRARY_PATH accordingly
    }
    p
}

// Regression test for #4277
#[test]
fn crate_library_path_env_var() {
    let p = project()
        .file(
            "src/main.rs",
            &format!(
                r##"
            fn main() {{
                let search_path = env!("{}");
                let paths = std::env::split_paths(&search_path).collect::<Vec<_>>();
                assert!(!paths.contains(&"".into()));
            }}
        "##,
                dylib_path_envvar()
            ),
        )
        .build();

    assert_that(
        setenv_for_removing_empty_component(p.cargo("run")),
        execs(),
    );
}

// Regression test for #4277
#[test]
fn build_with_fake_libc_not_loading() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .file("libc.so.6", r#""#)
        .build();

    assert_that(
        setenv_for_removing_empty_component(p.cargo("build")),
        execs(),
    );
}

// this is testing that src/<pkg-name>.rs still works (for now)
#[test]
fn many_crate_types_old_style_lib_location() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "foo"
            crate_type = ["rlib", "dylib"]
        "#,
        )
        .file("src/foo.rs", "pub fn foo() {}")
        .build();
    assert_that(
        p.cargo("build"),
        execs().with_stderr_contains(
            "\
[WARNING] path `[..]src/foo.rs` was erroneously implicitly accepted for library `foo`,
please rename the file to `src/lib.rs` or set lib.path in Cargo.toml",
        ),
    );

    assert_that(&p.root().join("target/debug/libfoo.rlib"), existing_file());
    let fname = format!("{}foo{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX);
    assert_that(&p.root().join("target/debug").join(&fname), existing_file());
}

#[test]
fn many_crate_types_correct() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "foo"
            crate_type = ["rlib", "dylib"]
        "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();
    assert_that(p.cargo("build"), execs());

    assert_that(&p.root().join("target/debug/libfoo.rlib"), existing_file());
    let fname = format!("{}foo{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX);
    assert_that(&p.root().join("target/debug").join(&fname), existing_file());
}

#[test]
fn self_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [dependencies.test]

            path = "."

            [lib]
            name = "test"
            path = "src/test.rs"
        "#,
        )
        .file("src/test.rs", "fn main() {}")
        .build();
    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] cyclic package dependency: package `test v0.0.0 ([..])` depends on itself. Cycle:
package `test v0.0.0 ([..]foo)`",
        ),
    );
}

#[test]
fn ignore_broken_symlinks() {
    // windows and symlinks don't currently agree that well
    if cfg!(windows) {
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .symlink("Notafile", "bar")
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("i am foo\n"),
    );
}

#[test]
fn missing_lib_and_bin() {
    let p = project().build();
    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]Cargo.toml`

Caused by:
  no targets specified in the manifest
  either src/lib.rs, src/main.rs, a [lib] section, or [[bin]] section must be present\n",
        ),
    );
}

#[test]
fn lto_build() {
    // FIXME: currently this hits a linker bug on 32-bit MSVC
    if cfg!(all(target_env = "msvc", target_pointer_width = "32")) {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.release]
            lto = true
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    assert_that(
        p.cargo("build -v --release"),
        execs().with_stderr(&format!(
            "\
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc --crate-name test src/main.rs --crate-type bin \
        --emit=dep-info,link \
        -C opt-level=3 \
        -C lto \
        -C metadata=[..] \
        --out-dir {dir}/target/release/deps \
        -L dependency={dir}/target/release/deps`
[FINISHED] release [optimized] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url(),
        )),
    );
}

#[test]
fn verbose_build() {
    let p = project()
        .file("src/lib.rs", "")
        .build();
    assert_that(
        p.cargo("build -v"),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc --crate-name foo src/lib.rs --crate-type lib \
        --emit=dep-info,link -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url(),
        )),
    );
}

#[test]
fn verbose_release_build() {
    let p = project()
        .file("src/lib.rs", "")
        .build();
    assert_that(
        p.cargo("build -v --release"),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc --crate-name foo src/lib.rs --crate-type lib \
        --emit=dep-info,link \
        -C opt-level=3 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}/target/release/deps`
[FINISHED] release [optimized] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url(),
        )),
    );
}

#[test]
fn verbose_release_build_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [dependencies.foo]
            path = "foo"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
            [package]

            name = "foo"
            version = "0.0.0"
            authors = []

            [lib]
            name = "foo"
            crate_type = ["dylib", "rlib"]
        "#,
        )
        .file("foo/src/lib.rs", "")
        .build();
    assert_that(
        p.cargo("build -v --release"),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.0 ({url}/foo)
[RUNNING] `rustc --crate-name foo foo/src/lib.rs \
        --crate-type dylib --crate-type rlib \
        --emit=dep-info,link \
        -C prefer-dynamic \
        -C opt-level=3 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}/target/release/deps`
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc --crate-name test src/lib.rs --crate-type lib \
        --emit=dep-info,link \
        -C opt-level=3 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}/target/release/deps \
        --extern foo={dir}/target/release/deps/{prefix}foo{suffix} \
        --extern foo={dir}/target/release/deps/libfoo.rlib`
[FINISHED] release [optimized] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url(),
            prefix = env::consts::DLL_PREFIX,
            suffix = env::consts::DLL_SUFFIX
        )),
    );
}

#[test]
fn explicit_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            authors = []

            [lib]
            name = "foo"
            path = "src/lib.rs"

            [[example]]
            name = "hello"
            path = "examples/ex-hello.rs"

            [[example]]
            name = "goodbye"
            path = "examples/ex-goodbye.rs"
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn get_hello() -> &'static str { "Hello" }
            pub fn get_goodbye() -> &'static str { "Goodbye" }
            pub fn get_world() -> &'static str { "World" }
        "#,
        )
        .file(
            "examples/ex-hello.rs",
            r#"
            extern crate foo;
            fn main() { println!("{}, {}!", foo::get_hello(), foo::get_world()); }
        "#,
        )
        .file(
            "examples/ex-goodbye.rs",
            r#"
            extern crate foo;
            fn main() { println!("{}, {}!", foo::get_goodbye(), foo::get_world()); }
        "#,
        )
        .build();

    assert_that(p.cargo("test -v"), execs());
    assert_that(
        process(&p.bin("examples/hello")),
        execs().with_stdout("Hello, World!\n"),
    );
    assert_that(
        process(&p.bin("examples/goodbye")),
        execs().with_stdout("Goodbye, World!\n"),
    );
}

#[test]
fn non_existing_example() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            authors = []

            [lib]
            name = "foo"
            path = "src/lib.rs"

            [[example]]
            name = "hello"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("test -v"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  can't find `hello` example, specify example.path",
        ),
    );
}

#[test]
fn non_existing_binary() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/bin/ehlo.rs", "")
        .build();

    assert_that(
        p.cargo("build -v"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  can't find `foo` bin, specify bin.path",
        ),
    );
}

#[test]
fn legacy_binary_paths_warnings() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            authors = []

            [[bin]]
            name = "bar"
        "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build -v"),
        execs().with_stderr_contains(
            "\
[WARNING] path `[..]src/main.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml",
        ),
    );

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            authors = []

            [[bin]]
            name = "bar"
        "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build -v"),
        execs().with_stderr_contains(
            "\
[WARNING] path `[..]src/bin/main.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml",
        ),
    );

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            authors = []

            [[bin]]
            name = "bar"
        "#,
        )
        .file("src/bar.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build -v"),
        execs().with_stderr_contains(
            "\
[WARNING] path `[..]src/bar.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml",
        ),
    );
}

#[test]
fn implicit_examples() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            pub fn get_hello() -> &'static str { "Hello" }
            pub fn get_goodbye() -> &'static str { "Goodbye" }
            pub fn get_world() -> &'static str { "World" }
        "#,
        )
        .file(
            "examples/hello.rs",
            r#"
            extern crate foo;
            fn main() {
                println!("{}, {}!", foo::get_hello(), foo::get_world());
            }
        "#,
        )
        .file(
            "examples/goodbye.rs",
            r#"
            extern crate foo;
            fn main() {
                println!("{}, {}!", foo::get_goodbye(), foo::get_world());
            }
        "#,
        )
        .build();

    assert_that(p.cargo("test"), execs());
    assert_that(
        process(&p.bin("examples/hello")),
        execs().with_stdout("Hello, World!\n"),
    );
    assert_that(
        process(&p.bin("examples/goodbye")),
        execs().with_stdout("Goodbye, World!\n"),
    );
}

#[test]
fn standard_build_no_ndebug() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/foo.rs",
            r#"
            fn main() {
                if cfg!(debug_assertions) {
                    println!("slow")
                } else {
                    println!("fast")
                }
            }
        "#,
        )
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("slow\n"),
    );
}

#[test]
fn release_build_ndebug() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/foo.rs",
            r#"
            fn main() {
                if cfg!(debug_assertions) {
                    println!("slow")
                } else {
                    println!("fast")
                }
            }
        "#,
        )
        .build();

    assert_that(p.cargo("build --release"), execs());
    assert_that(
        process(&p.release_bin("foo")),
        execs().with_stdout("fast\n"),
    );
}

#[test]
fn inferred_main_bin() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(process(&p.bin("foo")), execs());
}

#[test]
fn deletion_causes_failure() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs());
    p.change_file("Cargo.toml", &basic_manifest("foo", "0.0.1"));
    assert_that(p.cargo("build"), execs().with_status(101));
}

#[test]
fn bad_cargo_toml_in_target_dir() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("target/Cargo.toml", "bad-toml")
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(process(&p.bin("foo")), execs());
}

#[test]
fn lib_with_standard_name() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("syntax", "0.0.1"))
        .file("src/lib.rs", "pub fn foo() {}")
        .file("src/main.rs", "extern crate syntax; fn main() { syntax::foo() }")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[COMPILING] syntax v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
}

#[test]
fn simple_staticlib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
              [package]
              name = "foo"
              authors = []
              version = "0.0.1"

              [lib]
              name = "foo"
              crate-type = ["staticlib"]
        "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();

    // env var is a test for #1381
    assert_that(
        p.cargo("build").env("RUST_LOG", "nekoneko=trace"),
        execs(),
    );
}

#[test]
fn staticlib_rlib_and_bin() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
              [package]
              name = "foo"
              authors = []
              version = "0.0.1"

              [lib]
              name = "foo"
              crate-type = ["staticlib", "rlib"]
        "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file("src/main.rs", "extern crate foo; fn main() { foo::foo(); }")
        .build();

    assert_that(p.cargo("build -v"), execs());
}

#[test]
fn opt_out_of_bin() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
              bin = []

              [package]
              name = "foo"
              authors = []
              version = "0.0.1"
        "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "bad syntax")
        .build();
    assert_that(p.cargo("build"), execs());
}

#[test]
fn single_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
              [package]
              name = "foo"
              authors = []
              version = "0.0.1"

              [lib]
              name = "foo"
              path = "src/bar.rs"
        "#,
        )
        .file("src/bar.rs", "")
        .build();
    assert_that(p.cargo("build"), execs());
}

#[test]
fn freshness_ignores_excluded() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "build.rs"
            exclude = ["src/b*.rs"]
        "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
        .build();
    foo.root().move_into_the_past();

    assert_that(
        foo.cargo("build"),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            url = foo.url()
        )),
    );

    // Smoke test to make sure it doesn't compile again
    println!("first pass");
    assert_that(foo.cargo("build"), execs().with_stdout(""));

    // Modify an ignored file and make sure we don't rebuild
    println!("second pass");
    File::create(&foo.root().join("src/bar.rs")).unwrap();
    assert_that(foo.cargo("build"), execs().with_stdout(""));
}

#[test]
fn rebuild_preserves_out_dir() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = 'build.rs'
        "#,
        )
        .file(
            "build.rs",
            r#"
            use std::env;
            use std::fs::File;
            use std::path::Path;

            fn main() {
                let path = Path::new(&env::var("OUT_DIR").unwrap()).join("foo");
                if env::var_os("FIRST").is_some() {
                    File::create(&path).unwrap();
                } else {
                    File::create(&path).unwrap();
                }
            }
        "#,
        )
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
        .build();
    foo.root().move_into_the_past();

    assert_that(
        foo.cargo("build").env("FIRST", "1"),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            url = foo.url()
        )),
    );

    File::create(&foo.root().join("src/bar.rs")).unwrap();
    assert_that(
        foo.cargo("build"),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            url = foo.url()
        )),
    );
}

#[test]
fn dep_no_libs() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.0"))
        .file("bar/src/main.rs", "")
        .build();
    assert_that(foo.cargo("build"), execs());
}

#[test]
fn recompile_space_in_name() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [lib]
            name = "foo"
            path = "src/my lib.rs"
        "#,
        )
        .file("src/my lib.rs", "")
        .build();
    assert_that(foo.cargo("build"), execs());
    foo.root().move_into_the_past();
    assert_that(foo.cargo("build"), execs().with_stdout(""));
}

#[cfg(unix)]
#[test]
fn ignore_bad_directories() {
    use std::os::unix::prelude::*;
    let foo = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();
    let dir = foo.root().join("tmp");
    fs::create_dir(&dir).unwrap();
    let stat = fs::metadata(&dir).unwrap();
    let mut perms = stat.permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&dir, perms.clone()).unwrap();
    assert_that(foo.cargo("build"), execs());
    perms.set_mode(0o755);
    fs::set_permissions(&dir, perms).unwrap();
}

#[test]
fn bad_cargo_config() {
    let foo = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .file(".cargo/config", "this is not valid toml")
        .build();
    assert_that(
        foo.cargo("build -v"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] could not load Cargo configuration

Caused by:
  could not parse TOML configuration in `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  expected an equals, found an identifier at line 1
",
        ),
    );
}

#[test]
fn cargo_platform_specific_dependency() {
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "build.rs"

            [target.{host}.dependencies]
            dep = {{ path = "dep" }}
            [target.{host}.build-dependencies]
            build = {{ path = "build" }}
            [target.{host}.dev-dependencies]
            dev = {{ path = "dev" }}
        "#,
                host = host
            ),
        )
        .file("src/main.rs", "extern crate dep; fn main() { dep::dep() }")
        .file("tests/foo.rs", "extern crate dev; #[test] fn foo() { dev::dev() }")
        .file("build.rs", "extern crate build; fn main() { build::build(); }")
        .file("dep/Cargo.toml", &basic_manifest("dep", "0.5.0"))
        .file("dep/src/lib.rs", "pub fn dep() {}")
        .file("build/Cargo.toml", &basic_manifest("build", "0.5.0"))
        .file("build/src/lib.rs", "pub fn build() {}")
        .file("dev/Cargo.toml", &basic_manifest("dev", "0.5.0"))
        .file("dev/src/lib.rs", "pub fn dev() {}")
        .build();

    assert_that(p.cargo("build"), execs());

    assert_that(&p.bin("foo"), existing_file());
    assert_that(p.cargo("test"), execs());
}

#[test]
fn bad_platform_specific_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [target.wrong-target.dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("bar/src/lib.rs", r#"extern crate baz; pub fn gimme() -> String { format!("") }"#)
        .build();

    assert_that(p.cargo("build"), execs().with_status(101));
}

#[test]
fn cargo_platform_specific_dependency_wrong_platform() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [target.non-existing-triplet.dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("bar/src/lib.rs", "invalid rust file, should not be compiled")
        .build();

    p.cargo("build").exec_with_output().unwrap();

    assert_that(&p.bin("foo"), existing_file());
    assert_that(process(&p.bin("foo")), execs());

    let loc = p.root().join("Cargo.lock");
    let mut lockfile = String::new();
    File::open(&loc)
        .unwrap()
        .read_to_string(&mut lockfile)
        .unwrap();
    assert!(lockfile.contains("bar"))
}

#[test]
fn example_as_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["lib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    assert_that(p.cargo("build --example=ex"), execs());
    assert_that(&p.example_lib("ex", "lib"), existing_file());
}

#[test]
fn example_as_rlib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["rlib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    assert_that(p.cargo("build --example=ex"), execs());
    assert_that(&p.example_lib("ex", "rlib"), existing_file());
}

#[test]
fn example_as_dylib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["dylib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    assert_that(p.cargo("build --example=ex"), execs());
    assert_that(&p.example_lib("ex", "dylib"), existing_file());
}

#[test]
fn example_as_proc_macro() {
    if !is_nightly() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["proc-macro"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "#![feature(proc_macro)]")
        .build();

    assert_that(p.cargo("build --example=ex"), execs());
    assert_that(&p.example_lib("ex", "proc-macro"), existing_file());
}

#[test]
fn example_bin_same_name() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("examples/foo.rs", "fn main() {}")
        .build();

    p.cargo("test --no-run -v")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("foo"), is_not(existing_file()));
    // We expect a file of the form bin/foo-{metadata_hash}
    assert_that(&p.bin("examples/foo"), existing_file());

    p.cargo("test --no-run -v")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("foo"), is_not(existing_file()));
    // We expect a file of the form bin/foo-{metadata_hash}
    assert_that(&p.bin("examples/foo"), existing_file());
}

#[test]
fn compile_then_delete() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("run -v"), execs());
    assert_that(&p.bin("foo"), existing_file());
    if cfg!(windows) {
        // On windows unlinking immediately after running often fails, so sleep
        sleep_ms(100);
    }
    fs::remove_file(&p.bin("foo")).unwrap();
    assert_that(p.cargo("run -v"), execs());
}

#[test]
fn transitive_dependencies_not_available() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.aaaaa]
            path = "a"
        "#,
        )
        .file(
            "src/main.rs",
            "extern crate bbbbb; extern crate aaaaa; fn main() {}",
        )
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "aaaaa"
            version = "0.0.1"
            authors = []

            [dependencies.bbbbb]
            path = "../b"
        "#,
        )
        .file("a/src/lib.rs", "extern crate bbbbb;")
        .file("b/Cargo.toml", &basic_manifest("bbbbb", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -v"),
        execs()
            .with_status(101)
            .with_stderr_contains("[..] can't find crate for `bbbbb`[..]"),
    );
}

#[test]
fn cyclic_deps_rejected() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.a]
            path = "a"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [dependencies.foo]
            path = ".."
        "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build -v"),
                execs().with_status(101)
                       .with_stderr(
r#"[ERROR] cyclic package dependency: package `a v0.0.1 ([..])` depends on itself. Cycle:
package `a v0.0.1 ([..]a)`
    ... which is depended on by `foo v0.0.1 ([..]foo)`[..]"#));
}

#[test]
fn predictable_filenames() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo"
            crate-type = ["dylib", "rlib"]
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("build -v"), execs());
    assert_that(&p.root().join("target/debug/libfoo.rlib"), existing_file());
    let dylib_name = format!("{}foo{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX);
    assert_that(
        &p.root().join("target/debug").join(dylib_name),
        existing_file(),
    );
}

#[test]
fn dashes_to_underscores() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo-bar", "0.0.1"))
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo_bar; fn main() {}")
        .build();

    assert_that(p.cargo("build -v"), execs());
    assert_that(&p.bin("foo-bar"), existing_file());
}

#[test]
fn dashes_in_crate_name_bad() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo-bar"
        "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo_bar; fn main() {}")
        .build();

    assert_that(p.cargo("build -v"), execs().with_status(101));
}

#[test]
fn rustc_env_var() {
    let p = project()
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -v")
            .env("RUSTC", "rustc-that-does-not-exist"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] could not execute process `rustc-that-does-not-exist -vV` ([..])

Caused by:
[..]
",
        ),
    );
    assert_that(&p.bin("a"), is_not(existing_file()));
}

#[test]
fn filtering() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build --lib"), execs());
    assert_that(&p.bin("a"), is_not(existing_file()));

    assert_that(
        p.cargo("build --bin=a --example=a"),
        execs(),
    );
    assert_that(&p.bin("a"), existing_file());
    assert_that(&p.bin("b"), is_not(existing_file()));
    assert_that(&p.bin("examples/a"), existing_file());
    assert_that(&p.bin("examples/b"), is_not(existing_file()));
}

#[test]
fn filtering_implicit_bins() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build --bins"), execs());
    assert_that(&p.bin("a"), existing_file());
    assert_that(&p.bin("b"), existing_file());
    assert_that(&p.bin("examples/a"), is_not(existing_file()));
    assert_that(&p.bin("examples/b"), is_not(existing_file()));
}

#[test]
fn filtering_implicit_examples() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build --examples"), execs());
    assert_that(&p.bin("a"), is_not(existing_file()));
    assert_that(&p.bin("b"), is_not(existing_file()));
    assert_that(&p.bin("examples/a"), existing_file());
    assert_that(&p.bin("examples/b"), existing_file());
}

#[test]
fn ignore_dotfile() {
    let p = project()
        .file("src/bin/.a.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build"), execs());
}

#[test]
fn ignore_dotdirs() {
    let p = project()
        .file("src/bin/a.rs", "fn main() {}")
        .file(".git/Cargo.toml", "")
        .file(".pc/dummy-fix.patch/Cargo.toml", "")
        .build();

    assert_that(p.cargo("build"), execs());
}

#[test]
fn dotdir_root() {
    let p = ProjectBuilder::new(root().join(".foo"))
        .file("src/bin/a.rs", "fn main() {}")
        .build();
    assert_that(p.cargo("build"), execs());
}

#[test]
fn custom_target_dir_env() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .build();

    let exe_name = format!("foo{}", env::consts::EXE_SUFFIX);

    assert_that(
        p.cargo("build").env("CARGO_TARGET_DIR", "foo/target"),
        execs(),
    );
    assert_that(
        &p.root().join("foo/target/debug").join(&exe_name),
        existing_file(),
    );
    assert_that(
        &p.root().join("target/debug").join(&exe_name),
        is_not(existing_file()),
    );

    assert_that(p.cargo("build"), execs());
    assert_that(
        &p.root().join("foo/target/debug").join(&exe_name),
        existing_file(),
    );
    assert_that(
        &p.root().join("target/debug").join(&exe_name),
        existing_file(),
    );

    fs::create_dir(p.root().join(".cargo")).unwrap();
    File::create(p.root().join(".cargo/config"))
        .unwrap()
        .write_all(
            br#"
        [build]
        target-dir = "foo/target"
    "#,
        )
        .unwrap();
    assert_that(
        p.cargo("build").env("CARGO_TARGET_DIR", "bar/target"),
        execs(),
    );
    assert_that(
        &p.root().join("bar/target/debug").join(&exe_name),
        existing_file(),
    );
    assert_that(
        &p.root().join("foo/target/debug").join(&exe_name),
        existing_file(),
    );
    assert_that(
        &p.root().join("target/debug").join(&exe_name),
        existing_file(),
    );
}

#[test]
fn custom_target_dir_line_parameter() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .build();

    let exe_name = format!("foo{}", env::consts::EXE_SUFFIX);

    assert_that(
        p.cargo("build --target-dir foo/target"),
        execs(),
    );
    assert_that(
        &p.root().join("foo/target/debug").join(&exe_name),
        existing_file(),
    );
    assert_that(
        &p.root().join("target/debug").join(&exe_name),
        is_not(existing_file()),
    );

    assert_that(p.cargo("build"), execs());
    assert_that(
        &p.root().join("foo/target/debug").join(&exe_name),
        existing_file(),
    );
    assert_that(
        &p.root().join("target/debug").join(&exe_name),
        existing_file(),
    );

    fs::create_dir(p.root().join(".cargo")).unwrap();
    File::create(p.root().join(".cargo/config"))
        .unwrap()
        .write_all(
            br#"
        [build]
        target-dir = "foo/target"
    "#,
        )
        .unwrap();
    assert_that(
        p.cargo("build --target-dir bar/target"),
        execs(),
    );
    assert_that(
        &p.root().join("bar/target/debug").join(&exe_name),
        existing_file(),
    );
    assert_that(
        &p.root().join("foo/target/debug").join(&exe_name),
        existing_file(),
    );
    assert_that(
        &p.root().join("target/debug").join(&exe_name),
        existing_file(),
    );

    assert_that(
        p.cargo("build --target-dir foobar/target")
            .env("CARGO_TARGET_DIR", "bar/target"),
        execs(),
    );
    assert_that(
        &p.root().join("foobar/target/debug").join(&exe_name),
        existing_file(),
    );
    assert_that(
        &p.root().join("bar/target/debug").join(&exe_name),
        existing_file(),
    );
    assert_that(
        &p.root().join("foo/target/debug").join(&exe_name),
        existing_file(),
    );
    assert_that(
        &p.root().join("target/debug").join(&exe_name),
        existing_file(),
    );
}

#[test]
fn build_multiple_packages() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.d1]
                path = "d1"
            [dependencies.d2]
                path = "d2"

            [[bin]]
                name = "foo"
        "#,
        )
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("d1/Cargo.toml", &basic_bin_manifest("d1"))
        .file("d1/src/lib.rs", "")
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .file(
            "d2/Cargo.toml",
            r#"
            [package]
            name = "d2"
            version = "0.0.1"
            authors = []

            [[bin]]
                name = "d2"
                doctest = false
        "#,
        )
        .file("d2/src/main.rs", "fn main() { println!(\"d2\"); }")
        .build();

    assert_that(p.cargo("build -p d1 -p d2 -p foo"), execs());

    assert_that(&p.bin("foo"), existing_file());
    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("i am foo\n"),
    );

    let d1_path = &p.build_dir()
        .join("debug")
        .join(format!("d1{}", env::consts::EXE_SUFFIX));
    let d2_path = &p.build_dir()
        .join("debug")
        .join(format!("d2{}", env::consts::EXE_SUFFIX));

    assert_that(d1_path, existing_file());
    assert_that(process(d1_path), execs().with_stdout("d1"));

    assert_that(d2_path, existing_file());
    assert_that(process(d2_path), execs().with_stdout("d2"));
}

#[test]
fn invalid_spec() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.d1]
                path = "d1"

            [[bin]]
                name = "foo"
        "#,
        )
        .file("src/bin/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("d1/Cargo.toml", &basic_bin_manifest("d1"))
        .file("d1/src/lib.rs", "")
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .build();

    assert_that(
        p.cargo("build -p notAValidDep"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] package id specification `notAValidDep` matched no packages"),
    );

    assert_that(
        p.cargo("build -p d1 -p notAValidDep"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] package id specification `notAValidDep` matched no packages"),
    );
}

#[test]
fn manifest_with_bom_is_ok() {
    let p = project()
        .file(
            "Cargo.toml",
            "\u{FEFF}
            [package]
            name = \"foo\"
            version = \"0.0.1\"
            authors = []
        ",
        )
        .file("src/lib.rs", "")
        .build();
    assert_that(p.cargo("build -v"), execs());
}

#[test]
fn panic_abort_compiles_with_panic_abort() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [profile.dev]
            panic = 'abort'
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    assert_that(
        p.cargo("build -v"),
        execs()
            .with_stderr_contains("[..] -C panic=abort [..]"),
    );
}

#[test]
fn explicit_color_config_is_propagated_to_rustc() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("test", "0.0.0"))
        .file("src/lib.rs", "")
        .build();
    assert_that(
        p.cargo("build -v --color always"),
        execs().with_stderr_contains("[..]rustc [..] src/lib.rs --color always[..]"),
    );

    assert_that(p.cargo("clean"), execs());

    assert_that(
        p.cargo("build -v --color never"),
        execs().with_stderr(
            "\
[COMPILING] test v0.0.0 ([..])
[RUNNING] `rustc [..] --color never [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn compiler_json_error_format() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("build.rs", "fn main() { println!(\"cargo:rustc-cfg=xyz\") }")
        .file("src/main.rs", "fn main() { let unused = 92; }")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("bar/src/lib.rs", r#"fn dead() {}"#)
        .build();

    // Using jobs=1 to ensure that the order of messages is consistent.
    assert_that(
        p.cargo("build -v --message-format=json --jobs=1"),
        execs().with_json(
            r#"
    {
        "reason":"compiler-artifact",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["custom-build"],
            "crate_types":["bin"],
            "edition": "2015",
            "name":"build-script-build",
            "src_path":"[..]build.rs"
        },
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "0",
            "overflow_checks": true,
            "test": false
        },
        "features": [],
        "filenames": "{...}",
        "fresh": false
    }

    {
        "reason":"compiler-message",
        "package_id":"bar 0.5.0 ([..])",
        "target":{
            "kind":["lib"],
            "crate_types":["lib"],
            "edition": "2015",
            "name":"bar",
            "src_path":"[..]lib.rs"
        },
        "message":"{...}"
    }

    {
        "reason":"compiler-artifact",
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "0",
            "overflow_checks": true,
            "test": false
        },
        "features": [],
        "package_id":"bar 0.5.0 ([..])",
        "target":{
            "kind":["lib"],
            "crate_types":["lib"],
            "edition": "2015",
            "name":"bar",
            "src_path":"[..]lib.rs"
        },
        "filenames":["[..].rlib"],
        "fresh": false
    }

    {
        "reason":"build-script-executed",
        "package_id":"foo 0.5.0 ([..])",
        "linked_libs":[],
        "linked_paths":[],
        "env":[],
        "cfgs":["xyz"]
    }

    {
        "reason":"compiler-message",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["bin"],
            "crate_types":["bin"],
            "edition": "2015",
            "name":"foo",
            "src_path":"[..]main.rs"
        },
        "message":"{...}"
    }

    {
        "reason":"compiler-artifact",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["bin"],
            "crate_types":["bin"],
            "edition": "2015",
            "name":"foo",
            "src_path":"[..]main.rs"
        },
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "0",
            "overflow_checks": true,
            "test": false
        },
        "features": [],
        "filenames": "{...}",
        "fresh": false
    }
"#,
        ),
    );

    // With fresh build, we should repeat the artifacts,
    // but omit compiler warnings.
    assert_that(
        p.cargo("build -v --message-format=json --jobs=1"),
        execs().with_json(
            r#"
    {
        "reason":"compiler-artifact",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["custom-build"],
            "crate_types":["bin"],
            "edition": "2015",
            "name":"build-script-build",
            "src_path":"[..]build.rs"
        },
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "0",
            "overflow_checks": true,
            "test": false
        },
        "features": [],
        "filenames": "{...}",
        "fresh": true
    }

    {
        "reason":"compiler-artifact",
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "0",
            "overflow_checks": true,
            "test": false
        },
        "features": [],
        "package_id":"bar 0.5.0 ([..])",
        "target":{
            "kind":["lib"],
            "crate_types":["lib"],
            "edition": "2015",
            "name":"bar",
            "src_path":"[..]lib.rs"
        },
        "filenames":["[..].rlib"],
        "fresh": true
    }

    {
        "reason":"build-script-executed",
        "package_id":"foo 0.5.0 ([..])",
        "linked_libs":[],
        "linked_paths":[],
        "env":[],
        "cfgs":["xyz"]
    }

    {
        "reason":"compiler-artifact",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["bin"],
            "crate_types":["bin"],
            "edition": "2015",
            "name":"foo",
            "src_path":"[..]main.rs"
        },
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "0",
            "overflow_checks": true,
            "test": false
        },
        "features": [],
        "filenames": "{...}",
        "fresh": true
    }
"#,
        ),
    );
}

#[test]
fn wrong_message_format_option() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build --message-format XML"),
        execs().with_status(1).with_stderr_contains(
            "\
error: 'XML' isn't a valid value for '--message-format <FMT>'
<tab>[possible values: human, json, short]
",
        ),
    );
}

#[test]
fn message_format_json_forward_stderr() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() { let unused = 0; }")
        .build();

    assert_that(
        p.cargo("rustc --release --bin foo --message-format JSON"),
        execs().with_json(
            r#"
    {
        "reason":"compiler-message",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["bin"],
            "crate_types":["bin"],
            "edition": "2015",
            "name":"foo",
            "src_path":"[..]"
        },
        "message":"{...}"
    }

    {
        "reason":"compiler-artifact",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["bin"],
            "crate_types":["bin"],
            "edition": "2015",
            "name":"foo",
            "src_path":"[..]"
        },
        "profile":{
            "debug_assertions":false,
            "debuginfo":null,
            "opt_level":"3",
            "overflow_checks": false,
            "test":false
        },
        "features":[],
        "filenames": "{...}",
        "fresh": false
    }
"#,
        ),
    );
}

#[test]
fn no_warn_about_package_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [package.metadata]
            foo = "bar"
            a = true
            b = 3

            [package.metadata.another]
            bar = 3
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "[..] foo v0.0.1 ([..])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
        ),
    );
}

#[test]
fn cargo_build_empty_target() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build --target").arg(""),
        execs()
            .with_status(101)
            .with_stderr_contains("[..] target was empty"),
    );
}

#[test]
fn build_all_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bar = { path = "bar" }

            [workspace]
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    assert_that(
        p.cargo("build --all"),
        execs().with_stderr(
            "[..] Compiling bar v0.1.0 ([..])\n\
             [..] Compiling foo v0.1.0 ([..])\n\
             [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
        ),
    );
}

#[test]
fn build_all_exclude() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [workspace]
            members = ["bar", "baz"]
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    assert_that(
        p.cargo("build --all --exclude baz"),
        execs()
            .with_stderr_contains("[..]Compiling foo v0.1.0 [..]")
            .with_stderr_contains("[..]Compiling bar v0.1.0 [..]")
            .with_stderr_does_not_contain("[..]Compiling baz v0.1.0 [..]"),
    );
}

#[test]
fn build_all_workspace_implicit_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bar = { path = "bar" }

            [workspace]
        "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/c.rs", "fn main() {}")
        .file("examples/d.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .file("bar/src/bin/e.rs", "fn main() {}")
        .file("bar/src/bin/f.rs", "fn main() {}")
        .file("bar/examples/g.rs", "fn main() {}")
        .file("bar/examples/h.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build --all --examples"),
        execs().with_stderr(
            "[..] Compiling bar v0.1.0 ([..])\n\
             [..] Compiling foo v0.1.0 ([..])\n\
             [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
        ),
    );
    assert_that(&p.bin("a"), is_not(existing_file()));
    assert_that(&p.bin("b"), is_not(existing_file()));
    assert_that(&p.bin("examples/c"), existing_file());
    assert_that(&p.bin("examples/d"), existing_file());
    assert_that(&p.bin("e"), is_not(existing_file()));
    assert_that(&p.bin("f"), is_not(existing_file()));
    assert_that(&p.bin("examples/g"), existing_file());
    assert_that(&p.bin("examples/h"), existing_file());
}

#[test]
fn build_all_virtual_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar", "baz"]
        "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    // The order in which bar and baz are built is not guaranteed
    assert_that(
        p.cargo("build --all"),
        execs()
            .with_stderr_contains("[..] Compiling baz v0.1.0 ([..])")
            .with_stderr_contains("[..] Compiling bar v0.1.0 ([..])")
            .with_stderr(
                "[..] Compiling [..] v0.1.0 ([..])\n\
                 [..] Compiling [..] v0.1.0 ([..])\n\
                 [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
            ),
    );
}

#[test]
fn build_virtual_manifest_all_implied() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar", "baz"]
        "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    // The order in which bar and baz are built is not guaranteed
    assert_that(
        p.cargo("build"),
        execs()
            .with_stderr_contains("[..] Compiling baz v0.1.0 ([..])")
            .with_stderr_contains("[..] Compiling bar v0.1.0 ([..])")
            .with_stderr(
                "[..] Compiling [..] v0.1.0 ([..])\n\
                 [..] Compiling [..] v0.1.0 ([..])\n\
                 [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
            ),
    );
}

#[test]
fn build_virtual_manifest_one_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar", "baz"]
        "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file( "bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file( "baz/src/lib.rs", "pub fn baz() {}")
        .build();

    assert_that(
        p.cargo("build -p bar"),
        execs()
            .with_stderr_does_not_contain("[..]baz[..]")
            .with_stderr_contains("[..] Compiling bar v0.1.0 ([..])")
            .with_stderr(
                "[..] Compiling [..] v0.1.0 ([..])\n\
                 [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
            ),
    );
}

#[test]
fn build_all_virtual_manifest_implicit_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar", "baz"]
        "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .file("bar/src/bin/a.rs", "fn main() {}")
        .file("bar/src/bin/b.rs", "fn main() {}")
        .file("bar/examples/c.rs", "fn main() {}")
        .file("bar/examples/d.rs", "fn main() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "")
        .file("baz/src/bin/e.rs", "fn main() {}")
        .file("baz/src/bin/f.rs", "fn main() {}")
        .file("baz/examples/g.rs", "fn main() {}")
        .file("baz/examples/h.rs", "fn main() {}")
        .build();

    // The order in which bar and baz are built is not guaranteed
    assert_that(
        p.cargo("build --all --examples"),
        execs()
            .with_stderr_contains("[..] Compiling baz v0.1.0 ([..])")
            .with_stderr_contains("[..] Compiling bar v0.1.0 ([..])")
            .with_stderr(
                "[..] Compiling [..] v0.1.0 ([..])\n\
                 [..] Compiling [..] v0.1.0 ([..])\n\
                 [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
            ),
    );
    assert_that(&p.bin("a"), is_not(existing_file()));
    assert_that(&p.bin("b"), is_not(existing_file()));
    assert_that(&p.bin("examples/c"), existing_file());
    assert_that(&p.bin("examples/d"), existing_file());
    assert_that(&p.bin("e"), is_not(existing_file()));
    assert_that(&p.bin("f"), is_not(existing_file()));
    assert_that(&p.bin("examples/g"), existing_file());
    assert_that(&p.bin("examples/h"), existing_file());
}

#[test]
fn build_all_member_dependency_same_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a"]
        "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            [project]
            name = "a"
            version = "0.1.0"

            [dependencies]
            a = "0.1.0"
        "#,
        )
        .file("a/src/lib.rs", "pub fn a() {}")
        .build();

    Package::new("a", "0.1.0").publish();

    assert_that(
        p.cargo("build --all"),
        execs().with_stderr(
            "[..] Updating registry `[..]`\n\
             [..] Downloading a v0.1.0 ([..])\n\
             [..] Compiling a v0.1.0\n\
             [..] Compiling a v0.1.0 ([..])\n\
             [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
        ),
    );
}

#[test]
fn run_proper_binary() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
            [[bin]]
            name = "main"
            [[bin]]
            name = "other"
        "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", r#"fn main() { panic!("This should never be run."); }"#)
        .file("src/bin/other.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("run --bin other"),
        execs(),
    );
}

#[test]
fn run_proper_binary_main_rs() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("run --bin foo"),
        execs(),
    );
}

#[test]
fn run_proper_alias_binary_from_src() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
            [[bin]]
            name = "foo"
            [[bin]]
            name = "bar"
        "#,
        )
        .file("src/foo.rs", r#"fn main() { println!("foo"); }"#)
        .file("src/bar.rs", r#"fn main() { println!("bar"); }"#)
        .build();

    assert_that(p.cargo("build --all"), execs());
    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("foo\n"),
    );
    assert_that(
        process(&p.bin("bar")),
        execs().with_stdout("bar\n"),
    );
}

#[test]
fn run_proper_alias_binary_main_rs() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
            [[bin]]
            name = "foo"
            [[bin]]
            name = "bar"
        "#,
        )
        .file("src/main.rs", r#"fn main() { println!("main"); }"#)
        .build();

    assert_that(p.cargo("build --all"), execs());
    assert_that(
        process(&p.bin("foo")),
        execs().with_stdout("main\n"),
    );
    assert_that(
        process(&p.bin("bar")),
        execs().with_stdout("main\n"),
    );
}

#[test]
fn run_proper_binary_main_rs_as_foo() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#" fn main() { panic!("This should never be run."); }"#)
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("run --bin foo"),
        execs(),
    );
}

#[test]
fn rustc_wrapper() {
    // We don't have /usr/bin/env on Windows.
    if cfg!(windows) {
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    assert_that(
        p.cargo("build -v")
            .env("RUSTC_WRAPPER", "/usr/bin/env"),
        execs().with_stderr_contains("[RUNNING] `/usr/bin/env rustc --crate-name foo [..]"),
    );
}

#[test]
fn cdylib_not_lifted() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            authors = []
            version = "0.1.0"

            [lib]
            crate-type = ["cdylib"]
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs());

    let files = if cfg!(windows) {
        vec!["foo.dll.lib", "foo.dll.exp", "foo.dll"]
    } else if cfg!(target_os = "macos") {
        vec!["libfoo.dylib"]
    } else {
        vec!["libfoo.so"]
    };

    for file in files {
        println!("checking: {}", file);
        assert_that(
            &p.root().join("target/debug/deps").join(&file),
            existing_file(),
        );
    }
}

#[test]
fn cdylib_final_outputs() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo-bar"
            authors = []
            version = "0.1.0"

            [lib]
            crate-type = ["cdylib"]
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs());

    let files = if cfg!(windows) {
        vec!["foo_bar.dll.lib", "foo_bar.dll"]
    } else if cfg!(target_os = "macos") {
        vec!["libfoo_bar.dylib"]
    } else {
        vec!["libfoo_bar.so"]
    };

    for file in files {
        println!("checking: {}", file);
        assert_that(&p.root().join("target/debug").join(&file), existing_file());
    }
}

#[test]
fn deterministic_cfg_flags() {
    // This bug is non-deterministic

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.1.0"
            authors = []
            build = "build.rs"

            [features]
            default = ["f_a", "f_b", "f_c", "f_d"]
            f_a = []
            f_b = []
            f_c = []
            f_d = []
        "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-cfg=cfg_a");
                    println!("cargo:rustc-cfg=cfg_b");
                    println!("cargo:rustc-cfg=cfg_c");
                    println!("cargo:rustc-cfg=cfg_d");
                    println!("cargo:rustc-cfg=cfg_e");
                }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build -v"),
        execs().with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] [..]
[RUNNING] [..]
[RUNNING] `rustc --crate-name foo [..] \
--cfg[..]default[..]--cfg[..]f_a[..]--cfg[..]f_b[..]\
--cfg[..]f_c[..]--cfg[..]f_d[..] \
--cfg cfg_a --cfg cfg_b --cfg cfg_c --cfg cfg_d --cfg cfg_e`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        ),
    );
}

#[test]
fn explicit_bins_without_paths() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [[bin]]
            name = "foo"

            [[bin]]
            name = "bar"
        "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build"), execs());
}

#[test]
fn no_bin_in_src_with_lib() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/foo.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr_contains(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  can't find `foo` bin, specify bin.path",
        ),
    );
}

#[test]
fn inferred_bins() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .file("src/bin/baz/main.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("bar"), existing_file());
    assert_that(&p.bin("baz"), existing_file());
}

#[test]
fn inferred_bins_duplicate_name() {
    // this should fail, because we have two binaries with the same name
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .file("src/bin/bar/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr_contains(
            "[..]found duplicate binary name bar, but all binary targets must have a unique name[..]",
        ),
    );
}

#[test]
fn inferred_bin_path() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "foo"
        version = "0.1.0"
        authors = []

        [[bin]]
        name = "bar"
        # Note, no `path` key!
        "#,
        )
        .file("src/bin/bar/main.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(&p.bin("bar"), existing_file());
}

#[test]
fn inferred_examples() {
    let p = project()
        .file("src/lib.rs", "fn main() {}")
        .file("examples/bar.rs", "fn main() {}")
        .file("examples/baz/main.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("test"), execs());
    assert_that(&p.bin("examples/bar"), existing_file());
    assert_that(&p.bin("examples/baz"), existing_file());
}

#[test]
fn inferred_tests() {
    let p = project()
        .file("src/lib.rs", "fn main() {}")
        .file("tests/bar.rs", "fn main() {}")
        .file("tests/baz/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("test --test=bar --test=baz"),
        execs(),
    );
}

#[test]
fn inferred_benchmarks() {
    let p = project()
        .file("src/lib.rs", "fn main() {}")
        .file("benches/bar.rs", "fn main() {}")
        .file("benches/baz/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("bench --bench=bar --bench=baz"),
        execs(),
    );
}

#[test]
fn target_edition() {
    if !is_nightly() { // --edition is nightly-only
        return;
    }
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["edition"]
            [package]
            name = "foo"
            version = "0.0.1"

            [lib]
            edition = "2018"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -v").masquerade_as_nightly_cargo(),
        execs()
            .with_stderr_contains("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..]--edition=2018 [..]
"),
    );
}

#[test]
fn target_edition_override() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["edition"]
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            edition = "2018"

            [lib]
            edition = "2015"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -v").masquerade_as_nightly_cargo(),
        execs().with_stderr_contains("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..]--edition=2015 [..]
"),
    );
}

#[test]
fn target_edition_feature_gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            edition = "2018"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -v").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  editions are unstable

Caused by:
  feature `edition` is required

consider adding `cargo-features = [\"edition\"]` to the manifest
"
        ),
    );
}

#[test]
fn same_metadata_different_directory() {
    // A top-level crate built in two different workspaces should have the
    // same metadata hash.
    let p = project().at("foo1")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();
    let output = t!(String::from_utf8(
        t!(p.cargo("build -v").exec_with_output()).stderr,
    ));
    let metadata = output
        .split_whitespace()
        .find(|arg| arg.starts_with("metadata="))
        .unwrap();

    let p = project().at("foo2")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    assert_that(
        p.cargo("build -v"),
        execs()
            .with_stderr_contains(format!("[..]{}[..]", metadata)),
    );
}

#[test]
fn building_a_dependent_crate_witout_bin_should_fail() {
    Package::new("testless", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "testless"
            version = "0.1.0"

            [[bin]]
            name = "a_bin"
        "#,
        )
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            testless = "0.1.0"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(101)
            .with_stderr_contains("[..]can't find `a_bin` bin, specify bin.path"),
    );
}

#[test]
fn uplift_dsym_of_bin_on_mac() {
    if !cfg!(any(target_os = "macos", target_os = "ios")) {
        return;
    }
    let p = project()
        .file("src/main.rs", "fn main() { panic!(); }")
        .file("src/bin/b.rs", "fn main() { panic!(); }")
        .file("examples/c.rs", "fn main() { panic!(); }")
        .file("tests/d.rs", "fn main() { panic!(); }")
        .build();

    assert_that(
        p.cargo("build --bins --examples --tests"),
        execs(),
    );
    assert_that(&p.bin("foo.dSYM"), existing_dir());
    assert_that(&p.bin("b.dSYM"), existing_dir());
    assert!(
        p.bin("b.dSYM")
            .symlink_metadata()
            .expect("read metadata from b.dSYM")
            .file_type()
            .is_symlink()
    );
    assert_that(&p.bin("c.dSYM"), is_not(existing_dir()));
    assert_that(&p.bin("d.dSYM"), is_not(existing_dir()));
}

#[test]
fn uplift_pdb_of_bin_on_windows() {
    if !cfg!(all(target_os = "windows", target_env = "msvc")) {
        return;
    }
    let p = project()
        .file("src/main.rs", "fn main() { panic!(); }")
        .file("src/bin/b.rs", "fn main() { panic!(); }")
        .file("examples/c.rs", "fn main() { panic!(); }")
        .file("tests/d.rs", "fn main() { panic!(); }")
        .build();

    assert_that(
        p.cargo("build --bins --examples --tests"),
        execs(),
    );
    assert_that(&p.target_debug_dir().join("foo.pdb"), existing_file());
    assert_that(&p.target_debug_dir().join("b.pdb"), existing_file());
    assert_that(&p.target_debug_dir().join("c.pdb"), is_not(existing_file()));
    assert_that(&p.target_debug_dir().join("d.pdb"), is_not(existing_file()));
}

// Make sure that `cargo build` chooses the correct profile for building
// targets based on filters (assuming --profile is not specified).
#[test]
fn build_filter_infer_profile() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/t1.rs", "")
        .file("benches/b1.rs", "")
        .file("examples/ex1.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build -v"),
        execs()
            .with_stderr_contains(
                "\
                 [RUNNING] `rustc --crate-name foo src/lib.rs --crate-type lib \
                 --emit=dep-info,link[..]",
            )
            .with_stderr_contains(
                "\
                 [RUNNING] `rustc --crate-name foo src/main.rs --crate-type bin \
                 --emit=dep-info,link[..]",
            ),
    );

    p.root().join("target").rm_rf();
    assert_that(
        p.cargo("build -v --test=t1"),
        execs()
            .with_stderr_contains(
                "\
                 [RUNNING] `rustc --crate-name foo src/lib.rs --crate-type lib \
                 --emit=dep-info,link[..]",
            )
            .with_stderr_contains(
                "[RUNNING] `rustc --crate-name t1 tests/t1.rs --emit=dep-info,link[..]",
            )
            .with_stderr_contains(
                "\
                 [RUNNING] `rustc --crate-name foo src/main.rs --crate-type bin \
                 --emit=dep-info,link[..]",
            ),
    );

    p.root().join("target").rm_rf();
    assert_that(
        p.cargo("build -v --bench=b1"),
        execs()
            .with_stderr_contains(
                "\
                 [RUNNING] `rustc --crate-name foo src/lib.rs --crate-type lib \
                 --emit=dep-info,link[..]",
            )
            .with_stderr_contains(
                "\
                 [RUNNING] `rustc --crate-name b1 benches/b1.rs --emit=dep-info,link \
                 -C opt-level=3[..]",
            )
            .with_stderr_contains(
                "\
                 [RUNNING] `rustc --crate-name foo src/main.rs --crate-type bin \
                 --emit=dep-info,link[..]",
            ),
    );
}

#[test]
fn targets_selected_default() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .build();
    assert_that(
        p.cargo("build -v"),
        execs()
        // bin
        .with_stderr_contains("\
            [RUNNING] `rustc --crate-name foo src/main.rs --crate-type bin \
            --emit=dep-info,link[..]")
        // bench
        .with_stderr_does_not_contain("\
            [RUNNING] `rustc --crate-name foo src/main.rs --emit=dep-info,link \
            -C opt-level=3 --test [..]")
        // unit test
        .with_stderr_does_not_contain("\
            [RUNNING] `rustc --crate-name foo src/main.rs --emit=dep-info,link \
            -C debuginfo=2 --test [..]"),
    );
}

#[test]
fn targets_selected_all() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .build();
    assert_that(
        p.cargo("build -v --all-targets"),
        execs()
        // bin
        .with_stderr_contains("\
            [RUNNING] `rustc --crate-name foo src/main.rs --crate-type bin \
            --emit=dep-info,link[..]")
        // bench
        .with_stderr_contains("\
            [RUNNING] `rustc --crate-name foo src/main.rs --emit=dep-info,link \
            -C opt-level=3 --test [..]")
        // unit test
        .with_stderr_contains("\
            [RUNNING] `rustc --crate-name foo src/main.rs --emit=dep-info,link \
            -C debuginfo=2 --test [..]"),
    );
}

#[test]
fn all_targets_no_lib() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .build();
    assert_that(
        p.cargo("build -v --all-targets"),
        execs()
        // bin
        .with_stderr_contains("\
            [RUNNING] `rustc --crate-name foo src/main.rs --crate-type bin \
            --emit=dep-info,link[..]")
        // bench
        .with_stderr_contains("\
            [RUNNING] `rustc --crate-name foo src/main.rs --emit=dep-info,link \
            -C opt-level=3 --test [..]")
        // unit test
        .with_stderr_contains("\
            [RUNNING] `rustc --crate-name foo src/main.rs --emit=dep-info,link \
            -C debuginfo=2 --test [..]"),
    );
}

#[test]
fn no_linkable_target() {
    // Issue 3169. This is currently not an error as per discussion in PR #4797
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            [dependencies]
            the_lib = { path = "the_lib" }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "the_lib/Cargo.toml",
            r#"
            [package]
            name = "the_lib"
            version = "0.1.0"
            [lib]
            name = "the_lib"
            crate-type = ["staticlib"]
        "#,
        )
        .file("the_lib/src/lib.rs", "pub fn foo() {}")
        .build();
    assert_that(
        p.cargo("build"),
        execs().with_stderr_contains(
            "\
             [WARNING] The package `the_lib` provides no linkable [..] \
             while compiling `foo`. [..] in `the_lib`'s Cargo.toml. [..]",
        ),
    );
}

#[test]
fn avoid_dev_deps() {
    Package::new("foo", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []

            [dev-dependencies]
            baz = "1.0.0"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build"), execs().with_status(101));
    assert_that(
        p.cargo("build -Zavoid-dev-deps")
            .masquerade_as_nightly_cargo(),
        execs(),
    );
}

#[test]
fn invalid_jobs() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    assert_that(
        p.cargo("build --jobs over9000"),
        execs()
            .with_status(1)
            .with_stderr("error: Invalid value: could not parse `over9000` as a number"),
    );
}

#[test]
fn target_filters_workspace() {
    let ws = project()
        .at("ws")
        .file(
            "Cargo.toml",
            r#"
        [workspace]
        members = ["a", "b"]
        "#,
        ).file("a/Cargo.toml", &basic_lib_manifest("a"))
        .file("a/src/lib.rs", "")
        .file("a/examples/ex1.rs", "fn main() {}")
        .file("b/Cargo.toml", &basic_bin_manifest("b"))
        .file("b/src/main.rs", "fn main() {}")
        .file("b/examples/ex1.rs", "fn main() {}")
        .build();

    assert_that(
        ws.cargo("build -v --example ex"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] no example target named `ex`

Did you mean `ex1`?",
        ),
    );

    assert_that(
        ws.cargo("build -v --lib"),
        execs()
            .with_status(0)
            .with_stderr_contains("[RUNNING] `rustc [..]a/src/lib.rs[..]"),
    );

    assert_that(
        ws.cargo("build -v --example ex1"),
        execs()
            .with_status(0)
            .with_stderr_contains("[RUNNING] `rustc [..]a/examples/ex1.rs[..]")
            .with_stderr_contains("[RUNNING] `rustc [..]b/examples/ex1.rs[..]"),
    );
}

#[test]
fn target_filters_workspace_not_found() {
    let ws = project()
        .at("ws")
        .file(
            "Cargo.toml",
            r#"
        [workspace]
        members = ["a", "b"]
        "#,
        ).file("a/Cargo.toml", &basic_bin_manifest("a"))
        .file("a/src/main.rs", "fn main() {}")
        .file("b/Cargo.toml", &basic_bin_manifest("b"))
        .file("b/src/main.rs", "fn main() {}")
        .build();

    assert_that(
        ws.cargo("build -v --lib"),
        execs().with_status(101).with_stderr(
            "[ERROR] no library targets found in packages: a, b",
        ),
    );
}
