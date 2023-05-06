use std::cmp;
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead};
use std::iter::repeat;
use std::path::PathBuf;
use std::str;
use std::task::Poll;
use std::time::Duration;

use anyhow::{anyhow, bail, format_err, Context as _};
use cargo_util::paths;
use crates_io::{self, NewCrate, NewCrateDependency, Registry};
use curl::easy::{Easy, InfoType, SslOpt, SslVersion};
use log::{log, Level};
use pasetors::keys::{AsymmetricKeyPair, Generate};
use pasetors::paserk::FormatAsPaserk;
use termcolor::Color::Green;
use termcolor::ColorSpec;
use url::Url;

use crate::core::dependency::DepKind;
use crate::core::dependency::Dependency;
use crate::core::manifest::ManifestMetadata;
use crate::core::resolver::CliFeatures;
use crate::core::source::Source;
use crate::core::QueryKind;
use crate::core::{Package, SourceId, Workspace};
use crate::ops;
use crate::ops::Packages;
use crate::sources::{RegistrySource, SourceConfigMap, CRATES_IO_DOMAIN, CRATES_IO_REGISTRY};
use crate::util::auth::{
    paserk_public_from_paserk_secret, Secret, {self, AuthorizationError},
};
use crate::util::config::{Config, SslVersionConfig, SslVersionConfigRange};
use crate::util::errors::CargoResult;
use crate::util::important_paths::find_root_manifest_for_wd;
use crate::util::{truncate_with_ellipsis, IntoUrl};
use crate::util::{Progress, ProgressStyle};
use crate::{drop_print, drop_println, version};

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

pub struct PublishOpts<'cfg> {
    pub config: &'cfg Config,
    pub token: Option<Secret<String>>,
    pub index: Option<String>,
    pub verify: bool,
    pub allow_dirty: bool,
    pub jobs: Option<i32>,
    pub keep_going: bool,
    pub to_publish: ops::Packages,
    pub targets: Vec<String>,
    pub dry_run: bool,
    pub registry: Option<String>,
    pub cli_features: CliFeatures,
}

pub fn publish(ws: &Workspace<'_>, opts: &PublishOpts<'_>) -> CargoResult<()> {
    let specs = opts.to_publish.to_package_id_specs(ws)?;
    if specs.len() > 1 {
        bail!("the `-p` argument must be specified to select a single package to publish")
    }
    if Packages::Default == opts.to_publish && ws.is_virtual() {
        bail!("the `-p` argument must be specified in the root of a virtual workspace")
    }
    let member_ids = ws.members().map(|p| p.package_id());
    // Check that the spec matches exactly one member.
    specs[0].query(member_ids)?;
    let mut pkgs = ws.members_with_features(&specs, &opts.cli_features)?;
    // In `members_with_features_old`, it will add "current" package (determined by the cwd)
    // So we need filter
    pkgs = pkgs
        .into_iter()
        .filter(|(m, _)| specs.iter().any(|spec| spec.matches(m.package_id())))
        .collect();
    // Double check. It is safe theoretically, unless logic has updated.
    assert_eq!(pkgs.len(), 1);

    let (pkg, cli_features) = pkgs.pop().unwrap();

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
        if allowed_registries.is_empty() {
            bail!(
                "`{}` cannot be published.\n\
                 `package.publish` is set to `false` or an empty list in Cargo.toml and prevents publishing.",
                pkg.name(),
            );
        } else if !allowed_registries.contains(&reg_name) {
            bail!(
                "`{}` cannot be published.\n\
                 The registry `{}` is not listed in the `package.publish` value in Cargo.toml.",
                pkg.name(),
                reg_name
            );
        }
    }
    // This is only used to confirm that we can create a token before we build the package.
    // This causes the credential provider to be called an extra time, but keeps the same order of errors.
    let ver = pkg.version().to_string();
    let mutation = auth::Mutation::PrePublish;

    let (mut registry, reg_ids) = registry(
        opts.config,
        opts.token.as_ref().map(Secret::as_deref),
        opts.index.as_deref(),
        publish_registry.as_deref(),
        true,
        Some(mutation).filter(|_| !opts.dry_run),
    )?;
    verify_dependencies(pkg, &registry, reg_ids.original)?;

    // Prepare a tarball, with a non-suppressible warning if metadata
    // is missing since this is being put online.
    let tarball = ops::package_one(
        ws,
        pkg,
        &ops::PackageOpts {
            config: opts.config,
            verify: opts.verify,
            list: false,
            check_metadata: true,
            allow_dirty: opts.allow_dirty,
            to_package: ops::Packages::Default,
            targets: opts.targets.clone(),
            jobs: opts.jobs,
            keep_going: opts.keep_going,
            cli_features: cli_features,
        },
    )?
    .unwrap();

    if !opts.dry_run {
        let hash = cargo_util::Sha256::new()
            .update_file(tarball.file())?
            .finish_hex();
        let mutation = Some(auth::Mutation::Publish {
            name: pkg.name().as_str(),
            vers: &ver,
            cksum: &hash,
        });
        registry.set_token(Some(auth::auth_token(
            &opts.config,
            &reg_ids.original,
            None,
            mutation,
        )?));
    }

    opts.config
        .shell()
        .status("Uploading", pkg.package_id().to_string())?;
    transmit(
        opts.config,
        pkg,
        tarball.file(),
        &mut registry,
        reg_ids.original,
        opts.dry_run,
    )?;
    if !opts.dry_run {
        const DEFAULT_TIMEOUT: u64 = 60;
        let timeout = if opts.config.cli_unstable().publish_timeout {
            let timeout: Option<u64> = opts.config.get("publish.timeout")?;
            timeout.unwrap_or(DEFAULT_TIMEOUT)
        } else {
            DEFAULT_TIMEOUT
        };
        if 0 < timeout {
            let timeout = std::time::Duration::from_secs(timeout);
            wait_for_publish(opts.config, reg_ids.original, pkg, timeout)?;
        }
    }

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
            if registry_src.is_crates_io() || registry.host_is_crates_io() {
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
        ref rust_version,
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
                rust_version: rust_version.clone(),
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

fn wait_for_publish(
    config: &Config,
    registry_src: SourceId,
    pkg: &Package,
    timeout: std::time::Duration,
) -> CargoResult<()> {
    let version_req = format!("={}", pkg.version());
    let mut source = SourceConfigMap::empty(config)?.load(registry_src, &HashSet::new())?;
    // Disable the source's built-in progress bars. Repeatedly showing a bunch
    // of independent progress bars can be a little confusing. There is an
    // overall progress bar managed here.
    source.set_quiet(true);
    let source_description = source.source_id().to_string();
    let query = Dependency::parse(pkg.name(), Some(&version_req), registry_src)?;

    let now = std::time::Instant::now();
    let sleep_time = std::time::Duration::from_secs(1);
    let max = timeout.as_secs() as usize;
    // Short does not include the registry name.
    let short_pkg_description = format!("{} v{}", pkg.name(), pkg.version());
    config.shell().status(
        "Uploaded",
        format!("{short_pkg_description} to {source_description}"),
    )?;
    config.shell().note(format!(
        "Waiting for `{short_pkg_description}` to be available at {source_description}.\n\
        You may press ctrl-c to skip waiting; the crate should be available shortly."
    ))?;
    let mut progress = Progress::with_style("Waiting", ProgressStyle::Ratio, config);
    progress.tick_now(0, max, "")?;
    let is_available = loop {
        {
            let _lock = config.acquire_package_cache_lock()?;
            // Force re-fetching the source
            //
            // As pulling from a git source is expensive, we track when we've done it within the
            // process to only do it once, but we are one of the rare cases that needs to do it
            // multiple times
            config
                .updated_sources()
                .remove(&source.replaced_source_id());
            source.invalidate_cache();
            let summaries = loop {
                // Exact to avoid returning all for path/git
                match source.query_vec(&query, QueryKind::Exact) {
                    std::task::Poll::Ready(res) => {
                        break res?;
                    }
                    std::task::Poll::Pending => source.block_until_ready()?,
                }
            };
            if !summaries.is_empty() {
                break true;
            }
        }

        let elapsed = now.elapsed();
        if timeout < elapsed {
            config.shell().warn(format!(
                "timed out waiting for `{short_pkg_description}` to be available in {source_description}",
            ))?;
            config.shell().note(
                "The registry may have a backlog that is delaying making the \
                crate available. The crate should be available soon.",
            )?;
            break false;
        }

        progress.tick_now(elapsed.as_secs() as usize, max, "")?;
        std::thread::sleep(sleep_time);
    };
    if is_available {
        config.shell().status(
            "Published",
            format!("{short_pkg_description} at {source_description}"),
        )?;
    }

    Ok(())
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

pub fn needs_custom_http_transport(config: &Config) -> CargoResult<bool> {
    Ok(http_proxy_exists(config)?
        || *config.http_config()? != Default::default()
        || config.get_env_os("HTTP_TIMEOUT").is_some())
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
            .any(|v| config.get_env(v).is_ok()))
    }
}

pub fn registry_login(
    config: &Config,
    token: Option<Secret<&str>>,
    reg: Option<&str>,
    generate_keypair: bool,
    secret_key_required: bool,
    key_subject: Option<&str>,
) -> CargoResult<()> {
    let source_ids = get_source_id(config, None, reg)?;
    let reg_cfg = auth::registry_credential_config(config, &source_ids.original)?;

    let login_url = match registry(config, token.clone(), None, reg, false, None) {
        Ok((registry, _)) => Some(format!("{}/me", registry.host())),
        Err(e) if e.is::<AuthorizationError>() => e
            .downcast::<AuthorizationError>()
            .unwrap()
            .login_url
            .map(|u| u.to_string()),
        Err(e) => return Err(e),
    };
    let new_token;
    if generate_keypair || secret_key_required || key_subject.is_some() {
        if !config.cli_unstable().registry_auth {
            let flag = if generate_keypair {
                "generate-keypair"
            } else if secret_key_required {
                "secret-key"
            } else if key_subject.is_some() {
                "key-subject"
            } else {
                unreachable!("how did we get here");
            };
            bail!(
                "the `{flag}` flag is unstable, pass `-Z registry-auth` to enable it\n\
                 See https://github.com/rust-lang/cargo/issues/10519 for more \
                 information about the `{flag}` flag."
            );
        }
        assert!(token.is_none());
        // we are dealing with asymmetric tokens
        let (old_secret_key, old_key_subject) = match &reg_cfg {
            RegistryCredentialConfig::AsymmetricKey((old_secret_key, old_key_subject)) => {
                (Some(old_secret_key), old_key_subject.clone())
            }
            _ => (None, None),
        };
        let secret_key: Secret<String>;
        if generate_keypair {
            assert!(!secret_key_required);
            let kp = AsymmetricKeyPair::<pasetors::version3::V3>::generate().unwrap();
            secret_key = Secret::default().map(|mut key| {
                FormatAsPaserk::fmt(&kp.secret, &mut key).unwrap();
                key
            });
        } else if secret_key_required {
            assert!(!generate_keypair);
            drop_println!(config, "please paste the API secret key below");
            secret_key = Secret::default()
                .map(|mut line| {
                    let input = io::stdin();
                    input
                        .lock()
                        .read_line(&mut line)
                        .with_context(|| "failed to read stdin")
                        .map(|_| line.trim().to_string())
                })
                .transpose()?;
        } else {
            secret_key = old_secret_key
                .cloned()
                .ok_or_else(|| anyhow!("need a secret_key to set a key_subject"))?;
        }
        if let Some(p) = paserk_public_from_paserk_secret(secret_key.as_deref()) {
            drop_println!(config, "{}", &p);
        } else {
            bail!("not a validly formatted PASERK secret key");
        }
        new_token = RegistryCredentialConfig::AsymmetricKey((
            secret_key,
            match key_subject {
                Some(key_subject) => Some(key_subject.to_string()),
                None => old_key_subject,
            },
        ));
    } else {
        new_token = RegistryCredentialConfig::Token(match token {
            Some(token) => token.owned(),
            None => {
                if let Some(login_url) = login_url {
                    drop_println!(
                        config,
                        "please paste the token found on {} below",
                        login_url
                    )
                } else {
                    drop_println!(
                        config,
                        "please paste the token for {} below",
                        source_ids.original.display_registry_name()
                    )
                }

                let mut line = String::new();
                let input = io::stdin();
                input
                    .lock()
                    .read_line(&mut line)
                    .with_context(|| "failed to read stdin")?;
                // Automatically remove `cargo login` from an inputted token to
                // allow direct pastes from `registry.host()`/me.
                Secret::from(line.replace("cargo login", "").trim().to_string())
            }
        });

        if let Some(tok) = new_token.as_token() {
            crates_io::check_token(tok.as_ref().expose())?;
        }
    }
    if &reg_cfg == &new_token {
        config.shell().status("Login", "already logged in")?;
        return Ok(());
    }

    auth::login(config, &source_ids.original, new_token)?;

    config.shell().status(
        "Login",
        format!("token for `{}` saved", reg.unwrap_or(CRATES_IO_DOMAIN)),
    )?;
    Ok(())
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

pub fn search(
    query: &str,
    config: &Config,
    index: Option<String>,
    limit: u32,
    reg: Option<String>,
) -> CargoResult<()> {
    let (mut registry, source_ids) =
        registry(config, None, index.as_deref(), reg.as_deref(), false, None)?;
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
        let mut fragments = line.split(query).peekable();
        while let Some(fragment) = fragments.next() {
            let _ = config.shell().write_stdout(fragment, &ColorSpec::new());
            if fragments.peek().is_some() {
                let _ = config
                    .shell()
                    .write_stdout(query, &ColorSpec::new().set_bold(true).set_fg(Some(Green)));
            }
        }
        let _ = config.shell().write_stdout("\n", &ColorSpec::new());
    }

    let search_max_limit = 100;
    if total_crates > limit && limit < search_max_limit {
        let _ = config.shell().write_stdout(
            format_args!(
                "... and {} crates more (use --limit N to see more)\n",
                total_crates - limit
            ),
            &ColorSpec::new(),
        );
    } else if total_crates > limit && limit >= search_max_limit {
        let extra = if source_ids.original.is_crates_io() {
            let url = Url::parse_with_params("https://crates.io/search", &[("q", query)])?;
            format!(" (go to {url} to see more)")
        } else {
            String::new()
        };
        let _ = config.shell().write_stdout(
            format_args!("... and {} crates more{}\n", total_crates - limit, extra),
            &ColorSpec::new(),
        );
    }

    Ok(())
}
