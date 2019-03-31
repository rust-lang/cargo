use std::collections::{btree_map, BTreeMap, BTreeSet};
use std::env;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};

use failure::{bail, format_err};
use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::core::{Dependency, Package, PackageId, Source, SourceId};
use crate::ops::{self, CompileFilter, CompileOptions};
use crate::sources::PathSource;
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::{Config, ToSemver};
use crate::util::{FileLock, Filesystem, Freshness};

/// On-disk tracking for which package installed which binary.
///
/// v1 is an older style, v2 is a new (experimental) style that tracks more
/// information. The new style is only enabled with the `-Z install-upgrade`
/// flag (which sets the `unstable_upgrade` flag). v1 is still considered the
/// source of truth. When v2 is used, it will sync with any changes with v1,
/// and will continue to update v1.
///
/// This maintains a filesystem lock, preventing other instances of Cargo from
/// modifying at the same time. Drop the value to unlock.
pub struct InstallTracker {
    v1: CrateListingV1,
    v2: CrateListingV2,
    v1_lock: FileLock,
    v2_lock: Option<FileLock>,
    unstable_upgrade: bool,
}

#[derive(Default, Deserialize, Serialize)]
struct CrateListingV2 {
    installs: BTreeMap<PackageId, InstallInfo>,
    /// Forwards compatibility.
    #[serde(flatten)]
    other: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
struct InstallInfo {
    /// Version requested via `--version`.
    /// None if `--version` not specified. Currently not used, possibly may be
    /// used in the future.
    version_req: Option<String>,
    /// Set of binary names installed.
    bins: BTreeSet<String>,
    /// Set of features explicitly enabled.
    features: BTreeSet<String>,
    all_features: bool,
    no_default_features: bool,
    /// Either "debug" or "release".
    profile: String,
    /// The installation target.
    /// Either the host or the value specified in `--target`.
    /// None if unknown (when loading from v1).
    target: Option<String>,
    /// Output of `rustc -V`.
    /// None if unknown (when loading from v1).
    /// Currently not used, possibly may be used in the future.
    rustc: Option<String>,
    /// Forwards compatibility.
    #[serde(flatten)]
    other: BTreeMap<String, serde_json::Value>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct CrateListingV1 {
    v1: BTreeMap<PackageId, BTreeSet<String>>,
}

impl InstallTracker {
    /// Create an InstallTracker from information on disk.
    pub fn load(config: &Config, root: &Filesystem) -> CargoResult<InstallTracker> {
        let unstable_upgrade = config.cli_unstable().install_upgrade;
        let v1_lock = root.open_rw(Path::new(".crates.toml"), config, "crate metadata")?;
        let v2_lock = if unstable_upgrade {
            Some(root.open_rw(Path::new(".crates2.json"), config, "crate metadata")?)
        } else {
            None
        };

        let v1 = (|| -> CargoResult<_> {
            let mut contents = String::new();
            v1_lock.file().read_to_string(&mut contents)?;
            if contents.is_empty() {
                Ok(CrateListingV1::default())
            } else {
                Ok(toml::from_str(&contents)
                    .chain_err(|| format_err!("invalid TOML found for metadata"))?)
            }
        })()
        .chain_err(|| {
            format_err!(
                "failed to parse crate metadata at `{}`",
                v1_lock.path().to_string_lossy()
            )
        })?;

        let v2 = (|| -> CargoResult<_> {
            if unstable_upgrade {
                let mut contents = String::new();
                v2_lock
                    .as_ref()
                    .unwrap()
                    .file()
                    .read_to_string(&mut contents)?;
                let mut v2 = if contents.is_empty() {
                    CrateListingV2::default()
                } else {
                    serde_json::from_str(&contents)
                        .chain_err(|| format_err!("invalid JSON found for metadata"))?
                };
                v2.sync_v1(&v1)?;
                Ok(v2)
            } else {
                Ok(CrateListingV2::default())
            }
        })()
        .chain_err(|| {
            format_err!(
                "failed to parse crate metadata at `{}`",
                v2_lock.as_ref().unwrap().path().to_string_lossy()
            )
        })?;

        Ok(InstallTracker {
            v1,
            v2,
            v1_lock,
            v2_lock,
            unstable_upgrade,
        })
    }

    /// Checks if the given package should be built, and checks if executables
    /// already exist in the destination directory.
    ///
    /// Returns a tuple `(freshness, map)`. `freshness` indicates if the
    /// package should be built (`Dirty`) or if it is already up-to-date
    /// (`Fresh`). The map maps binary names to the PackageId that installed
    /// it (which is None if not known).
    ///
    /// Returns an error if there is a duplicate and `--force` is not used.
    pub fn check_upgrade(
        &self,
        dst: &Path,
        pkg: &Package,
        force: bool,
        opts: &CompileOptions<'_>,
        target: &str,
        _rustc: &str,
    ) -> CargoResult<(Freshness, BTreeMap<String, Option<PackageId>>)> {
        let exes = exe_names(pkg, &opts.filter);
        let duplicates = self.find_duplicates(dst, &exes);
        if force || duplicates.is_empty() {
            return Ok((Freshness::Dirty, duplicates));
        }
        // If any duplicates are not tracked, then --force is required.
        // If any duplicates are from a package with a different name, --force is required.
        let matching_duplicates: Vec<PackageId> = duplicates
            .values()
            .filter_map(|v| match v {
                Some(dupe_pkg_id) if dupe_pkg_id.name() == pkg.name() => Some(*dupe_pkg_id),
                _ => None,
            })
            .collect();

        if self.unstable_upgrade && matching_duplicates.len() == duplicates.len() {
            let source_id = pkg.package_id().source_id();
            if source_id.is_path() {
                return Ok((Freshness::Dirty, duplicates));
            }
            if matching_duplicates.iter().all(|dupe_pkg_id| {
                let info = self
                    .v2
                    .installs
                    .get(dupe_pkg_id)
                    .expect("dupes must be in sync");
                let precise_equal = if source_id.is_git() {
                    dupe_pkg_id.source_id().precise() == source_id.precise()
                } else {
                    true
                };

                dupe_pkg_id.version() == pkg.version()
                    && dupe_pkg_id.source_id() == source_id
                    && precise_equal
                    && info.features == feature_set(&opts.features)
                    && info.all_features == opts.all_features
                    && info.no_default_features == opts.no_default_features
                    && info.profile == profile_name(opts.build_config.release)
                    && (info.target.is_none()
                        || info.target.as_ref().map(|t| t.as_ref()) == Some(target))
                    && info.bins == exes
            }) {
                Ok((Freshness::Fresh, duplicates))
            } else {
                Ok((Freshness::Dirty, duplicates))
            }
        } else {
            // Format the error message.
            let mut msg = String::new();
            for (bin, p) in duplicates.iter() {
                msg.push_str(&format!("binary `{}` already exists in destination", bin));
                if let Some(p) = p.as_ref() {
                    msg.push_str(&format!(" as part of `{}`\n", p));
                } else {
                    msg.push_str("\n");
                }
            }
            msg.push_str("Add --force to overwrite");
            bail!("{}", msg);
        }
    }

    fn find_duplicates(
        &self,
        dst: &Path,
        exes: &BTreeSet<String>,
    ) -> BTreeMap<String, Option<PackageId>> {
        exes.iter()
            .filter_map(|name| {
                if !dst.join(&name).exists() {
                    None
                } else if self.unstable_upgrade {
                    let p = self.v2.package_for_bin(name);
                    Some((name.clone(), p))
                } else {
                    let p = self.v1.package_for_bin(name);
                    Some((name.clone(), p))
                }
            })
            .collect()
    }

    /// Mark that a package was installed.
    pub fn mark_installed(
        &mut self,
        package: &Package,
        bins: &BTreeSet<String>,
        version_req: Option<String>,
        opts: &CompileOptions<'_>,
        target: String,
        rustc: String,
    ) {
        if self.unstable_upgrade {
            self.v2
                .mark_installed(package, bins, version_req, opts, target, rustc)
        }
        self.v1.mark_installed(package, bins);
    }

    /// Save tracking information to disk.
    pub fn save(&self) -> CargoResult<()> {
        self.v1.save(&self.v1_lock).chain_err(|| {
            format_err!(
                "failed to write crate metadata at `{}`",
                self.v1_lock.path().to_string_lossy()
            )
        })?;

        if self.unstable_upgrade {
            self.v2.save(self.v2_lock.as_ref().unwrap()).chain_err(|| {
                format_err!(
                    "failed to write crate metadata at `{}`",
                    self.v2_lock.as_ref().unwrap().path().to_string_lossy()
                )
            })?;
        }
        Ok(())
    }

    /// Iterator of all installed binaries.
    /// Items are `(pkg_id, bins)` where `bins` is the set of binaries that
    /// package installed.
    pub fn all_installed_bins(&self) -> impl Iterator<Item = (&PackageId, &BTreeSet<String>)> {
        self.v1.v1.iter()
    }

    /// Set of binaries installed by a particular package.
    /// Returns None if the package is not installed.
    pub fn installed_bins(&self, pkg_id: PackageId) -> Option<&BTreeSet<String>> {
        self.v1.v1.get(&pkg_id)
    }

    /// Remove a package from the tracker.
    pub fn remove(&mut self, pkg_id: PackageId, bins: &BTreeSet<String>) {
        self.v1.remove(pkg_id, bins);
        if self.unstable_upgrade {
            self.v2.remove(pkg_id, bins);
        }
    }
}

impl CrateListingV1 {
    fn package_for_bin(&self, bin_name: &str) -> Option<PackageId> {
        self.v1
            .iter()
            .find(|(_, bins)| bins.contains(bin_name))
            .map(|(pkg_id, _)| *pkg_id)
    }

    fn mark_installed(&mut self, pkg: &Package, bins: &BTreeSet<String>) {
        // Remove bins from any other packages.
        for other_bins in self.v1.values_mut() {
            for bin in bins {
                other_bins.remove(bin);
            }
        }
        // Remove empty metadata lines. If only BTreeMap had `retain`.
        let to_remove = self
            .v1
            .iter()
            .filter_map(|(&p, set)| if set.is_empty() { Some(p) } else { None })
            .collect::<Vec<_>>();
        for p in to_remove.iter() {
            self.v1.remove(p);
        }
        // Add these bins.
        let mut bins = bins.clone();
        self.v1
            .entry(pkg.package_id())
            .and_modify(|set| set.append(&mut bins))
            .or_insert(bins);
    }

    fn remove(&mut self, pkg_id: PackageId, bins: &BTreeSet<String>) {
        let mut installed = match self.v1.entry(pkg_id) {
            btree_map::Entry::Occupied(e) => e,
            btree_map::Entry::Vacant(..) => panic!("v1 unexpected missing `{}`", pkg_id),
        };

        for bin in bins {
            installed.get_mut().remove(bin);
        }
        if installed.get().is_empty() {
            installed.remove();
        }
    }

    fn save(&self, lock: &FileLock) -> CargoResult<()> {
        let mut file = lock.file();
        file.seek(SeekFrom::Start(0))?;
        file.set_len(0)?;
        let data = toml::to_string(self)?;
        file.write_all(data.as_bytes())?;
        Ok(())
    }
}

impl CrateListingV2 {
    fn sync_v1(&mut self, v1: &CrateListingV1) -> CargoResult<()> {
        // Make the `bins` entries the same.
        for (pkg_id, bins) in &v1.v1 {
            self.installs
                .entry(*pkg_id)
                .and_modify(|info| info.bins = bins.clone())
                .or_insert_with(|| InstallInfo::from_v1(bins));
        }
        // Remove any packages that aren't present in v1.
        let to_remove: Vec<_> = self
            .installs
            .keys()
            .filter(|pkg_id| !v1.v1.contains_key(pkg_id))
            .cloned()
            .collect();
        for pkg_id in to_remove {
            self.installs.remove(&pkg_id);
        }
        Ok(())
    }

    fn package_for_bin(&self, bin_name: &str) -> Option<PackageId> {
        self.installs
            .iter()
            .find(|(_, info)| info.bins.contains(bin_name))
            .map(|(pkg_id, _)| *pkg_id)
    }

    fn mark_installed(
        &mut self,
        pkg: &Package,
        bins: &BTreeSet<String>,
        version_req: Option<String>,
        opts: &CompileOptions<'_>,
        target: String,
        rustc: String,
    ) {
        // Remove bins from any other packages.
        for info in &mut self.installs.values_mut() {
            for bin in bins {
                info.bins.remove(bin);
            }
        }
        // Remove empty metadata lines. If only BTreeMap had `retain`.
        let to_remove = self
            .installs
            .iter()
            .filter_map(|(&p, info)| if info.bins.is_empty() { Some(p) } else { None })
            .collect::<Vec<_>>();
        for p in to_remove.iter() {
            self.installs.remove(p);
        }
        // Add these bins.
        if let Some(info) = self.installs.get_mut(&pkg.package_id()) {
            for bin in bins {
                info.bins.remove(bin);
            }
            info.version_req = version_req;
            info.features = feature_set(&opts.features);
            info.all_features = opts.all_features;
            info.no_default_features = opts.no_default_features;
            info.profile = profile_name(opts.build_config.release).to_string();
            info.target = Some(target);
            info.rustc = Some(rustc);
        } else {
            self.installs.insert(
                pkg.package_id(),
                InstallInfo {
                    version_req,
                    bins: bins.clone(),
                    features: feature_set(&opts.features),
                    all_features: opts.all_features,
                    no_default_features: opts.no_default_features,
                    profile: profile_name(opts.build_config.release).to_string(),
                    target: Some(target),
                    rustc: Some(rustc),
                    other: BTreeMap::new(),
                },
            );
        }
    }

    fn remove(&mut self, pkg_id: PackageId, bins: &BTreeSet<String>) {
        let mut info_entry = match self.installs.entry(pkg_id) {
            btree_map::Entry::Occupied(e) => e,
            btree_map::Entry::Vacant(..) => panic!("v2 unexpected missing `{}`", pkg_id),
        };

        for bin in bins {
            info_entry.get_mut().bins.remove(bin);
        }
        if info_entry.get().bins.is_empty() {
            info_entry.remove();
        }
    }

    fn save(&self, lock: &FileLock) -> CargoResult<()> {
        let mut file = lock.file();
        file.seek(SeekFrom::Start(0))?;
        file.set_len(0)?;
        serde_json::to_writer(file, self)?;
        Ok(())
    }
}

impl InstallInfo {
    fn from_v1(set: &BTreeSet<String>) -> InstallInfo {
        InstallInfo {
            version_req: None,
            bins: set.clone(),
            features: BTreeSet::new(),
            all_features: false,
            no_default_features: false,
            profile: "release".to_string(),
            target: None,
            rustc: None,
            other: BTreeMap::new(),
        }
    }
}

/// Determines the root directory where installation is done.
pub fn resolve_root(flag: Option<&str>, config: &Config) -> CargoResult<Filesystem> {
    let config_root = config.get_path("install.root")?;
    Ok(flag
        .map(PathBuf::from)
        .or_else(|| env::var_os("CARGO_INSTALL_ROOT").map(PathBuf::from))
        .or_else(move || config_root.map(|v| v.val))
        .map(Filesystem::new)
        .unwrap_or_else(|| config.home().clone()))
}

/// Determines the `PathSource` from a `SourceId`.
pub fn path_source<'a>(source_id: SourceId, config: &'a Config) -> CargoResult<PathSource<'a>> {
    let path = source_id
        .url()
        .to_file_path()
        .map_err(|()| format_err!("path sources must have a valid path"))?;
    Ok(PathSource::new(&path, source_id, config))
}

/// Gets a Package based on command-line requirements.
pub fn select_pkg<'a, T>(
    mut source: T,
    name: Option<&str>,
    vers: Option<&str>,
    config: &Config,
    needs_update: bool,
    list_all: &mut dyn FnMut(&mut T) -> CargoResult<Vec<Package>>,
) -> CargoResult<(Package, Box<dyn Source + 'a>)>
where
    T: Source + 'a,
{
    if needs_update {
        source.update()?;
    }

    if let Some(name) = name {
        let vers = if let Some(v) = vers {
            // If the version begins with character <, >, =, ^, ~ parse it as a
            // version range, otherwise parse it as a specific version
            let first = v
                .chars()
                .nth(0)
                .ok_or_else(|| format_err!("no version provided for the `--vers` flag"))?;

            let is_req = "<>=^~".contains(first) || v.contains('*');
            if is_req {
                match v.parse::<VersionReq>() {
                    Ok(v) => Some(v.to_string()),
                    Err(_) => bail!(
                        "the `--vers` provided, `{}`, is \
                         not a valid semver version requirement\n\n\
                         Please have a look at \
                         https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html \
                         for the correct format",
                        v
                    ),
                }
            } else {
                match v.to_semver() {
                    Ok(v) => Some(format!("={}", v)),
                    Err(e) => {
                        let mut msg = if config.cli_unstable().install_upgrade {
                            format!(
                                "the `--vers` provided, `{}`, is \
                                 not a valid semver version: {}\n",
                                v, e
                            )
                        } else {
                            format!(
                                "the `--vers` provided, `{}`, is \
                                 not a valid semver version\n\n\
                                 historically Cargo treated this \
                                 as a semver version requirement \
                                 accidentally\nand will continue \
                                 to do so, but this behavior \
                                 will be removed eventually",
                                v
                            )
                        };

                        // If it is not a valid version but it is a valid version
                        // requirement, add a note to the warning
                        if v.parse::<VersionReq>().is_ok() {
                            msg.push_str(&format!(
                                "\nif you want to specify semver range, \
                                 add an explicit qualifier, like ^{}",
                                v
                            ));
                        }
                        if config.cli_unstable().install_upgrade {
                            bail!(msg);
                        } else {
                            config.shell().warn(&msg)?;
                        }
                        Some(v.to_string())
                    }
                }
            }
        } else {
            None
        };
        let vers = vers.as_ref().map(|s| &**s);
        let vers_spec = if vers.is_none() && source.source_id().is_registry() {
            // Avoid pre-release versions from crate.io
            // unless explicitly asked for
            Some("*")
        } else {
            vers
        };
        let dep = Dependency::parse_no_deprecated(name, vers_spec, source.source_id())?;
        let deps = source.query_vec(&dep)?;
        match deps.iter().map(|p| p.package_id()).max() {
            Some(pkgid) => {
                let pkg = Box::new(&mut source).download_now(pkgid, config)?;
                Ok((pkg, Box::new(source)))
            }
            None => {
                let vers_info = vers
                    .map(|v| format!(" with version `{}`", v))
                    .unwrap_or_default();
                bail!(
                    "could not find `{}` in {}{}",
                    name,
                    source.source_id(),
                    vers_info
                )
            }
        }
    } else {
        let candidates = list_all(&mut source)?;
        let binaries = candidates
            .iter()
            .filter(|cand| cand.targets().iter().filter(|t| t.is_bin()).count() > 0);
        let examples = candidates
            .iter()
            .filter(|cand| cand.targets().iter().filter(|t| t.is_example()).count() > 0);
        let pkg = match one(binaries, |v| multi_err("binaries", v))? {
            Some(p) => p,
            None => match one(examples, |v| multi_err("examples", v))? {
                Some(p) => p,
                None => bail!(
                    "no packages found with binaries or \
                     examples"
                ),
            },
        };
        return Ok((pkg.clone(), Box::new(source)));

        fn multi_err(kind: &str, mut pkgs: Vec<&Package>) -> String {
            pkgs.sort_unstable_by_key(|a| a.name());
            format!(
                "multiple packages with {} found: {}",
                kind,
                pkgs.iter()
                    .map(|p| p.name().as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

/// Get one element from the iterator.
/// Returns None if none left.
/// Returns error if there is more than one item in the iterator.
fn one<I, F>(mut i: I, f: F) -> CargoResult<Option<I::Item>>
where
    I: Iterator,
    F: FnOnce(Vec<I::Item>) -> String,
{
    match (i.next(), i.next()) {
        (Some(i1), Some(i2)) => {
            let mut v = vec![i1, i2];
            v.extend(i);
            Err(format_err!("{}", f(v)))
        }
        (Some(i), None) => Ok(Some(i)),
        (None, _) => Ok(None),
    }
}

fn profile_name(release: bool) -> &'static str {
    if release {
        "release"
    } else {
        "dev"
    }
}

/// Helper to convert features Vec to a BTreeSet.
fn feature_set(features: &[String]) -> BTreeSet<String> {
    features.iter().cloned().collect()
}

/// Helper to get the executable names from a filter.
pub fn exe_names(pkg: &Package, filter: &ops::CompileFilter) -> BTreeSet<String> {
    let to_exe = |name| format!("{}{}", name, env::consts::EXE_SUFFIX);
    match filter {
        CompileFilter::Default { .. } => pkg
            .targets()
            .iter()
            .filter(|t| t.is_bin())
            .map(|t| to_exe(t.name()))
            .collect(),
        CompileFilter::Only {
            ref bins,
            ref examples,
            ..
        } => {
            let all_bins: Vec<String> = bins.try_collect().unwrap_or_else(|| {
                pkg.targets()
                    .iter()
                    .filter(|t| t.is_bin())
                    .map(|t| t.name().to_string())
                    .collect()
            });
            let all_examples: Vec<String> = examples.try_collect().unwrap_or_else(|| {
                pkg.targets()
                    .iter()
                    .filter(|t| t.is_exe_example())
                    .map(|t| t.name().to_string())
                    .collect()
            });

            all_bins
                .iter()
                .chain(all_examples.iter())
                .map(|name| to_exe(name))
                .collect()
        }
    }
}
