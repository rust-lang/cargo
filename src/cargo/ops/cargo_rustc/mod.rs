use std::collections::{HashSet, HashMap};
use std::dynamic_lib::DynamicLibrary;
use std::ffi::CString;
use std::old_io::fs::{self, PathExtensions};
use std::os;
use std::old_path;
use std::sync::Arc;

use core::{SourceMap, Package, PackageId, PackageSet, Target, Resolve};
use util::{self, CargoResult, human, caused_human};
use util::{Config, internal, ChainError, Fresh, profile, join_paths};

use self::job::{Job, Work};
use self::job_queue::{JobQueue, Stage};

pub use self::compilation::Compilation;
pub use self::context::Context;
pub use self::context::Platform;
pub use self::engine::{CommandPrototype, CommandType, ExecEngine, ProcessEngine};
pub use self::layout::{Layout, LayoutProxy};
pub use self::custom_build::{BuildOutput, BuildMap};

mod context;
mod compilation;
mod custom_build;
mod engine;
mod fingerprint;
mod job;
mod job_queue;
mod layout;
mod links;

#[derive(PartialEq, Eq, Hash, Debug, Copy)]
pub enum Kind { Host, Target }

#[derive(Default, Clone)]
pub struct BuildConfig {
    pub host: TargetConfig,
    pub target: TargetConfig,
    pub jobs: u32,
    pub requested_target: Option<String>,
}

#[derive(Clone, Default)]
pub struct TargetConfig {
    pub ar: Option<String>,
    pub linker: Option<String>,
    pub overrides: HashMap<String, BuildOutput>,
}

/// Run `rustc` to figure out what its current version string is.
///
/// The second element of the tuple returned is the target triple that rustc
/// is a host for.
pub fn rustc_version() -> CargoResult<(String, String)> {
    rustc_new_version().or_else(|_| rustc_old_version())
}

pub fn rustc_old_version() -> CargoResult<(String, String)> {
    let output = try!(try!(util::process("rustc"))
        .arg("-v")
        .arg("verbose")
        .exec_with_output());
    let output = try!(String::from_utf8(output.output).map_err(|_| {
        internal("rustc -v didn't return utf8 output")
    }));
    let triple = {
        let triple = output.lines().filter(|l| {
            l.starts_with("host: ")
        }).map(|l| &l[6..]).next();
        let triple = try!(triple.chain_error(|| {
            internal("rustc -v didn't have a line for `host:`")
        }));
        triple.to_string()
    };
    Ok((output, triple))
}

pub fn rustc_new_version() -> CargoResult<(String, String)> {
    let output = try!(try!(util::process("rustc"))
        .arg("-vV")
        .exec_with_output());
    let output = try!(String::from_utf8(output.output).map_err(|_| {
        internal("rustc -v didn't return utf8 output")
    }));
    let triple = {
        let triple = output.lines().filter(|l| {
            l.starts_with("host: ")
        }).map(|l| &l[6..]).next();
        let triple = try!(triple.chain_error(|| {
            internal("rustc -v didn't have a line for `host:`")
        }));
        triple.to_string()
    };
    Ok((output, triple))
}

// This is a temporary assert that ensures the consistency of the arguments
// given the current limitations of Cargo. The long term fix is to have each
// Target know the absolute path to the build location.
fn uniq_target_dest<'a>(targets: &[&'a Target]) -> Option<&'a str> {
    let mut curr: Option<Option<&str>> = None;

    for t in targets.iter().filter(|t| !t.profile().is_custom_build()) {
        let dest = t.profile().dest();

        match curr {
            Some(curr) => assert!(curr == dest),
            None => curr = Some(dest)
        }
    }

    curr.unwrap()
}

// Returns a mapping of the root package plus its immediate dependencies to
// where the compiled libraries are all located.
pub fn compile_targets<'a, 'b>(env: &str,
                               targets: &[&'a Target],
                               pkg: &'a Package,
                               deps: &PackageSet,
                               resolve: &'a Resolve,
                               sources: &'a SourceMap<'a>,
                               config: &'a Config<'b>,
                               build_config: BuildConfig,
                               exec_engine: Option<Arc<Box<ExecEngine>>>)
                               -> CargoResult<Compilation> {
    if targets.is_empty() {
        return Ok(Compilation::new(pkg))
    }

    debug!("compile_targets: {}", pkg);

    try!(links::validate(deps));

    let dest = uniq_target_dest(targets);
    let root = if resolve.root() == pkg.package_id() {
        pkg
    } else {
        deps.iter().find(|p| p.package_id() == resolve.root()).unwrap()
    };
    let host_layout = Layout::new(root, None, dest);
    let target_layout = build_config.requested_target.as_ref().map(|target| {
        layout::Layout::new(root, Some(&target[]), dest)
    });

    let mut cx = try!(Context::new(env, resolve, sources, deps, config,
                                   host_layout, target_layout, pkg,
                                   build_config));
    if let Some(exec_engine) = exec_engine {
        cx.exec_engine = exec_engine.clone();
    }

    let mut queue = JobQueue::new(cx.resolve, deps, cx.jobs());

    // First ensure that the destination directory exists
    try!(cx.prepare(pkg));

    // Build up a list of pending jobs, each of which represent compiling a
    // particular package. No actual work is executed as part of this, that's
    // all done later as part of the `execute` function which will run
    // everything in order with proper parallelism.
    let mut compiled = HashSet::new();
    each_dep(pkg, &cx, |dep| {
        compiled.insert(dep.package_id().clone());
    });
    for dep in deps.iter() {
        if dep == pkg { continue }

        // Only compile lib targets for dependencies
        let targets = dep.targets().iter().filter(|target| {
            target.profile().is_custom_build() ||
                cx.is_relevant_target(*target)
        }).collect::<Vec<&Target>>();

        if targets.len() == 0 && dep.package_id() != resolve.root() {
            return Err(human(format!("Package `{}` has no library targets", dep)))
        }

        let compiled = compiled.contains(dep.package_id());
        try!(compile(&targets, dep, compiled, &mut cx, &mut queue));
    }

    try!(compile(targets, pkg, true, &mut cx, &mut queue));

    // Now that we've figured out everything that we're going to do, do it!
    try!(queue.execute(cx.config));

    let out_dir = cx.layout(pkg, Kind::Target).build_out(pkg)
                    .display().to_string();
    cx.compilation.extra_env.insert("OUT_DIR".to_string(), Some(out_dir));
    for (&(ref pkg, _), output) in cx.build_state.outputs.lock().unwrap().iter() {
        let any_dylib = output.library_links.iter().any(|l| {
            !l.ends_with(":static") && !l.ends_with(":framework")
        });
        if !any_dylib { continue }
        for dir in output.library_paths.iter() {
            cx.compilation.native_dirs.insert(pkg.clone(), dir.clone());
        }
    }
    Ok(cx.compilation)
}

fn compile<'a, 'b>(targets: &[&'a Target], pkg: &'a Package,
                   compiled: bool,
                   cx: &mut Context<'a, 'b>,
                   jobs: &mut JobQueue<'a, 'b>) -> CargoResult<()> {
    debug!("compile_pkg; pkg={}", pkg);
    let _p = profile::start(format!("preparing: {}", pkg));

    // Packages/targets which are actually getting compiled are constructed into
    // a real job. Packages which are *not* compiled still have their jobs
    // executed, but only if the work is fresh. This is to preserve their
    // artifacts if any exist.
    let job = if compiled {
        Job::new as fn(Work, Work) -> Job
    } else {
        Job::noop as fn(Work, Work) -> Job
    };

    if !compiled { jobs.ignore(pkg); }

    if targets.is_empty() {
        return Ok(())
    }

    // Prepare the fingerprint directory as the first step of building a package
    let (target1, target2) = fingerprint::prepare_init(cx, pkg, Kind::Target);
    let mut init = vec![(Job::new(target1, target2), Fresh)];
    if cx.requested_target().is_some() {
        let (plugin1, plugin2) = fingerprint::prepare_init(cx, pkg, Kind::Host);
        init.push((Job::new(plugin1, plugin2), Fresh));
    }
    jobs.enqueue(pkg, Stage::Start, init);

    // After the custom command has run, execute rustc for all targets of our
    // package.
    //
    // Each target has its own concept of freshness to ensure incremental
    // rebuilds on the *target* granularity, not the *package* granularity.
    let (mut libs, mut bins, mut lib_tests, mut bin_tests) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut build_custom, mut run_custom) = (Vec::new(), Vec::new());
    for &target in targets.iter() {
        let work = if target.profile().is_doc() {
            let rustdoc = try!(rustdoc(pkg, target, cx));
            vec![(rustdoc, Kind::Target)]
        } else {
            let req = cx.get_requirement(pkg, target);
            try!(rustc(pkg, target, cx, req))
        };

        // Figure out what stage this work will go into
        let dst = match (target.is_lib(),
                         target.profile().is_test(),
                         target.profile().is_custom_build()) {
            (_, _, true) => &mut build_custom,
            (true, true, _) => &mut lib_tests,
            (false, true, _) => &mut bin_tests,
            (true, false, _) => &mut libs,
            (false, false, _) if target.profile().env() == "test" => &mut bin_tests,
            (false, false, _) => &mut bins,
        };
        for (work, kind) in work.into_iter() {
            let (freshness, dirty, fresh) =
                try!(fingerprint::prepare_target(cx, pkg, target, kind));

            let dirty = Work::new(move |desc_tx| {
                try!(work.call(desc_tx.clone()));
                dirty.call(desc_tx)
            });
            dst.push((job(dirty, fresh), freshness));
        }

        // If this is a custom build command, we need to not only build the
        // script but we also need to run it. Note that this is a little nuanced
        // because we may need to run the build script multiple times. If the
        // package is needed in both a host and target context, we need to run
        // it once per context.
        if !target.profile().is_custom_build() { continue }
        let mut reqs = Vec::new();
        let requirement = targets.iter().fold(None::<Platform>, |req, t| {
            if !t.profile().is_custom_build() && !t.profile().is_doc() {
                let r2 = cx.get_requirement(pkg, *t);
                req.map(|r| r.combine(r2)).or(Some(r2))
            } else {
                req
            }
        }).unwrap_or(Platform::Target);
        match requirement {
            Platform::Target => reqs.push(Platform::Target),
            Platform::Plugin => reqs.push(Platform::Plugin),
            Platform::PluginAndTarget => {
                if cx.requested_target().is_some() {
                    reqs.push(Platform::Plugin);
                    reqs.push(Platform::Target);
                } else {
                    reqs.push(Platform::PluginAndTarget);
                }
            }
        }
        let before = run_custom.len();
        for &req in reqs.iter() {
            let kind = match req { Platform::Plugin => Kind::Host, _ => Kind::Target };
            let key = (pkg.package_id().clone(), kind);
            if pkg.manifest().links().is_some() &&
                cx.build_state.outputs.lock().unwrap().contains_key(&key) {
                    continue
                }
            let (dirty, fresh, freshness) =
                try!(custom_build::prepare(pkg, target, req, cx));
            run_custom.push((job(dirty, fresh), freshness));
        }

        // If no build scripts were run, no need to compile the build script!
        if run_custom.len() == before {
            dst.pop();
        }
    }

    jobs.enqueue(pkg, Stage::BuildCustomBuild, build_custom);
    jobs.enqueue(pkg, Stage::RunCustomBuild, run_custom);
    jobs.enqueue(pkg, Stage::Libraries, libs);
    jobs.enqueue(pkg, Stage::Binaries, bins);
    jobs.enqueue(pkg, Stage::BinaryTests, bin_tests);
    jobs.enqueue(pkg, Stage::LibraryTests, lib_tests);
    Ok(())
}

fn rustc(package: &Package, target: &Target,
         cx: &mut Context, req: Platform)
         -> CargoResult<Vec<(Work, Kind)> >{
    let crate_types = target.rustc_crate_types();
    let rustcs = try!(prepare_rustc(package, target, crate_types, cx, req));

    let plugin_deps = crawl_build_deps(cx, package, target, Kind::Host);

    return rustcs.into_iter().map(|(rustc, kind)| {
        let name = package.name().to_string();
        let is_path_source = package.package_id().source_id().is_path();
        let show_warnings = package.package_id() == cx.resolve.root() ||
                            is_path_source;
        let rustc = if show_warnings {rustc} else {rustc.arg("-Awarnings")};
        let exec_engine = cx.exec_engine.clone();

        let filenames = try!(cx.target_filenames(target));
        let root = cx.out_dir(package, kind, target);

        // Prepare the native lib state (extra -L and -l flags)
        let build_state = cx.build_state.clone();
        let current_id = package.package_id().clone();
        let plugin_deps = plugin_deps.clone();
        let mut native_lib_deps = crawl_build_deps(cx, package, target, kind);
        if package.has_custom_build() && !target.profile().is_custom_build() {
            native_lib_deps.insert(0, current_id.clone());
        }

        // If we are a binary and the package also contains a library, then we
        // don't pass the `-l` flags.
        let pass_l_flag = target.is_lib() || !package.targets().iter().any(|t| {
            t.is_lib()
        });

        let rustc_dep_info_loc = root.join(target.file_stem()).with_extension("d");
        let dep_info_loc = fingerprint::dep_info_loc(cx, package, target, kind);
        let cwd = cx.config.cwd().clone();

        Ok((Work::new(move |desc_tx| {
            let mut rustc = rustc;
            debug!("about to run: {}", rustc);

            // Only at runtime have we discovered what the extra -L and -l
            // arguments are for native libraries, so we process those here. We
            // also need to be sure to add any -L paths for our plugins to the
            // dynamic library load path as a plugin's dynamic library may be
            // located somewhere in there.
            let build_state = build_state.outputs.lock().unwrap();
            rustc = add_native_deps(rustc, &*build_state, native_lib_deps,
                                    kind, pass_l_flag, &current_id);
            rustc = try!(add_plugin_deps(rustc, &*build_state, plugin_deps));
            drop(build_state);

            // FIXME(rust-lang/rust#18913): we probably shouldn't have to do
            //                              this manually
            for filename in filenames.iter() {
                let dst = root.join(filename);
                if dst.exists() {
                    try!(fs::unlink(&dst));
                }
            }

            desc_tx.send(rustc.to_string()).ok();
            try!(exec_engine.exec(rustc).chain_error(|| {
                human(format!("Could not compile `{}`.", name))
            }));

            try!(fs::rename(&rustc_dep_info_loc, &dep_info_loc));
            try!(fingerprint::append_current_dir(&dep_info_loc, &cwd));

            Ok(())

        }), kind))
    }).collect();

    // Add all relevant -L and -l flags from dependencies (now calculated and
    // present in `state`) to the command provided
    fn add_native_deps(mut rustc: CommandPrototype,
                       build_state: &BuildMap,
                       native_lib_deps: Vec<PackageId>,
                       kind: Kind,
                       pass_l_flag: bool,
                       current_id: &PackageId) -> CommandPrototype {
        for id in native_lib_deps.into_iter() {
            debug!("looking up {} {:?}", id, kind);
            let output = &build_state[(id.clone(), kind)];
            for path in output.library_paths.iter() {
                rustc = rustc.arg("-L").arg(path);
            }
            if pass_l_flag && id == *current_id {
                for name in output.library_links.iter() {
                    rustc = rustc.arg("-l").arg(name);
                }
            }
        }
        return rustc;
    }
}

fn crawl_build_deps<'a>(cx: &'a Context, pkg: &'a Package,
                        target: &Target, kind: Kind) -> Vec<PackageId> {
    let mut deps = HashSet::new();
    visit(cx, pkg, target, kind, &mut HashSet::new(), &mut deps);
    let mut ret: Vec<_> = deps.into_iter().collect();
    ret.sort();
    return ret;

    fn visit<'a>(cx: &'a Context, pkg: &'a Package, target: &Target,
                 kind: Kind,
                 visiting: &mut HashSet<&'a PackageId>,
                 libs: &mut HashSet<PackageId>) {
        for &(pkg, target) in cx.dep_targets(pkg, target).iter() {
            let req = cx.get_requirement(pkg, target);
            if !req.includes(kind) { continue }
            if !visiting.insert(pkg.package_id()) { continue }

            if pkg.has_custom_build() {
                libs.insert(pkg.package_id().clone());
            }
            visit(cx, pkg, target, kind, visiting, libs);
            visiting.remove(&pkg.package_id());
        }
    }
}

// For all plugin dependencies, add their -L paths (now calculated and
// present in `state`) to the dynamic library load path for the command to
// execute.
#[allow(deprecated)] // need an OsStr based Command
fn add_plugin_deps(rustc: CommandPrototype,
                   build_state: &BuildMap,
                   plugin_deps: Vec<PackageId>)
                   -> CargoResult<CommandPrototype> {
    let var = DynamicLibrary::envvar();
    let search_path = rustc.get_env(var)
                           .unwrap_or(CString::from_slice(b""));
    let mut search_path = os::split_paths(search_path);
    for id in plugin_deps.into_iter() {
        let output = &build_state[(id, Kind::Host)];
        for path in output.library_paths.iter() {
            search_path.push(path.clone());
        }
    }
    let search_path = try!(join_paths(&search_path[], var));
    Ok(rustc.env(var, Some(search_path)))
}

fn prepare_rustc(package: &Package, target: &Target, crate_types: Vec<&str>,
                 cx: &Context, req: Platform)
                 -> CargoResult<Vec<(CommandPrototype, Kind)>> {
    let base = try!(process(CommandType::Rustc, package, target, cx));
    let base = build_base_args(cx, base, package, target, &crate_types);

    let target_cmd = build_plugin_args(base.clone(), cx, package, target, Kind::Target);
    let plugin_cmd = build_plugin_args(base, cx, package, target, Kind::Host);
    let target_cmd = try!(build_deps_args(target_cmd, target, package, cx,
                                          Kind::Target));
    let plugin_cmd = try!(build_deps_args(plugin_cmd, target, package, cx,
                                          Kind::Host));

    Ok(match req {
        Platform::Target => vec![(target_cmd, Kind::Target)],
        Platform::Plugin => vec![(plugin_cmd, Kind::Host)],
        Platform::PluginAndTarget if cx.requested_target().is_none() =>
            vec![(target_cmd, Kind::Target)],
        Platform::PluginAndTarget => vec![(target_cmd, Kind::Target),
                                          (plugin_cmd, Kind::Host)],
    })
}


fn rustdoc(package: &Package, target: &Target,
           cx: &mut Context) -> CargoResult<Work> {
    let kind = Kind::Target;
    let cx_root = cx.layout(package, kind).proxy().dest().join("doc");
    let rustdoc = try!(process(CommandType::Rustdoc, package, target, cx));
    let mut rustdoc = rustdoc.arg(root_path(cx, package, target))
                         .cwd(cx.config.cwd().clone())
                         .arg("-o").arg(cx_root)
                         .arg("--crate-name").arg(target.name());

    match cx.resolve.features(package.package_id()) {
        Some(features) => {
            for feat in features.iter() {
                rustdoc = rustdoc.arg("--cfg").arg(format!("feature=\"{}\"", feat));
            }
        }
        None => {}
    }

    let mut rustdoc = try!(build_deps_args(rustdoc, target, package, cx, kind));

    rustdoc = rustdoc.env("OUT_DIR", if package.has_custom_build() {
        Some(cx.layout(package, kind).build_out(package))
    } else {
        None
    });

    trace!("commands={}", rustdoc);

    let primary = package.package_id() == cx.resolve.root();
    let name = package.name().to_string();
    let desc = rustdoc.to_string();
    let exec_engine = cx.exec_engine.clone();

    Ok(Work::new(move |desc_tx| {
        desc_tx.send(desc).unwrap();
        if primary {
            try!(exec_engine.exec(rustdoc).chain_error(|| {
                human(format!("Could not document `{}`.", name))
            }))
        } else {
            try!(exec_engine.exec_with_output(rustdoc).and(Ok(())).map_err(|err| {
                match err.exit {
                    Some(..) => {
                        caused_human(format!("Could not document `{}`.",
                                             name), err)
                    }
                    None => {
                        caused_human("Failed to run rustdoc", err)
                    }
                }
            }))
        }
        Ok(())
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
fn root_path(cx: &Context, pkg: &Package, target: &Target) -> Path {
    let absolute = pkg.root().join(target.src_path());
    let cwd = cx.config.cwd();
    if cwd.is_ancestor_of(&absolute) {
        absolute.path_relative_from(cwd).unwrap_or(absolute)
    } else {
        absolute
    }
}

fn build_base_args(cx: &Context,
                   mut cmd: CommandPrototype,
                   pkg: &Package,
                   target: &Target,
                   crate_types: &[&str]) -> CommandPrototype {
    let metadata = target.metadata();

    // Move to cwd so the root_path() passed below is actually correct
    cmd = cmd.cwd(cx.config.cwd().clone());

    // TODO: Handle errors in converting paths into args
    cmd = cmd.arg(root_path(cx, pkg, target));

    cmd = cmd.arg("--crate-name").arg(target.name());

    for crate_type in crate_types.iter() {
        cmd = cmd.arg("--crate-type").arg(*crate_type);
    }

    // Despite whatever this target's profile says, we need to configure it
    // based off the profile found in the root package's targets.
    let profile = cx.profile(target);

    let prefer_dynamic = profile.is_for_host() ||
                         (crate_types.contains(&"dylib") &&
                          pkg.package_id() != cx.resolve.root());
    if prefer_dynamic {
        cmd = cmd.arg("-C").arg("prefer-dynamic");
    }

    if profile.opt_level() != 0 {
        cmd = cmd.arg("-C").arg(format!("opt-level={}", profile.opt_level()));
    }
    if (target.is_bin() || target.is_staticlib()) && profile.lto() {
        cmd = cmd.args(&["-C", "lto"]);
    } else {
        // There are some restrictions with LTO and codegen-units, so we
        // only add codegen units when LTO is not used.
        match profile.codegen_units() {
            Some(n) => cmd = cmd.arg("-C").arg(format!("codegen-units={}", n)),
            None => {},
        }
    }

    if profile.debug() {
        cmd = cmd.arg("-g");
    } else {
        cmd = cmd.args(&["--cfg", "ndebug"]);
    }

    if profile.is_test() && profile.uses_test_harness() {
        cmd = cmd.arg("--test");
    }

    match cx.resolve.features(pkg.package_id()) {
        Some(features) => {
            for feat in features.iter() {
                cmd = cmd.arg("--cfg").arg(format!("feature=\"{}\"", feat));
            }
        }
        None => {}
    }

    match metadata {
        Some(m) => {
            cmd = cmd.arg("-C").arg(format!("metadata={}", m.metadata));
            cmd = cmd.arg("-C").arg(format!("extra-filename={}", m.extra_filename));
        }
        None => {}
    }

    if profile.rpath() {
        cmd = cmd.arg("-C").arg("rpath");
    }

    return cmd;
}


fn build_plugin_args(mut cmd: CommandPrototype, cx: &Context, pkg: &Package,
                     target: &Target, kind: Kind) -> CommandPrototype {
    cmd = cmd.arg("--out-dir");
    cmd = cmd.arg(cx.out_dir(pkg, kind, target));

    cmd = cmd.arg("--emit=dep-info,link");

    if kind == Kind::Target {
        fn opt(cmd: CommandPrototype, key: &str, prefix: &str,
               val: Option<&str>) -> CommandPrototype {
            match val {
                Some(val) => {
                    cmd.arg(key)
                       .arg(format!("{}{}", prefix, val))
                }
                None => cmd
            }
        }

        cmd = opt(cmd, "--target", "", cx.requested_target());
        cmd = opt(cmd, "-C", "ar=", cx.ar(kind));
        cmd = opt(cmd, "-C", "linker=", cx.linker(kind));
    }

    return cmd;
}

fn build_deps_args(mut cmd: CommandPrototype, target: &Target, package: &Package,
                   cx: &Context,
                   kind: Kind) -> CargoResult<CommandPrototype> {
    let layout = cx.layout(package, kind);
    cmd = cmd.arg("-L").arg(format!("dependency={}", layout.root().display()));
    cmd = cmd.arg("-L").arg(format!("dependency={}", layout.deps().display()));

    cmd = cmd.env("OUT_DIR", if package.has_custom_build() {
        Some(layout.build_out(package))
    } else {
        None
    });

    for &(pkg, target) in cx.dep_targets(package, target).iter() {
        cmd = try!(link_to(cmd, pkg, target, cx, kind));
    }

    let targets = package.targets().iter().filter(|target| {
        target.is_lib() && target.profile().is_compile()
    });

    if (target.is_bin() || target.is_example()) &&
       !target.profile().is_custom_build() {
        for target in targets.filter(|f| f.is_rlib() || f.is_dylib()) {
            cmd = try!(link_to(cmd, package, target, cx, kind));
        }
    }

    return Ok(cmd);

    fn link_to(mut cmd: CommandPrototype, pkg: &Package, target: &Target,
               cx: &Context, kind: Kind) -> CargoResult<CommandPrototype> {
        // If this target is itself a plugin *or* if it's being linked to a
        // plugin, then we want the plugin directory. Otherwise we want the
        // target directory (hence the || here).
        let layout = cx.layout(pkg, match kind {
            Kind::Host => Kind::Host,
            Kind::Target if target.profile().is_for_host() => Kind::Host,
            Kind::Target => Kind::Target,
        });

        for filename in try!(cx.target_filenames(target)).iter() {
            if filename.as_bytes().ends_with(b".a") { continue }
            let mut v = Vec::new();
            v.push_all(target.name().as_bytes());
            v.push(b'=');
            v.push_all(layout.root().as_vec());
            v.push(old_path::SEP_BYTE);
            v.push_all(filename.as_bytes());
            cmd = cmd.arg("--extern").arg(&v);
        }
        return Ok(cmd);
    }
}

pub fn process(cmd: CommandType, pkg: &Package, _target: &Target,
               cx: &Context) -> CargoResult<CommandPrototype> {
    // When invoking a tool, we need the *host* deps directory in the dynamic
    // library search path for plugins and such which have dynamic dependencies.
    let layout = cx.layout(pkg, Kind::Host);
    let mut search_path = DynamicLibrary::search_path();
    search_path.push(layout.deps().clone());

    // We want to use the same environment and such as normal processes, but we
    // want to override the dylib search path with the one we just calculated.
    let search_path = try!(join_paths(&search_path, DynamicLibrary::envvar()));
    Ok(try!(cx.compilation.process(cmd, pkg))
              .env(DynamicLibrary::envvar(), Some(&search_path)))
}

fn each_dep<'a, F>(pkg: &Package, cx: &'a Context, mut f: F)
    where F: FnMut(&'a Package)
{
    let mut visited = HashSet::new();
    let pkg = cx.get_package(pkg.package_id());
    visit_deps(pkg, cx, &mut visited, &mut f);

    fn visit_deps<'a, F>(pkg: &'a Package, cx: &'a Context,
                         visited: &mut HashSet<&'a PackageId>, f: &mut F)
        where F: FnMut(&'a Package)
    {
        if !visited.insert(pkg.package_id()) { return }
        f(pkg);
        let deps = match cx.resolve.deps(pkg.package_id()) {
            Some(deps) => deps,
            None => return,
        };
        for dep_id in deps {
            visit_deps(cx.get_package(dep_id), cx, visited, f);
        }
    }
}

fn envify(s: &str) -> String {
    s.chars()
     .map(|c| c.to_uppercase())
     .map(|c| if c == '-' {'_'} else {c})
     .collect()
}
