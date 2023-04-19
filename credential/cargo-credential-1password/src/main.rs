//! Cargo registry 1password credential process.

use cargo_credential::{Credential, Error};
use serde::Deserialize;
use std::io::Read;
use std::process::{Command, Stdio};

const CARGO_TAG: &str = "cargo-registry";

/// Implementation of 1password keychain access for Cargo registries.
struct OnePasswordKeychain {
    account: Option<String>,
    vault: Option<String>,
}

/// 1password Login item type, used for the JSON output of `op item get`.
#[derive(Deserialize)]
struct Login {
    fields: Vec<Field>,
}

#[derive(Deserialize)]
struct Field {
    id: String,
    value: Option<String>,
}

/// 1password item from `op items list`.
#[derive(Deserialize)]
struct ListItem {
    id: String,
    urls: Vec<Url>,
}

#[derive(Deserialize)]
struct Url {
    href: String,
}

impl OnePasswordKeychain {
    fn new() -> Result<OnePasswordKeychain, Error> {
        let mut args = std::env::args().skip(1);
        let mut action = false;
        let mut account = None;
        let mut vault = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--account" => {
                    account = Some(args.next().ok_or("--account needs an arg")?);
                }
                "--vault" => {
                    vault = Some(args.next().ok_or("--vault needs an arg")?);
                }
                s if s.starts_with('-') => {
                    return Err(format!("unknown option {}", s).into());
                }
                _ => {
                    if action {
                        return Err("too many arguments".into());
                    } else {
                        action = true;
                    }
                }
            }
        }
        Ok(OnePasswordKeychain { account, vault })
    }

    fn signin(&self) -> Result<Option<String>, Error> {
        // If there are any session env vars, we'll assume that this is the
        // correct account, and that the user knows what they are doing.
        if std::env::vars().any(|(name, _)| name.starts_with("OP_SESSION_")) {
            return Ok(None);
        }
        let mut cmd = Command::new("op");
        cmd.args(&["signin", "--raw"]);
        cmd.stdout(Stdio::piped());
        self.with_tty(&mut cmd)?;
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn `op`: {}", e))?;
        let mut buffer = String::new();
        child
            .stdout
            .as_mut()
            .unwrap()
            .read_to_string(&mut buffer)
            .map_err(|e| format!("failed to get session from `op`: {}", e))?;
        if let Some(end) = buffer.find('\n') {
            buffer.truncate(end);
        }
        let status = child
            .wait()
            .map_err(|e| format!("failed to wait for `op`: {}", e))?;
        if !status.success() {
            return Err(format!("failed to run `op signin`: {}", status).into());
        }
        if buffer.is_empty() {
            // When using CLI integration, `op signin` returns no output,
            // so there is no need to set the session.
            return Ok(None);
        }
        Ok(Some(buffer))
    }

    fn make_cmd(&self, session: &Option<String>, args: &[&str]) -> Command {
        let mut cmd = Command::new("op");
        cmd.args(args);
        if let Some(account) = &self.account {
            cmd.arg("--account");
            cmd.arg(account);
        }
        if let Some(vault) = &self.vault {
            cmd.arg("--vault");
            cmd.arg(vault);
        }
        if let Some(session) = session {
            cmd.arg("--session");
            cmd.arg(session);
        }
        cmd
    }

    fn with_tty(&self, cmd: &mut Command) -> Result<(), Error> {
        #[cfg(unix)]
        const IN_DEVICE: &str = "/dev/tty";
        #[cfg(windows)]
        const IN_DEVICE: &str = "CONIN$";
        let stdin = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(IN_DEVICE)?;
        cmd.stdin(stdin);
        Ok(())
    }

    fn run_cmd(&self, mut cmd: Command) -> Result<String, Error> {
        cmd.stdout(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn `op`: {}", e))?;
        let mut buffer = String::new();
        child
            .stdout
            .as_mut()
            .unwrap()
            .read_to_string(&mut buffer)
            .map_err(|e| format!("failed to read `op` output: {}", e))?;
        let status = child
            .wait()
            .map_err(|e| format!("failed to wait for `op`: {}", e))?;
        if !status.success() {
            return Err(format!("`op` command exit error: {}", status).into());
        }
        Ok(buffer)
    }

    fn search(&self, session: &Option<String>, index_url: &str) -> Result<Option<String>, Error> {
        let cmd = self.make_cmd(
            session,
            &[
                "items",
                "list",
                "--categories",
                "Login",
                "--tags",
                CARGO_TAG,
                "--format",
                "json",
            ],
        );
        let buffer = self.run_cmd(cmd)?;
        let items: Vec<ListItem> = serde_json::from_str(&buffer)
            .map_err(|e| format!("failed to deserialize JSON from 1password list: {}", e))?;
        let mut matches = items
            .into_iter()
            .filter(|item| item.urls.iter().any(|url| url.href == index_url));
        match matches.next() {
            Some(login) => {
                // Should this maybe just sort on `updatedAt` and return the newest one?
                if matches.next().is_some() {
                    return Err(format!(
                        "too many 1password logins match registry `{}`, \
                        consider deleting the excess entries",
                        index_url
                    )
                    .into());
                }
                Ok(Some(login.id))
            }
            None => Ok(None),
        }
    }

    fn modify(
        &self,
        session: &Option<String>,
        id: &str,
        token: &str,
        _name: Option<&str>,
    ) -> Result<(), Error> {
        let cmd = self.make_cmd(
            session,
            &["item", "edit", id, &format!("password={}", token)],
        );
        self.run_cmd(cmd)?;
        Ok(())
    }

    fn create(
        &self,
        session: &Option<String>,
        index_url: &str,
        token: &str,
        name: Option<&str>,
    ) -> Result<(), Error> {
        let title = match name {
            Some(name) => format!("Cargo registry token for {}", name),
            None => "Cargo registry token".to_string(),
        };
        let mut cmd = self.make_cmd(
            session,
            &[
                "item",
                "create",
                "--category",
                "Login",
                &format!("password={}", token),
                &format!("url={}", index_url),
                "--title",
                &title,
                "--tags",
                CARGO_TAG,
            ],
        );
        // For unknown reasons, `op item create` seems to not be happy if
        // stdin is not a tty. Otherwise it returns with a 0 exit code without
        // doing anything.
        self.with_tty(&mut cmd)?;
        self.run_cmd(cmd)?;
        Ok(())
    }

    fn get_token(&self, session: &Option<String>, id: &str) -> Result<String, Error> {
        let cmd = self.make_cmd(session, &["item", "get", "--format=json", id]);
        let buffer = self.run_cmd(cmd)?;
        let item: Login = serde_json::from_str(&buffer)
            .map_err(|e| format!("failed to deserialize JSON from 1password get: {}", e))?;
        let password = item.fields.into_iter().find(|item| item.id == "password");
        match password {
            Some(password) => password
                .value
                .ok_or_else(|| format!("missing password value for entry").into()),
            None => Err("could not find password field".into()),
        }
    }

    fn delete(&self, session: &Option<String>, id: &str) -> Result<(), Error> {
        let cmd = self.make_cmd(session, &["item", "delete", id]);
        self.run_cmd(cmd)?;
        Ok(())
    }
}

impl Credential for OnePasswordKeychain {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn get(&self, index_url: &str) -> Result<String, Error> {
        let session = self.signin()?;
        if let Some(id) = self.search(&session, index_url)? {
            self.get_token(&session, &id)
        } else {
            return Err(format!(
                "no 1password entry found for registry `{}`, try `cargo login` to add a token",
                index_url
            )
            .into());
        }
    }

    fn store(&self, index_url: &str, token: &str, name: Option<&str>) -> Result<(), Error> {
        let session = self.signin()?;
        // Check if an item already exists.
        if let Some(id) = self.search(&session, index_url)? {
            self.modify(&session, &id, token, name)
        } else {
            self.create(&session, index_url, token, name)
        }
    }

    fn erase(&self, index_url: &str) -> Result<(), Error> {
        let session = self.signin()?;
        // Check if an item already exists.
        if let Some(id) = self.search(&session, index_url)? {
            self.delete(&session, &id)?;
        } else {
            eprintln!("not currently logged in to `{}`", index_url);
        }
        Ok(())
    }
}

fn main() {
    let op = match OnePasswordKeychain::new() {
        Ok(op) => op,
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    };
    cargo_credential::main(op);
}
