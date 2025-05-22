//! Tests for git authentication.

use std::collections::HashSet;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use cargo_test_support::basic_manifest;
use cargo_test_support::git::cargo_uses_gitoxide;
use cargo_test_support::paths;
use cargo_test_support::prelude::*;
use cargo_test_support::project;

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
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies.bar]
                    git = "http://127.0.0.1:{}/foo/bar"
                "#,
                addr.port()
            ),
        )
        .file("src/main.rs", "")
        .file(
            ".cargo/config.toml",
            "[net]
             retry = 0
            ",
        )
        .build();

    // This is a "contains" check because the last error differs by platform,
    // may span multiple lines, and isn't relevant to this test.
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(&format!(
            "\
[UPDATING] git repository `http://{addr}/foo/bar`
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update http://{addr}/foo/bar

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/bar-[HASH]

Caused by:
  failed to authenticate when downloading repository

  * attempted to find username/password via `credential.helper`, but maybe the found credentials were incorrect

  if the git CLI succeeds then `net.git-fetch-with-cli` may help here
  https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli

Caused by:
{trailer}
",
            trailer = if cargo_uses_gitoxide() {
              format!(r#"[CREDENTIAL]s provided for "http://{addr}/foo/bar" were not accepted by the remote

Caused by:
  Received HTTP status 401"#)
            } else {
              "  no authentication methods succeeded".to_string()
            }
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
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies.bar]
                    git = "https://127.0.0.1:{}/foo/bar"
                "#,
                addr.port()
            ),
        )
        .file("src/main.rs", "")
        .file(
            ".cargo/config.toml",
            "[net]
             retry = 0
            ",
        )
        .build();

    p.cargo("check -v")
        .with_status(101)
        .with_stderr_data(&format!(
            "\
[UPDATING] git repository `https://{addr}/foo/bar`
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update https://{addr}/foo/bar

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/bar-[HASH]

Caused by:
{errmsg}
",
            errmsg = if cargo_uses_gitoxide() {
                r"  network failure seems to have happened
  if a proxy or similar is necessary `net.git-fetch-with-cli` may help here
  https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli

Caused by:
  An IO error occurred when talking to the server

Caused by:
  [35] SSL connect error ([..])"
            } else if cfg!(windows) {
                "[..]failed to send request: [..]\n..."
            } else if cfg!(target_os = "macos") {
                // macOS is difficult to tests as some builds may use Security.framework,
                // while others may use OpenSSL. In that case, let's just not verify the error
                // message here.
                "..."
            } else {
                "[..]SSL [ERROR][..]"
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
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies.bar]
                    git = "ssh://127.0.0.1:{}/foo/bar"
                "#,
                addr.port()
            ),
        )
        .file("src/main.rs", "")
        .build();

    let expected = if cargo_uses_gitoxide() {
        // Due to the usage of `ssh` and `ssh.exe` respectively, the messages change.
        // This will be adjusted to use `ssh2` to get rid of this dependency and have uniform messaging.
        let message = if cfg!(windows) {
            // The order of multiple possible messages isn't deterministic within `ssh`, and `gitoxide` detects both
            // but gets to report only the first. Thus this test can flip-flop from one version of the error to the other
            // and we can't test for that.
            // We'd want to test for:
            // "[..]ssh: connect to host 127.0.0.1 [..]"
            //   ssh: connect to host example.org port 22: No route to host
            // "[..]banner exchange: Connection to 127.0.0.1 [..]"
            //   banner exchange: Connection to 127.0.0.1 port 62250: Software caused connection abort
            // But since there is no common meaningful sequence or word, we can only match a small telling sequence of characters.
            "[..]onnect[..]"
        } else {
            "[..]Connection [..] by [..]"
        };
        format!(
            "\
[UPDATING] git repository `ssh://{addr}/foo/bar`
...
{message}
...
"
        )
    } else {
        format!(
            "\
[UPDATING] git repository `ssh://{addr}/foo/bar`
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update ssh://{addr}/foo/bar

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/bar-[HASH]

Caused by:
  network failure seems to have happened
  if a proxy or similar is necessary `net.git-fetch-with-cli` may help here
  https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli

Caused by:
  failed to start SSH session: Failed getting banner; class=Ssh (23)
"
        )
    };
    p.cargo("check -v")
        .with_status(101)
        .with_stderr_data(expected)
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
                edition = "2015"
                authors = []

                [dependencies]
                foo = { git = "ssh://needs-proxy.invalid/git" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -v")
        .with_status(101)
        .with_stderr_data(format!(
            "\
[UPDATING] git repository `ssh://needs-proxy.invalid/git`
[WARNING] spurious network error (3 tries remaining): [..] resolve [..] needs-proxy.invalid: [..] known[..]
[WARNING] spurious network error (2 tries remaining): [..] resolve [..] needs-proxy.invalid: [..] known[..]
[WARNING] spurious network error (1 try remaining): [..] resolve [..] needs-proxy.invalid: [..] known[..]
[ERROR] failed to get `foo` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `foo`

Caused by:
  Unable to update ssh://needs-proxy.invalid/git

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/git-[HASH]

Caused by:
  network failure seems to have happened
  if a proxy or similar is necessary `net.git-fetch-with-cli` may help here
  https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli

Caused by:
{trailer}
",
            trailer = if cargo_uses_gitoxide() {
                r"  An IO error occurred when talking to the server

Caused by:
  ssh: Could not resolve hostname needs-proxy.invalid[..]"
            } else {
                "  failed to resolve address for needs-proxy.invalid: [..] known[..]; class=Net (12)"
            }
        ))
        .run();

    p.change_file(
        ".cargo/config.toml",
        "
            [net]
            git-fetch-with-cli = true
            ",
    );

    p.cargo("check -v")
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
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                git = "https://foo.bar/foo/bar"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(&format!(
            "\
[UPDATING] git repository `https://foo.bar/foo/bar`
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update https://foo.bar/foo/bar

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/bar-[HASH]

Caused by:
  failed to authenticate when downloading repository: http://{addr}/foo/bar

  * attempted to find username/password via `credential.helper`, but maybe the found credentials were incorrect

  if the git CLI succeeds then `net.git-fetch-with-cli` may help here
  https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli

Caused by:
...
"
        ))
        .run();

    t.join().ok().unwrap();
}
