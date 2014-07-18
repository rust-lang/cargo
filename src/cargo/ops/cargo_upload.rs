use std::collections::HashMap;
use std::io::{File, TempDir, SeekSet};
use std::io;
use std::os;

use curl::http;
use tar::Archive;
use flate2::{GzEncoder, BestCompression};

use core::source::{Source, CENTRAL};
use core::{Package, MultiShell};
use sources::PathSource;
use util::config;
use util::{CargoResult, human, internal, ChainError, Require};
use util::config::{Config, ConfigValue, Table};

pub struct UploadConfig {
    pub host: Option<String>,
    pub token: Option<String>,
}

pub fn upload(manifest_path: &Path,
              shell: &mut MultiShell,
              token: Option<String>,
              host: Option<String>) -> CargoResult<()> {
    // TODO: technically this is a git source if there's a .git directory and we
    // can use `git ls-files` to discover files instead of just blindly walking.
    let mut src = PathSource::for_path(&manifest_path.dir_path());
    try!(src.update());
    let pkg = try!(src.get_root_package());

    // Parse all configuration options
    let UploadConfig { host: host_config, token: token_config } =
            try!(upload_configuration());
    let token = try!(token.or(token_config).require(|| {
        human("no upload token found, please run `cargo login`")
    }));
    let host = host.or(host_config).unwrap_or(CENTRAL.to_string());

    // Shove the local repo into a tarball
    try!(shell.status("Packaging", pkg.get_package_id().to_string()));
    let (_tmpdir, tarball) = try!(tar(&pkg, &src, shell).chain_error(|| {
        human("failed to prepare local package for uploading")
    }));

    // Upload said tarball to the specified destination
    try!(shell.status("Uploading", pkg.get_package_id().to_string()));
    try!(transmit(&pkg, tarball, token.as_slice(),
                  host.as_slice()).chain_error(|| {
        human(format!("failed to upload package to registry: {}", host))
    }));

    Ok(())
}

fn tar(pkg: &Package, src: &PathSource, shell: &mut MultiShell)
       -> CargoResult<(TempDir, File)> {
    let root = src.path();

    let tmpdir = try!(TempDir::new("cargo-upload").require(|| {
        internal("couldn't create temporary directory")
    }));
    let filename = format!("{}-{}.tar.gz", pkg.get_name(), pkg.get_version());
    let dst = tmpdir.path().join(filename.as_slice());
    let tmpfile = try!(File::open_mode(&dst, io::Open, io::ReadWrite));

    // Prepare the encoder and its header
    let mut encoder = GzEncoder::new(tmpfile, BestCompression);
    encoder.filename(filename.as_slice()).unwrap();

    // Put all package files into a compressed archive
    let ar = Archive::new(encoder);
    for file in src.walk() {
        let file = try!(file.chain_error(|| {
            internal(format!("could not walk the source tree for `{}`",
                             pkg.get_name()))
        }));
        let relative = file.path_relative_from(root).unwrap();
        let relative = try!(relative.as_str().require(|| {
            human(format!("non-utf8 path in source directory: {}",
                          relative.display()))
        }));
        let mut file = try!(File::open(&file));
        try!(shell.verbose(|shell| {
            shell.status("Archiving", relative.as_slice())
        }));
        let path = format!("{}-{}/{}", pkg.get_name(),
                           pkg.get_version(), relative);
        try!(ar.append(path.as_slice(), &mut file).chain_error(|| {
            internal(format!("could not archive source file `{}`", relative))
        }));
    }

    Ok((tmpdir, try!(ar.unwrap().finish())))
}

fn transmit(pkg: &Package, mut tarball: File,
            token: &str, host: &str) -> CargoResult<()> {
    try!(tarball.seek(0, SeekSet));
    let stat = try!(tarball.stat());

    let url = format!("{}/packages/new", host.trim_right_chars('/'));
    let mut handle = http::handle();
    let mut req = handle.post(url.as_slice(),
                              &mut tarball as &mut Reader)
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
        let header = format!("{}|{}|{}", dep.get_name(), dep.get_version_req(),
                             dep.get_namespace());

        if i > 0 { dep_header.push_str(", "); }
        dep_header.push_str(header.as_slice());
    }
    req = req.header("X-Cargo-Pkg-Dep", dep_header.as_slice());

    let response = try!(req.exec());

    if response.get_code() != 200 {
        Err(internal(format!("failed to get a 200 response: {}", response)))
    } else {
        Ok(())
    }
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
            })).to_string())
        }
    };
    let token = match registry.find_equiv(&"token") {
        None => None,
        Some(token) => {
            Some(try!(token.string().chain_error(|| {
                internal("invalid configuration for key `token`")
            })).to_string())
        }
    };
    Ok(UploadConfig { host: host, token: token })
}

pub fn upload_login(shell: &mut MultiShell, token: String) -> CargoResult<()> {
    let config = try!(Config::new(shell, false, None, None));
    let UploadConfig { host, token: _ } = try!(upload_configuration());
    let mut map = HashMap::new();
    match host {
        Some(host) => {
            map.insert("host".to_string(), ConfigValue::new_string(host));
        }
        None => {}
    }
    map.insert("token".to_string(), ConfigValue::new_string(token));

    config::set_config(&config, config::Global, "registry", config::Table(map))
}
