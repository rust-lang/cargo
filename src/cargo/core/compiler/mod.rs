//! # Interact with the compiler
//!
//! If you consider [`ops::cargo_compile::compile`] as a `rustc` driver but on
//! Cargo side, this module is kinda the `rustc_interface` for that merits.
//! It contains all the interaction between Cargo and the rustc compiler,
//! from preparing the context for the entire build process, to scheduling
//! and executing each unit of work (e.g. running `rustc`), to managing and
//! caching the output artifact of a build.
//!
//! However, it hasn't yet exposed a clear definition of each phase or session,
//! like what rustc has done[^1]. Also, no one knows if Cargo really needs that.
//! To be pragmatic, here we list a handful of items you may want to learn:
//!
//! * [`BuildContext`] is a static context containing all information you need
//!   before a build gets started.
//! * [`Context`] is the center of the world, coordinating a running build and
//!   collecting information from it.
//! * [`custom_build`] is the home of build script executions and output parsing.
//! * [`fingerprint`] not only defines but also executes a set of rules to
//!   determine if a re-compile is needed.
//! * [`job_queue`] is where the parallelism, job scheduling, and communication
//!   machinery happen between Cargo and the compiler.
//! * [`layout`] defines and manages output artifacts of a build in the filesystem.
//! * [`unit_dependencies`] is for building a dependency graph for compilation
//!   from a result of dependency resolution.
//! * [`Unit`] contains sufficient information to build something, usually
//!   turning into a compiler invocation in a later phase.
//!
//! [^1]: Maybe [`-Zbuild-plan`](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-plan)
//!   was designed to serve that purpose but still [in flux](https://github.com/rust-lang/cargo/issues/7614).
//!
//! [`ops::cargo_compile::compile`]: crate::ops::compile

pub mod artifact;
mod build_config;
pub(crate) mod build_context;
mod build_plan;
mod compilation;
mod compile_kind;
pub(crate) mod context;
mod crate_type;
mod custom_build;
pub(crate) mod fingerprint;
pub mod future_incompat;
pub(crate) mod job_queue;
pub(crate) mod layout;
mod links;
mod lto;
mod output_depinfo;
pub mod rustdoc;
pub mod standard_lib;
mod timings;
mod unit;
pub mod unit_dependencies;
pub mod unit_graph;

use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::fs::{self, File};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Error};
use lazycell::LazyCell;
use tracing::{debug, trace};

pub use self::build_config::{BuildConfig, CompileMode, MessageFormat, TimingOutput};
pub use self::build_context::{
    BuildContext, FileFlavor, FileType, RustDocFingerprint, RustcTargetData, TargetInfo,
};
use self::build_plan::BuildPlan;
pub use self::compilation::{Compilation, Doctest, UnitOutput};
pub use self::compile_kind::{CompileKind, CompileTarget};
pub use self::context::{Context, Metadata};
pub use self::crate_type::CrateType;
pub use self::custom_build::LinkArgTarget;
pub use self::custom_build::{BuildOutput, BuildScriptOutputs, BuildScripts};
pub(crate) use self::fingerprint::DirtyReason;
pub use self::job_queue::Freshness;
use self::job_queue::{Job, JobQueue, JobState, Work};
pub(crate) use self::layout::Layout;
pub use self::lto::Lto;
use self::output_depinfo::output_depinfo;
use self::unit_graph::UnitDep;
use crate::core::compiler::future_incompat::FutureIncompatReport;
pub use crate::core::compiler::unit::{Unit, UnitInterner};
use crate::core::manifest::TargetSourcePath;
use crate::core::profiles::{PanicStrategy, Profile, Strip};
use crate::core::{Feature, PackageId, Target, Verbosity};
use crate::util::errors::{CargoResult, VerboseError};
use crate::util::interning::InternedString;
use crate::util::machine_message::{self, Message};
use crate::util::toml::TomlDebugInfo;
use crate::util::{add_path_args, internal, iter_join_onto, profile};
use cargo_util::{paths, ProcessBuilder, ProcessError};
use rustfix::diagnostics::Applicability;

const RUSTDOC_CRATE_VERSION_FLAG: &str = "--crate-version";

/// A glorified callback for executing calls to rustc. Rather than calling rustc
/// directly, we'll use an `Executor`, giving clients an opportunity to intercept
/// the build calls.
pub trait Executor: Send + Sync + 'static {
    /// Called after a rustc process invocation is prepared up-front for a given
    /// unit of work (may still be modified for runtime-known dependencies, when
    /// the work is actually executed).
    fn init(&self, _cx: &Context<'_, '_>, _unit: &Unit) {}

    /// In case of an `Err`, Cargo will not continue with the build process for
    /// this package.
    fn exec(
        &self,
        cmd: &ProcessBuilder,
        id: PackageId,
        target: &Target,
        mode: CompileMode,
        on_stdout_line: &mut dyn FnMut(&str) -> CargoResult<()>,
        on_stderr_line: &mut dyn FnMut(&str) -> CargoResult<()>,
    ) -> CargoResult<()>;

    /// Queried when queuing each unit of work. If it returns true, then the
    /// unit will always be rebuilt, independent of whether it needs to be.
    fn force_rebuild(&self, _unit: &Unit) -> bool {
        false
    }
}

/// A `DefaultExecutor` calls rustc without doing anything else. It is Cargo's
/// default behaviour.
#[derive(Copy, Clone)]
pub struct DefaultExecutor;

impl Executor for DefaultExecutor {
    fn exec(
        &self,
        cmd: &ProcessBuilder,
        _id: PackageId,
        _target: &Target,
        _mode: CompileMode,
        on_stdout_line: &mut dyn FnMut(&str) -> CargoResult<()>,
        on_stderr_line: &mut dyn FnMut(&str) -> CargoResult<()>,
    ) -> CargoResult<()> {
        cmd.exec_with_streaming(on_stdout_line, on_stderr_line, false)
            .map(drop)
    }
}

/// Builds up and enqueue a list of pending jobs onto the `job` queue.
///
/// Starting from the `unit`, this function recursively calls itself to build
/// all jobs for dependencies of the `unit`. Each of these jobs represents
/// compiling a particular package.
///
/// Note that **no actual work is executed as part of this**, that's all done
/// next as part of [`JobQueue::execute`] function which will run everything
/// in order with proper parallelism.
fn compile<'cfg>(
    cx: &mut Context<'_, 'cfg>,
    jobs: &mut JobQueue<'cfg>,
    plan: &mut BuildPlan,
    unit: &Unit,
    exec: &Arc<dyn Executor>,
    force_rebuild: bool,
) -> CargoResult<()> {
    let bcx = cx.bcx;
    let build_plan = bcx.build_config.build_plan;
    if !cx.compiled.insert(unit.clone()) {
        return Ok(());
    }

    // Build up the work to be done to compile this unit, enqueuing it once
    // we've got everything constructed.
    let p = profile::start(format!("preparing: {}/{}", unit.pkg, unit.target.name()));
    fingerprint::prepare_init(cx, unit)?;

    let job = if unit.mode.is_run_custom_build() {
        custom_build::prepare(cx, unit)?
    } else if unit.mode.is_doc_test() {
        // We run these targets later, so this is just a no-op for now.
        Job::new_fresh()
    } else if build_plan {
        Job::new_dirty(rustc(cx, unit, &exec.clone())?, None)
    } else {
        let force = exec.force_rebuild(unit) || force_rebuild;
        let mut job = fingerprint::prepare_target(cx, unit, force)?;
        job.before(if job.freshness().is_dirty() {
            let work = if unit.mode.is_doc() || unit.mode.is_doc_scrape() {
                rustdoc(cx, unit)?
            } else {
                rustc(cx, unit, exec)?
            };
            work.then(link_targets(cx, unit, false)?)
        } else {
            // We always replay the output cache,
            // since it might contain future-incompat-report messages
            let work = replay_output_cache(
                unit.pkg.package_id(),
                PathBuf::from(unit.pkg.manifest_path()),
                &unit.target,
                cx.files().message_cache_path(unit),
                cx.bcx.build_config.message_format,
                unit.show_warnings(bcx.config),
            );
            // Need to link targets on both the dirty and fresh.
            work.then(link_targets(cx, unit, true)?)
        });

        job
    };
    jobs.enqueue(cx, unit, job)?;
    drop(p);

    // Be sure to compile all dependencies of this target as well.
    let deps = Vec::from(cx.unit_deps(unit)); // Create vec due to mutable borrow.
    for dep in deps {
        compile(cx, jobs, plan, &dep.unit, exec, false)?;
    }
    if build_plan {
        plan.add(cx, unit)?;
    }

    Ok(())
}

/// Generates the warning message used when fallible doc-scrape units fail,
/// either for rustdoc or rustc.
fn make_failed_scrape_diagnostic(
    cx: &Context<'_, '_>,
    unit: &Unit,
    top_line: impl Display,
) -> String {
    let manifest_path = unit.pkg.manifest_path();
    let relative_manifest_path = manifest_path
        .strip_prefix(cx.bcx.ws.root())
        .unwrap_or(&manifest_path);

    format!(
        "\
{top_line}
    Try running with `--verbose` to see the error message.
    If an example should not be scanned, then consider adding `doc-scrape-examples = false` to its `[[example]]` definition in {}",
        relative_manifest_path.display()
    )
}

/// Creates a unit of work invoking `rustc` for building the `unit`.
fn rustc(cx: &mut Context<'_, '_>, unit: &Unit, exec: &Arc<dyn Executor>) -> CargoResult<Work> {
    let mut rustc = prepare_rustc(cx, unit)?;
    let build_plan = cx.bcx.build_config.build_plan;

    let name = unit.pkg.name();
    let buildkey = unit.buildkey();

    let outputs = cx.outputs(unit)?;
    let root = cx.files().out_dir(unit);

    // Prepare the native lib state (extra `-L` and `-l` flags).
    let build_script_outputs = Arc::clone(&cx.build_script_outputs);
    let current_id = unit.pkg.package_id();
    let manifest_path = PathBuf::from(unit.pkg.manifest_path());
    let build_scripts = cx.build_scripts.get(unit).cloned();

    // If we are a binary and the package also contains a library, then we
    // don't pass the `-l` flags.
    let pass_l_flag = unit.target.is_lib() || !unit.pkg.targets().iter().any(|t| t.is_lib());

    let dep_info_name = if cx.files().use_extra_filename(unit) {
        format!(
            "{}-{}.d",
            unit.target.crate_name(),
            cx.files().metadata(unit)
        )
    } else {
        format!("{}.d", unit.target.crate_name())
    };
    let rustc_dep_info_loc = root.join(dep_info_name);
    let dep_info_loc = fingerprint::dep_info_loc(cx, unit);

    let mut output_options = OutputOptions::new(cx, unit);
    let package_id = unit.pkg.package_id();
    let target = Target::clone(&unit.target);
    let mode = unit.mode;

    exec.init(cx, unit);
    let exec = exec.clone();

    let root_output = cx.files().host_dest().to_path_buf();
    let target_dir = cx.bcx.ws.target_dir().into_path_unlocked();
    let pkg_root = unit.pkg.root().to_path_buf();
    let cwd = rustc
        .get_cwd()
        .unwrap_or_else(|| cx.bcx.config.cwd())
        .to_path_buf();
    let fingerprint_dir = cx.files().fingerprint_dir(unit);
    let script_metadata = cx.find_build_script_metadata(unit);
    let is_local = unit.is_local();
    let artifact = unit.artifact;

    let hide_diagnostics_for_scrape_unit = cx.bcx.unit_can_fail_for_docscraping(unit)
        && !matches!(cx.bcx.config.shell().verbosity(), Verbosity::Verbose);
    let failed_scrape_diagnostic = hide_diagnostics_for_scrape_unit.then(|| {
        // If this unit is needed for doc-scraping, then we generate a diagnostic that
        // describes the set of reverse-dependencies that cause the unit to be needed.
        let target_desc = unit.target.description_named();
        let mut for_scrape_units = cx
            .bcx
            .scrape_units_have_dep_on(unit)
            .into_iter()
            .map(|unit| unit.target.description_named())
            .collect::<Vec<_>>();
        for_scrape_units.sort();
        let for_scrape_units = for_scrape_units.join(", ");
        make_failed_scrape_diagnostic(cx, unit, format_args!("failed to check {target_desc} in package `{name}` as a prerequisite for scraping examples from: {for_scrape_units}"))
    });
    if hide_diagnostics_for_scrape_unit {
        output_options.show_diagnostics = false;
    }

    return Ok(Work::new(move |state| {
        // Artifacts are in a different location than typical units,
        // hence we must assure the crate- and target-dependent
        // directory is present.
        if artifact.is_true() {
            paths::create_dir_all(&root)?;
        }

        // Only at runtime have we discovered what the extra -L and -l
        // arguments are for native libraries, so we process those here. We
        // also need to be sure to add any -L paths for our plugins to the
        // dynamic library load path as a plugin's dynamic library may be
        // located somewhere in there.
        // Finally, if custom environment variables have been produced by
        // previous build scripts, we include them in the rustc invocation.
        if let Some(build_scripts) = build_scripts {
            let script_outputs = build_script_outputs.lock().unwrap();
            if !build_plan {
                add_native_deps(
                    &mut rustc,
                    &script_outputs,
                    &build_scripts,
                    pass_l_flag,
                    &target,
                    current_id,
                )?;
                add_plugin_deps(&mut rustc, &script_outputs, &build_scripts, &root_output)?;
            }
            add_custom_flags(&mut rustc, &script_outputs, script_metadata)?;
        }

        for output in outputs.iter() {
            // If there is both an rmeta and rlib, rustc will prefer to use the
            // rlib, even if it is older. Therefore, we must delete the rlib to
            // force using the new rmeta.
            if output.path.extension() == Some(OsStr::new("rmeta")) {
                let dst = root.join(&output.path).with_extension("rlib");
                if dst.exists() {
                    paths::remove_file(&dst)?;
                }
            }

            // Some linkers do not remove the executable, but truncate and modify it.
            // That results in the old hard-link being modified even after renamed.
            // We delete the old artifact here to prevent this behavior from confusing users.
            // See rust-lang/cargo#8348.
            if output.hardlink.is_some() && output.path.exists() {
                _ = paths::remove_file(&output.path).map_err(|e| {
                    tracing::debug!(
                        "failed to delete previous output file `{:?}`: {e:?}",
                        output.path
                    );
                });
            }
        }

        fn verbose_if_simple_exit_code(err: Error) -> Error {
            // If a signal on unix (`code == None`) or an abnormal termination
            // on Windows (codes like `0xC0000409`), don't hide the error details.
            match err
                .downcast_ref::<ProcessError>()
                .as_ref()
                .and_then(|perr| perr.code)
            {
                Some(n) if cargo_util::is_simple_exit_code(n) => VerboseError::new(err).into(),
                _ => err,
            }
        }

        state.running(&rustc);
        let timestamp = paths::set_invocation_time(&fingerprint_dir)?;
        if build_plan {
            state.build_plan(buildkey, rustc.clone(), outputs.clone());
        } else {
            let result = exec
                .exec(
                    &rustc,
                    package_id,
                    &target,
                    mode,
                    &mut |line| on_stdout_line(state, line, package_id, &target),
                    &mut |line| {
                        on_stderr_line(
                            state,
                            line,
                            package_id,
                            &manifest_path,
                            &target,
                            &mut output_options,
                        )
                    },
                )
                .map_err(verbose_if_simple_exit_code)
                .with_context(|| {
                    // adapted from rustc_errors/src/lib.rs
                    let warnings = match output_options.warnings_seen {
                        0 => String::new(),
                        1 => "; 1 warning emitted".to_string(),
                        count => format!("; {} warnings emitted", count),
                    };
                    let errors = match output_options.errors_seen {
                        0 => String::new(),
                        1 => " due to previous error".to_string(),
                        count => format!(" due to {} previous errors", count),
                    };
                    let name = descriptive_pkg_name(&name, &target, &mode);
                    format!("could not compile {name}{errors}{warnings}")
                });

            if let Err(e) = result {
                if let Some(diagnostic) = failed_scrape_diagnostic {
                    state.warning(diagnostic)?;
                }

                return Err(e);
            }

            // Exec should never return with success *and* generate an error.
            debug_assert_eq!(output_options.errors_seen, 0);
        }

        if rustc_dep_info_loc.exists() {
            fingerprint::translate_dep_info(
                &rustc_dep_info_loc,
                &dep_info_loc,
                &cwd,
                &pkg_root,
                &target_dir,
                &rustc,
                // Do not track source files in the fingerprint for registry dependencies.
                is_local,
            )
            .with_context(|| {
                internal(format!(
                    "could not parse/generate dep info at: {}",
                    rustc_dep_info_loc.display()
                ))
            })?;
            // This mtime shift allows Cargo to detect if a source file was
            // modified in the middle of the build.
            paths::set_file_time_no_err(dep_info_loc, timestamp);
        }

        Ok(())
    }));

    // Add all relevant `-L` and `-l` flags from dependencies (now calculated and
    // present in `state`) to the command provided.
    fn add_native_deps(
        rustc: &mut ProcessBuilder,
        build_script_outputs: &BuildScriptOutputs,
        build_scripts: &BuildScripts,
        pass_l_flag: bool,
        target: &Target,
        current_id: PackageId,
    ) -> CargoResult<()> {
        for key in build_scripts.to_link.iter() {
            let output = build_script_outputs.get(key.1).ok_or_else(|| {
                internal(format!(
                    "couldn't find build script output for {}/{}",
                    key.0, key.1
                ))
            })?;
            for path in output.library_paths.iter() {
                rustc.arg("-L").arg(path);
            }

            if key.0 == current_id {
                if pass_l_flag {
                    for name in output.library_links.iter() {
                        rustc.arg("-l").arg(name);
                    }
                }
            }

            for (lt, arg) in &output.linker_args {
                // There was an unintentional change where cdylibs were
                // allowed to be passed via transitive dependencies. This
                // clause should have been kept in the `if` block above. For
                // now, continue allowing it for cdylib only.
                // See https://github.com/rust-lang/cargo/issues/9562
                if lt.applies_to(target) && (key.0 == current_id || *lt == LinkArgTarget::Cdylib) {
                    rustc.arg("-C").arg(format!("link-arg={}", arg));
                }
            }
        }
        Ok(())
    }
}

/// Link the compiled target (often of form `foo-{metadata_hash}`) to the
/// final target. This must happen during both "Fresh" and "Compile".
fn link_targets(cx: &mut Context<'_, '_>, unit: &Unit, fresh: bool) -> CargoResult<Work> {
    let bcx = cx.bcx;
    let outputs = cx.outputs(unit)?;
    let export_dir = cx.files().export_dir();
    let package_id = unit.pkg.package_id();
    let manifest_path = PathBuf::from(unit.pkg.manifest_path());
    let profile = unit.profile.clone();
    let unit_mode = unit.mode;
    let features = unit.features.iter().map(|s| s.to_string()).collect();
    let json_messages = bcx.build_config.emit_json();
    let executable = cx.get_executable(unit)?;
    let mut target = Target::clone(&unit.target);
    if let TargetSourcePath::Metabuild = target.src_path() {
        // Give it something to serialize.
        let path = unit.pkg.manifest().metabuild_path(cx.bcx.ws.target_dir());
        target.set_src_path(TargetSourcePath::Path(path));
    }

    Ok(Work::new(move |state| {
        // If we're a "root crate", e.g., the target of this compilation, then we
        // hard link our outputs out of the `deps` directory into the directory
        // above. This means that `cargo build` will produce binaries in
        // `target/debug` which one probably expects.
        let mut destinations = vec![];
        for output in outputs.iter() {
            let src = &output.path;
            // This may have been a `cargo rustc` command which changes the
            // output, so the source may not actually exist.
            if !src.exists() {
                continue;
            }
            let Some(dst) = output.hardlink.as_ref() else {
                destinations.push(src.clone());
                continue;
            };
            destinations.push(dst.clone());
            paths::link_or_copy(src, dst)?;
            if let Some(ref path) = output.export_path {
                let export_dir = export_dir.as_ref().unwrap();
                paths::create_dir_all(export_dir)?;

                paths::link_or_copy(src, path)?;
            }
        }

        if json_messages {
            let debuginfo = match profile.debuginfo.into_inner() {
                TomlDebugInfo::None => machine_message::ArtifactDebuginfo::Int(0),
                TomlDebugInfo::Limited => machine_message::ArtifactDebuginfo::Int(1),
                TomlDebugInfo::Full => machine_message::ArtifactDebuginfo::Int(2),
                TomlDebugInfo::LineDirectivesOnly => {
                    machine_message::ArtifactDebuginfo::Named("line-directives-only")
                }
                TomlDebugInfo::LineTablesOnly => {
                    machine_message::ArtifactDebuginfo::Named("line-tables-only")
                }
            };
            let art_profile = machine_message::ArtifactProfile {
                opt_level: profile.opt_level.as_str(),
                debuginfo: Some(debuginfo),
                debug_assertions: profile.debug_assertions,
                overflow_checks: profile.overflow_checks,
                test: unit_mode.is_any_test(),
            };

            let msg = machine_message::Artifact {
                package_id,
                manifest_path,
                target: &target,
                profile: art_profile,
                features,
                filenames: destinations,
                executable,
                fresh,
            }
            .to_json_string();
            state.stdout(msg)?;
        }
        Ok(())
    }))
}

// For all plugin dependencies, add their -L paths (now calculated and present
// in `build_script_outputs`) to the dynamic library load path for the command
// to execute.
fn add_plugin_deps(
    rustc: &mut ProcessBuilder,
    build_script_outputs: &BuildScriptOutputs,
    build_scripts: &BuildScripts,
    root_output: &Path,
) -> CargoResult<()> {
    let var = paths::dylib_path_envvar();
    let search_path = rustc.get_env(var).unwrap_or_default();
    let mut search_path = env::split_paths(&search_path).collect::<Vec<_>>();
    for (pkg_id, metadata) in &build_scripts.plugins {
        let output = build_script_outputs
            .get(*metadata)
            .ok_or_else(|| internal(format!("couldn't find libs for plugin dep {}", pkg_id)))?;
        search_path.append(&mut filter_dynamic_search_path(
            output.library_paths.iter(),
            root_output,
        ));
    }
    let search_path = paths::join_paths(&search_path, var)?;
    rustc.env(var, &search_path);
    Ok(())
}

// Determine paths to add to the dynamic search path from -L entries
//
// Strip off prefixes like "native=" or "framework=" and filter out directories
// **not** inside our output directory since they are likely spurious and can cause
// clashes with system shared libraries (issue #3366).
fn filter_dynamic_search_path<'a, I>(paths: I, root_output: &Path) -> Vec<PathBuf>
where
    I: Iterator<Item = &'a PathBuf>,
{
    let mut search_path = vec![];
    for dir in paths {
        let dir = match dir.to_str().and_then(|s| s.split_once("=")) {
            Some(("native" | "crate" | "dependency" | "framework" | "all", path)) => path.into(),
            _ => dir.clone(),
        };
        if dir.starts_with(&root_output) {
            search_path.push(dir);
        } else {
            debug!(
                "Not including path {} in runtime library search path because it is \
                 outside target root {}",
                dir.display(),
                root_output.display()
            );
        }
    }
    search_path
}

/// Prepares flags and environments we can compute for a `rustc` invocation
/// before the job queue starts compiling any unit.
///
/// This builds a static view of the invocation. Flags depending on the
/// completion of other units will be added later in runtime, such as flags
/// from build scripts.
fn prepare_rustc(cx: &Context<'_, '_>, unit: &Unit) -> CargoResult<ProcessBuilder> {
    let is_primary = cx.is_primary_package(unit);
    let is_workspace = cx.bcx.ws.is_member(&unit.pkg);

    let mut base = cx
        .compilation
        .rustc_process(unit, is_primary, is_workspace)?;

    if is_primary {
        base.env("CARGO_PRIMARY_PACKAGE", "1");
    }

    if unit.target.is_test() || unit.target.is_bench() {
        let tmp = cx.files().layout(unit.kind).prepare_tmp()?;
        base.env("CARGO_TARGET_TMPDIR", tmp.display().to_string());
    }

    base.inherit_jobserver(&cx.jobserver);
    build_base_args(cx, &mut base, unit)?;
    build_deps_args(&mut base, cx, unit)?;
    add_cap_lints(cx.bcx, unit, &mut base);
    base.args(cx.bcx.rustflags_args(unit));
    if cx.bcx.config.cli_unstable().binary_dep_depinfo {
        base.arg("-Z").arg("binary-dep-depinfo");
    }
    Ok(base)
}

/// Prepares flags and environments we can compute for a `rustdoc` invocation
/// before the job queue starts compiling any unit.
///
/// This builds a static view of the invocation. Flags depending on the
/// completion of other units will be added later in runtime, such as flags
/// from build scripts.
fn prepare_rustdoc(cx: &Context<'_, '_>, unit: &Unit) -> CargoResult<ProcessBuilder> {
    let bcx = cx.bcx;
    // script_metadata is not needed here, it is only for tests.
    let mut rustdoc = cx.compilation.rustdoc_process(unit, None)?;
    rustdoc.inherit_jobserver(&cx.jobserver);
    let crate_name = unit.target.crate_name();
    rustdoc.arg("--crate-name").arg(&crate_name);
    add_path_args(bcx.ws, unit, &mut rustdoc);
    add_cap_lints(bcx, unit, &mut rustdoc);

    if let CompileKind::Target(target) = unit.kind {
        rustdoc.arg("--target").arg(target.rustc_target());
    }
    let doc_dir = cx.files().out_dir(unit);
    rustdoc.arg("-o").arg(&doc_dir);
    rustdoc.args(&features_args(unit));
    rustdoc.args(&check_cfg_args(cx, unit));

    add_error_format_and_color(cx, &mut rustdoc);
    add_allow_features(cx, &mut rustdoc);

    rustdoc.args(unit.pkg.manifest().lint_rustflags());
    if let Some(args) = cx.bcx.extra_args_for(unit) {
        rustdoc.args(args);
    }

    let metadata = cx.metadata_for_doc_units[unit];
    rustdoc.arg("-C").arg(format!("metadata={}", metadata));

    if unit.mode.is_doc_scrape() {
        debug_assert!(cx.bcx.scrape_units.contains(unit));

        if unit.target.is_test() {
            rustdoc.arg("--scrape-tests");
        }

        rustdoc.arg("-Zunstable-options");

        rustdoc
            .arg("--scrape-examples-output-path")
            .arg(scrape_output_path(cx, unit)?);

        // Only scrape example for items from crates in the workspace, to reduce generated file size
        for pkg in cx.bcx.ws.members() {
            let names = pkg
                .targets()
                .iter()
                .map(|target| target.crate_name())
                .collect::<HashSet<_>>();
            for name in names {
                rustdoc.arg("--scrape-examples-target-crate").arg(name);
            }
        }
    }

    if should_include_scrape_units(cx.bcx, unit) {
        rustdoc.arg("-Zunstable-options");
    }

    build_deps_args(&mut rustdoc, cx, unit)?;
    rustdoc::add_root_urls(cx, unit, &mut rustdoc)?;

    rustdoc.args(bcx.rustdocflags_args(unit));

    if !crate_version_flag_already_present(&rustdoc) {
        append_crate_version_flag(unit, &mut rustdoc);
    }

    Ok(rustdoc)
}

/// Creates a unit of work invoking `rustdoc` for documenting the `unit`.
fn rustdoc(cx: &mut Context<'_, '_>, unit: &Unit) -> CargoResult<Work> {
    let mut rustdoc = prepare_rustdoc(cx, unit)?;

    let crate_name = unit.target.crate_name();
    let doc_dir = cx.files().out_dir(unit);
    // Create the documentation directory ahead of time as rustdoc currently has
    // a bug where concurrent invocations will race to create this directory if
    // it doesn't already exist.
    paths::create_dir_all(&doc_dir)?;

    let target_desc = unit.target.description_named();
    let name = unit.pkg.name();
    let build_script_outputs = Arc::clone(&cx.build_script_outputs);
    let package_id = unit.pkg.package_id();
    let manifest_path = PathBuf::from(unit.pkg.manifest_path());
    let target = Target::clone(&unit.target);
    let mut output_options = OutputOptions::new(cx, unit);
    let script_metadata = cx.find_build_script_metadata(unit);
    let scrape_outputs = if should_include_scrape_units(cx.bcx, unit) {
        Some(
            cx.bcx
                .scrape_units
                .iter()
                .map(|unit| Ok((cx.files().metadata(unit), scrape_output_path(cx, unit)?)))
                .collect::<CargoResult<HashMap<_, _>>>()?,
        )
    } else {
        None
    };

    let failed_scrape_units = Arc::clone(&cx.failed_scrape_units);
    let hide_diagnostics_for_scrape_unit = cx.bcx.unit_can_fail_for_docscraping(unit)
        && !matches!(cx.bcx.config.shell().verbosity(), Verbosity::Verbose);
    let failed_scrape_diagnostic = hide_diagnostics_for_scrape_unit.then(|| {
        make_failed_scrape_diagnostic(
            cx,
            unit,
            format_args!("failed to scan {target_desc} in package `{name}` for example code usage"),
        )
    });
    if hide_diagnostics_for_scrape_unit {
        output_options.show_diagnostics = false;
    }

    Ok(Work::new(move |state| {
        add_custom_flags(
            &mut rustdoc,
            &build_script_outputs.lock().unwrap(),
            script_metadata,
        )?;

        // Add the output of scraped examples to the rustdoc command.
        // This action must happen after the unit's dependencies have finished,
        // because some of those deps may be Docscrape units which have failed.
        // So we dynamically determine which `--with-examples` flags to pass here.
        if let Some(scrape_outputs) = scrape_outputs {
            let failed_scrape_units = failed_scrape_units.lock().unwrap();
            for (metadata, output_path) in &scrape_outputs {
                if !failed_scrape_units.contains(metadata) {
                    rustdoc.arg("--with-examples").arg(output_path);
                }
            }
        }

        let crate_dir = doc_dir.join(&crate_name);
        if crate_dir.exists() {
            // Remove output from a previous build. This ensures that stale
            // files for removed items are removed.
            debug!("removing pre-existing doc directory {:?}", crate_dir);
            paths::remove_dir_all(crate_dir)?;
        }
        state.running(&rustdoc);

        let result = rustdoc
            .exec_with_streaming(
                &mut |line| on_stdout_line(state, line, package_id, &target),
                &mut |line| {
                    on_stderr_line(
                        state,
                        line,
                        package_id,
                        &manifest_path,
                        &target,
                        &mut output_options,
                    )
                },
                false,
            )
            .with_context(|| format!("could not document `{}`", name));

        if let Err(e) = result {
            if let Some(diagnostic) = failed_scrape_diagnostic {
                state.warning(diagnostic)?;
            }

            return Err(e);
        }

        Ok(())
    }))
}

// The --crate-version flag could have already been passed in RUSTDOCFLAGS
// or as an extra compiler argument for rustdoc
fn crate_version_flag_already_present(rustdoc: &ProcessBuilder) -> bool {
    rustdoc.get_args().any(|flag| {
        flag.to_str()
            .map_or(false, |flag| flag.starts_with(RUSTDOC_CRATE_VERSION_FLAG))
    })
}

fn append_crate_version_flag(unit: &Unit, rustdoc: &mut ProcessBuilder) {
    rustdoc
        .arg(RUSTDOC_CRATE_VERSION_FLAG)
        .arg(unit.pkg.version().to_string());
}

/// Adds [`--cap-lints`] to the command to execute.
///
/// [`--cap-lints`]: https://doc.rust-lang.org/nightly/rustc/lints/levels.html#capping-lints
fn add_cap_lints(bcx: &BuildContext<'_, '_>, unit: &Unit, cmd: &mut ProcessBuilder) {
    // If this is an upstream dep we don't want warnings from, turn off all
    // lints.
    if !unit.show_warnings(bcx.config) {
        cmd.arg("--cap-lints").arg("allow");

    // If this is an upstream dep but we *do* want warnings, make sure that they
    // don't fail compilation.
    } else if !unit.is_local() {
        cmd.arg("--cap-lints").arg("warn");
    }
}

/// Forwards [`-Zallow-features`] if it is set for cargo.
///
/// [`-Zallow-features`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#allow-features
fn add_allow_features(cx: &Context<'_, '_>, cmd: &mut ProcessBuilder) {
    if let Some(allow) = &cx.bcx.config.cli_unstable().allow_features {
        let mut arg = String::from("-Zallow-features=");
        let _ = iter_join_onto(&mut arg, allow, ",");
        cmd.arg(&arg);
    }
}

/// Adds [`--error-format`] to the command to execute.
///
/// Cargo always uses JSON output. This has several benefits, such as being
/// easier to parse, handles changing formats (for replaying cached messages),
/// ensures atomic output (so messages aren't interleaved), allows for
/// intercepting messages like rmeta artifacts, etc. rustc includes a
/// "rendered" field in the JSON message with the message properly formatted,
/// which Cargo will extract and display to the user.
///
/// [`--error-format`]: https://doc.rust-lang.org/nightly/rustc/command-line-arguments.html#--error-format-control-how-errors-are-produced
fn add_error_format_and_color(cx: &Context<'_, '_>, cmd: &mut ProcessBuilder) {
    cmd.arg("--error-format=json");
    let mut json = String::from("--json=diagnostic-rendered-ansi,artifacts,future-incompat");

    match cx.bcx.build_config.message_format {
        MessageFormat::Short | MessageFormat::Json { short: true, .. } => {
            json.push_str(",diagnostic-short");
        }
        _ => {}
    }
    cmd.arg(json);

    let config = cx.bcx.config;
    if let Some(width) = config.shell().err_width().diagnostic_terminal_width() {
        cmd.arg(format!("--diagnostic-width={width}"));
    }
}

/// Adds essential rustc flags and environment variables to the command to execute.
fn build_base_args(cx: &Context<'_, '_>, cmd: &mut ProcessBuilder, unit: &Unit) -> CargoResult<()> {
    assert!(!unit.mode.is_run_custom_build());

    let bcx = cx.bcx;
    let Profile {
        ref opt_level,
        codegen_backend,
        codegen_units,
        debuginfo,
        debug_assertions,
        split_debuginfo,
        overflow_checks,
        rpath,
        ref panic,
        incremental,
        strip,
        rustflags: profile_rustflags,
        ..
    } = unit.profile.clone();
    let test = unit.mode.is_any_test();

    cmd.arg("--crate-name").arg(&unit.target.crate_name());

    let edition = unit.target.edition();
    edition.cmd_edition_arg(cmd);

    add_path_args(bcx.ws, unit, cmd);
    add_error_format_and_color(cx, cmd);
    add_allow_features(cx, cmd);

    let mut contains_dy_lib = false;
    if !test {
        for crate_type in &unit.target.rustc_crate_types() {
            cmd.arg("--crate-type").arg(crate_type.as_str());
            contains_dy_lib |= crate_type == &CrateType::Dylib;
        }
    }

    if unit.mode.is_check() {
        cmd.arg("--emit=dep-info,metadata");
    } else if !unit.requires_upstream_objects() {
        // Always produce metadata files for rlib outputs. Metadata may be used
        // in this session for a pipelined compilation, or it may be used in a
        // future Cargo session as part of a pipelined compile.
        cmd.arg("--emit=dep-info,metadata,link");
    } else {
        cmd.arg("--emit=dep-info,link");
    }

    let prefer_dynamic = (unit.target.for_host() && !unit.target.is_custom_build())
        || (contains_dy_lib && !cx.is_primary_package(unit));
    if prefer_dynamic {
        cmd.arg("-C").arg("prefer-dynamic");
    }

    if opt_level.as_str() != "0" {
        cmd.arg("-C").arg(&format!("opt-level={}", opt_level));
    }

    if *panic != PanicStrategy::Unwind {
        cmd.arg("-C").arg(format!("panic={}", panic));
    }

    cmd.args(&lto_args(cx, unit));

    if let Some(backend) = codegen_backend {
        cmd.arg("-Z").arg(&format!("codegen-backend={}", backend));
    }

    if let Some(n) = codegen_units {
        cmd.arg("-C").arg(&format!("codegen-units={}", n));
    }

    let debuginfo = debuginfo.into_inner();
    // Shorten the number of arguments if possible.
    if debuginfo != TomlDebugInfo::None {
        cmd.arg("-C").arg(format!("debuginfo={debuginfo}"));
        // This is generally just an optimization on build time so if we don't
        // pass it then it's ok. The values for the flag (off, packed, unpacked)
        // may be supported or not depending on the platform, so availability is
        // checked per-value. For example, at the time of writing this code, on
        // Windows the only stable valid value for split-debuginfo is "packed",
        // while on Linux "unpacked" is also stable.
        if let Some(split) = split_debuginfo {
            if cx
                .bcx
                .target_data
                .info(unit.kind)
                .supports_debuginfo_split(split)
            {
                cmd.arg("-C").arg(format!("split-debuginfo={split}"));
            }
        }
    }

    cmd.args(unit.pkg.manifest().lint_rustflags());
    cmd.args(&profile_rustflags);
    if let Some(args) = cx.bcx.extra_args_for(unit) {
        cmd.args(args);
    }

    // `-C overflow-checks` is implied by the setting of `-C debug-assertions`,
    // so we only need to provide `-C overflow-checks` if it differs from
    // the value of `-C debug-assertions` we would provide.
    if opt_level.as_str() != "0" {
        if debug_assertions {
            cmd.args(&["-C", "debug-assertions=on"]);
            if !overflow_checks {
                cmd.args(&["-C", "overflow-checks=off"]);
            }
        } else if overflow_checks {
            cmd.args(&["-C", "overflow-checks=on"]);
        }
    } else if !debug_assertions {
        cmd.args(&["-C", "debug-assertions=off"]);
        if overflow_checks {
            cmd.args(&["-C", "overflow-checks=on"]);
        }
    } else if !overflow_checks {
        cmd.args(&["-C", "overflow-checks=off"]);
    }

    if test && unit.target.harness() {
        cmd.arg("--test");

        // Cargo has historically never compiled `--test` binaries with
        // `panic=abort` because the `test` crate itself didn't support it.
        // Support is now upstream, however, but requires an unstable flag to be
        // passed when compiling the test. We require, in Cargo, an unstable
        // flag to pass to rustc, so register that here. Eventually this flag
        // will simply not be needed when the behavior is stabilized in the Rust
        // compiler itself.
        if *panic == PanicStrategy::Abort {
            cmd.arg("-Z").arg("panic-abort-tests");
        }
    } else if test {
        cmd.arg("--cfg").arg("test");
    }

    cmd.args(&features_args(unit));
    cmd.args(&check_cfg_args(cx, unit));

    let meta = cx.files().metadata(unit);
    cmd.arg("-C").arg(&format!("metadata={}", meta));
    if cx.files().use_extra_filename(unit) {
        cmd.arg("-C").arg(&format!("extra-filename=-{}", meta));
    }

    if rpath {
        cmd.arg("-C").arg("rpath");
    }

    cmd.arg("--out-dir").arg(&cx.files().out_dir(unit));

    fn opt(cmd: &mut ProcessBuilder, key: &str, prefix: &str, val: Option<&OsStr>) {
        if let Some(val) = val {
            let mut joined = OsString::from(prefix);
            joined.push(val);
            cmd.arg(key).arg(joined);
        }
    }

    if let CompileKind::Target(n) = unit.kind {
        cmd.arg("--target").arg(n.rustc_target());
    }

    opt(
        cmd,
        "-C",
        "linker=",
        cx.compilation
            .target_linker(unit.kind)
            .as_ref()
            .map(|s| s.as_ref()),
    );
    if incremental {
        let dir = cx.files().layout(unit.kind).incremental().as_os_str();
        opt(cmd, "-C", "incremental=", Some(dir));
    }

    if strip != Strip::None {
        cmd.arg("-C").arg(format!("strip={}", strip));
    }

    if unit.is_std {
        // -Zforce-unstable-if-unmarked prevents the accidental use of
        // unstable crates within the sysroot (such as "extern crate libc" or
        // any non-public crate in the sysroot).
        //
        // RUSTC_BOOTSTRAP allows unstable features on stable.
        cmd.arg("-Z")
            .arg("force-unstable-if-unmarked")
            .env("RUSTC_BOOTSTRAP", "1");
    }

    // Add `CARGO_BIN_EXE_` environment variables for building tests.
    if unit.target.is_test() || unit.target.is_bench() {
        for bin_target in unit
            .pkg
            .manifest()
            .targets()
            .iter()
            .filter(|target| target.is_bin())
        {
            let exe_path = cx
                .files()
                .bin_link_for_target(bin_target, unit.kind, cx.bcx)?;
            let name = bin_target
                .binary_filename()
                .unwrap_or(bin_target.name().to_string());
            let key = format!("CARGO_BIN_EXE_{}", name);
            cmd.env(&key, exe_path);
        }
    }
    Ok(())
}

/// All active features for the unit passed as `--cfg features=<feature-name>`.
fn features_args(unit: &Unit) -> Vec<OsString> {
    let mut args = Vec::with_capacity(unit.features.len() * 2);

    for feat in &unit.features {
        args.push(OsString::from("--cfg"));
        args.push(OsString::from(format!("feature=\"{}\"", feat)));
    }

    args
}

/// Generates the `--check-cfg` arguments for the `unit`.
/// See unstable feature [`check-cfg`].
///
/// [`check-cfg`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#check-cfg
fn check_cfg_args(cx: &Context<'_, '_>, unit: &Unit) -> Vec<OsString> {
    if cx.bcx.config.cli_unstable().check_cfg {
        // This generate something like this:
        //  - cfg(feature, values())
        //  - cfg(feature, values("foo", "bar"))
        //
        // NOTE: Despite only explicitly specifying `feature`, well known names and values
        // are implicitly enabled when one or more `--check-cfg` argument is passed.

        let gross_cap_estimation = unit.pkg.summary().features().len() * 7 + 25;
        let mut arg_feature = OsString::with_capacity(gross_cap_estimation);
        arg_feature.push("cfg(feature, values(");
        for (i, feature) in unit.pkg.summary().features().keys().enumerate() {
            if i != 0 {
                arg_feature.push(", ");
            }
            arg_feature.push("\"");
            arg_feature.push(feature);
            arg_feature.push("\"");
        }
        arg_feature.push("))");

        vec![
            OsString::from("-Zunstable-options"),
            OsString::from("--check-cfg"),
            arg_feature,
        ]
    } else {
        Vec::new()
    }
}

/// Adds LTO related codegen flags.
fn lto_args(cx: &Context<'_, '_>, unit: &Unit) -> Vec<OsString> {
    let mut result = Vec::new();
    let mut push = |arg: &str| {
        result.push(OsString::from("-C"));
        result.push(OsString::from(arg));
    };
    match cx.lto[unit] {
        lto::Lto::Run(None) => push("lto"),
        lto::Lto::Run(Some(s)) => push(&format!("lto={}", s)),
        lto::Lto::Off => {
            push("lto=off");
            push("embed-bitcode=no");
        }
        lto::Lto::ObjectAndBitcode => {} // this is rustc's default
        lto::Lto::OnlyBitcode => push("linker-plugin-lto"),
        lto::Lto::OnlyObject => push("embed-bitcode=no"),
    }
    result
}

/// Adds dependency-relevant rustc flags and environment variables
/// to the command to execute, such as [`-L`] and [`--extern`].
///
/// [`-L`]: https://doc.rust-lang.org/nightly/rustc/command-line-arguments.html#-l-add-a-directory-to-the-library-search-path
/// [`--extern`]: https://doc.rust-lang.org/nightly/rustc/command-line-arguments.html#--extern-specify-where-an-external-library-is-located
fn build_deps_args(cmd: &mut ProcessBuilder, cx: &Context<'_, '_>, unit: &Unit) -> CargoResult<()> {
    let bcx = cx.bcx;
    cmd.arg("-L").arg(&{
        let mut deps = OsString::from("dependency=");
        deps.push(cx.files().deps_dir(unit));
        deps
    });

    // Be sure that the host path is also listed. This'll ensure that proc macro
    // dependencies are correctly found (for reexported macros).
    if !unit.kind.is_host() {
        cmd.arg("-L").arg(&{
            let mut deps = OsString::from("dependency=");
            deps.push(cx.files().host_deps());
            deps
        });
    }

    let deps = cx.unit_deps(unit);

    // If there is not one linkable target but should, rustc fails later
    // on if there is an `extern crate` for it. This may turn into a hard
    // error in the future (see PR #4797).
    if !deps
        .iter()
        .any(|dep| !dep.unit.mode.is_doc() && dep.unit.target.is_linkable())
    {
        if let Some(dep) = deps.iter().find(|dep| {
            !dep.unit.mode.is_doc() && dep.unit.target.is_lib() && !dep.unit.artifact.is_true()
        }) {
            bcx.config.shell().warn(format!(
                "The package `{}` \
                 provides no linkable target. The compiler might raise an error while compiling \
                 `{}`. Consider adding 'dylib' or 'rlib' to key `crate-type` in `{}`'s \
                 Cargo.toml. This warning might turn into a hard error in the future.",
                dep.unit.target.crate_name(),
                unit.target.crate_name(),
                dep.unit.target.crate_name()
            ))?;
        }
    }

    let mut unstable_opts = false;

    for dep in deps {
        if dep.unit.mode.is_run_custom_build() {
            cmd.env("OUT_DIR", &cx.files().build_script_out_dir(&dep.unit));
        }
    }

    for arg in extern_args(cx, unit, &mut unstable_opts)? {
        cmd.arg(arg);
    }

    for (var, env) in artifact::get_env(cx, deps)? {
        cmd.env(&var, env);
    }

    // This will only be set if we're already using a feature
    // requiring nightly rust
    if unstable_opts {
        cmd.arg("-Z").arg("unstable-options");
    }

    Ok(())
}

/// Adds extra rustc flags and environment variables collected from the output
/// of a build-script to the command to execute, include custom environment
/// variables and `cfg`.
fn add_custom_flags(
    cmd: &mut ProcessBuilder,
    build_script_outputs: &BuildScriptOutputs,
    metadata: Option<Metadata>,
) -> CargoResult<()> {
    if let Some(metadata) = metadata {
        if let Some(output) = build_script_outputs.get(metadata) {
            for cfg in output.cfgs.iter() {
                cmd.arg("--cfg").arg(cfg);
            }
            if !output.check_cfgs.is_empty() {
                cmd.arg("-Zunstable-options");
                for check_cfg in &output.check_cfgs {
                    cmd.arg("--check-cfg").arg(check_cfg);
                }
            }
            for (name, value) in output.env.iter() {
                cmd.env(name, value);
            }
        }
    }

    Ok(())
}

/// Generates a list of `--extern` arguments.
pub fn extern_args(
    cx: &Context<'_, '_>,
    unit: &Unit,
    unstable_opts: &mut bool,
) -> CargoResult<Vec<OsString>> {
    let mut result = Vec::new();
    let deps = cx.unit_deps(unit);

    // Closure to add one dependency to `result`.
    let mut link_to =
        |dep: &UnitDep, extern_crate_name: InternedString, noprelude: bool| -> CargoResult<()> {
            let mut value = OsString::new();
            let mut opts = Vec::new();
            if unit
                .pkg
                .manifest()
                .unstable_features()
                .require(Feature::public_dependency())
                .is_ok()
                && !dep.public
            {
                opts.push("priv");
                *unstable_opts = true;
            }
            if noprelude {
                opts.push("noprelude");
                *unstable_opts = true;
            }
            if !opts.is_empty() {
                value.push(opts.join(","));
                value.push(":");
            }
            value.push(extern_crate_name.as_str());
            value.push("=");

            let mut pass = |file| {
                let mut value = value.clone();
                value.push(file);
                result.push(OsString::from("--extern"));
                result.push(value);
            };

            let outputs = cx.outputs(&dep.unit)?;

            if cx.only_requires_rmeta(unit, &dep.unit) || dep.unit.mode.is_check() {
                // Example: rlib dependency for an rlib, rmeta is all that is required.
                let output = outputs
                    .iter()
                    .find(|output| output.flavor == FileFlavor::Rmeta)
                    .expect("failed to find rmeta dep for pipelined dep");
                pass(&output.path);
            } else {
                // Example: a bin needs `rlib` for dependencies, it cannot use rmeta.
                for output in outputs.iter() {
                    if output.flavor == FileFlavor::Linkable {
                        pass(&output.path);
                    }
                }
            }
            Ok(())
        };

    for dep in deps {
        if dep.unit.target.is_linkable() && !dep.unit.mode.is_doc() {
            link_to(dep, dep.extern_crate_name, dep.noprelude)?;
        }
    }
    if unit.target.proc_macro() {
        // Automatically import `proc_macro`.
        result.push(OsString::from("--extern"));
        result.push(OsString::from("proc_macro"));
    }

    Ok(result)
}

fn envify(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_uppercase())
        .map(|c| if c == '-' { '_' } else { c })
        .collect()
}

/// Configuration of the display of messages emitted by the compiler,
/// e.g. diagnostics, warnings, errors, and message caching.
struct OutputOptions {
    /// What format we're emitting from Cargo itself.
    format: MessageFormat,
    /// Where to write the JSON messages to support playback later if the unit
    /// is fresh. The file is created lazily so that in the normal case, lots
    /// of empty files are not created. If this is None, the output will not
    /// be cached (such as when replaying cached messages).
    cache_cell: Option<(PathBuf, LazyCell<File>)>,
    /// If `true`, display any diagnostics.
    /// Other types of JSON messages are processed regardless
    /// of the value of this flag.
    ///
    /// This is used primarily for cache replay. If you build with `-vv`, the
    /// cache will be filled with diagnostics from dependencies. When the
    /// cache is replayed without `-vv`, we don't want to show them.
    show_diagnostics: bool,
    /// Tracks the number of warnings we've seen so far.
    warnings_seen: usize,
    /// Tracks the number of errors we've seen so far.
    errors_seen: usize,
}

impl OutputOptions {
    fn new(cx: &Context<'_, '_>, unit: &Unit) -> OutputOptions {
        let path = cx.files().message_cache_path(unit);
        // Remove old cache, ignore ENOENT, which is the common case.
        drop(fs::remove_file(&path));
        let cache_cell = Some((path, LazyCell::new()));
        OutputOptions {
            format: cx.bcx.build_config.message_format,
            cache_cell,
            show_diagnostics: true,
            warnings_seen: 0,
            errors_seen: 0,
        }
    }
}

fn on_stdout_line(
    state: &JobState<'_, '_>,
    line: &str,
    _package_id: PackageId,
    _target: &Target,
) -> CargoResult<()> {
    state.stdout(line.to_string())?;
    Ok(())
}

fn on_stderr_line(
    state: &JobState<'_, '_>,
    line: &str,
    package_id: PackageId,
    manifest_path: &std::path::Path,
    target: &Target,
    options: &mut OutputOptions,
) -> CargoResult<()> {
    if on_stderr_line_inner(state, line, package_id, manifest_path, target, options)? {
        // Check if caching is enabled.
        if let Some((path, cell)) = &mut options.cache_cell {
            // Cache the output, which will be replayed later when Fresh.
            let f = cell.try_borrow_mut_with(|| paths::create(path))?;
            debug_assert!(!line.contains('\n'));
            f.write_all(line.as_bytes())?;
            f.write_all(&[b'\n'])?;
        }
    }
    Ok(())
}

/// Returns true if the line should be cached.
fn on_stderr_line_inner(
    state: &JobState<'_, '_>,
    line: &str,
    package_id: PackageId,
    manifest_path: &std::path::Path,
    target: &Target,
    options: &mut OutputOptions,
) -> CargoResult<bool> {
    // We primarily want to use this function to process JSON messages from
    // rustc. The compiler should always print one JSON message per line, and
    // otherwise it may have other output intermingled (think RUST_LOG or
    // something like that), so skip over everything that doesn't look like a
    // JSON message.
    if !line.starts_with('{') {
        state.stderr(line.to_string())?;
        return Ok(true);
    }

    let mut compiler_message: Box<serde_json::value::RawValue> = match serde_json::from_str(line) {
        Ok(msg) => msg,

        // If the compiler produced a line that started with `{` but it wasn't
        // valid JSON, maybe it wasn't JSON in the first place! Forward it along
        // to stderr.
        Err(e) => {
            debug!("failed to parse json: {:?}", e);
            state.stderr(line.to_string())?;
            return Ok(true);
        }
    };

    let count_diagnostic = |level, options: &mut OutputOptions| {
        if level == "warning" {
            options.warnings_seen += 1;
        } else if level == "error" {
            options.errors_seen += 1;
        }
    };

    if let Ok(report) = serde_json::from_str::<FutureIncompatReport>(compiler_message.get()) {
        for item in &report.future_incompat_report {
            count_diagnostic(&*item.diagnostic.level, options);
        }
        state.future_incompat_report(report.future_incompat_report);
        return Ok(true);
    }

    // Depending on what we're emitting from Cargo itself, we figure out what to
    // do with this JSON message.
    match options.format {
        // In the "human" output formats (human/short) or if diagnostic messages
        // from rustc aren't being included in the output of Cargo's JSON
        // messages then we extract the diagnostic (if present) here and handle
        // it ourselves.
        MessageFormat::Human
        | MessageFormat::Short
        | MessageFormat::Json {
            render_diagnostics: true,
            ..
        } => {
            #[derive(serde::Deserialize)]
            struct CompilerMessage {
                rendered: String,
                message: String,
                level: String,
                children: Vec<PartialDiagnostic>,
            }

            // A partial rustfix::diagnostics::Diagnostic. We deserialize only a
            // subset of the fields because rustc's output can be extremely
            // deeply nested JSON in pathological cases involving macro
            // expansion. Rustfix's Diagnostic struct is recursive containing a
            // field `children: Vec<Self>`, and it can cause deserialization to
            // hit serde_json's default recursion limit, or overflow the stack
            // if we turn that off. Cargo only cares about the 1 field listed
            // here.
            #[derive(serde::Deserialize)]
            struct PartialDiagnostic {
                spans: Vec<PartialDiagnosticSpan>,
            }

            // A partial rustfix::diagnostics::DiagnosticSpan.
            #[derive(serde::Deserialize)]
            struct PartialDiagnosticSpan {
                suggestion_applicability: Option<Applicability>,
            }

            if let Ok(mut msg) = serde_json::from_str::<CompilerMessage>(compiler_message.get()) {
                if msg.message.starts_with("aborting due to")
                    || msg.message.ends_with("warning emitted")
                    || msg.message.ends_with("warnings emitted")
                {
                    // Skip this line; we'll print our own summary at the end.
                    return Ok(true);
                }
                // state.stderr will add a newline
                if msg.rendered.ends_with('\n') {
                    msg.rendered.pop();
                }
                let rendered = msg.rendered;
                if options.show_diagnostics {
                    let machine_applicable: bool = msg
                        .children
                        .iter()
                        .map(|child| {
                            child
                                .spans
                                .iter()
                                .filter_map(|span| span.suggestion_applicability)
                                .any(|app| app == Applicability::MachineApplicable)
                        })
                        .any(|b| b);
                    count_diagnostic(&msg.level, options);
                    state.emit_diag(msg.level, rendered, machine_applicable)?;
                }
                return Ok(true);
            }
        }

        // Remove color information from the rendered string if color is not
        // enabled. Cargo always asks for ANSI colors from rustc. This allows
        // cached replay to enable/disable colors without re-invoking rustc.
        MessageFormat::Json { ansi: false, .. } => {
            #[derive(serde::Deserialize, serde::Serialize)]
            struct CompilerMessage {
                rendered: String,
                #[serde(flatten)]
                other: std::collections::BTreeMap<String, serde_json::Value>,
            }
            if let Ok(mut error) = serde_json::from_str::<CompilerMessage>(compiler_message.get()) {
                error.rendered = anstream::adapter::strip_str(&error.rendered).to_string();
                let new_line = serde_json::to_string(&error)?;
                let new_msg: Box<serde_json::value::RawValue> = serde_json::from_str(&new_line)?;
                compiler_message = new_msg;
            }
        }

        // If ansi colors are desired then we should be good to go! We can just
        // pass through this message as-is.
        MessageFormat::Json { ansi: true, .. } => {}
    }

    // We always tell rustc to emit messages about artifacts being produced.
    // These messages feed into pipelined compilation, as well as timing
    // information.
    //
    // Look for a matching directive and inform Cargo internally that a
    // metadata file has been produced.
    #[derive(serde::Deserialize)]
    struct ArtifactNotification {
        artifact: String,
    }

    if let Ok(artifact) = serde_json::from_str::<ArtifactNotification>(compiler_message.get()) {
        trace!("found directive from rustc: `{}`", artifact.artifact);
        if artifact.artifact.ends_with(".rmeta") {
            debug!("looks like metadata finished early!");
            state.rmeta_produced();
        }
        return Ok(false);
    }

    // And failing all that above we should have a legitimate JSON diagnostic
    // from the compiler, so wrap it in an external Cargo JSON message
    // indicating which package it came from and then emit it.

    if !options.show_diagnostics {
        return Ok(true);
    }

    #[derive(serde::Deserialize)]
    struct CompilerMessage {
        level: String,
    }
    if let Ok(message) = serde_json::from_str::<CompilerMessage>(compiler_message.get()) {
        count_diagnostic(&message.level, options);
    }

    let msg = machine_message::FromCompiler {
        package_id,
        manifest_path,
        target,
        message: compiler_message,
    }
    .to_json_string();

    // Switch json lines from rustc/rustdoc that appear on stderr to stdout
    // instead. We want the stdout of Cargo to always be machine parseable as
    // stderr has our colorized human-readable messages.
    state.stdout(msg)?;
    Ok(true)
}

/// Creates a unit of work that replays the cached compiler message.
///
/// Usually used when a job is fresh and doesn't need to recompile.
fn replay_output_cache(
    package_id: PackageId,
    manifest_path: PathBuf,
    target: &Target,
    path: PathBuf,
    format: MessageFormat,
    show_diagnostics: bool,
) -> Work {
    let target = target.clone();
    let mut options = OutputOptions {
        format,
        cache_cell: None,
        show_diagnostics,
        warnings_seen: 0,
        errors_seen: 0,
    };
    Work::new(move |state| {
        if !path.exists() {
            // No cached output, probably didn't emit anything.
            return Ok(());
        }
        // We sometimes have gigabytes of output from the compiler, so avoid
        // loading it all into memory at once, as that can cause OOM where
        // otherwise there would be none.
        let file = paths::open(&path)?;
        let mut reader = std::io::BufReader::new(file);
        let mut line = String::new();
        loop {
            let length = reader.read_line(&mut line)?;
            if length == 0 {
                break;
            }
            let trimmed = line.trim_end_matches(&['\n', '\r'][..]);
            on_stderr_line(
                state,
                trimmed,
                package_id,
                &manifest_path,
                &target,
                &mut options,
            )?;
            line.clear();
        }
        Ok(())
    })
}

/// Provides a package name with descriptive target information,
/// e.g., '`foo` (bin "bar" test)', '`foo` (lib doctest)'.
fn descriptive_pkg_name(name: &str, target: &Target, mode: &CompileMode) -> String {
    let desc_name = target.description_named();
    let mode = if mode.is_rustc_test() && !(target.is_test() || target.is_bench()) {
        " test"
    } else if mode.is_doc_test() {
        " doctest"
    } else if mode.is_doc() {
        " doc"
    } else {
        ""
    };
    format!("`{name}` ({desc_name}{mode})")
}

/// Applies environment variables from config `[env]` to [`ProcessBuilder`].
fn apply_env_config(config: &crate::Config, cmd: &mut ProcessBuilder) -> CargoResult<()> {
    for (key, value) in config.env_config()?.iter() {
        // never override a value that has already been set by cargo
        if cmd.get_envs().contains_key(key) {
            continue;
        }

        if value.is_force() || config.get_env_os(key).is_none() {
            cmd.env(key, value.resolve(config));
        }
    }
    Ok(())
}

/// Checks if there are some scrape units waiting to be processed.
fn should_include_scrape_units(bcx: &BuildContext<'_, '_>, unit: &Unit) -> bool {
    unit.mode.is_doc() && bcx.scrape_units.len() > 0 && bcx.ws.unit_needs_doc_scrape(unit)
}

/// Gets the file path of function call information output from `rustdoc`.
fn scrape_output_path(cx: &Context<'_, '_>, unit: &Unit) -> CargoResult<PathBuf> {
    assert!(unit.mode.is_doc() || unit.mode.is_doc_scrape());
    cx.outputs(unit).map(|outputs| outputs[0].path.clone())
}
