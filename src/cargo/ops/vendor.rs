use crate::core::shell::Verbosity;
use crate::core::{GitReference, Package, Workspace};
use crate::ops;
use crate::sources::path::PathSource;
use crate::sources::CRATES_IO_REGISTRY;
use crate::util::cache_lock::CacheLockMode;
use crate::util::{try_canonicalize, CargoResult, GlobalContext};
use anyhow::{bail, Context as _};
use cargo_util::{paths, Sha256};
use serde::Serialize;
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
    let _lock = gctx.acquire_package_cache_lock(CacheLockMode::MutateExclusive)?;
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

fn sync(
    gctx: &GlobalContext,
    workspaces: &[&Workspace<'_>],
    opts: &VendorOptions<'_>,
) -> CargoResult<VendorConfig> {
    let dry_run = false;
    let canonical_destination = try_canonicalize(opts.destination);
    let canonical_destination = canonical_destination.as_deref().unwrap_or(opts.destination);
    let dest_dir_already_exists = canonical_destination.exists();

    paths::create_dir_all(&canonical_destination)?;
    let mut to_remove = HashSet::new();
    if !opts.no_delete {
        for entry in canonical_destination.read_dir()? {
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

    // First up attempt to work around rust-lang/cargo#5956. Apparently build
    // artifacts sprout up in Cargo's global cache for whatever reason, although
    // it's unsure what tool is causing these issues at this time. For now we
    // apply a heavy-hammer approach which is to delete Cargo's unpacked version
    // of each crate to start off with. After we do this we'll re-resolve and
    // redownload again, which should trigger Cargo to re-extract all the
    // crates.
    //
    // Note that errors are largely ignored here as this is a best-effort
    // attempt. If anything fails here we basically just move on to the next
    // crate to work with.
    for ws in workspaces {
        let (packages, resolve) =
            ops::resolve_ws(ws, dry_run).context("failed to load pkg lockfile")?;

        packages
            .get_many(resolve.iter())
            .context("failed to download packages")?;

        for pkg in resolve.iter() {
            // Don't delete actual source code!
            if pkg.source_id().is_path() {
                if let Ok(path) = pkg.source_id().url().to_file_path() {
                    if let Ok(path) = try_canonicalize(path) {
                        to_remove.remove(&path);
                    }
                }
                continue;
            }
            if pkg.source_id().is_git() {
                continue;
            }
            if let Ok(pkg) = packages.get_one(pkg) {
                drop(fs::remove_dir_all(pkg.root()));
            }
        }
    }

    let mut checksums = HashMap::new();
    let mut ids = BTreeMap::new();

    // Next up let's actually download all crates and start storing internal
    // tables about them.
    for ws in workspaces {
        let (packages, resolve) =
            ops::resolve_ws(ws, dry_run).context("failed to load pkg lockfile")?;

        packages
            .get_many(resolve.iter())
            .context("failed to download packages")?;

        for pkg in resolve.iter() {
            // No need to vendor path crates since they're already in the
            // repository
            if pkg.source_id().is_path() {
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
        let dst = canonical_destination.join(&dst_name);
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
        let pathsource = PathSource::new(src, id.source_id(), gctx);
        let paths = pathsource.list_files(pkg)?;
        let mut map = BTreeMap::new();
        cp_sources(pkg, src, &paths, &dst, &mut map, &mut tmp_buf, gctx)
            .with_context(|| format!("failed to copy over vendored sources for: {}", id))?;

        // Finally, emit the metadata about this package
        let json = serde_json::json!({
            "package": checksums.get(id),
            "files": map,
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
    } else if !dest_dir_already_exists {
        // Nothing to vendor. Remove the destination dir we've just created.
        paths::remove_dir(canonical_destination)?;
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

        match relative.to_str() {
            // Skip git config files as they're not relevant to builds most of
            // the time and if we respect them (e.g.  in git) then it'll
            // probably mess with the checksums when a vendor dir is checked
            // into someone else's source control
            Some(".gitattributes" | ".gitignore" | ".git") => continue,

            // Temporary Cargo files
            Some(".cargo-ok") => continue,

            // Skip patch-style orig/rej files. Published crates on crates.io
            // have `Cargo.toml.orig` which we don't want to use here and
            // otherwise these are rarely used as part of the build process.
            Some(filename) => {
                if filename.ends_with(".orig") || filename.ends_with(".rej") {
                    continue;
                }
            }
            _ => {}
        };

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
                "Generated Cargo.toml",
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
            copy_and_checksum(
                &dst,
                &mut dst_opts,
                &mut src,
                &p.display().to_string(),
                tmp_buf,
            )?
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
    if let Some(cargo_util_schemas::manifest::StringOrBool::String(path)) = &package.build {
        let path = paths::normalize_path(Path::new(path));
        let included = packaged_files.contains(&path);
        let build = if included {
            let path = path
                .into_os_string()
                .into_string()
                .map_err(|_err| anyhow::format_err!("non-UTF8 `package.build`"))?;
            let path = crate::util::toml::normalize_path_string_sep(path);
            cargo_util_schemas::manifest::StringOrBool::String(path)
        } else {
            gctx.shell().warn(format!(
                "ignoring `package.build` as `{}` is not included in the published package",
                path.display()
            ))?;
            cargo_util_schemas::manifest::StringOrBool::Bool(false)
        };
        package.build = Some(build);
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
    contents_path: &str,
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
