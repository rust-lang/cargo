use std::collections::{HashSet, HashMap};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::prelude::*;
use std::path::{self, PathBuf};
use std::sync::Arc;

use core::{SourceMap, Package, PackageId, PackageSet, Target, Resolve};
use core::{Profile, Profiles};
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

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum Kind { Host, Target }

#[derive(Default, Clone)]
pub struct BuildConfig {
    pub host: TargetConfig,
    pub target: TargetConfig,
    pub jobs: u32,
    pub requested_target: Option<String>,
    pub exec_engine: Option<Arc<Box<ExecEngine>>>,
    pub release: bool,
    pub doc_all: bool,
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
    let output = try!(try!(util::process("rustc"))
        .arg("-vV")
        .exec_with_output());
    let output = try!(String::from_utf8(output.stdout).map_err(|_| {
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

// Returns a mapping of the root package plus its immediate dependencies to
// where the compiled libraries are all located.
pub fn compile_targets<'a, 'b>(targets: &[(&'a Target, &'a Profile)],
                               pkg: &'a Package,
                               deps: &PackageSet,
                               resolve: &'a Resolve,
                               sources: &'a SourceMap<'a>,
                               config: &'a Config<'b>,
                               build_config: BuildConfig,
                               profiles: &'a Profiles)
                               -> CargoResult<Compilation> {
    if targets.is_empty() {
        return Ok(Compilation::new(pkg))
    }

    debug!("compile_targets: {}", pkg);

    try!(links::validate(deps));

    let dest = if build_config.release {"release"} else {"debug"};
    let root = if resolve.root() == pkg.package_id() {
        pkg
    } else {
        deps.iter().find(|p| p.package_id() == resolve.root()).unwrap()
    };
    let host_layout = Layout::new(root, None, &dest);
    let target_layout = build_config.requested_target.as_ref().map(|target| {
        layout::Layout::new(root, Some(&target), &dest)
    });

    let mut cx = try!(Context::new(resolve, sources, deps, config,
                                   host_layout, target_layout, pkg,
                                   build_config, profiles));

    let mut queue = JobQueue::new(cx.resolve, deps, cx.jobs());

    // Prep the context's build requirements and see the job graph for all
    // packages initially.
    {
        let _p = profile::start("preparing build directories");
        try!(cx.prepare(pkg, targets));
        prepare_init(&mut cx, pkg, &mut queue, &mut HashSet::new());
    }

    // Build up a list of pending jobs, each of which represent compiling a
    // particular package. No actual work is executed as part of this, that's
    // all done next as part of the `execute` function which will run
    // everything in order with proper parallelism.
    try!(compile(targets, pkg, &mut cx, &mut queue));

    // Now that we've figured out everything that we're going to do, do it!
    try!(queue.execute(cx.config));

    let out_dir = cx.layout(pkg, Kind::Target).build_out(pkg)
                    .display().to_string();
    cx.compilation.extra_env.insert("OUT_DIR".to_string(), out_dir);

    for &(target, profile) in targets {
        let kind = Kind::from(target);
        for filename in try!(cx.target_filenames(pkg, target, profile,
                                                 kind)).iter() {
            let dst = cx.out_dir(pkg, kind, target).join(filename);
            if profile.test {
                cx.compilation.tests.push((target.name().to_string(), dst));
            } else if target.is_bin() || target.is_example() {
                cx.compilation.binaries.push(dst);
            } else if target.is_lib() {
                let pkgid = pkg.package_id().clone();
                cx.compilation.libraries.entry(pkgid).or_insert(Vec::new())
                  .push((target.clone(), dst));
            }
            if !target.is_lib() { continue }

            // Include immediate lib deps as well
            for dep in cx.dep_targets(pkg, target, profile).iter() {
                let (pkg, target, profile) = *dep;
                let pkgid = pkg.package_id();
                if !target.is_lib() { continue }
                if profile.doc { continue }
                if cx.compilation.libraries.contains_key(&pkgid) { continue }

                let kind = kind.for_target(target);
                let v = try!(cx.target_filenames(pkg, target, profile, kind));
                let v = v.into_iter().map(|f| {
                    (target.clone(), cx.out_dir(pkg, kind, target).join(f))
                }).collect::<Vec<_>>();
                cx.compilation.libraries.insert(pkgid.clone(), v);
            }
        }
    }

    if let Some(feats) = cx.resolve.features(pkg.package_id()) {
        cx.compilation.features.extend(feats.iter().cloned());
    }

    for (&(ref pkg, _), output) in cx.build_state.outputs.lock().unwrap().iter() {
        let any_dylib = output.library_links.iter().any(|l| {
            !l.starts_with("static=") && !l.starts_with("framework=")
        });
        if !any_dylib { continue }
        for dir in output.library_paths.iter() {
            cx.compilation.native_dirs.insert(pkg.clone(), dir.clone());
        }
    }
    Ok(cx.compilation)
}

fn compile<'a, 'b>(targets: &[(&'a Target, &'a Profile)],
                   pkg: &'a Package,
                   cx: &mut Context<'a, 'b>,
                   jobs: &mut JobQueue<'a>) -> CargoResult<()> {
    debug!("compile_pkg; pkg={}", pkg);
    let profiling_marker = profile::start(format!("preparing: {}", pkg));

    // For each target/profile run the compiler or rustdoc accordingly. After
    // having done so we enqueue the job in the right portion of the dependency
    // graph and then move on to the next.
    //
    // This loop also takes care of enqueueing the work needed to actually run
    // the custom build commands as well.
    for &(target, profile) in targets {
        if !cx.compiled.insert((pkg.package_id(), target, profile)) {
            continue
        }

        let work = if profile.doc {
            let rustdoc = try!(rustdoc(pkg, target, profile, cx));
            vec![(rustdoc, Kind::Target)]
        } else {
            let req = cx.get_requirement(pkg, target);
            try!(rustc(pkg, target, profile, cx, req))
        };

        for (work, kind) in work.into_iter() {
            let (freshness, dirty, fresh) =
                try!(fingerprint::prepare_target(cx, pkg, target, profile, kind));

            let dirty = Work::new(move |desc_tx| {
                try!(work.call(desc_tx.clone()));
                dirty.call(desc_tx)
            });

            // Figure out what stage this work will go into
            let dst = match (target.is_lib(),
                             profile.test,
                             target.is_custom_build()) {
                (_, _, true) => jobs.queue(pkg, Stage::BuildCustomBuild),
                (true, true, _) => jobs.queue(pkg, Stage::LibraryTests),
                (false, true, _) => jobs.queue(pkg, Stage::BinaryTests),
                (true, false, _) => jobs.queue(pkg, Stage::Libraries),
                (false, false, _) if !target.is_bin() => {
                    jobs.queue(pkg, Stage::BinaryTests)
                }
                (false, false, _) => jobs.queue(pkg, Stage::Binaries),
            };
            dst.push((Job::new(dirty, fresh), freshness));
        }

        // If this is a custom build command, we need to not only build the
        // script but we also need to run it. Note that this is a little nuanced
        // because we may need to run the build script multiple times. If the
        // package is needed in both a host and target context, we need to run
        // it once per context.
        if !target.is_custom_build() { continue }
        let mut reqs = Vec::new();
        let requirement = pkg.targets().iter().filter(|t| !t.is_custom_build())
                             .fold(None::<Platform>, |req, t| {
            let r2 = cx.get_requirement(pkg, t);
            req.map(|r| r.combine(r2)).or(Some(r2))
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
        let before = jobs.queue(pkg, Stage::RunCustomBuild).len();
        for &req in reqs.iter() {
            let kind = match req {
                Platform::Plugin => Kind::Host,
                _ => Kind::Target,
            };
            let key = (pkg.package_id().clone(), kind);
            if pkg.manifest().links().is_some() &&
                cx.build_state.outputs.lock().unwrap().contains_key(&key) {
                    continue
                }
            let (dirty, fresh, freshness) =
                try!(custom_build::prepare(pkg, target, req, cx));
            let run_custom = jobs.queue(pkg, Stage::RunCustomBuild);
            run_custom.push((Job::new(dirty, fresh), freshness));
        }

        // If we didn't actually run the custom build command, then there's no
        // need to compile it.
        if jobs.queue(pkg, Stage::RunCustomBuild).len() == before {
            jobs.queue(pkg, Stage::BuildCustomBuild).pop();
        }
    }
    drop(profiling_marker);

    // Be sure to compile all dependencies of this target as well. Don't recurse
    // if we've already recursed, however.
    for &(target, profile) in targets {
        for &(pkg, target, p) in cx.dep_targets(pkg, target, profile).iter() {
            try!(compile(&[(target, p)], pkg, cx, jobs));
        }
    }

    Ok(())
}

fn prepare_init<'a, 'b>(cx: &mut Context<'a, 'b>,
                        pkg: &'a Package,
                        jobs: &mut JobQueue<'a>,
                        visited: &mut HashSet<&'a PackageId>) {
    if !visited.insert(pkg.package_id()) { return }

    // Set up all dependencies
    for dep in cx.resolve.deps(pkg.package_id()).into_iter().flat_map(|a| a) {
        let dep = cx.get_package(dep);
        prepare_init(cx, dep, jobs, visited);
    }

    // Initialize blank queues for each stage
    jobs.queue(pkg, Stage::BuildCustomBuild);
    jobs.queue(pkg, Stage::RunCustomBuild);
    jobs.queue(pkg, Stage::Libraries);
    jobs.queue(pkg, Stage::Binaries);
    jobs.queue(pkg, Stage::LibraryTests);
    jobs.queue(pkg, Stage::BinaryTests);
    jobs.queue(pkg, Stage::End);

    // Prepare the fingerprint directory as the first step of building a package
    let (target1, target2) = fingerprint::prepare_init(cx, pkg, Kind::Target);
    let init = jobs.queue(pkg, Stage::Start);
    if cx.requested_target().is_some() {
        let (plugin1, plugin2) = fingerprint::prepare_init(cx, pkg,
                                                           Kind::Host);
        init.push((Job::new(plugin1, plugin2), Fresh));
    }
    init.push((Job::new(target1, target2), Fresh));
}

fn rustc(package: &Package, target: &Target, profile: &Profile,
         cx: &mut Context, req: Platform)
         -> CargoResult<Vec<(Work, Kind)> >{
    let crate_types = target.rustc_crate_types();
    let rustcs = try!(prepare_rustc(package, target, profile, crate_types,
                                    cx, req));

    let plugin_deps = crawl_build_deps(cx, package, target, profile, Kind::Host);

    return rustcs.into_iter().map(|(mut rustc, kind)| {
        let name = package.name().to_string();
        let is_path_source = package.package_id().source_id().is_path();
        let show_warnings = package.package_id() == cx.resolve.root() ||
                            is_path_source;
        if !show_warnings {
            rustc.arg("-Awarnings");
        }
        let exec_engine = cx.exec_engine.clone();

        let filenames = try!(cx.target_filenames(package, target, profile,
                                                 kind));
        let root = cx.out_dir(package, kind, target);

        // Prepare the native lib state (extra -L and -l flags)
        let build_state = cx.build_state.clone();
        let current_id = package.package_id().clone();
        let plugin_deps = plugin_deps.clone();
        let mut native_lib_deps = crawl_build_deps(cx, package, target,
                                                   profile, kind);
        if package.has_custom_build() && !target.is_custom_build() {
            native_lib_deps.insert(0, current_id.clone());
        }

        // If we are a binary and the package also contains a library, then we
        // don't pass the `-l` flags.
        let pass_l_flag = target.is_lib() || !package.targets().iter().any(|t| {
            t.is_lib()
        });
        let do_rename = target.allows_underscores() && !profile.test;
        let real_name = target.name().to_string();
        let crate_name = target.crate_name();

        let rustc_dep_info_loc = if do_rename {
            root.join(&crate_name)
        } else {
            root.join(&cx.file_stem(package, target, profile))
        }.with_extension("d");
        let dep_info_loc = fingerprint::dep_info_loc(cx, package, target,
                                                     profile, kind);
        let cwd = cx.config.cwd().to_path_buf();

        Ok((Work::new(move |desc_tx| {
            debug!("about to run: {}", rustc);

            // Only at runtime have we discovered what the extra -L and -l
            // arguments are for native libraries, so we process those here. We
            // also need to be sure to add any -L paths for our plugins to the
            // dynamic library load path as a plugin's dynamic library may be
            // located somewhere in there.
            let build_state = build_state.outputs.lock().unwrap();
            add_native_deps(&mut rustc, &*build_state, native_lib_deps,
                            kind, pass_l_flag, &current_id);
            try!(add_plugin_deps(&mut rustc, &*build_state, plugin_deps));
            drop(build_state);

            // FIXME(rust-lang/rust#18913): we probably shouldn't have to do
            //                              this manually
            for filename in filenames.iter() {
                let dst = root.join(filename);
                if fs::metadata(&dst).is_ok() {
                    try!(fs::remove_file(&dst));
                }
            }

            desc_tx.send(rustc.to_string()).ok();
            try!(exec_engine.exec(rustc).chain_error(|| {
                human(format!("Could not compile `{}`.", name))
            }));

            if do_rename && real_name != crate_name {
                let dst = root.join(&filenames[0]);
                let src = dst.with_file_name(dst.file_name().unwrap()
                                                .to_str().unwrap()
                                                .replace(&real_name, &crate_name));
                try!(fs::rename(&src, &dst).chain_error(|| {
                    internal(format!("could not rename crate {:?}", src))
                }));
            }

            try!(fs::rename(&rustc_dep_info_loc, &dep_info_loc).chain_error(|| {
                internal(format!("could not rename dep info: {:?}",
                              rustc_dep_info_loc))
            }));
            try!(fingerprint::append_current_dir(&dep_info_loc, &cwd));

            Ok(())

        }), kind))
    }).collect();

    // Add all relevant -L and -l flags from dependencies (now calculated and
    // present in `state`) to the command provided
    fn add_native_deps(rustc: &mut CommandPrototype,
                       build_state: &BuildMap,
                       native_lib_deps: Vec<PackageId>,
                       kind: Kind,
                       pass_l_flag: bool,
                       current_id: &PackageId) {
        for id in native_lib_deps.into_iter() {
            debug!("looking up {} {:?}", id, kind);
            let output = &build_state[&(id.clone(), kind)];
            for path in output.library_paths.iter() {
                rustc.arg("-L").arg(path);
            }
            if id == *current_id {
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
    }
}

fn crawl_build_deps<'a>(cx: &'a Context,
                        pkg: &'a Package,
                        target: &Target,
                        profile: &Profile,
                        kind: Kind) -> Vec<PackageId> {
    let mut deps = HashSet::new();
    visit(cx, pkg, target, profile, kind, &mut HashSet::new(), &mut deps);
    let mut ret: Vec<_> = deps.into_iter().collect();
    ret.sort();
    return ret;

    fn visit<'a>(cx: &'a Context,
                 pkg: &'a Package, target: &Target, profile: &Profile,
                 kind: Kind,
                 visiting: &mut HashSet<&'a PackageId>,
                 libs: &mut HashSet<PackageId>) {
        for &(pkg, target, p) in cx.dep_targets(pkg, target, profile).iter() {
            if !target.linkable() { continue }
            let req = cx.get_requirement(pkg, target);
            if !req.includes(kind) { continue }
            if !visiting.insert(pkg.package_id()) { continue }

            if pkg.has_custom_build() {
                libs.insert(pkg.package_id().clone());
            }
            visit(cx, pkg, target, p, kind, visiting, libs);
            visiting.remove(&pkg.package_id());
        }
    }
}

// For all plugin dependencies, add their -L paths (now calculated and
// present in `state`) to the dynamic library load path for the command to
// execute.
fn add_plugin_deps(rustc: &mut CommandPrototype,
                   build_state: &BuildMap,
                   plugin_deps: Vec<PackageId>)
                   -> CargoResult<()> {
    let var = util::dylib_path_envvar();
    let search_path = rustc.get_env(var).unwrap_or(OsString::new());
    let mut search_path = env::split_paths(&search_path).collect::<Vec<_>>();
    for id in plugin_deps.into_iter() {
        debug!("adding libs for plugin dep: {}", id);
        let output = &build_state[&(id, Kind::Host)];
        for path in output.library_paths.iter() {
            search_path.push(path.clone());
        }
    }
    let search_path = try!(join_paths(&search_path, var));
    rustc.env(var, &search_path);
    Ok(())
}

fn prepare_rustc(package: &Package, target: &Target, profile: &Profile,
                 crate_types: Vec<&str>,
                 cx: &Context, req: Platform)
                 -> CargoResult<Vec<(CommandPrototype, Kind)>> {
    let mut base = try!(process(CommandType::Rustc, package, target, cx));
    build_base_args(cx, &mut base, package, target, profile, &crate_types);

    let mut targ_cmd = base.clone();
    let mut host_cmd = base;
    build_plugin_args(&mut targ_cmd, cx, package, target, Kind::Target);
    build_plugin_args(&mut host_cmd, cx, package, target, Kind::Host);
    try!(build_deps_args(&mut targ_cmd, target, profile, package, cx, Kind::Target));
    try!(build_deps_args(&mut host_cmd, target, profile, package, cx, Kind::Host));

    Ok(match req {
        Platform::Target => vec![(targ_cmd, Kind::Target)],
        Platform::Plugin => vec![(host_cmd, Kind::Host)],
        Platform::PluginAndTarget if cx.requested_target().is_none() => {
            vec![(targ_cmd, Kind::Target)]
        }
        Platform::PluginAndTarget => vec![(targ_cmd, Kind::Target),
                                          (host_cmd, Kind::Host)],
    })
}


fn rustdoc(package: &Package, target: &Target, profile: &Profile,
           cx: &mut Context) -> CargoResult<Work> {
    let kind = Kind::Target;
    let mut doc_dir = cx.get_package(cx.resolve.root()).absolute_target_dir();
    let mut rustdoc = try!(process(CommandType::Rustdoc, package, target, cx));
    rustdoc.arg(&root_path(cx, package, target))
           .cwd(cx.config.cwd())
           .arg("--crate-name").arg(&target.crate_name());

    if let Some(target) = cx.requested_target() {
        rustdoc.arg("--target").arg(target);
        doc_dir.push(target);
    }

    doc_dir.push("doc");

    rustdoc.arg("-o").arg(&doc_dir);

    match cx.resolve.features(package.package_id()) {
        Some(features) => {
            for feat in features {
                rustdoc.arg("--cfg").arg(&format!("feature=\"{}\"", feat));
            }
        }
        None => {}
    }

    try!(build_deps_args(&mut rustdoc, target, profile, package, cx, kind));

    if package.has_custom_build() {
        rustdoc.env("OUT_DIR", &cx.layout(package, kind).build_out(package));
    }

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
fn root_path(cx: &Context, pkg: &Package, target: &Target) -> PathBuf {
    let absolute = pkg.root().join(target.src_path());
    let cwd = cx.config.cwd();
    if absolute.starts_with(cwd) {
        util::without_prefix(&absolute, cwd).map(|s| {
            s.to_path_buf()
        }).unwrap_or(absolute)
    } else {
        absolute
    }
}

fn build_base_args(cx: &Context,
                   cmd: &mut CommandPrototype,
                   pkg: &Package,
                   target: &Target,
                   profile: &Profile,
                   crate_types: &[&str]) {
    let Profile {
        opt_level, lto, codegen_units, ref rustc_args, debuginfo, debug_assertions,
        rpath, test, doc: _doc,
    } = *profile;

    // Move to cwd so the root_path() passed below is actually correct
    cmd.cwd(cx.config.cwd());

    // TODO: Handle errors in converting paths into args
    cmd.arg(&root_path(cx, pkg, target));

    cmd.arg("--crate-name").arg(&target.crate_name());

    for crate_type in crate_types.iter() {
        cmd.arg("--crate-type").arg(crate_type);
    }

    let prefer_dynamic = target.for_host() ||
                         (crate_types.contains(&"dylib") &&
                          pkg.package_id() != cx.resolve.root());
    if prefer_dynamic {
        cmd.arg("-C").arg("prefer-dynamic");
    }

    if opt_level != 0 {
        cmd.arg("-C").arg(&format!("opt-level={}", opt_level));
    }

    // Disable LTO for host builds as prefer_dynamic and it are mutually
    // exclusive.
    if target.can_lto() && lto && !target.for_host() {
        cmd.args(&["-C", "lto"]);
    } else {
        // There are some restrictions with LTO and codegen-units, so we
        // only add codegen units when LTO is not used.
        match codegen_units {
            Some(n) => { cmd.arg("-C").arg(&format!("codegen-units={}", n)); }
            None => {},
        }
    }

    if debuginfo {
        cmd.arg("-g");
    }

    if let Some(ref args) = *rustc_args {
        cmd.args(args);
    }

    if debug_assertions && opt_level > 0 {
        cmd.args(&["-C", "debug-assertions=on"]);
    } else if !debug_assertions && opt_level == 0 {
        cmd.args(&["-C", "debug-assertions=off"]);
    }

    if test && target.harness() {
        cmd.arg("--test");
    }

    match cx.resolve.features(pkg.package_id()) {
        Some(features) => {
            for feat in features.iter() {
                cmd.arg("--cfg").arg(&format!("feature=\"{}\"", feat));
            }
        }
        None => {}
    }

    match cx.target_metadata(pkg, target, profile) {
        Some(m) => {
            cmd.arg("-C").arg(&format!("metadata={}", m.metadata));
            cmd.arg("-C").arg(&format!("extra-filename={}", m.extra_filename));
        }
        None => {}
    }

    if rpath {
        cmd.arg("-C").arg("rpath");
    }
}


fn build_plugin_args(cmd: &mut CommandPrototype, cx: &Context, pkg: &Package,
                     target: &Target, kind: Kind) {
    fn opt(cmd: &mut CommandPrototype, key: &str, prefix: &str,
           val: Option<&str>)  {
        if let Some(val) = val {
            cmd.arg(key).arg(&format!("{}{}", prefix, val));
        }
    }

    cmd.arg("--out-dir").arg(&cx.out_dir(pkg, kind, target));
    cmd.arg("--emit=dep-info,link");

    if kind == Kind::Target {
        opt(cmd, "--target", "", cx.requested_target());
    }

    opt(cmd, "-C", "ar=", cx.ar(kind));
    opt(cmd, "-C", "linker=", cx.linker(kind));
}

fn build_deps_args(cmd: &mut CommandPrototype,
                   target: &Target,
                   profile: &Profile,
                   package: &Package,
                   cx: &Context,
                   kind: Kind)
                   -> CargoResult<()> {
    let layout = cx.layout(package, kind);
    cmd.arg("-L").arg(&{
        let mut root = OsString::from("dependency=");
        root.push(layout.root());
        root
    });
    cmd.arg("-L").arg(&{
        let mut deps = OsString::from("dependency=");
        deps.push(layout.deps());
        deps
    });

    if package.has_custom_build() {
        cmd.env("OUT_DIR", &layout.build_out(package));
    }

    for &(pkg, target, p) in cx.dep_targets(package, target, profile).iter() {
        if target.linkable() {
            try!(link_to(cmd, pkg, target, p, cx, kind));
        }
    }

    return Ok(());

    fn link_to(cmd: &mut CommandPrototype, pkg: &Package, target: &Target,
               profile: &Profile, cx: &Context, kind: Kind) -> CargoResult<()> {
        let kind = kind.for_target(target);
        let layout = cx.layout(pkg, kind);

        for filename in try!(cx.target_filenames(pkg, target, profile, kind)).iter() {
            if filename.ends_with(".a") { continue }
            let mut v = OsString::new();
            v.push(&target.crate_name());
            v.push("=");
            v.push(layout.root());
            v.push(&path::MAIN_SEPARATOR.to_string());
            v.push(&filename);
            cmd.arg("--extern").arg(&v);
        }
        Ok(())
    }
}

pub fn process(cmd: CommandType, pkg: &Package, _target: &Target,
               cx: &Context) -> CargoResult<CommandPrototype> {
    // When invoking a tool, we need the *host* deps directory in the dynamic
    // library search path for plugins and such which have dynamic dependencies.
    let layout = cx.layout(pkg, Kind::Host);
    let mut search_path = util::dylib_path();
    search_path.push(layout.deps().to_path_buf());

    // We want to use the same environment and such as normal processes, but we
    // want to override the dylib search path with the one we just calculated.
    let search_path = try!(join_paths(&search_path, util::dylib_path_envvar()));
    let mut cmd = try!(cx.compilation.process(cmd, pkg));
    cmd.env(util::dylib_path_envvar(), &search_path);
    Ok(cmd)
}

fn envify(s: &str) -> String {
    s.chars()
     .flat_map(|c| c.to_uppercase())
     .map(|c| if c == '-' {'_'} else {c})
     .collect()
}

impl Kind {
    fn from(target: &Target) -> Kind {
        if target.for_host() {Kind::Host} else {Kind::Target}
    }

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
