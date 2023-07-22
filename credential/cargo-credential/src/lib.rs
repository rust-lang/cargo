//! Helper library for writing Cargo credential processes.
//!
//! A credential process should have a `struct` that implements the `Credential` trait.
//! The `main` function should be called with an instance of that struct, such as:
//!
//! ```rust,ignore
//! fn main() {
//!     cargo_credential::main(MyCredential);
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    fs::File,
    io::{self, BufRead, BufReader},
};
use time::OffsetDateTime;

mod secret;
pub use secret::Secret;

/// Message sent by the credential helper on startup
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CredentialHello {
    // Protocol versions supported by the credential process.
    pub v: Vec<u32>,
}

/// Credential provider that doesn't support any registries.
pub struct UnsupportedCredential;
impl Credential for UnsupportedCredential {
    fn perform(
        &self,
        _registry: &RegistryInfo,
        _action: &Action,
        _args: &[&str],
    ) -> Result<CredentialResponse, Error> {
        Err(Error::UrlNotSupported)
    }
}

/// Message sent by Cargo to the credential helper after the hello
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct CredentialRequest<'a> {
    // Cargo will respond with the highest common protocol supported by both.
    pub v: u32,
    #[serde(borrow)]
    pub registry: RegistryInfo<'a>,
    #[serde(borrow, flatten)]
    pub action: Action<'a>,
    /// Additional command-line arguments passed to the credential provider.
    pub args: Vec<&'a str>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct RegistryInfo<'a> {
    /// Registry index url
    pub index_url: &'a str,
    /// Name of the registry in configuration. May not be available.
    /// The crates.io registry will be `crates-io` (`CRATES_IO_REGISTRY`).
    pub name: Option<&'a str>,
    /// Headers from attempting to access a registry that resulted in a HTTP 401.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[non_exhaustive]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Action<'a> {
    #[serde(borrow)]
    Get(Operation<'a>),
    Login(LoginOptions<'a>),
    Logout,
}

impl<'a> Display for Action<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Get(_) => f.write_str("get"),
            Action::Login(_) => f.write_str("login"),
            Action::Logout => f.write_str("logout"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct LoginOptions<'a> {
    /// Token passed on the command line via --token or from stdin
    pub token: Option<Secret<&'a str>>,
    /// Optional URL that the user can visit to log in to the registry
    pub login_url: Option<&'a str>,
}

/// A record of what kind of operation is happening that we should generate a token for.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[non_exhaustive]
#[serde(tag = "operation", rename_all = "kebab-case")]
pub enum Operation<'a> {
    /// The user is attempting to fetch a crate.
    Read,
    /// The user is attempting to publish a crate.
    Publish {
        /// The name of the crate
        name: &'a str,
        /// The version of the crate
        vers: &'a str,
        /// The checksum of the crate file being uploaded
        cksum: &'a str,
    },
    /// The user is attempting to yank a crate.
    Yank {
        /// The name of the crate
        name: &'a str,
        /// The version of the crate
        vers: &'a str,
    },
    /// The user is attempting to unyank a crate.
    Unyank {
        /// The name of the crate
        name: &'a str,
        /// The version of the crate
        vers: &'a str,
    },
    /// The user is attempting to modify the owners of a crate.
    Owners {
        /// The name of the crate
        name: &'a str,
    },
}

/// Message sent by the credential helper
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum CredentialResponse {
    Get {
        token: Secret<String>,
        cache: CacheControl,
        operation_independent: bool,
    },
    Login,
    Logout,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum CacheControl {
    /// Do not cache this result.
    Never,
    /// Cache this result and use it for subsequent requests in the current Cargo invocation until the specified time.
    Expires(#[serde(with = "time::serde::timestamp")] OffsetDateTime),
    /// Cache this result and use it for all subsequent requests in the current Cargo invocation.
    Session,
}

/// Credential process JSON protocol version. Incrementing
/// this version will prevent new credential providers
/// from working with older versions of Cargo.
pub const PROTOCOL_VERSION_1: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "detail")]
#[non_exhaustive]
pub enum Error {
    UrlNotSupported,
    ProtocolNotSupported(u32),
    Subprocess(String),
    Io(String),
    Serde(String),
    Other(String),
    OperationNotSupported,
    NotFound,
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serde(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err.to_string())
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Error::Other(err)
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Error::Other(err.to_string())
    }
}

impl std::error::Error for Error {}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UrlNotSupported => {
                write!(f, "credential provider does not support this registry")
            }
            Error::ProtocolNotSupported(v) => write!(
                f,
                "credential provider does not support protocol version {v}"
            ),
            Error::Io(msg) => write!(f, "i/o error: {msg}"),
            Error::Serde(msg) => write!(f, "serialization error: {msg}"),
            Error::Other(msg) => write!(f, "error: {msg}"),
            Error::Subprocess(msg) => write!(f, "subprocess failed: {msg}"),
            Error::OperationNotSupported => write!(
                f,
                "credential provider does not support the requested operation"
            ),
            Error::NotFound => write!(f, "credential not found"),
        }
    }
}

pub trait Credential {
    /// Retrieves a token for the given registry.
    fn perform(
        &self,
        registry: &RegistryInfo,
        action: &Action,
        args: &[&str],
    ) -> Result<CredentialResponse, Error>;
}

/// Runs the credential interaction
pub fn main(credential: impl Credential) {
    let result = doit(credential);
    if result.is_err() {
        serde_json::to_writer(std::io::stdout(), &result)
            .expect("failed to serialize credential provider error");
        println!();
    }
}

fn doit(credential: impl Credential) -> Result<(), Error> {
    let hello = CredentialHello {
        v: vec![PROTOCOL_VERSION_1],
    };
    serde_json::to_writer(std::io::stdout(), &hello)?;
    println!();

    loop {
        let mut buffer = String::new();
        let len = std::io::stdin().read_line(&mut buffer)?;
        if len == 0 {
            return Ok(());
        }
        let request: CredentialRequest = serde_json::from_str(&buffer)?;
        if request.v != PROTOCOL_VERSION_1 {
            return Err(Error::ProtocolNotSupported(request.v));
        }
        serde_json::to_writer(
            std::io::stdout(),
            &credential.perform(&request.registry, &request.action, &request.args),
        )?;
        println!();
    }
}

/// Open stdin from the tty
pub fn tty() -> Result<File, io::Error> {
    #[cfg(unix)]
    const IN_DEVICE: &str = "/dev/tty";
    #[cfg(windows)]
    const IN_DEVICE: &str = "CONIN$";
    let stdin = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(IN_DEVICE)?;
    Ok(stdin)
}

/// Read a line of text from stdin.
pub fn read_line() -> Result<String, io::Error> {
    let mut reader = BufReader::new(tty()?);
    let mut buf = String::new();
    reader.read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

/// Prompt the user for a token.
pub fn read_token(
    login_options: &LoginOptions,
    registry: &RegistryInfo,
) -> Result<Secret<String>, Error> {
    if let Some(token) = &login_options.token {
        return Ok(token.to_owned());
    }

    if let Some(url) = login_options.login_url {
        eprintln!("please paste the token found on {url} below");
    } else if let Some(name) = registry.name {
        eprintln!("please paste the token for {name} below");
    } else {
        eprintln!("please paste the token for {} below", registry.index_url);
    }

    Ok(Secret::from(read_line()?))
}
