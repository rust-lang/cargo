use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::fs::File;
use std::io::SeekFrom;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::task::Poll;

use crate::core::PackageIdSpecQuery;
use crate::core::Shell;
use crate::core::Verbosity;
use crate::core::Workspace;
use crate::core::dependency::DepKind;
use crate::core::manifest::Target;
use crate::core::resolver::CliFeatures;
use crate::core::resolver::HasDevUnits;
use crate::core::{Package, PackageId, PackageSet, Resolve, SourceId};
use crate::ops::lockfile::LOCKFILE_NAME;
use crate::ops::registry::{RegistryOrIndex, infer_registry};
use crate::sources::path::PathEntry;
use crate::sources::{CRATES_IO_REGISTRY, PathSource};
use crate::util::FileLock;
use crate::util::Filesystem;
use crate::util::GlobalContext;
use crate::util::Graph;
use crate::util::HumanBytes;
use crate::util::cache_lock::CacheLockMode;
use crate::util::context::JobsConfig;
use crate::util::errors::CargoResult;
use crate::util::errors::ManifestError;
use crate::util::restricted_names;
use crate::util::toml::prepare_for_publish;
use crate::{drop_println, ops};
use annotate_snippets::Level;
use anyhow::{Context as _, bail};
use cargo_util::paths;
use cargo_util_schemas::index::{IndexPackage, RegistryDependency};
use cargo_util_schemas::messages;
use flate2::{Compression, GzBuilder};
use tar::{Builder, EntryType, Header, HeaderMode};
use tracing::debug;
use unicase::Ascii as UncasedAscii;

mod vcs;
mod verify;

/// Message format for `cargo package`.
///
/// Currently only affect the output of the `--list` flag.
#[derive(Debug, Clone)]
pub enum PackageMessageFormat {
    Human,
    Json,
}

impl PackageMessageFormat {
    pub const POSSIBLE_VALUES: [&str; 2] = ["human", "json"];

    pub const DEFAULT: &str = "human";
}

impl std::str::FromStr for PackageMessageFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<PackageMessageFormat, anyhow::Error> {
        match s {
            "human" => Ok(PackageMessageFormat::Human),
            "json" => Ok(PackageMessageFormat::Json),
            f => bail!("unknown message format `{f}`"),
        }
    }
}

#[derive(Clone)]
pub struct PackageOpts<'gctx> {
    pub gctx: &'gctx GlobalContext,
    pub list: bool,
    pub fmt: PackageMessageFormat,
    pub check_metadata: bool,
    pub allow_dirty: bool,
    pub include_lockfile: bool,
    pub verify: bool,
    pub jobs: Option<JobsConfig>,
    pub keep_going: bool,
    pub to_package: ops::Packages,
    pub targets: Vec<String>,
    pub cli_features: CliFeatures,
    pub reg_or_index: Option<ops::RegistryOrIndex>,
    /// Whether this packaging job is meant for a publishing dry-run.
    ///
    /// Packaging on its own has no side effects, so a dry-run doesn't
    /// make sense from that point of view. But dry-run publishing needs
    /// special packaging behavior, which this flag turns on.
    ///
    /// Specifically, we want dry-run packaging to work even if versions
    /// have not yet been bumped. But then if you dry-run packaging in
    /// a workspace with some declared versions that are already published,
    /// the package verification step can fail with checksum mismatches.
    /// So when dry-run is true, the verification step does some extra
    /// checksum fudging in the lock file.
    pub dry_run: bool,
}

const ORIGINAL_MANIFEST_FILE: &str = "Cargo.toml.orig";
const VCS_INFO_FILE: &str = ".cargo_vcs_info.json";

struct ArchiveFile {
    /// The relative path in the archive (not including the top-level package
    /// name directory).
    rel_path: PathBuf,
    /// String variant of `rel_path`, for convenience.
    rel_str: String,
    /// The contents to add to the archive.
    contents: FileContents,
}

enum FileContents {
    /// Absolute path to the file on disk to add to the archive.
    OnDisk(PathBuf),
    /// Generates a file.
    Generated(GeneratedFile),
}

enum GeneratedFile {
    /// Generates `Cargo.toml` by rewriting the original.
    ///
    /// Associated path is the original manifest path.
    Manifest(PathBuf),
    /// Generates `Cargo.lock`.
    ///
    /// Associated path is the path to the original lock file, if existing.
    Lockfile(Option<PathBuf>),
    /// Adds a `.cargo_vcs_info.json` file if in a git repo.
    VcsInfo(vcs::VcsInfo),
}

// Builds a tarball and places it in the output directory.
#[tracing::instrument(skip_all)]
fn create_package(
    ws: &Workspace<'_>,
    opts: &PackageOpts<'_>,
    pkg: &Package,
    ar_files: Vec<ArchiveFile>,
    local_reg: Option<&TmpRegistry<'_>>,
) -> CargoResult<FileLock> {
    let gctx = ws.gctx();
    let filecount = ar_files.len();

    // Check that the package dependencies are safe to deploy.
    for dep in pkg.dependencies() {
        super::check_dep_has_version(dep, false).map_err(|err| {
            ManifestError::new(
                err.context(format!(
                    "failed to verify manifest at `{}`",
                    pkg.manifest_path().display()
                )),
                pkg.manifest_path().into(),
            )
        })?;
    }

    let filename = pkg.package_id().tarball_name();
    let dir = ws.build_dir().join("package");
    let mut dst = {
        let tmp = format!(".{}", filename);
        dir.open_rw_exclusive_create(&tmp, gctx, "package scratch space")?
    };

    // Package up and test a temporary tarball and only move it to the final
    // location if it actually passes all our tests. Any previously existing
    // tarball can be assumed as corrupt or invalid, so we just blow it away if
    // it exists.
    gctx.shell()
        .status("Packaging", pkg.package_id().to_string())?;
    dst.file().set_len(0)?;
    let uncompressed_size = tar(ws, opts, pkg, local_reg, ar_files, dst.file(), &filename)
        .context("failed to prepare local package for uploading")?;

    dst.seek(SeekFrom::Start(0))?;
    let dst_path = dst.parent().join(&filename);
    dst.rename(&dst_path)?;

    let dst_metadata = dst
        .file()
        .metadata()
        .with_context(|| format!("could not learn metadata for: `{}`", dst_path.display()))?;
    let compressed_size = dst_metadata.len();

    let uncompressed = HumanBytes(uncompressed_size);
    let compressed = HumanBytes(compressed_size);

    let message = format!("{filecount} files, {uncompressed:.1} ({compressed:.1} compressed)");
    // It doesn't really matter if this fails.
    drop(gctx.shell().status("Packaged", message));

    return Ok(dst);
}

/// Packages an entire workspace.
///
/// Returns the generated package files. If `opts.list` is true, skips
/// generating package files and returns an empty list.
pub fn package(ws: &Workspace<'_>, opts: &PackageOpts<'_>) -> CargoResult<Vec<FileLock>> {
    let specs = &opts.to_package.to_package_id_specs(ws)?;
    // If -p is used, we should check spec is matched with the members (See #13719)
    if let ops::Packages::Packages(_) = opts.to_package {
        for spec in specs.iter() {
            let member_ids = ws.members().map(|p| p.package_id());
            spec.query(member_ids)?;
        }
    }
    let mut pkgs = ws.members_with_features(specs, &opts.cli_features)?;

    // In `members_with_features_old`, it will add "current" package (determined by the cwd)
    // So we need filter
    pkgs.retain(|(pkg, _feats)| specs.iter().any(|spec| spec.matches(pkg.package_id())));

    let packaged = do_package(ws, opts, pkgs)?;

    let mut result = Vec::new();
    let target_dir = ws.target_dir();
    let build_dir = ws.build_dir();
    if target_dir == build_dir {
        result.extend(packaged.into_iter().map(|(_, _, src)| src));
    } else {
        // Uplifting artifacts
        let artifact_dir = target_dir.join("package");
        for (pkg, _, src) in packaged {
            let filename = pkg.package_id().tarball_name();
            let dst =
                artifact_dir.open_rw_exclusive_create(filename, ws.gctx(), "uplifted package")?;
            src.file().seek(SeekFrom::Start(0))?;
            std::io::copy(&mut src.file(), &mut dst.file())?;
            result.push(dst);
        }
    }

    Ok(result)
}

/// Packages an entire workspace.
///
/// Returns the generated package files and the dependencies between them. If
/// `opts.list` is true, skips generating package files and returns an empty
/// list.
pub(crate) fn package_with_dep_graph(
    ws: &Workspace<'_>,
    opts: &PackageOpts<'_>,
    pkgs: Vec<(&Package, CliFeatures)>,
) -> CargoResult<LocalDependencies<(CliFeatures, FileLock)>> {
    let output = do_package(ws, opts, pkgs)?;

    Ok(local_deps(output.into_iter().map(
        |(pkg, opts, tarball)| (pkg, (opts.cli_features, tarball)),
    )))
}

fn do_package<'a>(
    ws: &Workspace<'_>,
    opts: &PackageOpts<'a>,
    pkgs: Vec<(&Package, CliFeatures)>,
) -> CargoResult<Vec<(Package, PackageOpts<'a>, FileLock)>> {
    if ws
        .lock_root()
        .as_path_unlocked()
        .join(LOCKFILE_NAME)
        .exists()
        && opts.include_lockfile
    {
        // Make sure the Cargo.lock is up-to-date and valid.
        let dry_run = false;
        let _ = ops::resolve_ws(ws, dry_run)?;
        // If Cargo.lock does not exist, it will be generated by `build_lock`
        // below, and will be validated during the verification step.
    }

    let deps = local_deps(pkgs.iter().map(|(p, f)| ((*p).clone(), f.clone())));
    let just_pkgs: Vec<_> = pkgs.iter().map(|p| p.0).collect();

    let mut local_reg = {
        // The publish registry doesn't matter unless there are local dependencies that will be
        // resolved,
        // so only try to get one if we need it. If they explicitly passed a
        // registry on the CLI, we check it no matter what.
        let sid = if (deps.has_dependencies() && (opts.include_lockfile || opts.verify))
            || opts.reg_or_index.is_some()
        {
            let sid = get_registry(ws.gctx(), &just_pkgs, opts.reg_or_index.clone())?;
            debug!("packaging for registry {}", sid);
            Some(sid)
        } else {
            None
        };
        let reg_dir = ws.build_dir().join("package").join("tmp-registry");
        sid.map(|sid| TmpRegistry::new(ws.gctx(), reg_dir, sid))
            .transpose()?
    };

    // Packages need to be created in dependency order, because dependencies must
    // be added to our local overlay before we can create lockfiles that depend on them.
    let sorted_pkgs = deps.sort();
    let mut outputs: Vec<(Package, PackageOpts<'_>, FileLock)> = Vec::new();
    for (pkg, cli_features) in sorted_pkgs {
        let opts = PackageOpts {
            cli_features: cli_features.clone(),
            to_package: ops::Packages::Default,
            ..opts.clone()
        };
        let ar_files = prepare_archive(ws, &pkg, &opts)?;

        if opts.list {
            match opts.fmt {
                PackageMessageFormat::Human => {
                    // While this form is called "human",
                    // it keeps the old file-per-line format for compatibility.
                    for ar_file in &ar_files {
                        drop_println!(ws.gctx(), "{}", ar_file.rel_str);
                    }
                }
                PackageMessageFormat::Json => {
                    let message = messages::PackageList {
                        id: pkg.package_id().to_spec(),
                        files: BTreeMap::from_iter(ar_files.into_iter().map(|f| {
                            let file = match f.contents {
                                FileContents::OnDisk(path) => messages::PackageFile::Copy { path },
                                FileContents::Generated(
                                    GeneratedFile::Manifest(path)
                                    | GeneratedFile::Lockfile(Some(path)),
                                ) => messages::PackageFile::Generate { path: Some(path) },
                                FileContents::Generated(
                                    GeneratedFile::VcsInfo(_) | GeneratedFile::Lockfile(None),
                                ) => messages::PackageFile::Generate { path: None },
                            };
                            (f.rel_path, file)
                        })),
                    };
                    let _ = ws.gctx().shell().print_json(&message);
                }
            }
        } else {
            let tarball = create_package(ws, &opts, &pkg, ar_files, local_reg.as_ref())?;
            if let Some(local_reg) = local_reg.as_mut() {
                if pkg.publish() != &Some(Vec::new()) {
                    local_reg.add_package(ws, &pkg, &tarball)?;
                }
            }
            outputs.push((pkg, opts, tarball));
        }
    }

    // Verify all packages in the workspace. This can be done in any order, since the dependencies
    // are already all in the local registry overlay.
    if opts.verify {
        for (pkg, opts, tarball) in &outputs {
            verify::run_verify(ws, pkg, tarball, local_reg.as_ref(), opts)
                .context("failed to verify package tarball")?
        }
    }

    Ok(outputs)
}

/// Determine which registry the packages are for.
///
/// The registry only affects the built packages if there are dependencies within the
/// packages that we're packaging: if we're packaging foo-bin and foo-lib, and foo-bin
/// depends on foo-lib, then the foo-lib entry in foo-bin's lockfile will depend on the
/// registry that we're building packages for.
fn get_registry(
    gctx: &GlobalContext,
    pkgs: &[&Package],
    reg_or_index: Option<RegistryOrIndex>,
) -> CargoResult<SourceId> {
    let reg_or_index = match reg_or_index.clone() {
        Some(r) => Some(r),
        None => infer_registry(pkgs)?,
    };

    // Validate the registry against the packages' allow-lists.
    let reg = reg_or_index
        .clone()
        .unwrap_or_else(|| RegistryOrIndex::Registry(CRATES_IO_REGISTRY.to_owned()));
    if let RegistryOrIndex::Registry(reg_name) = reg {
        for pkg in pkgs {
            if let Some(allowed) = pkg.publish().as_ref() {
                // If allowed is empty (i.e. package.publish is false), we let it slide.
                // This allows packaging unpublishable packages (although packaging might
                // fail later if the unpublishable package is a dependency of something else).
                if !allowed.is_empty() && !allowed.iter().any(|a| a == &reg_name) {
                    bail!(
                        "`{}` cannot be packaged.\n\
                         The registry `{}` is not listed in the `package.publish` value in Cargo.toml.",
                        pkg.name(),
                        reg_name
                    );
                }
            }
        }
    }
    Ok(ops::registry::get_source_id(gctx, reg_or_index.as_ref())?.replacement)
}

/// Just the part of the dependency graph that's between the packages we're packaging.
#[derive(Clone, Debug, Default)]
pub(crate) struct LocalDependencies<T> {
    pub packages: HashMap<PackageId, (Package, T)>,
    pub graph: Graph<PackageId, ()>,
}

impl<T: Clone> LocalDependencies<T> {
    pub fn sort(&self) -> Vec<(Package, T)> {
        self.graph
            .sort()
            .into_iter()
            .map(|name| self.packages[&name].clone())
            .collect()
    }

    pub fn has_dependencies(&self) -> bool {
        self.graph
            .iter()
            .any(|node| self.graph.edges(node).next().is_some())
    }
}

/// Build just the part of the dependency graph that's between the given packages,
/// ignoring dev dependencies.
///
/// We assume that the packages all belong to this workspace.
fn local_deps<T>(packages: impl Iterator<Item = (Package, T)>) -> LocalDependencies<T> {
    let packages: HashMap<PackageId, (Package, T)> = packages
        .map(|(pkg, payload)| (pkg.package_id(), (pkg, payload)))
        .collect();

    // Dependencies have source ids but not package ids. We draw an edge
    // whenever a dependency's source id matches one of our packages. This is
    // wrong in general because it doesn't require (e.g.) versions to match. But
    // since we're working only with path dependencies here, it should be fine.
    let source_to_pkg: HashMap<_, _> = packages
        .keys()
        .map(|pkg_id| (pkg_id.source_id(), *pkg_id))
        .collect();

    let mut graph = Graph::new();
    for (pkg, _payload) in packages.values() {
        graph.add(pkg.package_id());
        for dep in pkg.dependencies() {
            // We're only interested in local (i.e. living in this workspace) dependencies.
            if !dep.source_id().is_path() {
                continue;
            }

            // If local dev-dependencies don't have a version specified, they get stripped
            // on publish so we should ignore them.
            if dep.kind() == DepKind::Development && !dep.specified_req() {
                continue;
            };

            // We don't care about cycles
            if dep.source_id() == pkg.package_id().source_id() {
                continue;
            }

            if let Some(dep_pkg) = source_to_pkg.get(&dep.source_id()) {
                graph.link(pkg.package_id(), *dep_pkg);
            }
        }
    }

    LocalDependencies { packages, graph }
}

/// Performs pre-archiving checks and builds a list of files to archive.
#[tracing::instrument(skip_all)]
fn prepare_archive(
    ws: &Workspace<'_>,
    pkg: &Package,
    opts: &PackageOpts<'_>,
) -> CargoResult<Vec<ArchiveFile>> {
    let gctx = ws.gctx();
    let mut src = PathSource::new(pkg.root(), pkg.package_id().source_id(), gctx);
    src.load()?;

    if opts.check_metadata {
        check_metadata(pkg, gctx)?;
    }

    if !pkg.manifest().exclude().is_empty() && !pkg.manifest().include().is_empty() {
        gctx.shell().warn(
            "both package.include and package.exclude are specified; \
             the exclude list will be ignored",
        )?;
    }
    let src_files = src.list_files(pkg)?;

    // Check (git) repository state, getting the current commit hash.
    let vcs_info = vcs::check_repo_state(pkg, &src_files, ws, &opts)?;
    build_ar_list(ws, pkg, src_files, vcs_info, opts.include_lockfile)
}

/// Builds list of files to archive.
#[tracing::instrument(skip_all)]
fn build_ar_list(
    ws: &Workspace<'_>,
    pkg: &Package,
    src_files: Vec<PathEntry>,
    vcs_info: Option<vcs::VcsInfo>,
    include_lockfile: bool,
) -> CargoResult<Vec<ArchiveFile>> {
    let mut result = HashMap::new();
    let root = pkg.root();
    for src_file in &src_files {
        let rel_path = src_file.strip_prefix(&root)?;
        check_filename(rel_path, &mut ws.gctx().shell())?;
        let rel_str = rel_path.to_str().ok_or_else(|| {
            anyhow::format_err!("non-utf8 path in source directory: {}", rel_path.display())
        })?;
        match rel_str {
            "Cargo.lock" => continue,
            VCS_INFO_FILE | ORIGINAL_MANIFEST_FILE => anyhow::bail!(
                "invalid inclusion of reserved file name {} in package source",
                rel_str
            ),
            _ => {
                result
                    .entry(UncasedAscii::new(rel_str))
                    .or_insert_with(Vec::new)
                    .push(ArchiveFile {
                        rel_path: rel_path.to_owned(),
                        rel_str: rel_str.to_owned(),
                        contents: FileContents::OnDisk(src_file.to_path_buf()),
                    });
            }
        }
    }

    // Ensure we normalize for case insensitive filesystems (like on Windows) by removing the
    // existing entry, regardless of case, and adding in with the correct case
    if result.remove(&UncasedAscii::new("Cargo.toml")).is_some() {
        result
            .entry(UncasedAscii::new(ORIGINAL_MANIFEST_FILE))
            .or_insert_with(Vec::new)
            .push(ArchiveFile {
                rel_path: PathBuf::from(ORIGINAL_MANIFEST_FILE),
                rel_str: ORIGINAL_MANIFEST_FILE.to_string(),
                contents: FileContents::OnDisk(pkg.manifest_path().to_owned()),
            });
        result
            .entry(UncasedAscii::new("Cargo.toml"))
            .or_insert_with(Vec::new)
            .push(ArchiveFile {
                rel_path: PathBuf::from("Cargo.toml"),
                rel_str: "Cargo.toml".to_string(),
                contents: FileContents::Generated(GeneratedFile::Manifest(
                    pkg.manifest_path().to_owned(),
                )),
            });
    } else {
        ws.gctx().shell().warn(&format!(
            "no `Cargo.toml` file found when packaging `{}` (note the case of the file name).",
            pkg.name()
        ))?;
    }

    if include_lockfile {
        let lockfile_path = ws.lock_root().as_path_unlocked().join(LOCKFILE_NAME);
        let lockfile_path = lockfile_path.exists().then_some(lockfile_path);
        let rel_str = "Cargo.lock";
        result
            .entry(UncasedAscii::new(rel_str))
            .or_insert_with(Vec::new)
            .push(ArchiveFile {
                rel_path: PathBuf::from(rel_str),
                rel_str: rel_str.to_string(),
                contents: FileContents::Generated(GeneratedFile::Lockfile(lockfile_path)),
            });
    }

    if let Some(vcs_info) = vcs_info {
        let rel_str = VCS_INFO_FILE;
        result
            .entry(UncasedAscii::new(rel_str))
            .or_insert_with(Vec::new)
            .push(ArchiveFile {
                rel_path: PathBuf::from(rel_str),
                rel_str: rel_str.to_string(),
                contents: FileContents::Generated(GeneratedFile::VcsInfo(vcs_info)),
            });
    }

    let mut invalid_manifest_field: Vec<String> = vec![];

    let mut result = result.into_values().flatten().collect();
    if let Some(license_file) = &pkg.manifest().metadata().license_file {
        let license_path = Path::new(license_file);
        let abs_file_path = paths::normalize_path(&pkg.root().join(license_path));
        if abs_file_path.is_file() {
            check_for_file_and_add(
                "license-file",
                license_path,
                abs_file_path,
                pkg,
                &mut result,
                ws,
            )?;
        } else {
            error_on_nonexistent_file(
                &pkg,
                &license_path,
                "license-file",
                &mut invalid_manifest_field,
            );
        }
    }
    if let Some(readme) = &pkg.manifest().metadata().readme {
        let readme_path = Path::new(readme);
        let abs_file_path = paths::normalize_path(&pkg.root().join(readme_path));
        if abs_file_path.is_file() {
            check_for_file_and_add("readme", readme_path, abs_file_path, pkg, &mut result, ws)?;
        } else {
            error_on_nonexistent_file(&pkg, &readme_path, "readme", &mut invalid_manifest_field);
        }
    }

    if !invalid_manifest_field.is_empty() {
        return Err(anyhow::anyhow!(invalid_manifest_field.join("\n")));
    }

    for t in pkg
        .manifest()
        .targets()
        .iter()
        .filter(|t| t.is_custom_build())
    {
        if let Some(custom_build_path) = t.src_path().path() {
            let abs_custom_build_path = paths::normalize_path(&pkg.root().join(custom_build_path));
            if !abs_custom_build_path.is_file() || !abs_custom_build_path.starts_with(pkg.root()) {
                error_custom_build_file_not_in_package(pkg, &abs_custom_build_path, t)?;
            }
        }
    }

    result.sort_unstable_by(|a, b| a.rel_path.cmp(&b.rel_path));

    Ok(result)
}

fn check_for_file_and_add(
    label: &str,
    file_path: &Path,
    abs_file_path: PathBuf,
    pkg: &Package,
    result: &mut Vec<ArchiveFile>,
    ws: &Workspace<'_>,
) -> CargoResult<()> {
    match abs_file_path.strip_prefix(&pkg.root()) {
        Ok(rel_file_path) => {
            if !result.iter().any(|ar| ar.rel_path == rel_file_path) {
                result.push(ArchiveFile {
                    rel_path: rel_file_path.to_path_buf(),
                    rel_str: rel_file_path
                        .to_str()
                        .expect("everything was utf8")
                        .to_string(),
                    contents: FileContents::OnDisk(abs_file_path),
                })
            }
        }
        Err(_) => {
            // The file exists somewhere outside of the package.
            let file_name = file_path.file_name().unwrap();
            if result.iter().any(|ar| ar.rel_path == file_name) {
                ws.gctx().shell().warn(&format!(
                    "{} `{}` appears to be a path outside of the package, \
                            but there is already a file named `{}` in the root of the package. \
                            The archived crate will contain the copy in the root of the package. \
                            Update the {} to point to the path relative \
                            to the root of the package to remove this warning.",
                    label,
                    file_path.display(),
                    file_name.to_str().unwrap(),
                    label,
                ))?;
            } else {
                result.push(ArchiveFile {
                    rel_path: PathBuf::from(file_name),
                    rel_str: file_name.to_str().unwrap().to_string(),
                    contents: FileContents::OnDisk(abs_file_path),
                })
            }
        }
    }
    Ok(())
}

fn error_on_nonexistent_file(
    pkg: &Package,
    path: &Path,
    manifest_key_name: &'static str,
    invalid: &mut Vec<String>,
) {
    let rel_msg = if path.is_absolute() {
        "".to_string()
    } else {
        format!(" (relative to `{}`)", pkg.root().display())
    };

    let msg = format!(
        "{manifest_key_name} `{}` does not appear to exist{}.\n\
                Please update the {manifest_key_name} setting in the manifest at `{}`.",
        path.display(),
        rel_msg,
        pkg.manifest_path().display()
    );

    invalid.push(msg);
}

fn error_custom_build_file_not_in_package(
    pkg: &Package,
    path: &Path,
    target: &Target,
) -> CargoResult<Vec<ArchiveFile>> {
    let tip = {
        let description_name = target.description_named();
        if path.is_file() {
            format!(
                "the source file of {description_name} doesn't appear to be a path inside of the package.\n\
            It is at `{}`, whereas the root the package is `{}`.\n",
                path.display(),
                pkg.root().display()
            )
        } else {
            format!("the source file of {description_name} doesn't appear to exist.\n",)
        }
    };
    let msg = format!(
        "{}\
        This may cause issue during packaging, as modules resolution and resources included via macros are often relative to the path of source files.\n\
        Please update the `build` setting in the manifest at `{}` and point to a path inside the root of the package.",
        tip,
        pkg.manifest_path().display()
    );
    anyhow::bail!(msg)
}

/// Construct `Cargo.lock` for the package to be published.
fn build_lock(
    ws: &Workspace<'_>,
    opts: &PackageOpts<'_>,
    publish_pkg: &Package,
    local_reg: Option<&TmpRegistry<'_>>,
) -> CargoResult<String> {
    let gctx = ws.gctx();
    let mut orig_resolve = ops::load_pkg_lockfile(ws)?;

    let mut tmp_ws = Workspace::ephemeral(publish_pkg.clone(), ws.gctx(), None, true)?;

    // The local registry is an overlay used for simulating workspace packages
    // that are supposed to be in the published registry, but that aren't there
    // yet.
    if let Some(local_reg) = local_reg {
        tmp_ws.add_local_overlay(
            local_reg.upstream,
            local_reg.root.as_path_unlocked().to_owned(),
        );
        if opts.dry_run {
            if let Some(orig_resolve) = orig_resolve.as_mut() {
                let upstream_in_lock = if local_reg.upstream.is_crates_io() {
                    SourceId::crates_io(gctx)?
                } else {
                    local_reg.upstream
                };
                for (p, s) in local_reg.checksums() {
                    orig_resolve.set_checksum(p.with_source_id(upstream_in_lock), s.to_owned());
                }
            }
        }
    }
    let mut tmp_reg = tmp_ws.package_registry()?;

    let mut new_resolve = ops::resolve_with_previous(
        &mut tmp_reg,
        &tmp_ws,
        &CliFeatures::new_all(true),
        HasDevUnits::Yes,
        orig_resolve.as_ref(),
        None,
        &[],
        true,
    )?;

    let pkg_set = ops::get_resolved_packages(&new_resolve, tmp_reg)?;

    if let Some(orig_resolve) = orig_resolve {
        compare_resolve(gctx, tmp_ws.current()?, &orig_resolve, &new_resolve)?;
    }
    check_yanked(
        gctx,
        &pkg_set,
        &new_resolve,
        "consider updating to a version that is not yanked",
    )?;

    ops::resolve_to_string(&tmp_ws, &mut new_resolve)
}

// Checks that the package has some piece of metadata that a human can
// use to tell what the package is about.
fn check_metadata(pkg: &Package, gctx: &GlobalContext) -> CargoResult<()> {
    let md = pkg.manifest().metadata();

    let mut missing = vec![];

    macro_rules! lacking {
        ($( $($field: ident)||* ),*) => {{
            $(
                if $(md.$field.as_ref().map_or(true, |s| s.is_empty()))&&* {
                    $(missing.push(stringify!($field).replace("_", "-"));)*
                }
            )*
        }}
    }
    lacking!(
        description,
        license || license_file,
        documentation || homepage || repository
    );

    if !missing.is_empty() {
        let mut things = missing[..missing.len() - 1].join(", ");
        // `things` will be empty if and only if its length is 1 (i.e., the only case
        // to have no `or`).
        if !things.is_empty() {
            things.push_str(" or ");
        }
        things.push_str(missing.last().unwrap());

        gctx.shell().print_report(&[
            Level::WARNING.secondary_title(format!("manifest has no {things}"))
                .element(Level::NOTE.message("see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info"))
         ],
             false
        )?
    }

    Ok(())
}

/// Compresses and packages a list of [`ArchiveFile`]s and writes into the given file.
///
/// Returns the uncompressed size of the contents of the new archive file.
fn tar(
    ws: &Workspace<'_>,
    opts: &PackageOpts<'_>,
    pkg: &Package,
    local_reg: Option<&TmpRegistry<'_>>,
    ar_files: Vec<ArchiveFile>,
    dst: &File,
    filename: &str,
) -> CargoResult<u64> {
    // Prepare the encoder and its header.
    let filename = Path::new(filename);
    let encoder = GzBuilder::new()
        .filename(paths::path2bytes(filename)?)
        .write(dst, Compression::best());

    // Put all package files into a compressed archive.
    let mut ar = Builder::new(encoder);
    ar.sparse(false);
    let gctx = ws.gctx();

    let base_name = format!("{}-{}", pkg.name(), pkg.version());
    let base_path = Path::new(&base_name);
    let included = ar_files
        .iter()
        .map(|ar_file| ar_file.rel_path.clone())
        .collect::<Vec<_>>();
    let publish_pkg = prepare_for_publish(pkg, ws, Some(&included))?;

    let mut uncompressed_size = 0;
    for ar_file in ar_files {
        let ArchiveFile {
            rel_path,
            rel_str,
            contents,
        } = ar_file;
        let ar_path = base_path.join(&rel_path);
        gctx.shell()
            .verbose(|shell| shell.status("Archiving", &rel_str))?;
        let mut header = Header::new_gnu();
        match contents {
            FileContents::OnDisk(disk_path) => {
                let mut file = File::open(&disk_path).with_context(|| {
                    format!("failed to open for archiving: `{}`", disk_path.display())
                })?;
                let metadata = file.metadata().with_context(|| {
                    format!("could not learn metadata for: `{}`", disk_path.display())
                })?;
                header.set_metadata_in_mode(&metadata, HeaderMode::Deterministic);
                header.set_cksum();
                ar.append_data(&mut header, &ar_path, &mut file)
                    .with_context(|| {
                        format!("could not archive source file `{}`", disk_path.display())
                    })?;
                uncompressed_size += metadata.len() as u64;
            }
            FileContents::Generated(generated_kind) => {
                let contents = match generated_kind {
                    GeneratedFile::Manifest(_) => {
                        publish_pkg.manifest().to_normalized_contents()?
                    }
                    GeneratedFile::Lockfile(_) => build_lock(ws, opts, &publish_pkg, local_reg)?,
                    GeneratedFile::VcsInfo(ref s) => serde_json::to_string_pretty(s)?,
                };
                header.set_entry_type(EntryType::file());
                header.set_mode(0o644);
                header.set_size(contents.len() as u64);
                // use something nonzero to avoid rust-lang/cargo#9512
                header.set_mtime(1);
                header.set_cksum();
                ar.append_data(&mut header, &ar_path, contents.as_bytes())
                    .with_context(|| format!("could not archive source file `{}`", rel_str))?;
                uncompressed_size += contents.len() as u64;
            }
        }
    }

    let encoder = ar.into_inner()?;
    encoder.finish()?;
    Ok(uncompressed_size)
}

/// Generate warnings when packaging Cargo.lock, and the resolve have changed.
fn compare_resolve(
    gctx: &GlobalContext,
    current_pkg: &Package,
    orig_resolve: &Resolve,
    new_resolve: &Resolve,
) -> CargoResult<()> {
    if gctx.shell().verbosity() != Verbosity::Verbose {
        return Ok(());
    }
    let new_set: BTreeSet<PackageId> = new_resolve.iter().collect();
    let orig_set: BTreeSet<PackageId> = orig_resolve.iter().collect();
    let added = new_set.difference(&orig_set);
    // Removed entries are ignored, this is used to quickly find hints for why
    // an entry changed.
    let removed: Vec<&PackageId> = orig_set.difference(&new_set).collect();
    for pkg_id in added {
        if pkg_id.name() == current_pkg.name() && pkg_id.version() == current_pkg.version() {
            // Skip the package that is being created, since its SourceId
            // (directory) changes.
            continue;
        }
        // Check for candidates where the source has changed (such as [patch]
        // or a dependency with multiple sources like path/version).
        let removed_candidates: Vec<&PackageId> = removed
            .iter()
            .filter(|orig_pkg_id| {
                orig_pkg_id.name() == pkg_id.name() && orig_pkg_id.version() == pkg_id.version()
            })
            .cloned()
            .collect();
        let extra = match removed_candidates.len() {
            0 => {
                // This can happen if the original was out of date.
                let previous_versions: Vec<&PackageId> = removed
                    .iter()
                    .filter(|orig_pkg_id| orig_pkg_id.name() == pkg_id.name())
                    .cloned()
                    .collect();
                match previous_versions.len() {
                    0 => String::new(),
                    1 => format!(
                        ", previous version was `{}`",
                        previous_versions[0].version()
                    ),
                    _ => format!(
                        ", previous versions were: {}",
                        previous_versions
                            .iter()
                            .map(|pkg_id| format!("`{}`", pkg_id.version()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                }
            }
            1 => {
                // This can happen for multi-sourced dependencies like
                // `{path="...", version="..."}` or `[patch]` replacement.
                // `[replace]` is not captured in Cargo.lock.
                format!(
                    ", was originally sourced from `{}`",
                    removed_candidates[0].source_id()
                )
            }
            _ => {
                // I don't know if there is a way to actually trigger this,
                // but handle it just in case.
                let comma_list = removed_candidates
                    .iter()
                    .map(|pkg_id| format!("`{}`", pkg_id.source_id()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    ", was originally sourced from one of these sources: {}",
                    comma_list
                )
            }
        };
        let msg = format!(
            "package `{}` added to the packaged Cargo.lock file{}",
            pkg_id, extra
        );
        gctx.shell().note(msg)?;
    }
    Ok(())
}

pub fn check_yanked(
    gctx: &GlobalContext,
    pkg_set: &PackageSet<'_>,
    resolve: &Resolve,
    hint: &str,
) -> CargoResult<()> {
    // Checking the yanked status involves taking a look at the registry and
    // maybe updating files, so be sure to lock it here.
    let _lock = gctx.acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;

    let mut sources = pkg_set.sources_mut();
    let mut pending: Vec<PackageId> = resolve.iter().collect();
    let mut results = Vec::new();
    for (_id, source) in sources.sources_mut() {
        source.invalidate_cache();
    }
    while !pending.is_empty() {
        pending.retain(|pkg_id| {
            if let Some(source) = sources.get_mut(pkg_id.source_id()) {
                match source.is_yanked(*pkg_id) {
                    Poll::Ready(result) => results.push((*pkg_id, result)),
                    Poll::Pending => return true,
                }
            }
            false
        });
        for (_id, source) in sources.sources_mut() {
            source.block_until_ready()?;
        }
    }

    for (pkg_id, is_yanked) in results {
        if is_yanked? {
            gctx.shell().warn(format!(
                "package `{}` in Cargo.lock is yanked in registry `{}`, {}",
                pkg_id,
                pkg_id.source_id().display_registry_name(),
                hint
            ))?;
        }
    }
    Ok(())
}

// It can often be the case that files of a particular name on one platform
// can't actually be created on another platform. For example files with colons
// in the name are allowed on Unix but not on Windows.
//
// To help out in situations like this, issue about weird filenames when
// packaging as a "heads up" that something may not work on other platforms.
fn check_filename(file: &Path, shell: &mut Shell) -> CargoResult<()> {
    let Some(name) = file.file_name() else {
        return Ok(());
    };
    let Some(name) = name.to_str() else {
        anyhow::bail!(
            "path does not have a unicode filename which may not unpack \
             on all platforms: {}",
            file.display()
        )
    };
    let bad_chars = ['/', '\\', '<', '>', ':', '"', '|', '?', '*'];
    if let Some(c) = bad_chars.iter().find(|c| name.contains(**c)) {
        anyhow::bail!(
            "cannot package a filename with a special character `{}`: {}",
            c,
            file.display()
        )
    }
    if restricted_names::is_windows_reserved_path(file) {
        shell.warn(format!(
            "file {} is a reserved Windows filename, \
                it will not work on Windows platforms",
            file.display()
        ))?;
    }
    Ok(())
}

/// Manages a temporary local registry that we use to overlay our new packages on the
/// upstream registry. This way we can build lockfiles that depend on the new packages even
/// before they're published.
struct TmpRegistry<'a> {
    gctx: &'a GlobalContext,
    upstream: SourceId,
    root: Filesystem,
    checksums: HashMap<PackageId, String>,
    _lock: FileLock,
}

impl<'a> TmpRegistry<'a> {
    fn new(gctx: &'a GlobalContext, root: Filesystem, upstream: SourceId) -> CargoResult<Self> {
        root.create_dir()?;
        let _lock = root.open_rw_exclusive_create(".cargo-lock", gctx, "temporary registry")?;
        let slf = Self {
            gctx,
            root,
            upstream,
            checksums: HashMap::new(),
            _lock,
        };
        // If there's an old temporary registry, delete it.
        let index_path = slf.index_path().into_path_unlocked();
        if index_path.exists() {
            paths::remove_dir_all(index_path)?;
        }
        slf.index_path().create_dir()?;
        Ok(slf)
    }

    fn index_path(&self) -> Filesystem {
        self.root.join("index")
    }

    fn add_package(
        &mut self,
        ws: &Workspace<'_>,
        package: &Package,
        tar: &FileLock,
    ) -> CargoResult<()> {
        debug!(
            "adding package {}@{} to local overlay at {}",
            package.name(),
            package.version(),
            self.root.as_path_unlocked().display()
        );
        {
            let mut tar_copy = self.root.open_rw_exclusive_create(
                package.package_id().tarball_name(),
                self.gctx,
                "temporary package registry",
            )?;
            tar.file().seek(SeekFrom::Start(0))?;
            std::io::copy(&mut tar.file(), &mut tar_copy)?;
            tar_copy.flush()?;
        }

        let new_crate = super::registry::prepare_transmit(self.gctx, ws, package, self.upstream)?;

        tar.file().seek(SeekFrom::Start(0))?;
        let cksum = cargo_util::Sha256::new()
            .update_file(tar.file())?
            .finish_hex();

        self.checksums.insert(package.package_id(), cksum.clone());

        let deps: Vec<_> = new_crate
            .deps
            .into_iter()
            .map(|dep| {
                let name = dep
                    .explicit_name_in_toml
                    .clone()
                    .unwrap_or_else(|| dep.name.clone())
                    .into();
                let package = dep
                    .explicit_name_in_toml
                    .as_ref()
                    .map(|_| dep.name.clone().into());
                RegistryDependency {
                    name: name,
                    req: dep.version_req.into(),
                    features: dep.features.into_iter().map(|x| x.into()).collect(),
                    optional: dep.optional,
                    default_features: dep.default_features,
                    target: dep.target.map(|x| x.into()),
                    kind: Some(dep.kind.into()),
                    registry: dep.registry.map(|x| x.into()),
                    package: package,
                    public: None,
                    artifact: dep
                        .artifact
                        .map(|xs| xs.into_iter().map(|x| x.into()).collect()),
                    bindep_target: dep.bindep_target.map(|x| x.into()),
                    lib: dep.lib,
                }
            })
            .collect();

        let index_line = serde_json::to_string(&IndexPackage {
            name: new_crate.name.into(),
            vers: package.version().clone(),
            deps,
            features: new_crate
                .features
                .into_iter()
                .map(|(k, v)| (k.into(), v.into_iter().map(|x| x.into()).collect()))
                .collect(),
            features2: None,
            cksum,
            yanked: None,
            links: new_crate.links.map(|x| x.into()),
            rust_version: None,
            v: Some(2),
        })?;

        let file =
            cargo_util::registry::make_dep_path(&package.name().as_str().to_lowercase(), false);
        let mut dst = self.index_path().open_rw_exclusive_create(
            file,
            self.gctx,
            "temporary package registry",
        )?;
        dst.write_all(index_line.as_bytes())?;
        Ok(())
    }

    fn checksums(&self) -> impl Iterator<Item = (PackageId, &str)> {
        self.checksums.iter().map(|(p, s)| (*p, s.as_str()))
    }
}
