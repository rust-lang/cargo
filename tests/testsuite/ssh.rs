//! Network tests for SSH connections.
//!
//! Note that these tests will generally require setting CARGO_CONTAINER_TESTS
//! or CARGO_PUBLIC_NETWORK_TESTS.
//!
//! NOTE: The container tests almost certainly won't work on Windows.

use cargo_test_support::containers::{Container, ContainerHandle, MkFile};
use cargo_test_support::git::cargo_uses_gitoxide;
use cargo_test_support::{paths, process, project, Project};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn ssh_repo_url(container: &ContainerHandle, name: &str) -> String {
    let port = container.port_mappings[&22];
    format!("ssh://testuser@127.0.0.1:{port}/repos/{name}.git")
}

/// The path to the client's private key.
fn key_path() -> PathBuf {
    paths::home().join(".ssh/id_ed25519")
}

/// Generates the SSH keys for authenticating into the container.
fn gen_ssh_keys() -> String {
    let path = key_path();
    process("ssh-keygen")
        .args(&["-t", "ed25519", "-N", "", "-f"])
        .arg(&path)
        .exec_with_output()
        .unwrap();
    let pub_key = path.with_extension("pub");
    fs::read_to_string(pub_key).unwrap()
}

/// Handler for running ssh-agent for SSH authentication.
///
/// Be sure to set `SSH_AUTH_SOCK` when running a process in order to use the
/// agent. Keys will need to be copied into the container with the
/// `authorized_keys()` method.
struct Agent {
    sock: PathBuf,
    pid: String,
    ssh_dir: PathBuf,
    pub_key: String,
}

impl Agent {
    fn launch() -> Agent {
        let ssh_dir = paths::home().join(".ssh");
        fs::create_dir(&ssh_dir).unwrap();
        let pub_key = gen_ssh_keys();

        let sock = paths::root().join("agent");
        let output = process("ssh-agent")
            .args(&["-s", "-a"])
            .arg(&sock)
            .exec_with_output()
            .unwrap();
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        let start = stdout.find("SSH_AGENT_PID=").unwrap() + 14;
        let end = &stdout[start..].find(';').unwrap();
        let pid = (&stdout[start..start + end]).to_string();
        eprintln!("SSH_AGENT_PID={pid}");
        process("ssh-add")
            .arg(key_path())
            .env("SSH_AUTH_SOCK", &sock)
            .exec_with_output()
            .unwrap();
        Agent {
            sock,
            pid,
            ssh_dir,
            pub_key,
        }
    }

    /// Returns a `MkFile` which can be passed into the `Container` builder to
    /// copy an `authorized_keys` file containing this agent's public key.
    fn authorized_keys(&self) -> MkFile {
        MkFile::path("home/testuser/.ssh/authorized_keys")
            .contents(self.pub_key.as_bytes())
            .mode(0o600)
            .uid(100)
            .gid(101)
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        if let Err(e) = process("ssh-agent")
            .args(&["-k", "-a"])
            .arg(&self.sock)
            .env("SSH_AGENT_PID", &self.pid)
            .exec_with_output()
        {
            eprintln!("failed to stop ssh-agent: {e:?}");
        }
    }
}

/// Common project used for several tests.
fn foo_bar_project(url: &str) -> Project {
    project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = {{ git = "{url}" }}
            "#
            ),
        )
        .file("src/lib.rs", "")
        .build()
}

#[cargo_test(container_test)]
fn no_known_host() {
    // When host is not known, it should show an error.
    let sshd = Container::new("sshd").launch();
    let url = ssh_repo_url(&sshd, "bar");
    let p = foo_bar_project(&url);
    p.cargo("fetch")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] git repository `ssh://testuser@127.0.0.1:[..]/repos/bar.git`
error: failed to get `bar` as a dependency of package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update ssh://testuser@127.0.0.1:[..]/repos/bar.git

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/bar-[..]

Caused by:
  error: unknown SSH host key
  The SSH host key for `[127.0.0.1]:[..]` is not known and cannot be validated.

  To resolve this issue, add the host key to the `net.ssh.known-hosts` array in \
  your Cargo configuration (such as [ROOT]/home/.cargo/config.toml) or in your \
  OpenSSH known_hosts file at [ROOT]/home/.ssh/known_hosts

  The key to add is:

  [127.0.0.1]:[..] ecdsa-sha2-nistp256 AAAA[..]

  The ECDSA key fingerprint is: SHA256:[..]
  This fingerprint should be validated with the server administrator that it is correct.

  See https://doc.rust-lang.org/stable/cargo/appendix/git-authentication.html#ssh-known-hosts \
  for more information.
",
        )
        .run();
}

#[cargo_test(container_test)]
fn known_host_works() {
    // The key displayed in the error message should work when added to known_hosts.
    let agent = Agent::launch();
    let sshd = Container::new("sshd")
        .file(agent.authorized_keys())
        .launch();
    let url = ssh_repo_url(&sshd, "bar");
    let p = foo_bar_project(&url);
    let output = p
        .cargo("fetch")
        .env("SSH_AUTH_SOCK", &agent.sock)
        .build_command()
        .output()
        .unwrap();
    let stderr = std::str::from_utf8(&output.stderr).unwrap();

    // Validate the fingerprint while we're here.
    let fingerprint = stderr
        .lines()
        .find_map(|line| line.strip_prefix("  The ECDSA key fingerprint is: "))
        .unwrap()
        .trim();
    let finger_out = sshd.exec(&["ssh-keygen", "-l", "-f", "/etc/ssh/ssh_host_ecdsa_key.pub"]);
    let gen_finger = std::str::from_utf8(&finger_out.stdout).unwrap();
    // <key-size> <fingerprint> <comments…>
    let gen_finger = gen_finger.split_whitespace().nth(1).unwrap();
    assert_eq!(fingerprint, gen_finger);

    // Add the key to known_hosts, and try again.
    let key = stderr
        .lines()
        .find(|line| line.starts_with("  [127.0.0.1]:"))
        .unwrap()
        .trim();
    fs::write(agent.ssh_dir.join("known_hosts"), key).unwrap();
    p.cargo("fetch")
        .env("SSH_AUTH_SOCK", &agent.sock)
        .with_stderr("[UPDATING] git repository `ssh://testuser@127.0.0.1:[..]/repos/bar.git`")
        .run();
}

#[cargo_test(container_test)]
fn same_key_different_hostname() {
    // The error message should mention if an identical key was found.
    let agent = Agent::launch();
    let sshd = Container::new("sshd").launch();

    let hostkey = sshd.read_file("/etc/ssh/ssh_host_ecdsa_key.pub");
    let known_hosts = format!("example.com {hostkey}");
    fs::write(agent.ssh_dir.join("known_hosts"), known_hosts).unwrap();

    let url = ssh_repo_url(&sshd, "bar");
    let p = foo_bar_project(&url);
    p.cargo("fetch")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] git repository `ssh://testuser@127.0.0.1:[..]/repos/bar.git`
error: failed to get `bar` as a dependency of package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update ssh://testuser@127.0.0.1:[..]/repos/bar.git

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/bar-[..]

Caused by:
  error: unknown SSH host key
  The SSH host key for `[127.0.0.1]:[..]` is not known and cannot be validated.

  To resolve this issue, add the host key to the `net.ssh.known-hosts` array in \
  your Cargo configuration (such as [ROOT]/home/.cargo/config.toml) or in your \
  OpenSSH known_hosts file at [ROOT]/home/.ssh/known_hosts

  The key to add is:

  [127.0.0.1]:[..] ecdsa-sha2-nistp256 AAAA[..]

  The ECDSA key fingerprint is: SHA256:[..]
  This fingerprint should be validated with the server administrator that it is correct.
  Note: This host key was found, but is associated with a different host:
      [ROOT]/home/.ssh/known_hosts line 1: example.com

  See https://doc.rust-lang.org/stable/cargo/appendix/git-authentication.html#ssh-known-hosts \
  for more information.
",
        )
        .run();
}

#[cargo_test(container_test)]
fn known_host_without_port() {
    // A known_host entry without a port should match a connection to a non-standard port.
    let agent = Agent::launch();
    let sshd = Container::new("sshd")
        .file(agent.authorized_keys())
        .launch();

    let hostkey = sshd.read_file("/etc/ssh/ssh_host_ecdsa_key.pub");
    // The important part of this test is that this line does not have a port.
    let known_hosts = format!("127.0.0.1 {hostkey}");
    fs::write(agent.ssh_dir.join("known_hosts"), known_hosts).unwrap();
    let url = ssh_repo_url(&sshd, "bar");
    let p = foo_bar_project(&url);
    p.cargo("fetch")
        .env("SSH_AUTH_SOCK", &agent.sock)
        .with_stderr("[UPDATING] git repository `ssh://testuser@127.0.0.1:[..]/repos/bar.git`")
        .run();
}

#[cargo_test(container_test)]
fn hostname_case_insensitive() {
    // hostname checking should be case-insensitive.
    let agent = Agent::launch();
    let sshd = Container::new("sshd")
        .file(agent.authorized_keys())
        .launch();

    // Consider using `gethostname-rs` instead?
    let hostname = process("hostname").exec_with_output().unwrap();
    let hostname = std::str::from_utf8(&hostname.stdout).unwrap().trim();
    let inv_hostname = if hostname.chars().any(|c| c.is_lowercase()) {
        hostname.to_uppercase()
    } else {
        // There should be *some* chars in the name.
        assert!(hostname.chars().any(|c| c.is_uppercase()));
        hostname.to_lowercase()
    };
    eprintln!("converted {hostname} to {inv_hostname}");

    let hostkey = sshd.read_file("/etc/ssh/ssh_host_ecdsa_key.pub");
    let known_hosts = format!("{inv_hostname} {hostkey}");
    fs::write(agent.ssh_dir.join("known_hosts"), known_hosts).unwrap();
    let port = sshd.port_mappings[&22];
    let url = format!("ssh://testuser@{hostname}:{port}/repos/bar.git");
    let p = foo_bar_project(&url);
    p.cargo("fetch")
        .env("SSH_AUTH_SOCK", &agent.sock)
        .with_stderr(&format!(
            "[UPDATING] git repository `ssh://testuser@{hostname}:{port}/repos/bar.git`"
        ))
        .run();
}

#[cargo_test(container_test)]
fn invalid_key_error() {
    // An error when a known_host value doesn't match.
    let agent = Agent::launch();
    let sshd = Container::new("sshd")
        .file(agent.authorized_keys())
        .launch();

    let port = sshd.port_mappings[&22];
    let known_hosts = format!(
        "[127.0.0.1]:{port} ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBLqLMclVr7MDuaVsm3sEnnq2OrGxTFiHSw90wd6N14BU8xVC9cZldC3rJ58Wmw6bEVKPjk7foNG0lHwS5bCKX+U=\n"
    );
    fs::write(agent.ssh_dir.join("known_hosts"), known_hosts).unwrap();
    let url = ssh_repo_url(&sshd, "bar");
    let p = foo_bar_project(&url);
    p.cargo("fetch")
        .env("SSH_AUTH_SOCK", &agent.sock)
        .with_status(101)
        .with_stderr(&format!("\
[UPDATING] git repository `ssh://testuser@127.0.0.1:{port}/repos/bar.git`
error: failed to get `bar` as a dependency of package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update ssh://testuser@127.0.0.1:{port}/repos/bar.git

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/bar-[..]

Caused by:
  error: SSH host key has changed for `[127.0.0.1]:{port}`
  *********************************
  * WARNING: HOST KEY HAS CHANGED *
  *********************************
  This may be caused by a man-in-the-middle attack, or the server may have changed its host key.

  The ECDSA fingerprint for the key from the remote host is:
  SHA256:[..]

  You are strongly encouraged to contact the server administrator for `[127.0.0.1]:{port}` \
  to verify that this new key is correct.

  If you can verify that the server has a new key, you can resolve this error by \
  removing the old ecdsa-sha2-nistp256 key for `[127.0.0.1]:{port}` located at \
  [ROOT]/home/.ssh/known_hosts line 1, and adding the new key to the \
  `net.ssh.known-hosts` array in your Cargo configuration (such as \
  [ROOT]/home/.cargo/config.toml) or in your OpenSSH known_hosts file at \
  [ROOT]/home/.ssh/known_hosts

  The key provided by the remote host is:

  [127.0.0.1]:{port} ecdsa-sha2-nistp256 [..]

  See https://doc.rust-lang.org/stable/cargo/appendix/git-authentication.html#ssh-known-hosts for more information.
"))
        .run();
    // Add the key, it should work even with the old key left behind.
    let hostkey = sshd.read_file("/etc/ssh/ssh_host_ecdsa_key.pub");
    let known_hosts_path = agent.ssh_dir.join("known_hosts");
    let mut f = fs::OpenOptions::new()
        .append(true)
        .open(known_hosts_path)
        .unwrap();
    write!(f, "[127.0.0.1]:{port} {hostkey}").unwrap();
    drop(f);
    p.cargo("fetch")
        .env("SSH_AUTH_SOCK", &agent.sock)
        .with_stderr("[UPDATING] git repository `ssh://testuser@127.0.0.1:[..]/repos/bar.git`")
        .run();
}

// For unknown reasons, this test occasionally fails on Windows with a
// LIBSSH2_ERROR_KEY_EXCHANGE_FAILURE error:
//     failed to start SSH session: Unable to exchange encryption keys; class=Ssh (23)
#[cargo_test(public_network_test, ignore_windows = "test is flaky on windows")]
fn invalid_github_key() {
    // A key for github.com in known_hosts should override the built-in key.
    // This uses a bogus key which should result in an error.
    let ssh_dir = paths::home().join(".ssh");
    fs::create_dir(&ssh_dir).unwrap();
    let known_hosts = "\
        github.com ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBLqLMclVr7MDuaVsm3sEnnq2OrGxTFiHSw90wd6N14BU8xVC9cZldC3rJ58Wmw6bEVKPjk7foNG0lHwS5bCKX+U=\n\
        github.com ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQDgi+8rMcyFCBq5y7BXrb2aaYGhMjlU3QDy7YDvtNL5KSecYOsaqQHaXr87Bbx0EEkgbhK4kVMkmThlCoNITQS9Vc3zIMQ+Tg6+O4qXx719uCzywl50Tb5tDqPGMj54jcq3VUiu/dvse0yeehyvzoPNWewgGWLx11KI4A4wOwMnc6guhculEWe9DjGEjUQ34lPbmdfu/Hza7ZVu/RhgF/wc43uzXWB2KpMEqtuY1SgRlCZqTASoEtfKZi0AuM7AEdOwE5aTotS4CQZHWimb1bMFpF4DAq92CZ8Jhrm4rWETbO29WmjviCJEA3KNQyd3oA7H9AE9z/22PJaVEmjiZZ+wyLgwyIpOlsnHYNEdGeQMQ4SgLRkARLwcnKmByv1AAxsBW4LI3Os4FpwxVPdXHcBebydtvxIsbtUVkkq99nbsIlnSRFSTvb0alrdzRuKTdWpHtN1v9hagFqmeCx/kJfH76NXYBbtaWZhSOnxfEbhLYuOb+IS4jYzHAIkzy9FjVuk=\n\
        ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEeMB6BUAW6FfvfLxRO3kGASe0yXnrRT4kpqncsup2b2\n";
    fs::write(ssh_dir.join("known_hosts"), known_hosts).unwrap();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bitflags = { git = "ssh://git@github.com/rust-lang/bitflags.git", tag = "1.3.2" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .with_status(101)
        .with_stderr_contains(if cargo_uses_gitoxide() {
            "  git@github.com: Permission denied (publickey)."
        } else {
            "  error: SSH host key has changed for `github.com`"
        })
        .run();
}

// For unknown reasons, this test occasionally fails on Windows with a
// LIBSSH2_ERROR_KEY_EXCHANGE_FAILURE error:
//     failed to start SSH session: Unable to exchange encryption keys; class=Ssh (23)
#[cargo_test(public_network_test, ignore_windows = "test is flaky on windows")]
fn bundled_github_works() {
    // The bundled key for github.com works.
    //
    // Use a bogus auth sock to force an authentication error.
    // On Windows, if the agent service is running, it could allow a
    // successful authentication.
    //
    // If the bundled hostkey did not work, it would result in an "unknown SSH
    // host key" instead.
    let bogus_auth_sock = paths::home().join("ssh_auth_sock");
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bitflags = { git = "ssh://git@github.com/rust-lang/bitflags.git", tag = "1.3.2" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    let shared_stderr = "\
[UPDATING] git repository `ssh://git@github.com/rust-lang/bitflags.git`
error: failed to get `bitflags` as a dependency of package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bitflags`

Caused by:
  Unable to update ssh://git@github.com/rust-lang/bitflags.git?tag=1.3.2

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/bitflags-[..]

Caused by:
  failed to authenticate when downloading repository

  *";
    let expected = if cargo_uses_gitoxide() {
        format!(
            "{shared_stderr} attempted to find username/password via `credential.helper`, but maybe the found credentials were incorrect

  if the git CLI succeeds then `net.git-fetch-with-cli` may help here
  https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli

Caused by:
  Credentials provided for \"ssh://git@github.com/rust-lang/bitflags.git\" were not accepted by the remote

Caused by:
  git@github.com: Permission denied (publickey).
"
        )
    } else {
        format!(
            "{shared_stderr} attempted ssh-agent authentication, but no usernames succeeded: `git`

  if the git CLI succeeds then `net.git-fetch-with-cli` may help here
  https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli

Caused by:
  no authentication methods succeeded
"
        )
    };
    p.cargo("fetch")
        .env("SSH_AUTH_SOCK", &bogus_auth_sock)
        .with_status(101)
        .with_stderr(&expected)
        .run();

    let shared_stderr = "\
[UPDATING] git repository `ssh://git@github.com:22/rust-lang/bitflags.git`
error: failed to get `bitflags` as a dependency of package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bitflags`

Caused by:
  Unable to update ssh://git@github.com:22/rust-lang/bitflags.git?tag=1.3.2

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/bitflags-[..]

Caused by:
  failed to authenticate when downloading repository

  *";

    let expected = if cargo_uses_gitoxide() {
        format!(
            "{shared_stderr} attempted to find username/password via `credential.helper`, but maybe the found credentials were incorrect

  if the git CLI succeeds then `net.git-fetch-with-cli` may help here
  https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli

Caused by:
  Credentials provided for \"ssh://git@github.com:22/rust-lang/bitflags.git\" were not accepted by the remote

Caused by:
  git@github.com: Permission denied (publickey).
"
        )
    } else {
        format!(
            "{shared_stderr} attempted ssh-agent authentication, but no usernames succeeded: `git`

  if the git CLI succeeds then `net.git-fetch-with-cli` may help here
  https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli

Caused by:
  no authentication methods succeeded
"
        )
    };

    // Explicit :22 should also work with bundled.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bitflags = { git = "ssh://git@github.com:22/rust-lang/bitflags.git", tag = "1.3.2" }
        "#,
    );
    p.cargo("fetch")
        .env("SSH_AUTH_SOCK", &bogus_auth_sock)
        .with_status(101)
        .with_stderr(&expected)
        .run();
}

#[cargo_test(container_test)]
fn ssh_key_in_config() {
    // known_host in config works.
    let agent = Agent::launch();
    let sshd = Container::new("sshd")
        .file(agent.authorized_keys())
        .launch();
    let hostkey = sshd.read_file("/etc/ssh/ssh_host_ecdsa_key.pub");
    let url = ssh_repo_url(&sshd, "bar");
    let p = foo_bar_project(&url);
    p.change_file(
        ".cargo/config.toml",
        &format!(
            r#"
                [net.ssh]
                known-hosts = ['127.0.0.1 {}']
            "#,
            hostkey.trim()
        ),
    );
    p.cargo("fetch")
        .env("SSH_AUTH_SOCK", &agent.sock)
        .with_stderr("[UPDATING] git repository `ssh://testuser@127.0.0.1:[..]/repos/bar.git`")
        .run();
}
