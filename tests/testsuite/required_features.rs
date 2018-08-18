use support::is_nightly;
use support::install::{cargo_home, has_installed_exe};
use support::{execs, project};
use support::hamcrest::{assert_that, existing_file, is_not};

#[test]
fn build_bin_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a"]
            a = []

            [[bin]]
            name = "foo"
            required-features = ["a"]
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            extern crate foo;

            #[cfg(feature = "a")]
            fn test() {
                foo::foo();
            }

            fn main() {}
        "#,
        )
        .file("src/lib.rs", r#"#[cfg(feature = "a")] pub fn foo() {}"#)
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        p.cargo("build --no-default-features"),
        execs(),
    );

    assert_that(p.cargo("build --bin=foo"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        p.cargo("build --bin=foo --no-default-features"),
        execs().with_status(101).with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing e.g. `--features=\"a\"`
",
        ),
    );
}

#[test]
fn build_bin_arg_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            a = []

            [[bin]]
            name = "foo"
            required-features = ["a"]
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build --features a"),
        execs(),
    );
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn build_bin_multiple_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a", "b"]
            a = []
            b = ["a"]
            c = []

            [[bin]]
            name = "foo_1"
            path = "src/foo_1.rs"
            required-features = ["b", "c"]

            [[bin]]
            name = "foo_2"
            path = "src/foo_2.rs"
            required-features = ["a"]
        "#,
        )
        .file("src/foo_1.rs", "fn main() {}")
        .file("src/foo_2.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build"), execs());

    assert_that(&p.bin("foo_1"), is_not(existing_file()));
    assert_that(&p.bin("foo_2"), existing_file());

    assert_that(
        p.cargo("build --features c"),
        execs(),
    );

    assert_that(&p.bin("foo_1"), existing_file());
    assert_that(&p.bin("foo_2"), existing_file());

    assert_that(
        p.cargo("build --no-default-features"),
        execs(),
    );
}

#[test]
fn build_example_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a"]
            a = []

            [[example]]
            name = "foo"
            required-features = ["a"]
        "#,
        )
        .file("examples/foo.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build --example=foo"),
        execs(),
    );
    assert_that(&p.bin("examples/foo"), existing_file());

    assert_that(
        p.cargo("build --example=foo --no-default-features"),
        execs().with_status(101).with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing e.g. `--features=\"a\"`
",
        ),
    );
}

#[test]
fn build_example_arg_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            a = []

            [[example]]
            name = "foo"
            required-features = ["a"]
        "#,
        )
        .file("examples/foo.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build --example=foo --features a"),
        execs(),
    );
    assert_that(&p.bin("examples/foo"), existing_file());
}

#[test]
fn build_example_multiple_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a", "b"]
            a = []
            b = ["a"]
            c = []

            [[example]]
            name = "foo_1"
            required-features = ["b", "c"]

            [[example]]
            name = "foo_2"
            required-features = ["a"]
        "#,
        )
        .file("examples/foo_1.rs", "fn main() {}")
        .file("examples/foo_2.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build --example=foo_1"),
        execs().with_status(101).with_stderr(
            "\
error: target `foo_1` in package `foo` requires the features: `b`, `c`
Consider enabling them by passing e.g. `--features=\"b c\"`
",
        ),
    );
    assert_that(
        p.cargo("build --example=foo_2"),
        execs(),
    );

    assert_that(&p.bin("examples/foo_1"), is_not(existing_file()));
    assert_that(&p.bin("examples/foo_2"), existing_file());

    assert_that(
        p.cargo("build --example=foo_1 --features c"),
        execs(),
    );
    assert_that(
        p.cargo("build --example=foo_2 --features c"),
        execs(),
    );

    assert_that(&p.bin("examples/foo_1"), existing_file());
    assert_that(&p.bin("examples/foo_2"), existing_file());

    assert_that(
        p.cargo("build --example=foo_1 --no-default-features"),
        execs().with_status(101).with_stderr(
            "\
error: target `foo_1` in package `foo` requires the features: `b`, `c`
Consider enabling them by passing e.g. `--features=\"b c\"`
",
        ),
    );
    assert_that(
        p.cargo("build --example=foo_2 --no-default-features"),
        execs().with_status(101).with_stderr(
            "\
error: target `foo_2` in package `foo` requires the features: `a`
Consider enabling them by passing e.g. `--features=\"a\"`
",
        ),
    );
}

#[test]
fn test_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a"]
            a = []

            [[test]]
            name = "foo"
            required-features = ["a"]
        "#,
        )
        .file("tests/foo.rs", "#[test]\nfn test() {}")
        .build();

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains("test test ... ok"),
    );

    assert_that(
        p.cargo("test --no-default-features"),
        execs()
            .with_stderr(
                "[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"
            )
            .with_stdout(""),
    );

    assert_that(
        p.cargo("test --test=foo"),
        execs()
            .with_stderr(
                "\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]"
            )
            .with_stdout_contains("test test ... ok"),
    );

    assert_that(
        p.cargo("test --test=foo --no-default-features"),
        execs().with_status(101).with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing e.g. `--features=\"a\"`
",
        ),
    );
}

#[test]
fn test_arg_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            a = []

            [[test]]
            name = "foo"
            required-features = ["a"]
        "#,
        )
        .file("tests/foo.rs", "#[test]\nfn test() {}")
        .build();

    assert_that(
        p.cargo("test --features a"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains("test test ... ok"),
    );
}

#[test]
fn test_multiple_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a", "b"]
            a = []
            b = ["a"]
            c = []

            [[test]]
            name = "foo_1"
            required-features = ["b", "c"]

            [[test]]
            name = "foo_2"
            required-features = ["a"]
        "#,
        )
        .file("tests/foo_1.rs", "#[test]\nfn test() {}")
        .file("tests/foo_2.rs", "#[test]\nfn test() {}")
        .build();

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo_2-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains("test test ... ok"),
    );

    assert_that(
        p.cargo("test --features c"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo_1-[..][EXE]
[RUNNING] target/debug/deps/foo_2-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains_n("test test ... ok", 2),
    );

    assert_that(
        p.cargo("test --no-default-features"),
        execs()
            .with_stderr(
                "[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"
            )
            .with_stdout(""),
    );
}

#[test]
fn bench_default_features() {
    if !is_nightly() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a"]
            a = []

            [[bench]]
            name = "foo"
            required-features = ["a"]
        "#,
        )
        .file(
            "benches/foo.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }"#,
        )
        .build();

    assert_that(
        p.cargo("bench"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target/release/deps/foo-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains("test bench ... bench: [..]"),
    );

    assert_that(
        p.cargo("bench --no-default-features"),
        execs()
            .with_stderr("[FINISHED] release [optimized] target(s) in [..]".to_string())
            .with_stdout(""),
    );

    assert_that(
        p.cargo("bench --bench=foo"),
        execs()
            .with_stderr(
                "\
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target/release/deps/foo-[..][EXE]"
            )
            .with_stdout_contains("test bench ... bench: [..]"),
    );

    assert_that(
        p.cargo("bench --bench=foo --no-default-features"),
        execs().with_status(101).with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing e.g. `--features=\"a\"`
",
        ),
    );
}

#[test]
fn bench_arg_features() {
    if !is_nightly() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            a = []

            [[bench]]
            name = "foo"
            required-features = ["a"]
        "#,
        )
        .file(
            "benches/foo.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }"#,
        )
        .build();

    assert_that(
        p.cargo("bench --features a"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target/release/deps/foo-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains("test bench ... bench: [..]"),
    );
}

#[test]
fn bench_multiple_required_features() {
    if !is_nightly() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a", "b"]
            a = []
            b = ["a"]
            c = []

            [[bench]]
            name = "foo_1"
            required-features = ["b", "c"]

            [[bench]]
            name = "foo_2"
            required-features = ["a"]
        "#,
        )
        .file(
            "benches/foo_1.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }"#,
        )
        .file(
            "benches/foo_2.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }"#,
        )
        .build();

    assert_that(
        p.cargo("bench"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target/release/deps/foo_2-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains("test bench ... bench: [..]"),
    );

    assert_that(
        p.cargo("bench --features c"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target/release/deps/foo_1-[..][EXE]
[RUNNING] target/release/deps/foo_2-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains_n("test bench ... bench: [..]", 2),
    );

    assert_that(
        p.cargo("bench --no-default-features"),
        execs()
            .with_stderr("[FINISHED] release [optimized] target(s) in [..]")
            .with_stdout(""),
    );
}

#[test]
fn install_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a"]
            a = []

            [[bin]]
            name = "foo"
            required-features = ["a"]

            [[example]]
            name = "foo"
            required-features = ["a"]
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("examples/foo.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("install --path ."), execs());
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(p.cargo("uninstall foo"), execs());

    assert_that(
        p.cargo("install --path . --no-default-features"),
        execs().with_status(101).with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[FINISHED] release [optimized] target(s) in [..]
[ERROR] no binaries are available for install using the selected features
"
        ),
    );
    assert_that(cargo_home(), is_not(has_installed_exe("foo")));

    assert_that(
        p.cargo("install --path . --bin=foo"),
        execs(),
    );
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(p.cargo("uninstall foo"), execs());

    assert_that(
        p.cargo("install --path . --bin=foo --no-default-features"),
        execs().with_status(101).with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[ERROR] failed to compile `foo v0.0.1 ([..])`, intermediate artifacts can be found at \
    `[..]target`

Caused by:
  target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing e.g. `--features=\"a\"`
"
        ),
    );
    assert_that(cargo_home(), is_not(has_installed_exe("foo")));

    assert_that(
        p.cargo("install --path . --example=foo"),
        execs(),
    );
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(p.cargo("uninstall foo"), execs());

    assert_that(
        p.cargo("install --path . --example=foo --no-default-features"),
        execs().with_status(101).with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[ERROR] failed to compile `foo v0.0.1 ([..])`, intermediate artifacts can be found at \
    `[..]target`

Caused by:
  target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing e.g. `--features=\"a\"`
"
        ),
    );
    assert_that(cargo_home(), is_not(has_installed_exe("foo")));
}

#[test]
fn install_arg_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            a = []

            [[bin]]
            name = "foo"
            required-features = ["a"]
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("install --features a"),
        execs(),
    );
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(p.cargo("uninstall foo"), execs());
}

#[test]
fn install_multiple_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a", "b"]
            a = []
            b = ["a"]
            c = []

            [[bin]]
            name = "foo_1"
            path = "src/foo_1.rs"
            required-features = ["b", "c"]

            [[bin]]
            name = "foo_2"
            path = "src/foo_2.rs"
            required-features = ["a"]
        "#,
        )
        .file("src/foo_1.rs", "fn main() {}")
        .file("src/foo_2.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("install --path ."), execs());
    assert_that(cargo_home(), is_not(has_installed_exe("foo_1")));
    assert_that(cargo_home(), has_installed_exe("foo_2"));
    assert_that(p.cargo("uninstall foo"), execs());

    assert_that(
        p.cargo("install --path . --features c"),
        execs(),
    );
    assert_that(cargo_home(), has_installed_exe("foo_1"));
    assert_that(cargo_home(), has_installed_exe("foo_2"));
    assert_that(p.cargo("uninstall foo"), execs());

    assert_that(
        p.cargo("install --path . --no-default-features"),
        execs().with_status(101).with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[FINISHED] release [optimized] target(s) in [..]
[ERROR] no binaries are available for install using the selected features
",
        ),
    );
    assert_that(cargo_home(), is_not(has_installed_exe("foo_1")));
    assert_that(cargo_home(), is_not(has_installed_exe("foo_2")));
}

#[test]
fn dep_feature_in_toml() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { path = "bar", features = ["a"] }

            [[bin]]
            name = "foo"
            required-features = ["bar/a"]

            [[example]]
            name = "foo"
            required-features = ["bar/a"]

            [[test]]
            name = "foo"
            required-features = ["bar/a"]

            [[bench]]
            name = "foo"
            required-features = ["bar/a"]
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("examples/foo.rs", "fn main() {}")
        .file("tests/foo.rs", "#[test]\nfn test() {}")
        .file(
            "benches/foo.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }"#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [features]
            a = []
        "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs());

    // bin
    assert_that(p.cargo("build --bin=foo"), execs());
    assert_that(&p.bin("foo"), existing_file());

    // example
    assert_that(
        p.cargo("build --example=foo"),
        execs(),
    );
    assert_that(&p.bin("examples/foo"), existing_file());

    // test
    assert_that(
        p.cargo("test --test=foo"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains("test test ... ok"),
    );

    // bench
    if is_nightly() {
        assert_that(
            p.cargo("bench --bench=foo"),
            execs()
                .with_stderr(format!(
                    "\
[COMPILING] bar v0.0.1 ({0}/bar)
[COMPILING] foo v0.0.1 ({0})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target/release/deps/foo-[..][EXE]",
                    p.url()
                ))
                .with_stdout_contains("test bench ... bench: [..]"),
        );
    }

    // install
    assert_that(p.cargo("install"), execs());
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(p.cargo("uninstall foo"), execs());
}

#[test]
fn dep_feature_in_cmd_line() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { path = "bar" }

            [[bin]]
            name = "foo"
            required-features = ["bar/a"]

            [[example]]
            name = "foo"
            required-features = ["bar/a"]

            [[test]]
            name = "foo"
            required-features = ["bar/a"]

            [[bench]]
            name = "foo"
            required-features = ["bar/a"]
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("examples/foo.rs", "fn main() {}")
        .file("tests/foo.rs", "#[test]\nfn test() {}")
        .file(
            "benches/foo.rs",
            r#"
            #![feature(test)]
            extern crate test;

            #[bench]
            fn bench(_: &mut test::Bencher) {
            }"#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [features]
            a = []
        "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs());

    // bin
    assert_that(
        p.cargo("build --bin=foo"),
        execs().with_status(101).with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `bar/a`
Consider enabling them by passing e.g. `--features=\"bar/a\"`
",
        ),
    );

    assert_that(
        p.cargo("build --bin=foo --features bar/a"),
        execs(),
    );
    assert_that(&p.bin("foo"), existing_file());

    // example
    assert_that(
        p.cargo("build --example=foo"),
        execs().with_status(101).with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `bar/a`
Consider enabling them by passing e.g. `--features=\"bar/a\"`
",
        ),
    );

    assert_that(
        p.cargo("build --example=foo --features bar/a"),
        execs(),
    );
    assert_that(&p.bin("examples/foo"), existing_file());

    // test
    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(
                "[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"
            )
            .with_stdout(""),
    );

    assert_that(
        p.cargo("test --test=foo --features bar/a"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains("test test ... ok"),
    );

    // bench
    if is_nightly() {
        assert_that(
            p.cargo("bench"),
            execs()
                .with_stderr("[FINISHED] release [optimized] target(s) in [..]")
                .with_stdout(""),
        );

        assert_that(
            p.cargo("bench --bench=foo --features bar/a"),
            execs()
                .with_stderr(format!(
                    "\
[COMPILING] bar v0.0.1 ({0}/bar)
[COMPILING] foo v0.0.1 ({0})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target/release/deps/foo-[..][EXE]",
                    p.url()
                ))
                .with_stdout_contains("test bench ... bench: [..]"),
        );
    }

    // install
    assert_that(
        p.cargo("install --path ."),
        execs().with_status(101).with_stderr(
            "\
[INSTALLING] foo v0.0.1 ([..])
[FINISHED] release [optimized] target(s) in [..]
[ERROR] no binaries are available for install using the selected features
"
        ),
    );
    assert_that(cargo_home(), is_not(has_installed_exe("foo")));

    assert_that(
        p.cargo("install --features bar/a"),
        execs(),
    );
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(p.cargo("uninstall foo"), execs());
}

#[test]
fn test_skips_compiling_bin_with_missing_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            a = []

            [[bin]]
            name = "bin_foo"
            path = "src/bin/foo.rs"
            required-features = ["a"]
        "#,
        )
        .file("src/bin/foo.rs", "extern crate bar; fn main() {}")
        .file("tests/foo.rs", "")
        .file("benches/foo.rs", "")
        .build();

    assert_that(
        p.cargo("test"),
        execs()
            .with_stderr(format!(
                "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]",
                p.url()
            ))
            .with_stdout_contains("running 0 tests"),
    );

    assert_that(
        p.cargo("test --features a -j 1"),
        execs().with_status(101).with_stderr_contains(format!(
            "\
[COMPILING] foo v0.0.1 ({})
error[E0463]: can't find crate for `bar`",
            p.url()
        )),
    );

    if is_nightly() {
        assert_that(
            p.cargo("bench"),
            execs()
                .with_stderr(format!(
                    "\
[COMPILING] foo v0.0.1 ({})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] target/release/deps/foo-[..][EXE]",
                    p.url()
                ))
                .with_stdout_contains("running 0 tests"),
        );

        assert_that(
            p.cargo("bench --features a -j 1"),
            execs().with_status(101).with_stderr_contains(format!(
                "\
[COMPILING] foo v0.0.1 ({})
error[E0463]: can't find crate for `bar`",
                p.url()
            )),
        );
    }
}

#[test]
fn run_default() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = []
            a = []

            [[bin]]
            name = "foo"
            required-features = ["a"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo; fn main() {}")
        .build();

    assert_that(
        p.cargo("run"),
        execs().with_status(101).with_stderr(
            "\
error: target `foo` in package `foo` requires the features: `a`
Consider enabling them by passing e.g. `--features=\"a\"`
",
        ),
    );

    assert_that(
        p.cargo("run --features a"),
        execs(),
    );
}

#[test]
fn run_default_multiple_required_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a"]
            a = []
            b = []

            [[bin]]
            name = "foo1"
            path = "src/foo1.rs"
            required-features = ["a"]

            [[bin]]
            name = "foo2"
            path = "src/foo2.rs"
            required-features = ["b"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("src/foo1.rs", "extern crate foo; fn main() {}")
        .file("src/foo2.rs", "extern crate foo; fn main() {}")
        .build();

    assert_that(
        p.cargo("run"),
        execs().with_status(101).with_stderr(
            "\
             error: `cargo run` requires that a project only have one executable; \
             use the `--bin` option to specify which one to run\navailable binaries: foo1, foo2",
        ),
    );
}
