//! Definition of how to encode a `Resolve` into a TOML `Cargo.lock` file
//!
//! This module contains all machinery necessary to parse a `Resolve` from a
//! `Cargo.lock` as well as serialize a `Resolve` to a `Cargo.lock`.
//!
//! ## Changing `Cargo.lock`
//!
//! In general Cargo is quite conservative about changing the format of
//! `Cargo.lock`. Usage of new features in Cargo can change `Cargo.lock` at any
//! time, but otherwise changing the serialization of `Cargo.lock` is a
//! difficult operation to do that we typically avoid.
//!
//! The main problem with changing the format of `Cargo.lock` is that it can
//! cause quite a bad experience for end users who use different versions of
//! Cargo. If every PR to a project oscillates between the stable channel's
//! encoding of Cargo.lock and the nightly channel's encoding then that's a
//! pretty bad experience.
//!
//! We do, however, want to change `Cargo.lock` over time. (and we have!). To do
//! this the rules that we currently have are:
//!
//! * Add support for the new format to Cargo. This involves code changes in
//!   Cargo itself, likely by adding a new variant of `ResolveVersion` and
//!   branching on that where necessary. This is accompanied with tests in the
//!   `lockfile_compat` module.
//!
//!   * Do not update `ResolveVersion::default()`. The new lockfile format will
//!     not be used yet.
//!
//!   * Preserve the new format if found. This means that if Cargo finds the new
//!     version it'll keep using it, but otherwise it continues to use whatever
//!     format it previously found.
//!
//! * Wait a "long time". This is at least until the changes here hit stable
//!   Rust. Often though we wait a little longer to let the changes percolate
//!   into one or two older stable releases.
//!
//! * Change the return value of `ResolveVersion::default()` to the new format.
//!   This will cause new lock files to use the latest encoding as well as
//!   causing any operation which updates the lock file to update to the new
//!   format.
//!
//! This migration scheme in general means that Cargo we'll get *support* for a
//! new format into Cargo ASAP, but it won't be exercised yet (except in Cargo's
//! own tests). Eventually when stable/beta/nightly all have support for the new
//! format (and maybe a few previous stable versions) we flip the switch.
//! Projects on nightly will quickly start seeing changes, but
//! stable/beta/nightly will all understand this new format and will preserve
//! it.
//!
//! While this does mean that projects' `Cargo.lock` changes over time, it's
//! typically a pretty minimal effort change that's just "check in what's
//! there".
//!
//! ## Historical changes to `Cargo.lock`
//!
//! Listed from most recent to oldest, these are some of the changes we've made
//! to `Cargo.lock`'s serialization format:
//!
//! * A `version` marker is now at the top of the lock file which is a way for
//!   super-old Cargos (at least since this was implemented) to give a formal
//!   error if they see a lock file from a super-future Cargo. Additionally as
//!   part of this change the encoding of `git` dependencies in lock files
//!   changed where `branch = "master"` is now encoded with `branch=master`
//!   instead of with nothing at all.
//!
//! * The entries in `dependencies` arrays have been shortened and the
//!   `checksum` field now shows up directly in `[[package]]` instead of always
//!   at the end of the file. The goal of this change was to ideally reduce
//!   merge conflicts being generated on `Cargo.lock`. Updating a version of a
//!   package now only updates two lines in the file, the checksum and the
//!   version number, most of the time. Dependency edges are specified in a
//!   compact form where possible where just the name is listed. The
//!   version/source on dependency edges are only listed if necessary to
//!   disambiguate which version or which source is in use.
//!
//! * A comment at the top of the file indicates that the file is a generated
//!   file and contains the special symbol `@generated` to indicate to common
//!   review tools that it's a generated file.
//!
//! * A `[root]` entry for the "root crate" has been removed and instead now
//!   included in `[[package]]` like everything else.
//!
//! * All packages from registries contain a `checksum` which is a sha256
//!   checksum of the tarball the package is associated with. This is all stored
//!   in the `[metadata]` table of `Cargo.lock` which all versions of Cargo
//!   since 1.0 have preserved. The goal of this was to start recording
//!   checksums so mirror sources can be verified.
//!
//! ## Other oddities about `Cargo.lock`
//!
//! There's a few other miscellaneous weird things about `Cargo.lock` that you
//! may want to be aware of when reading this file:
//!
//! * All packages have a `source` listed to indicate where they come from. For
//!   `path` dependencies, however, no `source` is listed. There's no way we
//!   could emit a filesystem path name and have that be portable across
//!   systems, so all packages from a `path` are not listed with a `source`.
//!   Note that this also means that all packages with `path` sources must have
//!   unique names.
//!
//! * The `[metadata]` table in `Cargo.lock` is intended to be a generic mapping
//!   of strings to strings that's simply preserved by Cargo. This was a very
//!   early effort to be forward compatible against changes to `Cargo.lock`'s
//!   format. This is nowadays sort of deemed a bad idea though and we don't
//!   really use it that much except for `checksum`s historically. It's not
//!   really recommended to use this.
//!
//! * The actual literal on-disk serialiation is found in
//!   `src/cargo/ops/lockfile.rs` which basically renders a `toml::Value` in a
//!   special fashion to make sure we have strict control over the on-disk
//!   format.

use super::{Resolve, ResolveVersion};
use crate::core::{Dependency, GitReference, Package, PackageId, SourceId, Workspace};
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::util::{internal, Graph};
use anyhow::{bail, Context as _};
use serde::de;
use serde::ser;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::str::FromStr;
use tracing::debug;

/// The `Cargo.lock` structure.
#[derive(Serialize, Deserialize, Debug)]
pub struct EncodableResolve {
    version: Option<u32>,
    package: Option<Vec<EncodableDependency>>,
    /// `root` is optional to allow backward compatibility.
    root: Option<EncodableDependency>,
    metadata: Option<Metadata>,
    #[serde(default, skip_serializing_if = "Patch::is_empty")]
    patch: Patch,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Patch {
    unused: Vec<EncodableDependency>,
}

pub type Metadata = BTreeMap<String, String>;

impl EncodableResolve {
    /// Convert a `Cargo.lock` to a Resolve.
    ///
    /// Note that this `Resolve` is not "complete". For example, the
    /// dependencies do not know the difference between regular/dev/build
    /// dependencies, so they are not filled in. It also does not include
    /// `features`. Care should be taken when using this Resolve. One of the
    /// primary uses is to be used with `resolve_with_previous` to guide the
    /// resolver to create a complete Resolve.
    pub fn into_resolve(self, original: &str, ws: &Workspace<'_>) -> CargoResult<Resolve> {
        let unstable_lockfile_version_allowed = ws.config().cli_unstable().next_lockfile_bump;
        let path_deps = build_path_deps(ws)?;
        let mut checksums = HashMap::new();

        let mut version = match self.version {
            Some(4) if ws.config().nightly_features_allowed => {
                if unstable_lockfile_version_allowed {
                    ResolveVersion::V4
                } else {
                    anyhow::bail!("lock file version 4 requires `-Znext-lockfile-bump`");
                }
            }
            Some(3) => ResolveVersion::V3,
            Some(n) => bail!(
                "lock file version `{}` was found, but this version of Cargo \
                 does not understand this lock file, perhaps Cargo needs \
                 to be updated?",
                n,
            ),
            // Historically Cargo did not have a version indicator in lock
            // files, so this could either be the V1 or V2 encoding. We assume
            // an older format is being parsed until we see so otherwise.
            None => ResolveVersion::V1,
        };

        let packages = {
            let mut packages = self.package.unwrap_or_default();
            if let Some(root) = self.root {
                packages.insert(0, root);
            }
            packages
        };

        // `PackageId`s in the lock file don't include the `source` part
        // for workspace members, so we reconstruct proper IDs.
        let live_pkgs = {
            let mut live_pkgs = HashMap::new();
            let mut all_pkgs = HashSet::new();
            for pkg in packages.iter() {
                let enc_id = EncodablePackageId {
                    name: pkg.name.clone(),
                    version: Some(pkg.version.clone()),
                    source: pkg.source.clone(),
                };

                if !all_pkgs.insert(enc_id.clone()) {
                    anyhow::bail!("package `{}` is specified twice in the lockfile", pkg.name);
                }
                let id = match pkg.source.as_deref().or_else(|| path_deps.get(&pkg.name)) {
                    // We failed to find a local package in the workspace.
                    // It must have been removed and should be ignored.
                    None => {
                        debug!("path dependency now missing {} v{}", pkg.name, pkg.version);
                        continue;
                    }
                    Some(&source) => PackageId::new(&pkg.name, &pkg.version, source)?,
                };

                // If a package has a checksum listed directly on it then record
                // that here, and we also bump our version up to 2 since V1
                // didn't ever encode this field.
                if let Some(cksum) = &pkg.checksum {
                    version = version.max(ResolveVersion::V2);
                    checksums.insert(id, Some(cksum.clone()));
                }

                assert!(live_pkgs.insert(enc_id, (id, pkg)).is_none())
            }
            live_pkgs
        };

        // When decoding a V2 version the edges in `dependencies` aren't
        // guaranteed to have either version or source information. This `map`
        // is used to find package ids even if dependencies have missing
        // information. This map is from name to version to source to actual
        // package ID. (various levels to drill down step by step)
        let mut map = HashMap::new();
        for (id, _) in live_pkgs.values() {
            map.entry(id.name().as_str())
                .or_insert_with(HashMap::new)
                .entry(id.version().to_string())
                .or_insert_with(HashMap::new)
                .insert(id.source_id(), *id);
        }

        let mut lookup_id = |enc_id: &EncodablePackageId| -> Option<PackageId> {
            // The name of this package should always be in the larger list of
            // all packages.
            let by_version = map.get(enc_id.name.as_str())?;

            // If the version is provided, look that up. Otherwise if the
            // version isn't provided this is a V2 manifest and we should only
            // have one version for this name. If we have more than one version
            // for the name then it's ambiguous which one we'd use. That
            // shouldn't ever actually happen but in theory bad git merges could
            // produce invalid lock files, so silently ignore these cases.
            let by_source = match &enc_id.version {
                Some(version) => by_version.get(version)?,
                None => {
                    version = version.max(ResolveVersion::V2);
                    if by_version.len() == 1 {
                        by_version.values().next().unwrap()
                    } else {
                        return None;
                    }
                }
            };

            // This is basically the same as above. Note though that `source` is
            // always missing for path dependencies regardless of serialization
            // format. That means we have to handle the `None` case a bit more
            // carefully.
            match &enc_id.source {
                Some(source) => by_source.get(source).cloned(),
                None => {
                    // Look through all possible packages ids for this
                    // name/version. If there's only one `path` dependency then
                    // we are hardcoded to use that since `path` dependencies
                    // can't have a source listed.
                    let mut path_packages = by_source.values().filter(|p| p.source_id().is_path());
                    if let Some(path) = path_packages.next() {
                        if path_packages.next().is_some() {
                            return None;
                        }
                        Some(*path)

                    // ... otherwise if there's only one then we must be
                    // implicitly using that one due to a V2 serialization of
                    // the lock file
                    } else if by_source.len() == 1 {
                        let id = by_source.values().next().unwrap();
                        version = version.max(ResolveVersion::V2);
                        Some(*id)

                    // ... and failing that we probably had a bad git merge of
                    // `Cargo.lock` or something like that, so just ignore this.
                    } else {
                        None
                    }
                }
            }
        };

        let mut g = Graph::new();

        for (id, _) in live_pkgs.values() {
            g.add(*id);
        }

        for &(ref id, pkg) in live_pkgs.values() {
            let Some(ref deps) = pkg.dependencies else {
                continue;
            };

            for edge in deps.iter() {
                if let Some(to_depend_on) = lookup_id(edge) {
                    g.link(*id, to_depend_on);
                }
            }
        }

        let replacements = {
            let mut replacements = HashMap::new();
            for &(ref id, pkg) in live_pkgs.values() {
                if let Some(ref replace) = pkg.replace {
                    assert!(pkg.dependencies.is_none());
                    if let Some(replace_id) = lookup_id(replace) {
                        replacements.insert(*id, replace_id);
                    }
                }
            }
            replacements
        };

        let mut metadata = self.metadata.unwrap_or_default();

        // In the V1 serialization formats all checksums were listed in the lock
        // file in the `[metadata]` section, so if we're still V1 then look for
        // that here.
        let prefix = "checksum ";
        let mut to_remove = Vec::new();
        for (k, v) in metadata.iter().filter(|p| p.0.starts_with(prefix)) {
            to_remove.push(k.to_string());
            let k = k.strip_prefix(prefix).unwrap();
            let enc_id: EncodablePackageId = k
                .parse()
                .with_context(|| internal("invalid encoding of checksum in lockfile"))?;
            let Some(id) = lookup_id(&enc_id) else {
                continue;
            };

            let v = if v == "<none>" {
                None
            } else {
                Some(v.to_string())
            };
            checksums.insert(id, v);
        }
        // If `checksum` was listed in `[metadata]` but we were previously
        // listed as `V2` then assume some sort of bad git merge happened, so
        // discard all checksums and let's regenerate them later.
        if !to_remove.is_empty() && version >= ResolveVersion::V2 {
            checksums.drain();
        }
        for k in to_remove {
            metadata.remove(&k);
        }

        let mut unused_patches = Vec::new();
        for pkg in self.patch.unused {
            let id = match pkg.source.as_deref().or_else(|| path_deps.get(&pkg.name)) {
                Some(&src) => PackageId::new(&pkg.name, &pkg.version, src)?,
                None => continue,
            };
            unused_patches.push(id);
        }

        // We have a curious issue where in the "v1 format" we buggily had a
        // trailing blank line at the end of lock files under some specific
        // conditions.
        //
        // Cargo is trying to write new lockfies in the "v2 format" but if you
        // have no dependencies, for example, then the lockfile encoded won't
        // really have any indicator that it's in the new format (no
        // dependencies or checksums listed). This means that if you type `cargo
        // new` followed by `cargo build` it will generate a "v2 format" lock
        // file since none previously existed. When reading this on the next
        // `cargo build`, however, it generates a new lock file because when
        // reading in that lockfile we think it's the v1 format.
        //
        // To help fix this issue we special case here. If our lockfile only has
        // one trailing newline, not two, *and* it only has one package, then
        // this is actually the v2 format.
        if original.ends_with('\n')
            && !original.ends_with("\n\n")
            && version == ResolveVersion::V1
            && g.iter().count() == 1
        {
            version = ResolveVersion::V2;
        }

        Ok(Resolve::new(
            g,
            replacements,
            HashMap::new(),
            checksums,
            metadata,
            unused_patches,
            version,
            HashMap::new(),
        ))
    }
}

fn build_path_deps(ws: &Workspace<'_>) -> CargoResult<HashMap<String, SourceId>> {
    // If a crate is **not** a path source, then we're probably in a situation
    // such as `cargo install` with a lock file from a remote dependency. In
    // that case we don't need to fixup any path dependencies (as they're not
    // actually path dependencies any more), so we ignore them.
    let members = ws
        .members()
        .filter(|p| p.package_id().source_id().is_path())
        .collect::<Vec<_>>();

    let mut ret = HashMap::new();
    let mut visited = HashSet::new();
    for member in members.iter() {
        ret.insert(
            member.package_id().name().to_string(),
            member.package_id().source_id(),
        );
        visited.insert(member.package_id().source_id());
    }
    for member in members.iter() {
        build_pkg(member, ws, &mut ret, &mut visited);
    }
    for deps in ws.root_patch()?.values() {
        for dep in deps {
            build_dep(dep, ws, &mut ret, &mut visited);
        }
    }
    for (_, dep) in ws.root_replace() {
        build_dep(dep, ws, &mut ret, &mut visited);
    }

    return Ok(ret);

    fn build_pkg(
        pkg: &Package,
        ws: &Workspace<'_>,
        ret: &mut HashMap<String, SourceId>,
        visited: &mut HashSet<SourceId>,
    ) {
        for dep in pkg.dependencies() {
            build_dep(dep, ws, ret, visited);
        }
    }

    fn build_dep(
        dep: &Dependency,
        ws: &Workspace<'_>,
        ret: &mut HashMap<String, SourceId>,
        visited: &mut HashSet<SourceId>,
    ) {
        let id = dep.source_id();
        if visited.contains(&id) || !id.is_path() {
            return;
        }
        let path = match id.url().to_file_path() {
            Ok(p) => p.join("Cargo.toml"),
            Err(_) => return,
        };
        let Ok(pkg) = ws.load(&path) else { return };
        ret.insert(pkg.name().to_string(), pkg.package_id().source_id());
        visited.insert(pkg.package_id().source_id());
        build_pkg(&pkg, ws, ret, visited);
    }
}

impl Patch {
    fn is_empty(&self) -> bool {
        self.unused.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct EncodableDependency {
    name: String,
    version: String,
    source: Option<EncodableSourceId>,
    checksum: Option<String>,
    dependencies: Option<Vec<EncodablePackageId>>,
    replace: Option<EncodablePackageId>,
}

/// Pretty much equivalent to [`SourceId`] with a different serialization method.
///
/// The serialization for `SourceId` doesn't do URL encode for parameters.
/// In contrast, this type is aware of that whenever [`ResolveVersion`] allows
/// us to do so (v4 or later).
///
/// [`EncodableResolve`] turns into a `
#[derive(Deserialize, Debug, PartialOrd, Ord, Clone)]
#[serde(transparent)]
pub struct EncodableSourceId {
    inner: SourceId,
    /// We don't care about the deserialization of this, as the `url` crate
    /// will always decode as the URL was encoded. Only when a [`Resolve`]
    /// turns into a [`EncodableResolve`] will it set the value accordingly
    /// via [`encodable_source_id`].
    #[serde(skip)]
    encoded: bool,
}

impl EncodableSourceId {
    /// Creates a `EncodableSourceId` that always encodes URL params.
    fn new(inner: SourceId) -> Self {
        Self {
            inner,
            encoded: true,
        }
    }

    /// Creates a `EncodableSourceId` that doesn't encode URL params. This is
    /// for backward compatibility for order lockfile version.
    fn without_url_encoded(inner: SourceId) -> Self {
        Self {
            inner,
            encoded: false,
        }
    }

    /// Encodes the inner [`SourceId`] as a URL.
    fn as_url(&self) -> impl fmt::Display + '_ {
        if self.encoded {
            self.inner.as_encoded_url()
        } else {
            self.inner.as_url()
        }
    }
}

impl std::ops::Deref for EncodableSourceId {
    type Target = SourceId;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl ser::Serialize for EncodableSourceId {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        s.collect_str(&self.as_url())
    }
}

impl std::hash::Hash for EncodableSourceId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl std::cmp::PartialEq for EncodableSourceId {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl std::cmp::Eq for EncodableSourceId {}

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Clone)]
pub struct EncodablePackageId {
    name: String,
    version: Option<String>,
    source: Option<EncodableSourceId>,
}

impl fmt::Display for EncodablePackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(s) = &self.version {
            write!(f, " {}", s)?;
        }
        if let Some(s) = &self.source {
            write!(f, " ({})", s.as_url())?;
        }
        Ok(())
    }
}

impl FromStr for EncodablePackageId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> CargoResult<EncodablePackageId> {
        let mut s = s.splitn(3, ' ');
        let name = s.next().unwrap();
        let version = s.next();
        let source_id = match s.next() {
            Some(s) => {
                if let Some(s) = s.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
                    Some(SourceId::from_url(s)?)
                } else {
                    anyhow::bail!("invalid serialized PackageId")
                }
            }
            None => None,
        };

        Ok(EncodablePackageId {
            name: name.to_string(),
            version: version.map(|v| v.to_string()),
            // Default to url encoded.
            source: source_id.map(EncodableSourceId::new),
        })
    }
}

impl ser::Serialize for EncodablePackageId {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        s.collect_str(self)
    }
}

impl<'de> de::Deserialize<'de> for EncodablePackageId {
    fn deserialize<D>(d: D) -> Result<EncodablePackageId, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        String::deserialize(d).and_then(|string| {
            string
                .parse::<EncodablePackageId>()
                .map_err(de::Error::custom)
        })
    }
}

impl ser::Serialize for Resolve {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let mut ids: Vec<_> = self.iter().collect();
        ids.sort();

        let state = EncodeState::new(self);

        let encodable = ids
            .iter()
            .map(|&id| encodable_resolve_node(id, self, &state))
            .collect::<Vec<_>>();

        let mut metadata = self.metadata().clone();

        if self.version() == ResolveVersion::V1 {
            for &id in ids.iter().filter(|id| !id.source_id().is_path()) {
                let checksum = match self.checksums()[&id] {
                    Some(ref s) => &s[..],
                    None => "<none>",
                };
                let id = encodable_package_id(id, &state, self.version());
                metadata.insert(format!("checksum {}", id.to_string()), checksum.to_string());
            }
        }

        let metadata = if metadata.is_empty() {
            None
        } else {
            Some(metadata)
        };

        let patch = Patch {
            unused: self
                .unused_patches()
                .iter()
                .map(|id| EncodableDependency {
                    name: id.name().to_string(),
                    version: id.version().to_string(),
                    source: encodable_source_id(id.source_id(), self.version()),
                    dependencies: None,
                    replace: None,
                    checksum: if self.version() >= ResolveVersion::V2 {
                        self.checksums().get(id).and_then(|x| x.clone())
                    } else {
                        None
                    },
                })
                .collect(),
        };
        EncodableResolve {
            package: Some(encodable),
            root: None,
            metadata,
            patch,
            version: match self.version() {
                ResolveVersion::V4 => Some(4),
                ResolveVersion::V3 => Some(3),
                ResolveVersion::V2 | ResolveVersion::V1 => None,
            },
        }
        .serialize(s)
    }
}

pub struct EncodeState<'a> {
    counts: Option<HashMap<InternedString, HashMap<&'a semver::Version, usize>>>,
}

impl<'a> EncodeState<'a> {
    pub fn new(resolve: &'a Resolve) -> EncodeState<'a> {
        let counts = if resolve.version() >= ResolveVersion::V2 {
            let mut map = HashMap::new();
            for id in resolve.iter() {
                let slot = map
                    .entry(id.name())
                    .or_insert_with(HashMap::new)
                    .entry(id.version())
                    .or_insert(0);
                *slot += 1;
            }
            Some(map)
        } else {
            None
        };
        EncodeState { counts }
    }
}

fn encodable_resolve_node(
    id: PackageId,
    resolve: &Resolve,
    state: &EncodeState<'_>,
) -> EncodableDependency {
    let (replace, deps) = match resolve.replacement(id) {
        Some(id) => (
            Some(encodable_package_id(id, state, resolve.version())),
            None,
        ),
        None => {
            let mut deps = resolve
                .deps_not_replaced(id)
                .map(|(id, _)| encodable_package_id(id, state, resolve.version()))
                .collect::<Vec<_>>();
            deps.sort();
            (None, Some(deps))
        }
    };

    EncodableDependency {
        name: id.name().to_string(),
        version: id.version().to_string(),
        source: encodable_source_id(id.source_id(), resolve.version()),
        dependencies: deps,
        replace,
        checksum: if resolve.version() >= ResolveVersion::V2 {
            resolve.checksums().get(&id).and_then(|s| s.clone())
        } else {
            None
        },
    }
}

pub fn encodable_package_id(
    id: PackageId,
    state: &EncodeState<'_>,
    resolve_version: ResolveVersion,
) -> EncodablePackageId {
    let mut version = Some(id.version().to_string());
    let mut id_to_encode = id.source_id();
    if resolve_version <= ResolveVersion::V2 {
        if let Some(GitReference::Branch(b)) = id_to_encode.git_reference() {
            if b == "master" {
                id_to_encode =
                    SourceId::for_git(id_to_encode.url(), GitReference::DefaultBranch).unwrap();
            }
        }
    }
    let mut source = encodable_source_id(id_to_encode.without_precise(), resolve_version);
    if let Some(counts) = &state.counts {
        let version_counts = &counts[&id.name()];
        if version_counts[&id.version()] == 1 {
            source = None;
            if version_counts.len() == 1 {
                version = None;
            }
        }
    }
    EncodablePackageId {
        name: id.name().to_string(),
        version,
        source,
    }
}

fn encodable_source_id(id: SourceId, version: ResolveVersion) -> Option<EncodableSourceId> {
    if id.is_path() {
        None
    } else {
        Some(match version {
            ResolveVersion::V4 => EncodableSourceId::new(id),
            _ => EncodableSourceId::without_url_encoded(id),
        })
    }
}
