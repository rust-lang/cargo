use std::os;
use std::io::{mod, fs, File};
use std::io::fs::PathExtensions;

use git2::Config;

use util::{GitRepo, HgRepo, CargoResult, human, ChainError, config, internal};
use core::shell::MultiShell;

pub struct NewOptions<'a> {
    pub no_git: bool,
    pub git: bool,
    pub hg: bool,
    pub travis: bool,
    pub bin: bool,
    pub path: &'a str,
}

struct CargoNewConfig {
    name: Option<String>,
    email: Option<String>,
    git: Option<bool>,
}

pub fn new(opts: NewOptions, _shell: &mut MultiShell) -> CargoResult<()> {
    let path = os::getcwd().join(opts.path);
    if path.exists() {
        return Err(human(format!("Destination `{}` already exists",
                                 path.display())))
    }
    let name = path.filename_str().unwrap();
    for c in name.chars() {
        if c.is_alphanumeric() { continue }
        if c == '_' || c == '-' { continue }
        return Err(human(format!("Invalid character `{}` in crate name: `{}`",
                                 c, name).as_slice()));
    }
    mk(&path, name, &opts).chain_error(|| {
        human(format!("Failed to create project `{}` at `{}`",
                      name, path.display()))
    })
}

fn existing_git_repo(path: &Path) -> bool {
    GitRepo::discover(path).is_ok()
}

fn mk(path: &Path, name: &str, opts: &NewOptions) -> CargoResult<()> {
    let cfg = try!(global_config());
    let mut ignore = "/target\n".to_string();
    let no_git = !opts.git && (opts.no_git || cfg.git == Some(false));
    let in_existing_git_repo = existing_git_repo(&path.dir_path());
    if !opts.bin {
        ignore.push_str("/Cargo.lock\n");
    }

    if opts.hg {
        try!(HgRepo::init(path));
        try!(File::create(&path.join(".hgignore")).write(ignore.as_bytes()));
    } else if no_git || in_existing_git_repo {
        try!(fs::mkdir(path, io::USER_RWX));
    } else {
        try!(GitRepo::init(path));
        try!(File::create(&path.join(".gitignore")).write(ignore.as_bytes()));
    }

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

    if opts.travis {
        try!(File::create(&path.join(".travis.yml")).write_str("language: rust\n"));
    }

    try!(File::create(&path.join("Cargo.toml")).write_str(format!(
r#"[package]

name = "{}"
version = "0.0.1"
authors = ["{}"]
"#, name, author).as_slice()));

    try!(fs::mkdir(&path.join("src"), io::USER_RWX));

    if opts.bin {
        try!(File::create(&path.join("src/main.rs")).write_str("\
fn main() {
    println!(\"Hello, world!\")
}
"));
    } else {
        try!(File::create(&path.join("src/lib.rs")).write_str("\
#[test]
fn it_works() {
}
"));
    }

    Ok(())
}

fn discover_author() -> CargoResult<(String, Option<String>)> {
    let git_config = Config::open_default().ok();
    let git_config = git_config.as_ref();
    let name = git_config.and_then(|g| g.get_str("user.name").ok())
                         .map(|s| s.to_string())
                         .or_else(|| os::getenv("USER"))      // unix
                         .or_else(|| os::getenv("USERNAME")); // windows
    let name = match name {
        Some(name) => name,
        None => {
            let username_var = if cfg!(windows) {"USERNAME"} else {"USER"};
            return Err(human(format!("could not determine the current \
                                      user, please set ${}", username_var)))
        }
    };
    let email = git_config.and_then(|g| g.get_str("user.email").ok());

    let name = name.as_slice().trim().to_string();
    let email = email.map(|s| s.as_slice().trim().to_string());

    Ok((name, email))
}

fn global_config() -> CargoResult<CargoNewConfig> {
    let user_configs = try!(config::all_configs(os::getcwd()));
    let mut cfg = CargoNewConfig {
        name: None,
        email: None,
        git: None,
    };
    let cargo_new = match user_configs.find_equiv("cargo-new") {
        None => return Ok(cfg),
        Some(target) => try!(target.table().chain_error(|| {
            internal("invalid configuration for the key `cargo-new`")
        })),
    };
    cfg.name = match cargo_new.find_equiv("name") {
        None => None,
        Some(name) => {
            Some(try!(name.string().chain_error(|| {
                internal("invalid configuration for key `cargo-new.name`")
            })).ref0().to_string())
        }
    };
    cfg.email = match cargo_new.find_equiv("email") {
        None => None,
        Some(email) => {
            Some(try!(email.string().chain_error(|| {
                internal("invalid configuration for key `cargo-new.email`")
            })).ref0().to_string())
        }
    };
    cfg.git = match cargo_new.find_equiv("git") {
        None => None,
        Some(git) => {
            Some(try!(git.boolean().chain_error(|| {
                internal("invalid configuration for key `cargo-new.git`")
            })).val0())
        }
    };

    Ok(cfg)
}
