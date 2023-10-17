//! Helper library for writing Cargo credential providers.
//!
//! A credential process should have a `struct` that implements the `Credential` trait.
//! The `main` function should be called with an instance of that struct, such as:
//!
//! ```rust,ignore
//! fn main() {
//!     cargo_credential::main(MyCredential);
//! }
//! ```
//!
//! While in the `perform` function, stdin and stdout will be re-attached to the
//! active console. This allows credential providers to be interactive if necessary.
//!
//! ## Error handling
//! ### [`Error::UrlNotSupported`]
//! A credential provider may only support some registry URLs. If this is the case
//! and an unsupported index URL is passed to the provider, it should respond with
//! [`Error::UrlNotSupported`]. Other credential providers may be attempted by Cargo.
//!
//! ### [`Error::NotFound`]
//! When attempting an [`Action::Get`] or [`Action::Logout`], if a credential can not
//! be found, the provider should respond with [`Error::NotFound`]. Other credential
//! providers may be attempted by Cargo.
//!
//! ### [`Error::OperationNotSupported`]
//! A credential provider might not support all operations. For example if the provider
//! only supports [`Action::Get`], [`Error::OperationNotSupported`] should be returned
//! for all other requests.
//!
//! ### [`Error::Other`]
//! All other errors go here. The error will be shown to the user in Cargo, including
//! the full error chain using [`std::error::Error::source`].
//!
//! ## Example
//! ```rust,ignore
#![doc = include_str!("../examples/file-provider.rs")]
//! ```

use serde::{Deserialize, Serialize};
use std::{fmt::Display, io};
use time::OffsetDateTime;

mod error;
mod secret;
mod stdio;

pub use error::Error;
pub use secret::Secret;
use stdio::stdin_stdout_to_console;

/// Message sent by the credential helper on startup
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CredentialHello {
    // Protocol versions supported by the credential process.
    pub v: Vec<u32>,
}

/// Credential provider that doesn't support any registries.
pub struct UnsupportedCredential;
impl Credential for UnsupportedCredential {
    fn perform(
        &self,
        _registry: &RegistryInfo<'_>,
        _action: &Action<'_>,
        _args: &[&str],
    ) -> Result<CredentialResponse, Error> {
        Err(Error::UrlNotSupported)
    }
}

/// Message sent by Cargo to the credential helper after the hello
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct CredentialRequest<'a> {
    // Cargo will respond with the highest common protocol supported by both.
    pub v: u32,
    #[serde(borrow)]
    pub registry: RegistryInfo<'a>,
    #[serde(borrow, flatten)]
    pub action: Action<'a>,
    /// Additional command-line arguments passed to the credential provider.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub args: Vec<&'a str>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct RegistryInfo<'a> {
    /// Registry index url
    pub index_url: &'a str,
    /// Name of the registry in configuration. May not be available.
    /// The crates.io registry will be `crates-io` (`CRATES_IO_REGISTRY`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,
    /// Headers from attempting to access a registry that resulted in a HTTP 401.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub headers: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Action<'a> {
    #[serde(borrow)]
    Get(Operation<'a>),
    Login(LoginOptions<'a>),
    Logout,
    #[serde(other)]
    Unknown,
}

impl<'a> Display for Action<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Get(_) => f.write_str("get"),
            Action::Login(_) => f.write_str("login"),
            Action::Logout => f.write_str("logout"),
            Action::Unknown => f.write_str("<unknown>"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct LoginOptions<'a> {
    /// Token passed on the command line via --token or from stdin
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<Secret<&'a str>>,
    /// Optional URL that the user can visit to log in to the registry
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_url: Option<&'a str>,
}

/// A record of what kind of operation is happening that we should generate a token for.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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
    #[serde(other)]
    Unknown,
}

/// Message sent by the credential helper
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum CredentialResponse {
    Get {
        token: Secret<String>,
        #[serde(flatten)]
        cache: CacheControl,
        operation_independent: bool,
    },
    Login,
    Logout,
    #[serde(other)]
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "cache", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum CacheControl {
    /// Do not cache this result.
    Never,
    /// Cache this result and use it for subsequent requests in the current Cargo invocation until the specified time.
    Expires {
        #[serde(with = "time::serde::timestamp")]
        expiration: OffsetDateTime,
    },
    /// Cache this result and use it for all subsequent requests in the current Cargo invocation.
    Session,
    #[serde(other)]
    Unknown,
}

/// Credential process JSON protocol version. If the protocol needs to make
/// a breaking change, a new protocol version should be defined (`PROTOCOL_VERSION_2`).
/// This library should offer support for both protocols if possible, by signaling
/// in the `CredentialHello` message. Cargo will then choose which protocol to use,
/// or it will error if there are no common protocol versions available.
pub const PROTOCOL_VERSION_1: u32 = 1;
pub trait Credential {
    /// Retrieves a token for the given registry.
    fn perform(
        &self,
        registry: &RegistryInfo<'_>,
        action: &Action<'_>,
        args: &[&str],
    ) -> Result<CredentialResponse, Error>;
}

/// Runs the credential interaction
pub fn main(credential: impl Credential) {
    let result = doit(credential).map_err(|e| Error::Other(e));
    if result.is_err() {
        serde_json::to_writer(std::io::stdout(), &result)
            .expect("failed to serialize credential provider error");
        println!();
    }
}

fn doit(
    credential: impl Credential,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
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
        let request = deserialize_request(&buffer)?;
        let response = stdin_stdout_to_console(|| {
            credential.perform(&request.registry, &request.action, &request.args)
        })?;

        serde_json::to_writer(std::io::stdout(), &response)?;
        println!();
    }
}

/// Deserialize a request from Cargo.
fn deserialize_request(
    value: &str,
) -> Result<CredentialRequest<'_>, Box<dyn std::error::Error + Send + Sync>> {
    let request: CredentialRequest<'_> = serde_json::from_str(&value)?;
    if request.v != PROTOCOL_VERSION_1 {
        return Err(format!("unsupported protocol version {}", request.v).into());
    }
    Ok(request)
}

/// Read a line of text from stdin.
pub fn read_line() -> Result<String, io::Error> {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

/// Prompt the user for a token.
pub fn read_token(
    login_options: &LoginOptions<'_>,
    registry: &RegistryInfo<'_>,
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

    Ok(Secret::from(read_line().map_err(Box::new)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_version() {
        // This shouldn't ever happen in practice, since the credential provider signals to Cargo which
        // protocol versions it supports, and Cargo should only attempt to use one of those.
        let msg = r#"{"v":999, "registry": {"index-url":""}, "args":[], "kind": "unexpected"}"#;
        assert_eq!(
            "unsupported protocol version 999",
            deserialize_request(msg).unwrap_err().to_string()
        );
    }

    #[test]
    fn cache_control() {
        let cc = CacheControl::Expires {
            expiration: OffsetDateTime::from_unix_timestamp(1693928537).unwrap(),
        };
        let json = serde_json::to_string(&cc).unwrap();
        assert_eq!(json, r#"{"cache":"expires","expiration":1693928537}"#);

        let cc = CacheControl::Session;
        let json = serde_json::to_string(&cc).unwrap();
        assert_eq!(json, r#"{"cache":"session"}"#);

        let cc: CacheControl = serde_json::from_str(r#"{"cache":"unknown-kind"}"#).unwrap();
        assert_eq!(cc, CacheControl::Unknown);

        assert_eq!(
            "missing field `expiration`",
            serde_json::from_str::<CacheControl>(r#"{"cache":"expires"}"#)
                .unwrap_err()
                .to_string()
        );
    }

    #[test]
    fn credential_response() {
        let cr = CredentialResponse::Get {
            cache: CacheControl::Never,
            operation_independent: true,
            token: Secret::from("value".to_string()),
        };
        let json = serde_json::to_string(&cr).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"get","token":"value","cache":"never","operation_independent":true}"#
        );

        let cr = CredentialResponse::Login;
        let json = serde_json::to_string(&cr).unwrap();
        assert_eq!(json, r#"{"kind":"login"}"#);

        let cr: CredentialResponse =
            serde_json::from_str(r#"{"kind":"unknown-kind","extra-data":true}"#).unwrap();
        assert_eq!(cr, CredentialResponse::Unknown);

        let cr: CredentialResponse =
            serde_json::from_str(r#"{"kind":"login","extra-data":true}"#).unwrap();
        assert_eq!(cr, CredentialResponse::Login);

        let cr: CredentialResponse = serde_json::from_str(r#"{"kind":"get","token":"value","cache":"never","operation_independent":true,"extra-field-ignored":123}"#).unwrap();
        assert_eq!(
            cr,
            CredentialResponse::Get {
                cache: CacheControl::Never,
                operation_independent: true,
                token: Secret::from("value".to_string())
            }
        );
    }

    #[test]
    fn credential_request() {
        let get_oweners = CredentialRequest {
            v: PROTOCOL_VERSION_1,
            args: vec![],
            registry: RegistryInfo {
                index_url: "url",
                name: None,
                headers: vec![],
            },
            action: Action::Get(Operation::Owners { name: "pkg" }),
        };

        let json = serde_json::to_string(&get_oweners).unwrap();
        assert_eq!(
            json,
            r#"{"v":1,"registry":{"index-url":"url"},"kind":"get","operation":"owners","name":"pkg"}"#
        );

        let cr: CredentialRequest<'_> =
            serde_json::from_str(r#"{"extra-1":true,"v":1,"registry":{"index-url":"url","extra-2":true},"kind":"get","operation":"owners","name":"pkg","args":[]}"#).unwrap();
        assert_eq!(cr, get_oweners);
    }

    #[test]
    fn credential_request_logout() {
        let unknown = CredentialRequest {
            v: PROTOCOL_VERSION_1,
            args: vec![],
            registry: RegistryInfo {
                index_url: "url",
                name: None,
                headers: vec![],
            },
            action: Action::Logout,
        };

        let cr: CredentialRequest<'_> = serde_json::from_str(
            r#"{"v":1,"registry":{"index-url":"url"},"kind":"logout","extra-1":true,"args":[]}"#,
        )
        .unwrap();
        assert_eq!(cr, unknown);
    }

    #[test]
    fn credential_request_unknown() {
        let unknown = CredentialRequest {
            v: PROTOCOL_VERSION_1,
            args: vec![],
            registry: RegistryInfo {
                index_url: "",
                name: None,
                headers: vec![],
            },
            action: Action::Unknown,
        };

        let cr: CredentialRequest<'_> = serde_json::from_str(
            r#"{"v":1,"registry":{"index-url":""},"kind":"unexpected-1","extra-1":true,"args":[]}"#,
        )
        .unwrap();
        assert_eq!(cr, unknown);
    }
}
