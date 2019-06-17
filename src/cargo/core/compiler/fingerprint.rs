//! # Fingerprints
//!
//! This module implements change-tracking so that Cargo can know whether or
//! not something needs to be recompiled. A Cargo `Unit` can be either "dirty"
//! (needs to be recompiled) or "fresh" (it does not need to be recompiled).
//! There are several mechanisms that influence a Unit's freshness:
//!
//! - The `Metadata` hash isolates each Unit on the filesystem by being
//!   embedded in the filename. If something in the hash changes, then the
//!   output files will be missing, and the Unit will be dirty (missing
//!   outputs are considered "dirty").
//! - The `Fingerprint` is another hash, saved to the filesystem in the
//!   `.fingerprint` directory, that tracks information about the inputs to a
//!   Unit. If any of the inputs changes from the last compilation, then the
//!   Unit is considered dirty. A missing fingerprint (such as during the
//!   first build) is also considered dirty.
//! - Whether or not input files are actually present. For example a build
//!   script which says it depends on a nonexistent file `foo` is always rerun.
//! - Propagation throughout the dependency graph of file modification time
//!   information, used to detect changes on the filesystem. Each `Fingerprint`
//!   keeps track of what files it'll be processing, and when necessary it will
//!   check the `mtime` of each file (last modification time) and compare it to
//!   dependencies and output to see if files have been changed or if a change
//!   needs to force recompiles of downstream dependencies.
//!
//! Note: Fingerprinting is not a perfect solution. Filesystem mtime tracking
//! is notoriously imprecise and problematic. Only a small part of the
//! environment is captured. This is a balance of performance, simplicity, and
//! completeness. Sandboxing, hashing file contents, tracking every file
//! access, environment variable, and network operation would ensure more
//! reliable and reproducible builds at the cost of being complex, slow, and
//! platform-dependent.
//!
//! ## Fingerprints and Metadata
//!
//! Fingerprints and Metadata are similar, and track some of the same things.
//! The Metadata contains information that is required to keep Units separate.
//! The Fingerprint includes additional information that should cause a
//! recompile, but it is desired to reuse the same filenames. Generally the
//! items in the Metadata do not need to be in the Fingerprint. A comparison
//! of what is tracked:
//!
//! Value                                      | Fingerprint | Metadata
//! -------------------------------------------|-------------|----------
//! rustc                                      | ✓           | ✓
//! Profile                                    | ✓           | ✓
//! `cargo rustc` extra args                   | ✓           | ✓
//! CompileMode                                | ✓           | ✓
//! Target Name                                | ✓           | ✓
//! Target Kind (bin/lib/etc.)                 | ✓           | ✓
//! Enabled Features                           | ✓           | ✓
//! Immediate dependency’s hashes              | ✓[^1]       | ✓
//! Target or Host mode                        |             | ✓
//! __CARGO_DEFAULT_LIB_METADATA[^4]           |             | ✓
//! package_id                                 |             | ✓
//! authors, description, homepage, repo       | ✓           |
//! Target src path                            | ✓           |
//! Target path relative to ws                 | ✓           |
//! Target flags (test/bench/for_host/edition) | ✓           |
//! -C incremental=… flag                      | ✓           |
//! mtime of sources                           | ✓[^3]       |
//! RUSTFLAGS/RUSTDOCFLAGS                     | ✓           | ✓
//!
//! [^1]: Build script and bin dependencies are not included.
//!
//! [^3]: The mtime is only tracked for workspace members and path
//!       dependencies. Git dependencies track the git revision.
//!
//! [^4]: `__CARGO_DEFAULT_LIB_METADATA` is set by rustbuild to embed the
//!        release channel (bootstrap/stable/beta/nightly) in libstd.
//!
//! ## Fingerprint files
//!
//! Fingerprint information is stored in the
//! `target/{debug,release}/.fingerprint/` directory. Each Unit is stored in a
//! separate directory. Each Unit directory contains:
//!
//! - A file with a 16 hex-digit hash. This is the Fingerprint hash, used for
//!   quick loading and comparison.
//! - A `.json` file that contains details about the Fingerprint. This is only
//!   used to log details about *why* a fingerprint is considered dirty.
//!   `CARGO_LOG=cargo::core::compiler::fingerprint=trace cargo build` can be
//!   used to display this log information.
//! - A "dep-info" file which contains a list of source filenames for the
//!   target. This is produced by reading the output of `rustc
//!   --emit=dep-info` and packing it into a condensed format. Cargo uses this
//!   to check the mtime of every file to see if any of them have changed.
//! - An `invoked.timestamp` file whose filesystem mtime is updated every time
//!   the Unit is built. This is an experimental feature used for cleaning
//!   unused artifacts.
//!
//! Note that some units are a little different. A Unit for *running* a build
//! script or for `rustdoc` does not have a dep-info file (it's not
//! applicable). Build script `invoked.timestamp` files are in the build
//! output directory.
//!
//! ## Fingerprint calculation
//!
//! After the list of Units has been calculated, the Units are added to the
//! `JobQueue`. As each one is added, the fingerprint is calculated, and the
//! dirty/fresh status is recorded. A closure is used to update the fingerprint
//! on-disk when the Unit successfully finishes. The closure will recompute the
//! Fingerprint based on the updated information. If the Unit fails to compile,
//! the fingerprint is not updated.
//!
//! Fingerprints are cached in the `Context`. This makes computing
//! Fingerprints faster, but also is necessary for properly updating
//! dependency information. Since a Fingerprint includes the Fingerprints of
//! all dependencies, when it is updated, by using `Arc` clones, it
//! automatically picks up the updates to its dependencies.
//!
//! ## Considerations for inclusion in a fingerprint
//!
//! Over time we've realized a few items which historically were included in
//! fingerprint hashings should not actually be included. Examples are:
//!
//! * Modification time values. We strive to never include a modification time
//!   inside a `Fingerprint` to get hashed into an actual value. While
//!   theoretically fine to do, in practice this causes issues with common
//!   applications like Docker. Docker, after a layer is built, will zero out
//!   the nanosecond part of all filesystem modification times. This means that
//!   the actual modification time is different for all build artifacts, which
//!   if we tracked the actual values of modification times would cause
//!   unnecessary recompiles. To fix this we instead only track paths which are
//!   relevant. These paths are checked dynamically to see if they're up to
//!   date, and the modifiation time doesn't make its way into the fingerprint
//!   hash.
//!
//! * Absolute path names. We strive to maintain a property where if you rename
//!   a project directory Cargo will continue to preserve all build artifacts
//!   and reuse the cache. This means that we can't ever hash an absolute path
//!   name. Instead we always hash relative path names and the "root" is passed
//!   in at runtime dynamically. Some of this is best effort, but the general
//!   idea is that we assume all accesses within a crate stay within that
//!   crate.
//!
//! These are pretty tricky to test for unfortunately, but we should have a good
//! test suite nowadays and lord knows Cargo gets enough testing in the wild!
//!
//! ## Build scripts
//!
//! The *running* of a build script (`CompileMode::RunCustomBuild`) is treated
//! significantly different than all other Unit kinds. It has its own function
//! for calculating the Fingerprint (`calculate_run_custom_build`) and has some
//! unique considerations. It does not track the same information as a normal
//! Unit. The information tracked depends on the `rerun-if-changed` and
//! `rerun-if-env-changed` statements produced by the build script. If the
//! script does not emit either of these statements, the Fingerprint runs in
//! "old style" mode where an mtime change of *any* file in the package will
//! cause the build script to be re-run. Otherwise, the fingerprint *only*
//! tracks the individual "rerun-if" items listed by the build script.
//!
//! The "rerun-if" statements from a *previous* build are stored in the build
//! output directory in a file called `output`. Cargo parses this file when
//! the Unit for that build script is prepared for the `JobQueue`. The
//! Fingerprint code can then use that information to compute the Fingerprint
//! and compare against the old fingerprint hash.
//!
//! Care must be taken with build script Fingerprints because the
//! `Fingerprint::local` value may be changed after the build script runs
//! (such as if the build script adds or removes "rerun-if" items).
//!
//! Another complication is if a build script is overridden. In that case, the
//! fingerprint is the hash of the output of the override.
//!
//! ## Special considerations
//!
//! Registry dependencies do not track the mtime of files. This is because
//! registry dependencies are not expected to change (if a new version is
//! used, the Package ID will change, causing a rebuild). Cargo currently
//! partially works with Docker caching. When a Docker image is built, it has
//! normal mtime information. However, when a step is cached, the nanosecond
//! portions of all files is zeroed out. Currently this works, but care must
//! be taken for situations like these.
//!
//! HFS on macOS only supports 1 second timestamps. This causes a significant
//! number of problems, particularly with Cargo's testsuite which does rapid
//! builds in succession. Other filesystems have various degrees of
//! resolution.
//!
//! Various weird filesystems (such as network filesystems) also can cause
//! complications. Network filesystems may track the time on the server
//! (except when the time is set manually such as with
//! `filetime::set_file_times`). Not all filesystems support modifying the
//! mtime.
//!
//! See the `A-rebuild-detection` flag on the issue tracker for more:
//! <https://github.com/rust-lang/cargo/issues?q=is%3Aissue+is%3Aopen+label%3AA-rebuild-detection>

use std::collections::HashMap;
use std::env;
use std::fs;
use std::hash::{self, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use failure::{bail, format_err};
use filetime::FileTime;
use log::{debug, info};
use serde::de;
use serde::ser;
use serde::{Deserialize, Serialize};

use crate::core::Package;
use crate::util;
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::paths;
use crate::util::{internal, profile};

use super::custom_build::BuildDeps;
use super::job::{
    Freshness::{Dirty, Fresh},
    Job, Work,
};
use super::{BuildContext, Context, FileFlavor, Kind, Unit};

/// Determines if a `unit` is up-to-date, and if not prepares necessary work to
/// update the persisted fingerprint.
///
/// This function will inspect `unit`, calculate a fingerprint for it, and then
/// return an appropriate `Job` to run. The returned `Job` will be a noop if
/// `unit` is considered "fresh", or if it was previously built and cached.
/// Otherwise the `Job` returned will write out the true fingerprint to the
/// filesystem, to be executed after the unit's work has completed.
///
/// The `force` flag is a way to force the `Job` to be "dirty", or always
/// update the fingerprint. **Beware using this flag** because it does not
/// transitively propagate throughout the dependency graph, it only forces this
/// one unit which is very unlikely to be what you want unless you're
/// exclusively talking about top-level units.
pub fn prepare_target<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
    force: bool,
) -> CargoResult<Job> {
    let _p = profile::start(format!(
        "fingerprint: {} / {}",
        unit.pkg.package_id(),
        unit.target.name()
    ));
    let bcx = cx.bcx;
    let new = cx.files().fingerprint_dir(unit);
    let loc = new.join(&filename(cx, unit));

    debug!("fingerprint at: {}", loc.display());

    // Figure out if this unit is up to date. After calculating the fingerprint
    // compare it to an old version, if any, and attempt to print diagnostic
    // information about failed comparisons to aid in debugging.
    let fingerprint = calculate(cx, unit)?;
    let mtime_on_use = cx.bcx.config.cli_unstable().mtime_on_use;
    let compare = compare_old_fingerprint(&loc, &*fingerprint, mtime_on_use);
    log_compare(unit, &compare);

    // If our comparison failed (e.g., we're going to trigger a rebuild of this
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

    if compare.is_ok() && !force {
        return Ok(Job::new(Work::noop(), Fresh));
    }

    let write_fingerprint = if unit.mode.is_run_custom_build() {
        // For build scripts the `local` field of the fingerprint may change
        // while we're executing it. For example it could be in the legacy
        // "consider everything a dependency mode" and then we switch to "deps
        // are explicitly specified" mode.
        //
        // To handle this movement we need to regenerate the `local` field of a
        // build script's fingerprint after it's executed. We do this by
        // using the `build_script_local_fingerprints` function which returns a
        // thunk we can invoke on a foreign thread to calculate this.
        let state = Arc::clone(&cx.build_state);
        let key = (unit.pkg.package_id(), unit.kind);
        let (gen_local, _overridden) = build_script_local_fingerprints(cx, unit);
        let output_path = cx.build_explicit_deps[unit].build_script_output.clone();
        Work::new(move |_| {
            let outputs = state.outputs.lock().unwrap();
            let outputs = &outputs[&key];
            let deps = BuildDeps::new(&output_path, Some(outputs));

            // FIXME: it's basically buggy that we pass `None` to `call_box`
            // here. See documentation on `build_script_local_fingerprints`
            // below for more information. Despite this just try to proceed and
            // hobble along if it happens to return `Some`.
            if let Some(new_local) = gen_local.call_box(&deps, None)? {
                *fingerprint.local.lock().unwrap() = new_local;
                *fingerprint.memoized_hash.lock().unwrap() = None;
            }

            write_fingerprint(&loc, &fingerprint)
        })
    } else {
        Work::new(move |_| write_fingerprint(&loc, &fingerprint))
    };

    Ok(Job::new(write_fingerprint, Dirty))
}

/// Dependency edge information for fingerprints. This is generated for each
/// unit in `dep_targets` and is stored in a `Fingerprint` below.
#[derive(Clone)]
struct DepFingerprint {
    /// The hash of the package id that this dependency points to
    pkg_id: u64,
    /// The crate name we're using for this dependency, which if we change we'll
    /// need to recompile!
    name: String,
    /// Whether or not this dependency is flagged as a public dependency or not.
    public: bool,
    /// Whether or not this dependency is an rmeta dependency or a "full"
    /// dependency. In the case of an rmeta dependency our dependency edge only
    /// actually requires the rmeta from what we depend on, so when checking
    /// mtime information all files other than the rmeta can be ignored.
    only_requires_rmeta: bool,
    /// The dependency's fingerprint we recursively point to, containing all the
    /// other hash information we'd otherwise need.
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
    /// Hash of the version of `rustc` used.
    rustc: u64,
    /// Sorted list of cfg features enabled.
    features: String,
    /// Hash of the `Target` struct, including the target name,
    /// package-relative source path, edition, etc.
    target: u64,
    /// Hash of the `Profile`, `CompileMode`, and any extra flags passed via
    /// `cargo rustc` or `cargo rustdoc`.
    profile: u64,
    /// Hash of the path to the base source file. This is relative to the
    /// workspace root for path members, or absolute for other sources.
    path: u64,
    /// Fingerprints of dependencies.
    deps: Vec<DepFingerprint>,
    /// Information about the inputs that affect this Unit (such as source
    /// file mtimes or build script environment variables).
    local: Mutex<Vec<LocalFingerprint>>,
    /// Cached hash of the `Fingerprint` struct. Used to improve performance
    /// for hashing.
    #[serde(skip)]
    memoized_hash: Mutex<Option<u64>>,
    /// RUSTFLAGS/RUSTDOCFLAGS environment variable value (or config value).
    rustflags: Vec<String>,
    /// Hash of some metadata from the manifest, such as "authors", or
    /// "description", which are exposed as environment variables during
    /// compilation.
    metadata: u64,
    /// Description of whether the filesystem status for this unit is up to date
    /// or should be considered stale.
    #[serde(skip)]
    fs_status: FsStatus,
    /// Files, relative to `target_root`, that are produced by the step that
    /// this `Fingerprint` represents. This is used to detect when the whole
    /// fingerprint is out of date if this is missing, or if previous
    /// fingerprints output files are regenerated and look newer than this one.
    #[serde(skip)]
    outputs: Vec<PathBuf>,
}

/// Indication of the status on the filesystem for a particular unit.
enum FsStatus {
    /// This unit is to be considered stale, even if hash information all
    /// matches. The filesystem inputs have changed (or are missing) and the
    /// unit needs to subsequently be recompiled.
    Stale,

    /// This unit is up-to-date. All outputs and their corresponding mtime are
    /// listed in the payload here for other dependencies to compare against.
    UpToDate { mtimes: HashMap<PathBuf, FileTime> },
}

impl FsStatus {
    fn up_to_date(&self) -> bool {
        match self {
            FsStatus::UpToDate { .. } => true,
            FsStatus::Stale => false,
        }
    }
}

impl Default for FsStatus {
    fn default() -> FsStatus {
        FsStatus::Stale
    }
}

impl Serialize for DepFingerprint {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        (
            &self.pkg_id,
            &self.name,
            &self.public,
            &self.fingerprint.hash(),
        )
            .serialize(ser)
    }
}

impl<'de> Deserialize<'de> for DepFingerprint {
    fn deserialize<D>(d: D) -> Result<DepFingerprint, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let (pkg_id, name, public, hash) = <(u64, String, bool, u64)>::deserialize(d)?;
        Ok(DepFingerprint {
            pkg_id,
            name,
            public,
            fingerprint: Arc::new(Fingerprint {
                memoized_hash: Mutex::new(Some(hash)),
                ..Fingerprint::new()
            }),
            // This field is never read since it's only used in
            // `check_filesystem` which isn't used by fingerprints loaded from
            // disk.
            only_requires_rmeta: false,
        })
    }
}

/// A `LocalFingerprint` represents something that we use to detect direct
/// changes to a `Fingerprint`.
///
/// This is where we track file information, env vars, etc. This
/// `LocalFingerprint` struct is hashed and if the hash changes will force a
/// recompile of any fingerprint it's included into. Note that the "local"
/// terminology comes from the fact that it only has to do with one crate, and
/// `Fingerprint` tracks the transitive propagation of fingerprint changes.
///
/// Note that because this is hashed its contents are carefully managed. Like
/// mentioned in the above module docs, we don't want to hash absolute paths or
/// mtime information.
///
/// Also note that a `LocalFingerprint` is used in `check_filesystem` to detect
/// when the filesystem contains stale information (based on mtime currently).
/// The paths here don't change much between compilations but they're used as
/// inputs when we probe the filesystem looking at information.
#[derive(Debug, Serialize, Deserialize, Hash)]
enum LocalFingerprint {
    /// This is a precalculated fingerprint which has an opaque string we just
    /// hash as usual. This variant is primarily used for git/crates.io
    /// dependencies where the source never changes so we can quickly conclude
    /// that there's some string we can hash and it won't really change much.
    ///
    /// This is also used for build scripts with no `rerun-if-*` statements, but
    /// that's overall a mistake and causes bugs in Cargo. We shouldn't use this
    /// for build scripts.
    Precalculated(String),

    /// This is used for crate compilations. The `dep_info` file is a relative
    /// path anchored at `target_root(...)` to the dep-info file that Cargo
    /// generates (which is a custom serialization after parsing rustc's own
    /// `dep-info` output).
    ///
    /// The `dep_info` file, when present, also lists a number of other files
    /// for us to look at. If any of those files are newer than this file then
    /// we need to recompile.
    CheckDepInfo { dep_info: PathBuf },

    /// This represents a nonempty set of `rerun-if-changed` annotations printed
    /// out by a build script. The `output` file is a arelative file anchored at
    /// `target_root(...)` which is the actual output of the build script. That
    /// output has already been parsed and the paths printed out via
    /// `rerun-if-changed` are listed in `paths`. The `paths` field is relative
    /// to `pkg.root()`
    ///
    /// This is considered up-to-date if all of the `paths` are older than
    /// `output`, otherwise we need to recompile.
    RerunIfChanged {
        output: PathBuf,
        paths: Vec<PathBuf>,
    },

    /// This represents a single `rerun-if-env-changed` annotation printed by a
    /// build script. The exact env var and value are hashed here. There's no
    /// filesystem dependence here, and if the values are changed the hash will
    /// change forcing a recompile.
    RerunIfEnvChanged { var: String, val: Option<String> },
}

enum StaleFile {
    Missing(PathBuf),
    Changed {
        reference: PathBuf,
        reference_mtime: FileTime,
        stale: PathBuf,
        stale_mtime: FileTime,
    },
}

impl LocalFingerprint {
    /// Checks dynamically at runtime if this `LocalFingerprint` has a stale
    /// file.
    ///
    /// This will use the absolute root paths passed in if necessary to guide
    /// file accesses.
    fn find_stale_file(
        &self,
        pkg_root: &Path,
        target_root: &Path,
    ) -> CargoResult<Option<StaleFile>> {
        match self {
            // We need to parse `dep_info`, learn about all the files the crate
            // depends on, and then see if any of them are newer than the
            // dep_info file itself. If the `dep_info` file is missing then this
            // unit has never been compiled!
            LocalFingerprint::CheckDepInfo { dep_info } => {
                let dep_info = target_root.join(dep_info);
                if let Some(paths) = parse_dep_info(pkg_root, target_root, &dep_info)? {
                    Ok(find_stale_file(&dep_info, paths.iter()))
                } else {
                    Ok(Some(StaleFile::Missing(dep_info)))
                }
            }

            // We need to verify that no paths listed in `paths` are newer than
            // the `output` path itself, or the last time the build script ran.
            LocalFingerprint::RerunIfChanged { output, paths } => Ok(find_stale_file(
                &target_root.join(output),
                paths.iter().map(|p| pkg_root.join(p)),
            )),

            // These have no dependencies on the filesystem, and their values
            // are included natively in the `Fingerprint` hash so nothing
            // tocheck for here.
            LocalFingerprint::RerunIfEnvChanged { .. } => Ok(None),
            LocalFingerprint::Precalculated(..) => Ok(None),
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            LocalFingerprint::Precalculated(..) => "precalculated",
            LocalFingerprint::CheckDepInfo { .. } => "dep-info",
            LocalFingerprint::RerunIfChanged { .. } => "rerun-if-changed",
            LocalFingerprint::RerunIfEnvChanged { .. } => "rerun-if-env-changed",
        }
    }
}

#[derive(Debug)]
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
            local: Mutex::new(Vec::new()),
            memoized_hash: Mutex::new(None),
            rustflags: Vec::new(),
            metadata: 0,
            fs_status: FsStatus::Stale,
            outputs: Vec::new(),
        }
    }

    fn hash(&self) -> u64 {
        if let Some(s) = *self.memoized_hash.lock().unwrap() {
            return s;
        }
        let ret = util::hash_u64(self);
        *self.memoized_hash.lock().unwrap() = Some(ret);
        ret
    }

    /// Compares this fingerprint with an old version which was previously
    /// serialized to filesystem.
    ///
    /// The purpose of this is exclusively to produce a diagnostic message
    /// indicating why we're recompiling something. This function always returns
    /// an error, it will never return success.
    fn compare(&self, old: &Fingerprint) -> CargoResult<()> {
        if self.rustc != old.rustc {
            bail!("rust compiler has changed")
        }
        if self.features != old.features {
            bail!(
                "features have changed: {} != {}",
                self.features,
                old.features
            )
        }
        if self.target != old.target {
            bail!("target configuration has changed")
        }
        if self.path != old.path {
            bail!("path to the compiler has changed")
        }
        if self.profile != old.profile {
            bail!("profile configuration has changed")
        }
        if self.rustflags != old.rustflags {
            bail!("RUSTFLAGS has changed")
        }
        if self.metadata != old.metadata {
            bail!("metadata changed")
        }
        let my_local = self.local.lock().unwrap();
        let old_local = old.local.lock().unwrap();
        if my_local.len() != old_local.len() {
            bail!("local lens changed");
        }
        for (new, old) in my_local.iter().zip(old_local.iter()) {
            match (new, old) {
                (LocalFingerprint::Precalculated(a), LocalFingerprint::Precalculated(b)) => {
                    if a != b {
                        bail!("precalculated components have changed: {} != {}", a, b)
                    }
                }
                (
                    LocalFingerprint::CheckDepInfo { dep_info: adep },
                    LocalFingerprint::CheckDepInfo { dep_info: bdep },
                ) => {
                    if adep != bdep {
                        bail!("dep info output changed: {:?} != {:?}", adep, bdep)
                    }
                }
                (
                    LocalFingerprint::RerunIfChanged {
                        output: aout,
                        paths: apaths,
                    },
                    LocalFingerprint::RerunIfChanged {
                        output: bout,
                        paths: bpaths,
                    },
                ) => {
                    if aout != bout {
                        bail!("rerun-if-changed output changed: {:?} != {:?}", aout, bout)
                    }
                    if apaths != bpaths {
                        bail!(
                            "rerun-if-changed output changed: {:?} != {:?}",
                            apaths,
                            bpaths,
                        )
                    }
                }
                (
                    LocalFingerprint::RerunIfEnvChanged {
                        var: akey,
                        val: avalue,
                    },
                    LocalFingerprint::RerunIfEnvChanged {
                        var: bkey,
                        val: bvalue,
                    },
                ) => {
                    if *akey != *bkey {
                        bail!("env vars changed: {} != {}", akey, bkey);
                    }
                    if *avalue != *bvalue {
                        bail!(
                            "env var `{}` changed: previously {:?} now {:?}",
                            akey,
                            bvalue,
                            avalue
                        )
                    }
                }
                (a, b) => bail!(
                    "local fingerprint type has changed ({} => {})",
                    b.kind(),
                    a.kind()
                ),
            }
        }

        if self.deps.len() != old.deps.len() {
            bail!("number of dependencies has changed")
        }
        for (a, b) in self.deps.iter().zip(old.deps.iter()) {
            if a.name != b.name {
                let e = format_err!("`{}` != `{}`", a.name, b.name)
                    .context("unit dependency name changed");
                return Err(e.into());
            }

            if a.fingerprint.hash() != b.fingerprint.hash() {
                let e = format_err!(
                    "new ({}/{:x}) != old ({}/{:x})",
                    a.name,
                    a.fingerprint.hash(),
                    b.name,
                    b.fingerprint.hash()
                )
                .context("unit dependency information changed");
                return Err(e.into());
            }
        }

        if !self.fs_status.up_to_date() {
            bail!("current filesystem status shows we're outdated");
        }

        // This typically means some filesystem modifications happened or
        // something transitive was odd. In general we should strive to provide
        // a better error message than this, so if you see this message a lot it
        // likely means this method needs to be updated!
        bail!("two fingerprint comparison turned up nothing obvious");
    }

    /// Dynamically inspect the local filesystem to update the `fs_status` field
    /// of this `Fingerprint`.
    ///
    /// This function is used just after a `Fingerprint` is constructed to check
    /// the local state of the filesystem and propagate any dirtiness from
    /// dependencies up to this unit as well. This function assumes that the
    /// unit starts out as `FsStatus::Stale` and then it will optionally switch
    /// it to `UpToDate` if it can.
    fn check_filesystem(
        &mut self,
        pkg_root: &Path,
        target_root: &Path,
    ) -> CargoResult<()> {
        assert!(!self.fs_status.up_to_date());

        let mut mtimes = HashMap::new();

        // Get the `mtime` of all outputs. Optionally update their mtime
        // afterwards based on the `mtime_on_use` flag. Afterwards we want the
        // minimum mtime as it's the one we'll be comparing to inputs and
        // dependencies.
        for output in self.outputs.iter() {
            let mtime = match paths::mtime(output) {
                Ok(mtime) => mtime,

                // This path failed to report its `mtime`. It probably doesn't
                // exists, so leave ourselves as stale and bail out.
                Err(e) => {
                    log::debug!("failed to get mtime of {:?}: {}", output, e);
                    return Ok(());
                }
            };
            assert!(mtimes.insert(output.clone(), mtime).is_none());
        }

        let max_mtime = match mtimes.values().max() {
            Some(mtime) => mtime,

            // We had no output files. This means we're an overridden build
            // script and we're just always up to date because we aren't
            // watching the filesystem.
            None => {
                self.fs_status = FsStatus::UpToDate { mtimes };
                return Ok(());
            }
        };

        for dep in self.deps.iter() {
            let dep_mtimes = match &dep.fingerprint.fs_status {
                FsStatus::UpToDate { mtimes } => mtimes,
                // If our dependency is stale, so are we, so bail out.
                FsStatus::Stale => return Ok(()),
            };

            // If our dependency edge only requires the rmeta file to be present
            // then we only need to look at that one output file, otherwise we
            // need to consider all output files to see if we're out of date.
            let dep_mtime = if dep.only_requires_rmeta {
                dep_mtimes
                    .iter()
                    .filter_map(|(path, mtime)| {
                        if path.extension().and_then(|s| s.to_str()) == Some("rmeta") {
                            Some(mtime)
                        } else {
                            None
                        }
                    })
                    .next()
                    .expect("failed to find rmeta")
            } else {
                match dep_mtimes.values().max() {
                    Some(mtime) => mtime,
                    // If our dependencies is up to date and has no filesystem
                    // interactions, then we can move on to the next dependency.
                    None => continue,
                }
            };

            // If the dependency is newer than our own output then it was
            // recompiled previously. We transitively become stale ourselves in
            // that case, so bail out.
            //
            // Note that this comparison should probably be `>=`, not `>`, but
            // for a discussion of why it's `>` see the discussion about #5918
            // below in `find_stale`.
            if dep_mtime > max_mtime {
                log::info!("dependency on `{}` is newer than we are", dep.name);
                return Ok(());
            }
        }

        // If we reached this far then all dependencies are up to date. Check
        // all our `LocalFingerprint` information to see if we have any stale
        // files for this package itself. If we do find something log a helpful
        // message and bail out so we stay stale.
        for local in self.local.get_mut().unwrap().iter() {
            if let Some(file) = local.find_stale_file(pkg_root, target_root)? {
                file.log();
                return Ok(());
            }
        }

        // Everything was up to date! Record such.
        self.fs_status = FsStatus::UpToDate { mtimes };

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
            metadata,
            ref rustflags,
            ..
        } = *self;
        let local = local.lock().unwrap();
        (
            rustc, features, target, path, profile, &*local, metadata, rustflags,
        )
            .hash(h);

        h.write_usize(deps.len());
        for DepFingerprint {
            pkg_id,
            name,
            public,
            fingerprint,
            only_requires_rmeta: _, // static property, no need to hash
        } in deps
        {
            pkg_id.hash(h);
            name.hash(h);
            public.hash(h);
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

impl DepFingerprint {
    fn new<'a, 'cfg>(
        cx: &mut Context<'a, 'cfg>,
        parent: &Unit<'a>,
        dep: &Unit<'a>,
    ) -> CargoResult<DepFingerprint> {
        let fingerprint = calculate(cx, dep)?;
        let name = cx.bcx.extern_crate_name(parent, dep)?;
        let public = cx.bcx.is_public_dependency(parent, dep);

        // We need to be careful about what we hash here. We have a goal of
        // supporting renaming a project directory and not rebuilding
        // everything. To do that, however, we need to make sure that the cwd
        // doesn't make its way into any hashes, and one source of that is the
        // `SourceId` for `path` packages.
        //
        // We already have a requirement that `path` packages all have unique
        // names (sort of for this same reason), so if the package source is a
        // `path` then we just hash the name, but otherwise we hash the full
        // id as it won't change when the directory is renamed.
        let pkg_id = if dep.pkg.package_id().source_id().is_path() {
            util::hash_u64(dep.pkg.package_id().name())
        } else {
            util::hash_u64(dep.pkg.package_id())
        };

        Ok(DepFingerprint {
            pkg_id,
            name,
            public,
            fingerprint,
            only_requires_rmeta: cx.only_requires_rmeta(parent, dep),
        })
    }
}

impl StaleFile {
    /// Use the `log` crate to log a hopefully helpful message in diagnosing
    /// what file is considered stale and why. This is intended to be used in
    /// conjunction with `CARGO_LOG` to determine why Cargo is recompiling
    /// something. Currently there's no user-facing usage of this other than
    /// that.
    fn log(&self) {
        match self {
            StaleFile::Missing(path) => {
                log::info!("stale: missing {:?}", path);
            }
            StaleFile::Changed {
                reference,
                reference_mtime,
                stale,
                stale_mtime,
            } => {
                log::info!("stale: changed {:?}", stale);
                log::info!("          (vs) {:?}", reference);
                log::info!("               {:?} != {:?}", reference_mtime, stale_mtime);
            }
        }
    }
}

/// Calculates the fingerprint for a `unit`.
///
/// This fingerprint is used by Cargo to learn about when information such as:
///
/// * A non-path package changes (changes version, changes revision, etc).
/// * Any dependency changes
/// * The compiler changes
/// * The set of features a package is built with changes
/// * The profile a target is compiled with changes (e.g., opt-level changes)
/// * Any other compiler flags change that will affect the result
///
/// Information like file modification time is only calculated for path
/// dependencies.
fn calculate<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
) -> CargoResult<Arc<Fingerprint>> {
    // This function is slammed quite a lot, so the result is memoized.
    if let Some(s) = cx.fingerprints.get(unit) {
        return Ok(Arc::clone(s));
    }
    let mut fingerprint = if unit.mode.is_run_custom_build() {
        calculate_run_custom_build(cx, unit)?
    } else if unit.mode.is_doc_test() {
        panic!("doc tests do not fingerprint");
    } else {
        calculate_normal(cx, unit)?
    };

    // After we built the initial `Fingerprint` be sure to update the
    // `fs_status` field of it.
    let target_root = target_root(cx, unit);
    fingerprint.check_filesystem(unit.pkg.root(), &target_root)?;

    let fingerprint = Arc::new(fingerprint);
    cx.fingerprints.insert(*unit, Arc::clone(&fingerprint));
    Ok(fingerprint)
}

/// Calculate a fingerprint for a "normal" unit, or anything that's not a build
/// script. This is an internal helper of `calculate`, don't call directly.
fn calculate_normal<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
) -> CargoResult<Fingerprint> {
    // Recursively calculate the fingerprint for all of our dependencies.
    //
    // Skip fingerprints of binaries because they don't actually induce a
    // recompile, they're just dependencies in the sense that they need to be
    // built.
    let mut deps = cx
        .dep_targets(unit)
        .iter()
        .filter(|u| !u.target.is_bin())
        .map(|dep| DepFingerprint::new(cx, unit, dep))
        .collect::<CargoResult<Vec<_>>>()?;
    deps.sort_by(|a, b| a.pkg_id.cmp(&b.pkg_id));

    // Afterwards calculate our own fingerprint information. We specially
    // handle `path` packages to ensure we track files on the filesystem
    // correctly, but otherwise upstream packages like from crates.io or git
    // get bland fingerprints because they don't change without their
    // `PackageId` changing.
    let target_root = target_root(cx, unit);
    let local = if use_dep_info(unit) {
        let dep_info = dep_info_loc(cx, unit);
        let dep_info = dep_info.strip_prefix(&target_root).unwrap().to_path_buf();
        vec![LocalFingerprint::CheckDepInfo { dep_info }]
    } else {
        let fingerprint = pkg_fingerprint(cx.bcx, unit.pkg)?;
        vec![LocalFingerprint::Precalculated(fingerprint)]
    };

    // Figure out what the outputs of our unit is, and we'll be storing them
    // into the fingerprint as well.
    let outputs = cx
        .outputs(unit)?
        .iter()
        .filter(|output| output.flavor != FileFlavor::DebugInfo)
        .map(|output| output.path.clone())
        .collect();

    // Fill out a bunch more information that we'll be tracking typically
    // hashed to take up less space on disk as we just need to know when things
    // change.
    let extra_flags = if unit.mode.is_doc() {
        cx.bcx.rustdocflags_args(unit)
    } else {
        cx.bcx.rustflags_args(unit)
    };
    let profile_hash = util::hash_u64((&unit.profile, unit.mode, cx.bcx.extra_args_for(unit)));
    // Include metadata since it is exposed as environment variables.
    let m = unit.pkg.manifest().metadata();
    let metadata = util::hash_u64((&m.authors, &m.description, &m.homepage, &m.repository));
    Ok(Fingerprint {
        rustc: util::hash_u64(&cx.bcx.rustc.verbose_version),
        target: util::hash_u64(&unit.target),
        profile: profile_hash,
        // Note that .0 is hashed here, not .1 which is the cwd. That doesn't
        // actually affect the output artifact so there's no need to hash it.
        path: util::hash_u64(super::path_args(cx.bcx, unit).0),
        features: format!(
            "{:?}",
            cx.bcx.resolve.features_sorted(unit.pkg.package_id())
        ),
        deps,
        local: Mutex::new(local),
        memoized_hash: Mutex::new(None),
        metadata,
        rustflags: extra_flags.to_vec(),
        fs_status: FsStatus::Stale,
        outputs,
    })
}

// We want to use the mtime for files if we're a path source, but if we're a
// git/registry source, then the mtime of files may fluctuate, but they won't
// change so long as the source itself remains constant (which is the
// responsibility of the source)
fn use_dep_info(unit: &Unit<'_>) -> bool {
    let path = unit.pkg.summary().source_id().is_path();
    !unit.mode.is_doc() && path
}

/// Calculate a fingerprint for an "execute a build script" unit.  This is an
/// internal helper of `calculate`, don't call directly.
fn calculate_run_custom_build<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
) -> CargoResult<Fingerprint> {
    // Using the `BuildDeps` information we'll have previously parsed and
    // inserted into `build_explicit_deps` built an initial snapshot of the
    // `LocalFingerprint` list for this build script. If we previously executed
    // the build script this means we'll be watching files and env vars.
    // Otherwise if we haven't previously executed it we'll just start watching
    // the whole crate.
    let (gen_local, overridden) = build_script_local_fingerprints(cx, unit);
    let deps = &cx.build_explicit_deps[unit];
    let local = gen_local
        .call_box(deps, Some(&|| pkg_fingerprint(cx.bcx, unit.pkg)))?
        .unwrap();
    let output = deps.build_script_output.clone();

    // Include any dependencies of our execution, which is typically just the
    // compilation of the build script itself. (if the build script changes we
    // should be rerun!). Note though that if we're an overridden build script
    // we have no dependencies so no need to recurse in that case.
    let deps = if overridden {
        // Overridden build scripts don't need to track deps.
        vec![]
    } else {
        cx.dep_targets(unit)
            .iter()
            .map(|dep| DepFingerprint::new(cx, unit, dep))
            .collect::<CargoResult<Vec<_>>>()?
    };

    Ok(Fingerprint {
        local: Mutex::new(local),
        rustc: util::hash_u64(&cx.bcx.rustc.verbose_version),
        deps,
        outputs: if overridden { Vec::new() } else { vec![output] },

        // Most of the other info is blank here as we don't really include it
        // in the execution of the build script, but... this may be a latent
        // bug in Cargo.
        ..Fingerprint::new()
    })
}

/// Get ready to compute the `LocalFingerprint` values for a `RunCustomBuild`
/// unit.
///
/// This function has, what's on the surface, a seriously wonky interface.
/// You'll call this function and it'll return a closure and a boolean. The
/// boolean is pretty simple in that it indicates whether the `unit` has been
/// overridden via `.cargo/config`. The closure is much more complicated.
///
/// This closure is intended to capture any local state necessary to compute
/// the `LocalFingerprint` values for this unit. It is `Send` and `'static` to
/// be sent to other threads as well (such as when we're executing build
/// scripts). That deduplication is the rationale for the closure at least.
///
/// The arguments to the closure are a bit weirder, though, and I'll apologize
/// in advance for the weirdness too. The first argument to the closure (see
/// `MyFnOnce` below) is a `&BuildDeps`. This is the parsed version of a build
/// script, and when Cargo starts up this is cached from previous runs of a
/// build script. After a build script executes the output file is reparsed and
/// passed in here.
///
/// The second argument is the weirdest, it's *optionally* a closure to
/// call `pkg_fingerprint` below. The `pkg_fingerprint` below requires access
/// to "source map" located in `Context`. That's very non-`'static` and
/// non-`Send`, so it can't be used on other threads, such as when we invoke
/// this after a build script has finished. The `Option` allows us to for sure
/// calculate it on the main thread at the beginning, and then swallow the bug
/// for now where a worker thread after a build script has finished doesn't
/// have access. Ideally there would be no second argument or it would be more
/// "first class" and not an `Option` but something that can be sent between
/// threads. In any case, it's a bug for now.
///
/// This isn't the greatest of interfaces, and if there's suggestions to
/// improve please do so!
///
/// FIXME(#6779) - see all the words above
fn build_script_local_fingerprints<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
) -> (Box<dyn MyFnOnce + Send>, bool) {
    // First up, if this build script is entirely overridden, then we just
    // return the hash of what we overrode it with. This is the easy case!
    if let Some(fingerprint) = build_script_override_fingerprint(cx, unit) {
        debug!("override local fingerprints deps");
        return (
            Box::new(
                move |_: &BuildDeps, _: Option<&dyn Fn() -> CargoResult<String>>| {
                    Ok(Some(vec![fingerprint]))
                },
            ),
            true, // this is an overridden build script
        );
    }

    // ... Otherwise this is a "real" build script and we need to return a real
    // closure. Our returned closure classifies the build script based on
    // whether it prints `rerun-if-*`. If it *doesn't* print this it's where the
    // magical second argument comes into play, which fingerprints a whole
    // package. Remember that the fact that this is an `Option` is a bug, but a
    // longstanding bug, in Cargo. Recent refactorings just made it painfully
    // obvious.
    let script_root = cx.files().build_script_run_dir(unit);
    let pkg_root = unit.pkg.root().to_path_buf();
    let calculate =
        move |deps: &BuildDeps, pkg_fingerprint: Option<&dyn Fn() -> CargoResult<String>>| {
            if deps.rerun_if_changed.is_empty() && deps.rerun_if_env_changed.is_empty() {
                match pkg_fingerprint {
                    // FIXME: this is somewhat buggy with respect to docker and
                    // weird filesystems. The `Precalculated` variant
                    // constructed below will, for `path` dependencies, contain
                    // a stringified version of the mtime for the local crate.
                    // This violates one of the things we describe in this
                    // module's doc comment, never hashing mtimes. We should
                    // figure out a better scheme where a package fingerprint
                    // may be a string (like for a registry) or a list of files
                    // (like for a path dependency). Those list of files would
                    // be stored here rather than the the mtime of them.
                    Some(f) => {
                        debug!("old local fingerprints deps");
                        let s = f()?;
                        return Ok(Some(vec![LocalFingerprint::Precalculated(s)]));
                    }
                    None => return Ok(None),
                }
            }

            // Ok so now we're in "new mode" where we can have files listed as
            // dependencies as well as env vars listed as dependencies. Process
            // them all here.
            Ok(Some(local_fingerprints_deps(deps, &script_root, &pkg_root)))
        };

    // Note that `false` == "not overridden"
    (Box::new(calculate), false)
}

/// Create a `LocalFingerprint` for an overridden build script.
/// Returns None if it is not overridden.
fn build_script_override_fingerprint<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
) -> Option<LocalFingerprint> {
    let state = cx.build_state.outputs.lock().unwrap();
    let output = state.get(&(unit.pkg.package_id(), unit.kind))?;
    let s = format!(
        "overridden build state with hash: {}",
        util::hash_u64(output)
    );
    Some(LocalFingerprint::Precalculated(s))
}

/// Compute the `LocalFingerprint` values for a `RunCustomBuild` unit for
/// non-overridden new-style build scripts only. This is only used when `deps`
/// is already known to have a nonempty `rerun-if-*` somewhere.
fn local_fingerprints_deps(
    deps: &BuildDeps,
    target_root: &Path,
    pkg_root: &Path,
) -> Vec<LocalFingerprint> {
    debug!("new local fingerprints deps");
    let mut local = Vec::new();

    if !deps.rerun_if_changed.is_empty() {
        // Note that like the module comment above says we are careful to never
        // store an absolute path in `LocalFingerprint`, so ensure that we strip
        // absolute prefixes from them.
        let output = deps
            .build_script_output
            .strip_prefix(target_root)
            .unwrap()
            .to_path_buf();
        let paths = deps
            .rerun_if_changed
            .iter()
            .map(|p| p.strip_prefix(pkg_root).unwrap_or(p).to_path_buf())
            .collect();
        local.push(LocalFingerprint::RerunIfChanged { output, paths });
    }

    for var in deps.rerun_if_env_changed.iter() {
        let val = env::var(var).ok();
        local.push(LocalFingerprint::RerunIfEnvChanged {
            var: var.clone(),
            val,
        });
    }

    local
}

fn write_fingerprint(loc: &Path, fingerprint: &Fingerprint) -> CargoResult<()> {
    debug_assert_ne!(fingerprint.rustc, 0);
    // fingerprint::new().rustc == 0, make sure it doesn't make it to the file system.
    // This is mostly so outside tools can reliably find out what rust version this file is for,
    // as we can use the full hash.
    let hash = fingerprint.hash();
    debug!("write fingerprint ({:x}) : {}", hash, loc.display());
    paths::write(loc, util::to_hex(hash).as_bytes())?;

    let json = serde_json::to_string(fingerprint).unwrap();
    if cfg!(debug_assertions) {
        let f: Fingerprint = serde_json::from_str(&json).unwrap();
        assert_eq!(f.hash(), hash);
    }
    paths::write(&loc.with_extension("json"), json.as_bytes())?;
    Ok(())
}

/// Prepare for work when a package starts to build
pub fn prepare_init<'a, 'cfg>(cx: &mut Context<'a, 'cfg>, unit: &Unit<'a>) -> CargoResult<()> {
    let new1 = cx.files().fingerprint_dir(unit);

    // Doc tests have no output, thus no fingerprint.
    if !new1.exists() && !unit.mode.is_doc_test() {
        fs::create_dir(&new1)?;
    }

    Ok(())
}

/// Returns the location that the dep-info file will show up at for the `unit`
/// specified.
pub fn dep_info_loc<'a, 'cfg>(cx: &mut Context<'a, 'cfg>, unit: &Unit<'a>) -> PathBuf {
    cx.files()
        .fingerprint_dir(unit)
        .join(&format!("dep-{}", filename(cx, unit)))
}

/// Returns an absolute path that the `unit`'s outputs should always be relative
/// to. This `target_root` variable is used to store relative path names in
/// `Fingerprint` instead of absolute pathnames (see module comment).
fn target_root<'a, 'cfg>(cx: &mut Context<'a, 'cfg>, unit: &Unit<'a>) -> PathBuf {
    if unit.mode.is_run_custom_build() {
        cx.files().build_script_run_dir(unit)
    } else if unit.kind == Kind::Host {
        cx.files().host_root().to_path_buf()
    } else {
        cx.files().target_root().to_path_buf()
    }
}

fn compare_old_fingerprint(
    loc: &Path,
    new_fingerprint: &Fingerprint,
    mtime_on_use: bool,
) -> CargoResult<()> {
    let old_fingerprint_short = paths::read(loc)?;

    if mtime_on_use {
        // update the mtime so other cleaners know we used it
        let t = FileTime::from_system_time(SystemTime::now());
        filetime::set_file_times(loc, t, t)?;
    }

    let new_hash = new_fingerprint.hash();

    if util::to_hex(new_hash) == old_fingerprint_short && new_fingerprint.fs_status.up_to_date() {
        return Ok(());
    }

    let old_fingerprint_json = paths::read(&loc.with_extension("json"))?;
    let old_fingerprint: Fingerprint = serde_json::from_str(&old_fingerprint_json)
        .chain_err(|| internal("failed to deserialize json"))?;
    debug_assert_eq!(util::to_hex(old_fingerprint.hash()), old_fingerprint_short);
    let result = new_fingerprint.compare(&old_fingerprint);
    assert!(result.is_err());
    result
}

fn log_compare(unit: &Unit<'_>, compare: &CargoResult<()>) {
    let ce = match compare {
        Ok(..) => return,
        Err(e) => e,
    };
    info!(
        "fingerprint error for {}/{:?}/{:?}",
        unit.pkg, unit.mode, unit.target,
    );
    info!("    err: {}", ce);

    for cause in ce.iter_causes() {
        info!("  cause: {}", cause);
    }
}

// Parse the dep-info into a list of paths
pub fn parse_dep_info(
    pkg_root: &Path,
    target_root: &Path,
    dep_info: &Path,
) -> CargoResult<Option<Vec<PathBuf>>> {
    let data = match paths::read_bytes(dep_info) {
        Ok(data) => data,
        Err(_) => return Ok(None),
    };
    let paths = data
        .split(|&x| x == 0)
        .filter(|x| !x.is_empty())
        .map(|p| {
            let ty = match DepInfoPathType::from_byte(p[0]) {
                Some(ty) => ty,
                None => return Err(internal("dep-info invalid")),
            };
            let path = util::bytes2path(&p[1..])?;
            match ty {
                DepInfoPathType::PackageRootRelative => Ok(pkg_root.join(path)),
                // N.B. path might be absolute here in which case the join will have no effect
                DepInfoPathType::TargetRootRelative => Ok(target_root.join(path)),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if paths.is_empty() {
        Ok(None)
    } else {
        Ok(Some(paths))
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

fn find_stale_file<I>(reference: &Path, paths: I) -> Option<StaleFile>
where
    I: IntoIterator,
    I::Item: AsRef<Path>,
{
    let reference_mtime = match paths::mtime(reference) {
        Ok(mtime) => mtime,
        Err(..) => return Some(StaleFile::Missing(reference.to_path_buf())),
    };

    for path in paths {
        let path = path.as_ref();
        let path_mtime = match paths::mtime(path) {
            Ok(mtime) => mtime,
            Err(..) => return Some(StaleFile::Missing(path.to_path_buf())),
        };

        // TODO: fix #5918.
        // Note that equal mtimes should be considered "stale". For filesystems with
        // not much timestamp precision like 1s this is would be a conservative approximation
        // to handle the case where a file is modified within the same second after
        // a build starts. We want to make sure that incremental rebuilds pick that up!
        //
        // For filesystems with nanosecond precision it's been seen in the wild that
        // its "nanosecond precision" isn't really nanosecond-accurate. It turns out that
        // kernels may cache the current time so files created at different times actually
        // list the same nanosecond precision. Some digging on #5919 picked up that the
        // kernel caches the current time between timer ticks, which could mean that if
        // a file is updated at most 10ms after a build starts then Cargo may not
        // pick up the build changes.
        //
        // All in all, an equality check here would be a conservative assumption that,
        // if equal, files were changed just after a previous build finished.
        // Unfortunately this became problematic when (in #6484) cargo switch to more accurately
        // measuring the start time of builds.
        if path_mtime <= reference_mtime {
            continue;
        }

        return Some(StaleFile::Changed {
            reference: reference.to_path_buf(),
            reference_mtime,
            stale: path.to_path_buf(),
            stale_mtime: path_mtime,
        });
    }

    None
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
    } else if unit.mode.is_run_custom_build() {
        "run-"
    } else {
        ""
    };
    format!("{}{}-{}", flavor, kind, file_stem)
}

#[repr(u8)]
enum DepInfoPathType {
    // src/, e.g. src/lib.rs
    PackageRootRelative = 1,
    // target/debug/deps/lib...
    // or an absolute path /.../sysroot/...
    TargetRootRelative = 2,
}

impl DepInfoPathType {
    fn from_byte(b: u8) -> Option<DepInfoPathType> {
        match b {
            1 => Some(DepInfoPathType::PackageRootRelative),
            2 => Some(DepInfoPathType::TargetRootRelative),
            _ => None,
        }
    }
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
    rustc_cwd: &Path,
    pkg_root: &Path,
    target_root: &Path,
) -> CargoResult<()> {
    let target = parse_rustc_dep_info(rustc_dep_info)?;
    let deps = &target
        .get(0)
        .ok_or_else(|| internal("malformed dep-info format, no targets".to_string()))?
        .1;

    let mut new_contents = Vec::new();
    for file in deps {
        let file = rustc_cwd.join(file);
        let (ty, path) = if let Ok(stripped) = file.strip_prefix(pkg_root) {
            (DepInfoPathType::PackageRootRelative, stripped)
        } else if let Ok(stripped) = file.strip_prefix(target_root) {
            (DepInfoPathType::TargetRootRelative, stripped)
        } else {
            // It's definitely not target root relative, but this is an absolute path (since it was
            // joined to rustc_cwd) and as such re-joining it later to the target root will have no
            // effect.
            assert!(file.is_absolute(), "{:?} is absolute", file);
            (DepInfoPathType::TargetRootRelative, &*file)
        };
        new_contents.push(ty as u8);
        new_contents.extend(util::path2bytes(&path)?);
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

// This trait solely exists for the `build_script_local_fingerprints` function
// above, see documentation there for more information. If we had `Box<dyn
// FnOnce>` we wouldn't need this.
trait MyFnOnce {
    fn call_box(
        self: Box<Self>,
        f: &BuildDeps,
        pkg_fingerprint: Option<&dyn Fn() -> CargoResult<String>>,
    ) -> CargoResult<Option<Vec<LocalFingerprint>>>;
}

impl<F> MyFnOnce for F
where
    F: FnOnce(
        &BuildDeps,
        Option<&dyn Fn() -> CargoResult<String>>,
    ) -> CargoResult<Option<Vec<LocalFingerprint>>>,
{
    fn call_box(
        self: Box<Self>,
        f: &BuildDeps,
        pkg_fingerprint: Option<&dyn Fn() -> CargoResult<String>>,
    ) -> CargoResult<Option<Vec<LocalFingerprint>>> {
        (*self)(f, pkg_fingerprint)
    }
}
