//! Operations that interact with the [registry web API][1].
//!
//! [1]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html

mod login;
mod logout;
mod owner;
mod publish;
mod search;
mod yank;

use std::collections::HashSet;
use std::str;
use std::task::Poll;

use anyhow::{bail, format_err, Context as _};
use cargo_credential::{Operation, Secret};
use crates_io::{self, Registry};
use url::Url;

use crate::core::SourceId;
use crate::sources::source::Source;
use crate::sources::{RegistrySource, SourceConfigMap};
use crate::util::auth;
use crate::util::cache_lock::CacheLockMode;
use crate::util::config::{Config, PathAndArgs};
use crate::util::errors::CargoResult;
use crate::util::network::http::http_handle;

pub use self::login::registry_login;
pub use self::logout::registry_logout;
pub use self::owner::modify_owners;
pub use self::owner::OwnersOptions;
pub use self::publish::publish;
pub use self::publish::PublishOpts;
pub use self::search::search;
pub use self::yank::yank;

/// Represents either `--registry` or `--index` argument, which is mutually exclusive.
#[derive(Debug, Clone)]
pub enum RegistryOrIndex {
    Registry(String),
    Index(Url),
}

impl RegistryOrIndex {
    fn is_index(&self) -> bool {
        matches!(self, RegistryOrIndex::Index(..))
    }
}

/// Registry settings loaded from config files.
///
/// This is loaded based on the `--registry` flag and the config settings.
#[derive(Debug, PartialEq)]
pub enum RegistryCredentialConfig {
    None,
    /// The authentication token.
    Token(Secret<String>),
    /// Process used for fetching a token.
    Process(Vec<PathAndArgs>),
    /// Secret Key and subject for Asymmetric tokens.
    AsymmetricKey((Secret<String>, Option<String>)),
}

impl RegistryCredentialConfig {
    /// Returns `true` if the credential is [`None`].
    ///
    /// [`None`]: Self::None
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
    /// Returns `true` if the credential is [`Token`].
    ///
    /// [`Token`]: Self::Token
    pub fn is_token(&self) -> bool {
        matches!(self, Self::Token(..))
    }
    /// Returns `true` if the credential is [`AsymmetricKey`].
    ///
    /// [`AsymmetricKey`]: RegistryCredentialConfig::AsymmetricKey
    pub fn is_asymmetric_key(&self) -> bool {
        matches!(self, Self::AsymmetricKey(..))
    }
    pub fn as_token(&self) -> Option<Secret<&str>> {
        if let Self::Token(v) = self {
            Some(v.as_deref())
        } else {
            None
        }
    }
    pub fn as_process(&self) -> Option<&Vec<PathAndArgs>> {
        if let Self::Process(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_asymmetric_key(&self) -> Option<&(Secret<String>, Option<String>)> {
        if let Self::AsymmetricKey(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

/// Returns the `Registry` and `Source` based on command-line and config settings.
///
/// * `token_from_cmdline`: The token from the command-line. If not set, uses the token
///   from the config.
/// * `index`: The index URL from the command-line.
/// * `registry`: The registry name from the command-line. If neither
///   `registry`, or `index` are set, then uses `crates-io`.
/// * `force_update`: If `true`, forces the index to be updated.
/// * `token_required`: If `true`, the token will be set.
fn registry(
    config: &Config,
    token_from_cmdline: Option<Secret<&str>>,
    reg_or_index: Option<&RegistryOrIndex>,
    force_update: bool,
    token_required: Option<Operation<'_>>,
) -> CargoResult<(Registry, RegistrySourceIds)> {
    let source_ids = get_source_id(config, reg_or_index)?;

    let is_index = reg_or_index.map(|v| v.is_index()).unwrap_or_default();
    if is_index && token_required.is_some() && token_from_cmdline.is_none() {
        bail!("command-line argument --index requires --token to be specified");
    }
    if let Some(token) = token_from_cmdline {
        auth::cache_token_from_commandline(config, &source_ids.original, token);
    }

    let cfg = {
        let _lock = config.acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;
        let mut src = RegistrySource::remote(source_ids.replacement, &HashSet::new(), config)?;
        // Only update the index if `force_update` is set.
        if force_update {
            src.invalidate_cache()
        }
        let cfg = loop {
            match src.config()? {
                Poll::Pending => src
                    .block_until_ready()
                    .with_context(|| format!("failed to update {}", source_ids.replacement))?,
                Poll::Ready(cfg) => break cfg,
            }
        };
        cfg.expect("remote registries must have config")
    };
    let api_host = cfg
        .api
        .ok_or_else(|| format_err!("{} does not support API commands", source_ids.replacement))?;
    let token = if token_required.is_some() || cfg.auth_required {
        let operation = token_required.unwrap_or(Operation::Read);
        Some(auth::auth_token(
            config,
            &source_ids.original,
            None,
            operation,
            vec![],
            false,
        )?)
    } else {
        None
    };
    let handle = http_handle(config)?;
    Ok((
        Registry::new_handle(api_host, token, handle, cfg.auth_required),
        source_ids,
    ))
}

/// Gets the SourceId for an index or registry setting.
///
/// The `index` and `reg` values are from the command-line or config settings.
/// If both are None, and no source-replacement is configured, returns the source for crates.io.
/// If both are None, and source replacement is configured, returns an error.
///
/// The source for crates.io may be GitHub, index.crates.io, or a test-only registry depending
/// on configuration.
///
/// If `reg` is set, source replacement is not followed.
///
/// The return value is a pair of `SourceId`s: The first may be a built-in replacement of
/// crates.io (such as index.crates.io), while the second is always the original source.
fn get_source_id(
    config: &Config,
    reg_or_index: Option<&RegistryOrIndex>,
) -> CargoResult<RegistrySourceIds> {
    let sid = match reg_or_index {
        None => SourceId::crates_io(config)?,
        Some(RegistryOrIndex::Index(url)) => SourceId::for_registry(url)?,
        Some(RegistryOrIndex::Registry(r)) => SourceId::alt_registry(config, r)?,
    };
    // Load source replacements that are built-in to Cargo.
    let builtin_replacement_sid = SourceConfigMap::empty(config)?
        .load(sid, &HashSet::new())?
        .replaced_source_id();
    let replacement_sid = SourceConfigMap::new(config)?
        .load(sid, &HashSet::new())?
        .replaced_source_id();
    if reg_or_index.is_none() && replacement_sid != builtin_replacement_sid {
        // Neither --registry nor --index was passed and the user has configured source-replacement.
        if let Some(replacement_name) = replacement_sid.alt_registry_key() {
            bail!("crates-io is replaced with remote registry {replacement_name};\ninclude `--registry {replacement_name}` or `--registry crates-io`");
        } else {
            bail!("crates-io is replaced with non-remote-registry source {replacement_sid};\ninclude `--registry crates-io` to use crates.io");
        }
    } else {
        Ok(RegistrySourceIds {
            original: sid,
            replacement: builtin_replacement_sid,
        })
    }
}

struct RegistrySourceIds {
    /// Use when looking up the auth token, or writing out `Cargo.lock`
    original: SourceId,
    /// Use when interacting with the source (querying / publishing , etc)
    ///
    /// The source for crates.io may be replaced by a built-in source for accessing crates.io with
    /// the sparse protocol, or a source for the testing framework (when the replace_crates_io
    /// function is used)
    ///
    /// User-defined source replacement is not applied.
    replacement: SourceId,
}
