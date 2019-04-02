use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str;

use log::{info, trace};
use semver::Version;

use crate::core::dependency::Dependency;
use crate::core::{PackageId, SourceId, Summary};
use crate::sources::registry::RegistryData;
use crate::sources::registry::{RegistryPackage, INDEX_LOCK};
use crate::util::{internal, CargoResult, Config, Filesystem, ToSemver};

/// Crates.io treats hyphen and underscores as interchangeable, but the index and old Cargo do not.
/// Therefore, the index must store uncanonicalized version of the name so old Cargo's can find it.
/// This loop tries all possible combinations of switching hyphen and underscores to find the
/// uncanonicalized one. As all stored inputs have the correct spelling, we start with the spelling
/// as-provided.
struct UncanonicalizedIter<'s> {
    input: &'s str,
    num_hyphen_underscore: u32,
    hyphen_combination_num: u16,
}

impl<'s> UncanonicalizedIter<'s> {
    fn new(input: &'s str) -> Self {
        let num_hyphen_underscore = input.chars().filter(|&c| c == '_' || c == '-').count() as u32;
        UncanonicalizedIter {
            input,
            num_hyphen_underscore,
            hyphen_combination_num: 0,
        }
    }
}

impl<'s> Iterator for UncanonicalizedIter<'s> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.hyphen_combination_num > 0
            && self.hyphen_combination_num.trailing_zeros() >= self.num_hyphen_underscore
        {
            return None;
        }

        let ret = Some(
            self.input
                .chars()
                .scan(0u16, |s, c| {
                    // the check against 15 here's to prevent
                    // shift overflow on inputs with more then 15 hyphens
                    if (c == '_' || c == '-') && *s <= 15 {
                        let switch = (self.hyphen_combination_num & (1u16 << *s)) > 0;
                        let out = if (c == '_') ^ switch { '_' } else { '-' };
                        *s += 1;
                        Some(out)
                    } else {
                        Some(c)
                    }
                })
                .collect(),
        );
        self.hyphen_combination_num += 1;
        ret
    }
}

#[test]
fn no_hyphen() {
    assert_eq!(
        UncanonicalizedIter::new("test").collect::<Vec<_>>(),
        vec!["test".to_string()]
    )
}

#[test]
fn two_hyphen() {
    assert_eq!(
        UncanonicalizedIter::new("te-_st").collect::<Vec<_>>(),
        vec![
            "te-_st".to_string(),
            "te__st".to_string(),
            "te--st".to_string(),
            "te_-st".to_string()
        ]
    )
}

#[test]
fn overflow_hyphen() {
    assert_eq!(
        UncanonicalizedIter::new("te-_-_-_-_-_-_-_-_-st")
            .take(100)
            .count(),
        100
    )
}

pub struct RegistryIndex<'cfg> {
    source_id: SourceId,
    path: Filesystem,
    cache: HashMap<&'static str, Vec<(Summary, bool)>>,
    // `(name, vers)` -> `checksum`
    hashes: HashMap<&'static str, HashMap<Version, String>>,
    config: &'cfg Config,
    locked: bool,
}

impl<'cfg> RegistryIndex<'cfg> {
    pub fn new(
        source_id: SourceId,
        path: &Filesystem,
        config: &'cfg Config,
        locked: bool,
    ) -> RegistryIndex<'cfg> {
        RegistryIndex {
            source_id,
            path: path.clone(),
            cache: HashMap::new(),
            hashes: HashMap::new(),
            config,
            locked,
        }
    }

    /// Returns the hash listed for a specified `PackageId`.
    pub fn hash(&mut self, pkg: PackageId, load: &mut dyn RegistryData) -> CargoResult<String> {
        let name = pkg.name().as_str();
        let version = pkg.version();
        if let Some(s) = self.hashes.get(name).and_then(|v| v.get(version)) {
            return Ok(s.clone());
        }
        // Ok, we're missing the key, so parse the index file to load it.
        self.summaries(name, load)?;
        self.hashes
            .get(name)
            .and_then(|v| v.get(version))
            .ok_or_else(|| internal(format!("no hash listed for {}", pkg)))
            .map(|s| s.clone())
    }

    /// Parses the on-disk metadata for the package provided.
    ///
    /// Returns a list of pairs of `(summary, yanked)` for the package name specified.
    pub fn summaries(
        &mut self,
        name: &'static str,
        load: &mut dyn RegistryData,
    ) -> CargoResult<&Vec<(Summary, bool)>> {
        if self.cache.contains_key(name) {
            return Ok(&self.cache[name]);
        }
        let summaries = self.load_summaries(name, load)?;
        self.cache.insert(name, summaries);
        Ok(&self.cache[name])
    }

    fn load_summaries(
        &mut self,
        name: &str,
        load: &mut dyn RegistryData,
    ) -> CargoResult<Vec<(Summary, bool)>> {
        // Prepare the `RegistryData` which will lazily initialize internal data
        // structures. Note that this is also importantly needed to initialize
        // to avoid deadlocks where we acquire a lock below but the `load`
        // function inside *also* wants to acquire a lock. See an instance of
        // this on #5551.
        load.prepare()?;
        let (root, _lock) = if self.locked {
            let lock = self
                .path
                .open_ro(Path::new(INDEX_LOCK), self.config, "the registry index");
            match lock {
                Ok(lock) => (lock.path().parent().unwrap().to_path_buf(), Some(lock)),
                Err(_) => return Ok(Vec::new()),
            }
        } else {
            (self.path.clone().into_path_unlocked(), None)
        };

        let fs_name = name
            .chars()
            .flat_map(|c| c.to_lowercase())
            .collect::<String>();

        // See module comment for why this is structured the way it is.
        let raw_path = match fs_name.len() {
            1 => format!("1/{}", fs_name),
            2 => format!("2/{}", fs_name),
            3 => format!("3/{}/{}", &fs_name[..1], fs_name),
            _ => format!("{}/{}/{}", &fs_name[0..2], &fs_name[2..4], fs_name),
        };
        let mut ret = Vec::new();
        for path in UncanonicalizedIter::new(&raw_path).take(1024) {
            let mut hit_closure = false;
            let err = load.load(&root, Path::new(&path), &mut |contents| {
                hit_closure = true;
                let contents = str::from_utf8(contents)
                    .map_err(|_| failure::format_err!("registry index file was not valid utf-8"))?;
                ret.reserve(contents.lines().count());
                let lines = contents.lines().map(|s| s.trim()).filter(|l| !l.is_empty());

                // Attempt forwards-compatibility on the index by ignoring
                // everything that we ourselves don't understand, that should
                // allow future cargo implementations to break the
                // interpretation of each line here and older cargo will simply
                // ignore the new lines.
                ret.extend(lines.filter_map(|line| {
                    let (summary, locked) = match self.parse_registry_package(line) {
                        Ok(p) => p,
                        Err(e) => {
                            info!("failed to parse `{}` registry package: {}", name, e);
                            trace!("line: {}", line);
                            return None;
                        }
                    };
                    Some((summary, locked))
                }));

                Ok(())
            });

            // We ignore lookup failures as those are just crates which don't exist
            // or we haven't updated the registry yet. If we actually ran the
            // closure though then we care about those errors.
            if hit_closure {
                err?;
                // Crates.io ensures that there is only one hyphen and underscore equivalent
                // result in the index so return when we find it.
                return Ok(ret);
            }
        }

        Ok(ret)
    }

    /// Parses a line from the registry's index file into a `Summary` for a package.
    ///
    /// The returned boolean is whether or not the summary has been yanked.
    fn parse_registry_package(&mut self, line: &str) -> CargoResult<(Summary, bool)> {
        let RegistryPackage {
            name,
            vers,
            cksum,
            deps,
            features,
            yanked,
            links,
        } = serde_json::from_str(line)?;
        let pkgid = PackageId::new(&name, &vers, self.source_id)?;
        let name = pkgid.name();
        let deps = deps
            .into_iter()
            .map(|dep| dep.into_dep(self.source_id))
            .collect::<CargoResult<Vec<_>>>()?;
        let summary = Summary::new(pkgid, deps, &features, links, false)?;
        let summary = summary.set_checksum(cksum.clone());
        self.hashes
            .entry(name.as_str())
            .or_insert_with(HashMap::new)
            .insert(vers, cksum);
        Ok((summary, yanked.unwrap_or(false)))
    }

    pub fn query_inner(
        &mut self,
        dep: &Dependency,
        load: &mut dyn RegistryData,
        yanked_whitelist: &HashSet<PackageId>,
        f: &mut dyn FnMut(Summary),
    ) -> CargoResult<()> {
        if self.config.cli_unstable().offline {
            if self.query_inner_with_online(dep, load, yanked_whitelist, f, false)? != 0 {
                return Ok(());
            }
            // If offline, and there are no matches, try again with online.
            // This is necessary for dependencies that are not used (such as
            // target-cfg or optional), but are not downloaded. Normally the
            // build should succeed if they are not downloaded and not used,
            // but they still need to resolve. If they are actually needed
            // then cargo will fail to download and an error message
            // indicating that the required dependency is unavailable while
            // offline will be displayed.
        }
        self.query_inner_with_online(dep, load, yanked_whitelist, f, true)?;
        Ok(())
    }

    fn query_inner_with_online(
        &mut self,
        dep: &Dependency,
        load: &mut dyn RegistryData,
        yanked_whitelist: &HashSet<PackageId>,
        f: &mut dyn FnMut(Summary),
        online: bool,
    ) -> CargoResult<usize> {
        let source_id = self.source_id;
        let name = dep.package_name().as_str();
        let summaries = self.summaries(name, load)?;
        let summaries = summaries
            .iter()
            .filter(|&(summary, yanked)| {
                (online || load.is_crate_downloaded(summary.package_id()))
                    && (!yanked || {
                        log::debug!("{:?}", yanked_whitelist);
                        log::debug!("{:?}", summary.package_id());
                        yanked_whitelist.contains(&summary.package_id())
                    })
            })
            .map(|s| s.0.clone());

        // Handle `cargo update --precise` here. If specified, our own source
        // will have a precise version listed of the form
        // `<pkg>=<p_req>o-><f_req>` where `<pkg>` is the name of a crate on
        // this source, `<p_req>` is the version installed and `<f_req> is the
        // version requested (argument to `--precise`).
        let summaries = summaries.filter(|s| match source_id.precise() {
            Some(p) if p.starts_with(name) && p[name.len()..].starts_with('=') => {
                let mut vers = p[name.len() + 1..].splitn(2, "->");
                if dep
                    .version_req()
                    .matches(&vers.next().unwrap().to_semver().unwrap())
                {
                    vers.next().unwrap() == s.version().to_string()
                } else {
                    true
                }
            }
            _ => true,
        });

        let mut count = 0;
        for summary in summaries {
            f(summary);
            count += 1;
        }
        Ok(count)
    }
}
