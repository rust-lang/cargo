use cargo::util::paths::dylib_path_envvar;
use support::{self, ChannelChanger};
use support::{basic_bin_manifest, basic_lib_manifest, execs, project, Project, path2url};
use support::hamcrest::{assert_that, existing_file};

#[test]
fn simple() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    assert_that(
        p.cargo("run"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/foo[EXE]`",
                dir = path2url(p.root())
            ))
            .with_stdout("hello"),
    );
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn simple_quiet() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    assert_that(
        p.cargo("run -q"),
        execs().with_stdout("hello"),
    );

    assert_that(
        p.cargo("run --quiet"),
        execs().with_stdout("hello"),
    );
}

#[test]
fn simple_quiet_and_verbose() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    assert_that(
        p.cargo("run -q -v"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] cannot set both --verbose and --quiet"),
    );
}

#[test]
fn quiet_and_verbose_config() {
    let p = project()
        .file(
            ".cargo/config",
            r#"
            [term]
            verbose = true
        "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    assert_that(p.cargo("run -q"), execs());
}

#[test]
fn simple_with_args() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
            fn main() {
                assert_eq!(std::env::args().nth(1).unwrap(), "hello");
                assert_eq!(std::env::args().nth(2).unwrap(), "world");
            }
        "#,
        )
        .build();

    assert_that(
        p.cargo("run hello world"),
        execs(),
    );
}

#[test]
fn exit_code() {
    let p = project()
        .file("src/main.rs", "fn main() { std::process::exit(2); }")
        .build();

    let mut output = String::from(
        "\
[COMPILING] foo v0.0.1 (file[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target[..]`
",
    );
    if !cfg!(unix) {
        output.push_str(
            "[ERROR] process didn't exit successfully: `target[..]foo[..]` (exit code: 2)",
        );
    }
    assert_that(p.cargo("run"), execs().with_status(2).with_stderr(output));
}

#[test]
fn exit_code_verbose() {
    let p = project()
        .file("src/main.rs", "fn main() { std::process::exit(2); }")
        .build();

    let mut output = String::from(
        "\
[COMPILING] foo v0.0.1 (file[..])
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target[..]`
",
    );
    if !cfg!(unix) {
        output.push_str(
            "[ERROR] process didn't exit successfully: `target[..]foo[..]` (exit code: 2)",
        );
    }

    assert_that(
        p.cargo("run -v"),
        execs().with_status(2).with_stderr(output),
    );
}

#[test]
fn no_main_file() {
    let p = project()
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("run"),
        execs().with_status(101).with_stderr(
            "[ERROR] a bin target must be available \
             for `cargo run`\n",
        ),
    );
}

#[test]
fn too_many_bins() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "")
        .file("src/bin/b.rs", "")
        .build();

    assert_that(
        p.cargo("run"),
        // Using [..] here because the order is not stable
        execs().with_status(101).with_stderr(
            "[ERROR] `cargo run` requires that a project only \
             have one executable; use the `--bin` option \
             to specify which one to run\navailable binaries: [..]\n",
        ),
    );

    assert_that(
        p.cargo("run").masquerade_as_nightly_cargo(),
        // Using [..] here because the order is not stable
        execs().with_status(101).with_stderr(
            "[ERROR] `cargo run` could not determine which binary to run. \
             Use the `--bin` option to specify a binary, or (on \
             nightly) the `default-run` manifest key.\
             \navailable binaries: [..]\n",
        ),
    );
}

#[test]
fn specify_name() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "src/bin/a.rs",
            r#"
            #[allow(unused_extern_crates)]
            extern crate foo;
            fn main() { println!("hello a.rs"); }
        "#,
        )
        .file(
            "src/bin/b.rs",
            r#"
            #[allow(unused_extern_crates)]
            extern crate foo;
            fn main() { println!("hello b.rs"); }
        "#,
        )
        .build();

    assert_that(
        p.cargo("run --bin a -v"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[RUNNING] `rustc [..] src/lib.rs [..]`
[RUNNING] `rustc [..] src/bin/a.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/a[EXE]`",
                dir = path2url(p.root())
            ))
            .with_stdout("hello a.rs"),
    );

    assert_that(
        p.cargo("run --bin b -v"),
        execs()
            .with_stderr(
                "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] src/bin/b.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/b[EXE]`",
            )
            .with_stdout("hello b.rs"),
    );
}

#[test]
fn specify_default_run() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["default-run"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            default-run = "a"
        "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", r#"fn main() { println!("hello A"); }"#)
        .file("src/bin/b.rs", r#"fn main() { println!("hello B"); }"#)
        .build();

    assert_that(
        p.cargo("run").masquerade_as_nightly_cargo(),
        execs()
            .with_stdout("hello A"),
    );
    assert_that(
        p.cargo("run --bin a").masquerade_as_nightly_cargo(),
        execs()
            .with_stdout("hello A"),
    );
    assert_that(
        p.cargo("run --bin b").masquerade_as_nightly_cargo(),
        execs()
            .with_stdout("hello B"),
    );
}

#[test]
fn bogus_default_run() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["default-run"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            default-run = "b"
        "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", r#"fn main() { println!("hello A"); }"#)
        .build();

    assert_that(
        p.cargo("run").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "error: no bin target named `b`\n\nDid you mean [..]?",
        ),
    );
}

#[test]
fn default_run_unstable() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            default-run = "a"
        "#,
        )
        .file("src/bin/a.rs", r#"fn main() { println!("hello A"); }"#)
        .build();

    assert_that(
        p.cargo("run"),
        execs().with_status(101).with_stderr(
r#"error: failed to parse manifest at [..]

Caused by:
  the `default-run` manifest key is unstable

Caused by:
  feature `default-run` is required

this Cargo does not support nightly features, but if you
switch to nightly channel you can add
`cargo-features = ["default-run"]` to enable this feature
"#,
        ),
    );

    assert_that(
        p.cargo("run").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
r#"error: failed to parse manifest at [..]

Caused by:
  the `default-run` manifest key is unstable

Caused by:
  feature `default-run` is required

consider adding `cargo-features = ["default-run"]` to the manifest
"#,
        ),
    );
}

#[test]
fn run_example() {
    let p = project()
        .file("src/lib.rs", "")
        .file("examples/a.rs", r#"fn main() { println!("example"); }"#)
        .file("src/bin/a.rs", r#"fn main() { println!("bin"); }"#)
        .build();

    assert_that(
        p.cargo("run --example a"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/examples/a[EXE]`",
                dir = path2url(p.root())
            ))
            .with_stdout("example"),
    );
}

#[test]
fn run_library_example() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            [[example]]
            name = "bar"
            crate_type = ["lib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/bar.rs", "fn foo() {}")
        .build();

    assert_that(
        p.cargo("run --example bar"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] example target `bar` is a library and cannot be executed"),
    );
}

fn autodiscover_examples_project(rust_edition: &str, autoexamples: Option<bool>) -> Project {
    let autoexamples = match autoexamples {
        None => "".to_string(),
        Some(bool) => format!("autoexamples = {}", bool),
    };
    project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            cargo-features = ["edition"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            edition = "{rust_edition}"
            {autoexamples}

            [features]
            magic = []

            [[example]]
            name = "do_magic"
            required-features = ["magic"]
        "#,
                rust_edition = rust_edition,
                autoexamples = autoexamples
            ),
        )
        .file("examples/a.rs", r#"fn main() { println!("example"); }"#)
        .file(
            "examples/do_magic.rs",
            r#"
            fn main() { println!("magic example"); }
        "#,
        )
        .build()
}

#[test]
fn run_example_autodiscover_2015() {
    if !support::is_nightly() {
        return;
    }

    let p = autodiscover_examples_project("2015", None);
    assert_that(
        p.cargo("run --example a")
            .masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "warning: \
An explicit [[example]] section is specified in Cargo.toml which currently
disables Cargo from automatically inferring other example targets.
This inference behavior will change in the Rust 2018 edition and the following
files will be included as a example target:

* [..]a.rs

This is likely to break cargo build or cargo test as these files may not be
ready to be compiled as a example target today. You can future-proof yourself
and disable this warning by adding `autoexamples = false` to your [package]
section. You may also move the files to a location where Cargo would not
automatically infer them to be a target, such as in subfolders.

For more information on this warning you can consult
https://github.com/rust-lang/cargo/issues/5330
error: no example target named `a`
",
        ),
    );
}

#[test]
fn run_example_autodiscover_2015_with_autoexamples_enabled() {
    if !support::is_nightly() {
        return;
    }

    let p = autodiscover_examples_project("2015", Some(true));
    assert_that(
        p.cargo("run --example a")
            .masquerade_as_nightly_cargo(),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/examples/a[EXE]`",
                dir = path2url(p.root())
            ))
            .with_stdout("example"),
    );
}

#[test]
fn run_example_autodiscover_2015_with_autoexamples_disabled() {
    if !support::is_nightly() {
        return;
    }

    let p = autodiscover_examples_project("2015", Some(false));
    assert_that(
        p.cargo("run --example a")
            .masquerade_as_nightly_cargo(),
        execs()
            .with_status(101)
            .with_stderr("error: no example target named `a`\n"),
    );
}

#[test]
fn run_example_autodiscover_2018() {
    if !support::is_nightly() {
        return;
    }

    let p = autodiscover_examples_project("2018", None);
    assert_that(
        p.cargo("run --example a")
            .masquerade_as_nightly_cargo(),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/examples/a[EXE]`",
                dir = path2url(p.root())
            ))
            .with_stdout("example"),
    );
}

#[test]
fn run_bins() {
    let p = project()
        .file("src/lib.rs", "")
        .file("examples/a.rs", r#"fn main() { println!("example"); }"#)
        .file("src/bin/a.rs", r#"fn main() { println!("bin"); }"#)
        .build();

    assert_that(
        p.cargo("run --bins"),
        execs().with_status(1).with_stderr_contains(
            "error: Found argument '--bins' which wasn't expected, or isn't valid in this context",
        ),
    );
}

#[test]
fn run_with_filename() {
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
        p.cargo("run --bin bin.rs"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] no bin target named `bin.rs`"),
    );

    assert_that(
        p.cargo("run --bin a.rs"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] no bin target named `a.rs`

Did you mean `a`?",
        ),
    );

    assert_that(
        p.cargo("run --example example.rs"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] no example target named `example.rs`"),
    );

    assert_that(
        p.cargo("run --example a.rs"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] no example target named `a.rs`

Did you mean `a`?",
        ),
    );
}

#[test]
fn either_name_or_example() {
    let p = project()
        .file("src/bin/a.rs", r#"fn main() { println!("hello a.rs"); }"#)
        .file("examples/b.rs", r#"fn main() { println!("hello b.rs"); }"#)
        .build();

    assert_that(
        p.cargo("run --bin a --example b"),
        execs().with_status(101).with_stderr(
            "[ERROR] `cargo run` can run at most one \
             executable, but multiple were \
             specified",
        ),
    );
}

#[test]
fn one_bin_multiple_examples() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", r#"fn main() { println!("hello main.rs"); }"#)
        .file("examples/a.rs", r#"fn main() { println!("hello a.rs"); }"#)
        .file("examples/b.rs", r#"fn main() { println!("hello b.rs"); }"#)
        .build();

    assert_that(
        p.cargo("run"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/main[EXE]`",
                dir = path2url(p.root())
            ))
            .with_stdout("hello main.rs"),
    );
}

#[test]
fn example_with_release_flag() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "*"
            path = "bar"
        "#,
        )
        .file(
            "examples/a.rs",
            r#"
            extern crate bar;

            fn main() {
                if cfg!(debug_assertions) {
                    println!("slow1")
                } else {
                    println!("fast1")
                }
                bar::baz();
            }
        "#,
        )
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file(
            "bar/src/bar.rs",
            r#"
            pub fn baz() {
                if cfg!(debug_assertions) {
                    println!("slow2")
                } else {
                    println!("fast2")
                }
            }
        "#,
        )
        .build();

    assert_that(
        p.cargo("run -v --release --example a"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] bar v0.5.0 ({url}/bar)
[RUNNING] `rustc --crate-name bar bar/src/bar.rs --crate-type lib \
        --emit=dep-info,link \
        -C opt-level=3 \
        -C metadata=[..] \
        --out-dir {dir}/target/release/deps \
        -L dependency={dir}/target/release/deps`
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc --crate-name a examples/a.rs --crate-type bin \
        --emit=dep-info,link \
        -C opt-level=3 \
        -C metadata=[..] \
        --out-dir {dir}/target/release/examples \
        -L dependency={dir}/target/release/deps \
         --extern bar={dir}/target/release/deps/libbar-[..].rlib`
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] `target/release/examples/a[EXE]`
",
                dir = p.root().display(),
                url = path2url(p.root()),
            ))
            .with_stdout(
                "\
fast1
fast2",
            ),
    );

    assert_that(
        p.cargo("run -v --example a"),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] bar v0.5.0 ({url}/bar)
[RUNNING] `rustc --crate-name bar bar/src/bar.rs --crate-type lib \
        --emit=dep-info,link \
        -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir {dir}/target/debug/deps \
        -L dependency={dir}/target/debug/deps`
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc --crate-name a examples/a.rs --crate-type bin \
        --emit=dep-info,link \
        -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir {dir}/target/debug/examples \
        -L dependency={dir}/target/debug/deps \
         --extern bar={dir}/target/debug/deps/libbar-[..].rlib`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/examples/a[EXE]`
",
                dir = p.root().display(),
                url = path2url(p.root()),
            ))
            .with_stdout(
                "\
slow1
slow2",
            ),
    );
}

#[test]
fn run_dylib_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/main.rs", r#"extern crate bar; fn main() { bar::bar(); }"#)
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            crate-type = ["dylib"]
        "#,
        )
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    assert_that(
        p.cargo("run hello world"),
        execs(),
    );
}

#[test]
fn release_works() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
            fn main() { if cfg!(debug_assertions) { panic!() } }
        "#,
        )
        .build();

    assert_that(
        p.cargo("run --release"),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] release [optimized] target(s) in [..]
[RUNNING] `target/release/foo[EXE]`
",
            dir = path2url(p.root()),
        )),
    );
    assert_that(&p.release_bin("foo"), existing_file());
}

#[test]
fn run_bin_different_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[bin]]
            name = "bar"
        "#,
        )
        .file("src/bar.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("run"), execs());
}

#[test]
fn dashes_are_forwarded() {
    let p = project()
        .file(
            "src/bin/bar.rs",
            r#"
            fn main() {
                let s: Vec<String> = std::env::args().collect();
                assert_eq!(s[1], "--");
                assert_eq!(s[2], "a");
                assert_eq!(s[3], "--");
                assert_eq!(s[4], "b");
            }
        "#,
        )
        .build();

    assert_that(
        p.cargo("run -- -- a -- b"),
        execs(),
    );
}

#[test]
fn run_from_executable_folder() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    let cwd = p.root().join("target").join("debug");
    p.cargo("build").exec_with_output().unwrap();

    assert_that(
        p.cargo("run").cwd(cwd),
        execs()
            .with_stderr(
                "\
                 [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n\
                 [RUNNING] `./foo[EXE]`",
            )
            .with_stdout("hello"),
    );
}

#[test]
fn run_with_library_paths() {
    let p = project();

    // Only link search directories within the target output directory are
    // propagated through to dylib_path_envvar() (see #3366).
    let mut dir1 = p.target_debug_dir();
    dir1.push("foo\\backslash");

    let mut dir2 = p.target_debug_dir();
    dir2.push("dir=containing=equal=signs");

    let p = p.file(
        "Cargo.toml",
        r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#,
    ).file(
            "build.rs",
            &format!(
                r##"
            fn main() {{
                println!(r#"cargo:rustc-link-search=native={}"#);
                println!(r#"cargo:rustc-link-search={}"#);
            }}
        "##,
                dir1.display(),
                dir2.display()
            ),
        )
        .file(
            "src/main.rs",
            &format!(
                r##"
            fn main() {{
                let search_path = std::env::var_os("{}").unwrap();
                let paths = std::env::split_paths(&search_path).collect::<Vec<_>>();
                assert!(paths.contains(&r#"{}"#.into()));
                assert!(paths.contains(&r#"{}"#.into()));
            }}
        "##,
                dylib_path_envvar(),
                dir1.display(),
                dir2.display()
            ),
        )
        .build();

    assert_that(p.cargo("run"), execs());
}

#[test]
fn library_paths_sorted_alphabetically() {
    let p = project();

    let mut dir1 = p.target_debug_dir();
    dir1.push("zzzzzzz");

    let mut dir2 = p.target_debug_dir();
    dir2.push("BBBBBBB");

    let mut dir3 = p.target_debug_dir();
    dir3.push("aaaaaaa");

    let p = p.file(
        "Cargo.toml",
        r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#,
    ).file(
            "build.rs",
            &format!(
                r##"
            fn main() {{
                println!(r#"cargo:rustc-link-search=native={}"#);
                println!(r#"cargo:rustc-link-search=native={}"#);
                println!(r#"cargo:rustc-link-search=native={}"#);
            }}
        "##,
                dir1.display(),
                dir2.display(),
                dir3.display()
            ),
        )
        .file(
            "src/main.rs",
            &format!(
                r##"
            fn main() {{
                let search_path = std::env::var_os("{}").unwrap();
                let paths = std::env::split_paths(&search_path).collect::<Vec<_>>();
                // ASCII case-sensitive sort
                assert_eq!("BBBBBBB", paths[0].file_name().unwrap().to_string_lossy());
                assert_eq!("aaaaaaa", paths[1].file_name().unwrap().to_string_lossy());
                assert_eq!("zzzzzzz", paths[2].file_name().unwrap().to_string_lossy());
            }}
        "##,
                dylib_path_envvar()
            ),
        )
        .build();

    assert_that(p.cargo("run"), execs());
}

#[test]
fn fail_no_extra_verbose() {
    let p = project()
        .file("src/main.rs", "fn main() { std::process::exit(1); }")
        .build();

    assert_that(
        p.cargo("run -q"),
        execs().with_status(1).with_stdout("").with_stderr(""),
    );
}

#[test]
fn run_multiple_packages() {
    let p = project()
        .no_manifest()
        .file(
            "foo/Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [workspace]

            [dependencies]
            d1 = { path = "d1" }
            d2 = { path = "d2" }
            d3 = { path = "../d3" } # outside of the workspace

            [[bin]]
            name = "foo"
        "#,
        )
        .file("foo/src/foo.rs", "fn main() { println!(\"foo\"); }")
        .file("foo/d1/Cargo.toml", &basic_bin_manifest("d1"))
        .file("foo/d1/src/lib.rs", "")
        .file("foo/d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .file("foo/d2/Cargo.toml", &basic_bin_manifest("d2"))
        .file("foo/d2/src/main.rs", "fn main() { println!(\"d2\"); }")
        .file("d3/Cargo.toml", &basic_bin_manifest("d3"))
        .file("d3/src/main.rs", "fn main() { println!(\"d2\"); }")
        .build();

    let cargo = || {
        let mut process_builder = p.cargo("run");
        process_builder.cwd(p.root().join("foo"));
        process_builder
    };

    assert_that(
        cargo().arg("-p").arg("d1"),
        execs().with_stdout("d1"),
    );

    assert_that(
        cargo().arg("-p").arg("d2").arg("--bin").arg("d2"),
        execs().with_stdout("d2"),
    );

    assert_that(cargo(), execs().with_stdout("foo"));

    assert_that(cargo().arg("-p").arg("d1").arg("-p").arg("d2"),
                execs()
                    .with_status(1)
                    .with_stderr_contains("error: The argument '--package <SPEC>' was provided more than once, but cannot be used multiple times"));

    assert_that(
        cargo().arg("-p").arg("d3"),
        execs()
            .with_status(101)
            .with_stderr_contains("[ERROR] package `d3` is not a member of the workspace"),
    );
}

#[test]
fn explicit_bin_with_args() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
            fn main() {
                assert_eq!(std::env::args().nth(1).unwrap(), "hello");
                assert_eq!(std::env::args().nth(2).unwrap(), "world");
            }
        "#,
        )
        .build();

    assert_that(p.cargo("run --bin foo hello world"), execs());
}

#[test]
fn run_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
        "#,
        ).file("a/Cargo.toml", &basic_bin_manifest("a"))
        .file("a/src/main.rs", r#"fn main() {println!("run-a");}"#)
        .file("b/Cargo.toml", &basic_bin_manifest("b"))
        .file("b/src/main.rs", r#"fn main() {println!("run-b");}"#)
        .build();

    assert_that(
        p.cargo("run"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] `cargo run` requires that a project only have one executable[..]
available binaries: a, b",
        ),
    );
    assert_that(
        p.cargo("run --bin a"),
        execs().with_status(0).with_stdout("run-a"),
    );
}

#[test]
fn default_run_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
        "#,
        ).file(
            "a/Cargo.toml",
            r#"
            cargo-features = ["default-run"]

            [project]
            name = "a"
            version = "0.0.1"
            default-run = "a"
        "#,
        ).file("a/src/main.rs", r#"fn main() {println!("run-a");}"#)
        .file("b/Cargo.toml", &basic_bin_manifest("b"))
        .file("b/src/main.rs", r#"fn main() {println!("run-b");}"#)
        .build();

    assert_that(
        p.cargo("run").masquerade_as_nightly_cargo(),
        execs().with_status(0).with_stdout("run-a"),
    );
}
