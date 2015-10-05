use std::env;
use std::fmt;
use std::fs::{self, File};
use std::io::prelude::*;
use std::io;
use std::path::Path;
use std::str::{FromStr};

use rustc_serialize::{Decodable, Decoder};

use git2::Config as GitConfig;

use term::color::BLACK;
use time::{strftime, now};

use util::{GitRepo, HgRepo, CargoResult, CargoError, human, ChainError, internal};
use util::{Config, paths};

use toml;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VersionControl { Git, Hg, NoVcs }

pub struct NewOptions<'a> {
    pub version_control: Option<VersionControl>,
    pub bin: bool,
    pub path: &'a str,
    pub name: Option<&'a str>,
    pub license: Option<Vec<License>>,
}

impl Decodable for VersionControl {
    fn decode<D: Decoder>(d: &mut D) -> Result<VersionControl, D::Error> {
        Ok(match &try!(d.read_str())[..] {
            "git" => VersionControl::Git,
            "hg" => VersionControl::Hg,
            "none" => VersionControl::NoVcs,
            n => {
                let err = format!("could not decode '{}' as version control", n);
                return Err(d.error(&err));
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum License {
    MIT,
    BSD3,
    APACHE2,
    MPL2,
    GPL3,
}

impl License {
    fn write_file(&self, path: &Path, holders: &str) -> io::Result<()> {
        let mut license_text: Vec<u8> = Vec::new();
        let _ = self.license_text(&mut license_text, holders);
        file(path, &license_text)
    }

    fn license_text<F: io::Write>(&self, f: &mut F, holders: &str) -> io::Result<()> {
        let tm = now();
        let year = strftime("%Y", &tm).unwrap();
        match *self {
            // Not sure about this, would like to make this prettier
            License::MIT => write!(f, include_str!("../../../src/etc/licenses/MIT"),
                                   year = &year, holders = holders),
            License::BSD3 => write!(f, include_str!("../../../src/etc/licenses/BSD3"),
                                    year = &year, holders = holders),
            License::APACHE2 => write!(f, include_str!("../../../src/etc/licenses/APACHE2")),
            License::MPL2 => write!(f, include_str!("../../../src/etc/licenses/MPL2")),
            License::GPL3 => write!(f, include_str!("../../../src/etc/licenses/GPL3")),
        }
    }
}

impl FromStr for License {
    type Err = Box<CargoError>;
    fn from_str(s: &str) -> CargoResult<License> {
        Ok(match s.to_lowercase().as_ref() {
            "mit" => License::MIT,
            "bsd-3-clause" => License::BSD3,
            "apache-2.0" => License::APACHE2,
            "mpl-2.0" => License::MPL2,
            "gpl-3.0" => License::GPL3,
            _ => return Err(internal(format!("Unknown license {}", s))),
        })
    }
}

impl fmt::Display for License {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            &License::MIT => "MIT",
            &License::BSD3 => "BSD-3-Clause",
            &License::APACHE2 => "Apache-2.0",
            &License::MPL2 => "MPL-2.0",
            &License::GPL3 => "GPL-3.0",
        };
        write!(f, "{}", s)
    }
}

struct CargoNewConfig {
    name: Option<String>,
    email: Option<String>,
    version_control: Option<VersionControl>,
    license: Option<Vec<License>>,
}

pub fn new(opts: NewOptions, config: &Config) -> CargoResult<()> {
    let path = config.cwd().join(opts.path);
    if fs::metadata(&path).is_ok() {
        bail!("destination `{}` already exists", path.display())
    }
    let name = match opts.name {
        Some(name) => name,
        None => {
            let dir_name = try!(path.file_name().and_then(|s| s.to_str()).chain_error(|| {
                human(&format!("cannot create a project with a non-unicode name: {:?}",
                               path.file_name().unwrap()))
            }));
            if opts.bin {
                dir_name
            } else {
                let new_name = strip_rust_affixes(dir_name);
                if new_name != dir_name {
                    let message = format!(
                        "note: package will be named `{}`; use --name to override",
                        new_name);
                    try!(config.shell().say(&message, BLACK));
                }
                new_name
            }
        }
    };
    for c in name.chars() {
        if c.is_alphanumeric() { continue }
        if c == '_' || c == '-' { continue }
        bail!("Invalid character `{}` in crate name: `{}`", c, name)
    }
    mk(config, &path, name, &opts).chain_error(|| {
        human(format!("Failed to create project `{}` at `{}`",
                      name, path.display()))
    })
}

fn strip_rust_affixes(name: &str) -> &str {
    for &prefix in &["rust-", "rust_", "rs-", "rs_"] {
        if name.starts_with(prefix) {
            return &name[prefix.len()..];
        }
    }
    for &suffix in &["-rust", "_rust", "-rs", "_rs"] {
        if name.ends_with(suffix) {
            return &name[..name.len()-suffix.len()];
        }
    }
    name
}

fn existing_vcs_repo(path: &Path, cwd: &Path) -> bool {
    GitRepo::discover(path, cwd).is_ok() || HgRepo::discover(path, cwd).is_ok()
}

fn file(p: &Path, contents: &[u8]) -> io::Result<()> {
    try!(File::create(p)).write_all(contents)
}

#[allow(deprecated)] // connect => join in 1.3
fn join_licenses(v: Vec<String>) -> String {
    v.connect("/")
}

fn mk(config: &Config, path: &Path, name: &str,
      opts: &NewOptions) -> CargoResult<()> {
    let cfg = try!(global_config(config));
    let mut ignore = "target\n".to_string();
    let in_existing_vcs_repo = existing_vcs_repo(path.parent().unwrap(), config.cwd());
    if !opts.bin {
        ignore.push_str("Cargo.lock\n");
    }

    let vcs = match (opts.version_control, cfg.version_control, in_existing_vcs_repo) {
        (None, None, false) => VersionControl::Git,
        (None, Some(option), false) => option,
        (Some(option), _, _) => option,
        (_, _, true) => VersionControl::NoVcs,
    };

    match vcs {
        VersionControl::Git => {
            try!(GitRepo::init(path, config.cwd()));
            try!(paths::write(&path.join(".gitignore"), ignore.as_bytes()));
        },
        VersionControl::Hg => {
            try!(HgRepo::init(path, config.cwd()));
            try!(paths::write(&path.join(".hgignore"), ignore.as_bytes()));
        },
        VersionControl::NoVcs => {
            try!(fs::create_dir(path));
        },
    };

    let (author_name, email) = try!(discover_author());
    // Hoo boy, sure glad we've got exhaustivenes checking behind us.
    let author = match (cfg.name, cfg.email, author_name, email) {
        (Some(name), Some(email), _, _) |
        (Some(name), None, _, Some(email)) |
        (None, Some(email), name, _) |
        (None, None, name, Some(email)) => format!("{} <{}>", name, email),
        (Some(name), None, _, None) |
        (None, None, name, None) => name,
    };

    let license: Option<Vec<License>> = match (&opts.license, cfg.license) {
        (&None, None) => None,
        (&Some(ref lic), _) => Some(lic.clone()),
        (&_, Some(lic)) => Some(lic.clone()),
    };

    if license.is_some() {
        let license = license.unwrap();
        let license_string = join_licenses(license.iter()
                                                  .map(|l| format!("{}", l))
                                                  .collect::<Vec<_>>());
        try!(file(&path.join("Cargo.toml"), format!(
r#"[package]
name = "{}"
version = "0.1.0"
authors = [{}]
license = "{}"
"#, name, toml::Value::String(author.clone()), license_string).as_bytes()));

        // If there is more than one license, we suffix the filename
        // with the name of the license
        if license.len() > 1 {
            for l in &license {
                let upper = format!("{}", l).to_uppercase();
                try!(l.write_file(&path.join(format!("LICENSE-{}", upper)), &author));
            }
        } else {
            let license = license.get(0).unwrap();
            try!(license.write_file(&path.join("LICENSE"), &author))
        }
    } else {
        try!(file(&path.join("Cargo.toml"), format!(
r#"[package]
name = "{}"
version = "0.1.0"
authors = [{}]

[dependencies]
"#, name, toml::Value::String(author.clone())).as_bytes()));
    }

    try!(fs::create_dir(&path.join("src")));

    if opts.bin {
        try!(paths::write(&path.join("src/main.rs"), b"\
fn main() {
    println!(\"Hello, world!\");
}
"));
    } else {
        try!(paths::write(&path.join("src/lib.rs"), b"\
#[test]
fn it_works() {
}
"));
    }

    Ok(())
}

fn discover_author() -> CargoResult<(String, Option<String>)> {
    let git_config = GitConfig::open_default().ok();
    let git_config = git_config.as_ref();
    let name = git_config.and_then(|g| g.get_string("user.name").ok())
                         .map(|s| s.to_string())
                         .or_else(|| env::var("USER").ok())      // unix
                         .or_else(|| env::var("USERNAME").ok()); // windows
    let name = match name {
        Some(name) => name,
        None => {
            let username_var = if cfg!(windows) {"USERNAME"} else {"USER"};
            bail!("could not determine the current user, please set ${}",
                  username_var)
        }
    };
    let email = git_config.and_then(|g| g.get_string("user.email").ok())
                          .or_else(|| env::var("EMAIL").ok());

    let name = name.trim().to_string();
    let email = email.map(|s| s.trim().to_string());

    Ok((name, email))
}

fn global_config(config: &Config) -> CargoResult<CargoNewConfig> {
    let name = try!(config.get_string("cargo-new.name")).map(|s| s.0);
    let email = try!(config.get_string("cargo-new.email")).map(|s| s.0);
    let vcs = try!(config.get_string("cargo-new.vcs"));
    let license = try!(config.get_string("cargo-new.license"));

    let vcs = match vcs.as_ref().map(|p| (&p.0[..], &p.1)) {
        Some(("git", _)) => Some(VersionControl::Git),
        Some(("hg", _)) => Some(VersionControl::Hg),
        Some(("none", _)) => Some(VersionControl::NoVcs),
        Some((s, p)) => {
            return Err(internal(format!("invalid configuration for key \
                                         `cargo-new.vcs`, unknown vcs `{}` \
                                         (found in {:?})", s, p)))
        }
        None => None
    };
    let license: Option<Vec<License>> = match license.as_ref().map(|p| &p.0[..]) {
        Some(s) => {
            let r = &s[..];
            let mut licenses: Vec<License> = vec![];
            for lic in r.split("/") {
                licenses.push(try!(FromStr::from_str(lic)));
            }
            Some(licenses)
        },
        _ => None,
    };
    Ok(CargoNewConfig {
        name: name,
        email: email,
        version_control: vcs,
        license: license,
    })
}

#[cfg(test)]
mod tests {
    use super::strip_rust_affixes;

    #[test]
    fn affixes_stripped() {
        assert_eq!(strip_rust_affixes("rust-foo"), "foo");
        assert_eq!(strip_rust_affixes("foo-rs"), "foo");
        assert_eq!(strip_rust_affixes("rs_foo"), "foo");
        // Only one affix is stripped
        assert_eq!(strip_rust_affixes("rs-foo-rs"), "foo-rs");
        assert_eq!(strip_rust_affixes("foo-rs-rs"), "foo-rs");
        // It shouldn't touch the middle
        assert_eq!(strip_rust_affixes("some-rust-crate"), "some-rust-crate");
    }
}
