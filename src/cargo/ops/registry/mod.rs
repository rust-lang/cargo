//! Operations that interact with the [registry web API][1].
//!
//! [1]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html

mod login;
mod publish;
mod search;

use std::collections::HashSet;
use std::path::PathBuf;
use std::str;
use std::task::Poll;
use std::time::Duration;

use anyhow::{bail, format_err, Context as _};
use crates_io::{self, Registry};
use curl::easy::{Easy, InfoType, SslOpt, SslVersion};
use log::{log, Level};

use crate::core::source::Source;
use crate::core::{SourceId, Workspace};
use crate::sources::{RegistrySource, SourceConfigMap};
use crate::util::auth::{self, Secret};
use crate::util::config::{Config, SslVersionConfig, SslVersionConfigRange};
use crate::util::errors::CargoResult;
use crate::util::important_paths::find_root_manifest_for_wd;
use crate::util::network;
use crate::util::IntoUrl;
use crate::{drop_print, drop_println, version};

pub use self::login::registry_login;
pub use self::publish::publish;
pub use self::publish::PublishOpts;
pub use self::search::search;

/// Registry settings loaded from config files.
///
/// This is loaded based on the `--registry` flag and the config settings.
#[derive(Debug, PartialEq)]
pub enum RegistryCredentialConfig {
    None,
    /// The authentication token.
    Token(Secret<String>),
    /// Process used for fetching a token.
    Process((PathBuf, Vec<String>)),
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
    pub fn as_process(&self) -> Option<&(PathBuf, Vec<String>)> {
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
    index: Option<&str>,
    registry: Option<&str>,
    force_update: bool,
    token_required: Option<auth::Mutation<'_>>,
) -> CargoResult<(Registry, RegistrySourceIds)> {
    let source_ids = get_source_id(config, index, registry)?;

    if token_required.is_some() && index.is_some() && token_from_cmdline.is_none() {
        bail!("command-line argument --index requires --token to be specified");
    }
    if let Some(token) = token_from_cmdline {
        auth::cache_token(config, &source_ids.original, token);
    }

    let cfg = {
        let _lock = config.acquire_package_cache_lock()?;
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
        Some(auth::auth_token(
            config,
            &source_ids.original,
            None,
            token_required,
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

/// Creates a new HTTP handle with appropriate global configuration for cargo.
pub fn http_handle(config: &Config) -> CargoResult<Easy> {
    let (mut handle, timeout) = http_handle_and_timeout(config)?;
    timeout.configure(&mut handle)?;
    Ok(handle)
}

pub fn http_handle_and_timeout(config: &Config) -> CargoResult<(Easy, HttpTimeout)> {
    if config.frozen() {
        bail!(
            "attempting to make an HTTP request, but --frozen was \
             specified"
        )
    }
    if config.offline() {
        bail!(
            "attempting to make an HTTP request, but --offline was \
             specified"
        )
    }

    // The timeout option for libcurl by default times out the entire transfer,
    // but we probably don't want this. Instead we only set timeouts for the
    // connect phase as well as a "low speed" timeout so if we don't receive
    // many bytes in a large-ish period of time then we time out.
    let mut handle = Easy::new();
    let timeout = configure_http_handle(config, &mut handle)?;
    Ok((handle, timeout))
}

// Only use a custom transport if any HTTP options are specified,
// such as proxies or custom certificate authorities.
//
// The custom transport, however, is not as well battle-tested.
pub fn needs_custom_http_transport(config: &Config) -> CargoResult<bool> {
    Ok(
        network::proxy::http_proxy_exists(config.http_config()?, config)
            || *config.http_config()? != Default::default()
            || config.get_env_os("HTTP_TIMEOUT").is_some(),
    )
}

/// Configure a libcurl http handle with the defaults options for Cargo
pub fn configure_http_handle(config: &Config, handle: &mut Easy) -> CargoResult<HttpTimeout> {
    let http = config.http_config()?;
    if let Some(proxy) = network::proxy::http_proxy(http) {
        handle.proxy(&proxy)?;
    }
    if let Some(cainfo) = &http.cainfo {
        let cainfo = cainfo.resolve_path(config);
        handle.cainfo(&cainfo)?;
    }
    if let Some(check) = http.check_revoke {
        handle.ssl_options(SslOpt::new().no_revoke(!check))?;
    }

    if let Some(user_agent) = &http.user_agent {
        handle.useragent(user_agent)?;
    } else {
        handle.useragent(&format!("cargo {}", version()))?;
    }

    fn to_ssl_version(s: &str) -> CargoResult<SslVersion> {
        let version = match s {
            "default" => SslVersion::Default,
            "tlsv1" => SslVersion::Tlsv1,
            "tlsv1.0" => SslVersion::Tlsv10,
            "tlsv1.1" => SslVersion::Tlsv11,
            "tlsv1.2" => SslVersion::Tlsv12,
            "tlsv1.3" => SslVersion::Tlsv13,
            _ => bail!(
                "Invalid ssl version `{s}`,\
                 choose from 'default', 'tlsv1', 'tlsv1.0', 'tlsv1.1', 'tlsv1.2', 'tlsv1.3'."
            ),
        };
        Ok(version)
    }

    // Empty string accept encoding expands to the encodings supported by the current libcurl.
    handle.accept_encoding("")?;
    if let Some(ssl_version) = &http.ssl_version {
        match ssl_version {
            SslVersionConfig::Single(s) => {
                let version = to_ssl_version(s.as_str())?;
                handle.ssl_version(version)?;
            }
            SslVersionConfig::Range(SslVersionConfigRange { min, max }) => {
                let min_version = min
                    .as_ref()
                    .map_or(Ok(SslVersion::Default), |s| to_ssl_version(s))?;
                let max_version = max
                    .as_ref()
                    .map_or(Ok(SslVersion::Default), |s| to_ssl_version(s))?;
                handle.ssl_min_max_version(min_version, max_version)?;
            }
        }
    } else if cfg!(windows) {
        // This is a temporary workaround for some bugs with libcurl and
        // schannel and TLS 1.3.
        //
        // Our libcurl on Windows is usually built with schannel.
        // On Windows 11 (or Windows Server 2022), libcurl recently (late
        // 2022) gained support for TLS 1.3 with schannel, and it now defaults
        // to 1.3. Unfortunately there have been some bugs with this.
        // https://github.com/curl/curl/issues/9431 is the most recent. Once
        // that has been fixed, and some time has passed where we can be more
        // confident that the 1.3 support won't cause issues, this can be
        // removed.
        //
        // Windows 10 is unaffected. libcurl does not support TLS 1.3 on
        // Windows 10. (Windows 10 sorta had support, but it required enabling
        // an advanced option in the registry which was buggy, and libcurl
        // does runtime checks to prevent it.)
        handle.ssl_min_max_version(SslVersion::Default, SslVersion::Tlsv12)?;
    }

    if let Some(true) = http.debug {
        handle.verbose(true)?;
        log::debug!("{:#?}", curl::Version::get());
        handle.debug_function(|kind, data| {
            let (prefix, level) = match kind {
                InfoType::Text => ("*", Level::Debug),
                InfoType::HeaderIn => ("<", Level::Debug),
                InfoType::HeaderOut => (">", Level::Debug),
                InfoType::DataIn => ("{", Level::Trace),
                InfoType::DataOut => ("}", Level::Trace),
                InfoType::SslDataIn | InfoType::SslDataOut => return,
                _ => return,
            };
            let starts_with_ignore_case = |line: &str, text: &str| -> bool {
                line[..line.len().min(text.len())].eq_ignore_ascii_case(text)
            };
            match str::from_utf8(data) {
                Ok(s) => {
                    for mut line in s.lines() {
                        if starts_with_ignore_case(line, "authorization:") {
                            line = "Authorization: [REDACTED]";
                        } else if starts_with_ignore_case(line, "h2h3 [authorization:") {
                            line = "h2h3 [Authorization: [REDACTED]]";
                        } else if starts_with_ignore_case(line, "set-cookie") {
                            line = "set-cookie: [REDACTED]";
                        }
                        log!(level, "http-debug: {} {}", prefix, line);
                    }
                }
                Err(_) => {
                    log!(
                        level,
                        "http-debug: {} ({} bytes of data)",
                        prefix,
                        data.len()
                    );
                }
            }
        })?;
    }

    HttpTimeout::new(config)
}

#[must_use]
pub struct HttpTimeout {
    pub dur: Duration,
    pub low_speed_limit: u32,
}

impl HttpTimeout {
    pub fn new(config: &Config) -> CargoResult<HttpTimeout> {
        let http_config = config.http_config()?;
        let low_speed_limit = http_config.low_speed_limit.unwrap_or(10);
        let seconds = http_config
            .timeout
            .or_else(|| {
                config
                    .get_env("HTTP_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(30);
        Ok(HttpTimeout {
            dur: Duration::new(seconds, 0),
            low_speed_limit,
        })
    }

    pub fn configure(&self, handle: &mut Easy) -> CargoResult<()> {
        // The timeout option for libcurl by default times out the entire
        // transfer, but we probably don't want this. Instead we only set
        // timeouts for the connect phase as well as a "low speed" timeout so
        // if we don't receive many bytes in a large-ish period of time then we
        // time out.
        handle.connect_timeout(self.dur)?;
        handle.low_speed_time(self.dur)?;
        handle.low_speed_limit(self.low_speed_limit)?;
        Ok(())
    }
}

pub fn registry_logout(config: &Config, reg: Option<&str>) -> CargoResult<()> {
    let source_ids = get_source_id(config, None, reg)?;
    let reg_cfg = auth::registry_credential_config(config, &source_ids.original)?;
    let reg_name = source_ids.original.display_registry_name();
    if reg_cfg.is_none() {
        config.shell().status(
            "Logout",
            format!("not currently logged in to `{}`", reg_name),
        )?;
        return Ok(());
    }
    auth::logout(config, &source_ids.original)?;
    config.shell().status(
        "Logout",
        format!(
            "token for `{}` has been removed from local storage",
            reg_name
        ),
    )?;
    let location = if source_ids.original.is_crates_io() {
        "<https://crates.io/me>".to_string()
    } else {
        // The URL for the source requires network access to load the config.
        // That could be a fairly heavy operation to perform just to provide a
        // help message, so for now this just provides some generic text.
        // Perhaps in the future this could have an API to fetch the config if
        // it is cached, but avoid network access otherwise?
        format!("the `{reg_name}` website")
    };
    config.shell().note(format!(
        "This does not revoke the token on the registry server.\n    \
        If you need to revoke the token, visit {location} and follow the instructions there."
    ))?;
    Ok(())
}

pub struct OwnersOptions {
    pub krate: Option<String>,
    pub token: Option<Secret<String>>,
    pub index: Option<String>,
    pub to_add: Option<Vec<String>>,
    pub to_remove: Option<Vec<String>>,
    pub list: bool,
    pub registry: Option<String>,
}

pub fn modify_owners(config: &Config, opts: &OwnersOptions) -> CargoResult<()> {
    let name = match opts.krate {
        Some(ref name) => name.clone(),
        None => {
            let manifest_path = find_root_manifest_for_wd(config.cwd())?;
            let ws = Workspace::new(&manifest_path, config)?;
            ws.current()?.package_id().name().to_string()
        }
    };

    let mutation = auth::Mutation::Owners { name: &name };

    let (mut registry, _) = registry(
        config,
        opts.token.as_ref().map(Secret::as_deref),
        opts.index.as_deref(),
        opts.registry.as_deref(),
        true,
        Some(mutation),
    )?;

    if let Some(ref v) = opts.to_add {
        let v = v.iter().map(|s| &s[..]).collect::<Vec<_>>();
        let msg = registry.add_owners(&name, &v).with_context(|| {
            format!(
                "failed to invite owners to crate `{}` on registry at {}",
                name,
                registry.host()
            )
        })?;

        config.shell().status("Owner", msg)?;
    }

    if let Some(ref v) = opts.to_remove {
        let v = v.iter().map(|s| &s[..]).collect::<Vec<_>>();
        config
            .shell()
            .status("Owner", format!("removing {:?} from crate {}", v, name))?;
        registry.remove_owners(&name, &v).with_context(|| {
            format!(
                "failed to remove owners from crate `{}` on registry at {}",
                name,
                registry.host()
            )
        })?;
    }

    if opts.list {
        let owners = registry.list_owners(&name).with_context(|| {
            format!(
                "failed to list owners of crate `{}` on registry at {}",
                name,
                registry.host()
            )
        })?;
        for owner in owners.iter() {
            drop_print!(config, "{}", owner.login);
            match (owner.name.as_ref(), owner.email.as_ref()) {
                (Some(name), Some(email)) => drop_println!(config, " ({} <{}>)", name, email),
                (Some(s), None) | (None, Some(s)) => drop_println!(config, " ({})", s),
                (None, None) => drop_println!(config),
            }
        }
    }

    Ok(())
}

pub fn yank(
    config: &Config,
    krate: Option<String>,
    version: Option<String>,
    token: Option<Secret<String>>,
    index: Option<String>,
    undo: bool,
    reg: Option<String>,
) -> CargoResult<()> {
    let name = match krate {
        Some(name) => name,
        None => {
            let manifest_path = find_root_manifest_for_wd(config.cwd())?;
            let ws = Workspace::new(&manifest_path, config)?;
            ws.current()?.package_id().name().to_string()
        }
    };
    let version = match version {
        Some(v) => v,
        None => bail!("a version must be specified to yank"),
    };

    let message = if undo {
        auth::Mutation::Unyank {
            name: &name,
            vers: &version,
        }
    } else {
        auth::Mutation::Yank {
            name: &name,
            vers: &version,
        }
    };

    let (mut registry, _) = registry(
        config,
        token.as_ref().map(Secret::as_deref),
        index.as_deref(),
        reg.as_deref(),
        true,
        Some(message),
    )?;

    let package_spec = format!("{}@{}", name, version);
    if undo {
        config.shell().status("Unyank", package_spec)?;
        registry.unyank(&name, &version).with_context(|| {
            format!(
                "failed to undo a yank from the registry at {}",
                registry.host()
            )
        })?;
    } else {
        config.shell().status("Yank", package_spec)?;
        registry
            .yank(&name, &version)
            .with_context(|| format!("failed to yank from the registry at {}", registry.host()))?;
    }

    Ok(())
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
    index: Option<&str>,
    reg: Option<&str>,
) -> CargoResult<RegistrySourceIds> {
    let sid = match (reg, index) {
        (None, None) => SourceId::crates_io(config)?,
        (_, Some(i)) => SourceId::for_registry(&i.into_url()?)?,
        (Some(r), None) => SourceId::alt_registry(config, r)?,
    };
    // Load source replacements that are built-in to Cargo.
    let builtin_replacement_sid = SourceConfigMap::empty(config)?
        .load(sid, &HashSet::new())?
        .replaced_source_id();
    let replacement_sid = SourceConfigMap::new(config)?
        .load(sid, &HashSet::new())?
        .replaced_source_id();
    if reg.is_none() && index.is_none() && replacement_sid != builtin_replacement_sid {
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
