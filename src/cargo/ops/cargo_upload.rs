use std::collections::HashMap;
use std::io::File;
use std::os;
use std::str;
use serialize::json;

use curl::http;

use core::source::Source;
use core::{Package, MultiShell, SourceId};
use ops;
use sources::{PathSource, RegistrySource};
use util::config;
use util::{CargoResult, human, internal, ChainError, Require, ToUrl};
use util::config::{Config, Table};

pub struct UploadConfig {
    pub host: Option<String>,
    pub token: Option<String>,
}

pub fn upload(manifest_path: &Path,
              shell: &mut MultiShell,
              token: Option<String>,
              host: Option<String>) -> CargoResult<()> {
    let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(src.update());
    let pkg = try!(src.get_root_package());

    // Parse all configuration options
    let UploadConfig { token: token_config, .. } = try!(upload_configuration());
    let token = try!(token.or(token_config).require(|| {
        human("no upload token found, please run `cargo login`")
    }));
    let host = host.unwrap_or(try!(RegistrySource::url()).to_string());

    // First, prepare a tarball
    let tarball = try!(ops::package(manifest_path, shell));
    let tarball = try!(File::open(&tarball));

    // Upload said tarball to the specified destination
    try!(shell.status("Uploading", pkg.get_package_id().to_string()));
    try!(transmit(&pkg, tarball, token.as_slice(),
                  host.as_slice()).chain_error(|| {
        human(format!("failed to upload package to registry: {}", host))
    }));

    Ok(())
}

fn transmit(pkg: &Package, mut tarball: File,
            token: &str, host: &str) -> CargoResult<()> {
    let stat = try!(tarball.stat());
    let url = try!(host.to_url().map_err(human));
    let registry_src = SourceId::for_registry(&url);

    let url = format!("{}/packages/new", host.trim_right_chars('/'));
    let mut handle = http::handle();
    let mut req = handle.post(url.as_slice(), &mut tarball)
                        .content_length(stat.size as uint)
                        .content_type("application/x-tar")
                        .header("Content-Encoding", "x-gzip")
                        .header("X-Cargo-Auth", token)
                        .header("X-Cargo-Pkg-Name", pkg.get_name())
                        .header("X-Cargo-Pkg-Version",
                                pkg.get_version().to_string().as_slice());

    let mut dep_header = String::new();
    for (i, dep) in pkg.get_dependencies().iter().enumerate() {
        if !dep.is_transitive() { continue }
        if dep.get_source_id() != &registry_src {
            return Err(human(format!("All dependencies must come from the \
                                      same registry.\nDependency `{}` comes \
                                      from {} instead", dep.get_name(),
                                     dep.get_source_id())))
        }
        let header = format!("{}|{}", dep.get_name(), dep.get_version_req());
        if i > 0 { dep_header.push_str(";"); }
        dep_header.push_str(header.as_slice());
    }
    req = req.header("X-Cargo-Pkg-Dep", dep_header.as_slice());

    let response = try!(req.exec());

    if response.get_code() != 200 {
        return Err(internal(format!("failed to get a 200 response: {}",
                                    response)))
    }

    let body = try!(str::from_utf8(response.get_body()).require(|| {
        internal("failed to get a utf-8 response")
    }));

    #[deriving(Decodable)]
    struct Response { ok: bool }
    #[deriving(Decodable)]
    struct BadResponse { error: String }
    let json = try!(json::decode::<Response>(body));
    if json.ok { return Ok(()) }

    let json = try!(json::decode::<BadResponse>(body));
    Err(human(format!("failed to upload `{}`: {}", pkg, json.error)))
}

pub fn upload_configuration() -> CargoResult<UploadConfig> {
    let configs = try!(config::all_configs(os::getcwd()));
    let registry = match configs.find_equiv(&"registry") {
        None => return Ok(UploadConfig { host: None, token: None }),
        Some(registry) => try!(registry.table().chain_error(|| {
            internal("invalid configuration for the key `registry`")
        })),
    };
    let host = match registry.find_equiv(&"host") {
        None => None,
        Some(host) => {
            Some(try!(host.string().chain_error(|| {
                internal("invalid configuration for key `host`")
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
    Ok(UploadConfig { host: host, token: token })
}

pub fn upload_login(shell: &mut MultiShell, token: String) -> CargoResult<()> {
    let config = try!(Config::new(shell, None, None));
    let UploadConfig { host, token: _ } = try!(upload_configuration());
    let mut map = HashMap::new();
    let p = os::getcwd();
    match host {
        Some(host) => {
            map.insert("host".to_string(), config::String(host, p.clone()));
        }
        None => {}
    }
    map.insert("token".to_string(), config::String(token, p));

    config::set_config(&config, config::Global, "registry", config::Table(map))
}
