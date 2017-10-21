use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Deserializer};
use serde::de;

use git2::Config as GitConfig;
use git2::Repository as GitRepository;

use core::Workspace;
use ops::is_bad_artifact_name;
use util::{GitRepo, HgRepo, PijulRepo, FossilRepo, internal};
use util::{Config, paths};
use util::errors::{CargoError, CargoResult, CargoResultExt};

use toml;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VersionControl { Git, Hg, Pijul, Fossil, NoVcs }

pub struct NewOptions<'a> {
    pub version_control: Option<VersionControl>,
    pub bin: bool,
    pub lib: bool,
    pub path: &'a str,
    pub name: Option<&'a str>,
}

struct SourceFileInformation {
    relative_path: String,
    target_name: String,
    bin: bool,
}

struct MkOptions<'a> {
    version_control: Option<VersionControl>,
    path: &'a Path,
    name: &'a str,
    source_files: Vec<SourceFileInformation>,
    bin: bool,
}

impl<'de> Deserialize<'de> for VersionControl {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<VersionControl, D::Error> {
        Ok(match &String::deserialize(d)?[..] {
            "git" => VersionControl::Git,
            "hg" => VersionControl::Hg,
            "pijul" => VersionControl::Pijul,
            "fossil" => VersionControl::Fossil,
            "none" => VersionControl::NoVcs,
            n => {
                let value = de::Unexpected::Str(n);
                let msg = "unsupported version control system";
                return Err(de::Error::invalid_value(value, &msg));
            }
        })
    }
}

impl<'a> NewOptions<'a> {
    pub fn new(version_control: Option<VersionControl>,
           bin: bool,
           lib: bool,
           path: &'a str,
           name: Option<&'a str>) -> NewOptions<'a> {

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
        }
    }
}

struct CargoNewConfig {
    name: Option<String>,
    email: Option<String>,
    version_control: Option<VersionControl>,
}

fn get_name<'a>(path: &'a Path, opts: &'a NewOptions, config: &Config) -> CargoResult<&'a str> {
    if let Some(name) = opts.name {
        return Ok(name);
    }

    if path.file_name().is_none() {
        bail!("cannot auto-detect project name from path {:?} ; use --name to override",
                              path.as_os_str());
    }

    let dir_name = path.file_name().and_then(|s| s.to_str()).ok_or_else(|| {
        CargoError::from(format!("cannot create a project with a non-unicode name: {:?}",
                                 path.file_name().unwrap()))
    })?;

    if opts.bin {
        Ok(dir_name)
    } else {
        let new_name = strip_rust_affixes(dir_name);
        if new_name != dir_name {
            writeln!(config.shell().err(),
                     "note: package will be named `{}`; use --name to override",
                     new_name)?;
        }
        Ok(new_name)
    }
}

fn check_name(name: &str, is_bin: bool) -> CargoResult<()> {

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
    if blacklist.contains(&name) || (is_bin && is_bad_artifact_name(name)) {
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
                return Err(format!("cannot have a project with \
                                    multiple libraries, \
                                    found both `{}` and `{}`",
                                   plp, i.relative_path).into());
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

pub fn new(opts: &NewOptions, config: &Config) -> CargoResult<()> {
    let path = config.cwd().join(opts.path);
    if fs::metadata(&path).is_ok() {
        bail!("destination `{}` already exists\n\n\
            Use `cargo init` to initialize the directory\
            ", path.display()
        )
    }

    if opts.lib && opts.bin {
        bail!("can't specify both lib and binary outputs")
    }

    let name = get_name(&path, opts, config)?;
    check_name(name, opts.bin)?;

    let mkopts = MkOptions {
        version_control: opts.version_control,
        path: &path,
        name: name,
        source_files: vec![plan_new_source_file(opts.bin, name.to_string())],
        bin: opts.bin,
    };

    mk(config, &mkopts).chain_err(|| {
        format!("Failed to create project `{}` at `{}`",
                name, path.display())
    })
}

pub fn init(opts: &NewOptions, config: &Config) -> CargoResult<()> {
    let path = config.cwd().join(opts.path);

    let cargotoml_path = path.join("Cargo.toml");
    if fs::metadata(&cargotoml_path).is_ok() {
        bail!("`cargo init` cannot be run on existing Cargo projects")
    }

    if opts.lib && opts.bin {
        bail!("can't specify both lib and binary outputs");
    }

    let name = get_name(&path, opts, config)?;
    check_name(name, opts.bin)?;

    let mut src_paths_types = vec![];

    detect_source_paths_and_types(&path, name, &mut src_paths_types)?;

    if src_paths_types.is_empty() {
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

        if fs::metadata(&path.join(".pijul")).is_ok() {
            version_control = Some(VersionControl::Pijul);
            num_detected_vsces += 1;
        }

        if fs::metadata(&path.join(".fossil")).is_ok() {
            version_control = Some(VersionControl::Fossil);
            num_detected_vsces += 1;
        }

        // if none exists, maybe create git, like in `cargo new`

        if num_detected_vsces > 1 {
            bail!("more than one of .hg, .git, .pijul, .fossil configurations \
                              found and the ignore file can't be filled in as \
                              a result. specify --vcs to override detection");
        }
    }

    let mkopts = MkOptions {
        version_control: version_control,
        path: &path,
        name: name,
        bin: src_paths_types.iter().any(|x|x.bin),
        source_files: src_paths_types,
    };

    mk(config, &mkopts).chain_err(|| {
        format!("Failed to create project `{}` at `{}`",
                 name, path.display())
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
    // Please ensure that ignore and hgignore are in sync.
    let ignore = ["\n", "/target/\n", "**/*.rs.bk\n",
        if !opts.bin { "Cargo.lock\n" } else { "" }]
        .concat();
    // Mercurial glob ignores can't be rooted, so just sticking a 'syntax: glob' at the top of the
    // file will exclude too much. Instead, use regexp-based ignores. See 'hg help ignore' for
    // more.
    let hgignore = ["\n", "^target/\n", "glob:*.rs.bk\n",
        if !opts.bin { "glob:Cargo.lock\n" } else { "" }]
        .concat();

    let in_existing_vcs_repo = existing_vcs_repo(path.parent().unwrap_or(path), config.cwd());
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
            paths::append(&path.join(".hgignore"), hgignore.as_bytes())?;
        },
        VersionControl::Pijul => {
            if !fs::metadata(&path.join(".pijul")).is_ok() {
                PijulRepo::init(path, config.cwd())?;
            }
        },
        VersionControl::Fossil => {
            if !fs::metadata(&path.join(".fossil")).is_ok() {
                FossilRepo::init(path, config.cwd())?;
            }
        },
        VersionControl::NoVcs => {
            fs::create_dir_all(path)?;
        },
    };

    let (author_name, email) = discover_author()?;
    // Hoo boy, sure glad we've got exhaustiveness checking behind us.
    let author = match (cfg.name, cfg.email, author_name, email) {
        (Some(name), Some(email), _, _) |
        (Some(name), None, _, Some(email)) |
        (None, Some(email), name, _) |
        (None, None, name, Some(email)) => format!("{} <{}>", name, email),
        (Some(name), None, _, None) |
        (None, None, name, None) => name,
    };

    let mut cargotoml_path_specifier = String::new();

    // Calculate what [lib] and [[bin]]s do we need to append to Cargo.toml

    for i in &opts.source_files {
        if i.bin {
            if i.relative_path != "src/main.rs" {
                cargotoml_path_specifier.push_str(&format!(r#"
[[bin]]
name = "{}"
path = {}
"#, i.target_name, toml::Value::String(i.relative_path.clone())));
            }
        } else if i.relative_path != "src/lib.rs" {
            cargotoml_path_specifier.push_str(&format!(r#"
[lib]
name = "{}"
path = {}
"#, i.target_name, toml::Value::String(i.relative_path.clone())));
        }
    }

    // Create Cargo.toml file with necessary [lib] and [[bin]] sections, if needed

    paths::write(&path.join("Cargo.toml"), format!(
r#"[package]
name = "{}"
version = "0.1.0"
authors = [{}]

[dependencies]
{}"#, name, toml::Value::String(author), cargotoml_path_specifier).as_bytes())?;


    // Create all specified source files
    // (with respective parent directories)
    // if they are don't exist

    for i in &opts.source_files {
        let path_of_source_file = path.join(i.relative_path.clone());

        if let Some(src_dir) = path_of_source_file.parent() {
            fs::create_dir_all(src_dir)?;
        }

        let default_file_content : &[u8] = if i.bin {
            b"\
fn main() {
    println!(\"Hello, world!\");
}
"
        } else {
            b"\
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
"
        };

        if !fs::metadata(&path_of_source_file).map(|x| x.is_file()).unwrap_or(false) {
            paths::write(&path_of_source_file, default_file_content)?;
        }
    }

    if let Err(e) = Workspace::new(&path.join("Cargo.toml"), config) {
        let msg = format!("compiling this new crate may not work due to invalid \
                           workspace configuration\n\n{}", e);
        config.shell().warn(msg)?;
    }

    Ok(())
}

fn get_environment_variable(variables: &[&str] ) -> Option<String>{
    variables.iter()
             .filter_map(|var| env::var(var).ok())
             .next()
}

fn discover_author() -> CargoResult<(String, Option<String>)> {
    let cwd = env::current_dir()?;
    let git_config = if let Ok(repo) = GitRepository::discover(&cwd) {
        repo.config().ok().or_else(|| GitConfig::open_default().ok())
    } else {
        GitConfig::open_default().ok()
    };
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
