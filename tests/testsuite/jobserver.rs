//! Tests for the jobserver protocol.

use std::net::TcpListener;
use std::process::Command;
use std::thread;

use cargo_test_support::{cargo_exe, project};

#[cargo_test]
fn jobserver_exists() {
    let p = project()
        .file(
            "build.rs",
            r#"
                use std::env;

                fn main() {
                    let var = env::var("CARGO_MAKEFLAGS").unwrap();
                    let arg = var.split(' ')
                                 .find(|p| p.starts_with("--jobserver"))
                                 .unwrap();
                    let val = &arg[arg.find('=').unwrap() + 1..];
                    validate(val);
                }

                #[cfg(unix)]
                fn validate(s: &str) {
                    use std::fs::File;
                    use std::io::*;
                    use std::os::unix::prelude::*;

                    let fds = s.split(',').collect::<Vec<_>>();
                    println!("{}", s);
                    assert_eq!(fds.len(), 2);
                    unsafe {
                        let mut read = File::from_raw_fd(fds[0].parse().unwrap());
                        let mut write = File::from_raw_fd(fds[1].parse().unwrap());

                        let mut buf = [0];
                        assert_eq!(read.read(&mut buf).unwrap(), 1);
                        assert_eq!(write.write(&buf).unwrap(), 1);
                    }
                }

                #[cfg(windows)]
                fn validate(_: &str) {
                    // a little too complicated for a test...
                }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Explicitly use `-j2` to ensure that there's eventually going to be a
    // token to read from `validate` above, since running the build script
    // itself consumes a token.
    p.cargo("build -j2").run();
}

#[cargo_test]
fn makes_jobserver_used() {
    let make = if cfg!(windows) {
        "mingw32-make"
    } else {
        "make"
    };
    if Command::new(make).arg("--version").output().is_err() {
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
    let make = if cfg!(windows) {
        "mingw32-make"
    } else {
        "make"
    };
    if Command::new(make).arg("--version").output().is_err() {
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
        .with_stderr(
            "\
warning: a `-j` argument was passed to Cargo but Cargo is also configured \
with an external jobserver in its environment, ignoring the `-j` parameter
[COMPILING] [..]
[FINISHED] [..]
",
        )
        .run();
}
