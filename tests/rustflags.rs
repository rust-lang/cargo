extern crate cargotest;
extern crate hamcrest;

use std::io::Write;
use std::fs::{self, File};

use cargotest::rustc_host;
use cargotest::support::{project, project_in_home, execs, paths};
use hamcrest::assert_that;

#[test]
fn env_rustflags_normal_source() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file("benches/d.rs", r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }"#);
    p.build();

    // Use RUSTFLAGS to pass an argument that will generate an error
    assert_that(p.cargo("build").env("RUSTFLAGS", "-Z bogus")
                .arg("--lib"),
                execs().with_status(101));
    assert_that(p.cargo("build").env("RUSTFLAGS", "-Z bogus")
                .arg("--bin=a"),
                execs().with_status(101));
    assert_that(p.cargo("build").env("RUSTFLAGS", "-Z bogus")
                .arg("--example=b"),
                execs().with_status(101));
    assert_that(p.cargo("test").env("RUSTFLAGS", "-Z bogus"),
                execs().with_status(101));
    assert_that(p.cargo("bench").env("RUSTFLAGS", "-Z bogus"),
                execs().with_status(101));
}

#[test]
fn env_rustflags_build_script() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    // In this test if --cfg foo is passed the build will fail.
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() { }
            #[cfg(not(foo))]
            fn main() { }
        "#);
    p.build();

    assert_that(p.cargo("build").env("RUSTFLAGS", "--cfg foo"),
                execs().with_status(0));
}

#[test]
fn env_rustflags_build_script_dep() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            build = "build.rs"

            [build-dependencies.bar]
            path = "../bar"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() { }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", r#"
            fn bar() { }
            #[cfg(not(foo))]
            fn bar() { }
        "#);
    foo.build();
    bar.build();

    assert_that(foo.cargo("build").env("RUSTFLAGS", "--cfg foo"),
                execs().with_status(0));
}

#[test]
fn env_rustflags_plugin() {
    // RUSTFLAGS should be passed to rustc for plugins
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [lib]
            name = "foo"
            plugin = true
        "#)
        .file("src/lib.rs", r#"
            fn main() { }
            #[cfg(not(foo))]
            fn main() { }
        "#);
    p.build();

    assert_that(p.cargo("build").env("RUSTFLAGS", "--cfg foo"),
                execs().with_status(0));
}

#[test]
fn env_rustflags_plugin_dep() {
    // RUSTFLAGS should be passed to rustc for plugins
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [lib]
            name = "foo"
            plugin = true

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/lib.rs", r#"
            fn foo() { }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"

            [lib]
            name = "bar"
        "#)
        .file("src/lib.rs", r#"
            fn bar() { }
            #[cfg(not(foo))]
            fn bar() { }
        "#);
    foo.build();
    bar.build();

    assert_that(foo.cargo("build").env("RUSTFLAGS", "--cfg foo"),
                execs().with_status(0));
}

#[test]
fn env_rustflags_normal_source_with_target() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file("benches/d.rs", r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }"#);
    p.build();

    let host = &rustc_host();

    // Use RUSTFLAGS to pass an argument that will generate an error
    assert_that(p.cargo("build").env("RUSTFLAGS", "-Z bogus")
                .arg("--lib").arg("--target").arg(host),
                execs().with_status(101));
    assert_that(p.cargo("build").env("RUSTFLAGS", "-Z bogus")
                .arg("--bin=a").arg("--target").arg(host),
                execs().with_status(101));
    assert_that(p.cargo("build").env("RUSTFLAGS", "-Z bogus")
                .arg("--example=b").arg("--target").arg(host),
                execs().with_status(101));
    assert_that(p.cargo("test").env("RUSTFLAGS", "-Z bogus")
                .arg("--target").arg(host),
                execs().with_status(101));
    assert_that(p.cargo("bench").env("RUSTFLAGS", "-Z bogus")
                .arg("--target").arg(host),
                execs().with_status(101));
}

#[test]
fn env_rustflags_build_script_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() { }
            #[cfg(foo)]
            fn main() { }
        "#);
    p.build();

    let host = rustc_host();
    assert_that(p.cargo("build").env("RUSTFLAGS", "--cfg foo")
                .arg("--target").arg(host),
                execs().with_status(0));
}

#[test]
fn env_rustflags_build_script_dep_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            build = "build.rs"

            [build-dependencies.bar]
            path = "../bar"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() { }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", r#"
            fn bar() { }
            #[cfg(foo)]
            fn bar() { }
        "#);
    foo.build();
    bar.build();

    let host = rustc_host();
    assert_that(foo.cargo("build").env("RUSTFLAGS", "--cfg foo")
                .arg("--target").arg(host),
                execs().with_status(0));
}

#[test]
fn env_rustflags_plugin_with_target() {
    // RUSTFLAGS should not be passed to rustc for plugins
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [lib]
            name = "foo"
            plugin = true
        "#)
        .file("src/lib.rs", r#"
            fn main() { }
            #[cfg(foo)]
            fn main() { }
        "#);
    p.build();

    let host = rustc_host();
    assert_that(p.cargo("build").env("RUSTFLAGS", "--cfg foo")
                .arg("--target").arg(host),
                execs().with_status(0));
}

#[test]
fn env_rustflags_plugin_dep_with_target() {
    // RUSTFLAGS should not be passed to rustc for plugins
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [lib]
            name = "foo"
            plugin = true

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/lib.rs", r#"
            fn foo() { }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"

            [lib]
            name = "bar"
        "#)
        .file("src/lib.rs", r#"
            fn bar() { }
            #[cfg(foo)]
            fn bar() { }
        "#);
    foo.build();
    bar.build();

    let host = rustc_host();
    assert_that(foo.cargo("build").env("RUSTFLAGS", "--cfg foo")
                .arg("--target").arg(host),
                execs().with_status(0));
}

#[test]
fn env_rustflags_recompile() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build"),
                execs().with_status(0));
    // Setting RUSTFLAGS forces a recompile
    assert_that(p.cargo("build").env("RUSTFLAGS", "-Z bogus"),
                execs().with_status(101));
}

#[test]
fn env_rustflags_recompile2() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build").env("RUSTFLAGS", "--cfg foo"),
                execs().with_status(0));
    // Setting RUSTFLAGS forces a recompile
    assert_that(p.cargo("build").env("RUSTFLAGS", "-Z bogus"),
                execs().with_status(101));
}

#[test]
fn env_rustflags_no_recompile() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build").env("RUSTFLAGS", "--cfg foo"),
                execs().with_status(0));
    assert_that(p.cargo("build").env("RUSTFLAGS", "--cfg foo"),
                execs().with_stdout("").with_status(0));
}

#[test]
fn build_rustflags_normal_source() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file("benches/d.rs", r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }"#)
        .file(".cargo/config", r#"
            [build]
            rustflags = ["-Z", "bogus"]
            "#);
    p.build();

    assert_that(p.cargo("build")
                .arg("--lib"),
                execs().with_status(101));
    assert_that(p.cargo("build")
                .arg("--bin=a"),
                execs().with_status(101));
    assert_that(p.cargo("build")
                .arg("--example=b"),
                execs().with_status(101));
    assert_that(p.cargo("test"),
                execs().with_status(101));
    assert_that(p.cargo("bench"),
                execs().with_status(101));
}

#[test]
fn build_rustflags_build_script() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    // In this test if --cfg foo is passed the build will fail.
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() { }
            #[cfg(not(foo))]
            fn main() { }
        "#)
        .file(".cargo/config", r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#);
    p.build();

    assert_that(p.cargo("build"),
                execs().with_status(0));
}

#[test]
fn build_rustflags_build_script_dep() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            build = "build.rs"

            [build-dependencies.bar]
            path = "../bar"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() { }
        "#)
        .file(".cargo/config", r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", r#"
            fn bar() { }
            #[cfg(not(foo))]
            fn bar() { }
        "#);
    foo.build();
    bar.build();

    assert_that(foo.cargo("build"),
                execs().with_status(0));
}

#[test]
fn build_rustflags_plugin() {
    // RUSTFLAGS should be passed to rustc for plugins
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [lib]
            name = "foo"
            plugin = true
        "#)
        .file("src/lib.rs", r#"
            fn main() { }
            #[cfg(not(foo))]
            fn main() { }
        "#)
        .file(".cargo/config", r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#);
    p.build();

    assert_that(p.cargo("build"),
                execs().with_status(0));
}

#[test]
fn build_rustflags_plugin_dep() {
    // RUSTFLAGS should be passed to rustc for plugins
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [lib]
            name = "foo"
            plugin = true

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/lib.rs", r#"
            fn foo() { }
        "#)
        .file(".cargo/config", r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"

            [lib]
            name = "bar"
        "#)
        .file("src/lib.rs", r#"
            fn bar() { }
            #[cfg(not(foo))]
            fn bar() { }
        "#);
    foo.build();
    bar.build();

    assert_that(foo.cargo("build"),
                execs().with_status(0));
}

#[test]
fn build_rustflags_normal_source_with_target() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file("benches/d.rs", r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }"#)
        .file(".cargo/config", r#"
            [build]
            rustflags = ["-Z", "bogus"]
            "#);
    p.build();

    let ref host = rustc_host();

    // Use RUSTFLAGS to pass an argument that will generate an error
    assert_that(p.cargo("build")
                .arg("--lib").arg("--target").arg(host),
                execs().with_status(101));
    assert_that(p.cargo("build")
                .arg("--bin=a").arg("--target").arg(host),
                execs().with_status(101));
    assert_that(p.cargo("build")
                .arg("--example=b").arg("--target").arg(host),
                execs().with_status(101));
    assert_that(p.cargo("test")
                .arg("--target").arg(host),
                execs().with_status(101));
    assert_that(p.cargo("bench")
                .arg("--target").arg(host),
                execs().with_status(101));
}

#[test]
fn build_rustflags_build_script_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() { }
            #[cfg(foo)]
            fn main() { }
        "#)
        .file(".cargo/config", r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#);
    p.build();

    let host = rustc_host();
    assert_that(p.cargo("build")
                .arg("--target").arg(host),
                execs().with_status(0));
}

#[test]
fn build_rustflags_build_script_dep_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            build = "build.rs"

            [build-dependencies.bar]
            path = "../bar"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() { }
        "#)
        .file(".cargo/config", r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", r#"
            fn bar() { }
            #[cfg(foo)]
            fn bar() { }
        "#);
    foo.build();
    bar.build();

    let host = rustc_host();
    assert_that(foo.cargo("build")
                .arg("--target").arg(host),
                execs().with_status(0));
}

#[test]
fn build_rustflags_plugin_with_target() {
    // RUSTFLAGS should not be passed to rustc for plugins
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [lib]
            name = "foo"
            plugin = true
        "#)
        .file("src/lib.rs", r#"
            fn main() { }
            #[cfg(foo)]
            fn main() { }
        "#)
        .file(".cargo/config", r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#);
    p.build();

    let host = rustc_host();
    assert_that(p.cargo("build")
                .arg("--target").arg(host),
                execs().with_status(0));
}

#[test]
fn build_rustflags_plugin_dep_with_target() {
    // RUSTFLAGS should not be passed to rustc for plugins
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [lib]
            name = "foo"
            plugin = true

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/lib.rs", r#"
            fn foo() { }
        "#)
        .file(".cargo/config", r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"

            [lib]
            name = "bar"
        "#)
        .file("src/lib.rs", r#"
            fn bar() { }
            #[cfg(foo)]
            fn bar() { }
        "#);
    foo.build();
    bar.build();

    let host = rustc_host();
    assert_that(foo.cargo("build")
                .arg("--target").arg(host),
                execs().with_status(0));
}

#[test]
fn build_rustflags_recompile() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build"),
                execs().with_status(0));

    // Setting RUSTFLAGS forces a recompile
    let config = r#"
        [build]
        rustflags = ["-Z", "bogus"]
        "#;
    let config_file = paths::root().join("foo/.cargo/config");
    fs::create_dir_all(config_file.parent().unwrap()).unwrap();
    let mut config_file = File::create(config_file).unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    assert_that(p.cargo("build"),
                execs().with_status(101));
}

#[test]
fn build_rustflags_recompile2() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build").env("RUSTFLAGS", "--cfg foo"),
                execs().with_status(0));

    // Setting RUSTFLAGS forces a recompile
    let config = r#"
        [build]
        rustflags = ["-Z", "bogus"]
        "#;
    let config_file = paths::root().join("foo/.cargo/config");
    fs::create_dir_all(config_file.parent().unwrap()).unwrap();
    let mut config_file = File::create(config_file).unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    assert_that(p.cargo("build"),
                execs().with_status(101));
}

#[test]
fn build_rustflags_no_recompile() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#);
    p.build();

    assert_that(p.cargo("build").env("RUSTFLAGS", "--cfg foo"),
                execs().with_status(0));
    assert_that(p.cargo("build").env("RUSTFLAGS", "--cfg foo"),
                execs().with_stdout("").with_status(0));
}

#[test]
fn build_rustflags_with_home_config() {
    // We need a config file inside the home directory
    let home = paths::home();
    let home_config = home.join(".cargo");
    fs::create_dir(&home_config).unwrap();
    File::create(&home_config.join("config")).unwrap().write_all(br#"
        [build]
        rustflags = ["-Cllvm-args=-x86-asm-syntax=intel"]
    "#).unwrap();

    // And we need the project to be inside the home directory
    // so the walking process finds the home project twice.
    let p = project_in_home("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0));
}

#[test]
fn target_rustflags_normal_source() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file("benches/d.rs", r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }"#)
        .file(".cargo/config", &format!("
            [target.{}]
            rustflags = [\"-Z\", \"bogus\"]
            ", rustc_host()));
    p.build();

    assert_that(p.cargo("build")
                .arg("--lib"),
                execs().with_status(101));
    assert_that(p.cargo("build")
                .arg("--bin=a"),
                execs().with_status(101));
    assert_that(p.cargo("build")
                .arg("--example=b"),
                execs().with_status(101));
    assert_that(p.cargo("test"),
                execs().with_status(101));
    assert_that(p.cargo("bench"),
                execs().with_status(101));
}

// target.{}.rustflags takes precedence over build.rustflags
#[test]
fn target_rustflags_precedence() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", &format!("
            [build]
            rustflags = [\"--cfg\", \"foo\"]

            [target.{}]
            rustflags = [\"-Z\", \"bogus\"]
            ", rustc_host()));
    p.build();

    assert_that(p.cargo("build")
                .arg("--lib"),
                execs().with_status(101));
    assert_that(p.cargo("build")
                .arg("--bin=a"),
                execs().with_status(101));
    assert_that(p.cargo("build")
                .arg("--example=b"),
                execs().with_status(101));
    assert_that(p.cargo("test"),
                execs().with_status(101));
    assert_that(p.cargo("bench"),
                execs().with_status(101));
}

#[test]
fn cfg_rustflags_normal_source() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "pub fn t() {}")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(".cargo/config", &format!(r#"
            [target.'cfg({})']
            rustflags = ["--cfg", "bar"]
            "#, if rustc_host().contains("-windows-") {"windows"} else {"not(windows)"}));
    p.build();
    
    assert_that(p.cargo("build").arg("--lib").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    
    assert_that(p.cargo("build").arg("--bin=a").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    
    assert_that(p.cargo("build").arg("--example=b").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    
    assert_that(p.cargo("test").arg("--no-run").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    
    assert_that(p.cargo("bench").arg("--no-run").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] release [optimized] target(s) in [..]
"));
                
}

// target.'cfg(...)'.rustflags takes precedence over build.rustflags
#[test]
fn cfg_rustflags_precedence() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "pub fn t() {}")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(".cargo/config", &format!(r#"
            [build]
            rustflags = ["--cfg", "foo"]

            [target.'cfg({})']
            rustflags = ["--cfg", "bar"]
            "#, if rustc_host().contains("-windows-") { "windows" } else { "not(windows)" }));
    p.build();

    assert_that(p.cargo("build").arg("--lib").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("build").arg("--bin=a").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("build").arg("--example=b").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("test").arg("--no-run").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("bench").arg("--no-run").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] release [optimized] target(s) in [..]
"));
                
}

#[test]
fn target_rustflags_string_and_array_form1() {
    let p1 = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#);
    p1.build();

    assert_that(p1.cargo("build").arg("-v"),
        execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg foo[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    let p2 = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
            [build]
            rustflags = "--cfg foo"
            "#);
    p2.build();

    assert_that(p2.cargo("build").arg("-v"),
        execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg foo[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

}

#[test]
fn target_rustflags_string_and_array_form2() {
    let p1 = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file(".cargo/config", &format!(r#"
            [target.{}]
            rustflags = ["--cfg", "foo"]
        "#, rustc_host()))
        .file("src/lib.rs", "");
    p1.build();

    assert_that(p1.cargo("build").arg("-v"),
        execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg foo[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    let p2 = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
        "#)
        .file(".cargo/config", &format!(r#"
            [target.{}]
            rustflags = "--cfg foo"
        "#, rustc_host()))
        .file("src/lib.rs", "");
    p2.build();

    assert_that(p2.cargo("build").arg("-v"),
        execs().with_status(0).with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg foo[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

}
