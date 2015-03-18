use std::collections::HashSet;
use std::io::BufStream;
use std::io::prelude::*;
use std::net::TcpListener;
use std::thread;
use git2;

use support::{project, execs, UPDATING};
use support::paths;
use hamcrest::assert_that;

fn setup() {
}

// Test that HTTP auth is offered from `credential.helper`
test!(http_auth_offered {
    let a = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = a.socket_addr().unwrap();

    fn headers(rdr: &mut BufRead) -> HashSet<String> {
        let valid = ["GET", "Authorization", "Accept", "User-Agent"];
        rdr.lines().map(|s| s.unwrap())
           .take_while(|s| s.len() > 2)
           .map(|s| s.as_slice().trim().to_string())
           .filter(|s| {
               valid.iter().any(|prefix| s.as_slice().starts_with(*prefix))
            })
           .collect()
    }

    let t = thread::spawn(move|| {
        let mut s = BufStream::new(a.accept().unwrap().0);
        let req = headers(&mut s);
        s.write_all(b"\
            HTTP/1.1 401 Unauthorized\r\n\
            WWW-Authenticate: Basic realm=\"wheee\"\r\n
            \r\n\
        ").unwrap();
        assert_eq!(req, vec![
            "GET /foo/bar/info/refs?service=git-upload-pack HTTP/1.1",
            "Accept: */*",
            "User-Agent: git/1.0 (libgit2 0.22.0)",
        ].into_iter().map(|s| s.to_string()).collect());
        drop(s);

        let mut s = BufStream::new(a.accept().unwrap().0);
        let req = headers(&mut s);
        s.write_all(b"\
            HTTP/1.1 401 Unauthorized\r\n\
            WWW-Authenticate: Basic realm=\"wheee\"\r\n
            \r\n\
        ").unwrap();
        assert_eq!(req, vec![
            "GET /foo/bar/info/refs?service=git-upload-pack HTTP/1.1",
            "Authorization: Basic Zm9vOmJhcg==",
            "Accept: */*",
            "User-Agent: git/1.0 (libgit2 0.22.0)",
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
                   script.display().to_string().as_slice()).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            git = "http://127.0.0.1:{}/foo/bar"
        "#, addr.port()))
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stdout(format!("\
{updating} git repository `http://{addr}/foo/bar`
",
        updating = UPDATING,
        addr = addr,
        ).as_slice())
                      .with_stderr(format!("\
Unable to update http://{addr}/foo/bar

Caused by:
  failed to clone into: [..]

Caused by:
  [..] status code: 401
",
        addr = addr)));

    t.join().ok().unwrap();
});

// Boy, sure would be nice to have a TLS implementation in rust!
test!(https_something_happens {
    let a = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = a.socket_addr().unwrap();
    let t = thread::spawn(move|| {
        drop(a.accept().unwrap());
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
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stdout(format!("\
{updating} git repository `https://{addr}/foo/bar`
",
        updating = UPDATING,
        addr = addr,
        ).as_slice())
                      .with_stderr(format!("\
Unable to update https://{addr}/foo/bar

Caused by:
  failed to clone into: [..]

Caused by:
  {errmsg}
",
        addr = addr,
        errmsg = if cfg!(windows) {
            "[[..]] failed to send request: The connection with the server \
             was terminated abnormally\n"
        } else {
            "[[..]] SSL error: [..]"
        })));

    t.join().ok().unwrap();
});

// Boy, sure would be nice to have an SSH implementation in rust!
test!(ssh_something_happens {
    let a = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = a.socket_addr().unwrap();
    let t = thread::spawn(move|| {
        drop(a.accept().unwrap());
    });

    let p = project("foo")
        .file("Cargo.toml", format!(r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            git = "ssh://127.0.0.1:{}/foo/bar"
        "#, addr.port()).as_slice())
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stdout(format!("\
{updating} git repository `ssh://{addr}/foo/bar`
",
        updating = UPDATING,
        addr = addr,
        ).as_slice())
                      .with_stderr(format!("\
Unable to update ssh://{addr}/foo/bar

Caused by:
  failed to clone into: [..]

Caused by:
  [[..]] Failed to start SSH session: Failed getting banner
",
        addr = addr)));
    t.join().ok().unwrap();
});
