use std::env;
use std::fs::{self, File};
use std::iter::repeat;
use std::time::Duration;

use curl::easy::{Easy, SslOpt};
use git2;
use registry::{Registry, NewCrate, NewCrateDependency};

use url::percent_encoding::{percent_encode, QUERY_ENCODE_SET};

use version;
use core::source::Source;
use core::{Package, SourceId, Workspace};
use core::dependency::Kind;
use core::manifest::ManifestMetadata;
use ops;
use sources::{RegistrySource};
use util::config::{self, Config};
use util::paths;
use util::ToUrl;
use util::errors::{CargoError, CargoResult, CargoResultExt};
use util::important_paths::find_root_manifest_for_wd;

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
    pub target: Option<&'cfg str>,
    pub dry_run: bool,
}

pub fn publish(ws: &Workspace, opts: &PublishOpts) -> CargoResult<()> {
    let pkg = ws.current()?;

    if !pkg.publish() {
        bail!("some crates cannot be published.\n\
               `{}` is marked as unpublishable", pkg.name());
    }
    if !pkg.manifest().patch().is_empty() {
        bail!("published crates cannot contain [patch] sections");
    }

    let (mut registry, reg_id) = registry(opts.config,
                                          opts.token.clone(),
                                          opts.index.clone())?;
    verify_dependencies(pkg, &reg_id)?;

    // Prepare a tarball, with a non-surpressable warning if metadata
    // is missing since this is being put online.
    let tarball = ops::package(ws, &ops::PackageOpts {
        config: opts.config,
        verify: opts.verify,
        list: false,
        check_metadata: true,
        allow_dirty: opts.allow_dirty,
        target: opts.target,
        jobs: opts.jobs,
    })?.unwrap();

    // Upload said tarball to the specified destination
    opts.config.shell().status("Uploading", pkg.package_id().to_string())?;
    transmit(opts.config, pkg, tarball.file(), &mut registry, opts.dry_run)?;

    Ok(())
}

fn verify_dependencies(pkg: &Package, registry_src: &SourceId)
                       -> CargoResult<()> {
    for dep in pkg.dependencies().iter() {
        if dep.source_id().is_path() {
            if !dep.specified_req() {
                bail!("all path dependencies must have a version specified \
                       when publishing.\ndependency `{}` does not specify \
                       a version", dep.name())
            }
        } else if dep.source_id() != registry_src {
            if dep.source_id().is_registry() {
                bail!("crates cannot be published to crates.io with dependencies sourced from other\n\
                       registries either publish `{}` on crates.io or pull it into this repository\n\
                       and specify it with a path and version\n\
                       (crate `{}` is pulled from {}", dep.name(), dep.name(), dep.source_id());
            } else {
                bail!("crates cannot be published to crates.io with dependencies sourced from \
                       a repository\neither publish `{}` as its own crate on crates.io and \
                       specify a crates.io version as a dependency or pull it into this \
                       repository and specify it with a path and version\n(crate `{}` has \
                       repository path `{}`)", dep.name(), dep.name(),  dep.source_id());
            }
        }
    }
    Ok(())
}

fn transmit(config: &Config,
            pkg: &Package,
            tarball: &File,
            registry: &mut Registry,
            dry_run: bool) -> CargoResult<()> {
    let deps = pkg.dependencies().iter().map(|dep| {
        NewCrateDependency {
            optional: dep.is_optional(),
            default_features: dep.uses_default_features(),
            name: dep.name().to_string(),
            features: dep.features().to_vec(),
            version_req: dep.version_req().to_string(),
            target: dep.platform().map(|s| s.to_string()),
            kind: match dep.kind() {
                Kind::Normal => "normal",
                Kind::Build => "build",
                Kind::Development => "dev",
            }.to_string(),
        }
    }).collect::<Vec<NewCrateDependency>>();
    let manifest = pkg.manifest();
    let ManifestMetadata {
        ref authors, ref description, ref homepage, ref documentation,
        ref keywords, ref readme, ref repository, ref license, ref license_file,
        ref categories, ref badges,
    } = *manifest.metadata();
    let readme_content = match *readme {
        Some(ref readme) => Some(paths::read(&pkg.root().join(readme))?),
        None => None,
    };
    if let Some(ref file) = *license_file {
        if fs::metadata(&pkg.root().join(file)).is_err() {
            bail!("the license file `{}` does not exist", file)
        }
    }

    // Do not upload if performing a dry run
    if dry_run {
        config.shell().warn("aborting upload due to dry run")?;
        return Ok(());
    }

    let publish = registry.publish(&NewCrate {
        name: pkg.name().to_string(),
        vers: pkg.version().to_string(),
        deps: deps,
        features: pkg.summary().features().clone(),
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
    }, tarball);

    match publish {
        Ok(warnings) => {
            if !warnings.invalid_categories.is_empty() {
                let msg = format!("\
                    the following are not valid category slugs and were \
                    ignored: {}. Please see https://crates.io/category_slugs \
                    for the list of all category slugs. \
                    ", warnings.invalid_categories.join(", "));
                config.shell().warn(&msg)?;
            }

            if !warnings.invalid_badges.is_empty() {
                let msg = format!("\
                    the following are not valid badges and were ignored: {}. \
                    Either the badge type specified is unknown or a required \
                    attribute is missing. Please see \
                    http://doc.crates.io/manifest.html#package-metadata \
                    for valid badge types and their required attributes.",
                    warnings.invalid_badges.join(", "));
                config.shell().warn(&msg)?;
            }

            Ok(())
        },
        Err(e) => Err(e.into()),
    }
}

pub fn registry_configuration(config: &Config) -> CargoResult<RegistryConfig> {
    let index = config.get_string("registry.index")?.map(|p| p.val);
    let token = config.get_string("registry.token")?.map(|p| p.val);
    Ok(RegistryConfig { index: index, token: token })
}

pub fn registry(config: &Config,
                token: Option<String>,
                index: Option<String>) -> CargoResult<(Registry, SourceId)> {
    // Parse all configuration options
    let RegistryConfig {
        token: token_config,
        index: _index_config,
    } = registry_configuration(config)?;
    let token = token.or(token_config);
    let sid = match index {
        Some(index) => SourceId::for_registry(&index.to_url()?)?,
        None => SourceId::crates_io(config)?,
    };
    let api_host = {
        let mut src = RegistrySource::remote(&sid, config);
        src.update().chain_err(|| {
            format!("failed to update {}", sid)
        })?;
        (src.config()?).unwrap().api.unwrap()
    };
    let handle = http_handle(config)?;
    Ok((Registry::new_handle(api_host, token, handle), sid))
}

/// Create a new HTTP handle with appropriate global configuration for cargo.
pub fn http_handle(config: &Config) -> CargoResult<Easy> {
    if !config.network_allowed() {
        bail!("attempting to make an HTTP request, but --frozen was \
               specified")
    }

    // The timeout option for libcurl by default times out the entire transfer,
    // but we probably don't want this. Instead we only set timeouts for the
    // connect phase as well as a "low speed" timeout so if we don't receive
    // many bytes in a large-ish period of time then we time out.
    let mut handle = Easy::new();
    handle.connect_timeout(Duration::new(30, 0))?;
    handle.low_speed_limit(10 /* bytes per second */)?;
    handle.low_speed_time(Duration::new(30, 0))?;
    handle.useragent(&version().to_string())?;
    if let Some(proxy) = http_proxy(config)? {
        handle.proxy(&proxy)?;
    }
    if let Some(cainfo) = config.get_path("http.cainfo")? {
        handle.cainfo(&cainfo.val)?;
    }
    if let Some(check) = config.get_bool("http.check-revoke")? {
        handle.ssl_options(SslOpt::new().no_revoke(!check.val))?;
    }
    if let Some(timeout) = http_timeout(config)? {
        handle.connect_timeout(Duration::new(timeout as u64, 0))?;
        handle.low_speed_time(Duration::new(timeout as u64, 0))?;
    }
    Ok(handle)
}

/// Find an explicit HTTP proxy if one is available.
///
/// Favor cargo's `http.proxy`, then git's `http.proxy`. Proxies specified
/// via environment variables are picked up by libcurl.
fn http_proxy(config: &Config) -> CargoResult<Option<String>> {
    if let Some(s) = config.get_string("http.proxy")? {
        return Ok(Some(s.val))
    }
    if let Ok(cfg) = git2::Config::open_default() {
        if let Ok(s) = cfg.get_str("http.proxy") {
            return Ok(Some(s.to_string()))
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
pub fn http_proxy_exists(config: &Config) -> CargoResult<bool> {
    if http_proxy(config)?.is_some() {
        Ok(true)
    } else {
        Ok(["http_proxy", "HTTP_PROXY",
           "https_proxy", "HTTPS_PROXY"].iter().any(|v| env::var(v).is_ok()))
    }
}

pub fn http_timeout(config: &Config) -> CargoResult<Option<i64>> {
    if let Some(s) = config.get_i64("http.timeout")? {
        return Ok(Some(s.val))
    }
    Ok(env::var("HTTP_TIMEOUT").ok().and_then(|s| s.parse().ok()))
}

pub fn registry_login(config: &Config, token: String) -> CargoResult<()> {
    let RegistryConfig { token: old_token, .. } = registry_configuration(config)?;
    if let Some(old_token) = old_token {
        if old_token == token {
            return Ok(());
        }
    }

    config::save_credentials(config, token)
}

pub struct OwnersOptions {
    pub krate: Option<String>,
    pub token: Option<String>,
    pub index: Option<String>,
    pub to_add: Option<Vec<String>>,
    pub to_remove: Option<Vec<String>>,
    pub list: bool,
}

pub fn modify_owners(config: &Config, opts: &OwnersOptions) -> CargoResult<()> {
    let name = match opts.krate {
        Some(ref name) => name.clone(),
        None => {
            let manifest_path = find_root_manifest_for_wd(None, config.cwd())?;
            let pkg = Package::for_path(&manifest_path, config)?;
            pkg.name().to_string()
        }
    };

    let (mut registry, _) = registry(config, opts.token.clone(),
                                          opts.index.clone())?;

    if let Some(ref v) = opts.to_add {
        let v = v.iter().map(|s| &s[..]).collect::<Vec<_>>();
        let msg = registry.add_owners(&name, &v).map_err(|e| {
            CargoError::from(format!("failed to invite owners to crate {}: {}", name, e))
        })?;

        config.shell().status("Owner", msg)?;
    }

    if let Some(ref v) = opts.to_remove {
        let v = v.iter().map(|s| &s[..]).collect::<Vec<_>>();
        config.shell().status("Owner", format!("removing {:?} from crate {}",
                                                    v, name))?;
        registry.remove_owners(&name, &v).map_err(|e| {
            CargoError::from(format!("failed to remove owners from crate {}: {}", name, e))
        })?;
    }

    if opts.list {
        let owners = registry.list_owners(&name).map_err(|e| {
            CargoError::from(format!("failed to list owners of crate {}: {}", name, e))
        })?;
        for owner in owners.iter() {
            print!("{}", owner.login);
            match (owner.name.as_ref(), owner.email.as_ref()) {
                (Some(name), Some(email)) => println!(" ({} <{}>)", name, email),
                (Some(s), None) |
                (None, Some(s)) => println!(" ({})", s),
                (None, None) => println!(""),
            }
        }
    }

    Ok(())
}

pub fn yank(config: &Config,
            krate: Option<String>,
            version: Option<String>,
            token: Option<String>,
            index: Option<String>,
            undo: bool) -> CargoResult<()> {
    let name = match krate {
        Some(name) => name,
        None => {
            let manifest_path = find_root_manifest_for_wd(None, config.cwd())?;
            let pkg = Package::for_path(&manifest_path, config)?;
            pkg.name().to_string()
        }
    };
    let version = match version {
        Some(v) => v,
        None => bail!("a version must be specified to yank")
    };

    let (mut registry, _) = registry(config, token, index)?;

    if undo {
        config.shell().status("Unyank", format!("{}:{}", name, version))?;
        registry.unyank(&name, &version).map_err(|e| {
            CargoError::from(format!("failed to undo a yank: {}", e))
        })?;
    } else {
        config.shell().status("Yank", format!("{}:{}", name, version))?;
        registry.yank(&name, &version).map_err(|e| {
            CargoError::from(format!("failed to yank: {}", e))
        })?;
    }

    Ok(())
}

pub fn search(query: &str,
              config: &Config,
              index: Option<String>,
              limit: u8) -> CargoResult<()> {
    fn truncate_with_ellipsis(s: &str, max_length: usize) -> String {
        if s.len() < max_length {
            s.to_string()
        } else {
            format!("{}…", &s[..max_length - 1])
        }
    }

    let (mut registry, _) = registry(config, None, index)?;
    let (crates, total_crates) = registry.search(query, limit).map_err(|e| {
        CargoError::from(format!("failed to retrieve search results from the registry: {}", e))
    })?;

    let list_items = crates.iter()
        .map(|krate| (
            format!("{} = \"{}\"", krate.name, krate.max_version),
            krate.description.as_ref().map(|desc|
                truncate_with_ellipsis(&desc.replace("\n", " "), 128))
        ))
        .collect::<Vec<_>>();
    let description_margin = list_items.iter()
        .map(|&(ref left, _)| left.len() + 4)
        .max()
        .unwrap_or(0);

    for (name, description) in list_items.into_iter() {
        let line = match description {
            Some(desc) => {
                let space = repeat(' ').take(description_margin - name.len())
                                       .collect::<String>();
                name + &space + "# " + &desc
            }
            None => name
        };
        println!("{}", line);
    }

    let search_max_limit = 100;
    if total_crates > u32::from(limit) && limit < search_max_limit {
        println!("... and {} crates more (use --limit N to see more)",
                 total_crates - u32::from(limit));
    } else if total_crates > u32::from(limit) && limit >= search_max_limit {
        println!("... and {} crates more (go to http://crates.io/search?q={} to see more)",
                 total_crates - u32::from(limit),
                 percent_encode(query.as_bytes(), QUERY_ENCODE_SET));
    }

    Ok(())
}
