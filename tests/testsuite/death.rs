//! Tests for ctrl-C handling.

use cargo_test_support::{project, slow_cpu_multiplier};
use std::fs;
use std::io::{self, Read};
use std::net::TcpListener;
use std::process::{Child, Stdio};
use std::thread;
use std::time;

#[cargo_test]
fn ctrl_c_kills_everyone() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

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
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            &format!(
                r#"
                    use std::net::TcpStream;
                    use std::io::Read;

                    fn main() {{
                        let mut socket = TcpStream::connect("{}").unwrap();
                        let _ = socket.read(&mut [0; 10]);
                        panic!("that read should never return");
                    }}
                "#,
                addr
            ),
        )
        .build();

    let mut cargo = p.cargo("check").build_command();
    cargo
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("__CARGO_TEST_SETSID_PLEASE_DONT_USE_ELSEWHERE", "1");
    let mut child = cargo.spawn().unwrap();

    let mut sock = listener.accept().unwrap().0;
    ctrl_c(&mut child);

    assert!(!child.wait().unwrap().success());
    match sock.read(&mut [0; 10]) {
        Ok(n) => assert_eq!(n, 0),
        Err(e) => assert_eq!(e.kind(), io::ErrorKind::ConnectionReset),
    }

    // Ok so what we just did was spawn cargo that spawned a build script, then
    // we killed cargo in hopes of it killing the build script as well. If all
    // went well the build script is now dead. On Windows, however, this is
    // enforced with job objects which means that it may actually be in the
    // *process* of being torn down at this point.
    //
    // Now on Windows we can't completely remove a file until all handles to it
    // have been closed. Including those that represent running processes. So if
    // we were to return here then there may still be an open reference to some
    // file in the build directory. What we want to actually do is wait for the
    // build script to *complete* exit. Take care of that by blowing away the
    // build directory here, and panicking if we eventually spin too long
    // without being able to.
    for i in 0..10 {
        match fs::remove_dir_all(&p.root().join("target")) {
            Ok(()) => return,
            Err(e) => println!("attempt {}: {}", i, e),
        }
        thread::sleep(slow_cpu_multiplier(100));
    }

    panic!(
        "couldn't remove build directory after a few tries, seems like \
         we won't be able to!"
    );
}

#[cargo_test]
fn kill_cargo_add_never_corrupts_cargo_toml() {
    cargo_test_support::registry::init();
    cargo_test_support::registry::Package::new("my-package", "0.1.1+my-package").publish();

    let with_dependency = r#"
[package]
name = "foo"
version = "0.0.1"
authors = []

[dependencies]
my-package = "0.1.1"
"#;
    let without_dependency = r#"
[package]
name = "foo"
version = "0.0.1"
authors = []
"#;

    for sleep_time_ms in [30, 60, 90] {
        let p = project()
            .file("Cargo.toml", without_dependency)
            .file("src/lib.rs", "")
            .build();

        let mut cargo = p.cargo("add").arg("my-package").build_command();
        cargo
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cargo.spawn().unwrap();

        thread::sleep(time::Duration::from_millis(sleep_time_ms));

        assert!(child.kill().is_ok());
        assert!(child.wait().is_ok());

        // check the Cargo.toml
        let contents = fs::read(p.root().join("Cargo.toml")).unwrap();

        // not empty
        assert_ne!(
            contents, b"",
            "Cargo.toml is empty, and should not be at {} milliseconds",
            sleep_time_ms
        );

        // We should have the original Cargo.toml or the new one, nothing else.
        if std::str::from_utf8(&contents)
            .unwrap()
            .contains("[dependencies]")
        {
            assert_eq!(
                std::str::from_utf8(&contents).unwrap(),
                with_dependency,
                "Cargo.toml is with_dependency after add at {} milliseconds",
                sleep_time_ms
            );
        } else {
            assert_eq!(
                std::str::from_utf8(&contents).unwrap(),
                without_dependency,
                "Cargo.toml is without_dependency after add at {} milliseconds",
                sleep_time_ms
            );
        }
    }
}

#[cargo_test]
fn kill_cargo_remove_never_corrupts_cargo_toml() {
    let with_dependency = r#"
[package]
name = "foo"
version = "0.0.1"
authors = []
build = "build.rs"

[dependencies]
bar = "0.0.1"
"#;
    let without_dependency = r#"
[package]
name = "foo"
version = "0.0.1"
authors = []
build = "build.rs"
"#;

    // This test depends on killing the cargo process at the right time to cause a failed write.
    // Note that we're iterating and using the index as time in ms to sleep before killing the cargo process.
    // If it is working correctly, we never fail, but can't hang out here all day...
    // So we'll just run it a few times and hope for the best.
    for sleep_time_ms in [30, 60, 90] {
        // new basic project with a single dependency
        let p = project()
            .file("Cargo.toml", with_dependency)
            .file("src/lib.rs", "")
            .build();

        // run cargo remove the dependency
        let mut cargo = p.cargo("remove").arg("bar").build_command();
        cargo
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cargo.spawn().unwrap();

        thread::sleep(time::Duration::from_millis(sleep_time_ms));

        assert!(child.kill().is_ok());
        assert!(child.wait().is_ok());

        // check the Cargo.toml
        let contents = fs::read(p.root().join("Cargo.toml")).unwrap();

        // not empty
        assert_ne!(
            contents, b"",
            "Cargo.toml is empty, and should not be at {} milliseconds",
            sleep_time_ms
        );

        // We should have the original Cargo.toml or the new one, nothing else.
        if std::str::from_utf8(&contents)
            .unwrap()
            .contains("[dependencies]")
        {
            assert_eq!(
                std::str::from_utf8(&contents).unwrap(),
                with_dependency,
                "Cargo.toml is not the same as the original at {} milliseconds",
                sleep_time_ms
            );
        } else {
            assert_eq!(
                std::str::from_utf8(&contents).unwrap(),
                without_dependency,
                "Cargo.toml is not the same as expected at {} milliseconds",
                sleep_time_ms
            );
        }
    }
}

#[cfg(unix)]
pub fn ctrl_c(child: &mut Child) {
    let r = unsafe { libc::kill(-(child.id() as i32), libc::SIGINT) };
    if r < 0 {
        panic!("failed to kill: {}", io::Error::last_os_error());
    }
}

#[cfg(windows)]
pub fn ctrl_c(child: &mut Child) {
    child.kill().unwrap();
}
