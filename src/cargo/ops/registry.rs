use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::iter::repeat;
use std::path::{Path, PathBuf};

use curl::http;
use git2;
use registry::{Registry, NewCrate, NewCrateDependency};
use term::color::BLACK;

use core::source::Source;
use core::{Package, SourceId};
use core::dependency::Kind;
use core::manifest::ManifestMetadata;
use ops;
use sources::{RegistrySource};
use util::config;
use util::paths;
use util::{CargoResult, human, ChainError, ToUrl};
use util::config::{Config, ConfigValue, Location};
use util::important_paths::find_root_manifest_for_wd;

pub struct RegistryConfig {
    pub index: Option<String>,
    pub token: Option<String>,
}

pub fn publish(manifest_path: &Path,
               config: &Config,
               token: Option<String>,
               index: Option<String>,
               verify: bool) -> CargoResult<()> {
    let pkg = try!(Package::for_path(&manifest_path, config));

    if !pkg.publish() {
        bail!("some crates cannot be published.\n\
               `{}` is marked as unpublishable", pkg.name());
    }

    let (mut registry, reg_id) = try!(registry(config, token, index));
    try!(verify_dependencies(&pkg, &reg_id));

    // Prepare a tarball, with a non-surpressable warning if metadata
    // is missing since this is being put online.
    let tarball = try!(ops::package(manifest_path, config, verify,
                                    false, true)).unwrap();

    // Upload said tarball to the specified destination
    try!(config.shell().status("Uploading", pkg.package_id().to_string()));
    try!(transmit(&pkg, &tarball, &mut registry));

    Ok(())
}

fn verify_dependencies(pkg: &Package, registry_src: &SourceId)
                       -> CargoResult<()> {
    for dep in pkg.dependencies().iter() {
        if dep.source_id().is_path() {
            if dep.specified_req().is_none() {
                bail!("all path dependencies must have a version specified \
                       when publishing.\ndependency `{}` does not specify \
                       a version", dep.name())
            }
        } else if dep.source_id() != registry_src {
            bail!("all dependencies must come from the same source.\n\
                   dependency `{}` comes from {} instead",
                  dep.name(), dep.source_id())
        }
    }
    Ok(())
}

fn transmit(pkg: &Package, tarball: &Path, registry: &mut Registry)
            -> CargoResult<()> {
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
    } = *manifest.metadata();
    let readme = match *readme {
        Some(ref readme) => Some(try!(paths::read(&pkg.root().join(readme)))),
        None => None,
    };
    match *license_file {
        Some(ref file) => {
            if fs::metadata(&pkg.root().join(file)).is_err() {
                bail!("the license file `{}` does not exist", file)
            }
        }
        None => {}
    }
    registry.publish(&NewCrate {
        name: pkg.name().to_string(),
        vers: pkg.version().to_string(),
        deps: deps,
        features: pkg.summary().features().clone(),
        authors: authors.clone(),
        description: description.clone(),
        homepage: homepage.clone(),
        documentation: documentation.clone(),
        keywords: keywords.clone(),
        readme: readme,
        repository: repository.clone(),
        license: license.clone(),
        license_file: license_file.clone(),
    }, tarball).map_err(|e| {
        human(e.to_string())
    })
}

pub fn registry_configuration(config: &Config) -> CargoResult<RegistryConfig> {
    let index = try!(config.get_string("registry.index")).map(|p| p.val);
    let token = try!(config.get_string("registry.token")).map(|p| p.val);
    Ok(RegistryConfig { index: index, token: token })
}

pub fn registry(config: &Config,
                token: Option<String>,
                index: Option<String>) -> CargoResult<(Registry, SourceId)> {
    // Parse all configuration options
    let RegistryConfig {
        token: token_config,
        index: index_config,
    } = try!(registry_configuration(config));
    let token = token.or(token_config);
    let index = index.or(index_config).unwrap_or(RegistrySource::default_url());
    let index = try!(index.to_url().map_err(human));
    let sid = SourceId::for_registry(&index);
    let api_host = {
        let mut src = RegistrySource::new(&sid, config);
        try!(src.update().chain_error(|| {
            human(format!("failed to update registry {}", index))
        }));
        (try!(src.config())).api
    };
    let handle = try!(http_handle(config));
    Ok((Registry::new_handle(api_host, token, handle), sid))
}

/// Create a new HTTP handle with appropriate global configuration for cargo.
pub fn http_handle(config: &Config) -> CargoResult<http::Handle> {
    // The timeout option for libcurl by default times out the entire transfer,
    // but we probably don't want this. Instead we only set timeouts for the
    // connect phase as well as a "low speed" timeout so if we don't receive
    // many bytes in a large-ish period of time then we time out.
    let handle = http::handle().timeout(0)
                               .connect_timeout(30_000 /* milliseconds */)
                               .low_speed_limit(10 /* bytes per second */)
                               .low_speed_timeout(30 /* seconds */);
    let handle = match try!(http_proxy(config)) {
        Some(proxy) => handle.proxy(proxy),
        None => handle,
    };
    let handle = match try!(http_timeout(config)) {
        Some(timeout) => handle.connect_timeout(timeout as usize)
                               .low_speed_timeout((timeout as usize) / 1000),
        None => handle,
    };
    Ok(handle)
}

/// Find an explicit HTTP proxy if one is available.
///
/// Favor cargo's `http.proxy`, then git's `http.proxy`. Proxies specified
/// via environment variables are picked up by libcurl.
fn http_proxy(config: &Config) -> CargoResult<Option<String>> {
    match try!(config.get_string("http.proxy")) {
        Some(s) => return Ok(Some(s.val)),
        None => {}
    }
    match git2::Config::open_default() {
        Ok(cfg) => {
            match cfg.get_str("http.proxy") {
                Ok(s) => return Ok(Some(s.to_string())),
                Err(..) => {}
            }
        }
        Err(..) => {}
    }
    Ok(None)
}

/// Determine if an http proxy exists.
///
/// Checks the following for existence, in order:
///
/// * cargo's `http.proxy`
/// * git's `http.proxy`
/// * http_proxy env var
/// * HTTP_PROXY env var
/// * https_proxy env var
/// * HTTPS_PROXY env var
pub fn http_proxy_exists(config: &Config) -> CargoResult<bool> {
    if try!(http_proxy(config)).is_some() {
        Ok(true)
    } else {
        Ok(["http_proxy", "HTTP_PROXY",
           "https_proxy", "HTTPS_PROXY"].iter().any(|v| env::var(v).is_ok()))
    }
}

pub fn http_timeout(config: &Config) -> CargoResult<Option<i64>> {
    match try!(config.get_i64("http.timeout")) {
        Some(s) => return Ok(Some(s.val)),
        None => {}
    }
    Ok(env::var("HTTP_TIMEOUT").ok().and_then(|s| s.parse().ok()))
}

pub fn registry_login(config: &Config, token: String) -> CargoResult<()> {
    let RegistryConfig { index, token: _ } = try!(registry_configuration(config));
    let mut map = HashMap::new();
    let p = config.cwd().to_path_buf();
    match index {
        Some(index) => {
            map.insert("index".to_string(), ConfigValue::String(index, p.clone()));
        }
        None => {}
    }
    map.insert("token".to_string(), ConfigValue::String(token, p));

    config::set_config(config, Location::Global, "registry",
                       ConfigValue::Table(map, PathBuf::from(".")))
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
            let manifest_path = try!(find_root_manifest_for_wd(None, config.cwd()));
            let pkg = try!(Package::for_path(&manifest_path, config));
            pkg.name().to_string()
        }
    };

    let (mut registry, _) = try!(registry(config, opts.token.clone(),
                                          opts.index.clone()));

    match opts.to_add {
        Some(ref v) => {
            let v = v.iter().map(|s| &s[..]).collect::<Vec<_>>();
            try!(config.shell().status("Owner", format!("adding {:?} to crate {}",
                                                        v, name)));
            try!(registry.add_owners(&name, &v).map_err(|e| {
                human(format!("failed to add owners to crate {}: {}", name, e))
            }));
        }
        None => {}
    }

    match opts.to_remove {
        Some(ref v) => {
            let v = v.iter().map(|s| &s[..]).collect::<Vec<_>>();
            try!(config.shell().status("Owner", format!("removing {:?} from crate {}",
                                                        v, name)));
            try!(registry.remove_owners(&name, &v).map_err(|e| {
                human(format!("failed to remove owners from crate {}: {}", name, e))
            }));
        }
        None => {}
    }

    if opts.list {
        let owners = try!(registry.list_owners(&name).map_err(|e| {
            human(format!("failed to list owners of crate {}: {}", name, e))
        }));
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
            let manifest_path = try!(find_root_manifest_for_wd(None, config.cwd()));
            let pkg = try!(Package::for_path(&manifest_path, config));
            pkg.name().to_string()
        }
    };
    let version = match version {
        Some(v) => v,
        None => bail!("a version must be specified to yank")
    };

    let (mut registry, _) = try!(registry(config, token, index));

    if undo {
        try!(config.shell().status("Unyank", format!("{}:{}", name, version)));
        try!(registry.unyank(&name, &version).map_err(|e| {
            human(format!("failed to undo a yank: {}", e))
        }));
    } else {
        try!(config.shell().status("Yank", format!("{}:{}", name, version)));
        try!(registry.yank(&name, &version).map_err(|e| {
            human(format!("failed to yank: {}", e))
        }));
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

    let (mut registry, _) = try!(registry(config, None, index));
    let crates = try!(registry.search(query, limit).map_err(|e| {
        human(format!("failed to retrieve search results from the registry: {}", e))
    }));

    let list_items = crates.iter()
        .map(|krate| (
            format!("{} ({})", krate.name, krate.max_version),
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
                name + &space + &desc
            }
            None => name
        };
        try!(config.shell().say(line, BLACK));
    }

    Ok(())
}
