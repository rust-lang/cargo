use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};

use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::core::PackageId;
use crate::core::{Dependency, Package, Source, SourceId};
use crate::sources::PathSource;
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::{internal, Config, ToSemver};
use crate::util::{FileLock, Filesystem};

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum CrateListing {
    V1(CrateListingV1),
    Empty(Empty),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

#[derive(Deserialize, Serialize)]
pub struct CrateListingV1 {
    v1: BTreeMap<PackageId, BTreeSet<String>>,
}

impl CrateListingV1 {
    pub fn v1(&self) -> &BTreeMap<PackageId, BTreeSet<String>> {
        &self.v1
    }

    pub fn v1_mut(&mut self) -> &mut BTreeMap<PackageId, BTreeSet<String>> {
        &mut self.v1
    }
}

pub fn resolve_root(flag: Option<&str>, config: &Config) -> CargoResult<Filesystem> {
    let config_root = config.get_path("install.root")?;
    Ok(flag
        .map(PathBuf::from)
        .or_else(|| env::var_os("CARGO_INSTALL_ROOT").map(PathBuf::from))
        .or_else(move || config_root.map(|v| v.val))
        .map(Filesystem::new)
        .unwrap_or_else(|| config.home().clone()))
}

pub fn path_source<'a>(source_id: SourceId, config: &'a Config) -> CargoResult<PathSource<'a>> {
    let path = source_id
        .url()
        .to_file_path()
        .map_err(|()| failure::format_err!("path sources must have a valid path"))?;
    Ok(PathSource::new(&path, source_id, config))
}

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

    match name {
        Some(name) => {
            let vers = match vers {
                Some(v) => {
                    // If the version begins with character <, >, =, ^, ~ parse it as a
                    // version range, otherwise parse it as a specific version
                    let first = v.chars().nth(0).ok_or_else(|| {
                        failure::format_err!("no version provided for the `--vers` flag")
                    })?;

                    match first {
                        '<' | '>' | '=' | '^' | '~' => match v.parse::<VersionReq>() {
                            Ok(v) => Some(v.to_string()),
                            Err(_) => failure::bail!(
                                "the `--vers` provided, `{}`, is \
                                       not a valid semver version requirement\n\n
                                       Please have a look at \
                                       https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html \
                                       for the correct format",
                                v
                            ),
                        },
                        _ => match v.to_semver() {
                            Ok(v) => Some(format!("={}", v)),
                            Err(_) => {
                                let mut msg = format!(
                                    "\
                                     the `--vers` provided, `{}`, is \
                                     not a valid semver version\n\n\
                                     historically Cargo treated this \
                                     as a semver version requirement \
                                     accidentally\nand will continue \
                                     to do so, but this behavior \
                                     will be removed eventually",
                                    v
                                );

                                // If it is not a valid version but it is a valid version
                                // requirement, add a note to the warning
                                if v.parse::<VersionReq>().is_ok() {
                                    msg.push_str(&format!(
                                        "\nif you want to specify semver range, \
                                         add an explicit qualifier, like ^{}",
                                        v
                                    ));
                                }
                                config.shell().warn(&msg)?;
                                Some(v.to_string())
                            }
                        },
                    }
                }
                None => None,
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
                },
                None => {
                    let vers_info = vers
                        .map(|v| format!(" with version `{}`", v))
                        .unwrap_or_default();
                    failure::bail!(
                        "could not find `{}` in {}{}",
                        name,
                        source.source_id(),
                        vers_info
                    )
                }
            }
        }
        None => {
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
                    None => failure::bail!(
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
}

pub fn one<I, F>(mut i: I, f: F) -> CargoResult<Option<I::Item>>
where
    I: Iterator,
    F: FnOnce(Vec<I::Item>) -> String,
{
    match (i.next(), i.next()) {
        (Some(i1), Some(i2)) => {
            let mut v = vec![i1, i2];
            v.extend(i);
            Err(failure::format_err!("{}", f(v)))
        }
        (Some(i), None) => Ok(Some(i)),
        (None, _) => Ok(None),
    }
}

pub fn read_crate_list(file: &FileLock) -> CargoResult<CrateListingV1> {
    let listing = (|| -> CargoResult<_> {
        let mut contents = String::new();
        file.file().read_to_string(&mut contents)?;
        let listing =
            toml::from_str(&contents).chain_err(|| internal("invalid TOML found for metadata"))?;
        match listing {
            CrateListing::V1(v1) => Ok(v1),
            CrateListing::Empty(_) => Ok(CrateListingV1 {
                v1: BTreeMap::new(),
            }),
        }
    })()
    .chain_err(|| {
        failure::format_err!(
            "failed to parse crate metadata at `{}`",
            file.path().to_string_lossy()
        )
    })?;
    Ok(listing)
}

pub fn write_crate_list(file: &FileLock, listing: CrateListingV1) -> CargoResult<()> {
    (|| -> CargoResult<_> {
        let mut file = file.file();
        file.seek(SeekFrom::Start(0))?;
        file.set_len(0)?;
        let data = toml::to_string(&CrateListing::V1(listing))?;
        file.write_all(data.as_bytes())?;
        Ok(())
    })()
    .chain_err(|| {
        failure::format_err!(
            "failed to write crate metadata at `{}`",
            file.path().to_string_lossy()
        )
    })?;
    Ok(())
}

pub fn metadata(config: &Config, root: &Filesystem) -> CargoResult<FileLock> {
    root.open_rw(Path::new(".crates.toml"), config, "crate metadata")
}
