use std::os;
use std::io::{mod, fs, File};

use git2::{Repository, Config};

use util::{CargoResult, human, ChainError};
use core::shell::MultiShell;

pub struct NewOptions<'a> {
    pub git: bool,
    pub bin: bool,
    pub path: &'a str,
}

pub fn new(opts: NewOptions, _shell: &mut MultiShell) -> CargoResult<()> {
    let path = os::getcwd().join(opts.path);
    if path.exists() {
        return Err(human(format!("Destination `{}` already exists",
                                 path.display())))
    }
    let name = path.filename_str().unwrap();
    mk(&path, name, &opts).chain_error(|| {
        human(format!("Failed to create project `{}` at `{}`",
                      name, path.display()))
    })
}

fn mk(path: &Path, name: &str, opts: &NewOptions) -> CargoResult<()> {

    if opts.git {
        try!(Repository::init(path));
        let mut gitignore = "/target\n".to_string();
        if !opts.bin {
            gitignore.push_str("/Cargo.lock\n");
        }
        try!(File::create(&path.join(".gitignore")).write(gitignore.as_bytes()));
    } else {
        try!(fs::mkdir(path, io::UserRWX));
    }

    let author = try!(discover_author());
    try!(File::create(&path.join("Cargo.toml")).write_str(format!(
r#"[package]

name = "{}"
version = "0.0.1"
authors = ["{}"]
"#, name, author).as_slice()));

    try!(fs::mkdir(&path.join("src"), io::UserRWX));

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

fn discover_author() -> CargoResult<String> {
    let git_config = Config::open_default().ok();
    let git_config = git_config.as_ref();
    let name = git_config.and_then(|g| g.get_str("user.name").ok())
                         .map(|s| s.to_string())
                         .or_else(|| os::getenv("USER"));
    let name = match name {
        Some(name) => name,
        None => return Err(human("could not determine the current user, \
                                  please set $USER"))
    };
    let email = git_config.and_then(|g| g.get_str("user.email").ok());

    let name = name.as_slice().trim().to_string();
    let email = email.map(|s| s.as_slice().trim().to_string());

    Ok(match (name, email) {
        (name, Some(email)) => format!("{} <{}>", name, email),
        (name, None) => name,
    })
}
