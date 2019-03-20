use std::env;

use crate::support::registry::Package;
use crate::support::{basic_bin_manifest, basic_manifest, git, main_file, project};

#[test]
fn cargo_clean_simple() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build").run();
    assert!(p.build_dir().is_dir());

    p.cargo("clean").run();
    assert!(!p.build_dir().is_dir());
}

#[test]
fn different_dir() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("src/bar/a.rs", "")
        .build();

    p.cargo("build").run();
    assert!(p.build_dir().is_dir());

    p.cargo("clean")
        .cwd("src")
        .with_stdout("")
        .run();
    assert!(!p.build_dir().is_dir());
}

#[test]
fn clean_multiple_packages() {
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
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .file("d2/Cargo.toml", &basic_bin_manifest("d2"))
        .file("d2/src/main.rs", "fn main() { println!(\"d2\"); }")
        .build();

    p.cargo("build -p d1 -p d2 -p foo").run();

    let d1_path = &p
        .build_dir()
        .join("debug")
        .join(format!("d1{}", env::consts::EXE_SUFFIX));
    let d2_path = &p
        .build_dir()
        .join("debug")
        .join(format!("d2{}", env::consts::EXE_SUFFIX));

    assert!(p.bin("foo").is_file());
    assert!(d1_path.is_file());
    assert!(d2_path.is_file());

    p.cargo("clean -p d1 -p d2")
        .cwd("src")
        .with_stdout("")
        .run();
    assert!(p.bin("foo").is_file());
    assert!(!d1_path.is_file());
    assert!(!d2_path.is_file());
}

#[test]
fn clean_release() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build --release").run();

    p.cargo("clean -p foo").run();
    p.cargo("build --release").with_stdout("").run();

    p.cargo("clean -p foo --release").run();
    p.cargo("build --release")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] release [optimized] target(s) in [..]
",
        )
        .run();

    p.cargo("build").run();

    p.cargo("clean").arg("--release").run();
    assert!(p.build_dir().is_dir());
    assert!(p.build_dir().join("debug").is_dir());
    assert!(!p.build_dir().join("release").is_dir());
}

#[test]
fn clean_doc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("doc").run();

    let doc_path = &p.build_dir().join("doc");

    assert!(doc_path.is_dir());

    p.cargo("clean --doc").run();

    assert!(!doc_path.is_dir());
    assert!(p.build_dir().is_dir());
}

#[test]
fn build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
            use std::path::PathBuf;
            use std::env;

            fn main() {
                let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
                if env::var("FIRST").is_ok() {
                    std::fs::File::create(out.join("out")).unwrap();
                } else {
                    assert!(!std::fs::metadata(out.join("out")).is_ok());
                }
            }
        "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build").env("FIRST", "1").run();
    p.cargo("clean -p foo").run();
    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] build.rs [..]`
[RUNNING] `[..]build-script-build`
[RUNNING] `rustc [..] src/main.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[test]
fn clean_git() {
    let git = git::new("dep", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep", "0.5.0"))
            .file("src/lib.rs", "")
    })
    .unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            dep = {{ git = '{}' }}
        "#,
                git.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
    p.cargo("clean -p dep").with_stdout("").run();
    p.cargo("build").run();
}

#[test]
fn registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.1.0").publish();

    p.cargo("build").run();
    p.cargo("clean -p bar").with_stdout("").run();
    p.cargo("build").run();
}

#[test]
fn clean_verbose() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "foo"
        version = "0.0.1"

        [dependencies]
        bar = "0.1"
    "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.1.0").publish();

    p.cargo("build").run();
    p.cargo("clean -p bar --verbose")
        .with_stderr(
            "\
[REMOVING] [..]
[REMOVING] [..]
",
        )
        .run();
    p.cargo("build").run();
}
