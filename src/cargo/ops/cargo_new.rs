use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};

use git2::Config as GitConfig;
use git2::Repository as GitRepository;

use crate::core::{compiler, Workspace};
use crate::util::errors::{self, CargoResult, CargoResultExt};
use crate::util::{existing_vcs_repo, internal, FossilRepo, GitRepo, HgRepo, PijulRepo};
use crate::util::{paths, validate_package_name, Config};

use toml;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VersionControl {
    Git,
    Hg,
    Pijul,
    Fossil,
    NoVcs,
}

#[derive(Debug)]
pub struct NewOptions {
    pub version_control: Option<VersionControl>,
    pub kind: NewProjectKind,
    /// Absolute path to the directory for the new package
    pub path: PathBuf,
    pub name: Option<String>,
    pub edition: Option<String>,
    pub registry: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewProjectKind {
    Bin,
    Lib,
}

impl NewProjectKind {
    fn is_bin(self) -> bool {
        self == NewProjectKind::Bin
    }
}

impl fmt::Display for NewProjectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            NewProjectKind::Bin => "binary (application)",
            NewProjectKind::Lib => "library",
        }
        .fmt(f)
    }
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
    edition: Option<&'a str>,
    registry: Option<&'a str>,
}

impl NewOptions {
    pub fn new(
        version_control: Option<VersionControl>,
        bin: bool,
        lib: bool,
        path: PathBuf,
        name: Option<String>,
        edition: Option<String>,
        registry: Option<String>,
    ) -> CargoResult<NewOptions> {
        let kind = match (bin, lib) {
            (true, true) => failure::bail!("can't specify both lib and binary outputs"),
            (false, true) => NewProjectKind::Lib,
            // default to bin
            (_, false) => NewProjectKind::Bin,
        };

        let opts = NewOptions {
            version_control,
            kind,
            path,
            name,
            edition,
            registry,
        };
        Ok(opts)
    }
}

struct CargoNewConfig {
    name: Option<String>,
    email: Option<String>,
    version_control: Option<VersionControl>,
}

fn get_name<'a>(path: &'a Path, opts: &'a NewOptions) -> CargoResult<&'a str> {
    if let Some(ref name) = opts.name {
        return Ok(name);
    }

    let file_name = path.file_name().ok_or_else(|| {
        failure::format_err!(
            "cannot auto-detect package name from path {:?} ; use --name to override",
            path.as_os_str()
        )
    })?;

    file_name.to_str().ok_or_else(|| {
        failure::format_err!(
            "cannot create package with a non-unicode name: {:?}",
            file_name
        )
    })
}

fn check_name(name: &str, opts: &NewOptions) -> CargoResult<()> {
    // If --name is already used to override, no point in suggesting it
    // again as a fix.
    let name_help = match opts.name {
        Some(_) => "",
        None => "\nuse --name to override crate name",
    };

    // Ban keywords + test list found at
    // https://doc.rust-lang.org/grammar.html#keywords
    let blacklist = [
        "abstract", "alignof", "as", "become", "box", "break", "const", "continue", "crate", "do",
        "else", "enum", "extern", "false", "final", "fn", "for", "if", "impl", "in", "let", "loop",
        "macro", "match", "mod", "move", "mut", "offsetof", "override", "priv", "proc", "pub",
        "pure", "ref", "return", "self", "sizeof", "static", "struct", "super", "test", "trait",
        "true", "type", "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
    ];
    if blacklist.contains(&name) || (opts.kind.is_bin() && compiler::is_bad_artifact_name(name)) {
        failure::bail!(
            "The name `{}` cannot be used as a crate name{}",
            name,
            name_help
        )
    }

    if let Some(ref c) = name.chars().nth(0) {
        if c.is_digit(10) {
            failure::bail!(
                "Package names starting with a digit cannot be used as a crate name{}",
                name_help
            )
        }
    }

    validate_package_name(name, "crate name", name_help)?;
    Ok(())
}

fn detect_source_paths_and_types(
    package_path: &Path,
    package_name: &str,
    detected_files: &mut Vec<SourceFileInformation>,
) -> CargoResult<()> {
    let path = package_path;
    let name = package_name;

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
        Test {
            proposed_path: "src/main.rs".to_string(),
            handling: H::Bin,
        },
        Test {
            proposed_path: "main.rs".to_string(),
            handling: H::Bin,
        },
        Test {
            proposed_path: format!("src/{}.rs", name),
            handling: H::Detect,
        },
        Test {
            proposed_path: format!("{}.rs", name),
            handling: H::Detect,
        },
        Test {
            proposed_path: "src/lib.rs".to_string(),
            handling: H::Lib,
        },
        Test {
            proposed_path: "lib.rs".to_string(),
            handling: H::Lib,
        },
    ];

    for i in tests {
        let pp = i.proposed_path;

        // path/pp does not exist or is not a file
        if !fs::metadata(&path.join(&pp))
            .map(|x| x.is_file())
            .unwrap_or(false)
        {
            continue;
        }

        let sfi = match i.handling {
            H::Bin => SourceFileInformation {
                relative_path: pp,
                target_name: package_name.to_string(),
                bin: true,
            },
            H::Lib => SourceFileInformation {
                relative_path: pp,
                target_name: package_name.to_string(),
                bin: false,
            },
            H::Detect => {
                let content = paths::read(&path.join(pp.clone()))?;
                let isbin = content.contains("fn main");
                SourceFileInformation {
                    relative_path: pp,
                    target_name: package_name.to_string(),
                    bin: isbin,
                }
            }
        };
        detected_files.push(sfi);
    }

    // Check for duplicate lib attempt

    let mut previous_lib_relpath: Option<&str> = None;
    let mut duplicates_checker: BTreeMap<&str, &SourceFileInformation> = BTreeMap::new();

    for i in detected_files {
        if i.bin {
            if let Some(x) = BTreeMap::get::<str>(&duplicates_checker, i.target_name.as_ref()) {
                failure::bail!(
                    "\
multiple possible binary sources found:
  {}
  {}
cannot automatically generate Cargo.toml as the main target would be ambiguous",
                    &x.relative_path,
                    &i.relative_path
                );
            }
            duplicates_checker.insert(i.target_name.as_ref(), i);
        } else {
            if let Some(plp) = previous_lib_relpath {
                failure::bail!(
                    "cannot have a package with \
                     multiple libraries, \
                     found both `{}` and `{}`",
                    plp,
                    i.relative_path
                )
            }
            previous_lib_relpath = Some(&i.relative_path);
        }
    }

    Ok(())
}

fn plan_new_source_file(bin: bool, package_name: String) -> SourceFileInformation {
    if bin {
        SourceFileInformation {
            relative_path: "src/main.rs".to_string(),
            target_name: package_name,
            bin: true,
        }
    } else {
        SourceFileInformation {
            relative_path: "src/lib.rs".to_string(),
            target_name: package_name,
            bin: false,
        }
    }
}

pub fn new(opts: &NewOptions, config: &Config) -> CargoResult<()> {
    let path = &opts.path;
    if fs::metadata(path).is_ok() {
        failure::bail!(
            "destination `{}` already exists\n\n\
             Use `cargo init` to initialize the directory",
            path.display()
        )
    }

    let name = get_name(path, opts)?;
    check_name(name, opts)?;

    let mkopts = MkOptions {
        version_control: opts.version_control,
        path,
        name,
        source_files: vec![plan_new_source_file(opts.kind.is_bin(), name.to_string())],
        bin: opts.kind.is_bin(),
        edition: opts.edition.as_ref().map(|s| &**s),
        registry: opts.registry.as_ref().map(|s| &**s),
    };

    mk(config, &mkopts).chain_err(|| {
        failure::format_err!(
            "Failed to create package `{}` at `{}`",
            name,
            path.display()
        )
    })?;
    Ok(())
}

pub fn init(opts: &NewOptions, config: &Config) -> CargoResult<()> {
    let path = &opts.path;

    if fs::metadata(&path.join("Cargo.toml")).is_ok() {
        failure::bail!("`cargo init` cannot be run on existing Cargo packages")
    }

    let name = get_name(path, opts)?;
    check_name(name, opts)?;

    let mut src_paths_types = vec![];

    detect_source_paths_and_types(path, name, &mut src_paths_types)?;

    if src_paths_types.is_empty() {
        src_paths_types.push(plan_new_source_file(opts.kind.is_bin(), name.to_string()));
    } else {
        // --bin option may be ignored if lib.rs or src/lib.rs present
        // Maybe when doing `cargo init --bin` inside a library package stub,
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
            failure::bail!(
                "more than one of .hg, .git, .pijul, .fossil configurations \
                 found and the ignore file can't be filled in as \
                 a result. specify --vcs to override detection"
            );
        }
    }

    let mkopts = MkOptions {
        version_control,
        path,
        name,
        bin: src_paths_types.iter().any(|x| x.bin),
        source_files: src_paths_types,
        edition: opts.edition.as_ref().map(|s| &**s),
        registry: opts.registry.as_ref().map(|s| &**s),
    };

    mk(config, &mkopts).chain_err(|| {
        failure::format_err!(
            "Failed to create package `{}` at `{}`",
            name,
            path.display()
        )
    })?;
    Ok(())
}

/// IgnoreList
struct IgnoreList {
    /// git like formatted entries
    ignore: Vec<String>,
    /// mercurial formatted entries
    hg_ignore: Vec<String>,
}

impl IgnoreList {
    /// constructor to build a new ignore file
    fn new() -> IgnoreList {
        IgnoreList {
            ignore: Vec::new(),
            hg_ignore: Vec::new(),
        }
    }

    /// add a new entry to the ignore list. Requires two arguments with the
    /// entry in two different formats. One for "git style" entries and one for
    /// "mercurial like" entries.
    fn push(&mut self, ignore: &str, hg_ignore: &str) {
        self.ignore.push(ignore.to_string());
        self.hg_ignore.push(hg_ignore.to_string());
    }

    /// Return the correctly formatted content of the ignore file for the given
    /// version control system as `String`.
    fn format_new(&self, vcs: VersionControl) -> String {
        let ignore_items = match vcs {
            VersionControl::Hg => &self.hg_ignore,
            _ => &self.ignore,
        };

        ignore_items.join("\n") + "\n"
    }

    /// format_existing is used to format the IgnoreList when the ignore file
    /// already exists. It reads the contents of the given `BufRead` and
    /// checks if the contents of the ignore list are already existing in the
    /// file.
    fn format_existing<T: BufRead>(&self, existing: T, vcs: VersionControl) -> String {
        // TODO: is unwrap safe?
        let existing_items = existing.lines().collect::<Result<Vec<_>, _>>().unwrap();

        let ignore_items = match vcs {
            VersionControl::Hg => &self.hg_ignore,
            _ => &self.ignore,
        };

        let mut out = "\n\n#Added by cargo\n\
                       #\n\
                       #already existing elements are commented out\n\n"
            .to_string();

        for item in ignore_items {
            if existing_items.contains(item) {
                out.push('#');
            }
            out.push_str(item);
            out.push('\n');
        }

        out
    }
}

/// Writes the ignore file to the given directory. If the ignore file for the
/// given vcs system already exists, its content is read and duplicate ignore
/// file entries are filtered out.
fn write_ignore_file(
    base_path: &Path,
    list: &IgnoreList,
    vcs: VersionControl,
) -> CargoResult<String> {
    let fp_ignore = match vcs {
        VersionControl::Git => base_path.join(".gitignore"),
        VersionControl::Hg => base_path.join(".hgignore"),
        VersionControl::Pijul => base_path.join(".ignore"),
        VersionControl::Fossil => return Ok("".to_string()),
        VersionControl::NoVcs => return Ok("".to_string()),
    };

    let ignore: String = match fs::File::open(&fp_ignore) {
        Err(why) => match why.kind() {
            ErrorKind::NotFound => list.format_new(vcs),
            _ => return Err(failure::format_err!("{}", why)),
        },
        Ok(file) => list.format_existing(BufReader::new(file), vcs),
    };

    paths::append(&fp_ignore, ignore.as_bytes())?;

    Ok(ignore)
}

/// Initializes the correct VCS system based on the provided config.
fn init_vcs(path: &Path, vcs: VersionControl, config: &Config) -> CargoResult<()> {
    match vcs {
        VersionControl::Git => {
            if !path.join(".git").exists() {
                // Temporary fix to work around bug in libgit2 when creating a
                // directory in the root of a posix filesystem.
                // See: https://github.com/libgit2/libgit2/issues/5130
                fs::create_dir_all(path)?;
                GitRepo::init(path, config.cwd())?;
            }
        }
        VersionControl::Hg => {
            if !path.join(".hg").exists() {
                HgRepo::init(path, config.cwd())?;
            }
        }
        VersionControl::Pijul => {
            if !path.join(".pijul").exists() {
                PijulRepo::init(path, config.cwd())?;
            }
        }
        VersionControl::Fossil => {
            if !path.join(".fossil").exists() {
                FossilRepo::init(path, config.cwd())?;
            }
        }
        VersionControl::NoVcs => {
            fs::create_dir_all(path)?;
        }
    };

    Ok(())
}

fn mk(config: &Config, opts: &MkOptions<'_>) -> CargoResult<()> {
    let path = opts.path;
    let name = opts.name;
    let cfg = global_config(config)?;

    // Using the push method with two arguments ensures that the entries for
    // both `ignore` and `hgignore` are in sync.
    let mut ignore = IgnoreList::new();
    ignore.push("/target", "^target/");
    ignore.push("**/*.rs.bk", "glob:*.rs.bk");
    if !opts.bin {
        ignore.push("Cargo.lock", "glob:Cargo.lock");
    }

    let vcs = opts.version_control.unwrap_or_else(|| {
        let in_existing_vcs = existing_vcs_repo(path.parent().unwrap_or(path), config.cwd());
        match (cfg.version_control, in_existing_vcs) {
            (None, false) => VersionControl::Git,
            (Some(opt), false) => opt,
            (_, true) => VersionControl::NoVcs,
        }
    });

    init_vcs(path, vcs, config)?;
    write_ignore_file(path, &ignore, vcs)?;

    let (author_name, email) = discover_author()?;
    let author = match (cfg.name, cfg.email, author_name, email) {
        (Some(name), Some(email), _, _)
        | (Some(name), None, _, Some(email))
        | (None, Some(email), name, _)
        | (None, None, name, Some(email)) => {
            if email.is_empty() {
                name
            } else {
                format!("{} <{}>", name, email)
            }
        }
        (Some(name), None, _, None) | (None, None, name, None) => name,
    };

    let mut cargotoml_path_specifier = String::new();

    // Calculate what `[lib]` and `[[bin]]`s we need to append to `Cargo.toml`.

    for i in &opts.source_files {
        if i.bin {
            if i.relative_path != "src/main.rs" {
                cargotoml_path_specifier.push_str(&format!(
                    r#"
[[bin]]
name = "{}"
path = {}
"#,
                    i.target_name,
                    toml::Value::String(i.relative_path.clone())
                ));
            }
        } else if i.relative_path != "src/lib.rs" {
            cargotoml_path_specifier.push_str(&format!(
                r#"
[lib]
name = "{}"
path = {}
"#,
                i.target_name,
                toml::Value::String(i.relative_path.clone())
            ));
        }
    }

    // Create `Cargo.toml` file with necessary `[lib]` and `[[bin]]` sections, if needed.

    paths::write(
        &path.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{}"
version = "0.1.0"
authors = [{}]
edition = {}
{}
# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[dependencies]
{}"#,
            name,
            toml::Value::String(author),
            match opts.edition {
                Some(edition) => toml::Value::String(edition.to_string()),
                None => toml::Value::String("2018".to_string()),
            },
            match opts.registry {
                Some(registry) => format!(
                    "publish = {}\n",
                    toml::Value::Array(vec!(toml::Value::String(registry.to_string())))
                ),
                None => "".to_string(),
            },
            cargotoml_path_specifier
        )
        .as_bytes(),
    )?;

    // Create all specified source files (with respective parent directories) if they don't exist.

    for i in &opts.source_files {
        let path_of_source_file = path.join(i.relative_path.clone());

        if let Some(src_dir) = path_of_source_file.parent() {
            fs::create_dir_all(src_dir)?;
        }

        let default_file_content: &[u8] = if i.bin {
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

        if !fs::metadata(&path_of_source_file)
            .map(|x| x.is_file())
            .unwrap_or(false)
        {
            paths::write(&path_of_source_file, default_file_content)?;
        }
    }

    if let Err(e) = Workspace::new(&path.join("Cargo.toml"), config) {
        let msg = format!(
            "compiling this new crate may not work due to invalid \
             workspace configuration\n\n{}",
            errors::display_causes(&e)
        );
        config.shell().warn(msg)?;
    }

    Ok(())
}

fn get_environment_variable(variables: &[&str]) -> Option<String> {
    variables.iter().filter_map(|var| env::var(var).ok()).next()
}

fn discover_author() -> CargoResult<(String, Option<String>)> {
    let cwd = env::current_dir()?;
    let git_config = if let Ok(repo) = GitRepository::discover(&cwd) {
        repo.config()
            .ok()
            .or_else(|| GitConfig::open_default().ok())
    } else {
        GitConfig::open_default().ok()
    };
    let git_config = git_config.as_ref();
    let name_variables = [
        "CARGO_NAME",
        "GIT_AUTHOR_NAME",
        "GIT_COMMITTER_NAME",
        "USER",
        "USERNAME",
        "NAME",
    ];
    let name = get_environment_variable(&name_variables[0..3])
        .or_else(|| git_config.and_then(|g| g.get_string("user.name").ok()))
        .or_else(|| get_environment_variable(&name_variables[3..]));

    let name = match name {
        Some(name) => name,
        None => {
            let username_var = if cfg!(windows) { "USERNAME" } else { "USER" };
            failure::bail!(
                "could not determine the current user, please set ${}",
                username_var
            )
        }
    };
    let email_variables = [
        "CARGO_EMAIL",
        "GIT_AUTHOR_EMAIL",
        "GIT_COMMITTER_EMAIL",
        "EMAIL",
    ];
    let email = get_environment_variable(&email_variables[0..3])
        .or_else(|| git_config.and_then(|g| g.get_string("user.email").ok()))
        .or_else(|| get_environment_variable(&email_variables[3..]));

    let name = name.trim().to_string();
    let email = email.map(|s| {
        let mut s = s.trim();

        // In some cases emails will already have <> remove them since they
        // are already added when needed.
        if s.starts_with('<') && s.ends_with('>') {
            s = &s[1..s.len() - 1];
        }

        s.to_string()
    });

    Ok((name, email))
}

fn global_config(config: &Config) -> CargoResult<CargoNewConfig> {
    let name = config.get_string("cargo-new.name")?.map(|s| s.val);
    let email = config.get_string("cargo-new.email")?.map(|s| s.val);
    let vcs = config.get_string("cargo-new.vcs")?;

    let vcs = match vcs.as_ref().map(|p| (&p.val[..], &p.definition)) {
        Some(("git", _)) => Some(VersionControl::Git),
        Some(("hg", _)) => Some(VersionControl::Hg),
        Some(("pijul", _)) => Some(VersionControl::Pijul),
        Some(("none", _)) => Some(VersionControl::NoVcs),
        Some((s, p)) => {
            return Err(internal(format!(
                "invalid configuration for key \
                 `cargo-new.vcs`, unknown vcs `{}` \
                 (found in {})",
                s, p
            )));
        }
        None => None,
    };
    Ok(CargoNewConfig {
        name,
        email,
        version_control: vcs,
    })
}
