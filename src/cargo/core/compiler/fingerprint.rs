use std::env;
use std::fs;
use std::hash::{self, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use filetime::FileTime;
use log::{debug, info};
use serde::de;
use serde::ser;
use serde::{Deserialize, Serialize};

use crate::core::{Edition, Package};
use crate::util;
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::paths;
use crate::util::{internal, profile, Dirty, Fresh, Freshness};

use super::custom_build::BuildDeps;
use super::job::Work;
use super::{BuildContext, Context, FileFlavor, Unit};

/// A tuple result of the `prepare_foo` functions in this module.
///
/// The first element of the triple is whether the target in question is
/// currently fresh or not, and the second two elements are work to perform when
/// the target is dirty or fresh, respectively.
///
/// Both units of work are always generated because a fresh package may still be
/// rebuilt if some upstream dependency changes.
pub type Preparation = (Freshness, Work, Work);

/// Prepare the necessary work for the fingerprint for a specific target.
///
/// When dealing with fingerprints, cargo gets to choose what granularity
/// "freshness" is considered at. One option is considering freshness at the
/// package level. This means that if anything in a package changes, the entire
/// package is rebuilt, unconditionally. This simplicity comes at a cost,
/// however, in that test-only changes will cause libraries to be rebuilt, which
/// is quite unfortunate!
///
/// The cost was deemed high enough that fingerprints are now calculated at the
/// layer of a target rather than a package. Each target can then be kept track
/// of separately and only rebuilt as necessary. This requires cargo to
/// understand what the inputs are to a target, so we drive rustc with the
/// --dep-info flag to learn about all input files to a unit of compilation.
///
/// This function will calculate the fingerprint for a target and prepare the
/// work necessary to either write the fingerprint or copy over all fresh files
/// from the old directories to their new locations.
pub fn prepare_target<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
) -> CargoResult<Preparation> {
    let _p = profile::start(format!(
        "fingerprint: {} / {}",
        unit.pkg.package_id(),
        unit.target.name()
    ));
    let bcx = cx.bcx;
    let new = cx.files().fingerprint_dir(unit);
    let loc = new.join(&filename(cx, unit));

    debug!("fingerprint at: {}", loc.display());

    let fingerprint = calculate(cx, unit)?;
    let compare = compare_old_fingerprint(&loc, &*fingerprint);
    log_compare(unit, &compare);

    // If our comparison failed (e.g. we're going to trigger a rebuild of this
    // crate), then we also ensure the source of the crate passes all
    // verification checks before we build it.
    //
    // The `Source::verify` method is intended to allow sources to execute
    // pre-build checks to ensure that the relevant source code is all
    // up-to-date and as expected. This is currently used primarily for
    // directory sources which will use this hook to perform an integrity check
    // on all files in the source to ensure they haven't changed. If they have
    // changed then an error is issued.
    if compare.is_err() {
        let source_id = unit.pkg.package_id().source_id();
        let sources = bcx.packages.sources();
        let source = sources
            .get(source_id)
            .ok_or_else(|| internal("missing package source"))?;
        source.verify(unit.pkg.package_id())?;
    }

    let root = cx.files().out_dir(unit);
    let missing_outputs = {
        if unit.mode.is_doc() {
            !root
                .join(unit.target.crate_name())
                .join("index.html")
                .exists()
        } else {
            match cx
                .outputs(unit)?
                .iter()
                .filter(|output| output.flavor != FileFlavor::DebugInfo)
                .find(|output| !output.path.exists())
            {
                None => false,
                Some(output) => {
                    info!("missing output path {:?}", output.path);
                    true
                }
            }
        }
    };

    let allow_failure = bcx.extra_args_for(unit).is_some();
    let target_root = cx.files().target_root().to_path_buf();
    let write_fingerprint = Work::new(move |_| {
        match fingerprint.update_local(&target_root) {
            Ok(()) => {}
            Err(..) if allow_failure => return Ok(()),
            Err(e) => return Err(e),
        }
        write_fingerprint(&loc, &*fingerprint)
    });

    let fresh = compare.is_ok() && !missing_outputs;
    Ok((
        if fresh { Fresh } else { Dirty },
        write_fingerprint,
        Work::noop(),
    ))
}

/// A compilation unit dependency has a fingerprint that is comprised of:
/// * its package id
/// * its extern crate name
/// * its calculated fingerprint for the dependency
struct DepFingerprint {
    pkg_id: String,
    name: String,
    fingerprint: Arc<Fingerprint>,
}

/// A fingerprint can be considered to be a "short string" representing the
/// state of a world for a package.
///
/// If a fingerprint ever changes, then the package itself needs to be
/// recompiled. Inputs to the fingerprint include source code modifications,
/// compiler flags, compiler version, etc. This structure is not simply a
/// `String` due to the fact that some fingerprints cannot be calculated lazily.
///
/// Path sources, for example, use the mtime of the corresponding dep-info file
/// as a fingerprint (all source files must be modified *before* this mtime).
/// This dep-info file is not generated, however, until after the crate is
/// compiled. As a result, this structure can be thought of as a fingerprint
/// to-be. The actual value can be calculated via `hash()`, but the operation
/// may fail as some files may not have been generated.
///
/// Note that dependencies are taken into account for fingerprints because rustc
/// requires that whenever an upstream crate is recompiled that all downstream
/// dependants are also recompiled. This is typically tracked through
/// `DependencyQueue`, but it also needs to be retained here because Cargo can
/// be interrupted while executing, losing the state of the `DependencyQueue`
/// graph.
#[derive(Serialize, Deserialize)]
pub struct Fingerprint {
    rustc: u64,
    features: String,
    target: u64,
    profile: u64,
    path: u64,
    deps: Vec<DepFingerprint>,
    local: Vec<LocalFingerprint>,
    #[serde(skip_serializing, skip_deserializing)]
    memoized_hash: Mutex<Option<u64>>,
    rustflags: Vec<String>,
    edition: Edition,
}

impl Serialize for DepFingerprint {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        (&self.pkg_id, &self.name, &self.fingerprint.hash()).serialize(ser)
    }
}

impl<'de> Deserialize<'de> for DepFingerprint {
    fn deserialize<D>(d: D) -> Result<DepFingerprint, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let (pkg_id, name, hash) = <(String, String, u64)>::deserialize(d)?;
        Ok(DepFingerprint {
            pkg_id,
            name,
            fingerprint: Arc::new(Fingerprint {
                local: vec![LocalFingerprint::Precalculated(String::new())],
                memoized_hash: Mutex::new(Some(hash)),
                ..Fingerprint::new()
            }),
        })
    }
}

#[derive(Serialize, Deserialize, Hash)]
enum LocalFingerprint {
    Precalculated(String),
    MtimeBased(MtimeSlot, PathBuf),
    EnvBased(String, Option<String>),
}

impl LocalFingerprint {
    fn mtime(root: &Path, mtime: Option<FileTime>, path: &Path) -> LocalFingerprint {
        let mtime = MtimeSlot(Mutex::new(mtime));
        assert!(path.is_absolute());
        let path = path.strip_prefix(root).unwrap_or(path);
        LocalFingerprint::MtimeBased(mtime, path.to_path_buf())
    }
}

struct MtimeSlot(Mutex<Option<FileTime>>);

impl Fingerprint {
    fn new() -> Fingerprint {
        Fingerprint {
            rustc: 0,
            target: 0,
            profile: 0,
            path: 0,
            features: String::new(),
            deps: Vec::new(),
            local: Vec::new(),
            memoized_hash: Mutex::new(None),
            edition: Edition::Edition2015,
            rustflags: Vec::new(),
        }
    }

    fn update_local(&self, root: &Path) -> CargoResult<()> {
        for local in self.local.iter() {
            match *local {
                LocalFingerprint::MtimeBased(ref slot, ref path) => {
                    let path = root.join(path);
                    let mtime = paths::mtime(&path)?;
                    *slot.0.lock().unwrap() = Some(mtime);
                }
                LocalFingerprint::EnvBased(..) | LocalFingerprint::Precalculated(..) => continue,
            }
        }

        *self.memoized_hash.lock().unwrap() = None;
        Ok(())
    }

    fn hash(&self) -> u64 {
        if let Some(s) = *self.memoized_hash.lock().unwrap() {
            return s;
        }
        let ret = util::hash_u64(self);
        *self.memoized_hash.lock().unwrap() = Some(ret);
        ret
    }

    fn compare(&self, old: &Fingerprint) -> CargoResult<()> {
        if self.rustc != old.rustc {
            failure::bail!("rust compiler has changed")
        }
        if self.features != old.features {
            failure::bail!(
                "features have changed: {} != {}",
                self.features,
                old.features
            )
        }
        if self.target != old.target {
            failure::bail!("target configuration has changed")
        }
        if self.path != old.path {
            failure::bail!("path to the compiler has changed")
        }
        if self.profile != old.profile {
            failure::bail!("profile configuration has changed")
        }
        if self.rustflags != old.rustflags {
            failure::bail!("RUSTFLAGS has changed")
        }
        if self.local.len() != old.local.len() {
            failure::bail!("local lens changed");
        }
        if self.edition != old.edition {
            failure::bail!("edition changed")
        }
        for (new, old) in self.local.iter().zip(&old.local) {
            match (new, old) {
                (
                    &LocalFingerprint::Precalculated(ref a),
                    &LocalFingerprint::Precalculated(ref b),
                ) => {
                    if a != b {
                        failure::bail!("precalculated components have changed: {} != {}", a, b)
                    }
                }
                (
                    &LocalFingerprint::MtimeBased(ref on_disk_mtime, ref ap),
                    &LocalFingerprint::MtimeBased(ref previously_built_mtime, ref bp),
                ) => {
                    let on_disk_mtime = on_disk_mtime.0.lock().unwrap();
                    let previously_built_mtime = previously_built_mtime.0.lock().unwrap();

                    let should_rebuild = match (*on_disk_mtime, *previously_built_mtime) {
                        (None, None) => false,
                        (Some(_), None) | (None, Some(_)) => true,
                        (Some(on_disk), Some(previously_built)) => on_disk > previously_built,
                    };

                    if should_rebuild {
                        failure::bail!(
                            "mtime based components have changed: previously {:?} now {:?}, \
                             paths are {:?} and {:?}",
                            *previously_built_mtime,
                            *on_disk_mtime,
                            ap,
                            bp
                        )
                    }
                }
                (
                    &LocalFingerprint::EnvBased(ref akey, ref avalue),
                    &LocalFingerprint::EnvBased(ref bkey, ref bvalue),
                ) => {
                    if *akey != *bkey {
                        failure::bail!("env vars changed: {} != {}", akey, bkey);
                    }
                    if *avalue != *bvalue {
                        failure::bail!(
                            "env var `{}` changed: previously {:?} now {:?}",
                            akey,
                            bvalue,
                            avalue
                        )
                    }
                }
                _ => failure::bail!("local fingerprint type has changed"),
            }
        }

        if self.deps.len() != old.deps.len() {
            failure::bail!("number of dependencies has changed")
        }
        for (a, b) in self.deps.iter().zip(old.deps.iter()) {
            if a.name != b.name || a.fingerprint.hash() != b.fingerprint.hash() {
                failure::bail!("new ({}) != old ({})", a.pkg_id, b.pkg_id)
            }
        }
        Ok(())
    }
}

impl hash::Hash for Fingerprint {
    fn hash<H: Hasher>(&self, h: &mut H) {
        let Fingerprint {
            rustc,
            ref features,
            target,
            path,
            profile,
            ref deps,
            ref local,
            edition,
            ref rustflags,
            ..
        } = *self;
        (
            rustc, features, target, path, profile, local, edition, rustflags,
        )
            .hash(h);

        h.write_usize(deps.len());
        for DepFingerprint {
            pkg_id,
            name,
            fingerprint,
        } in deps
        {
            pkg_id.hash(h);
            name.hash(h);
            // use memoized dep hashes to avoid exponential blowup
            h.write_u64(Fingerprint::hash(fingerprint));
        }
    }
}

impl hash::Hash for MtimeSlot {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.lock().unwrap().hash(h)
    }
}

impl ser::Serialize for MtimeSlot {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.0
            .lock()
            .unwrap()
            .map(|ft| (ft.unix_seconds(), ft.nanoseconds()))
            .serialize(s)
    }
}

impl<'de> de::Deserialize<'de> for MtimeSlot {
    fn deserialize<D>(d: D) -> Result<MtimeSlot, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let kind: Option<(i64, u32)> = de::Deserialize::deserialize(d)?;
        Ok(MtimeSlot(Mutex::new(
            kind.map(|(s, n)| FileTime::from_unix_time(s, n)),
        )))
    }
}

/// Calculates the fingerprint for a package/target pair.
///
/// This fingerprint is used by Cargo to learn about when information such as:
///
/// * A non-path package changes (changes version, changes revision, etc).
/// * Any dependency changes
/// * The compiler changes
/// * The set of features a package is built with changes
/// * The profile a target is compiled with changes (e.g. opt-level changes)
///
/// Information like file modification time is only calculated for path
/// dependencies and is calculated in `calculate_target_fresh`.
fn calculate<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
) -> CargoResult<Arc<Fingerprint>> {
    let bcx = cx.bcx;
    if let Some(s) = cx.fingerprints.get(unit) {
        return Ok(Arc::clone(s));
    }

    // Next, recursively calculate the fingerprint for all of our dependencies.
    //
    // Skip the fingerprints of build scripts as they may not always be
    // available and the dirtiness propagation for modification is tracked
    // elsewhere. Also skip fingerprints of binaries because they don't actually
    // induce a recompile, they're just dependencies in the sense that they need
    // to be built.
    let deps = cx.dep_targets(unit);
    let deps = deps
        .iter()
        .filter(|u| !u.target.is_custom_build() && !u.target.is_bin())
        .map(|dep| {
            calculate(cx, dep).and_then(|fingerprint| {
                let name = cx.bcx.extern_crate_name(unit, dep)?;
                Ok(DepFingerprint {
                    pkg_id: dep.pkg.package_id().to_string(),
                    name,
                    fingerprint,
                })
            })
        })
        .collect::<CargoResult<Vec<_>>>()?;

    // And finally, calculate what our own local fingerprint is
    let local = if use_dep_info(unit) {
        let dep_info = dep_info_loc(cx, unit);
        let mtime = dep_info_mtime_if_fresh(unit.pkg, &dep_info)?;
        LocalFingerprint::mtime(cx.files().target_root(), mtime, &dep_info)
    } else {
        let fingerprint = pkg_fingerprint(&cx.bcx, unit.pkg)?;
        LocalFingerprint::Precalculated(fingerprint)
    };
    let mut deps = deps;
    deps.sort_by(|a, b| a.pkg_id.cmp(&b.pkg_id));
    let extra_flags = if unit.mode.is_doc() {
        bcx.rustdocflags_args(unit)?
    } else {
        bcx.rustflags_args(unit)?
    };
    let profile_hash = util::hash_u64(&(
        &unit.profile,
        unit.mode,
        bcx.extra_args_for(unit),
        cx.incremental_args(unit)?,
    ));
    let fingerprint = Arc::new(Fingerprint {
        rustc: util::hash_u64(&bcx.rustc.verbose_version),
        target: util::hash_u64(&unit.target),
        profile: profile_hash,
        // Note that .0 is hashed here, not .1 which is the cwd. That doesn't
        // actually affect the output artifact so there's no need to hash it.
        path: util::hash_u64(&super::path_args(&cx.bcx, unit).0),
        features: format!("{:?}", bcx.resolve.features_sorted(unit.pkg.package_id())),
        deps,
        local: vec![local],
        memoized_hash: Mutex::new(None),
        edition: unit.target.edition(),
        rustflags: extra_flags,
    });
    cx.fingerprints.insert(*unit, Arc::clone(&fingerprint));
    Ok(fingerprint)
}

// We want to use the mtime for files if we're a path source, but if we're a
// git/registry source, then the mtime of files may fluctuate, but they won't
// change so long as the source itself remains constant (which is the
// responsibility of the source)
fn use_dep_info(unit: &Unit<'_>) -> bool {
    let path = unit.pkg.summary().source_id().is_path();
    !unit.mode.is_doc() && path
}

/// Prepare the necessary work for the fingerprint of a build command.
///
/// Build commands are located on packages, not on targets. Additionally, we
/// don't have --dep-info to drive calculation of the fingerprint of a build
/// command. This brings up an interesting predicament which gives us a few
/// options to figure out whether a build command is dirty or not:
///
/// 1. A build command is dirty if *any* file in a package changes. In theory
///    all files are candidate for being used by the build command.
/// 2. A build command is dirty if any file in a *specific directory* changes.
///    This may lose information as it may require files outside of the specific
///    directory.
/// 3. A build command must itself provide a dep-info-like file stating how it
///    should be considered dirty or not.
///
/// The currently implemented solution is option (1), although it is planned to
/// migrate to option (2) in the near future.
pub fn prepare_build_cmd<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
) -> CargoResult<Preparation> {
    let _p = profile::start(format!("fingerprint build cmd: {}", unit.pkg.package_id()));
    let new = cx.files().fingerprint_dir(unit);
    let loc = new.join("build");

    debug!("fingerprint at: {}", loc.display());

    let (local, output_path) = build_script_local_fingerprints(cx, unit)?;
    let mut fingerprint = Fingerprint {
        local,
        rustc: util::hash_u64(&cx.bcx.rustc.verbose_version),
        ..Fingerprint::new()
    };
    let compare = compare_old_fingerprint(&loc, &fingerprint);
    log_compare(unit, &compare);

    // When we write out the fingerprint, we may want to actually change the
    // kind of fingerprint being recorded. If we started out, then the previous
    // run of the build script (or if it had never run before) may indicate to
    // use the `Precalculated` variant with the `pkg_fingerprint`. If the build
    // script then prints `rerun-if-changed`, however, we need to record what's
    // necessary for that fingerprint.
    //
    // Hence, if there were some `rerun-if-changed` directives forcibly change
    // the kind of fingerprint by reinterpreting the dependencies output by the
    // build script.
    let state = Arc::clone(&cx.build_state);
    let key = (unit.pkg.package_id(), unit.kind);
    let pkg_root = unit.pkg.root().to_path_buf();
    let target_root = cx.files().target_root().to_path_buf();
    let write_fingerprint = Work::new(move |_| {
        if let Some(output_path) = output_path {
            let outputs = state.outputs.lock().unwrap();
            let outputs = &outputs[&key];
            if !outputs.rerun_if_changed.is_empty() || !outputs.rerun_if_env_changed.is_empty() {
                let deps = BuildDeps::new(&output_path, Some(outputs));
                fingerprint.local = local_fingerprints_deps(&deps, &target_root, &pkg_root);
                fingerprint.update_local(&target_root)?;
            }
        }
        write_fingerprint(&loc, &fingerprint)
    });

    Ok((
        if compare.is_ok() { Fresh } else { Dirty },
        write_fingerprint,
        Work::noop(),
    ))
}

fn build_script_local_fingerprints<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
) -> CargoResult<(Vec<LocalFingerprint>, Option<PathBuf>)> {
    let state = cx.build_state.outputs.lock().unwrap();
    // First up, if this build script is entirely overridden, then we just
    // return the hash of what we overrode it with.
    //
    // Note that the `None` here means that we don't want to update the local
    // fingerprint afterwards because this is all just overridden.
    if let Some(output) = state.get(&(unit.pkg.package_id(), unit.kind)) {
        debug!("override local fingerprints deps");
        let s = format!(
            "overridden build state with hash: {}",
            util::hash_u64(output)
        );
        return Ok((vec![LocalFingerprint::Precalculated(s)], None));
    }

    // Next up we look at the previously listed dependencies for the build
    // script. If there are none then we're in the "old mode" where we just
    // assume that we're changed if anything in the packaged changed. The
    // `Some` here though means that we want to update our local fingerprints
    // after we're done as running this build script may have created more
    // dependencies.
    let deps = &cx.build_explicit_deps[unit];
    let output = deps.build_script_output.clone();
    if deps.rerun_if_changed.is_empty() && deps.rerun_if_env_changed.is_empty() {
        debug!("old local fingerprints deps");
        let s = pkg_fingerprint(&cx.bcx, unit.pkg)?;
        return Ok((vec![LocalFingerprint::Precalculated(s)], Some(output)));
    }

    // Ok so now we're in "new mode" where we can have files listed as
    // dependencies as well as env vars listed as dependencies. Process them all
    // here.
    Ok((
        local_fingerprints_deps(deps, cx.files().target_root(), unit.pkg.root()),
        Some(output),
    ))
}

fn local_fingerprints_deps(
    deps: &BuildDeps,
    target_root: &Path,
    pkg_root: &Path,
) -> Vec<LocalFingerprint> {
    debug!("new local fingerprints deps");
    let mut local = Vec::new();
    if !deps.rerun_if_changed.is_empty() {
        let output = &deps.build_script_output;
        let deps = deps.rerun_if_changed.iter().map(|p| pkg_root.join(p));
        let mtime = mtime_if_fresh(output, deps);
        local.push(LocalFingerprint::mtime(target_root, mtime, output));
    }

    for var in deps.rerun_if_env_changed.iter() {
        let val = env::var(var).ok();
        local.push(LocalFingerprint::EnvBased(var.clone(), val));
    }

    local
}

fn write_fingerprint(loc: &Path, fingerprint: &Fingerprint) -> CargoResult<()> {
    debug_assert_ne!(fingerprint.rustc, 0);
    // fingerprint::new().rustc == 0, make sure it doesn't make it to the file system.
    // This is mostly so outside tools can reliably find out what rust version this file is for,
    // as we can use the full hash.
    let hash = fingerprint.hash();
    debug!("write fingerprint: {}", loc.display());
    paths::write(loc, util::to_hex(hash).as_bytes())?;
    paths::write(
        &loc.with_extension("json"),
        &serde_json::to_vec(&fingerprint).unwrap(),
    )?;
    Ok(())
}

/// Prepare for work when a package starts to build
pub fn prepare_init<'a, 'cfg>(cx: &mut Context<'a, 'cfg>, unit: &Unit<'a>) -> CargoResult<()> {
    let new1 = cx.files().fingerprint_dir(unit);

    if fs::metadata(&new1).is_err() {
        fs::create_dir(&new1)?;
    }

    Ok(())
}

pub fn dep_info_loc<'a, 'cfg>(cx: &mut Context<'a, 'cfg>, unit: &Unit<'a>) -> PathBuf {
    cx.files()
        .fingerprint_dir(unit)
        .join(&format!("dep-{}", filename(cx, unit)))
}

fn compare_old_fingerprint(loc: &Path, new_fingerprint: &Fingerprint) -> CargoResult<()> {
    let old_fingerprint_short = paths::read(loc)?;
    let new_hash = new_fingerprint.hash();

    if util::to_hex(new_hash) == old_fingerprint_short {
        return Ok(());
    }

    let old_fingerprint_json = paths::read(&loc.with_extension("json"))?;
    let old_fingerprint = serde_json::from_str(&old_fingerprint_json)
        .chain_err(|| internal("failed to deserialize json"))?;
    new_fingerprint.compare(&old_fingerprint)
}

fn log_compare(unit: &Unit<'_>, compare: &CargoResult<()>) {
    let ce = match *compare {
        Ok(..) => return,
        Err(ref e) => e,
    };
    info!("fingerprint error for {}: {}", unit.pkg, ce);

    for cause in ce.iter_causes() {
        info!("  cause: {}", cause);
    }
}

// Parse the dep-info into a list of paths
pub fn parse_dep_info(pkg: &Package, dep_info: &Path) -> CargoResult<Option<Vec<PathBuf>>> {
    let data = match paths::read_bytes(dep_info) {
        Ok(data) => data,
        Err(_) => return Ok(None),
    };
    let paths = data
        .split(|&x| x == 0)
        .filter(|x| !x.is_empty())
        .map(|p| util::bytes2path(p).map(|p| pkg.root().join(p)))
        .collect::<Result<Vec<_>, _>>()?;
    if paths.is_empty() {
        Ok(None)
    } else {
        Ok(Some(paths))
    }
}

fn dep_info_mtime_if_fresh(pkg: &Package, dep_info: &Path) -> CargoResult<Option<FileTime>> {
    if let Some(paths) = parse_dep_info(pkg, dep_info)? {
        Ok(mtime_if_fresh(dep_info, paths.iter()))
    } else {
        Ok(None)
    }
}

fn pkg_fingerprint(bcx: &BuildContext<'_, '_>, pkg: &Package) -> CargoResult<String> {
    let source_id = pkg.package_id().source_id();
    let sources = bcx.packages.sources();

    let source = sources
        .get(source_id)
        .ok_or_else(|| internal("missing package source"))?;
    source.fingerprint(pkg)
}

fn mtime_if_fresh<I>(output: &Path, paths: I) -> Option<FileTime>
where
    I: IntoIterator,
    I::Item: AsRef<Path>,
{
    let mtime = match paths::mtime(output) {
        Ok(mtime) => mtime,
        Err(..) => return None,
    };

    let any_stale = paths.into_iter().any(|path| {
        let path = path.as_ref();
        let mtime2 = match paths::mtime(path) {
            Ok(mtime) => mtime,
            Err(..) => {
                info!("stale: {} -- missing", path.display());
                return true;
            }
        };

        // Note that equal mtimes are considered "stale". For filesystems with
        // not much timestamp precision like 1s this is a conservative approximation
        // to handle the case where a file is modified within the same second after
        // a build finishes. We want to make sure that incremental rebuilds pick that up!
        //
        // For filesystems with nanosecond precision it's been seen in the wild that
        // its "nanosecond precision" isn't really nanosecond-accurate. It turns out that
        // kernels may cache the current time so files created at different times actually
        // list the same nanosecond precision. Some digging on #5919 picked up that the
        // kernel caches the current time between timer ticks, which could mean that if
        // a file is updated at most 10ms after a build finishes then Cargo may not
        // pick up the build changes.
        //
        // All in all, the equality check here is a conservative assumption that,
        // if equal, files were changed just after a previous build finished.
        // It's hoped this doesn't cause too many issues in practice!
        if mtime2 >= mtime {
            info!("stale: {} -- {} vs {}", path.display(), mtime2, mtime);
            true
        } else {
            false
        }
    });

    if any_stale {
        None
    } else {
        Some(mtime)
    }
}

fn filename<'a, 'cfg>(cx: &mut Context<'a, 'cfg>, unit: &Unit<'a>) -> String {
    // file_stem includes metadata hash. Thus we have a different
    // fingerprint for every metadata hash version. This works because
    // even if the package is fresh, we'll still link the fresh target
    let file_stem = cx.files().file_stem(unit);
    let kind = unit.target.kind().description();
    let flavor = if unit.mode.is_any_test() {
        "test-"
    } else if unit.mode.is_doc() {
        "doc-"
    } else {
        ""
    };
    format!("{}{}-{}", flavor, kind, file_stem)
}

/// Parses the dep-info file coming out of rustc into a Cargo-specific format.
///
/// This function will parse `rustc_dep_info` as a makefile-style dep info to
/// learn about the all files which a crate depends on. This is then
/// re-serialized into the `cargo_dep_info` path in a Cargo-specific format.
///
/// The `pkg_root` argument here is the absolute path to the directory
/// containing `Cargo.toml` for this crate that was compiled. The paths listed
/// in the rustc dep-info file may or may not be absolute but we'll want to
/// consider all of them relative to the `root` specified.
///
/// The `rustc_cwd` argument is the absolute path to the cwd of the compiler
/// when it was invoked.
///
/// The serialized Cargo format will contain a list of files, all of which are
/// relative if they're under `root`. or absolute if they're elsewhere.
pub fn translate_dep_info(
    rustc_dep_info: &Path,
    cargo_dep_info: &Path,
    pkg_root: &Path,
    rustc_cwd: &Path,
) -> CargoResult<()> {
    let target = parse_rustc_dep_info(rustc_dep_info)?;
    let deps = &target
        .get(0)
        .ok_or_else(|| internal("malformed dep-info format, no targets".to_string()))?
        .1;

    let mut new_contents = Vec::new();
    for file in deps {
        let absolute = rustc_cwd.join(file);
        let path = absolute.strip_prefix(pkg_root).unwrap_or(&absolute);
        new_contents.extend(util::path2bytes(path)?);
        new_contents.push(0);
    }
    paths::write(cargo_dep_info, &new_contents)?;
    Ok(())
}

pub fn parse_rustc_dep_info(rustc_dep_info: &Path) -> CargoResult<Vec<(String, Vec<String>)>> {
    let contents = paths::read(rustc_dep_info)?;
    contents
        .lines()
        .filter_map(|l| l.find(": ").map(|i| (l, i)))
        .map(|(line, pos)| {
            let target = &line[..pos];
            let mut deps = line[pos + 2..].split_whitespace();

            let mut ret = Vec::new();
            while let Some(s) = deps.next() {
                let mut file = s.to_string();
                while file.ends_with('\\') {
                    file.pop();
                    file.push(' ');
                    file.push_str(deps.next().ok_or_else(|| {
                        internal("malformed dep-info format, trailing \\".to_string())
                    })?);
                }
                ret.push(file);
            }
            Ok((target.to_string(), ret))
        })
        .collect()
}
