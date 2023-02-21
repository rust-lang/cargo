//! Registry authentication support.

use crate::util::{config, config::ConfigKey, CanonicalUrl, CargoResult, Config, IntoUrl};
use anyhow::{bail, format_err, Context as _};
use cargo_util::ProcessError;
use core::fmt;
use pasetors::keys::{AsymmetricPublicKey, AsymmetricSecretKey};
use pasetors::paserk::FormatAsPaserk;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Write};
use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use url::Url;

use crate::core::SourceId;
use crate::ops::RegistryCredentialConfig;

use super::config::CredentialCacheValue;

/// A wrapper for values that should not be printed.
///
/// This type does not implement `Display`, and has a `Debug` impl that hides
/// the contained value.
///
/// ```
/// # use cargo::util::auth::Secret;
/// let token = Secret::from("super secret string");
/// assert_eq!(format!("{:?}", token), "Secret { inner: \"REDACTED\" }");
/// ```
///
/// Currently, we write a borrowed `Secret<T>` as `Secret<&T>`.
/// The [`as_deref`](Secret::as_deref) and [`owned`](Secret::owned) methods can
/// be used to convert back and forth between `Secret<String>` and `Secret<&str>`.
#[derive(Default, Clone, PartialEq, Eq)]
pub struct Secret<T> {
    inner: T,
}

impl<T> Secret<T> {
    /// Unwraps the contained value.
    ///
    /// Use of this method marks the boundary of where the contained value is
    /// hidden.
    pub fn expose(self) -> T {
        self.inner
    }

    /// Converts a `Secret<T>` to a `Secret<&T::Target>`.
    /// ```
    /// # use cargo::util::auth::Secret;
    /// let owned: Secret<String> = Secret::from(String::from("token"));
    /// let borrowed: Secret<&str> = owned.as_deref();
    /// ```
    pub fn as_deref(&self) -> Secret<&<T as Deref>::Target>
    where
        T: Deref,
    {
        Secret::from(self.inner.deref())
    }

    /// Converts a `Secret<T>` to a `Secret<&T>`.
    pub fn as_ref(&self) -> Secret<&T> {
        Secret::from(&self.inner)
    }

    /// Converts a `Secret<T>` to a `Secret<U>` by applying `f` to the contained value.
    pub fn map<U, F>(self, f: F) -> Secret<U>
    where
        F: FnOnce(T) -> U,
    {
        Secret::from(f(self.inner))
    }
}

impl<T: ToOwned + ?Sized> Secret<&T> {
    /// Converts a `Secret` containing a borrowed type to a `Secret` containing the
    /// corresponding owned type.
    /// ```
    /// # use cargo::util::auth::Secret;
    /// let borrowed: Secret<&str> = Secret::from("token");
    /// let owned: Secret<String> = borrowed.owned();
    /// ```
    pub fn owned(&self) -> Secret<<T as ToOwned>::Owned> {
        Secret::from(self.inner.to_owned())
    }
}

impl<T, E> Secret<Result<T, E>> {
    /// Converts a `Secret<Result<T, E>>` to a `Result<Secret<T>, E>`.
    pub fn transpose(self) -> Result<Secret<T>, E> {
        self.inner.map(|v| Secret::from(v))
    }
}

impl<T: AsRef<str>> Secret<T> {
    /// Checks if the contained value is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.as_ref().is_empty()
    }
}

impl<T> From<T> for Secret<T> {
    fn from(inner: T) -> Self {
        Self { inner }
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Secret")
            .field("inner", &"REDACTED")
            .finish()
    }
}

/// Get the credential configuration for a `SourceId`.
pub fn registry_credential_config(
    config: &Config,
    sid: &SourceId,
) -> CargoResult<RegistryCredentialConfig> {
    #[derive(Deserialize)]
    #[serde(rename_all = "kebab-case")]
    struct RegistryConfig {
        index: Option<String>,
        token: Option<String>,
        credential_process: Option<config::PathAndArgs>,
        secret_key: Option<String>,
        secret_key_subject: Option<String>,
        #[serde(rename = "default")]
        _default: Option<String>,
    }

    log::trace!("loading credential config for {}", sid);
    config.load_credentials()?;
    if !sid.is_remote_registry() {
        bail!(
            "{} does not support API commands.\n\
             Check for a source-replacement in .cargo/config.",
            sid
        );
    }

    // Handle crates.io specially, since it uses different configuration keys.
    if sid.is_crates_io() {
        config.check_registry_index_not_set()?;
        let RegistryConfig {
            token,
            credential_process,
            secret_key,
            secret_key_subject,
            ..
        } = config.get::<RegistryConfig>("registry")?;
        return registry_credential_config_inner(
            true,
            None,
            token.map(Secret::from),
            credential_process,
            secret_key.map(Secret::from),
            secret_key_subject,
            config,
        );
    }

    // Find the SourceId's name by its index URL. If environment variables
    // are available they will be preferred over configuration values.
    //
    // The fundamental problem is that we only know the index url of the registry
    // for certain. For example, an unnamed registry source can come from the `--index`
    // command line argument, or from a Cargo.lock file. For this reason, we always
    // attempt to discover the name by looking it up by the index URL.
    //
    // This also allows the authorization token for a registry to be set
    // without knowing the registry name by using the _INDEX and _TOKEN
    // environment variables.
    let name = {
        // Discover names from environment variables.
        let index = sid.canonical_url();
        let mut names: Vec<_> = config
            .env()
            .filter_map(|(k, v)| {
                Some((
                    k.strip_prefix("CARGO_REGISTRIES_")?
                        .strip_suffix("_INDEX")?,
                    v,
                ))
            })
            .filter_map(|(k, v)| Some((k, CanonicalUrl::new(&v.into_url().ok()?).ok()?)))
            .filter(|(_, v)| v == index)
            .map(|(k, _)| k.to_lowercase())
            .collect();

        // Discover names from the configuration only if none were found in the environment.
        if names.len() == 0 {
            names = config
                .get::<HashMap<String, RegistryConfig>>("registries")?
                .iter()
                .filter_map(|(k, v)| Some((k, v.index.as_deref()?)))
                .filter_map(|(k, v)| Some((k, CanonicalUrl::new(&v.into_url().ok()?).ok()?)))
                .filter(|(_, v)| v == index)
                .map(|(k, _)| k.to_string())
                .collect();
        }
        names.sort();
        match names.len() {
            0 => None,
            1 => Some(std::mem::take(&mut names[0])),
            _ => anyhow::bail!(
                "multiple registries are configured with the same index url '{}': {}",
                &sid.as_url(),
                names.join(", ")
            ),
        }
    };

    // It's possible to have a registry configured in a Cargo config file,
    // then override it with configuration from environment variables.
    // If the name doesn't match, leave a note to help the user understand
    // the potentially confusing situation.
    if let Some(name) = name.as_deref() {
        if Some(name) != sid.alt_registry_key() {
            config.shell().note(format!(
                "name of alternative registry `{}` set to `{name}`",
                sid.url()
            ))?
        }
    }

    let (token, credential_process, secret_key, secret_key_subject) = if let Some(name) = &name {
        log::debug!("found alternative registry name `{name}` for {sid}");
        let RegistryConfig {
            token,
            secret_key,
            secret_key_subject,
            credential_process,
            ..
        } = config.get::<RegistryConfig>(&format!("registries.{name}"))?;
        (token, credential_process, secret_key, secret_key_subject)
    } else {
        log::debug!("no registry name found for {sid}");
        (None, None, None, None)
    };

    registry_credential_config_inner(
        false,
        name.as_deref(),
        token.map(Secret::from),
        credential_process,
        secret_key.map(Secret::from),
        secret_key_subject,
        config,
    )
}

fn registry_credential_config_inner(
    is_crates_io: bool,
    name: Option<&str>,
    token: Option<Secret<String>>,
    credential_process: Option<config::PathAndArgs>,
    secret_key: Option<Secret<String>>,
    secret_key_subject: Option<String>,
    config: &Config,
) -> CargoResult<RegistryCredentialConfig> {
    let credential_process =
        credential_process.filter(|_| config.cli_unstable().credential_process);
    let secret_key = secret_key.filter(|_| config.cli_unstable().registry_auth);
    let secret_key_subject = secret_key_subject.filter(|_| config.cli_unstable().registry_auth);
    let err_both = |token_key: &str, proc_key: &str| {
        let registry = if is_crates_io {
            "".to_string()
        } else {
            format!(" for registry `{}`", name.unwrap_or("UN-NAMED"))
        };
        Err(format_err!(
            "both `{token_key}` and `{proc_key}` \
            were specified in the config{registry}.\n\
            Only one of these values may be set, remove one or the other to proceed.",
        ))
    };
    Ok(
        match (token, credential_process, secret_key, secret_key_subject) {
            (Some(_), Some(_), _, _) => return err_both("token", "credential-process"),
            (Some(_), _, Some(_), _) => return err_both("token", "secret-key"),
            (_, Some(_), Some(_), _) => return err_both("credential-process", "secret-key"),
            (_, _, None, Some(_)) => {
                let registry = if is_crates_io {
                    "".to_string()
                } else {
                    format!(" for registry `{}`", name.as_ref().unwrap())
                };
                return Err(format_err!(
                    "`secret-key-subject` was set but `secret-key` was not in the config{}.\n\
                    Either set the `secret-key` or remove the `secret-key-subject`.",
                    registry
                ));
            }
            (Some(token), _, _, _) => RegistryCredentialConfig::Token(token),
            (_, Some(process), _, _) => RegistryCredentialConfig::Process((
                process.path.resolve_program(config),
                process.args,
            )),
            (None, None, Some(key), subject) => {
                RegistryCredentialConfig::AsymmetricKey((key, subject))
            }
            (None, None, None, _) => {
                if !is_crates_io {
                    // If we couldn't find a registry-specific credential, try the global credential process.
                    if let Some(process) = config
                        .get::<Option<config::PathAndArgs>>("registry.credential-process")?
                        .filter(|_| config.cli_unstable().credential_process)
                    {
                        return Ok(RegistryCredentialConfig::Process((
                            process.path.resolve_program(config),
                            process.args,
                        )));
                    }
                }
                RegistryCredentialConfig::None
            }
        },
    )
}

#[derive(Debug, PartialEq)]
pub enum AuthorizationErrorReason {
    TokenMissing,
    TokenRejected,
}

impl fmt::Display for AuthorizationErrorReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthorizationErrorReason::TokenMissing => write!(f, "no token found"),
            AuthorizationErrorReason::TokenRejected => write!(f, "token rejected"),
        }
    }
}

/// An authorization error from accessing a registry.
#[derive(Debug)]
pub struct AuthorizationError {
    /// Url that was attempted
    pub sid: SourceId,
    /// Url where the user could log in.
    pub login_url: Option<Url>,
    /// Specific reason indicating what failed
    pub reason: AuthorizationErrorReason,
}
impl Error for AuthorizationError {}
impl fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.sid.is_crates_io() {
            write!(
                f,
                "{}, please run `cargo login`\nor use environment variable CARGO_REGISTRY_TOKEN",
                self.reason
            )
        } else if let Some(name) = self.sid.alt_registry_key() {
            let key = ConfigKey::from_str(&format!("registries.{name}.token"));
            write!(
                f,
                "{} for `{}`, please run `cargo login --registry {name}`\nor use environment variable {}",
                self.reason,
                self.sid.display_registry_name(),
                key.as_env_key(),
            )
        } else if self.reason == AuthorizationErrorReason::TokenMissing {
            write!(
                f,
                r#"{} for `{}`
consider setting up an alternate registry in Cargo's configuration
as described by https://doc.rust-lang.org/cargo/reference/registries.html

[registries]
my-registry = {{ index = "{}" }}
"#,
                self.reason,
                self.sid.display_registry_name(),
                self.sid.url()
            )
        } else {
            write!(
                f,
                r#"{} for `{}`"#,
                self.reason,
                self.sid.display_registry_name(),
            )
        }
    }
}

// Store a token in the cache for future calls.
pub fn cache_token(config: &Config, sid: &SourceId, token: Secret<&str>) {
    let url = sid.canonical_url();
    config.credential_cache().insert(
        url.clone(),
        CredentialCacheValue {
            from_commandline: true,
            independent_of_endpoint: true,
            token_value: token.owned(),
        },
    );
}

/// Returns the token to use for the given registry.
/// If a `login_url` is provided and a token is not available, the
/// login_url will be included in the returned error.
pub fn auth_token(
    config: &Config,
    sid: &SourceId,
    login_url: Option<&Url>,
    mutation: Option<Mutation<'_>>,
) -> CargoResult<String> {
    match auth_token_optional(config, sid, mutation.as_ref())? {
        Some(token) => Ok(token.expose()),
        None => Err(AuthorizationError {
            sid: sid.clone(),
            login_url: login_url.cloned(),
            reason: AuthorizationErrorReason::TokenMissing,
        }
        .into()),
    }
}

/// Returns the token to use for the given registry.
fn auth_token_optional(
    config: &Config,
    sid: &SourceId,
    mutation: Option<&'_ Mutation<'_>>,
) -> CargoResult<Option<Secret<String>>> {
    let mut cache = config.credential_cache();
    let url = sid.canonical_url();

    if let Some(cache_token_value) = cache.get(url) {
        // Tokens for endpoints that do not involve a mutation can always be reused.
        // If the value is put in the cache by the command line, then we reuse it without looking at the configuration.
        if cache_token_value.from_commandline
            || cache_token_value.independent_of_endpoint
            || mutation.is_none()
        {
            return Ok(Some(cache_token_value.token_value.clone()));
        }
    }

    let credential = registry_credential_config(config, sid)?;
    let (independent_of_endpoint, token) = match credential {
        RegistryCredentialConfig::None => return Ok(None),
        RegistryCredentialConfig::Token(config_token) => (true, config_token),
        RegistryCredentialConfig::Process(process) => {
            // todo: PASETO with process
            let (independent_of_endpoint, token) =
                run_command(config, &process, sid, Action::Get)?.unwrap();
            (independent_of_endpoint, Secret::from(token))
        }
        RegistryCredentialConfig::AsymmetricKey((secret_key, secret_key_subject)) => {
            let secret: Secret<AsymmetricSecretKey<pasetors::version3::V3>> =
                secret_key.map(|key| key.as_str().try_into()).transpose()?;
            let public: AsymmetricPublicKey<pasetors::version3::V3> = secret
                .as_ref()
                .map(|key| key.try_into())
                .transpose()?
                .expose();
            let kip: pasetors::paserk::Id = (&public).try_into()?;
            let iat = OffsetDateTime::now_utc();

            let message = Message {
                iat: &iat.format(&Rfc3339)?,
                sub: secret_key_subject.as_deref(),
                mutation: mutation.and_then(|m| {
                    Some(match m {
                        Mutation::PrePublish => return None,
                        Mutation::Publish { .. } => "publish",
                        Mutation::Yank { .. } => "yank",
                        Mutation::Unyank { .. } => "unyank",
                        Mutation::Owners { .. } => "owners",
                    })
                }),
                name: mutation.and_then(|m| {
                    Some(match m {
                        Mutation::PrePublish => return None,
                        Mutation::Publish { name, .. }
                        | Mutation::Yank { name, .. }
                        | Mutation::Unyank { name, .. }
                        | Mutation::Owners { name, .. } => *name,
                    })
                }),
                vers: mutation.and_then(|m| {
                    Some(match m {
                        Mutation::PrePublish | Mutation::Owners { .. } => return None,
                        Mutation::Publish { vers, .. }
                        | Mutation::Yank { vers, .. }
                        | Mutation::Unyank { vers, .. } => *vers,
                    })
                }),
                cksum: mutation.and_then(|m| {
                    Some(match m {
                        Mutation::PrePublish
                        | Mutation::Yank { .. }
                        | Mutation::Unyank { .. }
                        | Mutation::Owners { .. } => return None,
                        Mutation::Publish { cksum, .. } => *cksum,
                    })
                }),
                challenge: None, // todo: PASETO with challenges
                v: None,
            };
            let footer = Footer {
                url: &sid.url().to_string(),
                kip,
            };

            (
                false,
                secret
                    .map(|secret| {
                        pasetors::version3::PublicToken::sign(
                            &secret,
                            serde_json::to_string(&message)
                                .expect("cannot serialize")
                                .as_bytes(),
                            Some(
                                serde_json::to_string(&footer)
                                    .expect("cannot serialize")
                                    .as_bytes(),
                            ),
                            None,
                        )
                    })
                    .transpose()?,
            )
        }
    };

    if independent_of_endpoint || mutation.is_none() {
        cache.insert(
            url.clone(),
            CredentialCacheValue {
                from_commandline: false,
                independent_of_endpoint,
                token_value: token.clone(),
            },
        );
    }
    Ok(Some(token))
}

/// A record of what kind of operation is happening that we should generate a token for.
pub enum Mutation<'a> {
    /// Before we generate a crate file for the users attempt to publish,
    /// we need to check if we are configured correctly to generate a token.
    /// This variant is used to make sure that we can generate a token,
    /// to error out early if the token is not configured correctly.
    PrePublish,
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

/// The main body of an asymmetric token as describe in RFC 3231.
#[derive(serde::Serialize)]
struct Message<'a> {
    iat: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mutation: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vers: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cksum: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    challenge: Option<&'a str>,
    /// This field is not yet used. This field can be set to a value >1 to indicate a breaking change in the token format.
    #[serde(skip_serializing_if = "Option::is_none")]
    v: Option<u8>,
}
/// The footer of an asymmetric token as describe in RFC 3231.
#[derive(serde::Serialize)]
struct Footer<'a> {
    url: &'a str,
    kip: pasetors::paserk::Id,
}

enum Action {
    Get,
    Store(String),
    Erase,
}

/// Saves the given token.
pub fn login(config: &Config, sid: &SourceId, token: RegistryCredentialConfig) -> CargoResult<()> {
    match registry_credential_config(config, sid)? {
        RegistryCredentialConfig::Process(process) => {
            let token = token
                .as_token()
                .expect("credential_process cannot use login with a secret_key")
                .expose()
                .to_owned();
            run_command(config, &process, sid, Action::Store(token))?;
        }
        _ => {
            config::save_credentials(config, Some(token), &sid)?;
        }
    };
    Ok(())
}

/// Checks that a secret key is valid, and returns the associated public key in Paserk format.
pub(crate) fn paserk_public_from_paserk_secret(secret_key: Secret<&str>) -> Option<String> {
    let secret: Secret<AsymmetricSecretKey<pasetors::version3::V3>> =
        secret_key.map(|key| key.try_into()).transpose().ok()?;
    let public: AsymmetricPublicKey<pasetors::version3::V3> = secret
        .as_ref()
        .map(|key| key.try_into())
        .transpose()
        .ok()?
        .expose();
    let mut paserk_pub_key = String::new();
    FormatAsPaserk::fmt(&public, &mut paserk_pub_key).unwrap();
    Some(paserk_pub_key)
}

/// Removes the token for the given registry.
pub fn logout(config: &Config, sid: &SourceId) -> CargoResult<()> {
    match registry_credential_config(config, sid)? {
        RegistryCredentialConfig::Process(process) => {
            run_command(config, &process, sid, Action::Erase)?;
        }
        _ => {
            config::save_credentials(config, None, &sid)?;
        }
    };
    Ok(())
}

fn run_command(
    config: &Config,
    process: &(PathBuf, Vec<String>),
    sid: &SourceId,
    action: Action,
) -> CargoResult<Option<(bool, String)>> {
    let index_url = sid.url().as_str();
    let cred_proc;
    let (exe, args) = if process.0.to_str().unwrap_or("").starts_with("cargo:") {
        cred_proc = sysroot_credential(config, process)?;
        &cred_proc
    } else {
        process
    };
    if !args.iter().any(|arg| arg.contains("{action}")) {
        let msg = |which| {
            format!(
                "credential process `{}` cannot be used to {}, \
                 the credential-process configuration value must pass the \
                 `{{action}}` argument in the config to support this command",
                exe.display(),
                which
            )
        };
        match action {
            Action::Get => {}
            Action::Store(_) => bail!(msg("log in")),
            Action::Erase => bail!(msg("log out")),
        }
    }
    // todo: PASETO with process
    let independent_of_endpoint = true;
    let action_str = match action {
        Action::Get => "get",
        Action::Store(_) => "store",
        Action::Erase => "erase",
    };
    let args: Vec<_> = args
        .iter()
        .map(|arg| {
            arg.replace("{action}", action_str)
                .replace("{index_url}", index_url)
        })
        .collect();

    let mut cmd = Command::new(&exe);
    cmd.args(args)
        .env(crate::CARGO_ENV, config.cargo_exe()?)
        .env("CARGO_REGISTRY_INDEX_URL", index_url);
    if sid.is_crates_io() {
        cmd.env("CARGO_REGISTRY_NAME_OPT", "crates-io");
    } else if let Some(name) = sid.alt_registry_key() {
        cmd.env("CARGO_REGISTRY_NAME_OPT", name);
    }
    match action {
        Action::Get => {
            cmd.stdout(Stdio::piped());
        }
        Action::Store(_) => {
            cmd.stdin(Stdio::piped());
        }
        Action::Erase => {}
    }
    let mut child = cmd.spawn().with_context(|| {
        let verb = match action {
            Action::Get => "fetch",
            Action::Store(_) => "store",
            Action::Erase => "erase",
        };
        format!(
            "failed to execute `{}` to {} authentication token for registry `{}`",
            exe.display(),
            verb,
            sid.display_registry_name(),
        )
    })?;
    let mut token = None;
    match &action {
        Action::Get => {
            let mut buffer = String::new();
            log::debug!("reading into buffer");
            child
                .stdout
                .as_mut()
                .unwrap()
                .read_to_string(&mut buffer)
                .with_context(|| {
                    format!(
                        "failed to read token from registry credential process `{}`",
                        exe.display()
                    )
                })?;
            if let Some(end) = buffer.find('\n') {
                if buffer.len() > end + 1 {
                    bail!(
                        "credential process `{}` returned more than one line of output; \
                         expected a single token",
                        exe.display()
                    );
                }
                buffer.truncate(end);
            }
            token = Some((independent_of_endpoint, buffer));
        }
        Action::Store(token) => {
            writeln!(child.stdin.as_ref().unwrap(), "{}", token).with_context(|| {
                format!(
                    "failed to send token to registry credential process `{}`",
                    exe.display()
                )
            })?;
        }
        Action::Erase => {}
    }
    let status = child.wait().with_context(|| {
        format!(
            "registry credential process `{}` exit failure",
            exe.display()
        )
    })?;
    if !status.success() {
        let msg = match action {
            Action::Get => "failed to authenticate to registry",
            Action::Store(_) => "failed to store token to registry",
            Action::Erase => "failed to erase token from registry",
        };
        return Err(ProcessError::new(
            &format!(
                "registry credential process `{}` {} `{}`",
                exe.display(),
                msg,
                sid.display_registry_name()
            ),
            Some(status),
            None,
        )
        .into());
    }
    Ok(token)
}

/// Gets the path to the libexec processes in the sysroot.
fn sysroot_credential(
    config: &Config,
    process: &(PathBuf, Vec<String>),
) -> CargoResult<(PathBuf, Vec<String>)> {
    let cred_name = process.0.to_str().unwrap().strip_prefix("cargo:").unwrap();
    let cargo = config.cargo_exe()?;
    let root = cargo
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| format_err!("expected cargo path {}", cargo.display()))?;
    let exe = root.join("libexec").join(format!(
        "cargo-credential-{}{}",
        cred_name,
        std::env::consts::EXE_SUFFIX
    ));
    let mut args = process.1.clone();
    if !args.iter().any(|arg| arg == "{action}") {
        args.push("{action}".to_string());
    }
    Ok((exe, args))
}
