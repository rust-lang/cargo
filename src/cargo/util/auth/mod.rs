//! Registry authentication support.

use crate::{
    sources::CRATES_IO_REGISTRY,
    util::{config::ConfigKey, CanonicalUrl, CargoResult, Config, IntoUrl},
};
use anyhow::{bail, Context as _};
use cargo_credential::{
    Action, CacheControl, Credential, CredentialResponse, LoginOptions, Operation, RegistryInfo,
    Secret,
};

use core::fmt;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use time::{Duration, OffsetDateTime};
use url::Url;

use crate::core::SourceId;
use crate::util::config::Value;
use crate::util::credential::adaptor::BasicProcessCredential;
use crate::util::credential::paseto::PasetoCredential;

use super::{
    config::{CredentialCacheValue, OptValue, PathAndArgs},
    credential::process::CredentialProcessCredential,
    credential::token::TokenCredential,
};

/// `[registries.NAME]` tables.
///
/// The values here should be kept in sync with `RegistryConfigExtended`
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RegistryConfig {
    pub index: Option<String>,
    pub token: OptValue<Secret<String>>,
    pub credential_provider: Option<PathAndArgs>,
    pub secret_key: OptValue<Secret<String>>,
    pub secret_key_subject: Option<String>,
    #[serde(rename = "protocol")]
    _protocol: Option<String>,
}

/// The `[registry]` table, which more keys than the `[registries.NAME]` tables.
///
/// Note: nesting `RegistryConfig` inside this struct and using `serde(flatten)` *should* work
/// but fails with "invalid type: sequence, expected a value" when attempting to deserialize.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RegistryConfigExtended {
    pub index: Option<String>,
    pub token: OptValue<Secret<String>>,
    pub credential_provider: Option<PathAndArgs>,
    pub secret_key: OptValue<Secret<String>>,
    pub secret_key_subject: Option<String>,
    #[serde(rename = "default")]
    _default: Option<String>,
    #[serde(rename = "global-credential-providers")]
    _global_credential_providers: Option<Vec<String>>,
}

impl RegistryConfigExtended {
    pub fn to_registry_config(self) -> RegistryConfig {
        RegistryConfig {
            index: self.index,
            token: self.token,
            credential_provider: self.credential_provider,
            secret_key: self.secret_key,
            secret_key_subject: self.secret_key_subject,
            _protocol: None,
        }
    }
}

/// Get the list of credential providers for a registry source.
fn credential_provider(config: &Config, sid: &SourceId) -> CargoResult<Vec<Vec<String>>> {
    let cfg = registry_credential_config_raw(config, sid)?;
    let allow_cred_proc = config.cli_unstable().credential_process;
    let default_providers = || {
        if allow_cred_proc {
            // Enable the PASETO provider
            vec![
                vec!["cargo:token".to_string()],
                vec!["cargo:paseto".to_string()],
            ]
        } else {
            vec![vec!["cargo:token".to_string()]]
        }
    };
    let global_providers = config
        .get::<Option<Vec<Value<String>>>>("registry.global-credential-providers")?
        .filter(|p| !p.is_empty() && allow_cred_proc)
        .map(|p| {
            p.iter()
                .rev()
                .map(PathAndArgs::from_whitespace_separated_string)
                .map(|p| resolve_credential_alias(config, p))
                .collect()
        })
        .unwrap_or_else(default_providers);

    let providers = match cfg {
        // If there's a specific provider configured for this registry, use it.
        Some(RegistryConfig {
            credential_provider: Some(provider),
            token,
            secret_key,
            ..
        }) if allow_cred_proc => {
            if let Some(token) = token {
                config.shell().warn(format!(
                    "{sid} has a token configured in {} that will be ignored \
                    because a credential-provider is configured for this registry`",
                    token.definition
                ))?;
            }
            if let Some(secret_key) = secret_key {
                config.shell().warn(format!(
                    "{sid} has a secret-key configured in {} that will be ignored \
                    because a credential-provider is configured for this registry`",
                    secret_key.definition
                ))?;
            }
            vec![resolve_credential_alias(config, provider)]
        }

        // Warning for both `token` and `secret-key`, stating which will be ignored
        Some(RegistryConfig {
            token: Some(token),
            secret_key: Some(secret_key),
            ..
        }) if allow_cred_proc => {
            let token_pos = global_providers
                .iter()
                .position(|p| p.first().map(String::as_str) == Some("cargo:token"));
            let paseto_pos = global_providers
                .iter()
                .position(|p| p.first().map(String::as_str) == Some("cargo:paseto"));
            match (token_pos, paseto_pos) {
                (Some(token_pos), Some(paseto_pos)) => {
                    if token_pos < paseto_pos {
                        config.shell().warn(format!(
                            "{sid} has a `secret_key` configured in {} that will be ignored \
                        because a `token` is also configured, and the `cargo:token` provider is \
                        configured with higher precedence",
                            secret_key.definition
                        ))?;
                    } else {
                        config.shell().warn(format!("{sid} has a `token` configured in {} that will be ignored \
                        because a `secret_key` is also configured, and the `cargo:paseto` provider is \
                        configured with higher precedence", token.definition))?;
                    }
                }
                (_, _) => {
                    // One or both of the below individual warnings will trigger
                }
            }
            global_providers
        }

        // Check if a `token` is configured that will be ignored.
        Some(RegistryConfig {
            token: Some(token), ..
        }) => {
            if !global_providers
                .iter()
                .any(|p| p.first().map(String::as_str) == Some("cargo:token"))
            {
                config.shell().warn(format!(
                    "{sid} has a token configured in {} that will be ignored \
                    because the `cargo:token` credential provider is not listed in \
                    `registry.global-credential-providers`",
                    token.definition
                ))?;
            }
            global_providers
        }

        // Check if a asymmetric token is configured that will be ignored.
        Some(RegistryConfig {
            secret_key: Some(token),
            ..
        }) if allow_cred_proc => {
            if !global_providers
                .iter()
                .any(|p| p.first().map(String::as_str) == Some("cargo:paseto"))
            {
                config.shell().warn(format!(
                    "{sid} has a secret-key configured in {} that will be ignored \
                    because the `cargo:paseto` credential provider is not listed in \
                    `registry.global-credential-providers`",
                    token.definition
                ))?;
            }
            global_providers
        }

        // If we couldn't find a registry-specific provider, use the fallback provider list.
        None | Some(RegistryConfig { .. }) => global_providers,
    };
    Ok(providers)
}

/// Get the credential configuration for a `SourceId`.
pub fn registry_credential_config_raw(
    config: &Config,
    sid: &SourceId,
) -> CargoResult<Option<RegistryConfig>> {
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
        return Ok(config
            .get::<Option<RegistryConfigExtended>>("registry")?
            .map(|c| c.to_registry_config()));
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

    if let Some(name) = &name {
        log::debug!("found alternative registry name `{name}` for {sid}");
        config.get::<Option<RegistryConfig>>(&format!("registries.{name}"))
    } else {
        log::debug!("no registry name found for {sid}");
        Ok(None)
    }
}

/// Use the `[credential-alias]` table to see if the provider name has been aliased.
fn resolve_credential_alias(config: &Config, mut provider: PathAndArgs) -> Vec<String> {
    if provider.args.is_empty() {
        let key = format!("credential-alias.{}", provider.path.raw_value());
        if let Ok(alias) = config.get::<PathAndArgs>(&key) {
            log::debug!("resolving credential alias '{key}' -> '{alias:?}'");
            provider = alias;
        }
    }
    provider.args.insert(
        0,
        provider
            .path
            .resolve_program(config)
            .to_str()
            .unwrap()
            .to_string(),
    );
    provider.args
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
    /// The `registry.default` config value.
    pub default_registry: Option<String>,
    /// Url where the user could log in.
    pub login_url: Option<Url>,
    /// Specific reason indicating what failed
    pub reason: AuthorizationErrorReason,
}
impl Error for AuthorizationError {}
impl fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.sid.is_crates_io() {
            let args = if self.default_registry.is_some() {
                " --registry crates-io"
            } else {
                ""
            };
            write!(
                f,
                "{}, please run `cargo login{args}`\nor use environment variable CARGO_REGISTRY_TOKEN",
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
pub fn cache_token_from_commandline(config: &Config, sid: &SourceId, token: Secret<&str>) {
    let url = sid.canonical_url();
    config.credential_cache().insert(
        url.clone(),
        CredentialCacheValue {
            token_value: token.to_owned(),
            expiration: None,
            operation_independent: true,
        },
    );
}

fn credential_action(
    config: &Config,
    sid: &SourceId,
    action: Action<'_>,
    headers: Vec<String>,
) -> CargoResult<CredentialResponse> {
    let name = if sid.is_crates_io() {
        Some(CRATES_IO_REGISTRY)
    } else {
        sid.alt_registry_key()
    };
    let registry = RegistryInfo {
        index_url: sid.url().as_str(),
        name,
        headers,
    };
    let providers = credential_provider(config, sid)?;
    for provider in providers {
        let args: Vec<&str> = provider.iter().map(String::as_str).collect();
        let process = args[0];
        log::debug!("attempting credential provider: {args:?}");
        let provider: Box<dyn Credential> = match process {
            "cargo:token" => Box::new(TokenCredential::new(config)),
            "cargo:paseto" => Box::new(PasetoCredential::new(config)),
            "cargo:basic" => Box::new(BasicProcessCredential {}),
            "cargo:1password" => Box::new(cargo_credential_1password::OnePasswordCredential {}),
            "cargo:wincred" => Box::new(cargo_credential_wincred::WindowsCredential {}),
            "cargo:macos-keychain" => Box::new(cargo_credential_macos_keychain::MacKeychain {}),
            process => Box::new(CredentialProcessCredential::new(process)),
        };
        config.shell().verbose(|c| {
            c.status(
                "Credential",
                format!(
                    "{} {action} {}",
                    args.join(" "),
                    sid.display_registry_name()
                ),
            )
        })?;
        match provider.perform(&registry, &action, &args[1..]) {
            Ok(response) => return Ok(response),
            Err(cargo_credential::Error::UrlNotSupported)
            | Err(cargo_credential::Error::NotFound) => {}
            e => {
                return e.with_context(|| {
                    format!(
                        "credential provider `{}` failed action `{action}`",
                        args.join(" ")
                    )
                })
            }
        }
    }
    Err(cargo_credential::Error::NotFound.into())
}

/// Returns the token to use for the given registry.
/// If a `login_url` is provided and a token is not available, the
/// login_url will be included in the returned error.
pub fn auth_token(
    config: &Config,
    sid: &SourceId,
    login_url: Option<&Url>,
    operation: Operation<'_>,
    headers: Vec<String>,
) -> CargoResult<String> {
    match auth_token_optional(config, sid, operation, headers)? {
        Some(token) => Ok(token.expose()),
        None => Err(AuthorizationError {
            sid: sid.clone(),
            default_registry: config.default_registry()?,
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
    operation: Operation<'_>,
    headers: Vec<String>,
) -> CargoResult<Option<Secret<String>>> {
    log::trace!("token requested for {}", sid.display_registry_name());
    let mut cache = config.credential_cache();
    let url = sid.canonical_url();
    if let Some(cached_token) = cache.get(url) {
        if cached_token
            .expiration
            .map(|exp| OffsetDateTime::now_utc() + Duration::minutes(1) < exp)
            .unwrap_or(true)
        {
            if cached_token.operation_independent || matches!(operation, Operation::Read) {
                log::trace!("using token from in-memory cache");
                return Ok(Some(cached_token.token_value.clone()));
            }
        } else {
            // Remove expired token from the cache
            cache.remove(url);
        }
    }

    let credential_response = credential_action(config, sid, Action::Get(operation), headers);
    if let Some(e) = credential_response.as_ref().err() {
        if let Some(e) = e.downcast_ref::<cargo_credential::Error>() {
            if matches!(e, cargo_credential::Error::NotFound) {
                return Ok(None);
            }
        }
    }
    let credential_response = credential_response?;

    let CredentialResponse::Get {
        token,
        cache: cache_control,
        operation_independent,
    } = credential_response
    else {
        bail!("credential provider produced unexpected response for `get` request: {credential_response:?}")
    };
    let token = Secret::from(token);
    log::trace!("found token");
    let expiration = match cache_control {
        CacheControl::Expires(expiration) => Some(expiration),
        CacheControl::Session => None,
        CacheControl::Never | _ => return Ok(Some(token)),
    };

    cache.insert(
        url.clone(),
        CredentialCacheValue {
            token_value: token.clone(),
            expiration,
            operation_independent,
        },
    );
    Ok(Some(token))
}

/// Log out from the given registry.
pub fn logout(config: &Config, sid: &SourceId) -> CargoResult<()> {
    let credential_response = credential_action(config, sid, Action::Logout, vec![]);
    if let Some(e) = credential_response.as_ref().err() {
        if let Some(e) = e.downcast_ref::<cargo_credential::Error>() {
            if matches!(e, cargo_credential::Error::NotFound) {
                config.shell().status(
                    "Logout",
                    format!(
                        "not currently logged in to `{}`",
                        sid.display_registry_name()
                    ),
                )?;
                return Ok(());
            }
        }
    }
    let credential_response = credential_response?;
    let CredentialResponse::Logout = credential_response else {
        bail!("credential provider produced unexpected response for `logout` request: {credential_response:?}")
    };
    Ok(())
}

/// Log in to the given registry.
pub fn login(config: &Config, sid: &SourceId, options: LoginOptions<'_>) -> CargoResult<()> {
    let credential_response = credential_action(config, sid, Action::Login(options), vec![])?;
    let CredentialResponse::Login = credential_response else {
        bail!("credential provider produced unexpected response for `login` request: {credential_response:?}")
    };
    Ok(())
}
