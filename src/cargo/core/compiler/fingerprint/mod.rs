//! Tracks changes to determine if something needs to be recompiled.
//!
//! This module implements change-tracking so that Cargo can know whether or
//! not something needs to be recompiled. A Cargo [`Unit`] can be either "dirty"
//! (needs to be recompiled) or "fresh" (it does not need to be recompiled).
//!
//! ## Mechanisms affecting freshness
//!
//! There are several mechanisms that influence a Unit's freshness:
//!
//! - The [`Fingerprint`] is a hash, saved to the filesystem in the
//!   `.fingerprint` directory, that tracks information about the Unit. If the
//!   fingerprint is missing (such as the first time the unit is being
//!   compiled), then the unit is dirty. If any of the fingerprint fields
//!   change (like the name of the source file), then the Unit is considered
//!   dirty.
//!
//!   The `Fingerprint` also tracks the fingerprints of all its dependencies,
//!   so a change in a dependency will propagate the "dirty" status up.
//!
//! - Filesystem mtime tracking is also used to check if a unit is dirty.
//!   See the section below on "Mtime comparison" for more details. There
//!   are essentially two parts to mtime tracking:
//!
//!   1. The mtime of a Unit's output files is compared to the mtime of all
//!      its dependencies' output file mtimes (see
//!      [`check_filesystem`]). If any output is missing, or is
//!      older than a dependency's output, then the unit is dirty.
//!   2. The mtime of a Unit's source files is compared to the mtime of its
//!      dep-info file in the fingerprint directory (see [`find_stale_file`]).
//!      The dep-info file is used as an anchor to know when the last build of
//!      the unit was done. See the "dep-info files" section below for more
//!      details. If any input files are missing, or are newer than the
//!      dep-info, then the unit is dirty.
//!
//!  - Alternatively if you're using the unstable feature `checksum-freshness`
//!    mtimes are ignored entirely in favor of comparing first the file size, and
//!    then the checksum with a known prior value emitted by rustc. Only nightly
//!    rustc will emit the needed metadata at the time of writing. This is dependent
//!    on the unstable feature `-Z checksum-hash-algorithm`.
//!
//! Note: Fingerprinting is not a perfect solution. Filesystem mtime tracking
//! is notoriously imprecise and problematic. Only a small part of the
//! environment is captured. This is a balance of performance, simplicity, and
//! completeness. Sandboxing, hashing file contents, tracking every file
//! access, environment variable, and network operation would ensure more
//! reliable and reproducible builds at the cost of being complex, slow, and
//! platform-dependent.
//!
//! ## Fingerprints and [`UnitHash`]s
//!
//! [`Metadata`] tracks several [`UnitHash`]s, including
//! [`Metadata::unit_id`], [`Metadata::c_metadata`], and [`Metadata::c_extra_filename`].
//! See its documentation for more details.
//!
//! NOTE: Not all output files are isolated via filename hashes (like dylibs).
//! The fingerprint directory uses a hash, but sometimes units share the same
//! fingerprint directory (when they don't have Metadata) so care should be
//! taken to handle this!
//!
//! Fingerprints and [`UnitHash`]s are similar, and track some of the same things.
//! [`UnitHash`]s contains information that is required to keep Units separate.
//! The Fingerprint includes additional information that should cause a
//! recompile, but it is desired to reuse the same filenames. A comparison
//! of what is tracked:
//!
//! Value                                      | Fingerprint | `Metadata::unit_id` | `Metadata::c_metadata` | `Metadata::c_extra_filename`
//! -------------------------------------------|-------------|---------------------|------------------------|----------
//! rustc                                      | ✓           | ✓                   | ✓                      | ✓
//! [`Profile`]                                | ✓           | ✓                   | ✓                      | ✓
//! `cargo rustc` extra args                   | ✓           | ✓[^7]               |                        | ✓[^7]
//! [`CompileMode`]                            | ✓           | ✓                   | ✓                      | ✓
//! Target Name                                | ✓           | ✓                   | ✓                      | ✓
//! `TargetKind` (bin/lib/etc.)                | ✓           | ✓                   | ✓                      | ✓
//! Enabled Features                           | ✓           | ✓                   | ✓                      | ✓
//! Declared Features                          | ✓           |                     |                        |
//! Immediate dependency’s hashes              | ✓[^1]       | ✓                   | ✓                      | ✓
//! [`CompileKind`] (host/target)              | ✓           | ✓                   | ✓                      | ✓
//! `__CARGO_DEFAULT_LIB_METADATA`[^4]         |             | ✓                   | ✓                      | ✓
//! `package_id`                               |             | ✓                   | ✓                      | ✓
//! Target src path relative to ws             | ✓           |                     |                        |
//! Target flags (test/bench/for_host/edition) | ✓           |                     |                        |
//! -C incremental=… flag                      | ✓           |                     |                        |
//! mtime of sources                           | ✓[^3]       |                     |                        |
//! RUSTFLAGS/RUSTDOCFLAGS                     | ✓           | ✓[^7]               |                        | ✓[^7]
//! [`Lto`] flags                              | ✓           | ✓                   | ✓                      | ✓
//! config settings[^5]                        | ✓           |                     |                        |
//! `is_std`                                   |             | ✓                   | ✓                      | ✓
//! `[lints]` table[^6]                        | ✓           |                     |                        |
//! `[lints.rust.unexpected_cfgs.check-cfg]`   | ✓           |                     |                        |
//!
//! [^1]: Bin dependencies are not included.
//!
//! [^3]: See below for details on mtime tracking.
//!
//! [^4]: `__CARGO_DEFAULT_LIB_METADATA` is set by rustbuild to embed the
//!        release channel (bootstrap/stable/beta/nightly) in libstd.
//!
//! [^5]: Config settings that are not otherwise captured anywhere else.
//!       Currently, this is only `doc.extern-map`.
//!
//! [^6]: Via [`Manifest::lint_rustflags`][crate::core::Manifest::lint_rustflags]
//!
//! [^7]: extra-flags and RUSTFLAGS are conditionally excluded when `--remap-path-prefix` is
//!       present to avoid breaking build reproducibility while we wait for trim-paths
//!
//! When deciding what should go in the Metadata vs the Fingerprint, consider
//! that some files (like dylibs) do not have a hash in their filename. Thus,
//! if a value changes, only the fingerprint will detect the change (consider,
//! for example, swapping between different features). Fields that are only in
//! Metadata generally aren't relevant to the fingerprint because they
//! fundamentally change the output (like target vs host changes the directory
//! where it is emitted).
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
//! - A "dep-info" file which is a translation of rustc's `*.d` dep-info files
//!   to a Cargo-specific format that tweaks file names and is optimized for
//!   reading quickly.
//! - An `invoked.timestamp` file whose filesystem mtime is updated every time
//!   the Unit is built. This is used for capturing the time when the build
//!   starts, to detect if files are changed in the middle of the build. See
//!   below for more details.
//!
//! Note that some units are a little different. A Unit for *running* a build
//! script or for `rustdoc` does not have a dep-info file (it's not
//! applicable). Build script `invoked.timestamp` files are in the build
//! output directory.
//!
//! ## Fingerprint calculation
//!
//! After the list of Units has been calculated, the Units are added to the
//! [`JobQueue`]. As each one is added, the fingerprint is calculated, and the
//! dirty/fresh status is recorded. A closure is used to update the fingerprint
//! on-disk when the Unit successfully finishes. The closure will recompute the
//! Fingerprint based on the updated information. If the Unit fails to compile,
//! the fingerprint is not updated.
//!
//! Fingerprints are cached in the [`BuildRunner`]. This makes computing
//! Fingerprints faster, but also is necessary for properly updating
//! dependency information. Since a Fingerprint includes the Fingerprints of
//! all dependencies, when it is updated, by using `Arc` clones, it
//! automatically picks up the updates to its dependencies.
//!
//! ### dep-info files
//!
//! Cargo has several kinds of "dep info" files:
//!
//! * dep-info files generated by `rustc`.
//! * Fingerprint dep-info files translated from the first one.
//! * dep-info for external build system integration.
//! * Unstable `-Zbinary-dep-depinfo`.
//!
//! #### `rustc` dep-info files
//!
//! Cargo passes the `--emit=dep-info` flag to `rustc` so that `rustc` will
//! generate a "dep info" file (with the `.d` extension). This is a
//! Makefile-like syntax that includes all of the source files used to build
//! the crate. This file is used by Cargo to know which files to check to see
//! if the crate will need to be rebuilt. Example:
//!
//! ```makefile
//! /path/to/target/debug/deps/cargo-b6219d178925203d: src/bin/main.rs src/bin/cargo/cli.rs # … etc.
//! ```
//!
//! #### Fingerprint dep-info files
//!
//! After `rustc` exits successfully, Cargo will read the first kind of dep
//! info file and translate it into a binary format that is stored in the
//! fingerprint directory ([`translate_dep_info`]).
//!
//! These are used to quickly scan for any changed files. The mtime of the
//! fingerprint dep-info file itself is used as the reference for comparing the
//! source files to determine if any of the source files have been modified
//! (see [below](#mtime-comparison) for more detail).
//!
//! Note that Cargo parses the special `# env-var:...` comments in dep-info
//! files to learn about environment variables that the rustc compile depends on.
//! Cargo then later uses this to trigger a recompile if a referenced env var
//! changes (even if the source didn't change).
//! This also includes env vars generated from Cargo metadata like `CARGO_PKG_DESCRIPTION`.
//! (See [`crate::core::manifest::ManifestMetadata`]
//!
//! #### dep-info files for build system integration.
//!
//! There is also a third dep-info file. Cargo will extend the file created by
//! rustc with some additional information and saves this into the output
//! directory. This is intended for build system integration. See the
//! [`output_depinfo`] function for more detail.
//!
//! #### -Zbinary-dep-depinfo
//!
//! `rustc` has an experimental flag `-Zbinary-dep-depinfo`. This causes
//! `rustc` to include binary files (like rlibs) in the dep-info file. This is
//! primarily to support rustc development, so that Cargo can check the
//! implicit dependency to the standard library (which lives in the sysroot).
//! We want Cargo to recompile whenever the standard library rlib/dylibs
//! change, and this is a generic mechanism to make that work.
//!
//! ### Mtime comparison
//!
//! The use of modification timestamps is the most common way a unit will be
//! determined to be dirty or fresh between builds. There are many subtle
//! issues and edge cases with mtime comparisons. This gives a high-level
//! overview, but you'll need to read the code for the gritty details. Mtime
//! handling is different for different unit kinds. The different styles are
//! driven by the [`Fingerprint::local`] field, which is set based on the unit
//! kind.
//!
//! The status of whether or not the mtime is "stale" or "up-to-date" is
//! stored in [`Fingerprint::fs_status`].
//!
//! All units will compare the mtime of its newest output file with the mtimes
//! of the outputs of all its dependencies. If any output file is missing,
//! then the unit is stale. If any dependency is newer, the unit is stale.
//!
//! #### Normal package mtime handling
//!
//! [`LocalFingerprint::CheckDepInfo`] is used for checking the mtime of
//! packages. It compares the mtime of the input files (the source files) to
//! the mtime of the dep-info file (which is written last after a build is
//! finished). If the dep-info is missing, the unit is stale (it has never
//! been built). The list of input files comes from the dep-info file. See the
//! section above for details on dep-info files.
//!
//! Also note that although registry and git packages use [`CheckDepInfo`], none
//! of their source files are included in the dep-info (see
//! [`translate_dep_info`]), so for those kinds no mtime checking is done
//! (unless `-Zbinary-dep-depinfo` is used). Repository and git packages are
//! static, so there is no need to check anything.
//!
//! When a build is complete, the mtime of the dep-info file in the
//! fingerprint directory is modified to rewind it to the time when the build
//! started. This is done by creating an `invoked.timestamp` file when the
//! build starts to capture the start time. The mtime is rewound to the start
//! to handle the case where the user modifies a source file while a build is
//! running. Cargo can't know whether or not the file was included in the
//! build, so it takes a conservative approach of assuming the file was *not*
//! included, and it should be rebuilt during the next build.
//!
//! #### Rustdoc mtime handling
//!
//! Rustdoc does not emit a dep-info file, so Cargo currently has a relatively
//! simple system for detecting rebuilds. [`LocalFingerprint::Precalculated`] is
//! used for rustdoc units. For registry packages, this is the package
//! version. For git packages, it is the git hash. For path packages, it is
//! a string of the mtime of the newest file in the package.
//!
//! There are some known bugs with how this works, so it should be improved at
//! some point.
//!
//! #### Build script mtime handling
//!
//! Build script mtime handling runs in different modes. There is the "old
//! style" where the build script does not emit any `rerun-if` directives. In
//! this mode, Cargo will use [`LocalFingerprint::Precalculated`]. See the
//! "rustdoc" section above how it works.
//!
//! In the new-style, each `rerun-if` directive is translated to the
//! corresponding [`LocalFingerprint`] variant. The [`RerunIfChanged`] variant
//! compares the mtime of the given filenames against the mtime of the
//! "output" file.
//!
//! Similar to normal units, the build script "output" file mtime is rewound
//! to the time just before the build script is executed to handle mid-build
//! modifications.
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
//!   date, and the modification time doesn't make its way into the fingerprint
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
//! The *running* of a build script ([`CompileMode::RunCustomBuild`]) is treated
//! significantly different than all other Unit kinds. It has its own function
//! for calculating the Fingerprint ([`calculate_run_custom_build`]) and has some
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
//! the Unit for that build script is prepared for the [`JobQueue`]. The
//! Fingerprint code can then use that information to compute the Fingerprint
//! and compare against the old fingerprint hash.
//!
//! Care must be taken with build script Fingerprints because the
//! [`Fingerprint::local`] value may be changed after the build script runs
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
//! See the [`A-rebuild-detection`] label on the issue tracker for more.
//!
//! [`check_filesystem`]: Fingerprint::check_filesystem
//! [`Metadata`]: crate::core::compiler::Metadata
//! [`Metadata::unit_id`]: crate::core::compiler::Metadata::unit_id
//! [`Metadata::c_metadata`]: crate::core::compiler::Metadata::c_metadata
//! [`Metadata::c_extra_filename`]: crate::core::compiler::Metadata::c_extra_filename
//! [`UnitHash`]: crate::core::compiler::UnitHash
//! [`Profile`]: crate::core::profiles::Profile
//! [`CompileMode`]: crate::core::compiler::CompileMode
//! [`Lto`]: crate::core::compiler::Lto
//! [`CompileKind`]: crate::core::compiler::CompileKind
//! [`JobQueue`]: super::job_queue::JobQueue
//! [`output_depinfo`]: super::output_depinfo()
//! [`CheckDepInfo`]: LocalFingerprint::CheckDepInfo
//! [`RerunIfChanged`]: LocalFingerprint::RerunIfChanged
//! [`CompileMode::RunCustomBuild`]: crate::core::compiler::CompileMode::RunCustomBuild
//! [`A-rebuild-detection`]: https://github.com/rust-lang/cargo/issues?q=is%3Aissue+is%3Aopen+label%3AA-rebuild-detection

mod dep_info;
mod dirty_reason;

use std::collections::hash_map::{Entry, HashMap};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::hash::{self, Hash, Hasher};
use std::io::{self};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use anyhow::Context as _;
use anyhow::format_err;
use cargo_util::paths;
use filetime::FileTime;
use serde::de;
use serde::ser;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::core::Package;
use crate::core::compiler::unit_graph::UnitDep;
use crate::util;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::util::log_message::LogMessage;
use crate::util::{StableHasher, internal, path_args};
use crate::{CARGO_ENV, GlobalContext};

use super::custom_build::BuildDeps;
use super::{BuildContext, BuildRunner, FileFlavor, Job, Unit, Work};

pub use self::dep_info::Checksum;
pub use self::dep_info::parse_dep_info;
pub use self::dep_info::parse_rustc_dep_info;
pub use self::dep_info::translate_dep_info;
pub use self::dirty_reason::DirtyReason;

/// Determines if a [`Unit`] is up-to-date, and if not prepares necessary work to
/// update the persisted fingerprint.
///
/// This function will inspect `Unit`, calculate a fingerprint for it, and then
/// return an appropriate [`Job`] to run. The returned `Job` will be a noop if
/// `unit` is considered "fresh", or if it was previously built and cached.
/// Otherwise the `Job` returned will write out the true fingerprint to the
/// filesystem, to be executed after the unit's work has completed.
///
/// The `force` flag is a way to force the `Job` to be "dirty", or always
/// update the fingerprint. **Beware using this flag** because it does not
/// transitively propagate throughout the dependency graph, it only forces this
/// one unit which is very unlikely to be what you want unless you're
/// exclusively talking about top-level units.
#[tracing::instrument(
    skip(build_runner, unit),
    fields(package_id = %unit.pkg.package_id(), target = unit.target.name())
)]
pub fn prepare_target(
    build_runner: &mut BuildRunner<'_, '_>,
    unit: &Unit,
    force: bool,
) -> CargoResult<Job> {
    let bcx = build_runner.bcx;
    let loc = build_runner.files().fingerprint_file_path(unit, "");

    debug!("fingerprint at: {}", loc.display());

    // Figure out if this unit is up to date. After calculating the fingerprint
    // compare it to an old version, if any, and attempt to print diagnostic
    // information about failed comparisons to aid in debugging.
    let fingerprint = calculate(build_runner, unit)?;
    let mtime_on_use = build_runner.bcx.gctx.cli_unstable().mtime_on_use;
    let dirty_reason = compare_old_fingerprint(unit, &loc, &*fingerprint, mtime_on_use, force);

    let Some(dirty_reason) = dirty_reason else {
        return Ok(Job::new_fresh());
    };

    if let Some(logger) = bcx.logger {
        // Dont log FreshBuild as it is noisy.
        if !dirty_reason.is_fresh_build() {
            logger.log(LogMessage::Rebuild {
                package_id: unit.pkg.package_id().to_spec(),
                target: unit.target.clone(),
                mode: unit.mode,
                cause: dirty_reason.clone(),
            });
        }
    }

    // We're going to rebuild, so ensure the source of the crate passes all
    // verification checks before we build it.
    //
    // The `Source::verify` method is intended to allow sources to execute
    // pre-build checks to ensure that the relevant source code is all
    // up-to-date and as expected. This is currently used primarily for
    // directory sources which will use this hook to perform an integrity check
    // on all files in the source to ensure they haven't changed. If they have
    // changed then an error is issued.
    let source_id = unit.pkg.package_id().source_id();
    let sources = bcx.packages.sources();
    let source = sources
        .get(source_id)
        .ok_or_else(|| internal("missing package source"))?;
    source.verify(unit.pkg.package_id())?;

    // Clear out the old fingerprint file if it exists. This protects when
    // compilation is interrupted leaving a corrupt file. For example, a
    // project with a lib.rs and integration test (two units):
    //
    // 1. Build the library and integration test.
    // 2. Make a change to lib.rs (NOT the integration test).
    // 3. Build the integration test, hit Ctrl-C while linking. With gcc, this
    //    will leave behind an incomplete executable (zero size, or partially
    //    written). NOTE: The library builds successfully, it is the linking
    //    of the integration test that we are interrupting.
    // 4. Build the integration test again.
    //
    // Without the following line, then step 3 will leave a valid fingerprint
    // on the disk. Then step 4 will think the integration test is "fresh"
    // because:
    //
    // - There is a valid fingerprint hash on disk (written in step 1).
    // - The mtime of the output file (the corrupt integration executable
    //   written in step 3) is newer than all of its dependencies.
    // - The mtime of the integration test fingerprint dep-info file (written
    //   in step 1) is newer than the integration test's source files, because
    //   we haven't modified any of its source files.
    //
    // But the executable is corrupt and needs to be rebuilt. Clearing the
    // fingerprint at step 3 ensures that Cargo never mistakes a partially
    // written output as up-to-date.
    if loc.exists() {
        // Truncate instead of delete so that compare_old_fingerprint will
        // still log the reason for the fingerprint failure instead of just
        // reporting "failed to read fingerprint" during the next build if
        // this build fails.
        paths::write(&loc, b"")?;
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
        let build_script_outputs = Arc::clone(&build_runner.build_script_outputs);
        let metadata = build_runner.get_run_build_script_metadata(unit);
        let (gen_local, _overridden) = build_script_local_fingerprints(build_runner, unit)?;
        let output_path = build_runner.build_explicit_deps[unit]
            .build_script_output
            .clone();
        Work::new(move |_| {
            let outputs = build_script_outputs.lock().unwrap();
            let output = outputs
                .get(metadata)
                .expect("output must exist after running");
            let deps = BuildDeps::new(&output_path, Some(output));

            // FIXME: it's basically buggy that we pass `None` to `call_box`
            // here. See documentation on `build_script_local_fingerprints`
            // below for more information. Despite this just try to proceed and
            // hobble along if it happens to return `Some`.
            if let Some(new_local) = (gen_local)(&deps, None)? {
                *fingerprint.local.lock().unwrap() = new_local;
            }

            write_fingerprint(&loc, &fingerprint)
        })
    } else {
        Work::new(move |_| write_fingerprint(&loc, &fingerprint))
    };

    Ok(Job::new_dirty(write_fingerprint, dirty_reason))
}

/// Dependency edge information for fingerprints. This is generated for each
/// dependency and is stored in a [`Fingerprint`].
#[derive(Clone)]
struct DepFingerprint {
    /// The hash of the package id that this dependency points to
    pkg_id: u64,
    /// The crate name we're using for this dependency, which if we change we'll
    /// need to recompile!
    name: InternedString,
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
/// to-be. The actual value can be calculated via [`hash_u64()`], but the operation
/// may fail as some files may not have been generated.
///
/// Note that dependencies are taken into account for fingerprints because rustc
/// requires that whenever an upstream crate is recompiled that all downstream
/// dependents are also recompiled. This is typically tracked through
/// [`DependencyQueue`], but it also needs to be retained here because Cargo can
/// be interrupted while executing, losing the state of the [`DependencyQueue`]
/// graph.
///
/// [`hash_u64()`]: crate::core::compiler::fingerprint::Fingerprint::hash_u64
/// [`DependencyQueue`]: crate::util::DependencyQueue
#[derive(Serialize, Deserialize)]
pub struct Fingerprint {
    /// Hash of the version of `rustc` used.
    rustc: u64,
    /// Sorted list of cfg features enabled.
    features: String,
    /// Sorted list of all the declared cfg features.
    declared_features: String,
    /// Hash of the `Target` struct, including the target name,
    /// package-relative source path, edition, etc.
    target: u64,
    /// Hash of the [`Profile`], [`CompileMode`], and any extra flags passed via
    /// `cargo rustc` or `cargo rustdoc`.
    ///
    /// [`Profile`]: crate::core::profiles::Profile
    /// [`CompileMode`]: crate::core::compiler::CompileMode
    profile: u64,
    /// Hash of the path to the base source file. This is relative to the
    /// workspace root for path members, or absolute for other sources.
    path: u64,
    /// Fingerprints of dependencies.
    deps: Vec<DepFingerprint>,
    /// Information about the inputs that affect this Unit (such as source
    /// file mtimes or build script environment variables).
    local: Mutex<Vec<LocalFingerprint>>,
    /// Cached hash of the [`Fingerprint`] struct. Used to improve performance
    /// for hashing.
    #[serde(skip)]
    memoized_hash: Mutex<Option<u64>>,
    /// RUSTFLAGS/RUSTDOCFLAGS environment variable value (or config value).
    rustflags: Vec<String>,
    /// Hash of various config settings that change how things are compiled.
    config: u64,
    /// The rustc target. This is only relevant for `.json` files, otherwise
    /// the metadata hash segregates the units.
    compile_kind: u64,
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
#[derive(Clone, Default, Debug, Serialize)]
#[serde(tag = "fs_status", rename_all = "kebab-case")]
pub enum FsStatus {
    /// This unit is to be considered stale, even if hash information all
    /// matches.
    #[default]
    Stale,

    /// File system inputs have changed (or are missing), or there were
    /// changes to the environment variables that affect this unit. See
    /// the variants of [`StaleItem`] for more information.
    StaleItem(StaleItem),

    /// A dependency was stale.
    StaleDependency {
        name: InternedString,
        #[serde(serialize_with = "serialize_file_time")]
        dep_mtime: FileTime,
        #[serde(serialize_with = "serialize_file_time")]
        max_mtime: FileTime,
    },

    /// A dependency was stale.
    StaleDepFingerprint { name: InternedString },

    /// This unit is up-to-date. All outputs and their corresponding mtime are
    /// listed in the payload here for other dependencies to compare against.
    #[serde(skip)]
    UpToDate { mtimes: HashMap<PathBuf, FileTime> },
}

impl FsStatus {
    fn up_to_date(&self) -> bool {
        match self {
            FsStatus::UpToDate { .. } => true,
            FsStatus::Stale
            | FsStatus::StaleItem(_)
            | FsStatus::StaleDependency { .. }
            | FsStatus::StaleDepFingerprint { .. } => false,
        }
    }
}

/// Serialize FileTime as milliseconds with nano.
fn serialize_file_time<S>(ft: &FileTime, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let secs_as_millis = ft.unix_seconds() as f64 * 1000.0;
    let nanos_as_millis = ft.nanoseconds() as f64 / 1_000_000.0;
    (secs_as_millis + nanos_as_millis).serialize(s)
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
            &self.fingerprint.hash_u64(),
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
            name: name.into(),
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
    /// hash as usual. This variant is primarily used for rustdoc where we
    /// don't have a dep-info file to compare against.
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
    ///
    /// If the `checksum` bool is true then the `dep_info` file is expected to
    /// contain file checksums instead of file mtimes.
    CheckDepInfo { dep_info: PathBuf, checksum: bool },

    /// This represents a nonempty set of `rerun-if-changed` annotations printed
    /// out by a build script. The `output` file is a relative file anchored at
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

/// See [`FsStatus::StaleItem`].
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "stale_item", rename_all = "kebab-case")]
pub enum StaleItem {
    MissingFile {
        path: PathBuf,
    },
    UnableToReadFile {
        path: PathBuf,
    },
    FailedToReadMetadata {
        path: PathBuf,
    },
    FileSizeChanged {
        path: PathBuf,
        old_size: u64,
        new_size: u64,
    },
    ChangedFile {
        reference: PathBuf,
        #[serde(serialize_with = "serialize_file_time")]
        reference_mtime: FileTime,
        stale: PathBuf,
        #[serde(serialize_with = "serialize_file_time")]
        stale_mtime: FileTime,
    },
    ChangedChecksum {
        source: PathBuf,
        stored_checksum: Checksum,
        new_checksum: Checksum,
    },
    MissingChecksum {
        path: PathBuf,
    },
    ChangedEnv {
        var: String,
        previous: Option<String>,
        current: Option<String>,
    },
}

impl LocalFingerprint {
    /// Read the environment variable of the given env `key`, and creates a new
    /// [`LocalFingerprint::RerunIfEnvChanged`] for it. The `env_config` is used firstly
    /// to check if the env var is set in the config system as some envs need to be overridden.
    /// If not, it will fallback to `std::env::var`.
    ///
    // TODO: `std::env::var` is allowed at this moment. Should figure out
    // if it makes sense if permitting to read env from the env snapshot.
    #[allow(clippy::disallowed_methods)]
    fn from_env<K: AsRef<str>>(
        key: K,
        env_config: &Arc<HashMap<String, OsString>>,
    ) -> LocalFingerprint {
        let key = key.as_ref();
        let var = key.to_owned();
        let val = if let Some(val) = env_config.get(key) {
            val.to_str().map(ToOwned::to_owned)
        } else {
            env::var(key).ok()
        };
        LocalFingerprint::RerunIfEnvChanged { var, val }
    }

    /// Checks dynamically at runtime if this `LocalFingerprint` has a stale
    /// item inside of it.
    ///
    /// The main purpose of this function is to handle two different ways
    /// fingerprints can be invalidated:
    ///
    /// * One is a dependency listed in rustc's dep-info files is invalid. Note
    ///   that these could either be env vars or files. We check both here.
    ///
    /// * Another is the `rerun-if-changed` directive from build scripts. This
    ///   is where we'll find whether files have actually changed
    fn find_stale_item(
        &self,
        mtime_cache: &mut HashMap<PathBuf, FileTime>,
        checksum_cache: &mut HashMap<PathBuf, Checksum>,
        pkg: &Package,
        build_root: &Path,
        cargo_exe: &Path,
        gctx: &GlobalContext,
    ) -> CargoResult<Option<StaleItem>> {
        let pkg_root = pkg.root();
        match self {
            // We need to parse `dep_info`, learn about the crate's dependencies.
            //
            // For each env var we see if our current process's env var still
            // matches, and for each file we see if any of them are newer than
            // the `dep_info` file itself whose mtime represents the start of
            // rustc.
            LocalFingerprint::CheckDepInfo { dep_info, checksum } => {
                let dep_info = build_root.join(dep_info);
                let Some(info) = parse_dep_info(pkg_root, build_root, &dep_info)? else {
                    return Ok(Some(StaleItem::MissingFile { path: dep_info }));
                };
                for (key, previous) in info.env.iter() {
                    if let Some(value) = pkg.manifest().metadata().env_var(key.as_str()) {
                        if Some(value.as_ref()) == previous.as_deref() {
                            continue;
                        }
                    }

                    let current = if key == CARGO_ENV {
                        Some(cargo_exe.to_str().ok_or_else(|| {
                            format_err!(
                                "cargo exe path {} must be valid UTF-8",
                                cargo_exe.display()
                            )
                        })?)
                    } else {
                        if let Some(value) = gctx.env_config()?.get(key) {
                            value.to_str()
                        } else {
                            gctx.get_env(key).ok()
                        }
                    };
                    if current == previous.as_deref() {
                        continue;
                    }
                    return Ok(Some(StaleItem::ChangedEnv {
                        var: key.clone(),
                        previous: previous.clone(),
                        current: current.map(Into::into),
                    }));
                }
                if *checksum {
                    Ok(find_stale_file(
                        mtime_cache,
                        checksum_cache,
                        &dep_info,
                        info.files.iter().map(|(file, checksum)| (file, *checksum)),
                        *checksum,
                    ))
                } else {
                    Ok(find_stale_file(
                        mtime_cache,
                        checksum_cache,
                        &dep_info,
                        info.files.into_keys().map(|p| (p, None)),
                        *checksum,
                    ))
                }
            }

            // We need to verify that no paths listed in `paths` are newer than
            // the `output` path itself, or the last time the build script ran.
            LocalFingerprint::RerunIfChanged { output, paths } => Ok(find_stale_file(
                mtime_cache,
                checksum_cache,
                &build_root.join(output),
                paths.iter().map(|p| (pkg_root.join(p), None)),
                false,
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

impl Fingerprint {
    fn new() -> Fingerprint {
        Fingerprint {
            rustc: 0,
            target: 0,
            profile: 0,
            path: 0,
            features: String::new(),
            declared_features: String::new(),
            deps: Vec::new(),
            local: Mutex::new(Vec::new()),
            memoized_hash: Mutex::new(None),
            rustflags: Vec::new(),
            config: 0,
            compile_kind: 0,
            fs_status: FsStatus::Stale,
            outputs: Vec::new(),
        }
    }

    /// For performance reasons fingerprints will memoize their own hash, but
    /// there's also internal mutability with its `local` field which can
    /// change, for example with build scripts, during a build.
    ///
    /// This method can be used to bust all memoized hashes just before a build
    /// to ensure that after a build completes everything is up-to-date.
    pub fn clear_memoized(&self) {
        *self.memoized_hash.lock().unwrap() = None;
    }

    fn hash_u64(&self) -> u64 {
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
    /// [`DirtyReason`], indicating why we're recompiling something.
    fn compare(&self, old: &Fingerprint) -> DirtyReason {
        if self.rustc != old.rustc {
            return DirtyReason::RustcChanged;
        }
        if self.features != old.features {
            return DirtyReason::FeaturesChanged {
                old: old.features.clone(),
                new: self.features.clone(),
            };
        }
        if self.declared_features != old.declared_features {
            return DirtyReason::DeclaredFeaturesChanged {
                old: old.declared_features.clone(),
                new: self.declared_features.clone(),
            };
        }
        if self.target != old.target {
            return DirtyReason::TargetConfigurationChanged;
        }
        if self.path != old.path {
            return DirtyReason::PathToSourceChanged;
        }
        if self.profile != old.profile {
            return DirtyReason::ProfileConfigurationChanged;
        }
        if self.rustflags != old.rustflags {
            return DirtyReason::RustflagsChanged {
                old: old.rustflags.clone(),
                new: self.rustflags.clone(),
            };
        }
        if self.config != old.config {
            return DirtyReason::ConfigSettingsChanged;
        }
        if self.compile_kind != old.compile_kind {
            return DirtyReason::CompileKindChanged;
        }
        let my_local = self.local.lock().unwrap();
        let old_local = old.local.lock().unwrap();
        if my_local.len() != old_local.len() {
            return DirtyReason::LocalLengthsChanged;
        }
        for (new, old) in my_local.iter().zip(old_local.iter()) {
            match (new, old) {
                (LocalFingerprint::Precalculated(a), LocalFingerprint::Precalculated(b)) => {
                    if a != b {
                        return DirtyReason::PrecalculatedComponentsChanged {
                            old: b.to_string(),
                            new: a.to_string(),
                        };
                    }
                }
                (
                    LocalFingerprint::CheckDepInfo {
                        dep_info: adep,
                        checksum: checksum_a,
                    },
                    LocalFingerprint::CheckDepInfo {
                        dep_info: bdep,
                        checksum: checksum_b,
                    },
                ) => {
                    if adep != bdep {
                        return DirtyReason::DepInfoOutputChanged {
                            old: bdep.clone(),
                            new: adep.clone(),
                        };
                    }
                    if checksum_a != checksum_b {
                        return DirtyReason::ChecksumUseChanged { old: *checksum_b };
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
                        return DirtyReason::RerunIfChangedOutputFileChanged {
                            old: bout.clone(),
                            new: aout.clone(),
                        };
                    }
                    if apaths != bpaths {
                        return DirtyReason::RerunIfChangedOutputPathsChanged {
                            old: bpaths.clone(),
                            new: apaths.clone(),
                        };
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
                        return DirtyReason::EnvVarsChanged {
                            old: bkey.clone(),
                            new: akey.clone(),
                        };
                    }
                    if *avalue != *bvalue {
                        return DirtyReason::EnvVarChanged {
                            name: akey.clone(),
                            old_value: bvalue.clone(),
                            new_value: avalue.clone(),
                        };
                    }
                }
                (a, b) => {
                    return DirtyReason::LocalFingerprintTypeChanged {
                        old: b.kind(),
                        new: a.kind(),
                    };
                }
            }
        }

        if self.deps.len() != old.deps.len() {
            return DirtyReason::NumberOfDependenciesChanged {
                old: old.deps.len(),
                new: self.deps.len(),
            };
        }
        for (a, b) in self.deps.iter().zip(old.deps.iter()) {
            if a.name != b.name {
                return DirtyReason::UnitDependencyNameChanged {
                    old: b.name,
                    new: a.name,
                };
            }

            if a.fingerprint.hash_u64() != b.fingerprint.hash_u64() {
                return DirtyReason::UnitDependencyInfoChanged {
                    new_name: a.name,
                    new_fingerprint: a.fingerprint.hash_u64(),
                    old_name: b.name,
                    old_fingerprint: b.fingerprint.hash_u64(),
                };
            }
        }

        if !self.fs_status.up_to_date() {
            return DirtyReason::FsStatusOutdated(self.fs_status.clone());
        }

        // This typically means some filesystem modifications happened or
        // something transitive was odd. In general we should strive to provide
        // a better error message than this, so if you see this message a lot it
        // likely means this method needs to be updated!
        DirtyReason::NothingObvious
    }

    /// Dynamically inspect the local filesystem to update the `fs_status` field
    /// of this `Fingerprint`.
    ///
    /// This function is used just after a `Fingerprint` is constructed to check
    /// the local state of the filesystem and propagate any dirtiness from
    /// dependencies up to this unit as well. This function assumes that the
    /// unit starts out as [`FsStatus::Stale`] and then it will optionally switch
    /// it to `UpToDate` if it can.
    fn check_filesystem(
        &mut self,
        mtime_cache: &mut HashMap<PathBuf, FileTime>,
        checksum_cache: &mut HashMap<PathBuf, Checksum>,
        pkg: &Package,
        build_root: &Path,
        cargo_exe: &Path,
        gctx: &GlobalContext,
    ) -> CargoResult<()> {
        assert!(!self.fs_status.up_to_date());

        let pkg_root = pkg.root();
        let mut mtimes = HashMap::new();

        // Get the `mtime` of all outputs. Optionally update their mtime
        // afterwards based on the `mtime_on_use` flag. Afterwards we want the
        // minimum mtime as it's the one we'll be comparing to inputs and
        // dependencies.
        for output in self.outputs.iter() {
            let Ok(mtime) = paths::mtime(output) else {
                // This path failed to report its `mtime`. It probably doesn't
                // exists, so leave ourselves as stale and bail out.
                let item = StaleItem::FailedToReadMetadata {
                    path: output.clone(),
                };
                self.fs_status = FsStatus::StaleItem(item);
                return Ok(());
            };
            assert!(mtimes.insert(output.clone(), mtime).is_none());
        }

        let opt_max = mtimes.iter().max_by_key(|kv| kv.1);
        let Some((max_path, max_mtime)) = opt_max else {
            // We had no output files. This means we're an overridden build
            // script and we're just always up to date because we aren't
            // watching the filesystem.
            self.fs_status = FsStatus::UpToDate { mtimes };
            return Ok(());
        };
        debug!(
            "max output mtime for {:?} is {:?} {}",
            pkg_root, max_path, max_mtime
        );

        for dep in self.deps.iter() {
            let dep_mtimes = match &dep.fingerprint.fs_status {
                FsStatus::UpToDate { mtimes } => mtimes,
                // If our dependency is stale, so are we, so bail out.
                FsStatus::Stale
                | FsStatus::StaleItem(_)
                | FsStatus::StaleDependency { .. }
                | FsStatus::StaleDepFingerprint { .. } => {
                    self.fs_status = FsStatus::StaleDepFingerprint { name: dep.name };
                    return Ok(());
                }
            };

            // If our dependency edge only requires the rmeta file to be present
            // then we only need to look at that one output file, otherwise we
            // need to consider all output files to see if we're out of date.
            let (dep_path, dep_mtime) = if dep.only_requires_rmeta {
                dep_mtimes
                    .iter()
                    .find(|(path, _mtime)| {
                        path.extension().and_then(|s| s.to_str()) == Some("rmeta")
                    })
                    .expect("failed to find rmeta")
            } else {
                match dep_mtimes.iter().max_by_key(|kv| kv.1) {
                    Some(dep_mtime) => dep_mtime,
                    // If our dependencies is up to date and has no filesystem
                    // interactions, then we can move on to the next dependency.
                    None => continue,
                }
            };
            debug!(
                "max dep mtime for {:?} is {:?} {}",
                pkg_root, dep_path, dep_mtime
            );

            // If the dependency is newer than our own output then it was
            // recompiled previously. We transitively become stale ourselves in
            // that case, so bail out.
            //
            // Note that this comparison should probably be `>=`, not `>`, but
            // for a discussion of why it's `>` see the discussion about #5918
            // below in `find_stale`.
            if dep_mtime > max_mtime {
                info!(
                    "dependency on `{}` is newer than we are {} > {} {:?}",
                    dep.name, dep_mtime, max_mtime, pkg_root
                );

                self.fs_status = FsStatus::StaleDependency {
                    name: dep.name,
                    dep_mtime: *dep_mtime,
                    max_mtime: *max_mtime,
                };

                return Ok(());
            }
        }

        // If we reached this far then all dependencies are up to date. Check
        // all our `LocalFingerprint` information to see if we have any stale
        // files for this package itself. If we do find something log a helpful
        // message and bail out so we stay stale.
        for local in self.local.get_mut().unwrap().iter() {
            if let Some(item) = local.find_stale_item(
                mtime_cache,
                checksum_cache,
                pkg,
                build_root,
                cargo_exe,
                gctx,
            )? {
                item.log();
                self.fs_status = FsStatus::StaleItem(item);
                return Ok(());
            }
        }

        // Everything was up to date! Record such.
        self.fs_status = FsStatus::UpToDate { mtimes };
        debug!("filesystem up-to-date {:?}", pkg_root);

        Ok(())
    }
}

impl hash::Hash for Fingerprint {
    fn hash<H: Hasher>(&self, h: &mut H) {
        let Fingerprint {
            rustc,
            ref features,
            ref declared_features,
            target,
            path,
            profile,
            ref deps,
            ref local,
            config,
            compile_kind,
            ref rustflags,
            ..
        } = *self;
        let local = local.lock().unwrap();
        (
            rustc,
            features,
            declared_features,
            target,
            path,
            profile,
            &*local,
            config,
            compile_kind,
            rustflags,
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
            h.write_u64(fingerprint.hash_u64());
        }
    }
}

impl DepFingerprint {
    fn new(
        build_runner: &mut BuildRunner<'_, '_>,
        parent: &Unit,
        dep: &UnitDep,
    ) -> CargoResult<DepFingerprint> {
        let fingerprint = calculate(build_runner, &dep.unit)?;
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
        let pkg_id = if dep.unit.pkg.package_id().source_id().is_path() {
            util::hash_u64(dep.unit.pkg.package_id().name())
        } else {
            util::hash_u64(dep.unit.pkg.package_id())
        };

        Ok(DepFingerprint {
            pkg_id,
            name: dep.extern_crate_name,
            public: dep.public,
            fingerprint,
            only_requires_rmeta: build_runner.only_requires_rmeta(parent, &dep.unit),
        })
    }
}

impl StaleItem {
    /// Use the `log` crate to log a hopefully helpful message in diagnosing
    /// what file is considered stale and why. This is intended to be used in
    /// conjunction with `CARGO_LOG` to determine why Cargo is recompiling
    /// something. Currently there's no user-facing usage of this other than
    /// that.
    fn log(&self) {
        match self {
            StaleItem::MissingFile { path } => {
                info!("stale: missing {:?}", path);
            }
            StaleItem::UnableToReadFile { path } => {
                info!("stale: unable to read {:?}", path);
            }
            StaleItem::FailedToReadMetadata { path } => {
                info!("stale: couldn't read metadata {:?}", path);
            }
            StaleItem::ChangedFile {
                reference,
                reference_mtime,
                stale,
                stale_mtime,
            } => {
                info!("stale: changed {:?}", stale);
                info!("          (vs) {:?}", reference);
                info!("               {:?} < {:?}", reference_mtime, stale_mtime);
            }
            StaleItem::FileSizeChanged {
                path,
                new_size,
                old_size,
            } => {
                info!("stale: changed {:?}", path);
                info!("prior file size {old_size}");
                info!("  new file size {new_size}");
            }
            StaleItem::ChangedChecksum {
                source,
                stored_checksum,
                new_checksum,
            } => {
                info!("stale: changed {:?}", source);
                info!("prior checksum {stored_checksum}");
                info!("  new checksum {new_checksum}");
            }
            StaleItem::MissingChecksum { path } => {
                info!("stale: no prior checksum {:?}", path);
            }
            StaleItem::ChangedEnv {
                var,
                previous,
                current,
            } => {
                info!("stale: changed env {:?}", var);
                info!("       {:?} != {:?}", previous, current);
            }
        }
    }
}

/// Calculates the fingerprint for a [`Unit`].
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
fn calculate(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<Arc<Fingerprint>> {
    // This function is slammed quite a lot, so the result is memoized.
    if let Some(s) = build_runner.fingerprints.get(unit) {
        return Ok(Arc::clone(s));
    }
    let mut fingerprint = if unit.mode.is_run_custom_build() {
        calculate_run_custom_build(build_runner, unit)?
    } else if unit.mode.is_doc_test() {
        panic!("doc tests do not fingerprint");
    } else {
        calculate_normal(build_runner, unit)?
    };

    // After we built the initial `Fingerprint` be sure to update the
    // `fs_status` field of it.
    let build_root = build_root(build_runner);
    let cargo_exe = build_runner.bcx.gctx.cargo_exe()?;
    fingerprint.check_filesystem(
        &mut build_runner.mtime_cache,
        &mut build_runner.checksum_cache,
        &unit.pkg,
        &build_root,
        cargo_exe,
        build_runner.bcx.gctx,
    )?;

    let fingerprint = Arc::new(fingerprint);
    build_runner
        .fingerprints
        .insert(unit.clone(), Arc::clone(&fingerprint));
    Ok(fingerprint)
}

/// Calculate a fingerprint for a "normal" unit, or anything that's not a build
/// script. This is an internal helper of [`calculate`], don't call directly.
fn calculate_normal(
    build_runner: &mut BuildRunner<'_, '_>,
    unit: &Unit,
) -> CargoResult<Fingerprint> {
    let deps = {
        // Recursively calculate the fingerprint for all of our dependencies.
        //
        // Skip fingerprints of binaries because they don't actually induce a
        // recompile, they're just dependencies in the sense that they need to be
        // built. The only exception here are artifact dependencies,
        // which is an actual dependency that needs a recompile.
        //
        // Create Vec since mutable build_runner is needed in closure.
        let deps = Vec::from(build_runner.unit_deps(unit));
        let mut deps = deps
            .into_iter()
            .filter(|dep| !dep.unit.target.is_bin() || dep.unit.artifact.is_true())
            .map(|dep| DepFingerprint::new(build_runner, unit, &dep))
            .collect::<CargoResult<Vec<_>>>()?;
        deps.sort_by(|a, b| a.pkg_id.cmp(&b.pkg_id));
        deps
    };

    // Afterwards calculate our own fingerprint information.
    let build_root = build_root(build_runner);
    let is_any_doc_gen = unit.mode.is_doc() || unit.mode.is_doc_scrape();
    let rustdoc_depinfo_enabled = build_runner.bcx.gctx.cli_unstable().rustdoc_depinfo;
    let local = if is_any_doc_gen && !rustdoc_depinfo_enabled {
        // rustdoc does not have dep-info files.
        let fingerprint = pkg_fingerprint(build_runner.bcx, &unit.pkg).with_context(|| {
            format!(
                "failed to determine package fingerprint for documenting {}",
                unit.pkg
            )
        })?;
        vec![LocalFingerprint::Precalculated(fingerprint)]
    } else {
        let dep_info = dep_info_loc(build_runner, unit);
        let dep_info = dep_info.strip_prefix(&build_root).unwrap().to_path_buf();
        vec![LocalFingerprint::CheckDepInfo {
            dep_info,
            checksum: build_runner.bcx.gctx.cli_unstable().checksum_freshness,
        }]
    };

    // Figure out what the outputs of our unit is, and we'll be storing them
    // into the fingerprint as well.
    let outputs = build_runner
        .outputs(unit)?
        .iter()
        .filter(|output| {
            !matches!(
                output.flavor,
                FileFlavor::DebugInfo | FileFlavor::Auxiliary | FileFlavor::Sbom
            )
        })
        .map(|output| output.path.clone())
        .collect();

    // Fill out a bunch more information that we'll be tracking typically
    // hashed to take up less space on disk as we just need to know when things
    // change.
    let extra_flags = if unit.mode.is_doc() || unit.mode.is_doc_scrape() {
        &unit.rustdocflags
    } else {
        &unit.rustflags
    }
    .to_vec();

    let profile_hash = util::hash_u64((
        &unit.profile,
        unit.mode,
        build_runner.bcx.extra_args_for(unit),
        build_runner.lto[unit],
        unit.pkg.manifest().lint_rustflags(),
    ));
    let mut config = StableHasher::new();
    if let Some(linker) = build_runner.compilation.target_linker(unit.kind) {
        linker.hash(&mut config);
    }
    if unit.mode.is_doc() && build_runner.bcx.gctx.cli_unstable().rustdoc_map {
        if let Ok(map) = build_runner.bcx.gctx.doc_extern_map() {
            map.hash(&mut config);
        }
    }
    if let Some(allow_features) = &build_runner.bcx.gctx.cli_unstable().allow_features {
        allow_features.hash(&mut config);
    }
    let compile_kind = unit.kind.fingerprint_hash();
    let mut declared_features = unit.pkg.summary().features().keys().collect::<Vec<_>>();
    declared_features.sort(); // to avoid useless rebuild if the user orders it's features
    // differently
    Ok(Fingerprint {
        rustc: util::hash_u64(&build_runner.bcx.rustc().verbose_version),
        target: util::hash_u64(&unit.target),
        profile: profile_hash,
        // Note that .0 is hashed here, not .1 which is the cwd. That doesn't
        // actually affect the output artifact so there's no need to hash it.
        path: util::hash_u64(path_args(build_runner.bcx.ws, unit).0),
        features: format!("{:?}", unit.features),
        declared_features: format!("{declared_features:?}"),
        deps,
        local: Mutex::new(local),
        memoized_hash: Mutex::new(None),
        config: Hasher::finish(&config),
        compile_kind,
        rustflags: extra_flags,
        fs_status: FsStatus::Stale,
        outputs,
    })
}

/// Calculate a fingerprint for an "execute a build script" unit.  This is an
/// internal helper of [`calculate`], don't call directly.
fn calculate_run_custom_build(
    build_runner: &mut BuildRunner<'_, '_>,
    unit: &Unit,
) -> CargoResult<Fingerprint> {
    assert!(unit.mode.is_run_custom_build());
    // Using the `BuildDeps` information we'll have previously parsed and
    // inserted into `build_explicit_deps` built an initial snapshot of the
    // `LocalFingerprint` list for this build script. If we previously executed
    // the build script this means we'll be watching files and env vars.
    // Otherwise if we haven't previously executed it we'll just start watching
    // the whole crate.
    let (gen_local, overridden) = build_script_local_fingerprints(build_runner, unit)?;
    let deps = &build_runner.build_explicit_deps[unit];
    let local = (gen_local)(
        deps,
        Some(&|| {
            const IO_ERR_MESSAGE: &str = "\
An I/O error happened. Please make sure you can access the file.

By default, if your project contains a build script, cargo scans all files in
it to determine whether a rebuild is needed. If you don't expect to access the
file, specify `rerun-if-changed` in your build script.
See https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed for more information.";
            pkg_fingerprint(build_runner.bcx, &unit.pkg).map_err(|err| {
                let mut message = format!("failed to determine package fingerprint for build script for {}", unit.pkg);
                if err.root_cause().is::<io::Error>() {
                    message = format!("{}\n{}", message, IO_ERR_MESSAGE)
                }
                err.context(message)
            })
        }),
    )?
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
        // Create Vec since mutable build_runner is needed in closure.
        let deps = Vec::from(build_runner.unit_deps(unit));
        deps.into_iter()
            .map(|dep| DepFingerprint::new(build_runner, unit, &dep))
            .collect::<CargoResult<Vec<_>>>()?
    };

    let rustflags = unit.rustflags.to_vec();

    Ok(Fingerprint {
        local: Mutex::new(local),
        rustc: util::hash_u64(&build_runner.bcx.rustc().verbose_version),
        deps,
        outputs: if overridden { Vec::new() } else { vec![output] },
        rustflags,

        // Most of the other info is blank here as we don't really include it
        // in the execution of the build script, but... this may be a latent
        // bug in Cargo.
        ..Fingerprint::new()
    })
}

/// Get ready to compute the [`LocalFingerprint`] values
/// for a [`RunCustomBuild`] unit.
///
/// This function has, what's on the surface, a seriously wonky interface.
/// You'll call this function and it'll return a closure and a boolean. The
/// boolean is pretty simple in that it indicates whether the `unit` has been
/// overridden via `.cargo/config.toml`. The closure is much more complicated.
///
/// This closure is intended to capture any local state necessary to compute
/// the `LocalFingerprint` values for this unit. It is `Send` and `'static` to
/// be sent to other threads as well (such as when we're executing build
/// scripts). That deduplication is the rationale for the closure at least.
///
/// The arguments to the closure are a bit weirder, though, and I'll apologize
/// in advance for the weirdness too. The first argument to the closure is a
/// `&BuildDeps`. This is the parsed version of a build script, and when Cargo
/// starts up this is cached from previous runs of a build script.  After a
/// build script executes the output file is reparsed and passed in here.
///
/// The second argument is the weirdest, it's *optionally* a closure to
/// call [`pkg_fingerprint`]. The `pkg_fingerprint` requires access to
/// "source map" located in `Context`. That's very non-`'static` and
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
///
/// [`RunCustomBuild`]: crate::core::compiler::CompileMode::RunCustomBuild
fn build_script_local_fingerprints(
    build_runner: &mut BuildRunner<'_, '_>,
    unit: &Unit,
) -> CargoResult<(
    Box<
        dyn FnOnce(
                &BuildDeps,
                Option<&dyn Fn() -> CargoResult<String>>,
            ) -> CargoResult<Option<Vec<LocalFingerprint>>>
            + Send,
    >,
    bool,
)> {
    assert!(unit.mode.is_run_custom_build());
    // First up, if this build script is entirely overridden, then we just
    // return the hash of what we overrode it with. This is the easy case!
    if let Some(fingerprint) = build_script_override_fingerprint(build_runner, unit) {
        debug!("override local fingerprints deps {}", unit.pkg);
        return Ok((
            Box::new(
                move |_: &BuildDeps, _: Option<&dyn Fn() -> CargoResult<String>>| {
                    Ok(Some(vec![fingerprint]))
                },
            ),
            true, // this is an overridden build script
        ));
    }

    // ... Otherwise this is a "real" build script and we need to return a real
    // closure. Our returned closure classifies the build script based on
    // whether it prints `rerun-if-*`. If it *doesn't* print this it's where the
    // magical second argument comes into play, which fingerprints a whole
    // package. Remember that the fact that this is an `Option` is a bug, but a
    // longstanding bug, in Cargo. Recent refactorings just made it painfully
    // obvious.
    let pkg_root = unit.pkg.root().to_path_buf();
    let build_dir = build_root(build_runner);
    let env_config = Arc::clone(build_runner.bcx.gctx.env_config()?);
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
                    // be stored here rather than the mtime of them.
                    Some(f) => {
                        let s = f()?;
                        debug!(
                            "old local fingerprints deps {:?} precalculated={:?}",
                            pkg_root, s
                        );
                        return Ok(Some(vec![LocalFingerprint::Precalculated(s)]));
                    }
                    None => return Ok(None),
                }
            }

            // Ok so now we're in "new mode" where we can have files listed as
            // dependencies as well as env vars listed as dependencies. Process
            // them all here.
            Ok(Some(local_fingerprints_deps(
                deps,
                &build_dir,
                &pkg_root,
                &env_config,
            )))
        };

    // Note that `false` == "not overridden"
    Ok((Box::new(calculate), false))
}

/// Create a [`LocalFingerprint`] for an overridden build script.
/// Returns None if it is not overridden.
fn build_script_override_fingerprint(
    build_runner: &mut BuildRunner<'_, '_>,
    unit: &Unit,
) -> Option<LocalFingerprint> {
    // Build script output is only populated at this stage when it is
    // overridden.
    let build_script_outputs = build_runner.build_script_outputs.lock().unwrap();
    let metadata = build_runner.get_run_build_script_metadata(unit);
    // Returns None if it is not overridden.
    let output = build_script_outputs.get(metadata)?;
    let s = format!(
        "overridden build state with hash: {}",
        util::hash_u64(output)
    );
    Some(LocalFingerprint::Precalculated(s))
}

/// Compute the [`LocalFingerprint`] values for a [`RunCustomBuild`] unit for
/// non-overridden new-style build scripts only. This is only used when `deps`
/// is already known to have a nonempty `rerun-if-*` somewhere.
///
/// [`RunCustomBuild`]: crate::core::compiler::CompileMode::RunCustomBuild
fn local_fingerprints_deps(
    deps: &BuildDeps,
    build_root: &Path,
    pkg_root: &Path,
    env_config: &Arc<HashMap<String, OsString>>,
) -> Vec<LocalFingerprint> {
    debug!("new local fingerprints deps {:?}", pkg_root);
    let mut local = Vec::new();

    if !deps.rerun_if_changed.is_empty() {
        // Note that like the module comment above says we are careful to never
        // store an absolute path in `LocalFingerprint`, so ensure that we strip
        // absolute prefixes from them.
        let output = deps
            .build_script_output
            .strip_prefix(build_root)
            .unwrap()
            .to_path_buf();
        let paths = deps
            .rerun_if_changed
            .iter()
            .map(|p| p.strip_prefix(pkg_root).unwrap_or(p).to_path_buf())
            .collect();
        local.push(LocalFingerprint::RerunIfChanged { output, paths });
    }

    local.extend(
        deps.rerun_if_env_changed
            .iter()
            .map(|s| LocalFingerprint::from_env(s, env_config)),
    );

    local
}

/// Writes the short fingerprint hash value to `<loc>`
/// and logs detailed JSON information to `<loc>.json`.
fn write_fingerprint(loc: &Path, fingerprint: &Fingerprint) -> CargoResult<()> {
    debug_assert_ne!(fingerprint.rustc, 0);
    // fingerprint::new().rustc == 0, make sure it doesn't make it to the file system.
    // This is mostly so outside tools can reliably find out what rust version this file is for,
    // as we can use the full hash.
    let hash = fingerprint.hash_u64();
    debug!("write fingerprint ({:x}) : {}", hash, loc.display());
    paths::write(loc, util::to_hex(hash).as_bytes())?;

    let json = serde_json::to_string(fingerprint).unwrap();
    if cfg!(debug_assertions) {
        let f: Fingerprint = serde_json::from_str(&json).unwrap();
        assert_eq!(f.hash_u64(), hash);
    }
    paths::write(&loc.with_extension("json"), json.as_bytes())?;
    Ok(())
}

/// Prepare for work when a package starts to build
pub fn prepare_init(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<()> {
    let new1 = build_runner.files().fingerprint_dir(unit);

    // Doc tests have no output, thus no fingerprint.
    if !new1.exists() && !unit.mode.is_doc_test() {
        paths::create_dir_all(&new1)?;
    }

    Ok(())
}

/// Returns the location that the dep-info file will show up at
/// for the [`Unit`] specified.
pub fn dep_info_loc(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> PathBuf {
    build_runner.files().fingerprint_file_path(unit, "dep-")
}

/// Returns an absolute path that build directory.
/// All paths are rewritten to be relative to this.
fn build_root(build_runner: &BuildRunner<'_, '_>) -> PathBuf {
    build_runner.bcx.ws.build_dir().into_path_unlocked()
}

/// Reads the value from the old fingerprint hash file and compare.
///
/// If dirty, it then restores the detailed information
/// from the fingerprint JSON file, and provides an rich dirty reason.
fn compare_old_fingerprint(
    unit: &Unit,
    old_hash_path: &Path,
    new_fingerprint: &Fingerprint,
    mtime_on_use: bool,
    forced: bool,
) -> Option<DirtyReason> {
    if mtime_on_use {
        // update the mtime so other cleaners know we used it
        let t = FileTime::from_system_time(SystemTime::now());
        debug!("mtime-on-use forcing {:?} to {}", old_hash_path, t);
        paths::set_file_time_no_err(old_hash_path, t);
    }

    let compare = _compare_old_fingerprint(old_hash_path, new_fingerprint);

    match compare.as_ref() {
        Ok(None) => {}
        Ok(Some(reason)) => {
            info!(
                "fingerprint dirty for {}/{:?}/{:?}",
                unit.pkg, unit.mode, unit.target,
            );
            info!("    dirty: {reason:?}");
        }
        Err(e) => {
            info!(
                "fingerprint error for {}/{:?}/{:?}",
                unit.pkg, unit.mode, unit.target,
            );
            info!("    err: {e:?}");
        }
    }

    match compare {
        Ok(None) if forced => Some(DirtyReason::Forced),
        Ok(reason) => reason,
        Err(_) => Some(DirtyReason::FreshBuild),
    }
}

fn _compare_old_fingerprint(
    old_hash_path: &Path,
    new_fingerprint: &Fingerprint,
) -> CargoResult<Option<DirtyReason>> {
    let old_fingerprint_short = paths::read(old_hash_path)?;

    let new_hash = new_fingerprint.hash_u64();

    if util::to_hex(new_hash) == old_fingerprint_short && new_fingerprint.fs_status.up_to_date() {
        return Ok(None);
    }

    let old_fingerprint_json = paths::read(&old_hash_path.with_extension("json"))?;
    let old_fingerprint: Fingerprint = serde_json::from_str(&old_fingerprint_json)
        .with_context(|| internal("failed to deserialize json"))?;
    // Fingerprint can be empty after a failed rebuild (see comment in prepare_target).
    if !old_fingerprint_short.is_empty() {
        debug_assert_eq!(
            util::to_hex(old_fingerprint.hash_u64()),
            old_fingerprint_short
        );
    }

    Ok(Some(new_fingerprint.compare(&old_fingerprint)))
}

/// Calculates the fingerprint of a unit thats contains no dep-info files.
fn pkg_fingerprint(bcx: &BuildContext<'_, '_>, pkg: &Package) -> CargoResult<String> {
    let source_id = pkg.package_id().source_id();
    let sources = bcx.packages.sources();

    let source = sources
        .get(source_id)
        .ok_or_else(|| internal("missing package source"))?;
    source.fingerprint(pkg)
}

/// The `reference` file is considered as "stale" if any file from `paths` has a newer mtime.
fn find_stale_file<I, P>(
    mtime_cache: &mut HashMap<PathBuf, FileTime>,
    checksum_cache: &mut HashMap<PathBuf, Checksum>,
    reference: &Path,
    paths: I,
    use_checksums: bool,
) -> Option<StaleItem>
where
    I: IntoIterator<Item = (P, Option<(u64, Checksum)>)>,
    P: AsRef<Path>,
{
    let reference_mtime = match paths::mtime(reference) {
        Ok(mtime) => mtime,
        Err(..) => {
            return Some(StaleItem::MissingFile {
                path: reference.to_path_buf(),
            });
        }
    };

    let skippable_dirs = if let Ok(cargo_home) = home::cargo_home() {
        let skippable_dirs: Vec<_> = ["git", "registry"]
            .into_iter()
            .map(|subfolder| cargo_home.join(subfolder))
            .collect();
        Some(skippable_dirs)
    } else {
        None
    };
    for (path, prior_checksum) in paths {
        let path = path.as_ref();

        // Assuming anything in cargo_home/{git, registry} is immutable
        // (see also #9455 about marking the src directory readonly) which avoids rebuilds when CI
        // caches $CARGO_HOME/registry/{index, cache} and $CARGO_HOME/git/db across runs, keeping
        // the content the same but changing the mtime.
        if let Some(ref skippable_dirs) = skippable_dirs {
            if skippable_dirs.iter().any(|dir| path.starts_with(dir)) {
                continue;
            }
        }
        if use_checksums {
            let Some((file_len, prior_checksum)) = prior_checksum else {
                return Some(StaleItem::MissingChecksum {
                    path: path.to_path_buf(),
                });
            };
            let path_buf = path.to_path_buf();

            let path_checksum = match checksum_cache.entry(path_buf) {
                Entry::Occupied(o) => *o.get(),
                Entry::Vacant(v) => {
                    let Ok(current_file_len) = fs::metadata(&path).map(|m| m.len()) else {
                        return Some(StaleItem::FailedToReadMetadata {
                            path: path.to_path_buf(),
                        });
                    };
                    if current_file_len != file_len {
                        return Some(StaleItem::FileSizeChanged {
                            path: path.to_path_buf(),
                            new_size: current_file_len,
                            old_size: file_len,
                        });
                    }
                    let Ok(file) = File::open(path) else {
                        return Some(StaleItem::MissingFile {
                            path: path.to_path_buf(),
                        });
                    };
                    let Ok(checksum) = Checksum::compute(prior_checksum.algo(), file) else {
                        return Some(StaleItem::UnableToReadFile {
                            path: path.to_path_buf(),
                        });
                    };
                    *v.insert(checksum)
                }
            };
            if path_checksum == prior_checksum {
                continue;
            }
            return Some(StaleItem::ChangedChecksum {
                source: path.to_path_buf(),
                stored_checksum: prior_checksum,
                new_checksum: path_checksum,
            });
        } else {
            let path_mtime = match mtime_cache.entry(path.to_path_buf()) {
                Entry::Occupied(o) => *o.get(),
                Entry::Vacant(v) => {
                    let Ok(mtime) = paths::mtime_recursive(path) else {
                        return Some(StaleItem::MissingFile {
                            path: path.to_path_buf(),
                        });
                    };
                    *v.insert(mtime)
                }
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

            return Some(StaleItem::ChangedFile {
                reference: reference.to_path_buf(),
                reference_mtime,
                stale: path.to_path_buf(),
                stale_mtime: path_mtime,
            });
        }
    }

    debug!(
        "all paths up-to-date relative to {:?} mtime={}",
        reference, reference_mtime
    );
    None
}
