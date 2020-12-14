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
    sign_in_address: Option<String>,
    email: Option<String>,
}

/// 1password Login item type, used for the JSON output of `op get item`.
#[derive(Deserialize)]
struct Login {
    details: Details,
}

#[derive(Deserialize)]
struct Details {
    fields: Vec<Field>,
}

#[derive(Deserialize)]
struct Field {
    designation: String,
    value: String,
}

/// 1password item from `op list items`.
#[derive(Deserialize)]
struct ListItem {
    uuid: String,
    overview: Overview,
}

#[derive(Deserialize)]
struct Overview {
    title: String,
}

impl OnePasswordKeychain {
    fn new() -> Result<OnePasswordKeychain, Error> {
        let mut args = std::env::args().skip(1);
        let mut action = false;
        let mut account = None;
        let mut vault = None;
        let mut sign_in_address = None;
        let mut email = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--account" => {
                    account = Some(args.next().ok_or("--account needs an arg")?);
                }
                "--vault" => {
                    vault = Some(args.next().ok_or("--vault needs an arg")?);
                }
                "--sign-in-address" => {
                    sign_in_address = Some(args.next().ok_or("--sign-in-address needs an arg")?);
                }
                "--email" => {
                    email = Some(args.next().ok_or("--email needs an arg")?);
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
        if sign_in_address.is_none() && email.is_some() {
            return Err("--email requires --sign-in-address".into());
        }
        Ok(OnePasswordKeychain {
            account,
            vault,
            sign_in_address,
            email,
        })
    }

    fn signin(&self) -> Result<Option<String>, Error> {
        // If there are any session env vars, we'll assume that this is the
        // correct account, and that the user knows what they are doing.
        if std::env::vars().any(|(name, _)| name.starts_with("OP_SESSION_")) {
            return Ok(None);
        }
        let mut cmd = Command::new("op");
        cmd.arg("signin");
        if let Some(addr) = &self.sign_in_address {
            cmd.arg(addr);
            if let Some(email) = &self.email {
                cmd.arg(email);
            }
        }
        cmd.arg("--raw");
        cmd.stdout(Stdio::piped());
        #[cfg(unix)]
        const IN_DEVICE: &str = "/dev/tty";
        #[cfg(windows)]
        const IN_DEVICE: &str = "CONIN$";
        let stdin = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(IN_DEVICE)?;
        cmd.stdin(stdin);
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

    fn search(
        &self,
        session: &Option<String>,
        registry_name: &str,
    ) -> Result<Option<String>, Error> {
        let cmd = self.make_cmd(
            session,
            &[
                "list",
                "items",
                "--categories",
                "Login",
                "--tags",
                CARGO_TAG,
            ],
        );
        let buffer = self.run_cmd(cmd)?;
        let items: Vec<ListItem> = serde_json::from_str(&buffer)
            .map_err(|e| format!("failed to deserialize JSON from 1password list: {}", e))?;
        let mut matches = items
            .into_iter()
            .filter(|item| item.overview.title == registry_name);
        match matches.next() {
            Some(login) => {
                // Should this maybe just sort on `updatedAt` and return the newest one?
                if matches.next().is_some() {
                    return Err(format!(
                        "too many 1password logins match registry name {}, \
                        consider deleting the excess entries",
                        registry_name
                    )
                    .into());
                }
                Ok(Some(login.uuid))
            }
            None => Ok(None),
        }
    }

    fn modify(&self, session: &Option<String>, uuid: &str, token: &str) -> Result<(), Error> {
        let cmd = self.make_cmd(
            session,
            &["edit", "item", uuid, &format!("password={}", token)],
        );
        self.run_cmd(cmd)?;
        Ok(())
    }

    fn create(
        &self,
        session: &Option<String>,
        registry_name: &str,
        api_url: &str,
        token: &str,
    ) -> Result<(), Error> {
        let cmd = self.make_cmd(
            session,
            &[
                "create",
                "item",
                "Login",
                &format!("password={}", token),
                &format!("url={}", api_url),
                "--title",
                registry_name,
                "--tags",
                CARGO_TAG,
            ],
        );
        self.run_cmd(cmd)?;
        Ok(())
    }

    fn get_token(&self, session: &Option<String>, uuid: &str) -> Result<String, Error> {
        let cmd = self.make_cmd(session, &["get", "item", uuid]);
        let buffer = self.run_cmd(cmd)?;
        let item: Login = serde_json::from_str(&buffer)
            .map_err(|e| format!("failed to deserialize JSON from 1password get: {}", e))?;
        let password = item
            .details
            .fields
            .into_iter()
            .find(|item| item.designation == "password");
        match password {
            Some(password) => Ok(password.value),
            None => Err("could not find password field".into()),
        }
    }

    fn delete(&self, session: &Option<String>, uuid: &str) -> Result<(), Error> {
        let cmd = self.make_cmd(session, &["delete", "item", uuid]);
        self.run_cmd(cmd)?;
        Ok(())
    }
}

impl Credential for OnePasswordKeychain {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn get(&self, registry_name: &str, _api_url: &str) -> Result<String, Error> {
        let session = self.signin()?;
        if let Some(uuid) = self.search(&session, registry_name)? {
            self.get_token(&session, &uuid)
        } else {
            return Err(format!(
                "no 1password entry found for registry `{}`, try `cargo login` to add a token",
                registry_name
            )
            .into());
        }
    }

    fn store(&self, registry_name: &str, api_url: &str, token: &str) -> Result<(), Error> {
        let session = self.signin()?;
        // Check if an item already exists.
        if let Some(uuid) = self.search(&session, registry_name)? {
            self.modify(&session, &uuid, token)
        } else {
            self.create(&session, registry_name, api_url, token)
        }
    }

    fn erase(&self, registry_name: &str, _api_url: &str) -> Result<(), Error> {
        let session = self.signin()?;
        // Check if an item already exists.
        if let Some(uuid) = self.search(&session, registry_name)? {
            self.delete(&session, &uuid)?;
        } else {
            eprintln!("not currently logged in to `{}`", registry_name);
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
