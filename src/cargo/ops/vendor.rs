use crate::core::shell::Verbosity;
use crate::core::{GitReference, SourceId, Workspace};
use crate::ops;
use crate::sources::path::PathSource;
use crate::sources::CRATES_IO_REGISTRY;
use crate::util::{self, CargoResult, Config};
use anyhow::Context as _;
use cargo_util::{paths, Sha256};
use serde::Serialize;
use std::collections::HashSet;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use toml_edit::easy as toml;

pub struct VendorOptions<'a> {
    pub no_delete: bool,
    pub versioned_dirs: bool,
    pub destination: &'a Path,
    pub extra: Vec<PathBuf>,
}

pub fn vendor(ws: &Workspace<'_>, opts: &VendorOptions<'_>) -> CargoResult<()> {
    let config = ws.config();
    let mut extra_workspaces = Vec::new();
    for extra in opts.extra.iter() {
        let extra = config.cwd().join(extra);
        let ws = Workspace::new(&extra, config)?;
        extra_workspaces.push(ws);
    }
    let workspaces = extra_workspaces.iter().chain(Some(ws)).collect::<Vec<_>>();
    let vendor_config = sync(config, &workspaces, opts).with_context(|| "failed to sync")?;

    if config.shell().verbosity() != Verbosity::Quiet {
        if vendor_config.source.is_empty() {
            crate::drop_eprintln!(config, "There is no dependency to vendor in this project.");
        } else {
            crate::drop_eprint!(
                config,
                "To use vendored sources, add this to your .cargo/config.toml for this project:\n\n"
            );
            crate::drop_print!(
                config,
                "{}",
                &toml::to_string_pretty(&vendor_config).unwrap()
            );
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
        directory: PathBuf,
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
    config: &Config,
    workspaces: &[&Workspace<'_>],
    opts: &VendorOptions<'_>,
) -> CargoResult<VendorConfig> {
    let canonical_destination = opts.destination.canonicalize();
    let canonical_destination = canonical_destination.as_deref().unwrap_or(opts.destination);
    let dest_dir_already_exists = canonical_destination.exists();

    paths::create_dir_all(&canonical_destination)?;
    let mut to_remove = HashSet::new();
    let mut separate_source_paths = Vec::new();
    if !opts.no_delete {
        for entry in canonical_destination.read_dir()? {
            let entry = entry?;
            let filename = entry.file_name();
            let filename = filename.to_str();
            // Skip hidden files. See rust-lang/cargo#7109
            if filename.map_or(false, |s| s.starts_with('.')) {
                continue;
            }
            let path = entry.path();
            if filename.map_or(false, |s| s.starts_with('@')) {
                // Remove crates from separate source directories.
                for entry in path.read_dir()? {
                    let entry = entry?;
                    to_remove.insert(entry.path());
                }
                separate_source_paths.push(path);
            } else {
                // Remove crates from top level source directory.
                to_remove.insert(path);
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
            ops::resolve_ws(ws).with_context(|| "failed to load pkg lockfile")?;

        packages
            .get_many(resolve.iter())
            .with_context(|| "failed to download packages")?;

        for pkg in resolve.iter() {
            // Don't delete actual source code!
            if pkg.source_id().is_path() {
                if let Ok(path) = pkg.source_id().url().to_file_path() {
                    if let Ok(path) = path.canonicalize() {
                        to_remove.remove(&path);
                    }
                }
                continue;
            }
            if pkg.source_id().is_git() {
                continue;
            }
            if let Ok(pkg) = packages.get_one(pkg) {
                drop(fs::remove_dir_all(pkg.manifest_path().parent().unwrap()));
            }
        }
    }

    let mut checksums = HashMap::new();
    let mut ids = BTreeMap::new();

    // Next up let's actually download all crates and start storing internal
    // tables about them.
    for ws in workspaces {
        let (packages, resolve) =
            ops::resolve_ws(ws).with_context(|| "failed to load pkg lockfile")?;

        packages
            .get_many(resolve.iter())
            .with_context(|| "failed to download packages")?;

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
                    .with_context(|| "failed to fetch package")?
                    .clone(),
            );

            checksums.insert(pkg, resolve.checksums().get(&pkg).cloned());
        }
    }

    let (versions, major_source) = {
        let mut contains_duplicates = false;
        let mut versions = HashMap::new();
        for id in ids.keys() {
            let map = versions.entry(id.name()).or_insert_with(BTreeMap::default);
            use std::collections::btree_map::Entry;
            match map.entry(id.version()) {
                Entry::Vacant(entry) => {
                    entry.insert(id.source_id());
                }
                Entry::Occupied(_) => {
                    contains_duplicates |= true;
                }
            }
        }
        let major_source = contains_duplicates.then(|| {
            let mut counter = BTreeMap::new();
            for id in ids.keys() {
                counter
                    .entry(id.source_id())
                    .and_modify(|e| *e += 1)
                    .or_insert(1);
            }
            counter
                .into_iter()
                .max_by_key(|(_, v)| *v)
                .map(|(src, _)| src)
                .expect("no source")
        });
        (versions, major_source)
    };

    let mut sources = BTreeSet::new();
    let mut tmp_buf = [0; 64 * 1024];
    let mut source_paths = HashMap::new();
    for (id, pkg) in ids.iter() {
        // Next up, copy it to the vendor directory
        let src = pkg
            .manifest_path()
            .parent()
            .expect("manifest_path should point to a file");
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

        let dst = source_paths
            .entry(src)
            .or_insert_with(|| {
                vendor_path_for_source(canonical_destination, &id.source_id(), &major_source)
            })
            .join(&dst_name);

        to_remove.remove(&dst);

        let cksum = dst.join(".cargo-checksum.json");
        if dir_has_version_suffix && cksum.exists() {
            // Always re-copy directory without version suffix in case the version changed
            continue;
        }

        config.shell().status(
            "Vendoring",
            &format!("{} ({}) to {}", id, src.to_string_lossy(), dst.display()),
        )?;

        let _ = fs::remove_dir_all(&dst);
        let pathsource = PathSource::new(src, id.source_id(), config);
        let paths = pathsource.list_files(pkg)?;
        let mut map = BTreeMap::new();
        cp_sources(src, &paths, &dst, &mut map, &mut tmp_buf)
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
    for path in separate_source_paths {
        // Cleanup empty source directory.
        if path.read_dir()?.next().is_none() {
            paths::remove_dir(path)?;
        }
    }

    // add our vendored source
    let mut config = BTreeMap::new();

    let merged_source_name = "vendored-sources";

    // replace original sources with vendor
    for source_id in sources {
        let name = if source_id.is_default_registry() {
            CRATES_IO_REGISTRY.to_string()
        } else {
            source_id.url().to_string()
        };
        let replace_with = if should_merge_into_top(&source_id, &major_source) {
            merged_source_name.to_string()
        } else {
            format!("vendor+{name}")
        };

        config.entry(replace_with.clone()).or_insert_with(|| {
            let directory = vendor_path_for_source(opts.destination, &source_id, &major_source);
            VendorSource::Directory { directory }
        });

        let source = if source_id.is_default_registry() {
            VendorSource::Registry {
                registry: None,
                replace_with,
            }
        } else if source_id.is_remote_registry() {
            let registry = source_id.url().to_string();
            VendorSource::Registry {
                registry: Some(registry),
                replace_with,
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
                replace_with,
            }
        } else {
            panic!("Invalid source ID: {}", source_id)
        };
        config.insert(name, source);
    }

    if config.is_empty() && !dest_dir_already_exists {
        // Nothing to vendor. Remove the destination dir we've just created.
        paths::remove_dir(canonical_destination)?;
    }

    Ok(VendorConfig { source: config })
}

fn cp_sources(
    src: &Path,
    paths: &[PathBuf],
    dst: &Path,
    cksums: &mut BTreeMap<String, String>,
    tmp_buf: &mut [u8],
) -> CargoResult<()> {
    for p in paths {
        let relative = p.strip_prefix(&src).unwrap();

        match relative.to_str() {
            // Skip git config files as they're not relevant to builds most of
            // the time and if we respect them (e.g.  in git) then it'll
            // probably mess with the checksums when a vendor dir is checked
            // into someone else's source control
            Some(".gitattributes") | Some(".gitignore") | Some(".git") => continue,

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

        let cksum = copy_and_checksum(p, &dst, tmp_buf)?;
        cksums.insert(relative.to_str().unwrap().replace("\\", "/"), cksum);
    }
    Ok(())
}

fn copy_and_checksum(src_path: &Path, dst_path: &Path, buf: &mut [u8]) -> CargoResult<String> {
    let mut src = File::open(src_path).with_context(|| format!("failed to open {:?}", src_path))?;
    let mut dst_opts = OpenOptions::new();
    dst_opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let src_metadata = src
            .metadata()
            .with_context(|| format!("failed to stat {:?}", src_path))?;
        dst_opts.mode(src_metadata.mode());
    }
    let mut dst = dst_opts
        .open(dst_path)
        .with_context(|| format!("failed to create {:?}", dst_path))?;
    // Not going to bother setting mode on pre-existing files, since there
    // shouldn't be any under normal conditions.
    let mut cksum = Sha256::new();
    loop {
        let n = src
            .read(buf)
            .with_context(|| format!("failed to read from {:?}", src_path))?;
        if n == 0 {
            break Ok(cksum.finish_hex());
        }
        let data = &buf[..n];
        cksum.update(data);
        dst.write_all(data)
            .with_context(|| format!("failed to write to {:?}", dst_path))?;
    }
}

/// The absent of major source implies there is no duplicate versions from
/// different sources, so cargo are happy to merge all sources into top level
/// vendor directory.
fn should_merge_into_top(source: &SourceId, major: &Option<SourceId>) -> bool {
    major.as_ref().map_or(true, |major| source == major)
}

/// Determines where should the vendor crates from the source go.
///
/// The layout of vendor directory depends on whether there is any crate with
/// duplicate versions from different sources:
///
/// - If no, cargo simply merges crates from all sources into the top level of
///   vendor directory.
/// - If duplicates exist, the source of majority remains at the top, but the
///   others are relevated to separate directories prefixed with `@`. The
///   reason to prefix is to avoid name conflicts with valid crate names.
///
/// ```text
/// vendor/
/// ├── log/ # Crates at the top are from the source of majority
/// ├── serde/
/// ├── cfg-if/
/// └── @git-1936cea8af2a1111/ # git source
/// │  └── serde/
/// └── @registry-1ecc6299db9ec823/ # registry source
///    └── cfg-if/
/// ```
fn vendor_path_for_source(path: &Path, source: &SourceId, major: &Option<SourceId>) -> PathBuf {
    fn source_id_to_dir_name(src_id: SourceId) -> String {
        let src_type = if src_id.is_registry() {
            "registry"
        } else if src_id.is_git() {
            "git"
        } else {
            panic!("Invalid source ID: {src_id}")
        };
        struct SourceIdShortHash(SourceId);
        impl Hash for SourceIdShortHash {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.stable_hash(Path::new(""), state);
            }
        }
        let hash = util::short_hash(&SourceIdShortHash(src_id));
        format!("@{src_type}-{hash}")
    }

    let mut path = path.to_path_buf();
    if !should_merge_into_top(source, major) {
        // Sources other than the majority source go into their own directories.
        path.push(source_id_to_dir_name(*source));
    }
    path
}
