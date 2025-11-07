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
//! like what rustc has done. Also, no one knows if Cargo really needs that.
//! To be pragmatic, here we list a handful of items you may want to learn:
//!
//! * [`BuildContext`] is a static context containing all information you need
//!   before a build gets started.
//! * [`BuildRunner`] is the center of the world, coordinating a running build and
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
//! [`ops::cargo_compile::compile`]: crate::ops::compile

pub mod artifact;
mod build_config;
pub(crate) mod build_context;
pub(crate) mod build_runner;
mod compilation;
mod compile_kind;
mod crate_type;
mod custom_build;
pub(crate) mod fingerprint;
pub mod future_incompat;
pub(crate) mod job_queue;
pub(crate) mod layout;
mod links;
mod lto;
mod output_depinfo;
mod output_sbom;
pub mod rustdoc;
pub mod standard_lib;
mod timings;
mod unit;
pub mod unit_dependencies;
pub mod unit_graph;

use std::borrow::Cow;
use std::cell::OnceCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::fs::{self, File};
use std::io::{BufRead, BufWriter, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};
use anyhow::{Context as _, Error};
use cargo_platform::{Cfg, Platform};
use itertools::Itertools;
use regex::Regex;
use tracing::{debug, instrument, trace};

pub use self::build_config::UserIntent;
pub use self::build_config::{BuildConfig, CompileMode, MessageFormat, TimingOutput};
pub use self::build_context::{
    BuildContext, FileFlavor, FileType, RustDocFingerprint, RustcTargetData, TargetInfo,
};
pub use self::build_runner::{BuildRunner, Metadata, UnitHash};
pub use self::compilation::{Compilation, Doctest, UnitOutput};
pub use self::compile_kind::{CompileKind, CompileKindFallback, CompileTarget};
pub use self::crate_type::CrateType;
pub use self::custom_build::LinkArgTarget;
pub use self::custom_build::{BuildOutput, BuildScriptOutputs, BuildScripts, LibraryPath};
pub(crate) use self::fingerprint::DirtyReason;
pub use self::job_queue::Freshness;
use self::job_queue::{Job, JobQueue, JobState, Work};
pub(crate) use self::layout::Layout;
pub use self::lto::Lto;
use self::output_depinfo::output_depinfo;
use self::output_sbom::build_sbom;
use self::unit_graph::UnitDep;
use crate::core::compiler::future_incompat::FutureIncompatReport;
use crate::core::compiler::timings::SectionTiming;
pub use crate::core::compiler::unit::{Unit, UnitInterner};
use crate::core::manifest::TargetSourcePath;
use crate::core::profiles::{PanicStrategy, Profile, StripInner};
use crate::core::{Feature, PackageId, Target, Verbosity};
use crate::util::OnceExt;
use crate::util::context::WarningHandling;
use crate::util::errors::{CargoResult, VerboseError};
use crate::util::interning::InternedString;
use crate::util::lints::get_key_value;
use crate::util::machine_message::{self, Message};
use crate::util::{add_path_args, internal, path_args};
use cargo_util::{ProcessBuilder, ProcessError, paths};
use cargo_util_schemas::manifest::TomlDebugInfo;
use cargo_util_schemas::manifest::TomlTrimPaths;
use cargo_util_schemas::manifest::TomlTrimPathsValue;
use rustfix::diagnostics::Applicability;
pub(crate) use timings::CompilationSection;

const RUSTDOC_CRATE_VERSION_FLAG: &str = "--crate-version";

/// A glorified callback for executing calls to rustc. Rather than calling rustc
/// directly, we'll use an `Executor`, giving clients an opportunity to intercept
/// the build calls.
pub trait Executor: Send + Sync + 'static {
    /// Called after a rustc process invocation is prepared up-front for a given
    /// unit of work (may still be modified for runtime-known dependencies, when
    /// the work is actually executed).
    fn init(&self, _build_runner: &BuildRunner<'_, '_>, _unit: &Unit) {}

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
    #[instrument(name = "rustc", skip_all, fields(package = id.name().as_str(), process = cmd.to_string()))]
    fn exec(
        &self,
        cmd: &ProcessBuilder,
        id: PackageId,
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
#[tracing::instrument(skip(build_runner, jobs, exec))]
fn compile<'gctx>(
    build_runner: &mut BuildRunner<'_, 'gctx>,
    jobs: &mut JobQueue<'gctx>,
    unit: &Unit,
    exec: &Arc<dyn Executor>,
    force_rebuild: bool,
) -> CargoResult<()> {
    let bcx = build_runner.bcx;
    if !build_runner.compiled.insert(unit.clone()) {
        return Ok(());
    }

    // If we are in `--compile-time-deps` and the given unit is not a compile time
    // dependency, skip compiling the unit and jumps to dependencies, which still
    // have chances to be compile time dependencies
    if !unit.skip_non_compile_time_dep {
        // Build up the work to be done to compile this unit, enqueuing it once
        // we've got everything constructed.
        fingerprint::prepare_init(build_runner, unit)?;

        let job = if unit.mode.is_run_custom_build() {
            custom_build::prepare(build_runner, unit)?
        } else if unit.mode.is_doc_test() {
            // We run these targets later, so this is just a no-op for now.
            Job::new_fresh()
        } else {
            let force = exec.force_rebuild(unit) || force_rebuild;
            let mut job = fingerprint::prepare_target(build_runner, unit, force)?;
            job.before(if job.freshness().is_dirty() {
                let work = if unit.mode.is_doc() || unit.mode.is_doc_scrape() {
                    rustdoc(build_runner, unit)?
                } else {
                    rustc(build_runner, unit, exec)?
                };
                work.then(link_targets(build_runner, unit, false)?)
            } else {
                // We always replay the output cache,
                // since it might contain future-incompat-report messages
                let show_diagnostics = unit.show_warnings(bcx.gctx)
                    && build_runner.bcx.gctx.warning_handling()? != WarningHandling::Allow;
                let manifest = ManifestErrorContext::new(build_runner, unit);
                let work = replay_output_cache(
                    unit.pkg.package_id(),
                    manifest,
                    &unit.target,
                    build_runner.files().message_cache_path(unit),
                    build_runner.bcx.build_config.message_format,
                    show_diagnostics,
                );
                // Need to link targets on both the dirty and fresh.
                work.then(link_targets(build_runner, unit, true)?)
            });

            job
        };
        jobs.enqueue(build_runner, unit, job)?;
    }

    // Be sure to compile all dependencies of this target as well.
    let deps = Vec::from(build_runner.unit_deps(unit)); // Create vec due to mutable borrow.
    for dep in deps {
        compile(build_runner, jobs, &dep.unit, exec, false)?;
    }

    Ok(())
}

/// Generates the warning message used when fallible doc-scrape units fail,
/// either for rustdoc or rustc.
fn make_failed_scrape_diagnostic(
    build_runner: &BuildRunner<'_, '_>,
    unit: &Unit,
    top_line: impl Display,
) -> String {
    let manifest_path = unit.pkg.manifest_path();
    let relative_manifest_path = manifest_path
        .strip_prefix(build_runner.bcx.ws.root())
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
fn rustc(
    build_runner: &mut BuildRunner<'_, '_>,
    unit: &Unit,
    exec: &Arc<dyn Executor>,
) -> CargoResult<Work> {
    let mut rustc = prepare_rustc(build_runner, unit)?;

    let name = unit.pkg.name();

    let outputs = build_runner.outputs(unit)?;
    let root = build_runner.files().out_dir(unit);

    // Prepare the native lib state (extra `-L` and `-l` flags).
    let build_script_outputs = Arc::clone(&build_runner.build_script_outputs);
    let current_id = unit.pkg.package_id();
    let manifest = ManifestErrorContext::new(build_runner, unit);
    let build_scripts = build_runner.build_scripts.get(unit).cloned();

    // If we are a binary and the package also contains a library, then we
    // don't pass the `-l` flags.
    let pass_l_flag = unit.target.is_lib() || !unit.pkg.targets().iter().any(|t| t.is_lib());

    let dep_info_name =
        if let Some(c_extra_filename) = build_runner.files().metadata(unit).c_extra_filename() {
            format!("{}-{}.d", unit.target.crate_name(), c_extra_filename)
        } else {
            format!("{}.d", unit.target.crate_name())
        };
    let rustc_dep_info_loc = root.join(dep_info_name);
    let dep_info_loc = fingerprint::dep_info_loc(build_runner, unit);

    let mut output_options = OutputOptions::new(build_runner, unit);
    let package_id = unit.pkg.package_id();
    let target = Target::clone(&unit.target);
    let mode = unit.mode;

    exec.init(build_runner, unit);
    let exec = exec.clone();

    let root_output = build_runner.files().host_dest().to_path_buf();
    let build_dir = build_runner.bcx.ws.build_dir().into_path_unlocked();
    let pkg_root = unit.pkg.root().to_path_buf();
    let cwd = rustc
        .get_cwd()
        .unwrap_or_else(|| build_runner.bcx.gctx.cwd())
        .to_path_buf();
    let fingerprint_dir = build_runner.files().fingerprint_dir(unit);
    let script_metadatas = build_runner.find_build_script_metadatas(unit);
    let is_local = unit.is_local();
    let artifact = unit.artifact;
    let sbom_files = build_runner.sbom_output_files(unit)?;
    let sbom = build_sbom(build_runner, unit)?;

    let hide_diagnostics_for_scrape_unit = build_runner.bcx.unit_can_fail_for_docscraping(unit)
        && !matches!(
            build_runner.bcx.gctx.shell().verbosity(),
            Verbosity::Verbose
        );
    let failed_scrape_diagnostic = hide_diagnostics_for_scrape_unit.then(|| {
        // If this unit is needed for doc-scraping, then we generate a diagnostic that
        // describes the set of reverse-dependencies that cause the unit to be needed.
        let target_desc = unit.target.description_named();
        let mut for_scrape_units = build_runner
            .bcx
            .scrape_units_have_dep_on(unit)
            .into_iter()
            .map(|unit| unit.target.description_named())
            .collect::<Vec<_>>();
        for_scrape_units.sort();
        let for_scrape_units = for_scrape_units.join(", ");
        make_failed_scrape_diagnostic(build_runner, unit, format_args!("failed to check {target_desc} in package `{name}` as a prerequisite for scraping examples from: {for_scrape_units}"))
    });
    if hide_diagnostics_for_scrape_unit {
        output_options.show_diagnostics = false;
    }
    let env_config = Arc::clone(build_runner.bcx.gctx.env_config()?);
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
            add_native_deps(
                &mut rustc,
                &script_outputs,
                &build_scripts,
                pass_l_flag,
                &target,
                current_id,
                mode,
            )?;
            add_plugin_deps(&mut rustc, &script_outputs, &build_scripts, &root_output)?;
            add_custom_flags(&mut rustc, &script_outputs, script_metadatas)?;
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

        state.running(&rustc);
        let timestamp = paths::set_invocation_time(&fingerprint_dir)?;
        for file in sbom_files {
            tracing::debug!("writing sbom to {}", file.display());
            let outfile = BufWriter::new(paths::create(&file)?);
            serde_json::to_writer(outfile, &sbom)?;
        }

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
                        &manifest,
                        &target,
                        &mut output_options,
                    )
                },
            )
            .map_err(|e| {
                if output_options.errors_seen == 0 {
                    // If we didn't expect an error, do not require --verbose to fail.
                    // This is intended to debug
                    // https://github.com/rust-lang/crater/issues/733, where we are seeing
                    // Cargo exit unsuccessfully while seeming to not show any errors.
                    e
                } else {
                    verbose_if_simple_exit_code(e)
                }
            })
            .with_context(|| {
                // adapted from rustc_errors/src/lib.rs
                let warnings = match output_options.warnings_seen {
                    0 => String::new(),
                    1 => "; 1 warning emitted".to_string(),
                    count => format!("; {} warnings emitted", count),
                };
                let errors = match output_options.errors_seen {
                    0 => String::new(),
                    1 => " due to 1 previous error".to_string(),
                    count => format!(" due to {} previous errors", count),
                };
                let name = descriptive_pkg_name(&name, &target, &mode);
                format!("could not compile {name}{errors}{warnings}")
            });

        if let Err(e) = result {
            if let Some(diagnostic) = failed_scrape_diagnostic {
                state.warning(diagnostic);
            }

            return Err(e);
        }

        // Exec should never return with success *and* generate an error.
        debug_assert_eq!(output_options.errors_seen, 0);

        if rustc_dep_info_loc.exists() {
            fingerprint::translate_dep_info(
                &rustc_dep_info_loc,
                &dep_info_loc,
                &cwd,
                &pkg_root,
                &build_dir,
                &rustc,
                // Do not track source files in the fingerprint for registry dependencies.
                is_local,
                &env_config,
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
        mode: CompileMode,
    ) -> CargoResult<()> {
        let mut library_paths = vec![];

        for key in build_scripts.to_link.iter() {
            let output = build_script_outputs.get(key.1).ok_or_else(|| {
                internal(format!(
                    "couldn't find build script output for {}/{}",
                    key.0, key.1
                ))
            })?;
            library_paths.extend(output.library_paths.iter());
        }

        // NOTE: This very intentionally does not use the derived ord from LibraryPath because we need to
        // retain relative ordering within the same type (i.e. not lexicographic). The use of a stable sort
        // is also important here because it ensures that paths of the same type retain the same relative
        // ordering (for an unstable sort to work here, the list would need to retain the idx of each element
        // and then sort by that idx when the type is equivalent.
        library_paths.sort_by_key(|p| match p {
            LibraryPath::CargoArtifact(_) => 0,
            LibraryPath::External(_) => 1,
        });

        for path in library_paths.iter() {
            rustc.arg("-L").arg(path.as_ref());
        }

        for key in build_scripts.to_link.iter() {
            let output = build_script_outputs.get(key.1).ok_or_else(|| {
                internal(format!(
                    "couldn't find build script output for {}/{}",
                    key.0, key.1
                ))
            })?;

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
                if lt.applies_to(target, mode)
                    && (key.0 == current_id || *lt == LinkArgTarget::Cdylib)
                {
                    rustc.arg("-C").arg(format!("link-arg={}", arg));
                }
            }
        }
        Ok(())
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

/// Link the compiled target (often of form `foo-{metadata_hash}`) to the
/// final target. This must happen during both "Fresh" and "Compile".
fn link_targets(
    build_runner: &mut BuildRunner<'_, '_>,
    unit: &Unit,
    fresh: bool,
) -> CargoResult<Work> {
    let bcx = build_runner.bcx;
    let outputs = build_runner.outputs(unit)?;
    let export_dir = build_runner.files().export_dir();
    let package_id = unit.pkg.package_id();
    let manifest_path = PathBuf::from(unit.pkg.manifest_path());
    let profile = unit.profile.clone();
    let unit_mode = unit.mode;
    let features = unit.features.iter().map(|s| s.to_string()).collect();
    let json_messages = bcx.build_config.emit_json();
    let executable = build_runner.get_executable(unit)?;
    let mut target = Target::clone(&unit.target);
    if let TargetSourcePath::Metabuild = target.src_path() {
        // Give it something to serialize.
        let path = unit
            .pkg
            .manifest()
            .metabuild_path(build_runner.bcx.ws.build_dir());
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
                package_id: package_id.to_spec(),
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
            output.library_paths.iter().map(AsRef::as_ref),
            root_output,
        ));
    }
    let search_path = paths::join_paths(&search_path, var)?;
    rustc.env(var, &search_path);
    Ok(())
}

fn get_dynamic_search_path(path: &Path) -> &Path {
    match path.to_str().and_then(|s| s.split_once("=")) {
        Some(("native" | "crate" | "dependency" | "framework" | "all", path)) => Path::new(path),
        _ => path,
    }
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
        let dir = get_dynamic_search_path(dir);
        if dir.starts_with(&root_output) {
            search_path.push(dir.to_path_buf());
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
fn prepare_rustc(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<ProcessBuilder> {
    let gctx = build_runner.bcx.gctx;
    let is_primary = build_runner.is_primary_package(unit);
    let is_workspace = build_runner.bcx.ws.is_member(&unit.pkg);

    let mut base = build_runner
        .compilation
        .rustc_process(unit, is_primary, is_workspace)?;
    build_base_args(build_runner, &mut base, unit)?;
    if unit.pkg.manifest().is_embedded() {
        if !gctx.cli_unstable().script {
            anyhow::bail!(
                "parsing `{}` requires `-Zscript`",
                unit.pkg.manifest_path().display()
            );
        }
        base.arg("-Z").arg("crate-attr=feature(frontmatter)");
    }

    base.inherit_jobserver(&build_runner.jobserver);
    build_deps_args(&mut base, build_runner, unit)?;
    add_cap_lints(build_runner.bcx, unit, &mut base);
    if let Some(args) = build_runner.bcx.extra_args_for(unit) {
        base.args(args);
    }
    base.args(&unit.rustflags);
    if gctx.cli_unstable().binary_dep_depinfo {
        base.arg("-Z").arg("binary-dep-depinfo");
    }
    if build_runner.bcx.gctx.cli_unstable().checksum_freshness {
        base.arg("-Z").arg("checksum-hash-algorithm=blake3");
    }

    if is_primary {
        base.env("CARGO_PRIMARY_PACKAGE", "1");
        let file_list = std::env::join_paths(build_runner.sbom_output_files(unit)?)?;
        base.env("CARGO_SBOM_PATH", file_list);
    }

    if unit.target.is_test() || unit.target.is_bench() {
        let tmp = build_runner
            .files()
            .layout(unit.kind)
            .build_dir()
            .prepare_tmp()?;
        base.env("CARGO_TARGET_TMPDIR", tmp.display().to_string());
    }

    Ok(base)
}

/// Prepares flags and environments we can compute for a `rustdoc` invocation
/// before the job queue starts compiling any unit.
///
/// This builds a static view of the invocation. Flags depending on the
/// completion of other units will be added later in runtime, such as flags
/// from build scripts.
fn prepare_rustdoc(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<ProcessBuilder> {
    let bcx = build_runner.bcx;
    // script_metadata is not needed here, it is only for tests.
    let mut rustdoc = build_runner.compilation.rustdoc_process(unit, None)?;
    if unit.pkg.manifest().is_embedded() {
        if !bcx.gctx.cli_unstable().script {
            anyhow::bail!(
                "parsing `{}` requires `-Zscript`",
                unit.pkg.manifest_path().display()
            );
        }
        rustdoc.arg("-Z").arg("crate-attr=feature(frontmatter)");
    }
    rustdoc.inherit_jobserver(&build_runner.jobserver);
    let crate_name = unit.target.crate_name();
    rustdoc.arg("--crate-name").arg(&crate_name);
    add_path_args(bcx.ws, unit, &mut rustdoc);
    add_cap_lints(bcx, unit, &mut rustdoc);

    if let CompileKind::Target(target) = unit.kind {
        rustdoc.arg("--target").arg(target.rustc_target());
    }
    let doc_dir = build_runner.files().out_dir(unit);
    rustdoc.arg("-o").arg(&doc_dir);
    rustdoc.args(&features_args(unit));
    rustdoc.args(&check_cfg_args(unit));

    add_error_format_and_color(build_runner, &mut rustdoc);
    add_allow_features(build_runner, &mut rustdoc);

    if build_runner.bcx.gctx.cli_unstable().rustdoc_depinfo {
        // toolchain-shared-resources is required for keeping the shared styling resources
        // invocation-specific is required for keeping the original rustdoc emission
        let mut arg =
            OsString::from("--emit=toolchain-shared-resources,invocation-specific,dep-info=");
        arg.push(rustdoc_dep_info_loc(build_runner, unit));
        rustdoc.arg(arg);

        if build_runner.bcx.gctx.cli_unstable().checksum_freshness {
            rustdoc.arg("-Z").arg("checksum-hash-algorithm=blake3");
        }

        rustdoc.arg("-Zunstable-options");
    }

    if let Some(trim_paths) = unit.profile.trim_paths.as_ref() {
        trim_paths_args_rustdoc(&mut rustdoc, build_runner, unit, trim_paths)?;
    }

    rustdoc.args(unit.pkg.manifest().lint_rustflags());

    let metadata = build_runner.metadata_for_doc_units[unit];
    rustdoc
        .arg("-C")
        .arg(format!("metadata={}", metadata.c_metadata()));

    if unit.mode.is_doc_scrape() {
        debug_assert!(build_runner.bcx.scrape_units.contains(unit));

        if unit.target.is_test() {
            rustdoc.arg("--scrape-tests");
        }

        rustdoc.arg("-Zunstable-options");

        rustdoc
            .arg("--scrape-examples-output-path")
            .arg(scrape_output_path(build_runner, unit)?);

        // Only scrape example for items from crates in the workspace, to reduce generated file size
        for pkg in build_runner.bcx.packages.packages() {
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

    if should_include_scrape_units(build_runner.bcx, unit) {
        rustdoc.arg("-Zunstable-options");
    }

    build_deps_args(&mut rustdoc, build_runner, unit)?;
    rustdoc::add_root_urls(build_runner, unit, &mut rustdoc)?;

    rustdoc::add_output_format(build_runner, &mut rustdoc)?;

    if let Some(args) = build_runner.bcx.extra_args_for(unit) {
        rustdoc.args(args);
    }
    rustdoc.args(&unit.rustdocflags);

    if !crate_version_flag_already_present(&rustdoc) {
        append_crate_version_flag(unit, &mut rustdoc);
    }

    Ok(rustdoc)
}

/// Creates a unit of work invoking `rustdoc` for documenting the `unit`.
fn rustdoc(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<Work> {
    let mut rustdoc = prepare_rustdoc(build_runner, unit)?;

    let crate_name = unit.target.crate_name();
    let doc_dir = build_runner.files().out_dir(unit);
    // Create the documentation directory ahead of time as rustdoc currently has
    // a bug where concurrent invocations will race to create this directory if
    // it doesn't already exist.
    paths::create_dir_all(&doc_dir)?;

    let target_desc = unit.target.description_named();
    let name = unit.pkg.name();
    let build_script_outputs = Arc::clone(&build_runner.build_script_outputs);
    let package_id = unit.pkg.package_id();
    let target = Target::clone(&unit.target);
    let manifest = ManifestErrorContext::new(build_runner, unit);

    let rustdoc_dep_info_loc = rustdoc_dep_info_loc(build_runner, unit);
    let dep_info_loc = fingerprint::dep_info_loc(build_runner, unit);
    let build_dir = build_runner.bcx.ws.build_dir().into_path_unlocked();
    let pkg_root = unit.pkg.root().to_path_buf();
    let cwd = rustdoc
        .get_cwd()
        .unwrap_or_else(|| build_runner.bcx.gctx.cwd())
        .to_path_buf();
    let fingerprint_dir = build_runner.files().fingerprint_dir(unit);
    let is_local = unit.is_local();
    let env_config = Arc::clone(build_runner.bcx.gctx.env_config()?);
    let rustdoc_depinfo_enabled = build_runner.bcx.gctx.cli_unstable().rustdoc_depinfo;

    let mut output_options = OutputOptions::new(build_runner, unit);
    let script_metadatas = build_runner.find_build_script_metadatas(unit);
    let scrape_outputs = if should_include_scrape_units(build_runner.bcx, unit) {
        Some(
            build_runner
                .bcx
                .scrape_units
                .iter()
                .map(|unit| {
                    Ok((
                        build_runner.files().metadata(unit).unit_id(),
                        scrape_output_path(build_runner, unit)?,
                    ))
                })
                .collect::<CargoResult<HashMap<_, _>>>()?,
        )
    } else {
        None
    };

    let failed_scrape_units = Arc::clone(&build_runner.failed_scrape_units);
    let hide_diagnostics_for_scrape_unit = build_runner.bcx.unit_can_fail_for_docscraping(unit)
        && !matches!(
            build_runner.bcx.gctx.shell().verbosity(),
            Verbosity::Verbose
        );
    let failed_scrape_diagnostic = hide_diagnostics_for_scrape_unit.then(|| {
        make_failed_scrape_diagnostic(
            build_runner,
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
            script_metadatas,
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
        let timestamp = paths::set_invocation_time(&fingerprint_dir)?;

        let result = rustdoc
            .exec_with_streaming(
                &mut |line| on_stdout_line(state, line, package_id, &target),
                &mut |line| {
                    on_stderr_line(
                        state,
                        line,
                        package_id,
                        &manifest,
                        &target,
                        &mut output_options,
                    )
                },
                false,
            )
            .map_err(verbose_if_simple_exit_code)
            .with_context(|| format!("could not document `{}`", name));

        if let Err(e) = result {
            if let Some(diagnostic) = failed_scrape_diagnostic {
                state.warning(diagnostic);
            }

            return Err(e);
        }

        if rustdoc_depinfo_enabled && rustdoc_dep_info_loc.exists() {
            fingerprint::translate_dep_info(
                &rustdoc_dep_info_loc,
                &dep_info_loc,
                &cwd,
                &pkg_root,
                &build_dir,
                &rustdoc,
                // Should we track source file for doc gen?
                is_local,
                &env_config,
            )
            .with_context(|| {
                internal(format_args!(
                    "could not parse/generate dep info at: {}",
                    rustdoc_dep_info_loc.display()
                ))
            })?;
            // This mtime shift allows Cargo to detect if a source file was
            // modified in the middle of the build.
            paths::set_file_time_no_err(dep_info_loc, timestamp);
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
    if !unit.show_warnings(bcx.gctx) {
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
fn add_allow_features(build_runner: &BuildRunner<'_, '_>, cmd: &mut ProcessBuilder) {
    if let Some(allow) = &build_runner.bcx.gctx.cli_unstable().allow_features {
        use std::fmt::Write;
        let mut arg = String::from("-Zallow-features=");
        for f in allow {
            let _ = write!(&mut arg, "{f},");
        }
        cmd.arg(arg.trim_end_matches(','));
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
fn add_error_format_and_color(build_runner: &BuildRunner<'_, '_>, cmd: &mut ProcessBuilder) {
    let enable_timings = build_runner.bcx.gctx.cli_unstable().section_timings
        && !build_runner.bcx.build_config.timing_outputs.is_empty();
    if enable_timings {
        cmd.arg("-Zunstable-options");
    }

    cmd.arg("--error-format=json");
    let mut json = String::from("--json=diagnostic-rendered-ansi,artifacts,future-incompat");

    match build_runner.bcx.build_config.message_format {
        MessageFormat::Short | MessageFormat::Json { short: true, .. } => {
            json.push_str(",diagnostic-short");
        }
        _ => {}
    }

    if enable_timings {
        json.push_str(",timings");
    }

    cmd.arg(json);

    let gctx = build_runner.bcx.gctx;
    if let Some(width) = gctx.shell().err_width().diagnostic_terminal_width() {
        cmd.arg(format!("--diagnostic-width={width}"));
    }
}

/// Adds essential rustc flags and environment variables to the command to execute.
fn build_base_args(
    build_runner: &BuildRunner<'_, '_>,
    cmd: &mut ProcessBuilder,
    unit: &Unit,
) -> CargoResult<()> {
    assert!(!unit.mode.is_run_custom_build());

    let bcx = build_runner.bcx;
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
        trim_paths,
        hint_mostly_unused: profile_hint_mostly_unused,
        ..
    } = unit.profile.clone();
    let hints = unit.pkg.hints().cloned().unwrap_or_default();
    let test = unit.mode.is_any_test();

    let warn = |msg: &str| {
        bcx.gctx.shell().warn(format!(
            "{}@{}: {msg}",
            unit.pkg.package_id().name(),
            unit.pkg.package_id().version()
        ))
    };
    let unit_capped_warn = |msg: &str| {
        if unit.show_warnings(bcx.gctx) {
            warn(msg)
        } else {
            Ok(())
        }
    };

    cmd.arg("--crate-name").arg(&unit.target.crate_name());

    let edition = unit.target.edition();
    edition.cmd_edition_arg(cmd);

    add_path_args(bcx.ws, unit, cmd);
    add_error_format_and_color(build_runner, cmd);
    add_allow_features(build_runner, cmd);

    let mut contains_dy_lib = false;
    if !test {
        for crate_type in &unit.target.rustc_crate_types() {
            cmd.arg("--crate-type").arg(crate_type.as_str());
            contains_dy_lib |= crate_type == &CrateType::Dylib;
        }
    }

    if unit.mode.is_check() {
        cmd.arg("--emit=dep-info,metadata");
    } else if build_runner.bcx.gctx.cli_unstable().no_embed_metadata {
        // Nightly rustc supports the -Zembed-metadata=no flag, which tells it to avoid including
        // full metadata in rlib/dylib artifacts, to save space on disk. In this case, metadata
        // will only be stored in .rmeta files.
        // When we use this flag, we should also pass --emit=metadata to all artifacts that
        // contain useful metadata (rlib/dylib/proc macros), so that a .rmeta file is actually
        // generated. If we didn't do this, the full metadata would not get written anywhere.
        // However, we do not want to pass --emit=metadata to artifacts that never produce useful
        // metadata, such as binaries, because that would just unnecessarily create empty .rmeta
        // files on disk.
        if unit.benefits_from_no_embed_metadata() {
            cmd.arg("--emit=dep-info,metadata,link");
            cmd.args(&["-Z", "embed-metadata=no"]);
        } else {
            cmd.arg("--emit=dep-info,link");
        }
    } else {
        // If we don't use -Zembed-metadata=no, we emit .rmeta files only for rlib outputs.
        // This metadata may be used in this session for a pipelined compilation, or it may
        // be used in a future Cargo session as part of a pipelined compile.
        if !unit.requires_upstream_objects() {
            cmd.arg("--emit=dep-info,metadata,link");
        } else {
            cmd.arg("--emit=dep-info,link");
        }
    }

    let prefer_dynamic = (unit.target.for_host() && !unit.target.is_custom_build())
        || (contains_dy_lib && !build_runner.is_primary_package(unit));
    if prefer_dynamic {
        cmd.arg("-C").arg("prefer-dynamic");
    }

    if opt_level.as_str() != "0" {
        cmd.arg("-C").arg(&format!("opt-level={}", opt_level));
    }

    if *panic != PanicStrategy::Unwind {
        cmd.arg("-C").arg(format!("panic={}", panic));
    }
    if *panic == PanicStrategy::ImmediateAbort {
        cmd.arg("-Z").arg("unstable-options");
    }

    cmd.args(&lto_args(build_runner, unit));

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
            if build_runner
                .bcx
                .target_data
                .info(unit.kind)
                .supports_debuginfo_split(split)
            {
                cmd.arg("-C").arg(format!("split-debuginfo={split}"));
            }
        }
    }

    if let Some(trim_paths) = trim_paths {
        trim_paths_args(cmd, build_runner, unit, &trim_paths)?;
    }

    cmd.args(unit.pkg.manifest().lint_rustflags());
    cmd.args(&profile_rustflags);

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
        if *panic == PanicStrategy::Abort || *panic == PanicStrategy::ImmediateAbort {
            cmd.arg("-Z").arg("panic-abort-tests");
        }
    } else if test {
        cmd.arg("--cfg").arg("test");
    }

    cmd.args(&features_args(unit));
    cmd.args(&check_cfg_args(unit));

    let meta = build_runner.files().metadata(unit);
    cmd.arg("-C")
        .arg(&format!("metadata={}", meta.c_metadata()));
    if let Some(c_extra_filename) = meta.c_extra_filename() {
        cmd.arg("-C")
            .arg(&format!("extra-filename=-{c_extra_filename}"));
    }

    if rpath {
        cmd.arg("-C").arg("rpath");
    }

    cmd.arg("--out-dir")
        .arg(&build_runner.files().out_dir(unit));

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
        build_runner
            .compilation
            .target_linker(unit.kind)
            .as_ref()
            .map(|s| s.as_ref()),
    );
    if incremental {
        let dir = build_runner.files().incremental_dir(&unit);
        opt(cmd, "-C", "incremental=", Some(dir.as_os_str()));
    }

    let pkg_hint_mostly_unused = match hints.mostly_unused {
        None => None,
        Some(toml::Value::Boolean(b)) => Some(b),
        Some(v) => {
            unit_capped_warn(&format!(
                "ignoring unsupported value type ({}) for 'hints.mostly-unused', which expects a boolean",
                v.type_str()
            ))?;
            None
        }
    };
    if profile_hint_mostly_unused
        .or(pkg_hint_mostly_unused)
        .unwrap_or(false)
    {
        if bcx.gctx.cli_unstable().profile_hint_mostly_unused {
            cmd.arg("-Zhint-mostly-unused");
        } else {
            if profile_hint_mostly_unused.is_some() {
                // Profiles come from the top-level unit, so we don't use `unit_capped_warn` here.
                warn(
                    "ignoring 'hint-mostly-unused' profile option, pass `-Zprofile-hint-mostly-unused` to enable it",
                )?;
            } else if pkg_hint_mostly_unused.is_some() {
                unit_capped_warn(
                    "ignoring 'hints.mostly-unused', pass `-Zprofile-hint-mostly-unused` to enable it",
                )?;
            }
        }
    }

    let strip = strip.into_inner();
    if strip != StripInner::None {
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
            let exe_path = build_runner.files().bin_link_for_target(
                bin_target,
                unit.kind,
                build_runner.bcx,
            )?;
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

/// Like [`trim_paths_args`] but for rustdoc invocations.
fn trim_paths_args_rustdoc(
    cmd: &mut ProcessBuilder,
    build_runner: &BuildRunner<'_, '_>,
    unit: &Unit,
    trim_paths: &TomlTrimPaths,
) -> CargoResult<()> {
    match trim_paths {
        // rustdoc supports diagnostics trimming only.
        TomlTrimPaths::Values(values) if !values.contains(&TomlTrimPathsValue::Diagnostics) => {
            return Ok(());
        }
        _ => {}
    }

    // feature gate was checked during manifest/config parsing.
    cmd.arg("-Zunstable-options");

    // Order of `--remap-path-prefix` flags is important for `-Zbuild-std`.
    // We want to show `/rustc/<hash>/library/std` instead of `std-0.0.0`.
    cmd.arg(package_remap(build_runner, unit));
    cmd.arg(build_dir_remap(build_runner));
    cmd.arg(sysroot_remap(build_runner, unit));

    Ok(())
}

/// Generates the `--remap-path-scope` and `--remap-path-prefix` for [RFC 3127].
/// See also unstable feature [`-Ztrim-paths`].
///
/// [RFC 3127]: https://rust-lang.github.io/rfcs/3127-trim-paths.html
/// [`-Ztrim-paths`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#profile-trim-paths-option
fn trim_paths_args(
    cmd: &mut ProcessBuilder,
    build_runner: &BuildRunner<'_, '_>,
    unit: &Unit,
    trim_paths: &TomlTrimPaths,
) -> CargoResult<()> {
    if trim_paths.is_none() {
        return Ok(());
    }

    // feature gate was checked during manifest/config parsing.
    cmd.arg("-Zunstable-options");
    cmd.arg(format!("-Zremap-path-scope={trim_paths}"));

    // Order of `--remap-path-prefix` flags is important for `-Zbuild-std`.
    // We want to show `/rustc/<hash>/library/std` instead of `std-0.0.0`.
    cmd.arg(package_remap(build_runner, unit));
    cmd.arg(build_dir_remap(build_runner));
    cmd.arg(sysroot_remap(build_runner, unit));

    Ok(())
}

/// Path prefix remap rules for sysroot.
///
/// This remap logic aligns with rustc:
/// <https://github.com/rust-lang/rust/blob/c2ef3516/src/bootstrap/src/lib.rs#L1113-L1116>
fn sysroot_remap(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> OsString {
    let mut remap = OsString::from("--remap-path-prefix=");
    remap.push({
        // See also `detect_sysroot_src_path()`.
        let mut sysroot = build_runner.bcx.target_data.info(unit.kind).sysroot.clone();
        sysroot.push("lib");
        sysroot.push("rustlib");
        sysroot.push("src");
        sysroot.push("rust");
        sysroot
    });
    remap.push("=");
    remap.push("/rustc/");
    if let Some(commit_hash) = build_runner.bcx.rustc().commit_hash.as_ref() {
        remap.push(commit_hash);
    } else {
        remap.push(build_runner.bcx.rustc().version.to_string());
    }
    remap
}

/// Path prefix remap rules for dependencies.
///
/// * Git dependencies: remove `~/.cargo/git/checkouts` prefix.
/// * Registry dependencies: remove `~/.cargo/registry/src` prefix.
/// * Others (e.g. path dependencies):
///     * relative paths to workspace root if inside the workspace directory.
///     * otherwise remapped to `<pkg>-<version>`.
fn package_remap(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> OsString {
    let pkg_root = unit.pkg.root();
    let ws_root = build_runner.bcx.ws.root();
    let mut remap = OsString::from("--remap-path-prefix=");
    let source_id = unit.pkg.package_id().source_id();
    if source_id.is_git() {
        remap.push(
            build_runner
                .bcx
                .gctx
                .git_checkouts_path()
                .as_path_unlocked(),
        );
        remap.push("=");
    } else if source_id.is_registry() {
        remap.push(
            build_runner
                .bcx
                .gctx
                .registry_source_path()
                .as_path_unlocked(),
        );
        remap.push("=");
    } else if pkg_root.strip_prefix(ws_root).is_ok() {
        remap.push(ws_root);
        remap.push("=."); // remap to relative rustc work dir explicitly
    } else {
        remap.push(pkg_root);
        remap.push("=");
        remap.push(unit.pkg.name());
        remap.push("-");
        remap.push(unit.pkg.version().to_string());
    }
    remap
}

/// Remap all paths pointing to `build.build-dir`,
/// i.e., `[BUILD_DIR]/debug/deps/foo-[HASH].dwo` would be remapped to
/// `/cargo/build-dir/debug/deps/foo-[HASH].dwo`
/// (note the `/cargo/build-dir` prefix).
///
/// This covers scenarios like:
///
/// * Build script generated code. For example, a build script may call `file!`
///   macros, and the associated crate uses [`include!`] to include the expanded
///   [`file!`] macro in-place via the `OUT_DIR` environment.
/// * On Linux, `DW_AT_GNU_dwo_name` that contains paths to split debuginfo
///   files (dwp and dwo).
fn build_dir_remap(build_runner: &BuildRunner<'_, '_>) -> OsString {
    let build_dir = build_runner.bcx.ws.build_dir();
    let mut remap = OsString::from("--remap-path-prefix=");
    remap.push(build_dir.as_path_unlocked());
    remap.push("=/cargo/build-dir");
    remap
}

/// Generates the `--check-cfg` arguments for the `unit`.
fn check_cfg_args(unit: &Unit) -> Vec<OsString> {
    // The routine below generates the --check-cfg arguments. Our goals here are to
    // enable the checking of conditionals and pass the list of declared features.
    //
    // In the simplified case, it would resemble something like this:
    //
    //   --check-cfg=cfg() --check-cfg=cfg(feature, values(...))
    //
    // but having `cfg()` is redundant with the second argument (as well-known names
    // and values are implicitly enabled when one or more `--check-cfg` argument is
    // passed) so we don't emit it and just pass:
    //
    //   --check-cfg=cfg(feature, values(...))
    //
    // This way, even if there are no declared features, the config `feature` will
    // still be expected, meaning users would get "unexpected value" instead of name.
    // This wasn't always the case, see rust-lang#119930 for some details.

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

    // In addition to the package features, we also include the `test` cfg (since
    // compiler-team#785, as to be able to someday apply it conditionally), as well
    // the `docsrs` cfg from the docs.rs service.
    //
    // We include `docsrs` here (in Cargo) instead of rustc, since there is a much closer
    // relationship between Cargo and docs.rs than rustc and docs.rs. In particular, all
    // users of docs.rs use Cargo, but not all users of rustc (like Rust-for-Linux) use docs.rs.

    vec![
        OsString::from("--check-cfg"),
        OsString::from("cfg(docsrs,test)"),
        OsString::from("--check-cfg"),
        arg_feature,
    ]
}

/// Adds LTO related codegen flags.
fn lto_args(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> Vec<OsString> {
    let mut result = Vec::new();
    let mut push = |arg: &str| {
        result.push(OsString::from("-C"));
        result.push(OsString::from(arg));
    };
    match build_runner.lto[unit] {
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
fn build_deps_args(
    cmd: &mut ProcessBuilder,
    build_runner: &BuildRunner<'_, '_>,
    unit: &Unit,
) -> CargoResult<()> {
    let bcx = build_runner.bcx;
    if build_runner.bcx.gctx.cli_unstable().build_dir_new_layout {
        let mut map = BTreeMap::new();

        // Recursively add all depenendency args to rustc process
        add_dep_arg(&mut map, build_runner, unit);

        let paths = map.into_iter().map(|(_, path)| path).sorted_unstable();

        for path in paths {
            cmd.arg("-L").arg(&{
                let mut deps = OsString::from("dependency=");
                deps.push(path);
                deps
            });
        }
    } else {
        cmd.arg("-L").arg(&{
            let mut deps = OsString::from("dependency=");
            deps.push(build_runner.files().deps_dir(unit));
            deps
        });
    }

    // Be sure that the host path is also listed. This'll ensure that proc macro
    // dependencies are correctly found (for reexported macros).
    if !unit.kind.is_host() {
        cmd.arg("-L").arg(&{
            let mut deps = OsString::from("dependency=");
            deps.push(build_runner.files().host_deps(unit));
            deps
        });
    }

    let deps = build_runner.unit_deps(unit);

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
            let dep_name = dep.unit.target.crate_name();
            let name = unit.target.crate_name();
            bcx.gctx.shell().print_report(&[
                Level::WARNING.secondary_title(format!("the package `{dep_name}` provides no linkable target"))
                    .elements([
                        Level::NOTE.message(format!("this might cause `{name}` to fail compilation")),
                        Level::NOTE.message("this warning might turn into a hard error in the future"),
                        Level::HELP.message(format!("consider adding 'dylib' or 'rlib' to key 'crate-type' in `{dep_name}`'s Cargo.toml"))
                    ])
            ], false)?;
        }
    }

    let mut unstable_opts = false;

    // Add `OUT_DIR` environment variables for build scripts
    let first_custom_build_dep = deps.iter().find(|dep| dep.unit.mode.is_run_custom_build());
    if let Some(dep) = first_custom_build_dep {
        let out_dir = &build_runner.files().build_script_out_dir(&dep.unit);
        cmd.env("OUT_DIR", &out_dir);
    }

    // Adding output directory for each build script
    let is_multiple_build_scripts_enabled = unit
        .pkg
        .manifest()
        .unstable_features()
        .require(Feature::multiple_build_scripts())
        .is_ok();

    if is_multiple_build_scripts_enabled {
        for dep in deps {
            if dep.unit.mode.is_run_custom_build() {
                let out_dir = &build_runner.files().build_script_out_dir(&dep.unit);
                let target_name = dep.unit.target.name();
                let out_dir_prefix = target_name
                    .strip_prefix("build-script-")
                    .unwrap_or(target_name);
                let out_dir_name = format!("{out_dir_prefix}_OUT_DIR");
                cmd.env(&out_dir_name, &out_dir);
            }
        }
    }
    for arg in extern_args(build_runner, unit, &mut unstable_opts)? {
        cmd.arg(arg);
    }

    for (var, env) in artifact::get_env(build_runner, deps)? {
        cmd.env(&var, env);
    }

    // This will only be set if we're already using a feature
    // requiring nightly rust
    if unstable_opts {
        cmd.arg("-Z").arg("unstable-options");
    }

    Ok(())
}

fn add_dep_arg<'a, 'b: 'a>(
    map: &mut BTreeMap<&'a Unit, PathBuf>,
    build_runner: &'b BuildRunner<'b, '_>,
    unit: &'a Unit,
) {
    if map.contains_key(&unit) {
        return;
    }
    map.insert(&unit, build_runner.files().deps_dir(&unit));

    for dep in build_runner.unit_deps(unit) {
        add_dep_arg(map, build_runner, &dep.unit);
    }
}

/// Adds extra rustc flags and environment variables collected from the output
/// of a build-script to the command to execute, include custom environment
/// variables and `cfg`.
fn add_custom_flags(
    cmd: &mut ProcessBuilder,
    build_script_outputs: &BuildScriptOutputs,
    metadata_vec: Option<Vec<UnitHash>>,
) -> CargoResult<()> {
    if let Some(metadata_vec) = metadata_vec {
        for metadata in metadata_vec {
            if let Some(output) = build_script_outputs.get(metadata) {
                for cfg in output.cfgs.iter() {
                    cmd.arg("--cfg").arg(cfg);
                }
                for check_cfg in &output.check_cfgs {
                    cmd.arg("--check-cfg").arg(check_cfg);
                }
                for (name, value) in output.env.iter() {
                    cmd.env(name, value);
                }
            }
        }
    }

    Ok(())
}

/// Generates a list of `--extern` arguments.
pub fn extern_args(
    build_runner: &BuildRunner<'_, '_>,
    unit: &Unit,
    unstable_opts: &mut bool,
) -> CargoResult<Vec<OsString>> {
    let mut result = Vec::new();
    let deps = build_runner.unit_deps(unit);

    let no_embed_metadata = build_runner.bcx.gctx.cli_unstable().no_embed_metadata;

    // Closure to add one dependency to `result`.
    let mut link_to =
        |dep: &UnitDep, extern_crate_name: InternedString, noprelude: bool| -> CargoResult<()> {
            let mut value = OsString::new();
            let mut opts = Vec::new();
            let is_public_dependency_enabled = unit
                .pkg
                .manifest()
                .unstable_features()
                .require(Feature::public_dependency())
                .is_ok()
                || build_runner.bcx.gctx.cli_unstable().public_dependency;
            if !dep.public && unit.target.is_lib() && is_public_dependency_enabled {
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

            let outputs = build_runner.outputs(&dep.unit)?;

            if build_runner.only_requires_rmeta(unit, &dep.unit) || dep.unit.mode.is_check() {
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
                    // If we use -Zembed-metadata=no, we also need to pass the path to the
                    // corresponding .rmeta file to the linkable artifact, because the
                    // normal dependency (rlib) doesn't contain the full metadata.
                    else if no_embed_metadata && output.flavor == FileFlavor::Rmeta {
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
    cache_cell: Option<(PathBuf, OnceCell<File>)>,
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
    fn new(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> OutputOptions {
        let path = build_runner.files().message_cache_path(unit);
        // Remove old cache, ignore ENOENT, which is the common case.
        drop(fs::remove_file(&path));
        let cache_cell = Some((path, OnceCell::new()));
        let show_diagnostics =
            build_runner.bcx.gctx.warning_handling().unwrap_or_default() != WarningHandling::Allow;
        OutputOptions {
            format: build_runner.bcx.build_config.message_format,
            cache_cell,
            show_diagnostics,
            warnings_seen: 0,
            errors_seen: 0,
        }
    }
}

/// Cloned and sendable context about the manifest file.
///
/// Sometimes we enrich rustc's errors with some locations in the manifest file; this
/// contains a `Send`-able copy of the manifest information that we need for the
/// enriched errors.
struct ManifestErrorContext {
    /// The path to the manifest.
    path: PathBuf,
    /// The locations of various spans within the manifest.
    spans: toml::Spanned<toml::de::DeTable<'static>>,
    /// The raw manifest contents.
    contents: String,
    /// A lookup for all the unambiguous renamings, mapping from the original package
    /// name to the renamed one.
    rename_table: HashMap<InternedString, InternedString>,
    /// A list of targets we're compiling for, to determine which of the `[target.<something>.dependencies]`
    /// tables might be of interest.
    requested_kinds: Vec<CompileKind>,
    /// A list of all the collections of cfg values, one collection for each target, to determine
    /// which of the `[target.'cfg(...)'.dependencies]` tables might be of interest.
    cfgs: Vec<Vec<Cfg>>,
    host_name: InternedString,
    /// Cargo's working directory (for printing out a more friendly manifest path).
    cwd: PathBuf,
    /// Terminal width for formatting diagnostics.
    term_width: usize,
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
    manifest: &ManifestErrorContext,
    target: &Target,
    options: &mut OutputOptions,
) -> CargoResult<()> {
    if on_stderr_line_inner(state, line, package_id, manifest, target, options)? {
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
    manifest: &ManifestErrorContext,
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

    let res = serde_json::from_str::<SectionTiming>(compiler_message.get());
    if let Ok(timing_record) = res {
        state.on_section_timing_emitted(timing_record);
        return Ok(false);
    }

    // Returns `true` if the diagnostic was modified.
    let add_pub_in_priv_diagnostic = |diag: &mut String| -> bool {
        // We are parsing the compiler diagnostic here, as this information isn't
        // currently exposed elsewhere.
        // At the time of writing this comment, rustc emits two different
        // "exported_private_dependencies" errors:
        //  - type `FromPriv` from private dependency 'priv_dep' in public interface
        //  - struct `FromPriv` from private dependency 'priv_dep' is re-exported
        // This regex matches them both. To see if it needs to be updated, grep the rust
        // source for "EXPORTED_PRIVATE_DEPENDENCIES".
        static PRIV_DEP_REGEX: LazyLock<Regex> =
            LazyLock::new(|| Regex::new("from private dependency '([A-Za-z0-9-_]+)'").unwrap());
        if let Some(crate_name) = PRIV_DEP_REGEX.captures(diag).and_then(|m| m.get(1))
            && let Some(span) = manifest.find_crate_span(crate_name.as_str())
        {
            let rel_path = pathdiff::diff_paths(&manifest.path, &manifest.cwd)
                .unwrap_or_else(|| manifest.path.clone())
                .display()
                .to_string();
            let report = [Group::with_title(Level::NOTE.secondary_title(format!(
                "dependency `{}` declared here",
                crate_name.as_str()
            )))
            .element(
                Snippet::source(&manifest.contents)
                    .path(rel_path)
                    .annotation(AnnotationKind::Context.span(span)),
            )];

            let rendered = Renderer::styled()
                .term_width(manifest.term_width)
                .render(&report);
            diag.push_str(&rendered);
            diag.push('\n');
            return true;
        }
        false
    };

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
            struct CompilerMessage<'a> {
                // `rendered` contains escape sequences, which can't be
                // zero-copy deserialized by serde_json.
                // See https://github.com/serde-rs/json/issues/742
                rendered: String,
                #[serde(borrow)]
                message: Cow<'a, str>,
                #[serde(borrow)]
                level: Cow<'a, str>,
                children: Vec<PartialDiagnostic>,
                code: Option<DiagnosticCode>,
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

            #[derive(serde::Deserialize)]
            struct DiagnosticCode {
                code: String,
            }

            if let Ok(mut msg) = serde_json::from_str::<CompilerMessage<'_>>(compiler_message.get())
            {
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
                let mut rendered = msg.rendered;
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
                    if msg
                        .code
                        .as_ref()
                        .is_some_and(|c| c.code == "exported_private_dependencies")
                        && options.format != MessageFormat::Short
                    {
                        add_pub_in_priv_diagnostic(&mut rendered);
                    }
                    let lint = msg.code.is_some();
                    state.emit_diag(&msg.level, rendered, lint, machine_applicable)?;
                }
                return Ok(true);
            }
        }

        MessageFormat::Json { ansi, .. } => {
            #[derive(serde::Deserialize, serde::Serialize)]
            struct CompilerMessage<'a> {
                rendered: String,
                #[serde(flatten, borrow)]
                other: std::collections::BTreeMap<Cow<'a, str>, serde_json::Value>,
                code: Option<DiagnosticCode<'a>>,
            }

            #[derive(serde::Deserialize, serde::Serialize)]
            struct DiagnosticCode<'a> {
                code: String,
                #[serde(flatten, borrow)]
                other: std::collections::BTreeMap<Cow<'a, str>, serde_json::Value>,
            }

            if let Ok(mut error) =
                serde_json::from_str::<CompilerMessage<'_>>(compiler_message.get())
            {
                let modified_diag = if error
                    .code
                    .as_ref()
                    .is_some_and(|c| c.code == "exported_private_dependencies")
                {
                    add_pub_in_priv_diagnostic(&mut error.rendered)
                } else {
                    false
                };

                // Remove color information from the rendered string if color is not
                // enabled. Cargo always asks for ANSI colors from rustc. This allows
                // cached replay to enable/disable colors without re-invoking rustc.
                if !ansi {
                    error.rendered = anstream::adapter::strip_str(&error.rendered).to_string();
                }
                if !ansi || modified_diag {
                    let new_line = serde_json::to_string(&error)?;
                    compiler_message = serde_json::value::RawValue::from_string(new_line)?;
                }
            }
        }
    }

    // We always tell rustc to emit messages about artifacts being produced.
    // These messages feed into pipelined compilation, as well as timing
    // information.
    //
    // Look for a matching directive and inform Cargo internally that a
    // metadata file has been produced.
    #[derive(serde::Deserialize)]
    struct ArtifactNotification<'a> {
        #[serde(borrow)]
        artifact: Cow<'a, str>,
    }

    if let Ok(artifact) = serde_json::from_str::<ArtifactNotification<'_>>(compiler_message.get()) {
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
    struct CompilerMessage<'a> {
        #[serde(borrow)]
        message: Cow<'a, str>,
        #[serde(borrow)]
        level: Cow<'a, str>,
    }

    if let Ok(msg) = serde_json::from_str::<CompilerMessage<'_>>(compiler_message.get()) {
        if msg.message.starts_with("aborting due to")
            || msg.message.ends_with("warning emitted")
            || msg.message.ends_with("warnings emitted")
        {
            // Skip this line; we'll print our own summary at the end.
            return Ok(true);
        }
        count_diagnostic(&msg.level, options);
    }

    let msg = machine_message::FromCompiler {
        package_id: package_id.to_spec(),
        manifest_path: &manifest.path,
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

impl ManifestErrorContext {
    fn new(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> ManifestErrorContext {
        let mut duplicates = HashSet::new();
        let mut rename_table = HashMap::new();

        for dep in build_runner.unit_deps(unit) {
            let unrenamed_id = dep.unit.pkg.package_id().name();
            if duplicates.contains(&unrenamed_id) {
                continue;
            }
            match rename_table.entry(unrenamed_id) {
                std::collections::hash_map::Entry::Occupied(occ) => {
                    occ.remove_entry();
                    duplicates.insert(unrenamed_id);
                }
                std::collections::hash_map::Entry::Vacant(vac) => {
                    vac.insert(dep.extern_crate_name);
                }
            }
        }

        let bcx = build_runner.bcx;
        ManifestErrorContext {
            path: unit.pkg.manifest_path().to_owned(),
            spans: unit.pkg.manifest().document().clone(),
            contents: unit.pkg.manifest().contents().to_owned(),
            requested_kinds: bcx.target_data.requested_kinds().to_owned(),
            host_name: bcx.rustc().host,
            rename_table,
            cwd: path_args(build_runner.bcx.ws, unit).1,
            cfgs: bcx
                .target_data
                .requested_kinds()
                .iter()
                .map(|k| bcx.target_data.cfg(*k).to_owned())
                .collect(),
            term_width: bcx
                .gctx
                .shell()
                .err_width()
                .diagnostic_terminal_width()
                .unwrap_or(annotate_snippets::renderer::DEFAULT_TERM_WIDTH),
        }
    }

    fn requested_target_names(&self) -> impl Iterator<Item = &str> {
        self.requested_kinds.iter().map(|kind| match kind {
            CompileKind::Host => &self.host_name,
            CompileKind::Target(target) => target.short_name(),
        })
    }

    /// Find a span for the dependency that specifies this unrenamed crate, if it's unique.
    ///
    /// rustc diagnostics (at least for public-in-private) mention the un-renamed
    /// crate: if you have `foo = { package = "bar" }`, the rustc diagnostic will
    /// say "bar".
    ///
    /// This function does its best to find a span for "bar", but it could fail if
    /// there are multiple candidates:
    ///
    /// ```toml
    /// foo = { package = "bar" }
    /// baz = { path = "../bar", package = "bar" }
    /// ```
    fn find_crate_span(&self, unrenamed: &str) -> Option<Range<usize>> {
        let orig_name = self.rename_table.get(unrenamed)?.as_str();

        if let Some((k, v)) = get_key_value(&self.spans, &["dependencies", orig_name]) {
            // We make some effort to find the unrenamed text: in
            //
            // ```
            // foo = { package = "bar" }
            // ```
            //
            // we try to find the "bar", but fall back to "foo" if we can't (which might
            // happen if the renaming took place in the workspace, for example).
            if let Some(package) = v.get_ref().as_table().and_then(|t| t.get("package")) {
                return Some(package.span());
            } else {
                return Some(k.span());
            }
        }

        // The dependency could also be in a target-specific table, like
        // [target.x86_64-unknown-linux-gnu.dependencies] or
        // [target.'cfg(something)'.dependencies]. We filter out target tables
        // that don't match a requested target or a requested cfg.
        if let Some(target) = self
            .spans
            .as_ref()
            .get("target")
            .and_then(|t| t.as_ref().as_table())
        {
            for (platform, platform_table) in target.iter() {
                match platform.as_ref().parse::<Platform>() {
                    Ok(Platform::Name(name)) => {
                        if !self.requested_target_names().any(|n| n == name) {
                            continue;
                        }
                    }
                    Ok(Platform::Cfg(cfg_expr)) => {
                        if !self.cfgs.iter().any(|cfgs| cfg_expr.matches(cfgs)) {
                            continue;
                        }
                    }
                    Err(_) => continue,
                }

                let Some(platform_table) = platform_table.as_ref().as_table() else {
                    continue;
                };

                if let Some(deps) = platform_table
                    .get("dependencies")
                    .and_then(|d| d.as_ref().as_table())
                {
                    if let Some((k, v)) = deps.get_key_value(orig_name) {
                        if let Some(package) = v.get_ref().as_table().and_then(|t| t.get("package"))
                        {
                            return Some(package.span());
                        } else {
                            return Some(k.span());
                        }
                    }
                }
            }
        }
        None
    }
}

/// Creates a unit of work that replays the cached compiler message.
///
/// Usually used when a job is fresh and doesn't need to recompile.
fn replay_output_cache(
    package_id: PackageId,
    manifest: ManifestErrorContext,
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
            on_stderr_line(state, trimmed, package_id, &manifest, &target, &mut options)?;
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
pub(crate) fn apply_env_config(
    gctx: &crate::GlobalContext,
    cmd: &mut ProcessBuilder,
) -> CargoResult<()> {
    for (key, value) in gctx.env_config()?.iter() {
        // never override a value that has already been set by cargo
        if cmd.get_envs().contains_key(key) {
            continue;
        }
        cmd.env(key, value);
    }
    Ok(())
}

/// Checks if there are some scrape units waiting to be processed.
fn should_include_scrape_units(bcx: &BuildContext<'_, '_>, unit: &Unit) -> bool {
    unit.mode.is_doc() && bcx.scrape_units.len() > 0 && bcx.ws.unit_needs_doc_scrape(unit)
}

/// Gets the file path of function call information output from `rustdoc`.
fn scrape_output_path(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<PathBuf> {
    assert!(unit.mode.is_doc() || unit.mode.is_doc_scrape());
    build_runner
        .outputs(unit)
        .map(|outputs| outputs[0].path.clone())
}

/// Gets the dep-info file emitted by rustdoc.
fn rustdoc_dep_info_loc(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> PathBuf {
    let mut loc = build_runner.files().fingerprint_file_path(unit, "");
    loc.set_extension("d");
    loc
}
