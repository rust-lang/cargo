use std::env;
use std::fs::{self, DirEntry, File};
use std::path::{Path, PathBuf};
use std::collections::BTreeMap;
use rustc_serialize::{Decodable, Decoder};

use git2::Config as GitConfig;

use term::color::BLACK;

use handlebars::{Handlebars, no_escape};
use tempdir::TempDir;

use core::Workspace;
use sources::git::clone;
use util::{GitRepo, HgRepo, CargoResult, human, ChainError, internal};
use util::{Config, paths, template};
use util::template::{TemplateSet, TemplateFile, TemplateDirectory, TemplateType};
use util::template::{InputFileTemplateFile, InMemoryTemplateFile, get_template_type};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VersionControl { Git, Hg, NoVcs }

pub struct NewOptions<'a> {
    pub version_control: Option<VersionControl>,
    pub bin: bool,
    pub lib: bool,
    pub path: &'a str,
    pub name: Option<&'a str>,
    pub template_subdir: Option<&'a str>,
    pub template: Option<&'a str>,
}

struct SourceFileInformation {
    relative_path: String,
    target_name: String,
    bin: bool,
}

struct MkOptions<'a> {
    version_control: Option<VersionControl>,
    template_subdir: Option<&'a str>,
    template: Option<&'a str>,
    path: &'a Path,
    name: &'a str,
    bin: bool,
}

impl Decodable for VersionControl {
    fn decode<D: Decoder>(d: &mut D) -> Result<VersionControl, D::Error> {
        Ok(match &d.read_str()?[..] {
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

impl<'a> NewOptions<'a> {
    pub fn new(version_control: Option<VersionControl>,
           bin: bool,
           lib: bool,
           path: &'a str,
           name: Option<&'a str>,
           template_subdir: Option<&'a str>,
           template: Option<&'a str>) -> NewOptions<'a> {

        // default to lib
        let is_lib = if !bin {
            true
        }
        else {
            lib
        };

        NewOptions {
            version_control: version_control,
            bin: bin,
            lib: is_lib,
            path: path,
            name: name,
            template_subdir: template_subdir,
            template: template,
        }
    }
}

struct CargoNewConfig {
    name: Option<String>,
    email: Option<String>,
    version_control: Option<VersionControl>,
}

fn get_input_template(config: &Config, opts: &MkOptions) -> CargoResult<TemplateSet> {
    let name = opts.name;

    let template_type = try!(get_template_type(opts.template, opts.template_subdir));
    let template_set = match template_type {
        // given template is a remote git repository & needs to be cloned
        TemplateType::GitRepo(repo_url) => {
            let template_dir = try!(TempDir::new(name));
            config.shell().status("Cloning", &repo_url)?;
            clone(&repo_url, &template_dir.path(), &config)?;
            let template_path = find_template_subdir(&template_dir.path(), opts.template_subdir);
            TemplateSet {
                template_dir: Some(TemplateDirectory::Temp(template_dir)),
                template_files: try!(collect_template_dir(&template_path, opts.path))
            }
        },
        // given template is a local directory
        TemplateType::LocalDir(directory) => {
            // make sure that the template exists
            if fs::metadata(&directory).is_err() {
                bail!("template `{}` not found", directory);
            }
            let template_path = find_template_subdir(&PathBuf::from(&directory),
                                                     opts.template_subdir);
            TemplateSet {
                template_dir: Some(TemplateDirectory::Normal(PathBuf::from(directory))),
                template_files: try!(collect_template_dir(&template_path, opts.path))
            }
        },
        // no template given, use either "lib" or "bin" templates depending on the
        // presence of the --bin flag.
        TemplateType::Builtin => {
            let template_files = if opts.bin {
                create_bin_template()
            } else {
                create_lib_template()
            };
            TemplateSet {
                template_dir: None,
                template_files: template_files
            }
        }
    };
    Ok(template_set)
}

fn get_name<'a>(path: &'a Path, opts: &'a NewOptions, config: &Config) -> CargoResult<&'a str> {
    if let Some(name) = opts.name {
        return Ok(name);
    }

    if path.file_name().is_none() {
        bail!("cannot auto-detect project name from path {:?} ; use --name to override",
                              path.as_os_str());
    }

    let dir_name = path.file_name().and_then(|s| s.to_str()).chain_error(|| {
        human(&format!("cannot create a project with a non-unicode name: {:?}",
                       path.file_name().unwrap()))
    })?;

    if opts.bin {
        Ok(dir_name)
    } else {
        let new_name = strip_rust_affixes(dir_name);
        if new_name != dir_name {
            let message = format!(
                "note: package will be named `{}`; use --name to override",
                new_name);
            config.shell().say(&message, BLACK)?;
        }
        Ok(new_name)
    }
}

fn check_name(name: &str) -> CargoResult<()> {

    // Ban keywords + test list found at
    // https://doc.rust-lang.org/grammar.html#keywords
    let blacklist = ["abstract", "alignof", "as", "become", "box",
        "break", "const", "continue", "crate", "do",
        "else", "enum", "extern", "false", "final",
        "fn", "for", "if", "impl", "in",
        "let", "loop", "macro", "match", "mod",
        "move", "mut", "offsetof", "override", "priv",
        "proc", "pub", "pure", "ref", "return",
        "self", "sizeof", "static", "struct",
        "super", "test", "trait", "true", "type", "typeof",
        "unsafe", "unsized", "use", "virtual", "where",
        "while", "yield"];
    if blacklist.contains(&name) {
        bail!("The name `{}` cannot be used as a crate name\n\
               use --name to override crate name",
               name)
    }

    if let Some(ref c) = name.chars().nth(0) {
        if c.is_digit(10) {
            bail!("Package names starting with a digit cannot be used as a crate name\n\
               use --name to override crate name")
        }
    }

    for c in name.chars() {
        if c.is_alphanumeric() { continue }
        if c == '_' || c == '-' { continue }
        bail!("Invalid character `{}` in crate name: `{}`\n\
               use --name to override crate name",
              c, name)
    }
    Ok(())
}

fn detect_source_paths_and_types(project_path : &Path,
                                 project_name: &str,
                                 detected_files: &mut Vec<SourceFileInformation>,
                                 ) -> CargoResult<()> {
    let path = project_path;
    let name = project_name;

    enum H {
        Bin,
        Lib,
        Detect,
    }

    struct Test {
        proposed_path: String,
        handling: H,
    }

    let tests = vec![
        Test { proposed_path: format!("src/main.rs"),     handling: H::Bin },
        Test { proposed_path: format!("main.rs"),         handling: H::Bin },
        Test { proposed_path: format!("src/{}.rs", name), handling: H::Detect },
        Test { proposed_path: format!("{}.rs", name),     handling: H::Detect },
        Test { proposed_path: format!("src/lib.rs"),      handling: H::Lib },
        Test { proposed_path: format!("lib.rs"),          handling: H::Lib },
    ];

    for i in tests {
        let pp = i.proposed_path;

        // path/pp does not exist or is not a file
        if !fs::metadata(&path.join(&pp)).map(|x| x.is_file()).unwrap_or(false) {
            continue;
        }

        let sfi = match i.handling {
            H::Bin => {
                SourceFileInformation {
                    relative_path: pp,
                    target_name: project_name.to_string(),
                    bin: true
                }
            }
            H::Lib => {
                SourceFileInformation {
                    relative_path: pp,
                    target_name: project_name.to_string(),
                    bin: false
                }
            }
            H::Detect => {
                let content = paths::read(&path.join(pp.clone()))?;
                let isbin = content.contains("fn main");
                SourceFileInformation {
                    relative_path: pp,
                    target_name: project_name.to_string(),
                    bin: isbin
                }
            }
        };
        detected_files.push(sfi);
    }

    // Check for duplicate lib attempt

    let mut previous_lib_relpath : Option<&str> = None;
    let mut duplicates_checker : BTreeMap<&str, &SourceFileInformation> = BTreeMap::new();

    for i in detected_files {
        if i.bin {
            if let Some(x) = BTreeMap::get::<str>(&duplicates_checker, i.target_name.as_ref()) {
                bail!("\
multiple possible binary sources found:
  {}
  {}
cannot automatically generate Cargo.toml as the main target would be ambiguous",
                      &x.relative_path, &i.relative_path);
            }
            duplicates_checker.insert(i.target_name.as_ref(), i);
        } else {
            if let Some(plp) = previous_lib_relpath {
                return Err(human(format!("cannot have a project with \
                                         multiple libraries, \
                                         found both `{}` and `{}`",
                                         plp, i.relative_path)));
            }
            previous_lib_relpath = Some(&i.relative_path);
        }
    }

    Ok(())
}

fn plan_new_source_file(bin: bool, project_name: String) -> SourceFileInformation {
    if bin {
        SourceFileInformation {
             relative_path: "src/main.rs".to_string(),
             target_name: project_name,
             bin: true,
        }
    } else {
        SourceFileInformation {
             relative_path: "src/lib.rs".to_string(),
             target_name: project_name,
             bin: false,
        }
    }
}

pub fn new(opts: NewOptions, config: &Config) -> CargoResult<()> {
    let path = config.cwd().join(opts.path);
    if fs::metadata(&path).is_ok() {
        bail!("destination `{}` already exists",
              path.display())
    }

    if opts.lib && opts.bin {
        bail!("can't specify both lib and binary outputs");
    }

    let name = get_name(&path, &opts, config)?;
    check_name(name)?;

    let mkopts = MkOptions {
        version_control: opts.version_control,
        template_subdir: opts.template_subdir,
        template: opts.template,
        path: &path,
        name: name,
        bin: opts.bin,
    };

    mk(config, &mkopts).chain_error(|| {
        human(format!("Failed to create project `{}` at `{}`",
                      name, path.display()))
    })
}

pub fn init(opts: NewOptions, config: &Config) -> CargoResult<()> {
    let path = config.cwd().join(opts.path);

    let cargotoml_path = path.join("Cargo.toml");
    if fs::metadata(&cargotoml_path).is_ok() {
        bail!("`cargo init` cannot be run on existing Cargo projects")
    }

    if opts.lib && opts.bin {
        bail!("can't specify both lib and binary outputs");
    }

    let name = get_name(&path, &opts, config)?;
    check_name(name)?;

    let mut src_paths_types = vec![];

    detect_source_paths_and_types(&path, name, &mut src_paths_types)?;

    if src_paths_types.len() == 0 {
        src_paths_types.push(plan_new_source_file(opts.bin, name.to_string()));
    } else {
        // --bin option may be ignored if lib.rs or src/lib.rs present
        // Maybe when doing `cargo init --bin` inside a library project stub,
        // user may mean "initialize for library, but also add binary target"
    }

    let mut version_control = opts.version_control;

    if version_control == None {
        let mut num_detected_vsces = 0;

        if fs::metadata(&path.join(".git")).is_ok() {
            version_control = Some(VersionControl::Git);
            num_detected_vsces += 1;
        }

        if fs::metadata(&path.join(".hg")).is_ok() {
            version_control = Some(VersionControl::Hg);
            num_detected_vsces += 1;
        }

        // if none exists, maybe create git, like in `cargo new`

        if num_detected_vsces > 1 {
            bail!("both .git and .hg directories found \
                              and the ignore file can't be \
                              filled in as a result, \
                              specify --vcs to override detection");
        }
    }

    let mkopts = MkOptions {
        version_control: version_control,
        template_subdir: opts.template_subdir,
        template: opts.template,
        path: &path,
        name: name,
        bin: src_paths_types.iter().any(|x|x.bin),
    };

    mk(config, &mkopts).chain_error(|| {
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

fn mk(config: &Config, opts: &MkOptions) -> CargoResult<()> {
    let path = opts.path;
    let name = opts.name;
    let cfg = global_config(config)?;
    let ignore = ["target/\n", "**/*.rs.bk\n",
        if !opts.bin { "Cargo.lock\n" } else { "" }]
        .concat();

    let in_existing_vcs_repo = existing_vcs_repo(path.parent().unwrap(), config.cwd());
    let vcs = match (opts.version_control, cfg.version_control, in_existing_vcs_repo) {
        (None, None, false) => VersionControl::Git,
        (None, Some(option), false) => option,
        (Some(option), _, _) => option,
        (_, _, true) => VersionControl::NoVcs,
    };
    match vcs {
        VersionControl::Git => {
            if !fs::metadata(&path.join(".git")).is_ok() {
                GitRepo::init(path, config.cwd())?;
            }
            paths::append(&path.join(".gitignore"), ignore.as_bytes())?;
        },
        VersionControl::Hg => {
            if !fs::metadata(&path.join(".hg")).is_ok() {
                HgRepo::init(path, config.cwd())?;
            }
            paths::append(&path.join(".hgignore"), ignore.as_bytes())?;
        },
        VersionControl::NoVcs => {
            fs::create_dir_all(path)?;
        },
    };

    let (author_name, email) = discover_author()?;
    // Hoo boy, sure glad we've got exhaustivenes checking behind us.
    let author = match (cfg.name.clone(), cfg.email.clone(), author_name, email) {
        (Some(name), Some(email), _, _) |
        (Some(name), None, _, Some(email)) |
        (None, Some(email), name, _) |
        (None, None, name, Some(email)) => format!("{} <{}>", name, email),
        (Some(name), None, _, None) |
        (None, None, name, None) => name,
    };

    // construct the mapping used to populate the template
    // if in the future we want to make more varaibles available in
    // the templates, this would be the place to do it.
    let mut handlebars = Handlebars::new();
    // We don't want html escaping unless users explicitly ask for it...
    handlebars.register_escape_fn(no_escape);
    handlebars.register_helper("toml-escape", Box::new(template::toml_escape_helper));
    handlebars.register_helper("html-escape", Box::new(template::html_escape_helper));

    let mut data = BTreeMap::new();
    data.insert("name".to_owned(), name.to_owned());
    data.insert("author".to_owned(), author);

    let template_set = try!(get_input_template(config, opts));
    for template in template_set.template_files.iter() {
        let template_str = try!(template.template());
        let dest_path = path.join(template.path());

        // Skip files that already exist.
        if fs::metadata(&dest_path).is_ok() {
            continue;
        }

        let parent = try!(dest_path.parent()
                          .chain_error(|| {
                              human(format!("failed to make sure parent directory \
                                             exists for {}", dest_path.display()))
                          }));
        try!(fs::create_dir_all(&parent)
             .chain_error(|| {
                 human(format!("failed to create path to destination file {}",
                               parent.display()))
             }));

        // create the new file & render the template to it
        let mut dest_file = try!(File::create(&dest_path).chain_error(|| {
                                     human(format!("failed to open file for writing: {}",
                                                   dest_path.display()))
                                 }));

        try!(handlebars.template_renderw(&template_str, &data, &mut dest_file)
            .chain_error(|| {
                human(format!("Failed to render template for file: {}", dest_path.display()))
        }))
    }

    if let Err(e) = Workspace::new(&path.join("Cargo.toml"), config) {
        let msg = format!("compiling this new crate may not work due to invalid \
                           workspace configuration\n\n{}", e);
        config.shell().warn(msg)?;
    }
    Ok(())
}

// When the command line has --template=<repository-or-directory> and
// --template-subdir=<template-name> then find_template_subdir fixes up the name as appropriate.
fn find_template_subdir(template_dir: &Path, template: Option<&str>) -> PathBuf {
    match template {
        Some(template) => template_dir.join(template),
        None => template_dir.to_path_buf()
    }
}

fn collect_template_dir(template_path: &PathBuf, _: &Path) -> CargoResult<Vec<Box<TemplateFile>>> {
    let mut templates = Vec::<Box<TemplateFile>>::new();
    // For every file found inside the given template directory, compile it as a handlebars
    // template and render it with the above data to a new file inside the target directory
    try!(walk_template_dir(&template_path, &mut |entry| {
        let entry_path = entry.path();
        let dest_file_name = PathBuf::from(try!(entry_path.strip_prefix(&template_path)
                                  .chain_error(|| {
                                      human(format!("entry is somehow not a subpath \
                                                     of the directory being walked."))
                                  })));
        templates.push(Box::new(InputFileTemplateFile::new(entry_path, 
                                                           dest_file_name.to_path_buf())));
        Ok(())
    }));
    Ok(templates)
}

fn get_environment_variable(variables: &[&str] ) -> Option<String>{
    variables.iter()
             .filter_map(|var| env::var(var).ok())
             .next()
}

fn discover_author() -> CargoResult<(String, Option<String>)> {
    let git_config = GitConfig::open_default().ok();
    let git_config = git_config.as_ref();
    let name_variables = ["CARGO_NAME", "GIT_AUTHOR_NAME", "GIT_COMMITTER_NAME",
                         "USER", "USERNAME", "NAME"];
    let name = get_environment_variable(&name_variables[0..3])
                        .or_else(|| git_config.and_then(|g| g.get_string("user.name").ok()))
                        .or_else(|| get_environment_variable(&name_variables[3..]));

    let name = match name {
        Some(name) => name,
        None => {
            let username_var = if cfg!(windows) {"USERNAME"} else {"USER"};
            bail!("could not determine the current user, please set ${}",
                  username_var)
        }
    };
    let email_variables = ["CARGO_EMAIL", "GIT_AUTHOR_EMAIL", "GIT_COMMITTER_EMAIL",
                          "EMAIL"];
    let email = get_environment_variable(&email_variables[0..3])
                          .or_else(|| git_config.and_then(|g| g.get_string("user.email").ok()))
                          .or_else(|| get_environment_variable(&email_variables[3..]));

    let name = name.trim().to_string();
    let email = email.map(|s| s.trim().to_string());

    Ok((name, email))
}

fn global_config(config: &Config) -> CargoResult<CargoNewConfig> {
    let name = config.get_string("cargo-new.name")?.map(|s| s.val);
    let email = config.get_string("cargo-new.email")?.map(|s| s.val);
    let vcs = config.get_string("cargo-new.vcs")?;

    let vcs = match vcs.as_ref().map(|p| (&p.val[..], &p.definition)) {
        Some(("git", _)) => Some(VersionControl::Git),
        Some(("hg", _)) => Some(VersionControl::Hg),
        Some(("none", _)) => Some(VersionControl::NoVcs),
        Some((s, p)) => {
            return Err(internal(format!("invalid configuration for key \
                                         `cargo-new.vcs`, unknown vcs `{}` \
                                         (found in {})", s, p)))
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
fn walk_template_dir(dir: &Path, cb: &mut FnMut(DirEntry) -> CargoResult<()>) -> CargoResult<()> {
    let attr = try!(fs::metadata(&dir));
    let ignore_files = vec![".gitignore"];

    if !attr.is_dir() {
        return Ok(());
    }
    for entry in try!(fs::read_dir(dir)) {
        let entry = try!(entry);
        let attr = try!(fs::metadata(&entry.path()));
        if attr.is_dir() {
            if let Some(ref path_str) = entry.path().to_str() {
                if !&path_str.contains(".git") {
                    try!(walk_template_dir(&entry.path(), cb));
                }
            }
        } else {
            if let Some(file_name) = entry.path().file_name() {
                if ignore_files.contains(&file_name.to_str().unwrap()) {
                    continue
                }
            }
            try!(cb(entry));
        }
    }
    Ok(())
}

/// Create a generic template
///
/// This consists of a Cargo.toml, and a src directory.
fn create_generic_template() -> Vec<Box<TemplateFile>> {
    let template_file = Box::new(InMemoryTemplateFile::new(PathBuf::from("Cargo.toml"),
    String::from(r#"[package]
name = "{{name}}"
version = "0.1.0"
authors = [{{toml-escape author}}]

[dependencies]
"#)));
    vec![template_file]
}

/// Create a new "lib" project
fn create_lib_template() -> Vec<Box<TemplateFile>> {
    let mut template_files = create_generic_template();
    let lib_file = Box::new(InMemoryTemplateFile::new(PathBuf::from("src/lib.rs"),
    String::from(r#"#[test]
fn it_works() {
}
"#)));
    template_files.push(lib_file);
    template_files
}

/// Create a new "bin" project
fn create_bin_template() -> Vec<Box<TemplateFile>> {
    let mut template_files = create_generic_template();
    let main_file = Box::new(InMemoryTemplateFile::new(PathBuf::from("src/main.rs"),
String::from("fn main() {
    println!(\"Hello, world!\");
}
")));
    template_files.push(main_file);
    template_files
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
