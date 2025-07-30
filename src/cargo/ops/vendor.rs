use crate::core::SourceId;
use crate::core::shell::Verbosity;
use crate::core::{GitReference, Package, Workspace};
use crate::ops;
use crate::sources::CRATES_IO_REGISTRY;
use crate::sources::RegistrySource;
use crate::sources::SourceConfigMap;
use crate::sources::path::PathSource;
use crate::util::cache_lock::CacheLockMode;
use crate::util::{CargoResult, GlobalContext, try_canonicalize};

use anyhow::{Context as _, bail};
use cargo_util::{Sha256, paths};
use cargo_util_schemas::core::SourceKind;
use cargo_util_schemas::manifest::TomlPackageBuild;
use serde::Serialize;
use walkdir::WalkDir;

use std::collections::HashSet;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub struct VendorOptions<'a> {
    pub no_delete: bool,
    pub versioned_dirs: bool,
    pub destination: &'a Path,
    pub extra: Vec<PathBuf>,
    pub respect_source_config: bool,
}

pub fn vendor(ws: &Workspace<'_>, opts: &VendorOptions<'_>) -> CargoResult<()> {
    let gctx = ws.gctx();
    let mut extra_workspaces = Vec::new();
    for extra in opts.extra.iter() {
        let extra = gctx.cwd().join(extra);
        let ws = Workspace::new(&extra, gctx)?;
        extra_workspaces.push(ws);
    }
    let workspaces = extra_workspaces.iter().chain(Some(ws)).collect::<Vec<_>>();
    let _lock = gctx.acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;
    let vendor_config = sync(gctx, &workspaces, opts).context("failed to sync")?;

    if gctx.shell().verbosity() != Verbosity::Quiet {
        if vendor_config.source.is_empty() {
            crate::drop_eprintln!(gctx, "There is no dependency to vendor in this project.");
        } else {
            crate::drop_eprint!(
                gctx,
                "To use vendored sources, add this to your .cargo/config.toml for this project:\n\n"
            );
            crate::drop_print!(gctx, "{}", &toml::to_string_pretty(&vendor_config).unwrap());
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct VendorConfig {
    source: BTreeMap<String, VendorSource>,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase", untagged)]
enum VendorSource {
    Directory {
        directory: String,
    },
    Registry {
        registry: Option<String>,
        #[serde(rename = "replace-with")]
        replace_with: String,
    },
    Git {
        git: String,
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
        #[serde(rename = "replace-with")]
        replace_with: String,
    },
}

/// Cache for mapping replaced sources to replacements.
struct SourceReplacementCache<'gctx> {
    map: SourceConfigMap<'gctx>,
    cache: HashMap<SourceId, SourceId>,
}

impl SourceReplacementCache<'_> {
    fn new(
        gctx: &GlobalContext,
        respect_source_config: bool,
    ) -> CargoResult<SourceReplacementCache<'_>> {
        Ok(SourceReplacementCache {
            map: if respect_source_config {
                SourceConfigMap::new(gctx)
            } else {
                SourceConfigMap::empty(gctx)
            }?,
            cache: Default::default(),
        })
    }

    fn get(&mut self, id: SourceId) -> CargoResult<SourceId> {
        use std::collections::hash_map::Entry;
        match self.cache.entry(id) {
            Entry::Occupied(e) => Ok(e.get().clone()),
            Entry::Vacant(e) => {
                let replaced = self.map.load(id, &HashSet::new())?.replaced_source_id();
                Ok(e.insert(replaced).clone())
            }
        }
    }
}

fn sync(
    gctx: &GlobalContext,
    workspaces: &[&Workspace<'_>],
    opts: &VendorOptions<'_>,
) -> CargoResult<VendorConfig> {
    let dry_run = false;
    let vendor_dir = try_canonicalize(opts.destination);
    let vendor_dir = vendor_dir.as_deref().unwrap_or(opts.destination);
    let vendor_dir_already_exists = vendor_dir.exists();

    paths::create_dir_all(&vendor_dir)?;
    let mut to_remove = HashSet::new();
    if !opts.no_delete {
        for entry in vendor_dir.read_dir()? {
            let entry = entry?;
            if !entry
                .file_name()
                .to_str()
                .map_or(false, |s| s.starts_with('.'))
            {
                to_remove.insert(entry.path());
            }
        }
    }

    let mut source_replacement_cache =
        SourceReplacementCache::new(gctx, opts.respect_source_config)?;

    let mut checksums = HashMap::new();
    let mut ids = BTreeMap::new();

    // Let's download all crates and start storing internal tables about them.
    for ws in workspaces {
        let (packages, resolve) = ops::resolve_ws(ws, dry_run)
            .with_context(|| format!("failed to load lockfile for {}", ws.root().display()))?;

        packages
            .get_many(resolve.iter())
            .with_context(|| format!("failed to download packages for {}", ws.root().display()))?;

        for pkg in resolve.iter() {
            let sid = source_replacement_cache.get(pkg.source_id())?;

            // Don't vendor path crates since they're already in the repository
            if sid.is_path() {
                // And don't delete actual source code!
                if let Ok(path) = sid.url().to_file_path() {
                    if let Ok(path) = try_canonicalize(path) {
                        to_remove.remove(&path);
                    }
                }
                continue;
            }

            ids.insert(
                pkg,
                packages
                    .get_one(pkg)
                    .context("failed to fetch package")?
                    .clone(),
            );

            checksums.insert(pkg, resolve.checksums().get(&pkg).cloned());
        }
    }

    let mut versions = HashMap::new();
    for id in ids.keys() {
        let map = versions.entry(id.name()).or_insert_with(BTreeMap::default);
        if let Some(prev) = map.get(&id.version()) {
            bail!(
                "found duplicate version of package `{} v{}` \
                 vendored from two sources:\n\
                 \n\
                 \tsource 1: {}\n\
                 \tsource 2: {}",
                id.name(),
                id.version(),
                prev,
                id.source_id()
            );
        }
        map.insert(id.version(), id.source_id());
    }

    let mut sources = BTreeSet::new();
    let mut tmp_buf = [0; 64 * 1024];
    for (id, pkg) in ids.iter() {
        // Next up, copy it to the vendor directory
        let src = pkg.root();
        let max_version = *versions[&id.name()].iter().rev().next().unwrap().0;
        let dir_has_version_suffix = opts.versioned_dirs || id.version() != max_version;
        let dst_name = if dir_has_version_suffix {
            // Eg vendor/futures-0.1.13
            format!("{}-{}", id.name(), id.version())
        } else {
            // Eg vendor/futures
            id.name().to_string()
        };

        sources.insert(id.source_id());
        let dst = vendor_dir.join(&dst_name);
        to_remove.remove(&dst);
        let cksum = dst.join(".cargo-checksum.json");
        // Registries are the only immutable sources,
        // path and git dependencies' versions cannot be trusted to mean "no change"
        if dir_has_version_suffix && id.source_id().is_registry() && cksum.exists() {
            // Don't re-copy directory with version suffix in case it comes from a registry
            continue;
        }

        gctx.shell().status(
            "Vendoring",
            &format!("{} ({}) to {}", id, src.to_string_lossy(), dst.display()),
        )?;

        let _ = fs::remove_dir_all(&dst);

        let mut file_cksums = BTreeMap::new();

        // Need this mapping anyway because we will directly consult registry sources,
        // otherwise builtin source replacement (sparse registry) won't be respected.
        let sid = source_replacement_cache.get(id.source_id())?;

        if sid.is_registry() {
            // To keep the unpacked source from registry in a pristine state,
            // we'll do a direct extraction into the vendor directory.
            let registry = match sid.kind() {
                SourceKind::Registry | SourceKind::SparseRegistry => {
                    RegistrySource::remote(sid, &Default::default(), gctx)?
                }
                SourceKind::LocalRegistry => {
                    let path = sid.url().to_file_path().expect("local path");
                    RegistrySource::local(sid, &path, &Default::default(), gctx)
                }
                _ => unreachable!("not registry source: {sid}"),
            };

            let walkdir = |root| {
                WalkDir::new(root)
                    .into_iter()
                    // It is safe to skip errors,
                    // since we'll hit them during copying/reading later anyway.
                    .filter_map(|e| e.ok())
                    // There should be no symlink in tarballs on crates.io,
                    // but might be wrong for local registries.
                    // Hence here be conservative and include symlinks.
                    .filter(|e| e.file_type().is_file() || e.file_type().is_symlink())
            };
            let mut compute_file_cksums = |root| {
                for e in walkdir(root) {
                    let path = e.path();
                    let relative = path.strip_prefix(&dst).unwrap();
                    let cksum = Sha256::new()
                        .update_path(path)
                        .map(Sha256::finish_hex)
                        .with_context(|| format!("failed to checksum `{}`", path.display()))?;
                    file_cksums.insert(relative.to_str().unwrap().replace("\\", "/"), cksum);
                }
                Ok::<_, anyhow::Error>(())
            };
            if dir_has_version_suffix {
                registry.unpack_package_in(id, &vendor_dir, &vendor_this)?;
                compute_file_cksums(&dst)?;
            } else {
                // Due to the extra sanity check in registry unpack
                // (ensure it contain only one top-level directory with name `pkg-version`),
                // we can only unpack a directory with version suffix,
                // and move it to the no suffix directory.
                let staging_dir = tempfile::Builder::new()
                    .prefix(".vendor-staging")
                    .tempdir_in(vendor_dir)?;
                let unpacked_src =
                    registry.unpack_package_in(id, staging_dir.path(), &vendor_this)?;
                if let Err(e) = fs::rename(&unpacked_src, &dst) {
                    // This fallback is mainly for Windows 10 versions earlier than 1607.
                    // The destination of `fs::rename` can't be a directory in older versions.
                    // Can be removed once the minimal supported Windows version gets bumped.
                    tracing::warn!("failed to `mv {unpacked_src:?} {dst:?}`: {e}");
                    let paths: Vec<_> = walkdir(&unpacked_src).map(|e| e.into_path()).collect();
                    cp_sources(pkg, src, &paths, &dst, &mut file_cksums, &mut tmp_buf, gctx)
                        .with_context(|| format!("failed to copy vendored sources for {id}"))?;
                } else {
                    compute_file_cksums(&dst)?;
                }
            }
        } else {
            let paths = PathSource::new(src, sid, gctx)
                .list_files(pkg)?
                .into_iter()
                .map(|p| p.into_path_buf())
                .collect::<Vec<_>>();
            cp_sources(pkg, src, &paths, &dst, &mut file_cksums, &mut tmp_buf, gctx)
                .with_context(|| format!("failed to copy vendored sources for {id}"))?;
        }

        // Finally, emit the metadata about this package
        let json = serde_json::json!({
            "package": checksums.get(id),
            "files": file_cksums,
        });

        paths::write(&cksum, json.to_string())?;
    }

    for path in to_remove {
        if path.is_dir() {
            paths::remove_dir_all(&path)?;
        } else {
            paths::remove_file(&path)?;
        }
    }

    // add our vendored source
    let mut config = BTreeMap::new();

    let merged_source_name = "vendored-sources";

    // replace original sources with vendor
    for source_id in sources {
        let name = if source_id.is_crates_io() {
            CRATES_IO_REGISTRY.to_string()
        } else {
            // Remove `precise` since that makes the source name very long,
            // and isn't needed to disambiguate multiple sources.
            source_id.without_precise().as_url().to_string()
        };

        let source = if source_id.is_crates_io() {
            VendorSource::Registry {
                registry: None,
                replace_with: merged_source_name.to_string(),
            }
        } else if source_id.is_remote_registry() {
            let registry = source_id.url().to_string();
            VendorSource::Registry {
                registry: Some(registry),
                replace_with: merged_source_name.to_string(),
            }
        } else if source_id.is_git() {
            let mut branch = None;
            let mut tag = None;
            let mut rev = None;
            if let Some(reference) = source_id.git_reference() {
                match *reference {
                    GitReference::Branch(ref b) => branch = Some(b.clone()),
                    GitReference::Tag(ref t) => tag = Some(t.clone()),
                    GitReference::Rev(ref r) => rev = Some(r.clone()),
                    GitReference::DefaultBranch => {}
                }
            }
            VendorSource::Git {
                git: source_id.url().to_string(),
                branch,
                tag,
                rev,
                replace_with: merged_source_name.to_string(),
            }
        } else {
            panic!("Invalid source ID: {}", source_id)
        };
        config.insert(name, source);
    }

    if !config.is_empty() {
        config.insert(
            merged_source_name.to_string(),
            VendorSource::Directory {
                // Windows-flavour paths are valid here on Windows but Unix.
                // This backslash normalization is for making output paths more
                // cross-platform compatible.
                directory: opts.destination.to_string_lossy().replace("\\", "/"),
            },
        );
    } else if !vendor_dir_already_exists {
        // Nothing to vendor. Remove the destination dir we've just created.
        paths::remove_dir(vendor_dir)?;
    }

    Ok(VendorConfig { source: config })
}

fn cp_sources(
    pkg: &Package,
    src: &Path,
    paths: &[PathBuf],
    dst: &Path,
    cksums: &mut BTreeMap<String, String>,
    tmp_buf: &mut [u8],
    gctx: &GlobalContext,
) -> CargoResult<()> {
    for p in paths {
        let relative = p.strip_prefix(&src).unwrap();

        if !vendor_this(relative) {
            continue;
        }

        // Join pathname components individually to make sure that the joined
        // path uses the correct directory separators everywhere, since
        // `relative` may use Unix-style and `dst` may require Windows-style
        // backslashes.
        let dst = relative
            .iter()
            .fold(dst.to_owned(), |acc, component| acc.join(&component));

        paths::create_dir_all(dst.parent().unwrap())?;
        let mut dst_opts = OpenOptions::new();
        dst_opts.write(true).create(true).truncate(true);
        // When vendoring git dependencies, the manifest has not been normalized like it would be
        // when published. This causes issue when the manifest is using workspace inheritance.
        // To get around this issue we use the "original" manifest after `{}.workspace = true`
        // has been resolved for git dependencies.
        let cksum = if dst.file_name() == Some(OsStr::new("Cargo.toml"))
            && pkg.package_id().source_id().is_git()
        {
            let packaged_files = paths
                .iter()
                .map(|p| p.strip_prefix(src).unwrap().to_owned())
                .collect::<Vec<_>>();
            let vendored_pkg = prepare_for_vendor(pkg, &packaged_files, gctx)?;
            let contents = vendored_pkg.manifest().to_normalized_contents()?;
            copy_and_checksum(
                &dst,
                &mut dst_opts,
                &mut contents.as_bytes(),
                Path::new("Generated Cargo.toml"),
                tmp_buf,
            )?
        } else {
            let mut src = File::open(&p).with_context(|| format!("failed to open {:?}", &p))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
                let src_metadata = src
                    .metadata()
                    .with_context(|| format!("failed to stat {:?}", p))?;
                dst_opts.mode(src_metadata.mode());
            }
            copy_and_checksum(&dst, &mut dst_opts, &mut src, &p, tmp_buf)?
        };

        cksums.insert(relative.to_str().unwrap().replace("\\", "/"), cksum);
    }
    Ok(())
}

/// HACK: Perform the bare minimum of `prepare_for_publish` needed for #14348.
///
/// There are parts of `prepare_for_publish` that could be directly useful (e.g. stripping
/// `[workspace]`) while other parts that require other filesystem operations (moving the README
/// file) and ideally we'd reuse `cargo package` code to take care of all of this for us.
fn prepare_for_vendor(
    me: &Package,
    packaged_files: &[PathBuf],
    gctx: &GlobalContext,
) -> CargoResult<Package> {
    let contents = me.manifest().contents();
    let document = me.manifest().document();
    let original_toml = prepare_toml_for_vendor(
        me.manifest().normalized_toml().clone(),
        packaged_files,
        gctx,
    )?;
    let normalized_toml = original_toml.clone();
    let features = me.manifest().unstable_features().clone();
    let workspace_config = me.manifest().workspace_config().clone();
    let source_id = me.package_id().source_id();
    let mut warnings = Default::default();
    let mut errors = Default::default();
    let manifest = crate::util::toml::to_real_manifest(
        contents.to_owned(),
        document.clone(),
        original_toml,
        normalized_toml,
        features,
        workspace_config,
        source_id,
        me.manifest_path(),
        me.manifest().is_embedded(),
        gctx,
        &mut warnings,
        &mut errors,
    )?;
    let new_pkg = Package::new(manifest, me.manifest_path());
    Ok(new_pkg)
}

fn prepare_toml_for_vendor(
    mut me: cargo_util_schemas::manifest::TomlManifest,
    packaged_files: &[PathBuf],
    gctx: &GlobalContext,
) -> CargoResult<cargo_util_schemas::manifest::TomlManifest> {
    let package = me
        .package
        .as_mut()
        .expect("venedored manifests must have packages");
    // Validates if build script file is included in package. If not, warn and ignore.
    if let Some(custom_build_scripts) = package.normalized_build().expect("previously normalized") {
        let mut included_scripts = Vec::new();
        for script in custom_build_scripts {
            let path = paths::normalize_path(Path::new(script));
            let included = packaged_files.contains(&path);
            if included {
                let path = path
                    .into_os_string()
                    .into_string()
                    .map_err(|_err| anyhow::format_err!("non-UTF8 `package.build`"))?;
                let path = crate::util::toml::normalize_path_string_sep(path);
                included_scripts.push(path);
            } else {
                gctx.shell().warn(format!(
                    "ignoring `package.build` entry `{}` as it is not included in the published package",
                    path.display()
                ))?;
            }
        }
        package.build = Some(match included_scripts.len() {
            0 => TomlPackageBuild::Auto(false),
            1 => TomlPackageBuild::SingleScript(included_scripts[0].clone()),
            _ => TomlPackageBuild::MultipleScript(included_scripts),
        });
    }

    let lib = if let Some(target) = &me.lib {
        crate::util::toml::prepare_target_for_publish(
            target,
            Some(packaged_files),
            "library",
            gctx,
        )?
    } else {
        None
    };
    let bin = crate::util::toml::prepare_targets_for_publish(
        me.bin.as_ref(),
        Some(packaged_files),
        "binary",
        gctx,
    )?;
    let example = crate::util::toml::prepare_targets_for_publish(
        me.example.as_ref(),
        Some(packaged_files),
        "example",
        gctx,
    )?;
    let test = crate::util::toml::prepare_targets_for_publish(
        me.test.as_ref(),
        Some(packaged_files),
        "test",
        gctx,
    )?;
    let bench = crate::util::toml::prepare_targets_for_publish(
        me.bench.as_ref(),
        Some(packaged_files),
        "benchmark",
        gctx,
    )?;

    me.lib = lib;
    me.bin = bin;
    me.example = example;
    me.test = test;
    me.bench = bench;

    Ok(me)
}

fn copy_and_checksum<T: Read>(
    dst_path: &Path,
    dst_opts: &mut OpenOptions,
    contents: &mut T,
    contents_path: &Path,
    buf: &mut [u8],
) -> CargoResult<String> {
    let mut dst = dst_opts
        .open(dst_path)
        .with_context(|| format!("failed to create {:?}", dst_path))?;
    // Not going to bother setting mode on pre-existing files, since there
    // shouldn't be any under normal conditions.
    let mut cksum = Sha256::new();
    loop {
        let n = contents
            .read(buf)
            .with_context(|| format!("failed to read from {:?}", contents_path))?;
        if n == 0 {
            break Ok(cksum.finish_hex());
        }
        let data = &buf[..n];
        cksum.update(data);
        dst.write_all(data)
            .with_context(|| format!("failed to write to {:?}", dst_path))?;
    }
}

/// Filters files we want to vendor.
///
/// `relative` is a path relative to the package root.
fn vendor_this(relative: &Path) -> bool {
    match relative.to_str() {
        // Skip git config files as they're not relevant to builds most of
        // the time and if we respect them (e.g.  in git) then it'll
        // probably mess with the checksums when a vendor dir is checked
        // into someone else's source control
        Some(".gitattributes" | ".gitignore" | ".git") => false,

        // Temporary Cargo files
        Some(".cargo-ok") => false,

        _ => true,
    }
}
