mod build_config;
mod build_context;
mod build_plan;
mod compilation;
mod compile_kind;
mod context;
mod custom_build;
mod fingerprint;
mod job;
mod job_queue;
mod layout;
mod links;
mod output_depinfo;
pub mod standard_lib;
mod timings;
mod unit;
pub mod unit_dependencies;

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use failure::Error;
use lazycell::LazyCell;
use log::debug;

pub use self::build_config::{BuildConfig, CompileMode, MessageFormat, ProfileKind};
pub use self::build_context::{BuildContext, FileFlavor, TargetConfig, TargetInfo};
use self::build_plan::BuildPlan;
pub use self::compilation::{Compilation, Doctest};
pub use self::compile_kind::{CompileKind, CompileTarget};
pub use self::context::Context;
pub use self::custom_build::{BuildOutput, BuildScriptOutputs, BuildScripts};
pub use self::job::Freshness;
use self::job::{Job, Work};
use self::job_queue::{JobQueue, JobState};
pub use self::layout::is_bad_artifact_name;
use self::output_depinfo::output_depinfo;
use self::unit_dependencies::UnitDep;
pub use crate::core::compiler::unit::{Unit, UnitInterner};
use crate::core::manifest::TargetSourcePath;
use crate::core::profiles::{Lto, PanicStrategy, Profile};
use crate::core::Feature;
use crate::core::{PackageId, Target};
use crate::util::errors::{self, CargoResult, CargoResultExt, Internal, ProcessError};
use crate::util::machine_message::Message;
use crate::util::paths;
use crate::util::{self, machine_message, ProcessBuilder};
use crate::util::{internal, join_paths, profile};

/// A glorified callback for executing calls to rustc. Rather than calling rustc
/// directly, we'll use an `Executor`, giving clients an opportunity to intercept
/// the build calls.
pub trait Executor: Send + Sync + 'static {
    /// Called after a rustc process invocation is prepared up-front for a given
    /// unit of work (may still be modified for runtime-known dependencies, when
    /// the work is actually executed).
    fn init<'a, 'cfg>(&self, _cx: &Context<'a, 'cfg>, _unit: &Unit<'a>) {}

    /// In case of an `Err`, Cargo will not continue with the build process for
    /// this package.
    fn exec(
        &self,
        cmd: ProcessBuilder,
        id: PackageId,
        target: &Target,
        mode: CompileMode,
        on_stdout_line: &mut dyn FnMut(&str) -> CargoResult<()>,
        on_stderr_line: &mut dyn FnMut(&str) -> CargoResult<()>,
    ) -> CargoResult<()>;

    /// Queried when queuing each unit of work. If it returns true, then the
    /// unit will always be rebuilt, independent of whether it needs to be.
    fn force_rebuild(&self, _unit: &Unit<'_>) -> bool {
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
        cmd: ProcessBuilder,
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

fn compile<'a, 'cfg: 'a>(
    cx: &mut Context<'a, 'cfg>,
    jobs: &mut JobQueue<'a, 'cfg>,
    plan: &mut BuildPlan,
    unit: &Unit<'a>,
    exec: &Arc<dyn Executor>,
    force_rebuild: bool,
) -> CargoResult<()> {
    let bcx = cx.bcx;
    let build_plan = bcx.build_config.build_plan;
    if !cx.compiled.insert(*unit) {
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
        Job::new(Work::noop(), Freshness::Fresh)
    } else if build_plan {
        Job::new(rustc(cx, unit, &exec.clone())?, Freshness::Dirty)
    } else {
        let force = exec.force_rebuild(unit) || force_rebuild;
        let mut job = fingerprint::prepare_target(cx, unit, force)?;
        job.before(if job.freshness() == Freshness::Dirty {
            let work = if unit.mode.is_doc() {
                rustdoc(cx, unit)?
            } else {
                rustc(cx, unit, exec)?
            };
            work.then(link_targets(cx, unit, false)?)
        } else {
            let work = if cx.bcx.show_warnings(unit.pkg.package_id()) {
                replay_output_cache(
                    unit.pkg.package_id(),
                    unit.target,
                    cx.files().message_cache_path(unit),
                    cx.bcx.build_config.message_format,
                    cx.bcx.config.shell().supports_color(),
                )
            } else {
                Work::noop()
            };
            // Need to link targets on both the dirty and fresh.
            work.then(link_targets(cx, unit, true)?)
        });

        job
    };
    jobs.enqueue(cx, unit, job)?;
    drop(p);

    // Be sure to compile all dependencies of this target as well.
    for unit in cx.dep_targets(unit).iter() {
        compile(cx, jobs, plan, unit, exec, false)?;
    }
    if build_plan {
        plan.add(cx, unit)?;
    }

    Ok(())
}

fn rustc<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
    exec: &Arc<dyn Executor>,
) -> CargoResult<Work> {
    let mut rustc = prepare_rustc(cx, &unit.target.rustc_crate_types(), unit)?;
    let build_plan = cx.bcx.build_config.build_plan;

    let name = unit.pkg.name().to_string();
    let buildkey = unit.buildkey();

    add_cap_lints(cx.bcx, unit, &mut rustc);

    let outputs = cx.outputs(unit)?;
    let root = cx.files().out_dir(unit);
    let kind = unit.kind;

    // Prepare the native lib state (extra `-L` and `-l` flags).
    let build_script_outputs = Arc::clone(&cx.build_script_outputs);
    let current_id = unit.pkg.package_id();
    let build_scripts = cx.build_scripts.get(unit).cloned();

    // If we are a binary and the package also contains a library, then we
    // don't pass the `-l` flags.
    let pass_l_flag = unit.target.is_lib() || !unit.pkg.targets().iter().any(|t| t.is_lib());
    let pass_cdylib_link_args = unit.target.is_cdylib();
    let do_rename = unit.target.allows_underscores() && !unit.mode.is_any_test();
    let real_name = unit.target.name().to_string();
    let crate_name = unit.target.crate_name();

    // Rely on `target_filenames` iterator as source of truth rather than rederiving filestem.
    let rustc_dep_info_loc = if do_rename && cx.files().metadata(unit).is_none() {
        root.join(&crate_name)
    } else {
        root.join(&cx.files().file_stem(unit))
    }
    .with_extension("d");
    let dep_info_loc = fingerprint::dep_info_loc(cx, unit);

    rustc.args(cx.bcx.rustflags_args(unit));
    if cx.bcx.config.cli_unstable().binary_dep_depinfo {
        rustc.arg("-Zbinary-dep-depinfo");
    }
    let mut output_options = OutputOptions::new(cx, unit);
    let package_id = unit.pkg.package_id();
    let target = unit.target.clone();
    let mode = unit.mode;

    exec.init(cx, unit);
    let exec = exec.clone();

    let root_output = cx.files().host_root().to_path_buf();
    let target_dir = cx.bcx.ws.target_dir().into_path_unlocked();
    let pkg_root = unit.pkg.root().to_path_buf();
    let cwd = rustc
        .get_cwd()
        .unwrap_or_else(|| cx.bcx.config.cwd())
        .to_path_buf();
    let fingerprint_dir = cx.files().fingerprint_dir(unit);

    return Ok(Work::new(move |state| {
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
                    pass_cdylib_link_args,
                    current_id,
                )?;
                add_plugin_deps(&mut rustc, &script_outputs, &build_scripts, &root_output)?;
            }
            add_custom_env(&mut rustc, &script_outputs, current_id, kind)?;
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
        }

        fn internal_if_simple_exit_code(err: Error) -> Error {
            // If a signal on unix (`code == None`) or an abnormal termination
            // on Windows (codes like `0xC0000409`), don't hide the error details.
            match err
                .downcast_ref::<ProcessError>()
                .as_ref()
                .and_then(|perr| perr.exit.and_then(|e| e.code()))
            {
                Some(n) if errors::is_simple_exit_code(n) => Internal::new(err).into(),
                _ => err,
            }
        }

        state.running(&rustc);
        let timestamp = paths::set_invocation_time(&fingerprint_dir)?;
        if build_plan {
            state.build_plan(buildkey, rustc.clone(), outputs.clone());
        } else {
            exec.exec(
                rustc,
                package_id,
                &target,
                mode,
                &mut |line| on_stdout_line(state, line, package_id, &target),
                &mut |line| on_stderr_line(state, line, package_id, &target, &mut output_options),
            )
            .map_err(internal_if_simple_exit_code)
            .chain_err(|| format!("could not compile `{}`.", name))?;
        }

        if do_rename && real_name != crate_name {
            let dst = &outputs[0].path;
            let src = dst.with_file_name(
                dst.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace(&real_name, &crate_name),
            );
            if src.exists() && src.file_name() != dst.file_name() {
                fs::rename(&src, &dst)
                    .chain_err(|| internal(format!("could not rename crate {:?}", src)))?;
            }
        }

        if rustc_dep_info_loc.exists() {
            fingerprint::translate_dep_info(
                &rustc_dep_info_loc,
                &dep_info_loc,
                &cwd,
                &pkg_root,
                &target_dir,
                // Do not track source files in the fingerprint for registry dependencies.
                current_id.source_id().is_path(),
            )
            .chain_err(|| {
                internal(format!(
                    "could not parse/generate dep info at: {}",
                    rustc_dep_info_loc.display()
                ))
            })?;
            filetime::set_file_times(dep_info_loc, timestamp, timestamp)?;
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
        pass_cdylib_link_args: bool,
        current_id: PackageId,
    ) -> CargoResult<()> {
        for key in build_scripts.to_link.iter() {
            let output = build_script_outputs.get(key).ok_or_else(|| {
                internal(format!(
                    "couldn't find build script output for {}/{:?}",
                    key.0, key.1
                ))
            })?;
            for path in output.library_paths.iter() {
                rustc.arg("-L").arg(path);
            }
            if key.0 == current_id {
                for cfg in &output.cfgs {
                    rustc.arg("--cfg").arg(cfg);
                }
                if pass_l_flag {
                    for name in output.library_links.iter() {
                        rustc.arg("-l").arg(name);
                    }
                }
                if pass_cdylib_link_args {
                    for arg in output.linker_args.iter() {
                        let link_arg = format!("link-arg={}", arg);
                        rustc.arg("-C").arg(link_arg);
                    }
                }
            }
        }
        Ok(())
    }

    // Add all custom environment variables present in `state` (after they've
    // been put there by one of the `build_scripts`) to the command provided.
    fn add_custom_env(
        rustc: &mut ProcessBuilder,
        build_script_outputs: &BuildScriptOutputs,
        current_id: PackageId,
        kind: CompileKind,
    ) -> CargoResult<()> {
        let key = (current_id, kind);
        if let Some(output) = build_script_outputs.get(&key) {
            for &(ref name, ref value) in output.env.iter() {
                rustc.env(name, value);
            }
        }
        Ok(())
    }
}

/// Link the compiled target (often of form `foo-{metadata_hash}`) to the
/// final target. This must happen during both "Fresh" and "Compile".
fn link_targets<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
    fresh: bool,
) -> CargoResult<Work> {
    let bcx = cx.bcx;
    let outputs = cx.outputs(unit)?;
    let export_dir = cx.files().export_dir();
    let package_id = unit.pkg.package_id();
    let profile = unit.profile;
    let unit_mode = unit.mode;
    let features = unit.features.iter().map(|s| s.to_string()).collect();
    let json_messages = bcx.build_config.emit_json();
    let executable = cx.get_executable(unit)?;
    let mut target = unit.target.clone();
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
            let dst = match output.hardlink.as_ref() {
                Some(dst) => dst,
                None => {
                    destinations.push(src.clone());
                    continue;
                }
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
            let art_profile = machine_message::ArtifactProfile {
                opt_level: profile.opt_level.as_str(),
                debuginfo: profile.debuginfo,
                debug_assertions: profile.debug_assertions,
                overflow_checks: profile.overflow_checks,
                test: unit_mode.is_any_test(),
            };

            let msg = machine_message::Artifact {
                package_id,
                target: &target,
                profile: art_profile,
                features,
                filenames: destinations,
                executable,
                fresh,
            }
            .to_json_string();
            state.stdout(msg);
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
    root_output: &PathBuf,
) -> CargoResult<()> {
    let var = util::dylib_path_envvar();
    let search_path = rustc.get_env(var).unwrap_or_default();
    let mut search_path = env::split_paths(&search_path).collect::<Vec<_>>();
    for &id in build_scripts.plugins.iter() {
        let output = build_script_outputs
            .get(&(id, CompileKind::Host))
            .ok_or_else(|| internal(format!("couldn't find libs for plugin dep {}", id)))?;
        search_path.append(&mut filter_dynamic_search_path(
            output.library_paths.iter(),
            root_output,
        ));
    }
    let search_path = join_paths(&search_path, var)?;
    rustc.env(var, &search_path);
    Ok(())
}

// Determine paths to add to the dynamic search path from -L entries
//
// Strip off prefixes like "native=" or "framework=" and filter out directories
// **not** inside our output directory since they are likely spurious and can cause
// clashes with system shared libraries (issue #3366).
fn filter_dynamic_search_path<'a, I>(paths: I, root_output: &PathBuf) -> Vec<PathBuf>
where
    I: Iterator<Item = &'a PathBuf>,
{
    let mut search_path = vec![];
    for dir in paths {
        let dir = match dir.to_str() {
            Some(s) => {
                let mut parts = s.splitn(2, '=');
                match (parts.next(), parts.next()) {
                    (Some("native"), Some(path))
                    | (Some("crate"), Some(path))
                    | (Some("dependency"), Some(path))
                    | (Some("framework"), Some(path))
                    | (Some("all"), Some(path)) => path.into(),
                    _ => dir.clone(),
                }
            }
            None => dir.clone(),
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

fn prepare_rustc<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    crate_types: &[&str],
    unit: &Unit<'a>,
) -> CargoResult<ProcessBuilder> {
    let is_primary = cx.is_primary_package(unit);

    let mut base = cx
        .compilation
        .rustc_process(unit.pkg, unit.target, is_primary)?;
    base.inherit_jobserver(&cx.jobserver);
    build_base_args(cx, &mut base, unit, crate_types)?;
    build_deps_args(&mut base, cx, unit)?;
    Ok(base)
}

fn rustdoc<'a, 'cfg>(cx: &mut Context<'a, 'cfg>, unit: &Unit<'a>) -> CargoResult<Work> {
    let bcx = cx.bcx;
    let mut rustdoc = cx.compilation.rustdoc_process(unit.pkg, unit.target)?;
    rustdoc.inherit_jobserver(&cx.jobserver);
    rustdoc.arg("--crate-name").arg(&unit.target.crate_name());
    add_path_args(bcx, unit, &mut rustdoc);
    add_cap_lints(bcx, unit, &mut rustdoc);

    if let CompileKind::Target(target) = unit.kind {
        rustdoc.arg("--target").arg(target.rustc_target());
    }

    let doc_dir = cx.files().out_dir(unit);

    // Create the documentation directory ahead of time as rustdoc currently has
    // a bug where concurrent invocations will race to create this directory if
    // it doesn't already exist.
    paths::create_dir_all(&doc_dir)?;

    rustdoc.arg("-o").arg(doc_dir);

    for feat in &unit.features {
        rustdoc.arg("--cfg").arg(&format!("feature=\"{}\"", feat));
    }

    add_error_format_and_color(cx, &mut rustdoc, false)?;

    if let Some(args) = bcx.extra_args_for(unit) {
        rustdoc.args(args);
    }

    build_deps_args(&mut rustdoc, cx, unit)?;

    rustdoc.args(bcx.rustdocflags_args(unit));

    let name = unit.pkg.name().to_string();
    let build_script_outputs = Arc::clone(&cx.build_script_outputs);
    let key = (unit.pkg.package_id(), unit.kind);
    let package_id = unit.pkg.package_id();
    let target = unit.target.clone();
    let mut output_options = OutputOptions::new(cx, unit);

    Ok(Work::new(move |state| {
        if let Some(output) = build_script_outputs.lock().unwrap().get(&key) {
            for cfg in output.cfgs.iter() {
                rustdoc.arg("--cfg").arg(cfg);
            }
            for &(ref name, ref value) in output.env.iter() {
                rustdoc.env(name, value);
            }
        }
        state.running(&rustdoc);

        rustdoc
            .exec_with_streaming(
                &mut |line| on_stdout_line(state, line, package_id, &target),
                &mut |line| on_stderr_line(state, line, package_id, &target, &mut output_options),
                false,
            )
            .chain_err(|| format!("Could not document `{}`.", name))?;
        Ok(())
    }))
}

// The path that we pass to rustc is actually fairly important because it will
// show up in error messages (important for readability), debug information
// (important for caching), etc. As a result we need to be pretty careful how we
// actually invoke rustc.
//
// In general users don't expect `cargo build` to cause rebuilds if you change
// directories. That could be if you just change directories in the package or
// if you literally move the whole package wholesale to a new directory. As a
// result we mostly don't factor in `cwd` to this calculation. Instead we try to
// track the workspace as much as possible and we update the current directory
// of rustc/rustdoc where appropriate.
//
// The first returned value here is the argument to pass to rustc, and the
// second is the cwd that rustc should operate in.
fn path_args(bcx: &BuildContext<'_, '_>, unit: &Unit<'_>) -> (PathBuf, PathBuf) {
    let ws_root = bcx.ws.root();
    let src = match unit.target.src_path() {
        TargetSourcePath::Path(path) => path.to_path_buf(),
        TargetSourcePath::Metabuild => unit.pkg.manifest().metabuild_path(bcx.ws.target_dir()),
    };
    assert!(src.is_absolute());
    if unit.pkg.package_id().source_id().is_path() {
        if let Ok(path) = src.strip_prefix(ws_root) {
            return (path.to_path_buf(), ws_root.to_path_buf());
        }
    }
    (src, unit.pkg.root().to_path_buf())
}

fn add_path_args(bcx: &BuildContext<'_, '_>, unit: &Unit<'_>, cmd: &mut ProcessBuilder) {
    let (arg, cwd) = path_args(bcx, unit);
    cmd.arg(arg);
    cmd.cwd(cwd);
}

fn add_cap_lints(bcx: &BuildContext<'_, '_>, unit: &Unit<'_>, cmd: &mut ProcessBuilder) {
    // If this is an upstream dep we don't want warnings from, turn off all
    // lints.
    if !bcx.show_warnings(unit.pkg.package_id()) {
        cmd.arg("--cap-lints").arg("allow");

    // If this is an upstream dep but we *do* want warnings, make sure that they
    // don't fail compilation.
    } else if !unit.pkg.package_id().source_id().is_path() {
        cmd.arg("--cap-lints").arg("warn");
    }
}

/// Add error-format flags to the command.
///
/// Cargo always uses JSON output. This has several benefits, such as being
/// easier to parse, handles changing formats (for replaying cached messages),
/// ensures atomic output (so messages aren't interleaved), allows for
/// intercepting messages like rmeta artifacts, etc. rustc includes a
/// "rendered" field in the JSON message with the message properly formatted,
/// which Cargo will extract and display to the user.
fn add_error_format_and_color(
    cx: &Context<'_, '_>,
    cmd: &mut ProcessBuilder,
    pipelined: bool,
) -> CargoResult<()> {
    cmd.arg("--error-format=json");
    let mut json = String::from("--json=diagnostic-rendered-ansi");
    if pipelined {
        // Pipelining needs to know when rmeta files are finished. Tell rustc
        // to emit a message that cargo will intercept.
        json.push_str(",artifacts");
    }
    match cx.bcx.build_config.message_format {
        MessageFormat::Short | MessageFormat::Json { short: true, .. } => {
            json.push_str(",diagnostic-short");
        }
        _ => {}
    }
    cmd.arg(json);
    Ok(())
}

fn build_base_args<'a, 'cfg>(
    cx: &mut Context<'a, 'cfg>,
    cmd: &mut ProcessBuilder,
    unit: &Unit<'a>,
    crate_types: &[&str],
) -> CargoResult<()> {
    assert!(!unit.mode.is_run_custom_build());

    let bcx = cx.bcx;
    let Profile {
        ref opt_level,
        ref lto,
        codegen_units,
        debuginfo,
        debug_assertions,
        overflow_checks,
        rpath,
        ref panic,
        incremental,
        ..
    } = unit.profile;
    let test = unit.mode.is_any_test();

    cmd.arg("--crate-name").arg(&unit.target.crate_name());

    add_path_args(bcx, unit, cmd);
    add_error_format_and_color(cx, cmd, cx.rmeta_required(unit))?;

    if !test {
        for crate_type in crate_types.iter() {
            cmd.arg("--crate-type").arg(crate_type);
        }
    }

    if unit.mode.is_check() {
        cmd.arg("--emit=dep-info,metadata");
    } else if !unit.requires_upstream_objects() {
        // Always produce metdata files for rlib outputs. Metadata may be used
        // in this session for a pipelined compilation, or it may be used in a
        // future Cargo session as part of a pipelined compile.
        cmd.arg("--emit=dep-info,metadata,link");
    } else {
        cmd.arg("--emit=dep-info,link");
    }

    let prefer_dynamic = (unit.target.for_host() && !unit.target.is_custom_build())
        || (crate_types.contains(&"dylib") && bcx.ws.members().any(|p| p != unit.pkg));
    if prefer_dynamic {
        cmd.arg("-C").arg("prefer-dynamic");
    }

    if opt_level.as_str() != "0" {
        cmd.arg("-C").arg(&format!("opt-level={}", opt_level));
    }

    if *panic != PanicStrategy::Unwind {
        cmd.arg("-C").arg(format!("panic={}", panic));
    }

    // Disable LTO for host builds as prefer_dynamic and it are mutually
    // exclusive.
    if unit.target.can_lto() && !unit.target.for_host() {
        match *lto {
            Lto::Bool(false) => {}
            Lto::Bool(true) => {
                cmd.args(&["-C", "lto"]);
            }
            Lto::Named(ref s) => {
                cmd.arg("-C").arg(format!("lto={}", s));
            }
        }
    }

    if let Some(n) = codegen_units {
        // There are some restrictions with LTO and codegen-units, so we
        // only add codegen units when LTO is not used.
        cmd.arg("-C").arg(&format!("codegen-units={}", n));
    }

    if let Some(debuginfo) = debuginfo {
        cmd.arg("-C").arg(format!("debuginfo={}", debuginfo));
    }

    if let Some(args) = bcx.extra_args_for(unit) {
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
            cmd.arg("-Zpanic-abort-tests");
        }
    } else if test {
        cmd.arg("--cfg").arg("test");
    }

    for feat in &unit.features {
        cmd.arg("--cfg").arg(&format!("feature=\"{}\"", feat));
    }

    match cx.files().metadata(unit) {
        Some(m) => {
            cmd.arg("-C").arg(&format!("metadata={}", m));
            cmd.arg("-C").arg(&format!("extra-filename=-{}", m));
        }
        None => {
            cmd.arg("-C")
                .arg(&format!("metadata={}", cx.files().target_short_hash(unit)));
        }
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

    opt(cmd, "-C", "ar=", bcx.ar(unit.kind).map(|s| s.as_ref()));
    opt(
        cmd,
        "-C",
        "linker=",
        bcx.linker(unit.kind).map(|s| s.as_ref()),
    );
    if incremental {
        let dir = cx.files().layout(unit.kind).incremental().as_os_str();
        opt(cmd, "-C", "incremental=", Some(dir));
    }

    if unit.is_std {
        // -Zforce-unstable-if-unmarked prevents the accidental use of
        // unstable crates within the sysroot (such as "extern crate libc" or
        // any non-public crate in the sysroot).
        //
        // RUSTC_BOOTSTRAP allows unstable features on stable.
        cmd.arg("-Zforce-unstable-if-unmarked")
            .env("RUSTC_BOOTSTRAP", "1");
    }
    Ok(())
}

fn build_deps_args<'a, 'cfg>(
    cmd: &mut ProcessBuilder,
    cx: &mut Context<'a, 'cfg>,
    unit: &Unit<'a>,
) -> CargoResult<()> {
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

    // Create Vec since mutable cx is needed in closure below.
    let deps = Vec::from(cx.unit_deps(unit));

    // If there is not one linkable target but should, rustc fails later
    // on if there is an `extern crate` for it. This may turn into a hard
    // error in the future (see PR #4797).
    if !deps
        .iter()
        .any(|dep| !dep.unit.mode.is_doc() && dep.unit.target.linkable())
    {
        if let Some(dep) = deps
            .iter()
            .find(|dep| !dep.unit.mode.is_doc() && dep.unit.target.is_lib())
        {
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

    if let Some(sysroot) = cx.files().layout(unit.kind).sysroot() {
        if !unit.kind.is_host() {
            cmd.arg("--sysroot").arg(sysroot);
        }
    }

    for dep in deps {
        if !unit.is_std && dep.unit.is_std {
            // Dependency to sysroot crate uses --sysroot.
            continue;
        }
        if dep.unit.mode.is_run_custom_build() {
            cmd.env("OUT_DIR", &cx.files().build_script_out_dir(&dep.unit));
        }
        if dep.unit.target.linkable() && !dep.unit.mode.is_doc() {
            link_to(cmd, cx, unit, &dep, &mut unstable_opts)?;
        }
    }

    // This will only be set if we're already using a feature
    // requiring nightly rust
    if unstable_opts {
        cmd.arg("-Z").arg("unstable-options");
    }

    return Ok(());

    fn link_to<'a, 'cfg>(
        cmd: &mut ProcessBuilder,
        cx: &mut Context<'a, 'cfg>,
        current: &Unit<'a>,
        dep: &UnitDep<'a>,
        need_unstable_opts: &mut bool,
    ) -> CargoResult<()> {
        let mut value = OsString::new();
        value.push(dep.extern_crate_name.as_str());
        value.push("=");

        let mut pass = |file| {
            let mut value = value.clone();
            value.push(file);

            if current
                .pkg
                .manifest()
                .features()
                .require(Feature::public_dependency())
                .is_ok()
                && !dep.public
            {
                cmd.arg("--extern-private");
                *need_unstable_opts = true;
            } else {
                cmd.arg("--extern");
            }

            cmd.arg(&value);
        };

        let outputs = cx.outputs(&dep.unit)?;
        let mut outputs = outputs.iter().filter_map(|output| match output.flavor {
            FileFlavor::Linkable { rmeta } => Some((output, rmeta)),
            _ => None,
        });

        if cx.only_requires_rmeta(current, &dep.unit) {
            let (output, _rmeta) = outputs
                .find(|(_output, rmeta)| *rmeta)
                .expect("failed to find rlib dep for pipelined dep");
            pass(&output.path);
        } else {
            for (output, rmeta) in outputs {
                if !rmeta {
                    pass(&output.path);
                }
            }
        }
        Ok(())
    }
}

fn envify(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_uppercase())
        .map(|c| if c == '-' { '_' } else { c })
        .collect()
}

struct OutputOptions {
    /// What format we're emitting from Cargo itself.
    format: MessageFormat,
    /// Look for JSON message that indicates .rmeta file is available for
    /// pipelined compilation.
    look_for_metadata_directive: bool,
    /// Whether or not to display messages in color.
    color: bool,
    /// Where to write the JSON messages to support playback later if the unit
    /// is fresh. The file is created lazily so that in the normal case, lots
    /// of empty files are not created. If this is None, the output will not
    /// be cached (such as when replaying cached messages).
    cache_cell: Option<(PathBuf, LazyCell<File>)>,
}

impl OutputOptions {
    fn new<'a>(cx: &Context<'a, '_>, unit: &Unit<'a>) -> OutputOptions {
        let look_for_metadata_directive = cx.rmeta_required(unit);
        let color = cx.bcx.config.shell().supports_color();
        let path = cx.files().message_cache_path(unit);
        // Remove old cache, ignore ENOENT, which is the common case.
        drop(fs::remove_file(&path));
        let cache_cell = Some((path, LazyCell::new()));
        OutputOptions {
            format: cx.bcx.build_config.message_format,
            look_for_metadata_directive,
            color,
            cache_cell,
        }
    }
}

fn on_stdout_line(
    state: &JobState<'_>,
    line: &str,
    _package_id: PackageId,
    _target: &Target,
) -> CargoResult<()> {
    state.stdout(line.to_string());
    Ok(())
}

fn on_stderr_line(
    state: &JobState<'_>,
    line: &str,
    package_id: PackageId,
    target: &Target,
    options: &mut OutputOptions,
) -> CargoResult<()> {
    if on_stderr_line_inner(state, line, package_id, target, options)? {
        // Check if caching is enabled.
        if let Some((path, cell)) = &mut options.cache_cell {
            // Cache the output, which will be replayed later when Fresh.
            let f = cell.try_borrow_mut_with(|| File::create(path))?;
            debug_assert!(!line.contains('\n'));
            f.write_all(line.as_bytes())?;
            f.write_all(&[b'\n'])?;
        }
    }
    Ok(())
}

/// Returns true if the line should be cached.
fn on_stderr_line_inner(
    state: &JobState<'_>,
    line: &str,
    package_id: PackageId,
    target: &Target,
    options: &mut OutputOptions,
) -> CargoResult<bool> {
    // We primarily want to use this function to process JSON messages from
    // rustc. The compiler should always print one JSON message per line, and
    // otherwise it may have other output intermingled (think RUST_LOG or
    // something like that), so skip over everything that doesn't look like a
    // JSON message.
    if !line.starts_with('{') {
        state.stderr(line.to_string());
        return Ok(true);
    }

    let mut compiler_message: Box<serde_json::value::RawValue> = match serde_json::from_str(line) {
        Ok(msg) => msg,

        // If the compiler produced a line that started with `{` but it wasn't
        // valid JSON, maybe it wasn't JSON in the first place! Forward it along
        // to stderr.
        Err(e) => {
            debug!("failed to parse json: {:?}", e);
            state.stderr(line.to_string());
            return Ok(true);
        }
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
            struct CompilerMessage {
                rendered: String,
            }
            if let Ok(mut error) = serde_json::from_str::<CompilerMessage>(compiler_message.get()) {
                // state.stderr will add a newline
                if error.rendered.ends_with('\n') {
                    error.rendered.pop();
                }
                let rendered = if options.color {
                    error.rendered
                } else {
                    // Strip only fails if the the Writer fails, which is Cursor
                    // on a Vec, which should never fail.
                    strip_ansi_escapes::strip(&error.rendered)
                        .map(|v| String::from_utf8(v).expect("utf8"))
                        .expect("strip should never fail")
                };
                state.stderr(rendered);
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
                error.rendered = strip_ansi_escapes::strip(&error.rendered)
                    .map(|v| String::from_utf8(v).expect("utf8"))
                    .unwrap_or(error.rendered);
                let new_line = serde_json::to_string(&error)?;
                let new_msg: Box<serde_json::value::RawValue> = serde_json::from_str(&new_line)?;
                compiler_message = new_msg;
            }
        }

        // If ansi colors are desired then we should be good to go! We can just
        // pass through this message as-is.
        MessageFormat::Json { ansi: true, .. } => {}
    }

    // In some modes of execution we will execute rustc with `-Z
    // emit-artifact-notifications` to look for metadata files being produced. When this
    // happens we may be able to start subsequent compilations more quickly than
    // waiting for an entire compile to finish, possibly using more parallelism
    // available to complete a compilation session more quickly.
    //
    // In these cases look for a matching directive and inform Cargo internally
    // that a metadata file has been produced.
    if options.look_for_metadata_directive {
        #[derive(serde::Deserialize)]
        struct ArtifactNotification {
            artifact: String,
        }
        if let Ok(artifact) = serde_json::from_str::<ArtifactNotification>(compiler_message.get()) {
            log::trace!("found directive from rustc: `{}`", artifact.artifact);
            if artifact.artifact.ends_with(".rmeta") {
                log::debug!("looks like metadata finished early!");
                state.rmeta_produced();
            }
            return Ok(false);
        }
    }

    // And failing all that above we should have a legitimate JSON diagnostic
    // from the compiler, so wrap it in an external Cargo JSON message
    // indicating which package it came from and then emit it.
    let msg = machine_message::FromCompiler {
        package_id,
        target,
        message: compiler_message,
    }
    .to_json_string();

    // Switch json lines from rustc/rustdoc that appear on stderr to stdout
    // instead. We want the stdout of Cargo to always be machine parseable as
    // stderr has our colorized human-readable messages.
    state.stdout(msg);
    Ok(true)
}

fn replay_output_cache(
    package_id: PackageId,
    target: &Target,
    path: PathBuf,
    format: MessageFormat,
    color: bool,
) -> Work {
    let target = target.clone();
    let mut options = OutputOptions {
        format,
        look_for_metadata_directive: true,
        color,
        cache_cell: None,
    };
    Work::new(move |state| {
        if !path.exists() {
            // No cached output, probably didn't emit anything.
            return Ok(());
        }
        let contents = fs::read_to_string(&path)?;
        for line in contents.lines() {
            on_stderr_line(state, line, package_id, &target, &mut options)?;
        }
        Ok(())
    })
}
