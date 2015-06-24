use std::env;
use std::fs::{self, File, DirEntry, OpenOptions};
use std::io::prelude::*;
use std::io;
use std::path::{Path, PathBuf, Component};

use rustc_serialize::{Decodable, Decoder};

use git2::Config as GitConfig;
use git2::Repository;

use term::color::BLACK;

use util::{GitRepo, HgRepo, CargoResult, human, ChainError, internal};
use util::Config;

use toml;
use mustache;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VersionControl { Git, Hg, NoVcs }

pub struct NewOptions<'a> {
    pub version_control: Option<VersionControl>,
    pub bin: bool,
    pub path: &'a str,
    pub name: Option<&'a str>,
    pub template: Option<&'a str>,
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

struct CargoNewConfig {
    name: Option<String>,
    email: Option<String>,
    version_control: Option<VersionControl>,
}

pub fn new(opts: NewOptions, config: &Config) -> CargoResult<()> {
    let path = config.cwd().join(opts.path);
    if fs::metadata(&path).is_ok() {
        return Err(human(format!("Destination `{}` already exists",
                                 path.display())))
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
        return Err(human(&format!("Invalid character `{}` in crate name: `{}`",
                                  c, name)));
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

fn existing_vcs_repo(path: &Path) -> bool {
    GitRepo::discover(path).is_ok() || HgRepo::discover(path).is_ok()
}

fn file(p: &Path, contents: &[u8]) -> io::Result<()> {
    try!(File::create(p)).write_all(contents)
}

fn mk(config: &Config, path: &Path, name: &str,
      opts: &NewOptions) -> CargoResult<()> {
    let cfg = try!(global_config(config));
    let mut ignore = "target\n".to_string();
    let in_existing_vcs_repo = existing_vcs_repo(path.parent().unwrap());
    if !opts.bin {
        ignore.push_str("Cargo.lock\n");
    }

    let vcs = match (opts.version_control, cfg.version_control, in_existing_vcs_repo) {
        (None, None, false) => VersionControl::Git,
        (None, Some(option), false) => option,
        (Some(option), _, false) => option,
        (_, _, true) => VersionControl::NoVcs,
    };

    match vcs {
        VersionControl::Git => {
            try!(GitRepo::init(path));
            try!(file(&path.join(".gitignore"), ignore.as_bytes()));
        },
        VersionControl::Hg => {
            try!(HgRepo::init(path));
            try!(file(&path.join(".hgignore"), ignore.as_bytes()));
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

    // find template base dir
    let template_dir = config.template_path();
    if fs::metadata(&template_dir).is_err() {
        try!(fs::create_dir(&template_dir));
    }

    // create lib & bin templates if not already present.
    let lib_template = template_dir.join("lib");
    let bin_template = template_dir.join("bin");

    try!(create_bin_template(&bin_template));
    try!(create_lib_template(&lib_template));

    let template = match opts.template {
        // given template is a remote git repository & needs to be cloned
        // This will be cloned to .cargo/templates/<repo_name> where <repo_name>
        // is the last component of the given URL. For example:
        //
        //      http://github.com/rust-lang/some-template
        //      <repo_name> = some-template
        Some(template) if template.starts_with("http") ||
                          template.starts_with("git@") => {
            let path = PathBuf::from(template);

            let repo_name = match path.components().last().unwrap() {
                Component::Normal(p) => p,
                _ => {
                    return Err(human(format!("Could not determine repository name from: {}", path.display())))
                }
            };
            let template_path = template_dir.join(repo_name);

            match Repository::clone(template, &*template_path) {
                Ok(_) => {},
                Err(e) => {
                    return Err(human(format!("Failed to clone repository: {} - {}", path.display(), e)))
                }
            };

            template_path
        }

        // given template is assumed to already be present on the users system
        // in .cargo/templates/<name>.
        Some(template) => { template_dir.join(template) }

        // no template given, use either "lib" or "bin" templates depending on the
        // presence of the --bin flag.
        None => { if opts.bin { bin_template } else { lib_template } }
    };

    // make sure that the template exists
    if fs::metadata(&template).is_err() {
        return Err(human(format!("Template `{}` not found", template.display())))
    }

    // contruct the mapping used to populate the template
    // if in the future we want to make more varaibles available in
    // the templates, this would be the place to do it.
    let data = mustache::MapBuilder::new()
        .insert_str("name", name)
        .insert_str("authors", toml::Value::String(author))
        .build();


    // For every file found inside the given template directory, compile it as a mustache
    // template and render it with the above data to a new file inside the target directory
    try!(walk_template_dir(&template, &mut |entry| {
        let path = entry.path();
        let entry_str = path.to_str().unwrap();
        let template_str = template.to_str().unwrap();

        // the path we have here is the absolute path to the file in the template directory
        // we need to trim this down to be just the path from the root of the template.
        // For example:
        //    /home/user/.cargo/templates/foo/Cargo.toml => Cargo.toml
        //    /home/user/.cargo/templates/foo/src/main.rs => src/main.rs
        let mut file_name = entry_str.replace(template_str, "");
        if file_name.starts_with("/") {
            file_name.remove(0);
        }

        let template = match mustache::compile_path(&path) {
            Ok(template) => template,
            Err(e) => panic!("Problem generating template {} - {:?}", path.display(), e)
        };
        let mut new_path = PathBuf::from(name).join(file_name);

        // file_name could now refer to a file inside a directory which doesn't yet exist
        // to figure out if this is the case, get all the components in the file_name and check
        // how many there are. Files in the root of the new project direcotory will have two
        // components, anything more than that means the file is in a sub-directory, so we need
        // to create it.
        {
            let components  = new_path.components().collect::<Vec<_>>();
            if components.len() > 2 {
                if let Some(p) = new_path.parent() {
                    if fs::metadata(&p).is_err() {
                        let _ = fs::create_dir_all(&p);
                    }
                }
            }
        }

        // if the template file has the ".mustache" extension, remove that to get the correct
        // name for the generated file
        if let Some(ext) = new_path.clone().extension() {
            if ext.to_str().unwrap() == "mustache" {
                new_path.set_extension("");
            }
        }

        // create the new file & render the template to it
        let mut file = OpenOptions::new().write(true).create(true).open(&new_path).unwrap();
        template.render_data(&mut file, &data);
    }));

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
            return Err(human(format!("could not determine the current \
                                      user, please set ${}", username_var)))
        }
    };
    let email = git_config.and_then(|g| g.get_string("user.email").ok());

    let name = name.trim().to_string();
    let email = email.map(|s| s.trim().to_string());

    Ok((name, email))
}

fn global_config(config: &Config) -> CargoResult<CargoNewConfig> {
    let name = try!(config.get_string("cargo-new.name")).map(|s| s.0);
    let email = try!(config.get_string("cargo-new.email")).map(|s| s.0);
    let vcs = try!(config.get_string("cargo-new.vcs"));

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
    Ok(CargoNewConfig {
        name: name,
        email: email,
        version_control: vcs,
    })
}

/// Recursively list directory contents under `dir`, only visiting files.
///
/// This will also filter out files & files types which we don't want to
/// try generate templates for. Image files, for instance.
///
/// It also filters out certain files & file types, as we don't want t
///
/// We use this instead of std::fs::walk_dir as it is marked as unstable for now
///
/// This is a modified version of the example at:
///    http://doc.rust-lang.org/std/fs/fn.read_dir.html
fn walk_template_dir(dir: &Path, cb: &mut FnMut(DirEntry)) -> CargoResult<()> {
    let attr = try!(fs::metadata(&dir));

    let ignore_files = vec!(".gitignore");
    let ignore_types = vec!("png", "jpg", "gif");

    if attr.is_dir() {
        for entry in try!(fs::read_dir(dir)) {
            let entry = try!(entry);
            let attr = try!(fs::metadata(&entry.path()));
            if attr.is_dir() {
                if !&entry.path().to_str().unwrap().contains(".git") {
                    try!(walk_template_dir(&entry.path(), cb));
                }
            } else {
                if let &Some(extension) = &entry.path().extension() {
                    if ignore_types.contains(&extension.to_str().unwrap()) {
                        continue
                    }
                }
                if let &Some(file_name) = &entry.path().file_name() {
                    if ignore_files.contains(&file_name.to_str().unwrap()) {
                        continue
                    }
                }
                cb(entry);
            }
        }
    }
    Ok(())
}

/// Create a generic template
///
/// This consists of a Cargo.toml, and a src directory.
fn create_generic_template(path: &PathBuf) -> CargoResult<()> {
    match fs::metadata(&path) {
        Ok(_) => {}
        Err(_) => { try!(fs::create_dir(&path)); }
    }
    match fs::metadata(&path.join("src")) {
        Ok(_) => {}
        Err(_) => { try!(fs::create_dir(&path.join("src"))); }
    }
    try!(file(&path.join("Cargo.toml"), b"\
[package]
name = \"{{name}}\"
version = \"0.1.0\"
authors = [{{{authors}}}]
"));
    Ok(())
}

/// Create a new "lib" project
fn create_lib_template(path: &PathBuf) -> CargoResult<()> {
    try!(create_generic_template(&path));
    try!(file(&path.join("src/lib.rs"), b"\
#[test]
fn it_works() {
}
"));
    Ok(())
}

/// Create a new "bin" project
fn create_bin_template(path: &PathBuf) -> CargoResult<()> {
    try!(create_generic_template(&path));
    try!(file(&path.join("src/main.rs"), b"\
fn main() {
    println!(\"Hello, world!\");
}
"));

    Ok(())
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
