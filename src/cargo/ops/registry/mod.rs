//! Operations that interact with the [registry web API][1].
//!
//! [1]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html

mod info;
mod login;
mod logout;
mod owner;
mod publish;
mod search;
mod yank;

use std::collections::HashSet;
use std::str;
use std::task::Poll;

use anyhow::{Context as _, bail, format_err};
use cargo_credential::{Operation, Secret};
use crates_io::Registry;
use url::Url;

use crate::core::{Package, PackageId, SourceId};
use crate::sources::source::Source;
use crate::sources::{RegistrySource, SourceConfigMap};
use crate::util::auth;
use crate::util::cache_lock::CacheLockMode;
use crate::util::context::{GlobalContext, PathAndArgs};
use crate::util::errors::CargoResult;
use crate::util::network::http::http_handle;

pub use self::info::info;
pub use self::login::registry_login;
pub use self::logout::registry_logout;
pub use self::owner::OwnersOptions;
pub use self::owner::modify_owners;
pub use self::publish::PublishOpts;
pub use self::publish::publish;
pub use self::search::search;
pub use self::yank::yank;

pub(crate) use self::publish::prepare_transmit;

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
/// * `source_ids`: The source IDs for the registry. It contains the original source ID and
///   the replacement source ID.
/// * `token_from_cmdline`: The token from the command-line. If not set, uses the token
///   from the config.
/// * `index`: The index URL from the command-line.
/// * `registry`: The registry name from the command-line. If neither
///   `registry`, or `index` are set, then uses `crates-io`.
/// * `force_update`: If `true`, forces the index to be updated.
/// * `token_required`: If `true`, the token will be set.
fn registry<'gctx>(
    gctx: &'gctx GlobalContext,
    source_ids: &RegistrySourceIds,
    token_from_cmdline: Option<Secret<&str>>,
    reg_or_index: Option<&RegistryOrIndex>,
    force_update: bool,
    token_required: Option<Operation<'_>>,
) -> CargoResult<(Registry, RegistrySource<'gctx>)> {
    let is_index = reg_or_index.map(|v| v.is_index()).unwrap_or_default();
    if is_index && token_required.is_some() && token_from_cmdline.is_none() {
        bail!("command-line argument --index requires --token to be specified");
    }
    if let Some(token) = token_from_cmdline {
        auth::cache_token_from_commandline(gctx, &source_ids.original, token);
    }

    let mut src = RegistrySource::remote(source_ids.replacement, &HashSet::new(), gctx)?;
    let cfg = {
        let _lock = gctx.acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;
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
            gctx,
            &source_ids.original,
            None,
            operation,
            vec![],
            false,
        )?)
    } else {
        None
    };
    let handle = http_handle(gctx)?;
    Ok((
        Registry::new_handle(api_host, token, handle, cfg.auth_required),
        src,
    ))
}

/// Gets the `SourceId` for an index or registry setting.
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
pub(crate) fn get_source_id(
    gctx: &GlobalContext,
    reg_or_index: Option<&RegistryOrIndex>,
) -> CargoResult<RegistrySourceIds> {
    let sid = get_initial_source_id(gctx, reg_or_index)?;
    let (builtin_replacement_sid, replacement_sid) = get_replacement_source_ids(gctx, sid)?;

    if reg_or_index.is_none() && replacement_sid != builtin_replacement_sid {
        bail!(gen_replacement_error(replacement_sid));
    } else {
        Ok(RegistrySourceIds {
            original: sid,
            replacement: builtin_replacement_sid,
        })
    }
}

/// Very similar to [`get_source_id`], but is used when the `package_id` is known.
fn get_source_id_with_package_id(
    gctx: &GlobalContext,
    package_id: Option<PackageId>,
    reg_or_index: Option<&RegistryOrIndex>,
) -> CargoResult<(bool, RegistrySourceIds)> {
    let (use_package_source_id, sid) = match (&reg_or_index, package_id) {
        (None, Some(package_id)) => (true, package_id.source_id()),
        (None, None) => (false, SourceId::crates_io(gctx)?),
        (Some(RegistryOrIndex::Index(url)), None) => (false, SourceId::for_registry(url)?),
        (Some(RegistryOrIndex::Registry(r)), None) => (false, SourceId::alt_registry(gctx, r)?),
        (Some(reg_or_index), Some(package_id)) => {
            let sid = get_initial_source_id_from_registry_or_index(gctx, reg_or_index)?;
            let package_source_id = package_id.source_id();
            // 1. Same registry, use the package's source.
            // 2. Use the package's source if the specified registry is a replacement for the package's source.
            if sid == package_source_id
                || is_replacement_for_package_source(gctx, sid, package_source_id)?
            {
                (true, package_source_id)
            } else {
                (false, sid)
            }
        }
    };

    let (builtin_replacement_sid, replacement_sid) = get_replacement_source_ids(gctx, sid)?;

    if reg_or_index.is_none() && replacement_sid != builtin_replacement_sid {
        bail!(gen_replacement_error(replacement_sid));
    } else {
        Ok((
            use_package_source_id,
            RegistrySourceIds {
                original: sid,
                replacement: builtin_replacement_sid,
            },
        ))
    }
}

fn get_initial_source_id(
    gctx: &GlobalContext,
    reg_or_index: Option<&RegistryOrIndex>,
) -> CargoResult<SourceId> {
    match reg_or_index {
        None => SourceId::crates_io(gctx),
        Some(reg_or_index) => get_initial_source_id_from_registry_or_index(gctx, reg_or_index),
    }
}

fn get_initial_source_id_from_registry_or_index(
    gctx: &GlobalContext,
    reg_or_index: &RegistryOrIndex,
) -> CargoResult<SourceId> {
    match reg_or_index {
        RegistryOrIndex::Index(url) => SourceId::for_registry(url),
        RegistryOrIndex::Registry(r) => SourceId::alt_registry(gctx, r),
    }
}

fn get_replacement_source_ids(
    gctx: &GlobalContext,
    sid: SourceId,
) -> CargoResult<(SourceId, SourceId)> {
    let builtin_replacement_sid = SourceConfigMap::empty(gctx)?
        .load(sid, &HashSet::new())?
        .replaced_source_id();
    let replacement_sid = SourceConfigMap::new(gctx)?
        .load(sid, &HashSet::new())?
        .replaced_source_id();
    Ok((builtin_replacement_sid, replacement_sid))
}

fn is_replacement_for_package_source(
    gctx: &GlobalContext,
    sid: SourceId,
    package_source_id: SourceId,
) -> CargoResult<bool> {
    let pkg_source_replacement_sid = SourceConfigMap::new(gctx)?
        .load(package_source_id, &HashSet::new())?
        .replaced_source_id();
    Ok(pkg_source_replacement_sid == sid)
}

fn gen_replacement_error(replacement_sid: SourceId) -> String {
    // Neither --registry nor --index was passed and the user has configured source-replacement.
    let error_message = if let Some(replacement_name) = replacement_sid.alt_registry_key() {
        format!(
            "crates-io is replaced with remote registry {};\ninclude `--registry {}` or `--registry crates-io`",
            replacement_name, replacement_name
        )
    } else {
        format!(
            "crates-io is replaced with non-remote-registry source {};\ninclude `--registry crates-io` to use crates.io",
            replacement_sid
        )
    };

    error_message
}

pub(crate) struct RegistrySourceIds {
    /// Use when looking up the auth token, or writing out `Cargo.lock`
    pub(crate) original: SourceId,
    /// Use when interacting with the source (querying / publishing , etc)
    ///
    /// The source for crates.io may be replaced by a built-in source for accessing crates.io with
    /// the sparse protocol, or a source for the testing framework (when the `replace_crates_io`
    /// function is used)
    ///
    /// User-defined source replacement is not applied.
    pub(crate) replacement: SourceId,
}

/// If this set of packages has an unambiguous publish registry, find it.
pub(crate) fn infer_registry(pkgs: &[&Package]) -> CargoResult<Option<RegistryOrIndex>> {
    // Ignore "publish = false" packages while inferring the registry.
    let publishable_pkgs: Vec<_> = pkgs
        .iter()
        .filter(|p| p.publish() != &Some(Vec::new()))
        .collect();

    let Some((first, rest)) = publishable_pkgs.split_first() else {
        return Ok(None);
    };

    // If all packages have the same publish settings, we take that as the default.
    if rest.iter().all(|p| p.publish() == first.publish()) {
        match publishable_pkgs[0].publish().as_deref() {
            Some([unique_pkg_reg]) => {
                Ok(Some(RegistryOrIndex::Registry(unique_pkg_reg.to_owned())))
            }
            None | Some([]) => Ok(None),
            Some(regs) => {
                let mut regs: Vec<_> = regs.iter().map(|s| format!("\"{}\"", s)).collect();
                regs.sort();
                regs.dedup();
                // unwrap: the match block ensures that there's more than one reg.
                let (last_reg, regs) = regs.split_last().unwrap();
                bail!(
                    "--registry is required to disambiguate between {} or {} registries",
                    regs.join(", "),
                    last_reg
                )
            }
        }
    } else {
        let common_regs = publishable_pkgs
            .iter()
            // `None` means "all registries", so drop them instead of including them
            // in the intersection.
            .filter_map(|p| p.publish().as_deref())
            .map(|p| p.iter().collect::<HashSet<_>>())
            .reduce(|xs, ys| xs.intersection(&ys).cloned().collect())
            .unwrap_or_default();
        if common_regs.is_empty() {
            bail!("conflicts between `package.publish` fields in the selected packages");
        } else {
            bail!("--registry is required because not all `package.publish` settings agree",);
        }
    }
}
