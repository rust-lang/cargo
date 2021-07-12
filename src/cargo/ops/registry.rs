use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead};
use std::iter::repeat;
use std::path::PathBuf;
use std::str;
use std::time::Duration;
use std::{cmp, env};

use anyhow::{bail, format_err, Context as _};
use cargo_util::paths;
use crates_io::{self, NewCrate, NewCrateDependency, Registry};
use curl::easy::{Easy, InfoType, SslOpt, SslVersion};
use log::{log, Level};
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};

use crate::core::dependency::DepKind;
use crate::core::manifest::ManifestMetadata;
use crate::core::resolver::CliFeatures;
use crate::core::source::Source;
use crate::core::{Package, SourceId, Workspace};
use crate::ops;
use crate::sources::{RegistrySource, SourceConfigMap, CRATES_IO_REGISTRY};
use crate::util::config::{self, Config, SslVersionConfig, SslVersionConfigRange};
use crate::util::errors::CargoResult;
use crate::util::important_paths::find_root_manifest_for_wd;
use crate::util::validate_package_name;
use crate::util::IntoUrl;
use crate::{drop_print, drop_println, version};

mod auth;

/// Registry settings loaded from config files.
///
/// This is loaded based on the `--registry` flag and the config settings.
#[derive(Debug)]
pub struct RegistryConfig {
    /// The index URL. If `None`, use crates.io.
    pub index: Option<String>,
    /// The authentication token.
    pub token: Option<String>,
    /// Process used for fetching a token.
    pub credential_process: Option<(PathBuf, Vec<String>)>,
}

pub struct PublishOpts<'cfg> {
    pub config: &'cfg Config,
    pub token: Option<String>,
    pub index: Option<String>,
    pub verify: bool,
    pub allow_dirty: bool,
    pub jobs: Option<u32>,
    pub targets: Vec<String>,
    pub dry_run: bool,
    pub registry: Option<String>,
    pub cli_features: CliFeatures,
}

pub fn publish(ws: &Workspace<'_>, opts: &PublishOpts<'_>) -> CargoResult<()> {
    let pkg = ws.current()?;
    let mut publish_registry = opts.registry.clone();

    if let Some(ref allowed_registries) = *pkg.publish() {
        if publish_registry.is_none() && allowed_registries.len() == 1 {
            // If there is only one allowed registry, push to that one directly,
            // even though there is no registry specified in the command.
            let default_registry = &allowed_registries[0];
            if default_registry != CRATES_IO_REGISTRY {
                // Don't change the registry for crates.io and don't warn the user.
                // crates.io will be defaulted even without this.
                opts.config.shell().note(&format!(
                    "Found `{}` as only allowed registry. Publishing to it automatically.",
                    default_registry
                ))?;
                publish_registry = Some(default_registry.clone());
            }
        }

        let reg_name = publish_registry
            .clone()
            .unwrap_or_else(|| CRATES_IO_REGISTRY.to_string());
        if !allowed_registries.contains(&reg_name) {
            bail!(
                "`{}` cannot be published.\n\
                 The registry `{}` is not listed in the `publish` value in Cargo.toml.",
                pkg.name(),
                reg_name
            );
        }
    }

    let (mut registry, _reg_cfg, reg_id) = registry(
        opts.config,
        opts.token.clone(),
        opts.index.clone(),
        publish_registry,
        true,
        !opts.dry_run,
    )?;
    verify_dependencies(pkg, &registry, reg_id)?;

    // Prepare a tarball, with a non-suppressible warning if metadata
    // is missing since this is being put online.
    let tarball = ops::package(
        ws,
        &ops::PackageOpts {
            config: opts.config,
            verify: opts.verify,
            list: false,
            check_metadata: true,
            allow_dirty: opts.allow_dirty,
            targets: opts.targets.clone(),
            jobs: opts.jobs,
            cli_features: opts.cli_features.clone(),
        },
    )?
    .unwrap();

    // Upload said tarball to the specified destination
    opts.config
        .shell()
        .status("Uploading", pkg.package_id().to_string())?;
    transmit(
        opts.config,
        pkg,
        tarball.file(),
        &mut registry,
        reg_id,
        opts.dry_run,
    )?;

    Ok(())
}

fn verify_dependencies(
    pkg: &Package,
    registry: &Registry,
    registry_src: SourceId,
) -> CargoResult<()> {
    for dep in pkg.dependencies().iter() {
        if super::check_dep_has_version(dep, true)? {
            continue;
        }
        // TomlManifest::prepare_for_publish will rewrite the dependency
        // to be just the `version` field.
        if dep.source_id() != registry_src {
            if !dep.source_id().is_registry() {
                // Consider making SourceId::kind a public type that we can
                // exhaustively match on. Using match can help ensure that
                // every kind is properly handled.
                panic!("unexpected source kind for dependency {:?}", dep);
            }
            // Block requests to send to crates.io with alt-registry deps.
            // This extra hostname check is mostly to assist with testing,
            // but also prevents someone using `--index` to specify
            // something that points to crates.io.
            if registry_src.is_default_registry() || registry.host_is_crates_io() {
                bail!("crates cannot be published to crates.io with dependencies sourced from other\n\
                       registries. `{}` needs to be published to crates.io before publishing this crate.\n\
                       (crate `{}` is pulled from {})",
                      dep.package_name(),
                      dep.package_name(),
                      dep.source_id());
            }
        }
    }
    Ok(())
}

fn transmit(
    config: &Config,
    pkg: &Package,
    tarball: &File,
    registry: &mut Registry,
    registry_id: SourceId,
    dry_run: bool,
) -> CargoResult<()> {
    let deps = pkg
        .dependencies()
        .iter()
        .filter(|dep| {
            // Skip dev-dependency without version.
            dep.is_transitive() || dep.specified_req()
        })
        .map(|dep| {
            // If the dependency is from a different registry, then include the
            // registry in the dependency.
            let dep_registry_id = match dep.registry_id() {
                Some(id) => id,
                None => SourceId::crates_io(config)?,
            };
            // In the index and Web API, None means "from the same registry"
            // whereas in Cargo.toml, it means "from crates.io".
            let dep_registry = if dep_registry_id != registry_id {
                Some(dep_registry_id.url().to_string())
            } else {
                None
            };

            Ok(NewCrateDependency {
                optional: dep.is_optional(),
                default_features: dep.uses_default_features(),
                name: dep.package_name().to_string(),
                features: dep.features().iter().map(|s| s.to_string()).collect(),
                version_req: dep.version_req().to_string(),
                target: dep.platform().map(|s| s.to_string()),
                kind: match dep.kind() {
                    DepKind::Normal => "normal",
                    DepKind::Build => "build",
                    DepKind::Development => "dev",
                }
                .to_string(),
                registry: dep_registry,
                explicit_name_in_toml: dep.explicit_name_in_toml().map(|s| s.to_string()),
            })
        })
        .collect::<CargoResult<Vec<NewCrateDependency>>>()?;
    let manifest = pkg.manifest();
    let ManifestMetadata {
        ref authors,
        ref description,
        ref homepage,
        ref documentation,
        ref keywords,
        ref readme,
        ref repository,
        ref license,
        ref license_file,
        ref categories,
        ref badges,
        ref links,
    } = *manifest.metadata();
    let readme_content = readme
        .as_ref()
        .map(|readme| {
            paths::read(&pkg.root().join(readme))
                .with_context(|| format!("failed to read `readme` file for package `{}`", pkg))
        })
        .transpose()?;
    if let Some(ref file) = *license_file {
        if !pkg.root().join(file).exists() {
            bail!("the license file `{}` does not exist", file)
        }
    }

    // Do not upload if performing a dry run
    if dry_run {
        config.shell().warn("aborting upload due to dry run")?;
        return Ok(());
    }

    let string_features = match manifest.original().features() {
        Some(features) => features
            .iter()
            .map(|(feat, values)| {
                (
                    feat.to_string(),
                    values.iter().map(|fv| fv.to_string()).collect(),
                )
            })
            .collect::<BTreeMap<String, Vec<String>>>(),
        None => BTreeMap::new(),
    };

    let warnings = registry
        .publish(
            &NewCrate {
                name: pkg.name().to_string(),
                vers: pkg.version().to_string(),
                deps,
                features: string_features,
                authors: authors.clone(),
                description: description.clone(),
                homepage: homepage.clone(),
                documentation: documentation.clone(),
                keywords: keywords.clone(),
                categories: categories.clone(),
                readme: readme_content,
                readme_file: readme.clone(),
                repository: repository.clone(),
                license: license.clone(),
                license_file: license_file.clone(),
                badges: badges.clone(),
                links: links.clone(),
                v: None,
            },
            tarball,
        )
        .with_context(|| format!("failed to publish to registry at {}", registry.host()))?;

    if !warnings.invalid_categories.is_empty() {
        let msg = format!(
            "the following are not valid category slugs and were \
             ignored: {}. Please see https://crates.io/category_slugs \
             for the list of all category slugs. \
             ",
            warnings.invalid_categories.join(", ")
        );
        config.shell().warn(&msg)?;
    }

    if !warnings.invalid_badges.is_empty() {
        let msg = format!(
            "the following are not valid badges and were ignored: {}. \
             Either the badge type specified is unknown or a required \
             attribute is missing. Please see \
             https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata \
             for valid badge types and their required attributes.",
            warnings.invalid_badges.join(", ")
        );
        config.shell().warn(&msg)?;
    }

    if !warnings.other.is_empty() {
        for msg in warnings.other {
            config.shell().warn(&msg)?;
        }
    }

    Ok(())
}

/// Returns the index and token from the config file for the given registry.
///
/// `registry` is typically the registry specified on the command-line. If
/// `None`, `index` is set to `None` to indicate it should use crates.io.
pub fn registry_configuration(
    config: &Config,
    registry: Option<&str>,
) -> CargoResult<RegistryConfig> {
    let err_both = |token_key: &str, proc_key: &str| {
        Err(format_err!(
            "both `{TOKEN_KEY}` and `{PROC_KEY}` \
             were specified in the config\n\
             Only one of these values may be set, remove one or the other to proceed.",
            TOKEN_KEY = token_key,
            PROC_KEY = proc_key,
        ))
    };
    // `registry.default` is handled in command-line parsing.
    let (index, token, process) = match registry {
        Some(registry) => {
            validate_package_name(registry, "registry name", "")?;
            let index = Some(config.get_registry_index(registry)?.to_string());
            let token_key = format!("registries.{}.token", registry);
            let token = config.get_string(&token_key)?.map(|p| p.val);
            let process = if config.cli_unstable().credential_process {
                let mut proc_key = format!("registries.{}.credential-process", registry);
                let mut process = config.get::<Option<config::PathAndArgs>>(&proc_key)?;
                if process.is_none() && token.is_none() {
                    // This explicitly ignores the global credential-process if
                    // the token is set, as that is "more specific".
                    proc_key = String::from("registry.credential-process");
                    process = config.get::<Option<config::PathAndArgs>>(&proc_key)?;
                } else if process.is_some() && token.is_some() {
                    return err_both(&token_key, &proc_key);
                }
                process
            } else {
                None
            };
            (index, token, process)
        }
        None => {
            // Use crates.io default.
            config.check_registry_index_not_set()?;
            let token = config.get_string("registry.token")?.map(|p| p.val);
            let process = if config.cli_unstable().credential_process {
                let process =
                    config.get::<Option<config::PathAndArgs>>("registry.credential-process")?;
                if token.is_some() && process.is_some() {
                    return err_both("registry.token", "registry.credential-process");
                }
                process
            } else {
                None
            };
            (None, token, process)
        }
    };

    let credential_process =
        process.map(|process| (process.path.resolve_program(config), process.args));

    Ok(RegistryConfig {
        index,
        token,
        credential_process,
    })
}

/// Returns the `Registry` and `Source` based on command-line and config settings.
///
/// * `token`: The token from the command-line. If not set, uses the token
///   from the config.
/// * `index`: The index URL from the command-line. This is ignored if
///   `registry` is set.
/// * `registry`: The registry name from the command-line. If neither
///   `registry`, or `index` are set, then uses `crates-io`, honoring
///   `[source]` replacement if defined.
/// * `force_update`: If `true`, forces the index to be updated.
/// * `validate_token`: If `true`, the token must be set.
fn registry(
    config: &Config,
    token: Option<String>,
    index: Option<String>,
    registry: Option<String>,
    force_update: bool,
    validate_token: bool,
) -> CargoResult<(Registry, RegistryConfig, SourceId)> {
    if index.is_some() && registry.is_some() {
        // Otherwise we would silently ignore one or the other.
        bail!("both `--index` and `--registry` should not be set at the same time");
    }
    // Parse all configuration options
    let reg_cfg = registry_configuration(config, registry.as_deref())?;
    let opt_index = reg_cfg.index.as_ref().or_else(|| index.as_ref());
    let sid = get_source_id(config, opt_index, registry.as_ref())?;
    if !sid.is_remote_registry() {
        bail!(
            "{} does not support API commands.\n\
             Check for a source-replacement in .cargo/config.",
            sid
        );
    }
    let api_host = {
        let _lock = config.acquire_package_cache_lock()?;
        let mut src = RegistrySource::remote(sid, &HashSet::new(), config);
        // Only update the index if the config is not available or `force` is set.
        let cfg = src.config();
        let mut updated_cfg = || {
            src.update()
                .with_context(|| format!("failed to update {}", sid))?;
            src.config()
        };

        let cfg = if force_update {
            updated_cfg()?
        } else {
            cfg.or_else(|_| updated_cfg())?
        };

        cfg.and_then(|cfg| cfg.api)
            .ok_or_else(|| format_err!("{} does not support API commands", sid))?
    };
    let token = if validate_token {
        if index.is_some() {
            if token.is_none() {
                bail!("command-line argument --index requires --token to be specified");
            }
            token
        } else {
            // Check `is_default_registry` so that the crates.io index can
            // change config.json's "api" value, and this won't affect most
            // people. It will affect those using source replacement, but
            // hopefully that's a relatively small set of users.
            if token.is_none()
                && reg_cfg.token.is_some()
                && registry.is_none()
                && !sid.is_default_registry()
                && !crates_io::is_url_crates_io(&api_host)
            {
                config.shell().warn(
                    "using `registry.token` config value with source \
                        replacement is deprecated\n\
                        This may become a hard error in the future; \
                        see <https://github.com/rust-lang/cargo/issues/xxx>.\n\
                        Use the --token command-line flag to remove this warning.",
                )?;
                reg_cfg.token.clone()
            } else {
                let token = auth::auth_token(
                    config,
                    token.as_deref(),
                    reg_cfg.token.as_deref(),
                    reg_cfg.credential_process.as_ref(),
                    registry.as_deref(),
                    &api_host,
                )?;
                log::debug!("found token {:?}", token);
                Some(token)
            }
        }
    } else {
        None
    };
    let handle = http_handle(config)?;
    Ok((Registry::new_handle(api_host, token, handle), reg_cfg, sid))
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
    if !config.network_allowed() {
        bail!("can't make HTTP request in the offline mode")
    }

    // The timeout option for libcurl by default times out the entire transfer,
    // but we probably don't want this. Instead we only set timeouts for the
    // connect phase as well as a "low speed" timeout so if we don't receive
    // many bytes in a large-ish period of time then we time out.
    let mut handle = Easy::new();
    let timeout = configure_http_handle(config, &mut handle)?;
    Ok((handle, timeout))
}

pub fn needs_custom_http_transport(config: &Config) -> CargoResult<bool> {
    Ok(http_proxy_exists(config)?
        || *config.http_config()? != Default::default()
        || env::var_os("HTTP_TIMEOUT").is_some())
}

/// Configure a libcurl http handle with the defaults options for Cargo
pub fn configure_http_handle(config: &Config, handle: &mut Easy) -> CargoResult<HttpTimeout> {
    let http = config.http_config()?;
    if let Some(proxy) = http_proxy(config)? {
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
                "Invalid ssl version `{}`,\
                 choose from 'default', 'tlsv1', 'tlsv1.0', 'tlsv1.1', 'tlsv1.2', 'tlsv1.3'.",
                s
            ),
        };
        Ok(version)
    }
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
            match str::from_utf8(data) {
                Ok(s) => {
                    for mut line in s.lines() {
                        if line.starts_with("Authorization:") {
                            line = "Authorization: [REDACTED]";
                        } else if line[..line.len().min(10)].eq_ignore_ascii_case("set-cookie") {
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
        let config = config.http_config()?;
        let low_speed_limit = config.low_speed_limit.unwrap_or(10);
        let seconds = config
            .timeout
            .or_else(|| env::var("HTTP_TIMEOUT").ok().and_then(|s| s.parse().ok()))
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

/// Finds an explicit HTTP proxy if one is available.
///
/// Favor cargo's `http.proxy`, then git's `http.proxy`. Proxies specified
/// via environment variables are picked up by libcurl.
fn http_proxy(config: &Config) -> CargoResult<Option<String>> {
    let http = config.http_config()?;
    if let Some(s) = &http.proxy {
        return Ok(Some(s.clone()));
    }
    if let Ok(cfg) = git2::Config::open_default() {
        if let Ok(s) = cfg.get_string("http.proxy") {
            return Ok(Some(s));
        }
    }
    Ok(None)
}

/// Determine if an http proxy exists.
///
/// Checks the following for existence, in order:
///
/// * cargo's `http.proxy`
/// * git's `http.proxy`
/// * `http_proxy` env var
/// * `HTTP_PROXY` env var
/// * `https_proxy` env var
/// * `HTTPS_PROXY` env var
fn http_proxy_exists(config: &Config) -> CargoResult<bool> {
    if http_proxy(config)?.is_some() {
        Ok(true)
    } else {
        Ok(["http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY"]
            .iter()
            .any(|v| env::var(v).is_ok()))
    }
}

pub fn registry_login(
    config: &Config,
    token: Option<String>,
    reg: Option<String>,
) -> CargoResult<()> {
    let (registry, reg_cfg, _) = registry(config, token.clone(), None, reg.clone(), false, false)?;

    let token = match token {
        Some(token) => token,
        None => {
            drop_println!(
                config,
                "please paste the API Token found on {}/me below",
                registry.host()
            );
            let mut line = String::new();
            let input = io::stdin();
            input
                .lock()
                .read_line(&mut line)
                .with_context(|| "failed to read stdin")?;
            // Automatically remove `cargo login` from an inputted token to
            // allow direct pastes from `registry.host()`/me.
            line.replace("cargo login", "").trim().to_string()
        }
    };

    if let Some(old_token) = &reg_cfg.token {
        if old_token == &token {
            config.shell().status("Login", "already logged in")?;
            return Ok(());
        }
    }

    auth::login(
        config,
        token,
        reg_cfg.credential_process.as_ref(),
        reg.as_deref(),
        registry.host(),
    )?;

    config.shell().status(
        "Login",
        format!(
            "token for `{}` saved",
            reg.as_ref().map_or("crates.io", String::as_str)
        ),
    )?;
    Ok(())
}

pub fn registry_logout(config: &Config, reg: Option<String>) -> CargoResult<()> {
    let (registry, reg_cfg, _) = registry(config, None, None, reg.clone(), false, false)?;
    let reg_name = reg.as_deref().unwrap_or("crates.io");
    if reg_cfg.credential_process.is_none() && reg_cfg.token.is_none() {
        config.shell().status(
            "Logout",
            format!("not currently logged in to `{}`", reg_name),
        )?;
        return Ok(());
    }
    auth::logout(
        config,
        reg_cfg.credential_process.as_ref(),
        reg.as_deref(),
        registry.host(),
    )?;
    config.shell().status(
        "Logout",
        format!(
            "token for `{}` has been removed from local storage",
            reg_name
        ),
    )?;
    Ok(())
}

pub struct OwnersOptions {
    pub krate: Option<String>,
    pub token: Option<String>,
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

    let (mut registry, _, _) = registry(
        config,
        opts.token.clone(),
        opts.index.clone(),
        opts.registry.clone(),
        true,
        true,
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
    token: Option<String>,
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

    let (mut registry, _, _) = registry(config, token, index, reg, true, true)?;

    if undo {
        config
            .shell()
            .status("Unyank", format!("{}:{}", name, version))?;
        registry.unyank(&name, &version).with_context(|| {
            format!(
                "failed to undo a yank from the registry at {}",
                registry.host()
            )
        })?;
    } else {
        config
            .shell()
            .status("Yank", format!("{}:{}", name, version))?;
        registry
            .yank(&name, &version)
            .with_context(|| format!("failed to yank from the registry at {}", registry.host()))?;
    }

    Ok(())
}

/// Gets the SourceId for an index or registry setting.
///
/// The `index` and `reg` values are from the command-line or config settings.
/// If both are None, returns the source for crates.io.
fn get_source_id(
    config: &Config,
    index: Option<&String>,
    reg: Option<&String>,
) -> CargoResult<SourceId> {
    match (reg, index) {
        (Some(r), _) => SourceId::alt_registry(config, r),
        (_, Some(i)) => SourceId::for_registry(&i.into_url()?),
        _ => {
            let map = SourceConfigMap::new(config)?;
            let src = map.load(SourceId::crates_io(config)?, &HashSet::new())?;
            Ok(src.replaced_source_id())
        }
    }
}

pub fn search(
    query: &str,
    config: &Config,
    index: Option<String>,
    limit: u32,
    reg: Option<String>,
) -> CargoResult<()> {
    fn truncate_with_ellipsis(s: &str, max_width: usize) -> String {
        // We should truncate at grapheme-boundary and compute character-widths,
        // yet the dependencies on unicode-segmentation and unicode-width are
        // not worth it.
        let mut chars = s.chars();
        let mut prefix = (&mut chars).take(max_width - 1).collect::<String>();
        if chars.next().is_some() {
            prefix.push('…');
        }
        prefix
    }

    let (mut registry, _, source_id) = registry(config, None, index, reg, false, false)?;
    let (crates, total_crates) = registry.search(query, limit).with_context(|| {
        format!(
            "failed to retrieve search results from the registry at {}",
            registry.host()
        )
    })?;

    let names = crates
        .iter()
        .map(|krate| format!("{} = \"{}\"", krate.name, krate.max_version))
        .collect::<Vec<String>>();

    let description_margin = names.iter().map(|s| s.len() + 4).max().unwrap_or_default();

    let description_length = cmp::max(80, 128 - description_margin);

    let descriptions = crates.iter().map(|krate| {
        krate
            .description
            .as_ref()
            .map(|desc| truncate_with_ellipsis(&desc.replace("\n", " "), description_length))
    });

    for (name, description) in names.into_iter().zip(descriptions) {
        let line = match description {
            Some(desc) => {
                let space = repeat(' ')
                    .take(description_margin - name.len())
                    .collect::<String>();
                name + &space + "# " + &desc
            }
            None => name,
        };
        drop_println!(config, "{}", line);
    }

    let search_max_limit = 100;
    if total_crates > limit && limit < search_max_limit {
        drop_println!(
            config,
            "... and {} crates more (use --limit N to see more)",
            total_crates - limit
        );
    } else if total_crates > limit && limit >= search_max_limit {
        let extra = if source_id.is_default_registry() {
            format!(
                " (go to https://crates.io/search?q={} to see more)",
                percent_encode(query.as_bytes(), NON_ALPHANUMERIC)
            )
        } else {
            String::new()
        };
        drop_println!(
            config,
            "... and {} crates more{}",
            total_crates - limit,
            extra
        );
    }

    Ok(())
}
