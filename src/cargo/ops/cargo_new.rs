use crate::core::{Edition, Shell, Workspace};
use crate::util::errors::CargoResult;
use crate::util::{existing_vcs_repo, FossilRepo, GitRepo, HgRepo, PijulRepo};
use crate::util::{restricted_names, Config};
use anyhow::Context as _;
use cargo_util::paths;
use serde::de;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::{from_utf8, FromStr};
use toml_edit::easy as toml;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VersionControl {
    Git,
    Hg,
    Pijul,
    Fossil,
    NoVcs,
}

impl FromStr for VersionControl {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, anyhow::Error> {
        match s {
            "git" => Ok(VersionControl::Git),
            "hg" => Ok(VersionControl::Hg),
            "pijul" => Ok(VersionControl::Pijul),
            "fossil" => Ok(VersionControl::Fossil),
            "none" => Ok(VersionControl::NoVcs),
            other => anyhow::bail!("unknown vcs specification: `{}`", other),
        }
    }
}

impl<'de> de::Deserialize<'de> for VersionControl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

#[derive(Debug)]
pub struct NewOptions {
    pub version_control: Option<VersionControl>,
    pub kind: NewProjectKind,
    pub auto_detect_kind: bool,
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
        let auto_detect_kind = !bin && !lib;

        let kind = match (bin, lib) {
            (true, true) => anyhow::bail!("can't specify both lib and binary outputs"),
            (false, true) => NewProjectKind::Lib,
            (_, false) => NewProjectKind::Bin,
        };

        let opts = NewOptions {
            version_control,
            kind,
            auto_detect_kind,
            path,
            name,
            edition,
            registry,
        };
        Ok(opts)
    }
}

#[derive(Deserialize)]
struct CargoNewConfig {
    #[deprecated = "cargo-new no longer supports adding the authors field"]
    #[allow(dead_code)]
    name: Option<String>,

    #[deprecated = "cargo-new no longer supports adding the authors field"]
    #[allow(dead_code)]
    email: Option<String>,

    #[serde(rename = "vcs")]
    version_control: Option<VersionControl>,
}

fn get_name<'a>(path: &'a Path, opts: &'a NewOptions) -> CargoResult<&'a str> {
    if let Some(ref name) = opts.name {
        return Ok(name);
    }

    let file_name = path.file_name().ok_or_else(|| {
        anyhow::format_err!(
            "cannot auto-detect package name from path {:?} ; use --name to override",
            path.as_os_str()
        )
    })?;

    file_name.to_str().ok_or_else(|| {
        anyhow::format_err!(
            "cannot create package with a non-unicode name: {:?}",
            file_name
        )
    })
}

fn check_name(
    name: &str,
    show_name_help: bool,
    has_bin: bool,
    shell: &mut Shell,
) -> CargoResult<()> {
    // If --name is already used to override, no point in suggesting it
    // again as a fix.
    let name_help = if show_name_help {
        "\nIf you need a package name to not match the directory name, consider using --name flag."
    } else {
        ""
    };
    let bin_help = || {
        let mut help = String::from(name_help);
        if has_bin {
            help.push_str(&format!(
                "\n\
                If you need a binary with the name \"{name}\", use a valid package \
                name, and set the binary name to be different from the package. \
                This can be done by setting the binary filename to `src/bin/{name}.rs` \
                or change the name in Cargo.toml with:\n\
                \n    \
                [[bin]]\n    \
                name = \"{name}\"\n    \
                path = \"src/main.rs\"\n\
            ",
                name = name
            ));
        }
        help
    };
    restricted_names::validate_package_name(name, "package name", &bin_help())?;

    if restricted_names::is_keyword(name) {
        anyhow::bail!(
            "the name `{}` cannot be used as a package name, it is a Rust keyword{}",
            name,
            bin_help()
        );
    }
    if restricted_names::is_conflicting_artifact_name(name) {
        if has_bin {
            anyhow::bail!(
                "the name `{}` cannot be used as a package name, \
                it conflicts with cargo's build directory names{}",
                name,
                name_help
            );
        } else {
            shell.warn(format!(
                "the name `{}` will not support binary \
                executables with that name, \
                it conflicts with cargo's build directory names",
                name
            ))?;
        }
    }
    if name == "test" {
        anyhow::bail!(
            "the name `test` cannot be used as a package name, \
            it conflicts with Rust's built-in test library{}",
            bin_help()
        );
    }
    if ["core", "std", "alloc", "proc_macro", "proc-macro"].contains(&name) {
        shell.warn(format!(
            "the name `{}` is part of Rust's standard library\n\
            It is recommended to use a different name to avoid problems.{}",
            name,
            bin_help()
        ))?;
    }
    if restricted_names::is_windows_reserved(name) {
        if cfg!(windows) {
            anyhow::bail!(
                "cannot use name `{}`, it is a reserved Windows filename{}",
                name,
                name_help
            );
        } else {
            shell.warn(format!(
                "the name `{}` is a reserved Windows filename\n\
                This package will not work on Windows platforms.",
                name
            ))?;
        }
    }
    if restricted_names::is_non_ascii_name(name) {
        shell.warn(format!(
            "the name `{}` contains non-ASCII characters\n\
            Support for non-ASCII crate names is experimental and only valid \
            on the nightly toolchain.",
            name
        ))?;
    }

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
        if !path.join(&pp).is_file() {
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
                anyhow::bail!(
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
                anyhow::bail!(
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

fn calculate_new_project_kind(
    requested_kind: NewProjectKind,
    auto_detect_kind: bool,
    found_files: &Vec<SourceFileInformation>,
) -> NewProjectKind {
    let bin_file = found_files.iter().find(|x| x.bin);

    let kind_from_files = if !found_files.is_empty() && bin_file.is_none() {
        NewProjectKind::Lib
    } else {
        NewProjectKind::Bin
    };

    if auto_detect_kind {
        return kind_from_files;
    }

    requested_kind
}

pub fn new(opts: &NewOptions, config: &Config) -> CargoResult<()> {
    let path = &opts.path;
    if path.exists() {
        anyhow::bail!(
            "destination `{}` already exists\n\n\
             Use `cargo init` to initialize the directory",
            path.display()
        )
    }

    let is_bin = opts.kind.is_bin();

    let name = get_name(path, opts)?;
    check_name(name, opts.name.is_none(), is_bin, &mut config.shell())?;

    let mkopts = MkOptions {
        version_control: opts.version_control,
        path,
        name,
        source_files: vec![plan_new_source_file(opts.kind.is_bin(), name.to_string())],
        bin: is_bin,
        edition: opts.edition.as_deref(),
        registry: opts.registry.as_deref(),
    };

    mk(config, &mkopts).with_context(|| {
        format!(
            "Failed to create package `{}` at `{}`",
            name,
            path.display()
        )
    })?;
    Ok(())
}

pub fn init(opts: &NewOptions, config: &Config) -> CargoResult<NewProjectKind> {
    // This is here just as a random location to exercise the internal error handling.
    if std::env::var_os("__CARGO_TEST_INTERNAL_ERROR").is_some() {
        return Err(crate::util::internal("internal error test"));
    }

    let path = &opts.path;

    if path.join("Cargo.toml").exists() {
        anyhow::bail!("`cargo init` cannot be run on existing Cargo packages")
    }

    let name = get_name(path, opts)?;

    let mut src_paths_types = vec![];

    detect_source_paths_and_types(path, name, &mut src_paths_types)?;

    let kind = calculate_new_project_kind(opts.kind, opts.auto_detect_kind, &src_paths_types);
    let has_bin = kind.is_bin();

    if src_paths_types.is_empty() {
        src_paths_types.push(plan_new_source_file(has_bin, name.to_string()));
    } else if src_paths_types.len() == 1 && !src_paths_types.iter().any(|x| x.bin == has_bin) {
        // we've found the only file and it's not the type user wants. Change the type and warn
        let file_type = if src_paths_types[0].bin {
            NewProjectKind::Bin
        } else {
            NewProjectKind::Lib
        };
        config.shell().warn(format!(
            "file `{}` seems to be a {} file",
            src_paths_types[0].relative_path, file_type
        ))?;
        src_paths_types[0].bin = has_bin
    } else if src_paths_types.len() > 1 && !has_bin {
        // We have found both lib and bin files and the user would like us to treat both as libs
        anyhow::bail!(
            "cannot have a package with \
             multiple libraries, \
             found both `{}` and `{}`",
            src_paths_types[0].relative_path,
            src_paths_types[1].relative_path
        )
    }

    check_name(name, opts.name.is_none(), has_bin, &mut config.shell())?;

    let mut version_control = opts.version_control;

    if version_control == None {
        let mut num_detected_vsces = 0;

        if path.join(".git").exists() {
            version_control = Some(VersionControl::Git);
            num_detected_vsces += 1;
        }

        if path.join(".hg").exists() {
            version_control = Some(VersionControl::Hg);
            num_detected_vsces += 1;
        }

        if path.join(".pijul").exists() {
            version_control = Some(VersionControl::Pijul);
            num_detected_vsces += 1;
        }

        if path.join(".fossil").exists() {
            version_control = Some(VersionControl::Fossil);
            num_detected_vsces += 1;
        }

        // if none exists, maybe create git, like in `cargo new`

        if num_detected_vsces > 1 {
            anyhow::bail!(
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
        bin: has_bin,
        source_files: src_paths_types,
        edition: opts.edition.as_deref(),
        registry: opts.registry.as_deref(),
    };

    mk(config, &mkopts).with_context(|| {
        format!(
            "Failed to create package `{}` at `{}`",
            name,
            path.display()
        )
    })?;
    Ok(kind)
}

/// IgnoreList
struct IgnoreList {
    /// git like formatted entries
    ignore: Vec<String>,
    /// mercurial formatted entries
    hg_ignore: Vec<String>,
    /// Fossil-formatted entries.
    fossil_ignore: Vec<String>,
}

impl IgnoreList {
    /// constructor to build a new ignore file
    fn new() -> IgnoreList {
        IgnoreList {
            ignore: Vec::new(),
            hg_ignore: Vec::new(),
            fossil_ignore: Vec::new(),
        }
    }

    /// Add a new entry to the ignore list. Requires three arguments with the
    /// entry in possibly three different formats. One for "git style" entries,
    /// one for "mercurial style" entries and one for "fossil style" entries.
    fn push(&mut self, ignore: &str, hg_ignore: &str, fossil_ignore: &str) {
        self.ignore.push(ignore.to_string());
        self.hg_ignore.push(hg_ignore.to_string());
        self.fossil_ignore.push(fossil_ignore.to_string());
    }

    /// Return the correctly formatted content of the ignore file for the given
    /// version control system as `String`.
    fn format_new(&self, vcs: VersionControl) -> String {
        let ignore_items = match vcs {
            VersionControl::Hg => &self.hg_ignore,
            VersionControl::Fossil => &self.fossil_ignore,
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
            VersionControl::Fossil => &self.fossil_ignore,
            _ => &self.ignore,
        };

        let mut out = String::new();

        // Fossil does not support `#` comments.
        if vcs != VersionControl::Fossil {
            out.push_str("\n\n# Added by cargo\n");
            if ignore_items
                .iter()
                .any(|item| existing_items.contains(item))
            {
                out.push_str("#\n# already existing elements were commented out\n");
            }
            out.push('\n');
        }

        for item in ignore_items {
            if existing_items.contains(item) {
                if vcs == VersionControl::Fossil {
                    // Just merge for Fossil.
                    continue;
                }
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
fn write_ignore_file(base_path: &Path, list: &IgnoreList, vcs: VersionControl) -> CargoResult<()> {
    // Fossil only supports project-level settings in a dedicated subdirectory.
    if vcs == VersionControl::Fossil {
        paths::create_dir_all(base_path.join(".fossil-settings"))?;
    }

    for fp_ignore in match vcs {
        VersionControl::Git => vec![base_path.join(".gitignore")],
        VersionControl::Hg => vec![base_path.join(".hgignore")],
        VersionControl::Pijul => vec![base_path.join(".ignore")],
        // Fossil has a cleaning functionality configured in a separate file.
        VersionControl::Fossil => vec![
            base_path.join(".fossil-settings/ignore-glob"),
            base_path.join(".fossil-settings/clean-glob"),
        ],
        VersionControl::NoVcs => return Ok(()),
    } {
        let ignore: String = match paths::open(&fp_ignore) {
            Err(err) => match err.downcast_ref::<std::io::Error>() {
                Some(io_err) if io_err.kind() == ErrorKind::NotFound => list.format_new(vcs),
                _ => return Err(err),
            },
            Ok(file) => list.format_existing(BufReader::new(file), vcs),
        };

        paths::append(&fp_ignore, ignore.as_bytes())?;
    }

    Ok(())
}

/// Initializes the correct VCS system based on the provided config.
fn init_vcs(path: &Path, vcs: VersionControl, config: &Config) -> CargoResult<()> {
    match vcs {
        VersionControl::Git => {
            if !path.join(".git").exists() {
                // Temporary fix to work around bug in libgit2 when creating a
                // directory in the root of a posix filesystem.
                // See: https://github.com/libgit2/libgit2/issues/5130
                paths::create_dir_all(path)?;
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
            paths::create_dir_all(path)?;
        }
    };

    Ok(())
}

fn mk(config: &Config, opts: &MkOptions<'_>) -> CargoResult<()> {
    let path = opts.path;
    let name = opts.name;
    let cfg = config.get::<CargoNewConfig>("cargo-new")?;

    // Using the push method with multiple arguments ensures that the entries
    // for all mutually-incompatible VCS in terms of syntax are in sync.
    let mut ignore = IgnoreList::new();
    ignore.push("/target", "^target/", "target");
    if !opts.bin {
        ignore.push("/Cargo.lock", "^Cargo.lock$", "Cargo.lock");
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
edition = {}
{}
# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[dependencies]
{}"#,
            name,
            match opts.edition {
                Some(edition) => toml::Value::String(edition.to_string()),
                None => toml::Value::String(Edition::LATEST_STABLE.to_string()),
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
            paths::create_dir_all(src_dir)?;
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
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
"
        };

        if !path_of_source_file.is_file() {
            paths::write(&path_of_source_file, default_file_content)?;

            // Format the newly created source file
            match Command::new("rustfmt").arg(&path_of_source_file).output() {
                Err(e) => log::warn!("failed to call rustfmt: {}", e),
                Ok(output) => {
                    if !output.status.success() {
                        log::warn!("rustfmt failed: {:?}", from_utf8(&output.stdout));
                    }
                }
            };
        }
    }

    if let Err(e) = Workspace::new(&path.join("Cargo.toml"), config) {
        crate::display_warning_with_error(
            "compiling this new package may not work due to invalid \
             workspace configuration",
            &e,
            &mut config.shell(),
        );
    }

    Ok(())
}
