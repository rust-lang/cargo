//! Tests for git authentication.

use std::collections::HashSet;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use cargo_test_support::paths;
use cargo_test_support::{basic_manifest, project};

fn setup_failed_auth_test() -> (SocketAddr, JoinHandle<()>, Arc<AtomicUsize>) {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    fn headers(rdr: &mut dyn BufRead) -> HashSet<String> {
        let valid = ["GET", "Authorization", "Accept"];
        rdr.lines()
            .map(|s| s.unwrap())
            .take_while(|s| s.len() > 2)
            .map(|s| s.trim().to_string())
            .filter(|s| valid.iter().any(|prefix| s.starts_with(*prefix)))
            .collect()
    }

    let connections = Arc::new(AtomicUsize::new(0));
    let connections2 = connections.clone();
    let t = thread::spawn(move || {
        let mut conn = BufReader::new(server.accept().unwrap().0);
        let req = headers(&mut conn);
        connections2.fetch_add(1, SeqCst);
        conn.get_mut()
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\n\
              WWW-Authenticate: Basic realm=\"wheee\"\r\n\
              Content-Length: 0\r\n\
              \r\n",
            )
            .unwrap();
        assert_eq!(
            req,
            vec![
                "GET /foo/bar/info/refs?service=git-upload-pack HTTP/1.1",
                "Accept: */*",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect()
        );

        let req = headers(&mut conn);
        connections2.fetch_add(1, SeqCst);
        conn.get_mut()
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\n\
              WWW-Authenticate: Basic realm=\"wheee\"\r\n\
              \r\n",
            )
            .unwrap();
        assert_eq!(
            req,
            vec![
                "GET /foo/bar/info/refs?service=git-upload-pack HTTP/1.1",
                "Authorization: Basic Zm9vOmJhcg==",
                "Accept: */*",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect()
        );
    });

    let script = project()
        .at("script")
        .file("Cargo.toml", &basic_manifest("script", "0.1.0"))
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    println!("username=foo");
                    println!("password=bar");
                }
            "#,
        )
        .build();

    script.cargo("build -v").run();
    let script = script.bin("script");

    let config = paths::home().join(".gitconfig");
    let mut config = git2::Config::open(&config).unwrap();
    config
        .set_str(
            "credential.helper",
            // This is a bash script so replace `\` with `/` for Windows
            &script.display().to_string().replace("\\", "/"),
        )
        .unwrap();
    (addr, t, connections)
}

// Tests that HTTP auth is offered from `credential.helper`.
#[cargo_test]
fn http_auth_offered() {
    let (addr, t, connections) = setup_failed_auth_test();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies.bar]
                    git = "http://127.0.0.1:{}/foo/bar"
                "#,
                addr.port()
            ),
        )
        .file("src/main.rs", "")
        .file(
            ".cargo/config",
            "[net]
             retry = 0
            ",
        )
        .build();

    // This is a "contains" check because the last error differs by platform,
    // may span multiple lines, and isn't relevant to this test.
    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(&format!(
            "\
[UPDATING] git repository `http://{addr}/foo/bar`
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 [..]`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update http://{addr}/foo/bar

Caused by:
  failed to clone into: [..]

Caused by:
  failed to authenticate when downloading repository

  * attempted to find username/password via `credential.helper`, but [..]

  if the git CLI succeeds then `net.git-fetch-with-cli` may help here
  https://[..]

Caused by:
",
            addr = addr
        ))
        .run();

    assert_eq!(connections.load(SeqCst), 2);
    t.join().ok().unwrap();
}

// Boy, sure would be nice to have a TLS implementation in rust!
#[cargo_test]
fn https_something_happens() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let t = thread::spawn(move || {
        let mut conn = server.accept().unwrap().0;
        drop(conn.write(b"1234"));
        drop(conn.shutdown(std::net::Shutdown::Write));
        drop(conn.read(&mut [0; 16]));
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies.bar]
                    git = "https://127.0.0.1:{}/foo/bar"
                "#,
                addr.port()
            ),
        )
        .file("src/main.rs", "")
        .file(
            ".cargo/config",
            "[net]
             retry = 0
            ",
        )
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains(&format!(
            "[UPDATING] git repository `https://{addr}/foo/bar`",
            addr = addr
        ))
        .with_stderr_contains(&format!(
            "\
Caused by:
  {errmsg}
",
            errmsg = if cfg!(windows) {
                "[..]failed to send request: [..]"
            } else if cfg!(target_os = "macos") {
                // macOS is difficult to tests as some builds may use Security.framework,
                // while others may use OpenSSL. In that case, let's just not verify the error
                // message here.
                "[..]"
            } else {
                "[..]SSL error: [..]"
            }
        ))
        .run();

    t.join().ok().unwrap();
}

// It would sure be nice to have an SSH implementation in Rust!
#[cargo_test]
fn ssh_something_happens() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let t = thread::spawn(move || {
        drop(server.accept().unwrap());
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies.bar]
                    git = "ssh://127.0.0.1:{}/foo/bar"
                "#,
                addr.port()
            ),
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains(&format!(
            "[UPDATING] git repository `ssh://{addr}/foo/bar`",
            addr = addr
        ))
        .with_stderr_contains(
            "\
Caused by:
  [..]failed to start SSH session: Failed getting banner[..]
",
        )
        .run();
    t.join().ok().unwrap();
}

#[cargo_test]
fn net_err_suggests_fetch_with_cli() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []

                [dependencies]
                foo = { git = "ssh://needs-proxy.invalid/git" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] git repository `ssh://needs-proxy.invalid/git`
warning: spurious network error[..]
warning: spurious network error[..]
[ERROR] failed to get `foo` as a dependency of package `foo v0.0.0 [..]`

Caused by:
  failed to load source for dependency `foo`

Caused by:
  Unable to update ssh://needs-proxy.invalid/git

Caused by:
  failed to clone into: [..]

Caused by:
  network failure seems to have happened
  if a proxy or similar is necessary `net.git-fetch-with-cli` may help here
  https://[..]

Caused by:
  failed to resolve address for needs-proxy.invalid[..]
",
        )
        .run();

    p.change_file(
        ".cargo/config",
        "
            [net]
            git-fetch-with-cli = true
            ",
    );

    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains("[..]Unable to update[..]")
        .with_stderr_does_not_contain("[..]try enabling `git-fetch-with-cli`[..]")
        .run();
}

#[cargo_test]
fn instead_of_url_printed() {
    let (addr, t, _connections) = setup_failed_auth_test();
    let config = paths::home().join(".gitconfig");
    let mut config = git2::Config::open(&config).unwrap();
    config
        .set_str(
            &format!("url.http://{}/.insteadOf", addr),
            "https://foo.bar/",
        )
        .unwrap();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                git = "https://foo.bar/foo/bar"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(&format!(
            "\
[UPDATING] git repository `https://foo.bar/foo/bar`
[ERROR] failed to get `bar` as a dependency of package `foo [..]`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update https://foo.bar/foo/bar

Caused by:
  failed to clone into: [..]

Caused by:
  failed to authenticate when downloading repository: http://{addr}/foo/bar

  * attempted to find username/password via `credential.helper`, but maybe the found credentials were incorrect

  if the git CLI succeeds then `net.git-fetch-with-cli` may help here
  https://[..]

Caused by:
  [..]
",
            addr = addr
        ))
        .run();

    t.join().ok().unwrap();
}
