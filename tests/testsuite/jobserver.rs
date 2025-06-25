//! Tests for the jobserver protocol.

use std::env;
use std::net::TcpListener;
use std::process::Command;
use std::thread;

use crate::prelude::*;
use crate::utils::cargo_exe;
use cargo_test_support::basic_bin_manifest;
use cargo_test_support::install::assert_has_installed_exe;
use cargo_test_support::paths;
use cargo_test_support::{project, rustc_host, str};
use cargo_util::is_ci;

const EXE_CONTENT: &str = r#"
use std::env;

fn main() {
    let var = env::var("CARGO_MAKEFLAGS").expect("no jobserver from env");
    let arg = var.split(' ')
                 .find(|p| p.starts_with("--jobserver"))
                .unwrap();
    let val = &arg[arg.find('=').unwrap() + 1..];
    validate(val);
}

#[cfg(unix)]
fn validate(s: &str) {
    use std::fs::{self, File};
    use std::io::*;
    use std::os::unix::prelude::*;

    if let Some((r, w)) = s.split_once(',') {
        // `--jobserver-auth=R,W`
        unsafe {
            let mut read = File::from_raw_fd(r.parse().unwrap());
            let mut write = File::from_raw_fd(w.parse().unwrap());

            let mut buf = [0];
            assert_eq!(read.read(&mut buf).unwrap(), 1);
            assert_eq!(write.write(&buf).unwrap(), 1);
        }
    } else {
        // `--jobserver-auth=fifo:PATH` is the default since GNU Make 4.4
        let (_, path) = s.split_once(':').expect("fifo:PATH");
        assert!(fs::metadata(path).unwrap().file_type().is_fifo());
    }
}

#[cfg(windows)]
fn validate(_: &str) {
    // a little too complicated for a test...
}
"#;

fn make_exe() -> &'static str {
    if cfg!(windows) {
        "mingw32-make"
    } else if cfg!(target_os = "aix") {
        "gmake"
    } else {
        "make"
    }
}

#[cargo_test]
fn jobserver_exists() {
    let p = project()
        .file("build.rs", EXE_CONTENT)
        .file("src/lib.rs", "")
        .build();

    // Explicitly use `-j2` to ensure that there's eventually going to be a
    // token to read from `validate` above, since running the build script
    // itself consumes a token.
    p.cargo("check -j2").run();
}

#[cargo_test]
fn external_subcommand_inherits_jobserver() {
    let make = make_exe();
    if Command::new(make).arg("--version").output().is_err() {
        return;
    }

    let name = "cargo-jobserver-check";
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "{name}"
                    version = "0.0.1"
                "#
            ),
        )
        .file("src/main.rs", EXE_CONTENT)
        .file(
            "Makefile",
            "\
all:
\t+$(CARGO) jobserver-check
",
        )
        .build();

    p.cargo("install --path .").run();
    assert_has_installed_exe(paths::cargo_home(), name);

    p.process(make).env("CARGO", cargo_exe()).arg("-j2").run();
}

#[cargo_test]
fn runner_inherits_jobserver() {
    let make = make_exe();
    if Command::new(make).arg("--version").output().is_err() {
        return;
    }

    let runner = "runner";
    project()
        .at(runner)
        .file("Cargo.toml", &basic_bin_manifest(runner))
        .file(
            "src/main.rs",
            r#"
            pub fn main() {
                eprintln!("this is a runner");
                let args: Vec<String> = std::env::args().collect();
                let status = std::process::Command::new(&args[1]).status().unwrap();
                assert!(status.success());
            }
            "#,
        )
        .build()
        .cargo("install --path .")
        .run();

    // Add .cargo/bin to PATH
    let mut path: Vec<_> = env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect();
    path.push(paths::cargo_home().join("bin"));
    let path = &env::join_paths(path).unwrap();
    assert_has_installed_exe(paths::cargo_home(), runner);

    let host = rustc_host();
    let config_value = &format!("target.{host}.runner = \"{runner}\"");

    let name = "cargo-jobserver-check";
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "{name}"
                    version = "0.0.1"
                    edition = "2015"
                "#
            ),
        )
        .file(
            "src/lib.rs",
            r#"
#[test]
fn test() {
    _ = std::env::var("CARGO_MAKEFLAGS").expect("no jobserver from env");
}
        "#,
        )
        .file("src/main.rs", EXE_CONTENT)
        .file(
            "Makefile",
            &format!(
                "\
run:
\t+$(CARGO) run

run-runner:
\t+$(CARGO) run --config '{config_value}'

test:
\t+$(CARGO) test --lib

test-runner:
\t+$(CARGO) test --lib --config '{config_value}'
",
            ),
        )
        .build();

    // jobserver can be inherited from env
    p.process(make)
        .env("CARGO", cargo_exe())
        .arg("run")
        .arg("-j2")
        .run();
    p.process(make)
        .env("PATH", path)
        .env("CARGO", cargo_exe())
        .arg("run-runner")
        .arg("-j2")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `runner target/debug/cargo-jobserver-check[EXE]`
this is a runner

"#]])
        .run();
    p.process(make)
        .env("CARGO", cargo_exe())
        .arg("test")
        .arg("-j2")
        .run();
    p.process(make)
        .env("PATH", path)
        .env("CARGO", cargo_exe())
        .arg("test-runner")
        .arg("-j2")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/cargo_jobserver_check-[HASH][EXE])
this is a runner

"#]])
        .run();

    // but not from `-j` flag
    p.cargo("run -j2")
        .with_status(101)
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/cargo-jobserver-check[EXE]`
...
[..]no jobserver from env[..]
...

"#]])
        .run();
    p.cargo("run -j2")
        .env("PATH", path)
        .arg("--config")
        .arg(config_value)
        .with_status(101)
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `runner target/debug/cargo-jobserver-check[EXE]`
this is a runner
...
[..]no jobserver from env[..]
...

"#]])
        .run();
    p.cargo("test -j2")
        .with_status(101)
        .with_stdout_data("...\n[..]no jobserver from env[..]\n...")
        .run();
    p.cargo("test -j2")
        .env("PATH", path)
        .arg("--config")
        .arg(config_value)
        .with_status(101)
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/cargo_jobserver_check-[HASH][EXE])
this is a runner
...

"#]])
        .with_stdout_data("...\n[..]no jobserver from env[..]\n...")
        .run();
}

#[cargo_test]
fn makes_jobserver_used() {
    let make = make_exe();
    if !is_ci() && Command::new(make).arg("--version").output().is_err() {
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

                [dependencies]
                d1 = { path = "d1" }
                d2 = { path = "d2" }
                d3 = { path = "d3" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.1"
                authors = []
                build = "../dbuild.rs"
            "#,
        )
        .file("d1/src/lib.rs", "")
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.1"
                authors = []
                build = "../dbuild.rs"
            "#,
        )
        .file("d2/src/lib.rs", "")
        .file(
            "d3/Cargo.toml",
            r#"
                [package]
                name = "d3"
                version = "0.0.1"
                authors = []
                build = "../dbuild.rs"
            "#,
        )
        .file("d3/src/lib.rs", "")
        .file(
            "dbuild.rs",
            r#"
                use std::net::TcpStream;
                use std::env;
                use std::io::Read;

                fn main() {
                    let addr = env::var("ADDR").unwrap();
                    let mut stream = TcpStream::connect(addr).unwrap();
                    let mut v = Vec::new();
                    stream.read_to_end(&mut v).unwrap();
                }
            "#,
        )
        .file(
            "Makefile",
            "\
all:
\t+$(CARGO) build
",
        )
        .build();

    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();

    let child = thread::spawn(move || {
        let a1 = l.accept().unwrap();
        let a2 = l.accept().unwrap();
        l.set_nonblocking(true).unwrap();

        for _ in 0..1000 {
            assert!(l.accept().is_err());
            thread::yield_now();
        }

        drop(a1);
        l.set_nonblocking(false).unwrap();
        let a3 = l.accept().unwrap();

        drop((a2, a3));
    });

    p.process(make)
        .env("CARGO", cargo_exe())
        .env("ADDR", addr.to_string())
        .arg("-j2")
        .run();
    child.join().unwrap();
}

#[cargo_test]
fn jobserver_and_j() {
    let make = make_exe();
    if !is_ci() && Command::new(make).arg("--version").output().is_err() {
        return;
    }

    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Makefile",
            "\
all:
\t+$(CARGO) build -j2
",
        )
        .build();

    p.process(make)
        .env("CARGO", cargo_exe())
        .arg("-j2")
        .with_stderr_data(str![[r#"
[WARNING] a `-j` argument was passed to Cargo but Cargo is also configured with an external jobserver in its environment, ignoring the `-j` parameter
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
