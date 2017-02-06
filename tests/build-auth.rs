extern crate bufstream;
extern crate git2;
extern crate cargotest;
extern crate hamcrest;

use std::collections::HashSet;
use std::io::prelude::*;
use std::net::TcpListener;
use std::thread;

use bufstream::BufStream;
use cargotest::support::paths;
use cargotest::support::{project, execs};
use hamcrest::assert_that;

// Test that HTTP auth is offered from `credential.helper`
#[test]
fn http_auth_offered() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    fn headers(rdr: &mut BufRead) -> HashSet<String> {
        let valid = ["GET", "Authorization", "Accept", "User-Agent"];
        rdr.lines().map(|s| s.unwrap())
           .take_while(|s| s.len() > 2)
           .map(|s| s.trim().to_string())
           .filter(|s| {
               valid.iter().any(|prefix| s.starts_with(*prefix))
            })
           .collect()
    }

    let t = thread::spawn(move|| {
        let mut conn = BufStream::new(server.accept().unwrap().0);
        let req = headers(&mut conn);
        let user_agent = if cfg!(windows) {
            "User-Agent: git/1.0 (libgit2 0.25.0)"
        } else {
            "User-Agent: git/2.0 (libgit2 0.25.0)"
        };
        conn.write_all(b"\
            HTTP/1.1 401 Unauthorized\r\n\
            WWW-Authenticate: Basic realm=\"wheee\"\r\n
            \r\n\
        ").unwrap();
        assert_eq!(req, vec![
            "GET /foo/bar/info/refs?service=git-upload-pack HTTP/1.1",
            "Accept: */*",
            user_agent,
        ].into_iter().map(|s| s.to_string()).collect());
        drop(conn);

        let mut conn = BufStream::new(server.accept().unwrap().0);
        let req = headers(&mut conn);
        conn.write_all(b"\
            HTTP/1.1 401 Unauthorized\r\n\
            WWW-Authenticate: Basic realm=\"wheee\"\r\n
            \r\n\
        ").unwrap();
        assert_eq!(req, vec![
            "GET /foo/bar/info/refs?service=git-upload-pack HTTP/1.1",
            "Authorization: Basic Zm9vOmJhcg==",
            "Accept: */*",
            user_agent,
        ].into_iter().map(|s| s.to_string()).collect());
    });

    let script = project("script")
        .file("Cargo.toml", r#"
            [project]
            name = "script"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {
                println!("username=foo");
                println!("password=bar");
            }
        "#);

    assert_that(script.cargo_process("build").arg("-v"),
                execs().with_status(0));
    let script = script.bin("script");

    let config = paths::home().join(".gitconfig");
    let mut config = git2::Config::open(&config).unwrap();
    config.set_str("credential.helper",
                   &script.display().to_string()).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            git = "http://127.0.0.1:{}/foo/bar"
        "#, addr.port()))
        .file("src/main.rs", "")
        .file(".cargo/config","\
        [net]
        retry = 0
        ");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(&format!("\
[UPDATING] git repository `http://{addr}/foo/bar`
[ERROR] failed to load source for a dependency on `bar`

Caused by:
  Unable to update http://{addr}/foo/bar

Caused by:
  failed to clone into: [..]

Caused by:
  failed to authenticate when downloading repository
attempted to find username/password via `credential.helper`, but [..]

To learn more, run the command again with --verbose.
",
        addr = addr)));

    t.join().ok().unwrap();
}

// Boy, sure would be nice to have a TLS implementation in rust!
#[test]
fn https_something_happens() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let t = thread::spawn(move|| {
        let mut conn = server.accept().unwrap().0;
        drop(conn.write(b"1234"));
        drop(conn.shutdown(std::net::Shutdown::Write));
        drop(conn.read(&mut [0; 16]));
    });

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            git = "https://127.0.0.1:{}/foo/bar"
        "#, addr.port()))
        .file("src/main.rs", "")
        .file(".cargo/config","\
        [net]
        retry = 0
        ");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr_contains(&format!("\
[UPDATING] git repository `https://{addr}/foo/bar`
", addr = addr))
                    .with_stderr_contains(&format!("\
Caused by:
  {errmsg}
",
        errmsg = if cfg!(windows) {
            "[[..]] failed to send request: [..]\n"
        } else if cfg!(target_os = "macos") {
            // OSX is difficult to tests as some builds may use
            // Security.framework and others may use OpenSSL. In that case let's
            // just not verify the error message here.
            "[..]"
        } else {
            "[..] SSL error: [..]"
        })));

    t.join().ok().unwrap();
}

// Boy, sure would be nice to have an SSH implementation in rust!
#[test]
fn ssh_something_happens() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let t = thread::spawn(move|| {
        drop(server.accept().unwrap());
    });

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            git = "ssh://127.0.0.1:{}/foo/bar"
        "#, addr.port()))
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr_contains(&format!("\
[UPDATING] git repository `ssh://{addr}/foo/bar`
", addr = addr))
                    .with_stderr_contains("\
Caused by:
  [[..]] Failed to start SSH session: Failed getting banner
"));
    t.join().ok().unwrap();
}
