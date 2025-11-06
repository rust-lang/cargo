//! How to execute a build script and parse its output.
//!
//! ## Preparing a build script run
//!
//! A [build script] is an optional Rust script Cargo will run before building
//! your package. As of this writing, two kinds of special [`Unit`]s will be
//! constructed when there is a build script in a package.
//!
//! * Build script compilation --- This unit is generally the same as units
//!   that would compile other Cargo targets. It will recursively creates units
//!   of its dependencies. One biggest difference is that the [`Unit`] of
//!   compiling a build script is flagged as [`TargetKind::CustomBuild`].
//! * Build script execution --- During the construction of the [`UnitGraph`],
//!   Cargo inserts a [`Unit`] with [`CompileMode::RunCustomBuild`]. This unit
//!   depends on the unit of compiling the associated build script, to ensure
//!   the executable is available before running. The [`Work`] of running the
//!   build script is prepared in the function [`prepare`].
//!
//! ## Running a build script
//!
//! When running a build script, Cargo is aware of the progress and the result
//! of a build script. Standard output is the chosen interprocess communication
//! between Cargo and build script processes. A set of strings is defined for
//! that purpose. These strings, a.k.a. instructions, are interpreted by
//! [`BuildOutput::parse`] and stored in [`BuildRunner::build_script_outputs`].
//! The entire execution work is constructed by [`build_work`].
//!
//! [build script]: https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html
//! [`TargetKind::CustomBuild`]: crate::core::manifest::TargetKind::CustomBuild
//! [`UnitGraph`]: super::unit_graph::UnitGraph
//! [`CompileMode::RunCustomBuild`]: crate::core::compiler::CompileMode::RunCustomBuild
//! [instructions]: https://doc.rust-lang.org/cargo/reference/build-scripts.html#outputs-of-the-build-script

use super::{BuildRunner, Job, Unit, Work, fingerprint, get_dynamic_search_path};
use crate::core::compiler::CompileMode;
use crate::core::compiler::artifact;
use crate::core::compiler::build_runner::UnitHash;
use crate::core::compiler::job_queue::JobState;
use crate::core::{PackageId, Target, profiles::ProfileRoot};
use crate::util::errors::CargoResult;
use crate::util::internal;
use crate::util::machine_message::{self, Message};
use anyhow::{Context as _, bail};
use cargo_platform::Cfg;
use cargo_util::paths;
use cargo_util_schemas::manifest::RustVersion;
use std::collections::hash_map::{Entry, HashMap};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::str::{self, FromStr};
use std::sync::{Arc, Mutex};

/// A build script instruction that tells Cargo to display an error after the
/// build script has finished running. Read [the doc] for more.
///
/// [the doc]: https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#cargo-error
const CARGO_ERROR_SYNTAX: &str = "cargo::error=";
/// Deprecated: A build script instruction that tells Cargo to display a warning after the
/// build script has finished running. Read [the doc] for more.
///
/// [the doc]: https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#cargo-warning
const OLD_CARGO_WARNING_SYNTAX: &str = "cargo:warning=";
/// A build script instruction that tells Cargo to display a warning after the
/// build script has finished running. Read [the doc] for more.
///
/// [the doc]: https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#cargo-warning
const NEW_CARGO_WARNING_SYNTAX: &str = "cargo::warning=";

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

pub type LogMessage = (Severity, String);

/// Represents a path added to the library search path.
///
/// We need to keep track of requests to add search paths within the cargo build directory
/// separately from paths outside of Cargo. The reason is that we want to give precedence to linking
/// against libraries within the Cargo build directory even if a similar library exists in the
/// system (e.g. crate A adds `/usr/lib` to the search path and then a later build of crate B adds
/// `target/debug/...` to satisfy its request to link against the library B that it built, but B is
/// also found in `/usr/lib`).
///
/// There's some nuance here because we want to preserve relative order of paths of the same type.
/// For example, if the build process would in declaration order emit the following linker line:
/// ```bash
/// -L/usr/lib -Ltarget/debug/build/crate1/libs -L/lib -Ltarget/debug/build/crate2/libs)
/// ```
///
/// we want the linker to actually receive:
/// ```bash
/// -Ltarget/debug/build/crate1/libs -Ltarget/debug/build/crate2/libs) -L/usr/lib -L/lib
/// ```
///
/// so that the library search paths within the crate artifacts directory come first but retain
/// relative ordering while the system library paths come after while still retaining relative
/// ordering among them; ordering is the order they are emitted within the build process,
/// not lexicographic order.
///
/// WARNING: Even though this type implements PartialOrd + Ord, this is a lexicographic ordering.
/// The linker line will require an explicit sorting algorithm. PartialOrd + Ord is derived because
/// BuildOutput requires it but that ordering is different from the one for the linker search path,
/// at least today. It may be worth reconsidering & perhaps it's ok if BuildOutput doesn't have
/// a lexicographic ordering for the library_paths? I'm not sure the consequence of that.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum LibraryPath {
    /// The path is pointing within the output folder of the crate and takes priority over
    /// external paths when passed to the linker.
    CargoArtifact(PathBuf),
    /// The path is pointing outside of the crate's build location. The linker will always
    /// receive such paths after `CargoArtifact`.
    External(PathBuf),
}

impl LibraryPath {
    fn new(p: PathBuf, script_out_dir: &Path) -> Self {
        let search_path = get_dynamic_search_path(&p);
        if search_path.starts_with(script_out_dir) {
            Self::CargoArtifact(p)
        } else {
            Self::External(p)
        }
    }

    pub fn into_path_buf(self) -> PathBuf {
        match self {
            LibraryPath::CargoArtifact(p) | LibraryPath::External(p) => p,
        }
    }
}

impl AsRef<PathBuf> for LibraryPath {
    fn as_ref(&self) -> &PathBuf {
        match self {
            LibraryPath::CargoArtifact(p) | LibraryPath::External(p) => p,
        }
    }
}

/// Contains the parsed output of a custom build script.
#[derive(Clone, Debug, Hash, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct BuildOutput {
    /// Paths to pass to rustc with the `-L` flag.
    pub library_paths: Vec<LibraryPath>,
    /// Names and link kinds of libraries, suitable for the `-l` flag.
    pub library_links: Vec<String>,
    /// Linker arguments suitable to be passed to `-C link-arg=<args>`
    pub linker_args: Vec<(LinkArgTarget, String)>,
    /// Various `--cfg` flags to pass to the compiler.
    pub cfgs: Vec<String>,
    /// Various `--check-cfg` flags to pass to the compiler.
    pub check_cfgs: Vec<String>,
    /// Additional environment variables to run the compiler with.
    pub env: Vec<(String, String)>,
    /// Metadata to pass to the immediate dependencies.
    pub metadata: Vec<(String, String)>,
    /// Paths to trigger a rerun of this build script.
    /// May be absolute or relative paths (relative to package root).
    pub rerun_if_changed: Vec<PathBuf>,
    /// Environment variables which, when changed, will cause a rebuild.
    pub rerun_if_env_changed: Vec<String>,
    /// Errors and warnings generated by this build.
    ///
    /// These are only displayed if this is a "local" package, `-vv` is used, or
    /// there is a build error for any target in this package. Note that any log
    /// message of severity `Error` will by itself cause a build error, and will
    /// cause all log messages to be displayed.
    pub log_messages: Vec<LogMessage>,
}

/// Map of packages to build script output.
///
/// This initially starts out as empty. Overridden build scripts get
/// inserted during `build_map`. The rest of the entries are added
/// immediately after each build script runs.
///
/// The [`UnitHash`] is the unique metadata hash for the `RunCustomBuild` Unit of
/// the package. It needs a unique key, since the build script can be run
/// multiple times with different profiles or features. We can't embed a
/// `Unit` because this structure needs to be shareable between threads.
#[derive(Default)]
pub struct BuildScriptOutputs {
    outputs: HashMap<UnitHash, BuildOutput>,
}

/// Linking information for a `Unit`.
///
/// See [`build_map`] for more details.
#[derive(Default)]
pub struct BuildScripts {
    /// List of build script outputs this Unit needs to include for linking. Each
    /// element is an index into `BuildScriptOutputs`.
    ///
    /// Cargo will use this `to_link` vector to add `-L` flags to compiles as we
    /// propagate them upwards towards the final build. Note, however, that we
    /// need to preserve the ordering of `to_link` to be topologically sorted.
    /// This will ensure that build scripts which print their paths properly will
    /// correctly pick up the files they generated (if there are duplicates
    /// elsewhere).
    ///
    /// To preserve this ordering, the (id, metadata) is stored in two places, once
    /// in the `Vec` and once in `seen_to_link` for a fast lookup. We maintain
    /// this as we're building interactively below to ensure that the memory
    /// usage here doesn't blow up too much.
    ///
    /// For more information, see #2354.
    pub to_link: Vec<(PackageId, UnitHash)>,
    /// This is only used while constructing `to_link` to avoid duplicates.
    seen_to_link: HashSet<(PackageId, UnitHash)>,
    /// Host-only dependencies that have build scripts. Each element is an
    /// index into `BuildScriptOutputs`.
    ///
    /// This is the set of transitive dependencies that are host-only
    /// (proc-macro, plugin, build-dependency) that contain a build script.
    /// Any `BuildOutput::library_paths` path relative to `target` will be
    /// added to `LD_LIBRARY_PATH` so that the compiler can find any dynamic
    /// libraries a build script may have generated.
    pub plugins: BTreeSet<(PackageId, UnitHash)>,
}

/// Dependency information as declared by a build script that might trigger
/// a recompile of itself.
#[derive(Debug)]
pub struct BuildDeps {
    /// Absolute path to the file in the target directory that stores the
    /// output of the build script.
    pub build_script_output: PathBuf,
    /// Files that trigger a rebuild if they change.
    pub rerun_if_changed: Vec<PathBuf>,
    /// Environment variables that trigger a rebuild if they change.
    pub rerun_if_env_changed: Vec<String>,
}

/// Represents one of the instructions from `cargo::rustc-link-arg-*` build
/// script instruction family.
///
/// In other words, indicates targets that custom linker arguments applies to.
///
/// See the [build script documentation][1] for more.
///
/// [1]: https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#cargorustc-link-argflag
#[derive(Clone, Hash, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinkArgTarget {
    /// Represents `cargo::rustc-link-arg=FLAG`.
    All,
    /// Represents `cargo::rustc-cdylib-link-arg=FLAG`.
    Cdylib,
    /// Represents `cargo::rustc-link-arg-bins=FLAG`.
    Bin,
    /// Represents `cargo::rustc-link-arg-bin=BIN=FLAG`.
    SingleBin(String),
    /// Represents `cargo::rustc-link-arg-tests=FLAG`.
    Test,
    /// Represents `cargo::rustc-link-arg-benches=FLAG`.
    Bench,
    /// Represents `cargo::rustc-link-arg-examples=FLAG`.
    Example,
}

impl LinkArgTarget {
    /// Checks if this link type applies to a given [`Target`].
    pub fn applies_to(&self, target: &Target, mode: CompileMode) -> bool {
        let is_test = mode.is_any_test();
        match self {
            LinkArgTarget::All => true,
            LinkArgTarget::Cdylib => !is_test && target.is_cdylib(),
            LinkArgTarget::Bin => target.is_bin(),
            LinkArgTarget::SingleBin(name) => target.is_bin() && target.name() == name,
            LinkArgTarget::Test => target.is_test(),
            LinkArgTarget::Bench => target.is_bench(),
            LinkArgTarget::Example => target.is_exe_example(),
        }
    }
}

/// Prepares a `Work` that executes the target as a custom build script.
#[tracing::instrument(skip_all)]
pub fn prepare(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<Job> {
    let metadata = build_runner.get_run_build_script_metadata(unit);
    if build_runner
        .build_script_outputs
        .lock()
        .unwrap()
        .contains_key(metadata)
    {
        // The output is already set, thus the build script is overridden.
        fingerprint::prepare_target(build_runner, unit, false)
    } else {
        build_work(build_runner, unit)
    }
}

/// Emits the output of a build script as a [`machine_message::BuildScript`]
/// JSON string to standard output.
fn emit_build_output(
    state: &JobState<'_, '_>,
    output: &BuildOutput,
    out_dir: &Path,
    package_id: PackageId,
) -> CargoResult<()> {
    let library_paths = output
        .library_paths
        .iter()
        .map(|l| l.as_ref().display().to_string())
        .collect::<Vec<_>>();

    let msg = machine_message::BuildScript {
        package_id: package_id.to_spec(),
        linked_libs: &output.library_links,
        linked_paths: &library_paths,
        cfgs: &output.cfgs,
        env: &output.env,
        out_dir,
    }
    .to_json_string();
    state.stdout(msg)?;
    Ok(())
}

/// Constructs the unit of work of running a build script.
///
/// The construction includes:
///
/// * Set environment variables for the build script run.
/// * Create the output dir (`OUT_DIR`) for the build script output.
/// * Determine if the build script needs a re-run.
/// * Run the build script and store its output.
fn build_work(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<Job> {
    assert!(unit.mode.is_run_custom_build());
    let bcx = &build_runner.bcx;
    let dependencies = build_runner.unit_deps(unit);
    let build_script_unit = dependencies
        .iter()
        .find(|d| !d.unit.mode.is_run_custom_build() && d.unit.target.is_custom_build())
        .map(|d| &d.unit)
        .expect("running a script not depending on an actual script");
    let script_dir = build_runner.files().build_script_dir(build_script_unit);
    let script_out_dir = build_runner.files().build_script_out_dir(unit);
    let script_run_dir = build_runner.files().build_script_run_dir(unit);

    if let Some(deps) = unit.pkg.manifest().metabuild() {
        prepare_metabuild(build_runner, build_script_unit, deps)?;
    }

    // Building the command to execute
    let to_exec = script_dir.join(unit.target.name());

    // Start preparing the process to execute, starting out with some
    // environment variables. Note that the profile-related environment
    // variables are not set with this the build script's profile but rather the
    // package's library profile.
    // NOTE: if you add any profile flags, be sure to update
    // `Profiles::get_profile_run_custom_build` so that those flags get
    // carried over.
    let to_exec = to_exec.into_os_string();
    let mut cmd = build_runner.compilation.host_process(to_exec, &unit.pkg)?;
    let debug = unit.profile.debuginfo.is_turned_on();
    cmd.env("OUT_DIR", &script_out_dir)
        .env("CARGO_MANIFEST_DIR", unit.pkg.root())
        .env("CARGO_MANIFEST_PATH", unit.pkg.manifest_path())
        .env("NUM_JOBS", &bcx.jobs().to_string())
        .env("TARGET", bcx.target_data.short_name(&unit.kind))
        .env("DEBUG", debug.to_string())
        .env("OPT_LEVEL", &unit.profile.opt_level)
        .env(
            "PROFILE",
            match unit.profile.root {
                ProfileRoot::Release => "release",
                ProfileRoot::Debug => "debug",
            },
        )
        .env("HOST", &bcx.host_triple())
        .env("RUSTC", &bcx.rustc().path)
        .env("RUSTDOC", &*bcx.gctx.rustdoc()?)
        .inherit_jobserver(&build_runner.jobserver);

    // Find all artifact dependencies and make their file and containing directory discoverable using environment variables.
    for (var, value) in artifact::get_env(build_runner, dependencies)? {
        cmd.env(&var, value);
    }

    if let Some(linker) = &build_runner.compilation.target_linker(unit.kind) {
        cmd.env("RUSTC_LINKER", linker);
    }

    if let Some(links) = unit.pkg.manifest().links() {
        cmd.env("CARGO_MANIFEST_LINKS", links);
    }

    if let Some(trim_paths) = unit.profile.trim_paths.as_ref() {
        cmd.env("CARGO_TRIM_PATHS", trim_paths.to_string());
    }

    // Be sure to pass along all enabled features for this package, this is the
    // last piece of statically known information that we have.
    for feat in &unit.features {
        cmd.env(&format!("CARGO_FEATURE_{}", super::envify(feat)), "1");
    }

    let mut cfg_map = HashMap::new();
    cfg_map.insert(
        "feature",
        unit.features.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );
    for cfg in bcx.target_data.cfg(unit.kind) {
        match *cfg {
            Cfg::Name(ref n) => {
                cfg_map.insert(n.as_str(), Vec::new());
            }
            Cfg::KeyPair(ref k, ref v) => {
                let values = cfg_map.entry(k.as_str()).or_default();
                values.push(v.as_str());
            }
        }
    }
    for (k, v) in cfg_map {
        if k == "debug_assertions" {
            // This cfg is always true and misleading, so avoid setting it.
            // That is because Cargo queries rustc without any profile settings.
            continue;
        }
        // FIXME: We should handle raw-idents somehow instead of predenting they
        // don't exist here
        let k = format!("CARGO_CFG_{}", super::envify(k));
        cmd.env(&k, v.join(","));
    }

    // Also inform the build script of the rustc compiler context.
    if let Some(wrapper) = bcx.rustc().wrapper.as_ref() {
        cmd.env("RUSTC_WRAPPER", wrapper);
    } else {
        cmd.env_remove("RUSTC_WRAPPER");
    }
    cmd.env_remove("RUSTC_WORKSPACE_WRAPPER");
    if build_runner.bcx.ws.is_member(&unit.pkg) {
        if let Some(wrapper) = bcx.rustc().workspace_wrapper.as_ref() {
            cmd.env("RUSTC_WORKSPACE_WRAPPER", wrapper);
        }
    }
    cmd.env("CARGO_ENCODED_RUSTFLAGS", unit.rustflags.join("\x1f"));
    cmd.env_remove("RUSTFLAGS");

    if build_runner.bcx.ws.gctx().extra_verbose() {
        cmd.display_env_vars();
    }

    // Gather the set of native dependencies that this package has along with
    // some other variables to close over.
    //
    // This information will be used at build-time later on to figure out which
    // sorts of variables need to be discovered at that time.
    let lib_deps = dependencies
        .iter()
        .filter_map(|dep| {
            if dep.unit.mode.is_run_custom_build() {
                let dep_metadata = build_runner.get_run_build_script_metadata(&dep.unit);
                Some((
                    dep.unit.pkg.manifest().links().unwrap().to_string(),
                    dep.unit.pkg.package_id(),
                    dep_metadata,
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let library_name = unit.pkg.library().map(|t| t.crate_name());
    let pkg_descr = unit.pkg.to_string();
    let build_script_outputs = Arc::clone(&build_runner.build_script_outputs);
    let id = unit.pkg.package_id();
    let output_file = script_run_dir.join("output");
    let err_file = script_run_dir.join("stderr");
    let root_output_file = script_run_dir.join("root-output");
    let host_target_root = build_runner.files().host_dest().to_path_buf();
    let all = (
        id,
        library_name.clone(),
        pkg_descr.clone(),
        Arc::clone(&build_script_outputs),
        output_file.clone(),
        script_out_dir.clone(),
    );
    let build_scripts = build_runner.build_scripts.get(unit).cloned();
    let json_messages = bcx.build_config.emit_json();
    let extra_verbose = bcx.gctx.extra_verbose();
    let (prev_output, prev_script_out_dir) = prev_build_output(build_runner, unit);
    let metadata_hash = build_runner.get_run_build_script_metadata(unit);

    paths::create_dir_all(&script_dir)?;
    paths::create_dir_all(&script_out_dir)?;

    let nightly_features_allowed = build_runner.bcx.gctx.nightly_features_allowed;
    let targets: Vec<Target> = unit.pkg.targets().to_vec();
    let msrv = unit.pkg.rust_version().cloned();
    // Need a separate copy for the fresh closure.
    let targets_fresh = targets.clone();
    let msrv_fresh = msrv.clone();

    let env_profile_name = unit.profile.name.to_uppercase();
    let built_with_debuginfo = build_runner
        .bcx
        .unit_graph
        .get(unit)
        .and_then(|deps| deps.iter().find(|dep| dep.unit.target == unit.target))
        .map(|dep| dep.unit.profile.debuginfo.is_turned_on())
        .unwrap_or(false);

    // Prepare the unit of "dirty work" which will actually run the custom build
    // command.
    //
    // Note that this has to do some extra work just before running the command
    // to determine extra environment variables and such.
    let dirty = Work::new(move |state| {
        // Make sure that OUT_DIR exists.
        //
        // If we have an old build directory, then just move it into place,
        // otherwise create it!
        paths::create_dir_all(&script_out_dir)
            .context("failed to create script output directory for build command")?;

        // For all our native lib dependencies, pick up their metadata to pass
        // along to this custom build command. We're also careful to augment our
        // dynamic library search path in case the build script depended on any
        // native dynamic libraries.
        {
            let build_script_outputs = build_script_outputs.lock().unwrap();
            for (name, dep_id, dep_metadata) in lib_deps {
                let script_output = build_script_outputs.get(dep_metadata).ok_or_else(|| {
                    internal(format!(
                        "failed to locate build state for env vars: {}/{}",
                        dep_id, dep_metadata
                    ))
                })?;
                let data = &script_output.metadata;
                for (key, value) in data.iter() {
                    cmd.env(
                        &format!("DEP_{}_{}", super::envify(&name), super::envify(key)),
                        value,
                    );
                }
            }
            if let Some(build_scripts) = build_scripts {
                super::add_plugin_deps(
                    &mut cmd,
                    &build_script_outputs,
                    &build_scripts,
                    &host_target_root,
                )?;
            }
        }

        // And now finally, run the build command itself!
        state.running(&cmd);
        let timestamp = paths::set_invocation_time(&script_run_dir)?;
        let prefix = format!("[{} {}] ", id.name(), id.version());
        let mut log_messages_in_case_of_panic = Vec::new();
        let span = tracing::debug_span!("build_script", process = cmd.to_string());
        let output = span.in_scope(|| {
            cmd.exec_with_streaming(
                &mut |stdout| {
                    if let Some(error) = stdout.strip_prefix(CARGO_ERROR_SYNTAX) {
                        log_messages_in_case_of_panic.push((Severity::Error, error.to_owned()));
                    }
                    if let Some(warning) = stdout
                        .strip_prefix(OLD_CARGO_WARNING_SYNTAX)
                        .or(stdout.strip_prefix(NEW_CARGO_WARNING_SYNTAX))
                    {
                        log_messages_in_case_of_panic.push((Severity::Warning, warning.to_owned()));
                    }
                    if extra_verbose {
                        state.stdout(format!("{}{}", prefix, stdout))?;
                    }
                    Ok(())
                },
                &mut |stderr| {
                    if extra_verbose {
                        state.stderr(format!("{}{}", prefix, stderr))?;
                    }
                    Ok(())
                },
                true,
            )
            .with_context(|| {
                let mut build_error_context =
                    format!("failed to run custom build command for `{}`", pkg_descr);

                // If we're opting into backtraces, mention that build dependencies' backtraces can
                // be improved by requesting debuginfo to be built, if we're not building with
                // debuginfo already.
                //
                // ALLOWED: Other tools like `rustc` might read it directly
                // through `std::env`. We should make their behavior consistent.
                #[allow(clippy::disallowed_methods)]
                if let Ok(show_backtraces) = std::env::var("RUST_BACKTRACE") {
                    if !built_with_debuginfo && show_backtraces != "0" {
                        build_error_context.push_str(&format!(
                            "\n\
                            note: To improve backtraces for build dependencies, set the \
                            CARGO_PROFILE_{env_profile_name}_BUILD_OVERRIDE_DEBUG=true environment \
                            variable to enable debug information generation.",
                        ));
                    }
                }

                build_error_context
            })
        });

        // If the build failed
        if let Err(error) = output {
            insert_log_messages_in_build_outputs(
                build_script_outputs,
                id,
                metadata_hash,
                log_messages_in_case_of_panic,
            );
            return Err(error);
        }
        // ... or it logged any errors
        else if log_messages_in_case_of_panic
            .iter()
            .any(|(severity, _)| *severity == Severity::Error)
        {
            insert_log_messages_in_build_outputs(
                build_script_outputs,
                id,
                metadata_hash,
                log_messages_in_case_of_panic,
            );
            anyhow::bail!("build script logged errors");
        }

        let output = output.unwrap();

        // After the build command has finished running, we need to be sure to
        // remember all of its output so we can later discover precisely what it
        // was, even if we don't run the build command again (due to freshness).
        //
        // This is also the location where we provide feedback into the build
        // state informing what variables were discovered via our script as
        // well.
        paths::write(&output_file, &output.stdout)?;
        // This mtime shift allows Cargo to detect if a source file was
        // modified in the middle of the build.
        paths::set_file_time_no_err(output_file, timestamp);
        paths::write(&err_file, &output.stderr)?;
        paths::write(&root_output_file, paths::path2bytes(&script_out_dir)?)?;
        let parsed_output = BuildOutput::parse(
            &output.stdout,
            library_name,
            &pkg_descr,
            &script_out_dir,
            &script_out_dir,
            nightly_features_allowed,
            &targets,
            &msrv,
        )?;

        if json_messages {
            emit_build_output(state, &parsed_output, script_out_dir.as_path(), id)?;
        }
        build_script_outputs
            .lock()
            .unwrap()
            .insert(id, metadata_hash, parsed_output);
        Ok(())
    });

    // Now that we've prepared our work-to-do, we need to prepare the fresh work
    // itself to run when we actually end up just discarding what we calculated
    // above.
    let fresh = Work::new(move |state| {
        let (id, library_name, pkg_descr, build_script_outputs, output_file, script_out_dir) = all;
        let output = match prev_output {
            Some(output) => output,
            None => BuildOutput::parse_file(
                &output_file,
                library_name,
                &pkg_descr,
                &prev_script_out_dir,
                &script_out_dir,
                nightly_features_allowed,
                &targets_fresh,
                &msrv_fresh,
            )?,
        };

        if json_messages {
            emit_build_output(state, &output, script_out_dir.as_path(), id)?;
        }

        build_script_outputs
            .lock()
            .unwrap()
            .insert(id, metadata_hash, output);
        Ok(())
    });

    let mut job = fingerprint::prepare_target(build_runner, unit, false)?;
    if job.freshness().is_dirty() {
        job.before(dirty);
    } else {
        job.before(fresh);
    }
    Ok(job)
}

/// When a build script run fails, store only log messages, and nuke other
/// outputs, as they are likely broken.
fn insert_log_messages_in_build_outputs(
    build_script_outputs: Arc<Mutex<BuildScriptOutputs>>,
    id: PackageId,
    metadata_hash: UnitHash,
    log_messages: Vec<LogMessage>,
) {
    let build_output_with_only_log_messages = BuildOutput {
        log_messages,
        ..BuildOutput::default()
    };
    build_script_outputs.lock().unwrap().insert(
        id,
        metadata_hash,
        build_output_with_only_log_messages,
    );
}

impl BuildOutput {
    /// Like [`BuildOutput::parse`] but from a file path.
    pub fn parse_file(
        path: &Path,
        library_name: Option<String>,
        pkg_descr: &str,
        script_out_dir_when_generated: &Path,
        script_out_dir: &Path,
        nightly_features_allowed: bool,
        targets: &[Target],
        msrv: &Option<RustVersion>,
    ) -> CargoResult<BuildOutput> {
        let contents = paths::read_bytes(path)?;
        BuildOutput::parse(
            &contents,
            library_name,
            pkg_descr,
            script_out_dir_when_generated,
            script_out_dir,
            nightly_features_allowed,
            targets,
            msrv,
        )
    }

    /// Parses the output instructions of a build script.
    ///
    /// * `pkg_descr` --- for error messages
    /// * `library_name` --- for determining if `RUSTC_BOOTSTRAP` should be allowed
    pub fn parse(
        input: &[u8],
        // Takes String instead of InternedString so passing `unit.pkg.name()` will give a compile error.
        library_name: Option<String>,
        pkg_descr: &str,
        script_out_dir_when_generated: &Path,
        script_out_dir: &Path,
        nightly_features_allowed: bool,
        targets: &[Target],
        msrv: &Option<RustVersion>,
    ) -> CargoResult<BuildOutput> {
        let mut library_paths = Vec::new();
        let mut library_links = Vec::new();
        let mut linker_args = Vec::new();
        let mut cfgs = Vec::new();
        let mut check_cfgs = Vec::new();
        let mut env = Vec::new();
        let mut metadata = Vec::new();
        let mut rerun_if_changed = Vec::new();
        let mut rerun_if_env_changed = Vec::new();
        let mut log_messages = Vec::new();
        let whence = format!("build script of `{}`", pkg_descr);
        // Old syntax:
        //    cargo:rustc-flags=VALUE
        //    cargo:KEY=VALUE (for other unreserved keys)
        // New syntax:
        //    cargo::rustc-flags=VALUE
        //    cargo::metadata=KEY=VALUE (for other unreserved keys)
        // Due to backwards compatibility, no new keys can be added to this old format.
        const RESERVED_PREFIXES: &[&str] = &[
            "rustc-flags=",
            "rustc-link-lib=",
            "rustc-link-search=",
            "rustc-link-arg-cdylib=",
            "rustc-cdylib-link-arg=",
            "rustc-link-arg-bins=",
            "rustc-link-arg-bin=",
            "rustc-link-arg-tests=",
            "rustc-link-arg-benches=",
            "rustc-link-arg-examples=",
            "rustc-link-arg=",
            "rustc-cfg=",
            "rustc-check-cfg=",
            "rustc-env=",
            "warning=",
            "rerun-if-changed=",
            "rerun-if-env-changed=",
        ];
        const DOCS_LINK_SUGGESTION: &str = "See https://doc.rust-lang.org/cargo/reference/build-scripts.html#outputs-of-the-build-script \
                for more information about build script outputs.";

        fn has_reserved_prefix(flag: &str) -> bool {
            RESERVED_PREFIXES
                .iter()
                .any(|reserved_prefix| flag.starts_with(reserved_prefix))
        }

        fn check_minimum_supported_rust_version_for_new_syntax(
            pkg_descr: &str,
            msrv: &Option<RustVersion>,
            flag: &str,
        ) -> CargoResult<()> {
            if let Some(msrv) = msrv {
                let new_syntax_added_in = RustVersion::from_str("1.77.0")?;
                if !new_syntax_added_in.is_compatible_with(msrv.as_partial()) {
                    let old_syntax_suggestion = if has_reserved_prefix(flag) {
                        format!(
                            "Switch to the old `cargo:{flag}` syntax (note the single colon).\n"
                        )
                    } else if flag.starts_with("metadata=") {
                        let old_format_flag = flag.strip_prefix("metadata=").unwrap();
                        format!(
                            "Switch to the old `cargo:{old_format_flag}` syntax instead of `cargo::{flag}` (note the single colon).\n"
                        )
                    } else {
                        String::new()
                    };

                    bail!(
                        "the `cargo::` syntax for build script output instructions was added in \
                        Rust 1.77.0, but the minimum supported Rust version of `{pkg_descr}` is {msrv}.\n\
                        {old_syntax_suggestion}\
                        {DOCS_LINK_SUGGESTION}"
                    );
                }
            }

            Ok(())
        }

        fn parse_directive<'a>(
            whence: &str,
            line: &str,
            data: &'a str,
            old_syntax: bool,
        ) -> CargoResult<(&'a str, &'a str)> {
            let mut iter = data.splitn(2, "=");
            let key = iter.next();
            let value = iter.next();
            match (key, value) {
                (Some(a), Some(b)) => Ok((a, b.trim_end())),
                _ => bail!(
                    "invalid output in {whence}: `{line}`\n\
                    Expected a line with `{syntax}KEY=VALUE` with an `=` character, \
                    but none was found.\n\
                    {DOCS_LINK_SUGGESTION}",
                    syntax = if old_syntax { "cargo:" } else { "cargo::" },
                ),
            }
        }

        fn parse_metadata<'a>(
            whence: &str,
            line: &str,
            data: &'a str,
            old_syntax: bool,
        ) -> CargoResult<(&'a str, &'a str)> {
            let mut iter = data.splitn(2, "=");
            let key = iter.next();
            let value = iter.next();
            match (key, value) {
                (Some(a), Some(b)) => Ok((a, b.trim_end())),
                _ => bail!(
                    "invalid output in {whence}: `{line}`\n\
                    Expected a line with `{syntax}KEY=VALUE` with an `=` character, \
                    but none was found.\n\
                    {DOCS_LINK_SUGGESTION}",
                    syntax = if old_syntax {
                        "cargo:"
                    } else {
                        "cargo::metadata="
                    },
                ),
            }
        }

        for line in input.split(|b| *b == b'\n') {
            let line = match str::from_utf8(line) {
                Ok(line) => line.trim(),
                Err(..) => continue,
            };
            let mut old_syntax = false;
            let (key, value) = if let Some(data) = line.strip_prefix("cargo::") {
                check_minimum_supported_rust_version_for_new_syntax(pkg_descr, msrv, data)?;
                // For instance, `cargo::rustc-flags=foo` or `cargo::metadata=foo=bar`.
                parse_directive(whence.as_str(), line, data, old_syntax)?
            } else if let Some(data) = line.strip_prefix("cargo:") {
                old_syntax = true;
                // For instance, `cargo:rustc-flags=foo`.
                if has_reserved_prefix(data) {
                    parse_directive(whence.as_str(), line, data, old_syntax)?
                } else {
                    // For instance, `cargo:foo=bar`.
                    ("metadata", data)
                }
            } else {
                // Skip this line since it doesn't start with "cargo:" or "cargo::".
                continue;
            };
            // This will rewrite paths if the target directory has been moved.
            let value = value.replace(
                script_out_dir_when_generated.to_str().unwrap(),
                script_out_dir.to_str().unwrap(),
            );

            let syntax_prefix = if old_syntax { "cargo:" } else { "cargo::" };
            macro_rules! check_and_add_target {
                ($target_kind: expr, $is_target_kind: expr, $link_type: expr) => {
                    if !targets.iter().any(|target| $is_target_kind(target)) {
                        bail!(
                            "invalid instruction `{}{}` from {}\n\
                                The package {} does not have a {} target.",
                            syntax_prefix,
                            key,
                            whence,
                            pkg_descr,
                            $target_kind
                        );
                    }
                    linker_args.push(($link_type, value));
                };
            }

            // Keep in sync with TargetConfig::parse_links_overrides.
            match key {
                "rustc-flags" => {
                    let (paths, links) = BuildOutput::parse_rustc_flags(&value, &whence)?;
                    library_links.extend(links.into_iter());
                    library_paths.extend(
                        paths
                            .into_iter()
                            .map(|p| LibraryPath::new(p, script_out_dir)),
                    );
                }
                "rustc-link-lib" => library_links.push(value.to_string()),
                "rustc-link-search" => {
                    library_paths.push(LibraryPath::new(PathBuf::from(value), script_out_dir))
                }
                "rustc-link-arg-cdylib" | "rustc-cdylib-link-arg" => {
                    if !targets.iter().any(|target| target.is_cdylib()) {
                        log_messages.push((
                            Severity::Warning,
                            format!(
                                "{}{} was specified in the build script of {}, \
                             but that package does not contain a cdylib target\n\
                             \n\
                             Allowing this was an unintended change in the 1.50 \
                             release, and may become an error in the future. \
                             For more information, see \
                             <https://github.com/rust-lang/cargo/issues/9562>.",
                                syntax_prefix, key, pkg_descr
                            ),
                        ));
                    }
                    linker_args.push((LinkArgTarget::Cdylib, value))
                }
                "rustc-link-arg-bins" => {
                    check_and_add_target!("bin", Target::is_bin, LinkArgTarget::Bin);
                }
                "rustc-link-arg-bin" => {
                    let (bin_name, arg) = value.split_once('=').ok_or_else(|| {
                        anyhow::format_err!(
                            "invalid instruction `{}{}={}` from {}\n\
                                The instruction should have the form {}{}=BIN=ARG",
                            syntax_prefix,
                            key,
                            value,
                            whence,
                            syntax_prefix,
                            key
                        )
                    })?;
                    if !targets
                        .iter()
                        .any(|target| target.is_bin() && target.name() == bin_name)
                    {
                        bail!(
                            "invalid instruction `{}{}` from {}\n\
                                The package {} does not have a bin target with the name `{}`.",
                            syntax_prefix,
                            key,
                            whence,
                            pkg_descr,
                            bin_name
                        );
                    }
                    linker_args.push((
                        LinkArgTarget::SingleBin(bin_name.to_owned()),
                        arg.to_string(),
                    ));
                }
                "rustc-link-arg-tests" => {
                    check_and_add_target!("test", Target::is_test, LinkArgTarget::Test);
                }
                "rustc-link-arg-benches" => {
                    check_and_add_target!("benchmark", Target::is_bench, LinkArgTarget::Bench);
                }
                "rustc-link-arg-examples" => {
                    check_and_add_target!("example", Target::is_example, LinkArgTarget::Example);
                }
                "rustc-link-arg" => {
                    linker_args.push((LinkArgTarget::All, value));
                }
                "rustc-cfg" => cfgs.push(value.to_string()),
                "rustc-check-cfg" => check_cfgs.push(value.to_string()),
                "rustc-env" => {
                    let (key, val) = BuildOutput::parse_rustc_env(&value, &whence)?;
                    // Build scripts aren't allowed to set RUSTC_BOOTSTRAP.
                    // See https://github.com/rust-lang/cargo/issues/7088.
                    if key == "RUSTC_BOOTSTRAP" {
                        // If RUSTC_BOOTSTRAP is already set, the user of Cargo knows about
                        // bootstrap and still wants to override the channel. Give them a way to do
                        // so, but still emit a warning that the current crate shouldn't be trying
                        // to set RUSTC_BOOTSTRAP.
                        // If this is a nightly build, setting RUSTC_BOOTSTRAP wouldn't affect the
                        // behavior, so still only give a warning.
                        // NOTE: cargo only allows nightly features on RUSTC_BOOTSTRAP=1, but we
                        // want setting any value of RUSTC_BOOTSTRAP to downgrade this to a warning
                        // (so that `RUSTC_BOOTSTRAP=library_name` will work)
                        let rustc_bootstrap_allows = |name: Option<&str>| {
                            let name = match name {
                                // as of 2021, no binaries on crates.io use RUSTC_BOOTSTRAP, so
                                // fine-grained opt-outs aren't needed. end-users can always use
                                // RUSTC_BOOTSTRAP=1 from the top-level if it's really a problem.
                                None => return false,
                                Some(n) => n,
                            };
                            // ALLOWED: the process of rustc bootstrapping reads this through
                            // `std::env`. We should make the behavior consistent. Also, we
                            // don't advertise this for bypassing nightly.
                            #[allow(clippy::disallowed_methods)]
                            std::env::var("RUSTC_BOOTSTRAP")
                                .map_or(false, |var| var.split(',').any(|s| s == name))
                        };
                        if nightly_features_allowed
                            || rustc_bootstrap_allows(library_name.as_deref())
                        {
                            log_messages.push((Severity::Warning, format!("Cannot set `RUSTC_BOOTSTRAP={}` from {}.\n\
                                note: Crates cannot set `RUSTC_BOOTSTRAP` themselves, as doing so would subvert the stability guarantees of Rust for your project.",
                                val, whence
                            )));
                        } else {
                            // Setting RUSTC_BOOTSTRAP would change the behavior of the crate.
                            // Abort with an error.
                            bail!(
                                "Cannot set `RUSTC_BOOTSTRAP={}` from {}.\n\
                                note: Crates cannot set `RUSTC_BOOTSTRAP` themselves, as doing so would subvert the stability guarantees of Rust for your project.\n\
                                help: If you're sure you want to do this in your project, set the environment variable `RUSTC_BOOTSTRAP={}` before running cargo instead.",
                                val,
                                whence,
                                library_name.as_deref().unwrap_or("1"),
                            );
                        }
                    } else {
                        env.push((key, val));
                    }
                }
                "error" => log_messages.push((Severity::Error, value.to_string())),
                "warning" => log_messages.push((Severity::Warning, value.to_string())),
                "rerun-if-changed" => rerun_if_changed.push(PathBuf::from(value)),
                "rerun-if-env-changed" => rerun_if_env_changed.push(value.to_string()),
                "metadata" => {
                    let (key, value) = parse_metadata(whence.as_str(), line, &value, old_syntax)?;
                    metadata.push((key.to_owned(), value.to_owned()));
                }
                _ => bail!(
                    "invalid output in {whence}: `{line}`\n\
                    Unknown key: `{key}`.\n\
                    {DOCS_LINK_SUGGESTION}",
                ),
            }
        }

        Ok(BuildOutput {
            library_paths,
            library_links,
            linker_args,
            cfgs,
            check_cfgs,
            env,
            metadata,
            rerun_if_changed,
            rerun_if_env_changed,
            log_messages,
        })
    }

    /// Parses [`cargo::rustc-flags`] instruction.
    ///
    /// [`cargo::rustc-flags`]: https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#cargorustc-flagsflags
    pub fn parse_rustc_flags(
        value: &str,
        whence: &str,
    ) -> CargoResult<(Vec<PathBuf>, Vec<String>)> {
        let value = value.trim();
        let mut flags_iter = value
            .split(|c: char| c.is_whitespace())
            .filter(|w| w.chars().any(|c| !c.is_whitespace()));
        let (mut library_paths, mut library_links) = (Vec::new(), Vec::new());

        while let Some(flag) = flags_iter.next() {
            if flag.starts_with("-l") || flag.starts_with("-L") {
                // Check if this flag has no space before the value as is
                // common with tools like pkg-config
                // e.g. -L/some/dir/local/lib or -licui18n
                let (flag, mut value) = flag.split_at(2);
                if value.is_empty() {
                    value = match flags_iter.next() {
                        Some(v) => v,
                        None => bail! {
                            "Flag in rustc-flags has no value in {}: {}",
                            whence,
                            value
                        },
                    }
                }

                match flag {
                    "-l" => library_links.push(value.to_string()),
                    "-L" => library_paths.push(PathBuf::from(value)),

                    // This was already checked above
                    _ => unreachable!(),
                };
            } else {
                bail!(
                    "Only `-l` and `-L` flags are allowed in {}: `{}`",
                    whence,
                    value
                )
            }
        }
        Ok((library_paths, library_links))
    }

    /// Parses [`cargo::rustc-env`] instruction.
    ///
    /// [`cargo::rustc-env`]: https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#rustc-env
    pub fn parse_rustc_env(value: &str, whence: &str) -> CargoResult<(String, String)> {
        match value.split_once('=') {
            Some((n, v)) => Ok((n.to_owned(), v.to_owned())),
            _ => bail!("Variable rustc-env has no value in {whence}: {value}"),
        }
    }
}

/// Prepares the Rust script for the unstable feature [metabuild].
///
/// [metabuild]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#metabuild
fn prepare_metabuild(
    build_runner: &BuildRunner<'_, '_>,
    unit: &Unit,
    deps: &[String],
) -> CargoResult<()> {
    let mut output = Vec::new();
    let available_deps = build_runner.unit_deps(unit);
    // Filter out optional dependencies, and look up the actual lib name.
    let meta_deps: Vec<_> = deps
        .iter()
        .filter_map(|name| {
            available_deps
                .iter()
                .find(|d| d.unit.pkg.name().as_str() == name.as_str())
                .map(|d| d.unit.target.crate_name())
        })
        .collect();
    output.push("fn main() {\n".to_string());
    for dep in &meta_deps {
        output.push(format!("    {}::metabuild();\n", dep));
    }
    output.push("}\n".to_string());
    let output = output.join("");
    let path = unit
        .pkg
        .manifest()
        .metabuild_path(build_runner.bcx.ws.build_dir());
    paths::create_dir_all(path.parent().unwrap())?;
    paths::write_if_changed(path, &output)?;
    Ok(())
}

impl BuildDeps {
    /// Creates a build script dependency information from a previous
    /// build script output path and the content.
    pub fn new(output_file: &Path, output: Option<&BuildOutput>) -> BuildDeps {
        BuildDeps {
            build_script_output: output_file.to_path_buf(),
            rerun_if_changed: output
                .map(|p| &p.rerun_if_changed)
                .cloned()
                .unwrap_or_default(),
            rerun_if_env_changed: output
                .map(|p| &p.rerun_if_env_changed)
                .cloned()
                .unwrap_or_default(),
        }
    }
}

/// Computes several maps in [`BuildRunner`].
///
/// - [`build_scripts`]: A map that tracks which build scripts each package
///   depends on.
/// - [`build_explicit_deps`]: Dependency statements emitted by build scripts
///   from a previous run.
/// - [`build_script_outputs`]: Pre-populates this with any overridden build
///   scripts.
///
/// The important one here is [`build_scripts`], which for each `(package,
/// metadata)` stores a [`BuildScripts`] object which contains a list of
/// dependencies with build scripts that the unit should consider when linking.
/// For example this lists all dependencies' `-L` flags which need to be
/// propagated transitively.
///
/// The given set of units to this function is the initial set of
/// targets/profiles which are being built.
///
/// [`build_scripts`]: BuildRunner::build_scripts
/// [`build_explicit_deps`]: BuildRunner::build_explicit_deps
/// [`build_script_outputs`]: BuildRunner::build_script_outputs
pub fn build_map(build_runner: &mut BuildRunner<'_, '_>) -> CargoResult<()> {
    let mut ret = HashMap::new();
    for unit in &build_runner.bcx.roots {
        build(&mut ret, build_runner, unit)?;
    }
    build_runner
        .build_scripts
        .extend(ret.into_iter().map(|(k, v)| (k, Arc::new(v))));
    return Ok(());

    // Recursive function to build up the map we're constructing. This function
    // memoizes all of its return values as it goes along.
    fn build<'a>(
        out: &'a mut HashMap<Unit, BuildScripts>,
        build_runner: &mut BuildRunner<'_, '_>,
        unit: &Unit,
    ) -> CargoResult<&'a BuildScripts> {
        // Do a quick pre-flight check to see if we've already calculated the
        // set of dependencies.
        if out.contains_key(unit) {
            return Ok(&out[unit]);
        }

        // If there is a build script override, pre-fill the build output.
        if unit.mode.is_run_custom_build() {
            if let Some(links) = unit.pkg.manifest().links() {
                if let Some(output) = unit.links_overrides.get(links) {
                    let metadata = build_runner.get_run_build_script_metadata(unit);
                    build_runner.build_script_outputs.lock().unwrap().insert(
                        unit.pkg.package_id(),
                        metadata,
                        output.clone(),
                    );
                }
            }
        }

        let mut ret = BuildScripts::default();

        // If a package has a build script, add itself as something to inspect for linking.
        if !unit.target.is_custom_build() && unit.pkg.has_custom_build() {
            let script_metas = build_runner
                .find_build_script_metadatas(unit)
                .expect("has_custom_build should have RunCustomBuild");
            for script_meta in script_metas {
                add_to_link(&mut ret, unit.pkg.package_id(), script_meta);
            }
        }

        if unit.mode.is_run_custom_build() {
            parse_previous_explicit_deps(build_runner, unit);
        }

        // We want to invoke the compiler deterministically to be cache-friendly
        // to rustc invocation caching schemes, so be sure to generate the same
        // set of build script dependency orderings via sorting the targets that
        // come out of the `Context`.
        let mut dependencies: Vec<Unit> = build_runner
            .unit_deps(unit)
            .iter()
            .map(|d| d.unit.clone())
            .collect();
        dependencies.sort_by_key(|u| u.pkg.package_id());

        for dep_unit in dependencies.iter() {
            let dep_scripts = build(out, build_runner, dep_unit)?;

            if dep_unit.target.for_host() {
                ret.plugins.extend(dep_scripts.to_link.iter().cloned());
            } else if dep_unit.target.is_linkable() {
                for &(pkg, metadata) in dep_scripts.to_link.iter() {
                    add_to_link(&mut ret, pkg, metadata);
                }
            }
        }

        match out.entry(unit.clone()) {
            Entry::Vacant(entry) => Ok(entry.insert(ret)),
            Entry::Occupied(_) => panic!("cyclic dependencies in `build_map`"),
        }
    }

    // When adding an entry to 'to_link' we only actually push it on if the
    // script hasn't seen it yet (e.g., we don't push on duplicates).
    fn add_to_link(scripts: &mut BuildScripts, pkg: PackageId, metadata: UnitHash) {
        if scripts.seen_to_link.insert((pkg, metadata)) {
            scripts.to_link.push((pkg, metadata));
        }
    }

    /// Load any dependency declarations from a previous build script run.
    fn parse_previous_explicit_deps(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) {
        let script_run_dir = build_runner.files().build_script_run_dir(unit);
        let output_file = script_run_dir.join("output");
        let (prev_output, _) = prev_build_output(build_runner, unit);
        let deps = BuildDeps::new(&output_file, prev_output.as_ref());
        build_runner.build_explicit_deps.insert(unit.clone(), deps);
    }
}

/// Returns the previous parsed `BuildOutput`, if any, from a previous
/// execution.
///
/// Also returns the directory containing the output, typically used later in
/// processing.
fn prev_build_output(
    build_runner: &mut BuildRunner<'_, '_>,
    unit: &Unit,
) -> (Option<BuildOutput>, PathBuf) {
    let script_out_dir = build_runner.files().build_script_out_dir(unit);
    let script_run_dir = build_runner.files().build_script_run_dir(unit);
    let root_output_file = script_run_dir.join("root-output");
    let output_file = script_run_dir.join("output");

    let prev_script_out_dir = paths::read_bytes(&root_output_file)
        .and_then(|bytes| paths::bytes2path(&bytes))
        .unwrap_or_else(|_| script_out_dir.clone());

    (
        BuildOutput::parse_file(
            &output_file,
            unit.pkg.library().map(|t| t.crate_name()),
            &unit.pkg.to_string(),
            &prev_script_out_dir,
            &script_out_dir,
            build_runner.bcx.gctx.nightly_features_allowed,
            unit.pkg.targets(),
            &unit.pkg.rust_version().cloned(),
        )
        .ok(),
        prev_script_out_dir,
    )
}

impl BuildScriptOutputs {
    /// Inserts a new entry into the map.
    fn insert(&mut self, pkg_id: PackageId, metadata: UnitHash, parsed_output: BuildOutput) {
        match self.outputs.entry(metadata) {
            Entry::Vacant(entry) => {
                entry.insert(parsed_output);
            }
            Entry::Occupied(entry) => panic!(
                "build script output collision for {}/{}\n\
                old={:?}\nnew={:?}",
                pkg_id,
                metadata,
                entry.get(),
                parsed_output
            ),
        }
    }

    /// Returns `true` if the given key already exists.
    fn contains_key(&self, metadata: UnitHash) -> bool {
        self.outputs.contains_key(&metadata)
    }

    /// Gets the build output for the given key.
    pub fn get(&self, meta: UnitHash) -> Option<&BuildOutput> {
        self.outputs.get(&meta)
    }

    /// Returns an iterator over all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&UnitHash, &BuildOutput)> {
        self.outputs.iter()
    }
}
