use std::collections::HashMap;
use std::io::File;
use std::io::fs::PathExtensions;
use std::os;
use term::color::BLACK;

use curl::http;
use git2;
use registry::{Registry, NewCrate, NewCrateDependency};

use core::source::Source;
use core::{Package, MultiShell, SourceId};
use core::dependency::Kind;
use core::manifest::ManifestMetadata;
use ops;
use sources::{PathSource, RegistrySource};
use util::config;
use util::{CargoResult, human, internal, ChainError, ToUrl};
use util::config::{Config, ConfigValue, Location};
use util::important_paths::find_root_manifest_for_cwd;

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

    // Prepare a tarball, with a non-surpressable warning if metadata
    // is missing since this is being put online.
    let tarball = try!(ops::package(manifest_path, shell, verify, false, true)).unwrap();

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
                                          a version specified when \
                                          publishing.\n\
                                          dependency `{}` does not specify \
                                          a version", dep.get_name())))
            }
        } else if dep.get_source_id() != registry_src {
            return Err(human(format!("all dependencies must come from the \
                                      same source.\ndependency `{}` comes \
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
            target: dep.get_only_for_platform().map(|s| s.to_string()),
            kind: match dep.get_kind() {
                Kind::Normal => "normal",
                Kind::Build => "build",
                Kind::Development => "dev",
            }.to_string(),
        }
    }).collect::<Vec<NewCrateDependency>>();
    let manifest = pkg.get_manifest();
    let ManifestMetadata {
        ref authors, ref description, ref homepage, ref documentation,
        ref keywords, ref readme, ref repository, ref license, ref license_file,
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
    match *license_file {
        Some(ref file) => {
            if !pkg.get_root().join(file).exists() {
                return Err(human(format!("the license file `{}` does not exist",
                                         file)))
            }
        }
        None => {}
    }
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
        license_file: license_file.clone(),
    }, tarball).map_err(|e| {
        human(e.to_string())
    })
}

pub fn registry_configuration() -> CargoResult<RegistryConfig> {
    let configs = try!(config::all_configs(try!(os::getcwd())));
    let registry = match configs.get("registry") {
        None => return Ok(RegistryConfig { index: None, token: None }),
        Some(registry) => try!(registry.table().chain_error(|| {
            internal("invalid configuration for the key `registry`")
        })),
    };
    let index = match registry.get("index") {
        None => None,
        Some(index) => {
            Some(try!(index.string().chain_error(|| {
                internal("invalid configuration for key `index`")
            })).0.to_string())
        }
    };
    let token = match registry.get("token") {
        None => None,
        Some(token) => {
            Some(try!(token.string().chain_error(|| {
                internal("invalid configuration for key `token`")
            })).0.to_string())
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
    let token = token.or(token_config);
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
    let configs = try!(config::all_configs(try!(os::getcwd())));
    match configs.get("http") {
        Some(http) => {
            let http = try!(http.table().chain_error(|| {
                internal("invalid configuration for the key `http`")
            }));
            match http.get("proxy") {
                Some(proxy) => {
                    return Ok(Some(try!(proxy.string().chain_error(|| {
                        internal("invalid configuration for key `http.proxy`")
                    })).0.to_string()))
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
    let p = try!(os::getcwd());
    match index {
        Some(index) => {
            map.insert("index".to_string(), ConfigValue::String(index, p.clone()));
        }
        None => {}
    }
    map.insert("token".to_string(), ConfigValue::String(token, p));

    config::set_config(&config, Location::Global, "registry",
                       ConfigValue::Table(map))
}

pub struct OwnersOptions {
    pub krate: Option<String>,
    pub token: Option<String>,
    pub index: Option<String>,
    pub to_add: Option<Vec<String>>,
    pub to_remove: Option<Vec<String>>,
    pub list: bool,
}

pub fn modify_owners(shell: &mut MultiShell,
                     opts: &OwnersOptions) -> CargoResult<()> {
    let name = match opts.krate {
        Some(ref name) => name.clone(),
        None => {
            let manifest_path = try!(find_root_manifest_for_cwd(None));
            let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
            try!(src.update());
            let pkg = try!(src.get_root_package());
            pkg.get_name().to_string()
        }
    };

    let (mut registry, _) = try!(registry(shell, opts.token.clone(),
                                          opts.index.clone()));

    match opts.to_add {
        Some(ref v) => {
            let v = v.iter().map(|s| s.as_slice()).collect::<Vec<_>>();
            try!(shell.status("Owner", format!("adding `{:#}` to `{}`", v, name)));
            try!(registry.add_owners(name.as_slice(), v.as_slice()).map_err(|e| {
                human(format!("failed to add owners: {}", e))
            }));
        }
        None => {}
    }

    match opts.to_remove {
        Some(ref v) => {
            let v = v.iter().map(|s| s.as_slice()).collect::<Vec<_>>();
            try!(shell.status("Owner", format!("removing `{:#}` from `{}`",
                                               v, name)));
            try!(registry.remove_owners(name.as_slice(), v.as_slice()).map_err(|e| {
                human(format!("failed to add owners: {}", e))
            }));
        }
        None => {}
    }

    if opts.list {
        let owners = try!(registry.list_owners(name.as_slice()).map_err(|e| {
            human(format!("failed to list owners: {}", e))
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

pub fn search(query: &str, shell: &mut MultiShell, index: Option<String>) -> CargoResult<()> {
    fn truncate_with_ellipsis(s: &str, max_length: uint) -> String {
        if s.len() < max_length {
            s.to_string()
        } else {
            format!("{}…", s[..max_length - 1])
        }
    }

    let (mut registry, _) = try!(registry(shell, None, index));

    let crates = try!(registry.search(query).map_err(|e| {
        human(format!("failed to retrieve search results from the registry: {}", e))
    }));

    let list_items = crates.iter()
        .map(|krate| (
            format!("{} ({})", krate.name, krate.max_version),
            krate.description.as_ref().map(|desc|
                truncate_with_ellipsis(desc.replace("\n", " ").as_slice(), 128))
        ))
        .collect::<Vec<_>>();
    let description_margin = list_items.iter()
        .map(|&(ref left, _)| left.len() + 4)
        .max()
        .unwrap_or(0);

    for (name, description) in list_items.into_iter() {
        let line = match description {
            Some(desc) => {
                let space = String::from_char(
                    description_margin - name.len(),
                    ' ');
                name.to_string() + space.as_slice() + desc.as_slice()
            }
            None => name
        };
        try!(shell.say(line, BLACK));
    }

    Ok(())
}
