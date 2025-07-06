use crate::core::{Edition, Shell, Workspace};
use crate::util::errors::CargoResult;
use crate::util::important_paths::find_root_manifest_for_wd;
use crate::util::{FossilRepo, GitRepo, HgRepo, PijulRepo, existing_vcs_repo};
use crate::util::{GlobalContext, restricted_names};
use anyhow::{Context as _, anyhow};
use cargo_util::paths::{self, write_atomic};
use cargo_util_schemas::manifest::PackageName;
use serde::Deserialize;
use serde::de;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt, slice};
use toml_edit::{Array, Value};

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
    bin: bool,
}

struct MkOptions<'a> {
    version_control: Option<VersionControl>,
    path: &'a Path,
    name: &'a str,
    source_files: Vec<SourceFileInformation>,
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
#[serde(rename_all = "kebab-case")]
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

/// See also `util::toml::embedded::sanitize_name`
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
        if has_bin && !name.is_empty() {
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
    PackageName::new(name).map_err(|err| {
        let help = bin_help();
        anyhow::anyhow!("{err}{help}")
    })?;

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
            Non-ASCII crate names are not supported by Rust.",
            name
        ))?;
    }
    let name_in_lowercase = name.to_lowercase();
    if name != name_in_lowercase {
        shell.warn(format!(
            "the name `{name}` is not snake_case or kebab-case which is recommended for package names, consider `{name_in_lowercase}`"
        ))?;
    }

    Ok(())
}

/// Checks if the path contains any invalid PATH env characters.
fn check_path(path: &Path, shell: &mut Shell) -> CargoResult<()> {
    // warn if the path contains characters that will break `env::join_paths`
    if let Err(_) = paths::join_paths(slice::from_ref(&OsStr::new(path)), "") {
        let path = path.to_string_lossy();
        shell.warn(format!(
            "the path `{path}` contains invalid PATH characters (usually `:`, `;`, or `\"`)\n\
            It is recommended to use a different name to avoid problems."
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
                bin: true,
            },
            H::Lib => SourceFileInformation {
                relative_path: pp,
                bin: false,
            },
            H::Detect => {
                let content = paths::read(&path.join(pp.clone()))?;
                let isbin = content.contains("fn main");
                SourceFileInformation {
                    relative_path: pp,
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
            if let Some(x) = BTreeMap::get::<str>(&duplicates_checker, &name) {
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
            duplicates_checker.insert(name, i);
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

fn plan_new_source_file(bin: bool) -> SourceFileInformation {
    if bin {
        SourceFileInformation {
            relative_path: "src/main.rs".to_string(),
            bin: true,
        }
    } else {
        SourceFileInformation {
            relative_path: "src/lib.rs".to_string(),
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

pub fn new(opts: &NewOptions, gctx: &GlobalContext) -> CargoResult<()> {
    let path = &opts.path;
    let name = get_name(path, opts)?;
    gctx.shell()
        .status("Creating", format!("{} `{}` package", opts.kind, name))?;

    if path.exists() {
        anyhow::bail!(
            "destination `{}` already exists\n\n\
             Use `cargo init` to initialize the directory",
            path.display()
        )
    }
    check_path(path, &mut gctx.shell())?;

    let is_bin = opts.kind.is_bin();

    check_name(name, opts.name.is_none(), is_bin, &mut gctx.shell())?;

    let mkopts = MkOptions {
        version_control: opts.version_control,
        path,
        name,
        source_files: vec![plan_new_source_file(opts.kind.is_bin())],
        edition: opts.edition.as_deref(),
        registry: opts.registry.as_deref(),
    };

    mk(gctx, &mkopts).with_context(|| {
        format!(
            "Failed to create package `{}` at `{}`",
            name,
            path.display()
        )
    })?;
    Ok(())
}

pub fn init(opts: &NewOptions, gctx: &GlobalContext) -> CargoResult<NewProjectKind> {
    // This is here just as a random location to exercise the internal error handling.
    if gctx.get_env_os("__CARGO_TEST_INTERNAL_ERROR").is_some() {
        return Err(crate::util::internal("internal error test"));
    }

    let path = &opts.path;
    let name = get_name(path, opts)?;
    let mut src_paths_types = vec![];
    detect_source_paths_and_types(path, name, &mut src_paths_types)?;
    let kind = calculate_new_project_kind(opts.kind, opts.auto_detect_kind, &src_paths_types);
    gctx.shell()
        .status("Creating", format!("{} package", opts.kind))?;

    if path.join("Cargo.toml").exists() {
        anyhow::bail!("`cargo init` cannot be run on existing Cargo packages")
    }
    check_path(path, &mut gctx.shell())?;

    let has_bin = kind.is_bin();

    if src_paths_types.is_empty() {
        src_paths_types.push(plan_new_source_file(has_bin));
    } else if src_paths_types.len() == 1 && !src_paths_types.iter().any(|x| x.bin == has_bin) {
        // we've found the only file and it's not the type user wants. Change the type and warn
        let file_type = if src_paths_types[0].bin {
            NewProjectKind::Bin
        } else {
            NewProjectKind::Lib
        };
        gctx.shell().warn(format!(
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

    check_name(name, opts.name.is_none(), has_bin, &mut gctx.shell())?;

    let mut version_control = opts.version_control;

    if version_control == None {
        let mut num_detected_vcses = 0;

        if path.join(".git").exists() {
            version_control = Some(VersionControl::Git);
            num_detected_vcses += 1;
        }

        if path.join(".hg").exists() {
            version_control = Some(VersionControl::Hg);
            num_detected_vcses += 1;
        }

        if path.join(".pijul").exists() {
            version_control = Some(VersionControl::Pijul);
            num_detected_vcses += 1;
        }

        if path.join(".fossil").exists() {
            version_control = Some(VersionControl::Fossil);
            num_detected_vcses += 1;
        }

        // if none exists, maybe create git, like in `cargo new`

        if num_detected_vcses > 1 {
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
        source_files: src_paths_types,
        edition: opts.edition.as_deref(),
        registry: opts.registry.as_deref(),
    };

    mk(gctx, &mkopts).with_context(|| {
        format!(
            "Failed to create package `{}` at `{}`",
            name,
            path.display()
        )
    })?;
    Ok(kind)
}

/// `IgnoreList`
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

    /// `format_existing` is used to format the `IgnoreList` when the ignore file
    /// already exists. It reads the contents of the given `BufRead` and
    /// checks if the contents of the ignore list are already existing in the
    /// file.
    fn format_existing<T: BufRead>(&self, existing: T, vcs: VersionControl) -> CargoResult<String> {
        let mut existing_items = Vec::new();
        for (i, item) in existing.lines().enumerate() {
            match item {
                Ok(s) => existing_items.push(s),
                Err(err) => match err.kind() {
                    ErrorKind::InvalidData => {
                        return Err(anyhow!(
                            "Character at line {} is invalid. Cargo only supports UTF-8.",
                            i
                        ));
                    }
                    _ => return Err(anyhow!(err)),
                },
            }
        }

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

        Ok(out)
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
            Ok(file) => list.format_existing(BufReader::new(file), vcs)?,
        };

        paths::append(&fp_ignore, ignore.as_bytes())?;
    }

    Ok(())
}

/// Initializes the correct VCS system based on the provided config.
fn init_vcs(path: &Path, vcs: VersionControl, gctx: &GlobalContext) -> CargoResult<()> {
    match vcs {
        VersionControl::Git => {
            if !path.join(".git").exists() {
                // Temporary fix to work around bug in libgit2 when creating a
                // directory in the root of a posix filesystem.
                // See: https://github.com/libgit2/libgit2/issues/5130
                paths::create_dir_all(path)?;
                GitRepo::init(path, gctx.cwd())?;
            }
        }
        VersionControl::Hg => {
            if !path.join(".hg").exists() {
                HgRepo::init(path, gctx.cwd())?;
            }
        }
        VersionControl::Pijul => {
            if !path.join(".pijul").exists() {
                PijulRepo::init(path, gctx.cwd())?;
            }
        }
        VersionControl::Fossil => {
            if !path.join(".fossil").exists() {
                FossilRepo::init(path, gctx.cwd())?;
            }
        }
        VersionControl::NoVcs => {
            paths::create_dir_all(path)?;
        }
    };

    Ok(())
}

fn mk(gctx: &GlobalContext, opts: &MkOptions<'_>) -> CargoResult<()> {
    let path = opts.path;
    let name = opts.name;
    let cfg = gctx.get::<CargoNewConfig>("cargo-new")?;

    // Using the push method with multiple arguments ensures that the entries
    // for all mutually-incompatible VCS in terms of syntax are in sync.
    let mut ignore = IgnoreList::new();
    ignore.push("/target", "^target$", "target");

    let vcs = opts.version_control.unwrap_or_else(|| {
        let in_existing_vcs = existing_vcs_repo(path.parent().unwrap_or(path), gctx.cwd());
        match (cfg.version_control, in_existing_vcs) {
            (None, false) => VersionControl::Git,
            (Some(opt), false) => opt,
            (_, true) => VersionControl::NoVcs,
        }
    });

    init_vcs(path, vcs, gctx)?;
    write_ignore_file(path, &ignore, vcs)?;

    // Create `Cargo.toml` file with necessary `[lib]` and `[[bin]]` sections, if needed.
    let mut manifest = toml_edit::DocumentMut::new();
    manifest["package"] = toml_edit::Item::Table(toml_edit::Table::new());
    manifest["package"]["name"] = toml_edit::value(name);
    manifest["package"]["version"] = toml_edit::value("0.1.0");
    let edition = match opts.edition {
        Some(edition) => edition.to_string(),
        None => Edition::LATEST_STABLE.to_string(),
    };
    manifest["package"]["edition"] = toml_edit::value(edition);
    if let Some(registry) = opts.registry {
        let mut array = toml_edit::Array::default();
        array.push(registry);
        manifest["package"]["publish"] = toml_edit::value(array);
    }
    let dep_table = toml_edit::Table::default();
    manifest["dependencies"] = toml_edit::Item::Table(dep_table);

    // Calculate what `[lib]` and `[[bin]]`s we need to append to `Cargo.toml`.
    for i in &opts.source_files {
        if i.bin {
            if i.relative_path != "src/main.rs" {
                let mut bin = toml_edit::Table::new();
                bin["name"] = toml_edit::value(name);
                bin["path"] = toml_edit::value(i.relative_path.clone());
                manifest["bin"]
                    .or_insert(toml_edit::Item::ArrayOfTables(
                        toml_edit::ArrayOfTables::new(),
                    ))
                    .as_array_of_tables_mut()
                    .expect("bin is an array of tables")
                    .push(bin);
            }
        } else if i.relative_path != "src/lib.rs" {
            let mut lib = toml_edit::Table::new();
            lib["path"] = toml_edit::value(i.relative_path.clone());
            manifest["lib"] = toml_edit::Item::Table(lib);
        }
    }

    let manifest_path = paths::normalize_path(&path.join("Cargo.toml"));
    if let Ok(root_manifest_path) = find_root_manifest_for_wd(&manifest_path) {
        let root_manifest = paths::read(&root_manifest_path)?;
        // Sometimes the root manifest is not a valid manifest, so we only try to parse it if it is.
        // This should not block the creation of the new project. It is only a best effort to
        // inherit the workspace package keys.
        if let Ok(mut workspace_document) = root_manifest.parse::<toml_edit::DocumentMut>() {
            let display_path = get_display_path(&root_manifest_path, &path)?;
            let can_be_a_member = can_be_workspace_member(&display_path, &workspace_document)?;
            // Only try to inherit the workspace stuff if the new package can be a member of the workspace.
            if can_be_a_member {
                if let Some(workspace_package_keys) = workspace_document
                    .get("workspace")
                    .and_then(|workspace| workspace.get("package"))
                    .and_then(|package| package.as_table())
                {
                    update_manifest_with_inherited_workspace_package_keys(
                        opts,
                        &mut manifest,
                        workspace_package_keys,
                    )
                }
                // Try to inherit the workspace lints key if it exists.
                if workspace_document
                    .get("workspace")
                    .and_then(|workspace| workspace.get("lints"))
                    .is_some()
                {
                    let mut table = toml_edit::Table::new();
                    table["workspace"] = toml_edit::value(true);
                    manifest["lints"] = toml_edit::Item::Table(table);
                }

                // Try to add the new package to the workspace members.
                if update_manifest_with_new_member(
                    &root_manifest_path,
                    &mut workspace_document,
                    &display_path,
                )? {
                    gctx.shell().status(
                        "Adding",
                        format!(
                            "`{}` as member of workspace at `{}`",
                            PathBuf::from(&display_path)
                                .file_name()
                                .unwrap()
                                .to_str()
                                .unwrap(),
                            root_manifest_path.parent().unwrap().display()
                        ),
                    )?
                }
            }
        }
    }

    paths::write(&manifest_path, manifest.to_string())?;

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
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
"
        };

        if !path_of_source_file.is_file() {
            paths::write(&path_of_source_file, default_file_content)?;

            // Format the newly created source file
            if let Err(e) = cargo_util::ProcessBuilder::new("rustfmt")
                .arg(&path_of_source_file)
                .exec_with_output()
            {
                tracing::warn!("failed to call rustfmt: {:#}", e);
            }
        }
    }

    if let Err(e) = Workspace::new(&manifest_path, gctx) {
        crate::display_warning_with_error(
            "compiling this new package may not work due to invalid \
             workspace configuration",
            &e,
            &mut gctx.shell(),
        );
    }

    gctx.shell().note(
        "see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html",
    )?;

    Ok(())
}

// Update the manifest with the inherited workspace package keys.
// If the option is not set, the key is removed from the manifest.
// If the option is set, keep the value from the manifest.
fn update_manifest_with_inherited_workspace_package_keys(
    opts: &MkOptions<'_>,
    manifest: &mut toml_edit::DocumentMut,
    workspace_package_keys: &toml_edit::Table,
) {
    if workspace_package_keys.is_empty() {
        return;
    }

    let try_remove_and_inherit_package_key = |key: &str, manifest: &mut toml_edit::DocumentMut| {
        let package = manifest["package"]
            .as_table_mut()
            .expect("package is a table");
        package.remove(key);
        let mut table = toml_edit::Table::new();
        table.set_dotted(true);
        table["workspace"] = toml_edit::value(true);
        package.insert(key, toml_edit::Item::Table(table));
    };

    // Inherit keys from the workspace.
    // Only keep the value from the manifest if the option is set.
    for (key, _) in workspace_package_keys {
        if key == "edition" && opts.edition.is_some() {
            continue;
        }
        if key == "publish" && opts.registry.is_some() {
            continue;
        }

        try_remove_and_inherit_package_key(key, manifest);
    }
}

/// Adds the new package member to the [workspace.members] array.
/// - It first checks if the name matches any element in [workspace.exclude],
///  and it ignores the name if there is a match.
/// - Then it check if the name matches any element already in [workspace.members],
/// and it ignores the name if there is a match.
/// - If [workspace.members] doesn't exist in the manifest, it will add a new section
/// with the new package in it.
fn update_manifest_with_new_member(
    root_manifest_path: &Path,
    workspace_document: &mut toml_edit::DocumentMut,
    display_path: &str,
) -> CargoResult<bool> {
    let Some(workspace) = workspace_document.get_mut("workspace") else {
        return Ok(false);
    };

    // If the members element already exist, check if one of the patterns
    // in the array already includes the new package's relative path.
    // - Add the relative path if the members don't match the new package's path.
    // - Create a new members array if there are no members element in the workspace yet.
    if let Some(members) = workspace
        .get_mut("members")
        .and_then(|members| members.as_array_mut())
    {
        for member in members.iter() {
            let pat = member
                .as_str()
                .with_context(|| format!("invalid non-string member `{}`", member))?;
            let pattern = glob::Pattern::new(pat)
                .with_context(|| format!("cannot build glob pattern from `{}`", pat))?;

            if pattern.matches(&display_path) {
                return Ok(false);
            }
        }

        let was_sorted = members.iter().map(Value::as_str).is_sorted();
        members.push(display_path);
        if was_sorted {
            members.sort_by(|lhs, rhs| lhs.as_str().cmp(&rhs.as_str()));
        }
    } else {
        let mut array = Array::new();
        array.push(display_path);

        workspace["members"] = toml_edit::value(array);
    }

    write_atomic(
        &root_manifest_path,
        workspace_document.to_string().to_string().as_bytes(),
    )?;
    Ok(true)
}

fn get_display_path(root_manifest_path: &Path, package_path: &Path) -> CargoResult<String> {
    // Find the relative path for the package from the workspace root directory.
    let workspace_root = root_manifest_path.parent().with_context(|| {
        format!(
            "workspace root manifest doesn't have a parent directory `{}`",
            root_manifest_path.display()
        )
    })?;
    let relpath = pathdiff::diff_paths(package_path, workspace_root).with_context(|| {
        format!(
            "path comparison requires two absolute paths; package_path: `{}`, workspace_path: `{}`",
            package_path.display(),
            workspace_root.display()
        )
    })?;

    let mut components = Vec::new();
    for comp in relpath.iter() {
        let comp = comp.to_str().with_context(|| {
            format!("invalid unicode component in path `{}`", relpath.display())
        })?;
        components.push(comp);
    }
    let display_path = components.join("/");
    Ok(display_path)
}

// Check if the package can be a member of the workspace.
fn can_be_workspace_member(
    display_path: &str,
    workspace_document: &toml_edit::DocumentMut,
) -> CargoResult<bool> {
    if let Some(exclude) = workspace_document
        .get("workspace")
        .and_then(|workspace| workspace.get("exclude"))
        .and_then(|exclude| exclude.as_array())
    {
        for member in exclude {
            let pat = member
                .as_str()
                .with_context(|| format!("invalid non-string exclude path `{}`", member))?;
            if pat == display_path {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
