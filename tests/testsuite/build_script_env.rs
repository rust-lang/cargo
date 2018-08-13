use std::fs::File;

use support::sleep_ms;
use support::{execs, project};
use support::hamcrest::assert_that;

#[test]
fn rerun_if_env_changes() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo:rerun-if-env-changed=FOO");
            }
        "#,
        )
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        ),
    );
    assert_that(
        p.cargo("build").env("FOO", "bar"),
        execs().with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        ),
    );
    assert_that(
        p.cargo("build").env("FOO", "baz"),
        execs().with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        ),
    );
    assert_that(
        p.cargo("build").env("FOO", "baz"),
        execs().with_stderr("[FINISHED] [..]"),
    );
    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        ),
    );
}

#[test]
fn rerun_if_env_or_file_changes() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo:rerun-if-env-changed=FOO");
                println!("cargo:rerun-if-changed=foo");
            }
        "#,
        )
        .file("foo", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        ),
    );
    assert_that(
        p.cargo("build").env("FOO", "bar"),
        execs().with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        ),
    );
    assert_that(
        p.cargo("build").env("FOO", "bar"),
        execs().with_stderr("[FINISHED] [..]"),
    );
    sleep_ms(1000);
    File::create(p.root().join("foo")).unwrap();
    assert_that(
        p.cargo("build").env("FOO", "bar"),
        execs().with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        ),
    );
}
