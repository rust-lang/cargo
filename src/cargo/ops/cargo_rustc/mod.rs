use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::path::{self, PathBuf};
use std::sync::Arc;

use serde_json;

use core::{Package, PackageId, PackageSet, Target, Resolve};
use core::{Profile, Profiles, Workspace};
use core::shell::ColorConfig;
use util::{self, CargoResult, ProcessBuilder, ProcessError, human, machine_message};
use util::{Config, internal, ChainError, profile, join_paths, short_hash};
use util::Freshness;

use self::job::{Job, Work};
use self::job_queue::JobQueue;

use self::output_depinfo::output_depinfo;

pub use self::compilation::Compilation;
pub use self::context::{Context, Unit};
pub use self::custom_build::{BuildOutput, BuildMap, BuildScripts};

mod compilation;
mod context;
mod custom_build;
mod fingerprint;
mod job;
mod job_queue;
mod layout;
mod links;
mod output_depinfo;

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy, PartialOrd, Ord)]
pub enum Kind { Host, Target }

#[derive(Default, Clone)]
pub struct BuildConfig {
    pub host_triple: String,
    pub host: TargetConfig,
    pub requested_target: Option<String>,
    pub target: TargetConfig,
    pub jobs: u32,
    pub release: bool,
    pub test: bool,
    pub doc_all: bool,
    pub json_messages: bool,
}

#[derive(Clone, Default)]
pub struct TargetConfig {
    pub ar: Option<PathBuf>,
    pub linker: Option<PathBuf>,
    pub overrides: HashMap<String, BuildOutput>,
}

pub type PackagesToBuild<'a> = [(&'a Package, Vec<(&'a Target, &'a Profile)>)];

/// A glorified callback for executing calls to rustc. Rather than calling rustc
/// directly, we'll use an Executor, giving clients an opportunity to intercept
/// the build calls.
pub trait Executor: Send + Sync + 'static {
    fn init(&self, _cx: &Context) {}
    /// If execution succeeds, the ContinueBuild value indicates whether Cargo
    /// should continue with the build process for this package.
    fn exec(&self, cmd: ProcessBuilder, _id: &PackageId) -> Result<(), ProcessError> {
        cmd.exec()?;
        Ok(())
    }

    fn exec_json(&self,
                 cmd: ProcessBuilder,
                 _id: &PackageId,
                 handle_stdout: &mut FnMut(&str) -> CargoResult<()>,
                 handle_stderr: &mut FnMut(&str) -> CargoResult<()>)
                 -> Result<(), ProcessError> {
        cmd.exec_with_streaming(handle_stdout, handle_stderr)?;
        Ok(())
    }
}

/// A DefaultExecutor calls rustc without doing anything else. It is Cargo's
/// default behaviour.
#[derive(Copy, Clone)]
pub struct DefaultExecutor;

impl Executor for DefaultExecutor {}

// Returns a mapping of the root package plus its immediate dependencies to
// where the compiled libraries are all located.
pub fn compile_targets<'a, 'cfg: 'a>(ws: &Workspace<'cfg>,
                                     pkg_targets: &'a PackagesToBuild<'a>,
                                     packages: &'a PackageSet<'cfg>,
                                     resolve: &'a Resolve,
                                     config: &'cfg Config,
                                     build_config: BuildConfig,
                                     profiles: &'a Profiles,
                                     exec: Arc<Executor>)
                                     -> CargoResult<Compilation<'cfg>> {
    let units = pkg_targets.iter().flat_map(|&(pkg, ref targets)| {
        let default_kind = if build_config.requested_target.is_some() {
            Kind::Target
        } else {
            Kind::Host
        };
        targets.iter().map(move |&(target, profile)| {
            Unit {
                pkg: pkg,
                target: target,
                profile: profile,
                kind: if target.for_host() {Kind::Host} else {default_kind},
            }
        })
    }).collect::<Vec<_>>();

    let mut cx = Context::new(ws, resolve, packages, config,
                                   build_config, profiles)?;

    let mut queue = JobQueue::new(&cx);

    cx.prepare()?;
    cx.probe_target_info(&units)?;
    cx.build_used_in_plugin_map(&units)?;
    custom_build::build_map(&mut cx, &units)?;

    for unit in units.iter() {
        // Build up a list of pending jobs, each of which represent
        // compiling a particular package. No actual work is executed as
        // part of this, that's all done next as part of the `execute`
        // function which will run everything in order with proper
        // parallelism.
        compile(&mut cx, &mut queue, unit, exec.clone())?;
    }

    // Now that we've figured out everything that we're going to do, do it!
    queue.execute(&mut cx)?;

    for unit in units.iter() {
        for (dst, link_dst, _linkable) in cx.target_filenames(unit)? {
            let bindst = match link_dst {
                Some(link_dst) => link_dst,
                None => dst.clone(),
            };

            if unit.profile.test {
                cx.compilation.tests.push((unit.pkg.clone(),
                                           unit.target.kind().clone(),
                                           unit.target.name().to_string(),
                                           dst));
            } else if unit.target.is_bin() || unit.target.is_example() {
                cx.compilation.binaries.push(bindst);
            } else if unit.target.is_lib() {
                let pkgid = unit.pkg.package_id().clone();
                cx.compilation.libraries.entry(pkgid).or_insert(HashSet::new())
                  .insert((unit.target.clone(), dst));
            }
        }

        for dep in cx.dep_targets(unit)?.iter() {
            if !unit.target.is_lib() { continue }

            if dep.profile.run_custom_build {
                let out_dir = cx.build_script_out_dir(dep).display().to_string();
                cx.compilation.extra_env.entry(dep.pkg.package_id().clone())
                  .or_insert(Vec::new())
                  .push(("OUT_DIR".to_string(), out_dir));
            }

            if !dep.target.is_lib() { continue }
            if dep.profile.doc { continue }

            let v = cx.target_filenames(dep)?;
            cx.compilation.libraries
                .entry(unit.pkg.package_id().clone())
                .or_insert(HashSet::new())
                .extend(v.into_iter().map(|(f, _, _)| {
                    (dep.target.clone(), f)
                }));
        }

        let feats = cx.resolve.features(&unit.pkg.package_id());
        cx.compilation.cfgs.entry(unit.pkg.package_id().clone())
            .or_insert_with(HashSet::new)
            .extend(feats.iter().map(|feat| format!("feature=\"{}\"", feat)));

        output_depinfo(&mut cx, unit)?;
    }

    for (&(ref pkg, _), output) in cx.build_state.outputs.lock().unwrap().iter() {
        cx.compilation.cfgs.entry(pkg.clone())
            .or_insert_with(HashSet::new)
            .extend(output.cfgs.iter().cloned());

        for dir in output.library_paths.iter() {
            cx.compilation.native_dirs.insert(dir.clone());
        }
    }
    cx.compilation.target = cx.target_triple().to_string();
    Ok(cx.compilation)
}

fn compile<'a, 'cfg: 'a>(cx: &mut Context<'a, 'cfg>,
                         jobs: &mut JobQueue<'a>,
                         unit: &Unit<'a>,
                         exec: Arc<Executor>) -> CargoResult<()> {
    if !cx.compiled.insert(*unit) {
        return Ok(())
    }

    // Build up the work to be done to compile this unit, enqueuing it once
    // we've got everything constructed.
    let p = profile::start(format!("preparing: {}/{}", unit.pkg,
                                   unit.target.name()));
    fingerprint::prepare_init(cx, unit)?;
    cx.links.validate(unit)?;

    let (dirty, fresh, freshness) = if unit.profile.run_custom_build {
        custom_build::prepare(cx, unit)?
    } else if unit.profile.doc && unit.profile.test {
        // we run these targets later, so this is just a noop for now
        (Work::new(|_| Ok(())), Work::new(|_| Ok(())), Freshness::Fresh)
    } else {
        let (freshness, dirty, fresh) = fingerprint::prepare_target(cx, unit)?;
        let work = if unit.profile.doc {
            rustdoc(cx, unit)?
        } else {
            rustc(cx, unit, exec.clone())?
        };
        // Need to link targets on both the dirty and fresh
        let dirty = work.then(link_targets(cx, unit, false)?).then(dirty);
        let fresh = link_targets(cx, unit, true)?.then(fresh);
        (dirty, fresh, freshness)
    };
    jobs.enqueue(cx, unit, Job::new(dirty, fresh), freshness)?;
    drop(p);

    // Be sure to compile all dependencies of this target as well.
    for unit in cx.dep_targets(unit)?.iter() {
        compile(cx, jobs, unit, exec.clone())?;
    }

    Ok(())
}

fn rustc(cx: &mut Context, unit: &Unit, exec: Arc<Executor>) -> CargoResult<Work> {
    let crate_types = unit.target.rustc_crate_types();
    let mut rustc = prepare_rustc(cx, crate_types, unit)?;

    let name = unit.pkg.name().to_string();

    // If this is an upstream dep we don't want warnings from, turn off all
    // lints.
    if !cx.show_warnings(unit.pkg.package_id()) {
        rustc.arg("--cap-lints").arg("allow");

    // If this is an upstream dep but we *do* want warnings, make sure that they
    // don't fail compilation.
    } else if !unit.pkg.package_id().source_id().is_path() {
        rustc.arg("--cap-lints").arg("warn");
    }

    let filenames = cx.target_filenames(unit)?;
    let root = cx.out_dir(unit);

    // Prepare the native lib state (extra -L and -l flags)
    let build_state = cx.build_state.clone();
    let current_id = unit.pkg.package_id().clone();
    let build_deps = load_build_deps(cx, unit);

    // If we are a binary and the package also contains a library, then we
    // don't pass the `-l` flags.
    let pass_l_flag = unit.target.is_lib() ||
                      !unit.pkg.targets().iter().any(|t| t.is_lib());
    let do_rename = unit.target.allows_underscores() && !unit.profile.test;
    let real_name = unit.target.name().to_string();
    let crate_name = unit.target.crate_name();

    // XXX(Rely on target_filenames iterator as source of truth rather than rederiving filestem)
    let rustc_dep_info_loc = if do_rename && cx.target_metadata(unit).is_none() {
        root.join(&crate_name)
    } else {
        root.join(&cx.file_stem(unit))
    }.with_extension("d");
    let dep_info_loc = fingerprint::dep_info_loc(cx, unit);
    let cwd = cx.config.cwd().to_path_buf();

    rustc.args(&cx.incremental_args(unit)?);
    rustc.args(&cx.rustflags_args(unit)?);
    let json_messages = cx.build_config.json_messages;
    let package_id = unit.pkg.package_id().clone();
    let target = unit.target.clone();

    exec.init(cx);
    let exec = exec.clone();

    return Ok(Work::new(move |state| {
        // Only at runtime have we discovered what the extra -L and -l
        // arguments are for native libraries, so we process those here. We
        // also need to be sure to add any -L paths for our plugins to the
        // dynamic library load path as a plugin's dynamic library may be
        // located somewhere in there.
        if let Some(build_deps) = build_deps {
            let build_state = build_state.outputs.lock().unwrap();
            add_native_deps(&mut rustc, &build_state, &build_deps,
                                 pass_l_flag, &current_id)?;
            add_plugin_deps(&mut rustc, &build_state, &build_deps)?;
        }

        // FIXME(rust-lang/rust#18913): we probably shouldn't have to do
        //                              this manually
        for &(ref filename, ref _link_dst, _linkable) in filenames.iter() {
            let mut dsts = vec![root.join(filename)];
            // If there is both an rmeta and rlib, rustc will prefer to use the
            // rlib, even if it is older. Therefore, we must delete the rlib to
            // force using the new rmeta.
            if dsts[0].extension() == Some(&OsStr::new("rmeta")) {
                dsts.push(root.join(filename).with_extension("rlib"));
            }
            for dst in &dsts {
                if fs::metadata(dst).is_ok() {
                    fs::remove_file(dst).chain_error(|| {
                        human(format!("Could not remove file: {}.", dst.display()))
                    })?;
                }
            }
        }

        state.running(&rustc);
        if json_messages {
            exec.exec_json(rustc, &package_id,
                &mut |line| if !line.is_empty() {
                    Err(internal(&format!("compiler stdout is not empty: `{}`", line)))
                } else {
                    Ok(())
                },
                &mut |line| {
                    // stderr from rustc can have a mix of JSON and non-JSON output
                    if line.starts_with('{') {
                        // Handle JSON lines
                        let compiler_message = serde_json::from_str(line).map_err(|_| {
                            internal(&format!("compiler produced invalid json: `{}`", line))
                        })?;

                        machine_message::emit(machine_message::FromCompiler {
                            package_id: &package_id,
                            target: &target,
                            message: compiler_message,
                        });
                    } else {
                        // Forward non-JSON to stderr
                        writeln!(io::stderr(), "{}", line)?;
                    }
                    Ok(())
                }
            ).chain_error(|| {
                human(format!("Could not compile `{}`.", name))
            })?;
        } else {
            exec.exec(rustc, &package_id).chain_error(|| {
                human(format!("Could not compile `{}`.", name))
            })?;
        }

        if do_rename && real_name != crate_name {
            let dst = &filenames[0].0;
            let src = dst.with_file_name(dst.file_name().unwrap()
                                            .to_str().unwrap()
                                            .replace(&real_name, &crate_name));
            if src.exists() && src.file_name() != dst.file_name() {
                fs::rename(&src, &dst).chain_error(|| {
                    internal(format!("could not rename crate {:?}", src))
                })?;
            }
        }

        if fs::metadata(&rustc_dep_info_loc).is_ok() {
            info!("Renaming dep_info {:?} to {:?}", rustc_dep_info_loc, dep_info_loc);
            fs::rename(&rustc_dep_info_loc, &dep_info_loc).chain_error(|| {
                internal(format!("could not rename dep info: {:?}",
                              rustc_dep_info_loc))
            })?;
            fingerprint::append_current_dir(&dep_info_loc, &cwd)?;
        }

        Ok(())
    }));

    // Add all relevant -L and -l flags from dependencies (now calculated and
    // present in `state`) to the command provided
    fn add_native_deps(rustc: &mut ProcessBuilder,
                       build_state: &BuildMap,
                       build_scripts: &BuildScripts,
                       pass_l_flag: bool,
                       current_id: &PackageId) -> CargoResult<()> {
        for key in build_scripts.to_link.iter() {
            let output = build_state.get(key).chain_error(|| {
                internal(format!("couldn't find build state for {}/{:?}",
                                 key.0, key.1))
            })?;
            for path in output.library_paths.iter() {
                rustc.arg("-L").arg(path);
            }
            if key.0 == *current_id {
                for cfg in &output.cfgs {
                    rustc.arg("--cfg").arg(cfg);
                }
                if pass_l_flag {
                    for name in output.library_links.iter() {
                        rustc.arg("-l").arg(name);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Link the compiled target (often of form foo-{metadata_hash}) to the
/// final target. This must happen during both "Fresh" and "Compile"
fn link_targets(cx: &mut Context, unit: &Unit, fresh: bool) -> CargoResult<Work> {
    let filenames = cx.target_filenames(unit)?;
    let package_id = unit.pkg.package_id().clone();
    let target = unit.target.clone();
    let profile = unit.profile.clone();
    let features = cx.resolve.features_sorted(&package_id).into_iter()
        .map(|s| s.to_owned())
        .collect();
    let json_messages = cx.build_config.json_messages;

    Ok(Work::new(move |_| {
        // If we're a "root crate", e.g. the target of this compilation, then we
        // hard link our outputs out of the `deps` directory into the directory
        // above. This means that `cargo build` will produce binaries in
        // `target/debug` which one probably expects.
        let mut destinations = vec![];
        for &(ref src, ref link_dst, _linkable) in filenames.iter() {
            // This may have been a `cargo rustc` command which changes the
            // output, so the source may not actually exist.
            if !src.exists() {
                continue
            }
            let dst = match link_dst.as_ref() {
                Some(dst) => dst,
                None => {
                    destinations.push(src.display().to_string());
                    continue;
                }
            };
            destinations.push(dst.display().to_string());

            debug!("linking {} to {}", src.display(), dst.display());
            if dst.exists() {
                fs::remove_file(&dst).chain_error(|| {
                    human(format!("failed to remove: {}", dst.display()))
                })?;
            }
            fs::hard_link(src, dst)
                 .or_else(|err| {
                     debug!("hard link failed {}. falling back to fs::copy", err);
                     fs::copy(src, dst).map(|_| ())
                 })
                 .chain_error(|| {
                     human(format!("failed to link or copy `{}` to `{}`",
                                   src.display(), dst.display()))
            })?;
        }

        if json_messages {
            machine_message::emit(machine_message::Artifact {
                package_id: &package_id,
                target: &target,
                profile: &profile,
                features: features,
                filenames: destinations,
                fresh: fresh,
            });
        }
        Ok(())
    }))
}

fn load_build_deps(cx: &Context, unit: &Unit) -> Option<Arc<BuildScripts>> {
    cx.build_scripts.get(unit).cloned()
}

// For all plugin dependencies, add their -L paths (now calculated and
// present in `state`) to the dynamic library load path for the command to
// execute.
fn add_plugin_deps(rustc: &mut ProcessBuilder,
                   build_state: &BuildMap,
                   build_scripts: &BuildScripts)
                   -> CargoResult<()> {
    let var = util::dylib_path_envvar();
    let search_path = rustc.get_env(var).unwrap_or(OsString::new());
    let mut search_path = env::split_paths(&search_path).collect::<Vec<_>>();
    for id in build_scripts.plugins.iter() {
        let key = (id.clone(), Kind::Host);
        let output = build_state.get(&key).chain_error(|| {
            internal(format!("couldn't find libs for plugin dep {}", id))
        })?;
        for path in output.library_paths.iter() {
            search_path.push(path.clone());
        }
    }
    let search_path = join_paths(&search_path, var)?;
    rustc.env(var, &search_path);
    Ok(())
}

fn prepare_rustc(cx: &mut Context,
                 crate_types: Vec<&str>,
                 unit: &Unit) -> CargoResult<ProcessBuilder> {
    let mut base = cx.compilation.rustc_process(unit.pkg)?;
    build_base_args(cx, &mut base, unit, &crate_types);
    build_deps_args(&mut base, cx, unit)?;
    Ok(base)
}


fn rustdoc(cx: &mut Context, unit: &Unit) -> CargoResult<Work> {
    let mut rustdoc = cx.compilation.rustdoc_process(unit.pkg)?;
    rustdoc.arg("--crate-name").arg(&unit.target.crate_name())
           .cwd(cx.config.cwd())
           .arg(&root_path(cx, unit));

    if unit.kind != Kind::Host {
        if let Some(target) = cx.requested_target() {
            rustdoc.arg("--target").arg(target);
        }
    }

    let doc_dir = cx.out_dir(unit);

    // Create the documentation directory ahead of time as rustdoc currently has
    // a bug where concurrent invocations will race to create this directory if
    // it doesn't already exist.
    fs::create_dir_all(&doc_dir)?;

    rustdoc.arg("-o").arg(doc_dir);

    for feat in cx.resolve.features(unit.pkg.package_id()) {
        rustdoc.arg("--cfg").arg(&format!("feature=\"{}\"", feat));
    }

    if let Some(ref args) = unit.profile.rustdoc_args {
        rustdoc.args(args);
    }

    build_deps_args(&mut rustdoc, cx, unit)?;

    rustdoc.args(&cx.rustdocflags_args(unit)?);

    let name = unit.pkg.name().to_string();
    let build_state = cx.build_state.clone();
    let key = (unit.pkg.package_id().clone(), unit.kind);

    Ok(Work::new(move |state| {
        if let Some(output) = build_state.outputs.lock().unwrap().get(&key) {
            for cfg in output.cfgs.iter() {
                rustdoc.arg("--cfg").arg(cfg);
            }
        }
        state.running(&rustdoc);
        rustdoc.exec().chain_error(|| {
            human(format!("Could not document `{}`.", name))
        })
    }))
}

// The path that we pass to rustc is actually fairly important because it will
// show up in error messages and the like. For this reason we take a few moments
// to ensure that something shows up pretty reasonably.
//
// The heuristic here is fairly simple, but the key idea is that the path is
// always "relative" to the current directory in order to be found easily. The
// path is only actually relative if the current directory is an ancestor if it.
// This means that non-path dependencies (git/registry) will likely be shown as
// absolute paths instead of relative paths.
fn root_path(cx: &Context, unit: &Unit) -> PathBuf {
    let absolute = unit.pkg.root().join(unit.target.src_path());
    let cwd = cx.config.cwd();
    if absolute.starts_with(cwd) {
        util::without_prefix(&absolute, cwd).map(|s| {
            s.to_path_buf()
        }).unwrap_or(absolute)
    } else {
        absolute
    }
}

fn build_base_args(cx: &mut Context,
                   cmd: &mut ProcessBuilder,
                   unit: &Unit,
                   crate_types: &[&str]) {
    let Profile {
        ref opt_level, lto, codegen_units, ref rustc_args, debuginfo,
        debug_assertions, overflow_checks, rpath, test, doc: _doc,
        run_custom_build, ref panic, rustdoc_args: _, check,
    } = *unit.profile;
    assert!(!run_custom_build);

    // Move to cwd so the root_path() passed below is actually correct
    cmd.cwd(cx.config.cwd());

    cmd.arg("--crate-name").arg(&unit.target.crate_name());

    cmd.arg(&root_path(cx, unit));

    let color_config = cx.config.shell().color_config();
    if color_config != ColorConfig::Auto {
        cmd.arg("--color").arg(&color_config.to_string());
    }

    if cx.build_config.json_messages {
        cmd.arg("--error-format").arg("json");
    }

    if !test {
        for crate_type in crate_types.iter() {
            cmd.arg("--crate-type").arg(crate_type);
        }
    }

    if check {
        cmd.arg("--emit=dep-info,metadata");
    } else {
        cmd.arg("--emit=dep-info,link");
    }

    let prefer_dynamic = (unit.target.for_host() &&
                          !unit.target.is_custom_build()) ||
                         (crate_types.contains(&"dylib") &&
                          cx.ws.members().find(|&p| p != unit.pkg).is_some());
    if prefer_dynamic {
        cmd.arg("-C").arg("prefer-dynamic");
    }

    if opt_level != "0" {
        cmd.arg("-C").arg(&format!("opt-level={}", opt_level));
    }

    // If a panic mode was configured *and* we're not ever going to be used in a
    // plugin, then we can compile with that panic mode.
    //
    // If we're used in a plugin then we'll eventually be linked to libsyntax
    // most likely which isn't compiled with a custom panic mode, so we'll just
    // get an error if we actually compile with that. This fixes `panic=abort`
    // crates which have plugin dependencies, but unfortunately means that
    // dependencies shared between the main application and plugins must be
    // compiled without `panic=abort`. This isn't so bad, though, as the main
    // application will still be compiled with `panic=abort`.
    if let Some(panic) = panic.as_ref() {
        if !cx.used_in_plugin.contains(unit) {
            cmd.arg("-C").arg(format!("panic={}", panic));
        }
    }

    // Disable LTO for host builds as prefer_dynamic and it are mutually
    // exclusive.
    if unit.target.can_lto() && lto && !unit.target.for_host() {
        cmd.args(&["-C", "lto"]);
    } else {
        // There are some restrictions with LTO and codegen-units, so we
        // only add codegen units when LTO is not used.
        if let Some(n) = codegen_units {
            cmd.arg("-C").arg(&format!("codegen-units={}", n));
        }
    }

    if let Some(debuginfo) = debuginfo {
        cmd.arg("-C").arg(format!("debuginfo={}", debuginfo));
    }

    if let Some(ref args) = *rustc_args {
        cmd.args(args);
    }

    // -C overflow-checks is implied by the setting of -C debug-assertions,
    // so we only need to provide -C overflow-checks if it differs from
    // the value of -C debug-assertions we would provide.
    if opt_level != "0" {
        if debug_assertions {
            cmd.args(&["-C", "debug-assertions=on"]);
            if !overflow_checks {
                cmd.args(&["-C", "overflow-checks=off"]);
            }
        } else if overflow_checks {
            cmd.args(&["-C", "overflow-checks=on"]);
        }
    } else {
        if !debug_assertions {
            cmd.args(&["-C", "debug-assertions=off"]);
            if overflow_checks {
                cmd.args(&["-C", "overflow-checks=on"]);
            }
        } else if !overflow_checks {
            cmd.args(&["-C", "overflow-checks=off"]);
        }
    }

    if test && unit.target.harness() {
        cmd.arg("--test");
    } else if test {
        cmd.arg("--cfg").arg("test");
    }

    for feat in cx.resolve.features(unit.pkg.package_id()).iter() {
        cmd.arg("--cfg").arg(&format!("feature=\"{}\"", feat));
    }

    match cx.target_metadata(unit) {
        Some(m) => {
            cmd.arg("-C").arg(&format!("metadata={}", m));
            cmd.arg("-C").arg(&format!("extra-filename=-{}", m));
        }
        None => {
            cmd.arg("-C").arg(&format!("metadata={}", short_hash(unit.pkg)));
        }
    }

    if rpath {
        cmd.arg("-C").arg("rpath");
    }

    cmd.arg("--out-dir").arg(&cx.out_dir(unit));

    fn opt(cmd: &mut ProcessBuilder, key: &str, prefix: &str,
           val: Option<&OsStr>)  {
        if let Some(val) = val {
            let mut joined = OsString::from(prefix);
            joined.push(val);
            cmd.arg(key).arg(joined);
        }
    }

    if unit.kind == Kind::Target {
        opt(cmd, "--target", "", cx.requested_target().map(|s| s.as_ref()));
    }

    opt(cmd, "-C", "ar=", cx.ar(unit.kind).map(|s| s.as_ref()));
    opt(cmd, "-C", "linker=", cx.linker(unit.kind).map(|s| s.as_ref()));
}


fn build_deps_args(cmd: &mut ProcessBuilder, cx: &mut Context, unit: &Unit)
                   -> CargoResult<()> {
    cmd.arg("-L").arg(&{
        let mut deps = OsString::from("dependency=");
        deps.push(cx.deps_dir(unit));
        deps
    });

    // Be sure that the host path is also listed. This'll ensure that proc-macro
    // dependencies are correctly found (for reexported macros).
    if let Kind::Target = unit.kind {
        cmd.arg("-L").arg(&{
            let mut deps = OsString::from("dependency=");
            deps.push(cx.host_deps());
            deps
        });
    }

    for unit in cx.dep_targets(unit)?.iter() {
        if unit.profile.run_custom_build {
            cmd.env("OUT_DIR", &cx.build_script_out_dir(unit));
        }
        if unit.target.linkable() && !unit.profile.doc {
            link_to(cmd, cx, unit)?;
        }
    }

    return Ok(());

    fn link_to(cmd: &mut ProcessBuilder, cx: &mut Context, unit: &Unit)
               -> CargoResult<()> {
        for (dst, _link_dst, linkable) in cx.target_filenames(unit)? {
            if !linkable {
                continue
            }
            let mut v = OsString::new();
            v.push(&unit.target.crate_name());
            v.push("=");
            v.push(cx.out_dir(unit));
            v.push(&path::MAIN_SEPARATOR.to_string());
            v.push(&dst.file_name().unwrap());
            cmd.arg("--extern").arg(&v);
        }
        Ok(())
    }
}

fn envify(s: &str) -> String {
    s.chars()
     .flat_map(|c| c.to_uppercase())
     .map(|c| if c == '-' {'_'} else {c})
     .collect()
}

impl Kind {
    fn for_target(&self, target: &Target) -> Kind {
        // Once we start compiling for the `Host` kind we continue doing so, but
        // if we are a `Target` kind and then we start compiling for a target
        // that needs to be on the host we lift ourselves up to `Host`
        match *self {
            Kind::Host => Kind::Host,
            Kind::Target if target.for_host() => Kind::Host,
            Kind::Target => Kind::Target,
        }
    }
}
