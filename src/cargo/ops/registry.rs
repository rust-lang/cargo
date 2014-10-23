use std::collections::HashMap;
use std::io::File;
use std::os;

use curl::http;
use git2;
use registry::{Registry, NewCrate, NewCrateDependency};

use core::source::Source;
use core::{Package, MultiShell, SourceId, RegistryKind};
use core::manifest::ManifestMetadata;
use ops;
use sources::{PathSource, RegistrySource};
use util::config;
use util::{CargoResult, human, internal, ChainError, Require, ToUrl};
use util::config::{Config, Table};

pub struct RegistryConfig {
    pub index: Option<String>,
    pub token: Option<String>,
}

pub fn publish(manifest_path: &Path,
               shell: &mut MultiShell,
               token: Option<String>,
               index: Option<String>,
               verify: bool) -> CargoResult<()> {
    let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(src.update());
    let pkg = try!(src.get_root_package());

    let (mut registry, reg_id) = try!(registry(shell, token, index));
    try!(verify_dependencies(&pkg, &reg_id));

    // Prepare a tarball
    let tarball = try!(ops::package(manifest_path, shell, verify));

    // Upload said tarball to the specified destination
    try!(shell.status("Uploading", pkg.get_package_id().to_string()));
    try!(transmit(&pkg, &tarball, &mut registry));

    Ok(())
}

fn verify_dependencies(pkg: &Package, registry_src: &SourceId)
                       -> CargoResult<()> {
    for dep in pkg.get_dependencies().iter() {
        if dep.get_source_id().is_path() {
            if dep.get_specified_req().is_none() {
                return Err(human(format!("all path dependencies must have \
                                          a version specified when being \
                                          uploaded to the registry.\n\
                                          dependency `{}` does not specify \
                                          a version", dep.get_name())))
            }
        } else if dep.get_source_id() != registry_src {
            return Err(human(format!("all dependencies must come from the \
                                      same registry.\ndependency `{}` comes \
                                      from {} instead", dep.get_name(),
                                     dep.get_source_id())))
        }
    }
    Ok(())
}

fn transmit(pkg: &Package, tarball: &Path, registry: &mut Registry)
            -> CargoResult<()> {
    let deps = pkg.get_dependencies().iter().map(|dep| {
        NewCrateDependency {
            optional: dep.is_optional(),
            default_features: dep.uses_default_features(),
            name: dep.get_name().to_string(),
            features: dep.get_features().to_vec(),
            version_req: dep.get_version_req().to_string(),
            target: None, // FIXME: fill this out
        }
    }).collect::<Vec<NewCrateDependency>>();
    let manifest = pkg.get_manifest();
    let ManifestMetadata {
        ref authors, ref description, ref homepage, ref documentation,
        ref keywords, ref readme, ref repository, ref license,
    } = *manifest.get_metadata();
    let readme = match *readme {
        Some(ref readme) => {
            let path = pkg.get_root().join(readme.as_slice());
            Some(try!(File::open(&path).read_to_string().chain_error(|| {
                human("failed to read the specified README")
            })))
        }
        None => None,
    };
    registry.publish(&NewCrate {
        name: pkg.get_name().to_string(),
        vers: pkg.get_version().to_string(),
        deps: deps,
        features: pkg.get_summary().get_features().clone(),
        authors: authors.clone(),
        description: description.clone(),
        homepage: homepage.clone(),
        documentation: documentation.clone(),
        keywords: keywords.clone(),
        readme: readme,
        repository: repository.clone(),
        license: license.clone(),
    }, tarball).map_err(|e| {
        human(e.to_string())
    })
}

pub fn registry_configuration() -> CargoResult<RegistryConfig> {
    let configs = try!(config::all_configs(os::getcwd()));
    let registry = match configs.find_equiv(&"registry") {
        None => return Ok(RegistryConfig { index: None, token: None }),
        Some(registry) => try!(registry.table().chain_error(|| {
            internal("invalid configuration for the key `registry`")
        })),
    };
    let index = match registry.find_equiv(&"index") {
        None => None,
        Some(index) => {
            Some(try!(index.string().chain_error(|| {
                internal("invalid configuration for key `index`")
            })).ref0().to_string())
        }
    };
    let token = match registry.find_equiv(&"token") {
        None => None,
        Some(token) => {
            Some(try!(token.string().chain_error(|| {
                internal("invalid configuration for key `token`")
            })).ref0().to_string())
        }
    };
    Ok(RegistryConfig { index: index, token: token })
}

pub fn registry(shell: &mut MultiShell,
                token: Option<String>,
                index: Option<String>) -> CargoResult<(Registry, SourceId)> {
    // Parse all configuration options
    let RegistryConfig {
        token: token_config,
        index: index_config,
    } = try!(registry_configuration());
    let token = try!(token.or(token_config).require(|| {
        human("no upload token found, please run `cargo login`")
    }));
    let index = index.or(index_config).unwrap_or(RegistrySource::default_url());
    let index = try!(index.as_slice().to_url().map_err(human));
    let sid = SourceId::for_registry(&index);
    let api_host = {
        let mut config = try!(Config::new(shell, None, None));
        let mut src = RegistrySource::new(&sid, &mut config);
        try!(src.update().chain_error(|| {
            human(format!("Failed to update registry {}", index))
        }));
        (try!(src.config())).api
    };
    let handle = try!(http_handle());
    Ok((Registry::new_handle(api_host, token, handle), sid))
}

/// Create a new HTTP handle with appropriate global configuration for cargo.
pub fn http_handle() -> CargoResult<http::Handle> {
    Ok(match try!(http_proxy()) {
        Some(proxy) => http::handle().proxy(proxy),
        None => http::handle(),
    })
}

/// Find a globally configured HTTP proxy if one is available.
///
/// Favor cargo's `http.proxy`, then git's `http.proxy`, then finally a
/// HTTP_PROXY env var.
pub fn http_proxy() -> CargoResult<Option<String>> {
    let configs = try!(config::all_configs(os::getcwd()));
    match configs.find_equiv(&"http") {
        Some(http) => {
            let http = try!(http.table().chain_error(|| {
                internal("invalid configuration for the key `http`")
            }));
            match http.find_equiv(&"proxy") {
                Some(proxy) => {
                    return Ok(Some(try!(proxy.string().chain_error(|| {
                        internal("invalid configuration for key `http.proxy`")
                    })).ref0().to_string()))
                }
                None => {},
            }
        }
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
    Ok(os::getenv("HTTP_PROXY"))
}

pub fn registry_login(shell: &mut MultiShell, token: String) -> CargoResult<()> {
    let config = try!(Config::new(shell, None, None));
    let RegistryConfig { index, token: _ } = try!(registry_configuration());
    let mut map = HashMap::new();
    let p = os::getcwd();
    match index {
        Some(index) => {
            map.insert("index".to_string(), config::String(index, p.clone()));
        }
        None => {}
    }
    map.insert("token".to_string(), config::String(token, p));

    config::set_config(&config, config::Global, "registry", config::Table(map))
}

pub fn modify_owners(manifest_path: &Path,
                     shell: &mut MultiShell,
                     krate: Option<String>,
                     token: Option<String>,
                     index: Option<String>,
                     to_add: Option<Vec<String>>,
                     to_remove: Option<Vec<String>>) -> CargoResult<()> {
    let name = match krate {
        Some(name) => name,
        None => {
            let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
            try!(src.update());
            let pkg = try!(src.get_root_package());
            pkg.get_name().to_string()
        }
    };

    let (mut registry, _) = try!(registry(shell, token, index));

    match to_add {
        Some(v) => {
            let v = v.iter().map(|s| s.as_slice()).collect::<Vec<_>>();
            try!(shell.status("Owner", format!("adding `{:#}` to `{}`", v, name)));
            try!(registry.add_owners(name.as_slice(), v.as_slice()).map_err(|e| {
                human(format!("failed to add owners: {}", e))
            }));
        }
        None => {}
    }

    match to_remove {
        Some(v) => {
            let v = v.iter().map(|s| s.as_slice()).collect::<Vec<_>>();
            try!(shell.status("Owner", format!("removing `{:#}` from `{}`",
                                               v, name)));
            try!(registry.remove_owners(name.as_slice(), v.as_slice()).map_err(|e| {
                human(format!("failed to add owners: {}", e))
            }));
        }
        None => {}
    }

    Ok(())
}

pub fn yank(manifest_path: &Path,
            shell: &mut MultiShell,
            krate: Option<String>,
            version: Option<String>,
            token: Option<String>,
            index: Option<String>,
            undo: bool) -> CargoResult<()> {
    let name = match krate {
        Some(name) => name,
        None => {
            let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
            try!(src.update());
            let pkg = try!(src.get_root_package());
            pkg.get_name().to_string()
        }
    };
    let version = match version {
        Some(v) => v,
        None => return Err(human("a version must be specified to yank"))
    };

    let (mut registry, _) = try!(registry(shell, token, index));

    if undo {
        try!(shell.status("Unyank", format!("{}:{}", name, version)));
        try!(registry.unyank(name.as_slice(), version.as_slice()).map_err(|e| {
            human(format!("failed to undo a yank: {}", e))
        }));
    } else {
        try!(shell.status("Yank", format!("{}:{}", name, version)));
        try!(registry.yank(name.as_slice(), version.as_slice()).map_err(|e| {
            human(format!("failed to yank: {}", e))
        }));
    }

    Ok(())
}
