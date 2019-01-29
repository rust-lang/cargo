use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufRead};
use std::iter::repeat;
use std::str;
use std::time::Duration;
use std::{cmp, env};

use crates_io::{NewCrate, NewCrateDependency, Registry};
use curl::easy::{Easy, InfoType, SslOpt};
use log::{log, Level};
use url::percent_encoding::{percent_encode, QUERY_ENCODE_SET};

use crate::core::dependency::Kind;
use crate::core::manifest::ManifestMetadata;
use crate::core::source::Source;
use crate::core::{Package, SourceId, Workspace};
use crate::ops;
use crate::sources::{RegistrySource, SourceConfigMap};
use crate::util::config::{self, Config};
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::important_paths::find_root_manifest_for_wd;
use crate::util::ToUrl;
use crate::util::{paths, validate_package_name};
use crate::version;

pub struct RegistryConfig {
    pub index: Option<String>,
    pub token: Option<String>,
}

pub struct PublishOpts<'cfg> {
    pub config: &'cfg Config,
    pub token: Option<String>,
    pub index: Option<String>,
    pub verify: bool,
    pub allow_dirty: bool,
    pub jobs: Option<u32>,
    pub target: Option<String>,
    pub dry_run: bool,
    pub registry: Option<String>,
    pub features: Vec<String>,
    pub all_features: bool,
    pub no_default_features: bool,
}

pub fn publish(ws: &Workspace<'_>, opts: &PublishOpts<'_>) -> CargoResult<()> {
    let pkg = ws.current()?;

    if let Some(ref allowed_registries) = *pkg.publish() {
        if !match opts.registry {
            Some(ref registry) => allowed_registries.contains(registry),
            None => false,
        } {
            failure::bail!(
                "some crates cannot be published.\n\
                 `{}` is marked as unpublishable",
                pkg.name()
            );
        }
    }

    if !pkg.manifest().patch().is_empty() {
        failure::bail!("published crates cannot contain [patch] sections");
    }

    let (mut registry, reg_id) = registry(
        opts.config,
        opts.token.clone(),
        opts.index.clone(),
        opts.registry.clone(),
        true,
    )?;
    verify_dependencies(pkg, &registry, reg_id)?;

    // Prepare a tarball, with a non-surpressable warning if metadata
    // is missing since this is being put online.
    let tarball = ops::package(
        ws,
        &ops::PackageOpts {
            config: opts.config,
            verify: opts.verify,
            list: false,
            check_metadata: true,
            allow_dirty: opts.allow_dirty,
            target: opts.target.clone(),
            jobs: opts.jobs,
            features: opts.features.clone(),
            all_features: opts.all_features,
            no_default_features: opts.no_default_features,
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
        if dep.source_id().is_path() {
            if !dep.specified_req() {
                failure::bail!(
                    "all path dependencies must have a version specified \
                     when publishing.\ndependency `{}` does not specify \
                     a version",
                    dep.package_name()
                )
            }
        } else if dep.source_id() != registry_src {
            if dep.source_id().is_registry() {
                // Block requests to send to crates.io with alt-registry deps.
                // This extra hostname check is mostly to assist with testing,
                // but also prevents someone using `--index` to specify
                // something that points to crates.io.
                let is_crates_io = registry
                    .host()
                    .to_url()
                    .map(|u| u.host_str() == Some("crates.io"))
                    .unwrap_or(false);
                if registry_src.is_default_registry() || is_crates_io {
                    failure::bail!("crates cannot be published to crates.io with dependencies sourced from other\n\
                           registries either publish `{}` on crates.io or pull it into this repository\n\
                           and specify it with a path and version\n\
                           (crate `{}` is pulled from {})",
                          dep.package_name(),
                          dep.package_name(),
                          dep.source_id());
                }
            } else {
                failure::bail!(
                    "crates cannot be published with dependencies sourced from \
                     a repository\neither publish `{}` as its own crate and \
                     specify a version as a dependency or pull it into this \
                     repository and specify it with a path and version\n(crate `{}` has \
                     repository path `{}`)",
                    dep.package_name(),
                    dep.package_name(),
                    dep.source_id()
                );
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
                    Kind::Normal => "normal",
                    Kind::Build => "build",
                    Kind::Development => "dev",
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
    let readme_content = match *readme {
        Some(ref readme) => Some(paths::read(&pkg.root().join(readme))?),
        None => None,
    };
    if let Some(ref file) = *license_file {
        if fs::metadata(&pkg.root().join(file)).is_err() {
            failure::bail!("the license file `{}` does not exist", file)
        }
    }

    // Do not upload if performing a dry run
    if dry_run {
        config.shell().warn("aborting upload due to dry run")?;
        return Ok(());
    }

    let summary = pkg.summary();
    let string_features = summary
        .features()
        .iter()
        .map(|(feat, values)| {
            (
                feat.to_string(),
                values.iter().map(|fv| fv.to_string(&summary)).collect(),
            )
        })
        .collect::<BTreeMap<String, Vec<String>>>();

    let publish = registry.publish(
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
        },
        tarball,
    );

    match publish {
        Ok(warnings) => {
            if !warnings.invalid_categories.is_empty() {
                let msg = format!(
                    "\
                     the following are not valid category slugs and were \
                     ignored: {}. Please see https://crates.io/category_slugs \
                     for the list of all category slugs. \
                     ",
                    warnings.invalid_categories.join(", ")
                );
                config.shell().warn(&msg)?;
            }

            if !warnings.invalid_badges.is_empty() {
                let msg = format!(
                    "\
                     the following are not valid badges and were ignored: {}. \
                     Either the badge type specified is unknown or a required \
                     attribute is missing. Please see \
                     http://doc.crates.io/manifest.html#package-metadata \
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
        Err(e) => Err(e),
    }
}

pub fn registry_configuration(
    config: &Config,
    registry: Option<String>,
) -> CargoResult<RegistryConfig> {
    let (index, token) = match registry {
        Some(registry) => {
            validate_package_name(&registry, "registry name", "")?;
            (
                Some(config.get_registry_index(&registry)?.to_string()),
                config
                    .get_string(&format!("registries.{}.token", registry))?
                    .map(|p| p.val),
            )
        }
        None => {
            // Checking out for default index and token
            (
                config.get_string("registry.index")?.map(|p| p.val),
                config.get_string("registry.token")?.map(|p| p.val),
            )
        }
    };

    Ok(RegistryConfig { index, token })
}

pub fn registry(
    config: &Config,
    token: Option<String>,
    index: Option<String>,
    registry: Option<String>,
    force_update: bool,
) -> CargoResult<(Registry, SourceId)> {
    // Parse all configuration options
    let RegistryConfig {
        token: token_config,
        index: index_config,
    } = registry_configuration(config, registry.clone())?;
    let token = token.or(token_config);
    let sid = get_source_id(config, index_config.or(index), registry)?;
    let api_host = {
        let mut src = RegistrySource::remote(sid, &HashSet::new(), config);
        // Only update the index if the config is not available or `force` is set.
        let cfg = src.config();
        let cfg = if force_update || cfg.is_err() {
            src.update()
                .chain_err(|| format!("failed to update {}", sid))?;
            cfg.or_else(|_| src.config())?
        } else {
            cfg.unwrap()
        };
        cfg.and_then(|cfg| cfg.api)
            .ok_or_else(|| failure::format_err!("{} does not support API commands", sid))?
    };
    let handle = http_handle(config)?;
    Ok((Registry::new_handle(api_host, token, handle), sid))
}

/// Create a new HTTP handle with appropriate global configuration for cargo.
pub fn http_handle(config: &Config) -> CargoResult<Easy> {
    let (mut handle, timeout) = http_handle_and_timeout(config)?;
    timeout.configure(&mut handle)?;
    Ok(handle)
}

pub fn http_handle_and_timeout(config: &Config) -> CargoResult<(Easy, HttpTimeout)> {
    if config.frozen() {
        failure::bail!(
            "attempting to make an HTTP request, but --frozen was \
             specified"
        )
    }
    if !config.network_allowed() {
        failure::bail!("can't make HTTP request in the offline mode")
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
    let proxy_exists = http_proxy_exists(config)?;
    let timeout = HttpTimeout::new(config)?.is_non_default();
    let cainfo = config.get_path("http.cainfo")?;
    let check_revoke = config.get_bool("http.check-revoke")?;
    let user_agent = config.get_string("http.user-agent")?;

    Ok(proxy_exists
        || timeout
        || cainfo.is_some()
        || check_revoke.is_some()
        || user_agent.is_some())
}

/// Configure a libcurl http handle with the defaults options for Cargo
pub fn configure_http_handle(config: &Config, handle: &mut Easy) -> CargoResult<HttpTimeout> {
    if let Some(proxy) = http_proxy(config)? {
        handle.proxy(&proxy)?;
    }
    if let Some(cainfo) = config.get_path("http.cainfo")? {
        handle.cainfo(&cainfo.val)?;
    }
    if let Some(check) = config.get_bool("http.check-revoke")? {
        handle.ssl_options(SslOpt::new().no_revoke(!check.val))?;
    }
    if let Some(user_agent) = config.get_string("http.user-agent")? {
        handle.useragent(&user_agent.val)?;
    } else {
        handle.useragent(&version().to_string())?;
    }

    if let Some(true) = config.get::<Option<bool>>("http.debug")? {
        handle.verbose(true)?;
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
                    for line in s.lines() {
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
        let low_speed_limit = config
            .get::<Option<u32>>("http.low-speed-limit")?
            .unwrap_or(10);
        let seconds = config
            .get::<Option<u64>>("http.timeout")?
            .or_else(|| env::var("HTTP_TIMEOUT").ok().and_then(|s| s.parse().ok()))
            .unwrap_or(30);
        Ok(HttpTimeout {
            dur: Duration::new(seconds, 0),
            low_speed_limit,
        })
    }

    fn is_non_default(&self) -> bool {
        self.dur != Duration::new(30, 0) || self.low_speed_limit != 10
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

/// Find an explicit HTTP proxy if one is available.
///
/// Favor cargo's `http.proxy`, then git's `http.proxy`. Proxies specified
/// via environment variables are picked up by libcurl.
fn http_proxy(config: &Config) -> CargoResult<Option<String>> {
    if let Some(s) = config.get_string("http.proxy")? {
        return Ok(Some(s.val));
    }
    if let Ok(cfg) = git2::Config::open_default() {
        if let Ok(s) = cfg.get_str("http.proxy") {
            return Ok(Some(s.to_string()));
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
    let (registry, _) = registry(config, token.clone(), None, reg.clone(), false)?;

    let token = match token {
        Some(token) => token,
        None => {
            println!(
                "please visit {}/me and paste the API Token below",
                registry.host()
            );
            let mut line = String::new();
            let input = io::stdin();
            input
                .lock()
                .read_line(&mut line)
                .chain_err(|| "failed to read stdin")
                .map_err(failure::Error::from)?;
            line.trim().to_string()
        }
    };

    let RegistryConfig {
        token: old_token, ..
    } = registry_configuration(config, reg.clone())?;

    if let Some(old_token) = old_token {
        if old_token == token {
            config.shell().status("Login", "already logged in")?;
            return Ok(());
        }
    }

    config::save_credentials(config, token, reg.clone())?;
    config.shell().status(
        "Login",
        format!(
            "token for `{}` saved",
            reg.as_ref().map_or("crates.io", String::as_str)
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

    let (mut registry, _) = registry(
        config,
        opts.token.clone(),
        opts.index.clone(),
        opts.registry.clone(),
        true,
    )?;

    if let Some(ref v) = opts.to_add {
        let v = v.iter().map(|s| &s[..]).collect::<Vec<_>>();
        let msg = registry.add_owners(&name, &v).map_err(|e| {
            failure::format_err!("failed to invite owners to crate {}: {}", name, e)
        })?;

        config.shell().status("Owner", msg)?;
    }

    if let Some(ref v) = opts.to_remove {
        let v = v.iter().map(|s| &s[..]).collect::<Vec<_>>();
        config
            .shell()
            .status("Owner", format!("removing {:?} from crate {}", v, name))?;
        registry
            .remove_owners(&name, &v)
            .chain_err(|| format!("failed to remove owners from crate {}", name))?;
    }

    if opts.list {
        let owners = registry
            .list_owners(&name)
            .chain_err(|| format!("failed to list owners of crate {}", name))?;
        for owner in owners.iter() {
            print!("{}", owner.login);
            match (owner.name.as_ref(), owner.email.as_ref()) {
                (Some(name), Some(email)) => println!(" ({} <{}>)", name, email),
                (Some(s), None) | (None, Some(s)) => println!(" ({})", s),
                (None, None) => println!(),
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
        None => failure::bail!("a version must be specified to yank"),
    };

    let (mut registry, _) = registry(config, token, index, reg, true)?;

    if undo {
        config
            .shell()
            .status("Unyank", format!("{}:{}", name, version))?;
        registry
            .unyank(&name, &version)
            .chain_err(|| "failed to undo a yank")?;
    } else {
        config
            .shell()
            .status("Yank", format!("{}:{}", name, version))?;
        registry
            .yank(&name, &version)
            .chain_err(|| "failed to yank")?;
    }

    Ok(())
}

fn get_source_id(
    config: &Config,
    index: Option<String>,
    reg: Option<String>,
) -> CargoResult<SourceId> {
    match (reg, index) {
        (Some(r), _) => SourceId::alt_registry(config, &r),
        (_, Some(i)) => SourceId::for_registry(&i.to_url()?),
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

    let (mut registry, _) = registry(config, None, index, reg, false)?;
    let (crates, total_crates) = registry
        .search(query, limit)
        .chain_err(|| "failed to retrieve search results from the registry")?;

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
        println!("{}", line);
    }

    let search_max_limit = 100;
    if total_crates > limit && limit < search_max_limit {
        println!(
            "... and {} crates more (use --limit N to see more)",
            total_crates - limit
        );
    } else if total_crates > limit && limit >= search_max_limit {
        println!(
            "... and {} crates more (go to http://crates.io/search?q={} to see more)",
            total_crates - limit,
            percent_encode(query.as_bytes(), QUERY_ENCODE_SET)
        );
    }

    Ok(())
}
