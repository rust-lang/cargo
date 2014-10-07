use std::collections::HashSet;
use std::dynamic_lib::DynamicLibrary;
use std::io::{fs, USER_RWX};
use std::io::fs::PathExtensions;
use std::os;

use core::{SourceMap, Package, PackageId, PackageSet, Target, Resolve};
use util::{CargoResult, ProcessBuilder, CargoError, human, caused_human};
use util::{Config, internal, ChainError, Fresh, profile};

use self::job::{Job, Work};
use self::job_queue as jq;
use self::job_queue::JobQueue;

pub use self::compilation::Compilation;
pub use self::context::Context;
pub use self::context::{PlatformPlugin, PlatformPluginAndTarget};
pub use self::context::{PlatformRequirement, PlatformTarget};
pub use self::layout::{Layout, LayoutProxy};

mod context;
mod compilation;
mod fingerprint;
mod job;
mod job_queue;
mod layout;

#[deriving(PartialEq, Eq)]
pub enum Kind { KindPlugin, KindTarget }

// This is a temporary assert that ensures the consistency of the arguments
// given the current limitations of Cargo. The long term fix is to have each
// Target know the absolute path to the build location.
fn uniq_target_dest<'a>(targets: &[&'a Target]) -> Option<&'a str> {
    let mut curr: Option<Option<&str>> = None;

    for t in targets.iter() {
        let dest = t.get_profile().get_dest();

        match curr {
            Some(curr) => assert!(curr == dest),
            None => curr = Some(dest)
        }
    }

    curr.unwrap()
}

// Returns a mapping of the root package plus its immediate dependencies to
// where the compiled libraries are all located.
pub fn compile_targets<'a>(env: &str, targets: &[&'a Target], pkg: &'a Package,
                           deps: &PackageSet, resolve: &'a Resolve,
                           sources: &'a SourceMap,
                           config: &'a mut Config<'a>)
                           -> CargoResult<Compilation> {
    if targets.is_empty() {
        return Ok(Compilation::new(pkg))
    }

    debug!("compile_targets; targets={}; pkg={}; deps={}", targets, pkg, deps);

    let dest = uniq_target_dest(targets);
    let root = deps.iter().find(|p| p.get_package_id() == resolve.root()).unwrap();
    let host_layout = Layout::new(root, None, dest);
    let target_layout = config.target().map(|target| {
        layout::Layout::new(root, Some(target), dest)
    });

    let mut cx = try!(Context::new(env, resolve, sources, deps, config,
                                   host_layout, target_layout, pkg));
    let mut queue = JobQueue::new(cx.resolve, deps, cx.config);

    // First ensure that the destination directory exists
    try!(cx.prepare(pkg));

    // Build up a list of pending jobs, each of which represent compiling a
    // particular package. No actual work is executed as part of this, that's
    // all done later as part of the `execute` function which will run
    // everything in order with proper parallelism.
    let mut compiled = HashSet::new();
    each_dep(pkg, &cx, |dep| {
        compiled.insert(dep.get_package_id().clone());
    });
    for dep in deps.iter() {
        if dep == pkg { continue }

        // Only compile lib targets for dependencies
        let targets = dep.get_targets().iter().filter(|target| {
            cx.is_relevant_target(*target)
        }).collect::<Vec<&Target>>();

        if targets.len() == 0 && dep.get_package_id() != resolve.root() {
            return Err(human(format!("Package `{}` has no library targets", dep)))
        }

        let compiled = compiled.contains(dep.get_package_id());
        try!(compile(targets.as_slice(), dep, compiled, &mut cx, &mut queue));
    }

    try!(compile(targets, pkg, true, &mut cx, &mut queue));

    // Now that we've figured out everything that we're going to do, do it!
    try!(queue.execute(cx.config));

    Ok(cx.compilation)
}

fn compile<'a, 'b>(targets: &[&'a Target], pkg: &'a Package,
                   compiled: bool,
                   cx: &mut Context<'a, 'b>,
                   jobs: &mut JobQueue<'a, 'b>) -> CargoResult<()> {
    debug!("compile_pkg; pkg={}; targets={}", pkg, targets);
    let _p = profile::start(format!("preparing: {}", pkg));

    // Packages/targets which are actually getting compiled are constructed into
    // a real job. Packages which are *not* compiled still have their jobs
    // executed, but only if the work is fresh. This is to preserve their
    // artifacts if any exist.
    let job = if compiled {Job::new} else {Job::noop};
    if !compiled { jobs.ignore(pkg); }

    if targets.is_empty() {
        return Ok(())
    }

    // Prepare the fingerprint directory as the first step of building a package
    let (target1, target2) = fingerprint::prepare_init(cx, pkg, KindTarget);
    let mut init = vec![(Job::new(target1, target2, String::new()), Fresh)];
    if cx.config.target().is_some() {
        let (plugin1, plugin2) = fingerprint::prepare_init(cx, pkg, KindPlugin);
        init.push((Job::new(plugin1, plugin2, String::new()), Fresh));
    }
    jobs.enqueue(pkg, jq::StageStart, init);

    // First part of the build step of a target is to execute all of the custom
    // build commands.
    let mut build_cmds = Vec::new();
    for (i, build_cmd) in pkg.get_manifest().get_build().iter().enumerate() {
        let work = try!(compile_custom(pkg, build_cmd.as_slice(), cx, i == 0));
        build_cmds.push(work);
    }
    let (freshness, dirty, fresh) =
        try!(fingerprint::prepare_build_cmd(cx, pkg));
    let desc = match build_cmds.len() {
        0 => String::new(),
        1 => pkg.get_manifest().get_build()[0].to_string(),
        _ => format!("custom build commands"),
    };
    let dirty = proc() {
        for cmd in build_cmds.into_iter() { try!(cmd()) }
        dirty()
    };
    jobs.enqueue(pkg, jq::StageCustomBuild, vec![(job(dirty, fresh, desc),
                                                  freshness)]);

    // After the custom command has run, execute rustc for all targets of our
    // package.
    //
    // Each target has its own concept of freshness to ensure incremental
    // rebuilds on the *target* granularity, not the *package* granularity.
    let (mut libs, mut bins, mut tests) = (Vec::new(), Vec::new(), Vec::new());
    for &target in targets.iter() {
        let work = if target.get_profile().is_doc() {
            let (rustdoc, desc) = try!(rustdoc(pkg, target, cx));
            vec![(rustdoc, KindTarget, desc)]
        } else {
            let req = cx.get_requirement(pkg, target);
            try!(rustc(pkg, target, cx, req))
        };

        let dst = match (target.is_lib(), target.get_profile().is_test()) {
            (_, true) => &mut tests,
            (true, _) => &mut libs,
            (false, false) if target.get_profile().get_env() == "test" => &mut tests,
            (false, false) => &mut bins,
        };
        for (work, kind, desc) in work.into_iter() {
            let (freshness, dirty, fresh) =
                try!(fingerprint::prepare_target(cx, pkg, target, kind));

            let dirty = proc() { try!(work()); dirty() };
            dst.push((job(dirty, fresh, desc), freshness));
        }
    }
    jobs.enqueue(pkg, jq::StageLibraries, libs);
    jobs.enqueue(pkg, jq::StageBinaries, bins);
    jobs.enqueue(pkg, jq::StageTests, tests);
    Ok(())
}

fn compile_custom(pkg: &Package, cmd: &str,
                  cx: &Context, first: bool) -> CargoResult<Work> {
    let root = cx.get_package(cx.resolve.root());
    let profile = root.get_manifest().get_targets().iter()
                      .find(|target| target.get_profile().get_env() == cx.env())
                      .map(|target| target.get_profile());
    let profile = match profile {
        Some(profile) => profile,
        None => return Err(internal(format!("no profile for {}", cx.env())))
    };

    // TODO: this needs to be smarter about splitting
    let mut cmd = cmd.split(' ');
    // TODO: this shouldn't explicitly pass `KindTarget` for dest/deps_dir, we
    //       may be building a C lib for a plugin
    let layout = cx.layout(pkg, KindTarget);
    let output = layout.native(pkg);
    let old_output = layout.proxy().old_native(pkg);
    let mut p = process(cmd.next().unwrap(), pkg, cx)
                     .env("OUT_DIR", Some(&output))
                     .env("DEPS_DIR", Some(&output))
                     .env("TARGET", Some(cx.target_triple()))
                     .env("DEBUG", Some(profile.get_debug().to_string()))
                     .env("OPT_LEVEL", Some(profile.get_opt_level().to_string()))
                     .env("PROFILE", Some(profile.get_env()));
    for arg in cmd {
        p = p.arg(arg);
    }
    for &(pkg, _) in cx.dep_targets(pkg).iter() {
        let name: String = pkg.get_name().chars().map(|c| {
            match c {
                '-' => '_',
                c => c.to_uppercase(),
            }
        }).collect();
        p = p.env(format!("DEP_{}_OUT_DIR", name).as_slice(),
                  Some(&layout.native(pkg)));
    }
    let pkg = pkg.to_string();

    Ok(proc() {
        if first {
            try!(if old_output.exists() {
                fs::rename(&old_output, &output)
            } else {
                fs::mkdir(&output, USER_RWX)
            }.chain_error(|| {
                internal("failed to create output directory for build command")
            }));
        }
        try!(p.exec_with_output().map(|_| ()).map_err(|mut e| {
            e.msg = format!("Failed to run custom build command for `{}`\n{}",
                            pkg, e.msg);
            e.mark_human()
        }));
        Ok(())
    })
}

fn rustc(package: &Package, target: &Target,
         cx: &mut Context, req: PlatformRequirement)
         -> CargoResult<Vec<(Work, Kind, String)> >{
    let crate_types = target.rustc_crate_types();
    let rustcs = try!(prepare_rustc(package, target, crate_types, cx, req));

    Ok(rustcs.into_iter().map(|(rustc, kind)| {
        let name = package.get_name().to_string();
        let desc = rustc.to_string();
        let is_path_source = package.get_package_id().get_source_id().is_path();
        let show_warnings = package.get_package_id() == cx.resolve.root() ||
                            is_path_source;
        let rustc = if show_warnings {rustc} else {rustc.arg("-Awarnings")};

        (proc() {
            try!(rustc.exec().chain_error(|| {
                human(format!("Could not compile `{}`.", name))
            }));
            Ok(())
        }, kind, desc)
    }).collect())
}

fn prepare_rustc(package: &Package, target: &Target, crate_types: Vec<&str>,
                 cx: &Context, req: PlatformRequirement)
                 -> CargoResult<Vec<(ProcessBuilder, Kind)>> {
    let base = process("rustc", package, cx);
    let base = build_base_args(cx, base, package, target, crate_types.as_slice());

    let target_cmd = build_plugin_args(base.clone(), cx, package, target, KindTarget);
    let plugin_cmd = build_plugin_args(base, cx, package, target, KindPlugin);
    let target_cmd = try!(build_deps_args(target_cmd, target, package, cx,
                                          KindTarget));
    let plugin_cmd = try!(build_deps_args(plugin_cmd, target, package, cx,
                                          KindPlugin));

    Ok(match req {
        PlatformTarget => vec![(target_cmd, KindTarget)],
        PlatformPlugin => vec![(plugin_cmd, KindPlugin)],
        PlatformPluginAndTarget if cx.config.target().is_none() =>
            vec![(target_cmd, KindTarget)],
        PlatformPluginAndTarget => vec![(target_cmd, KindTarget),
                                        (plugin_cmd, KindPlugin)],
    })
}


fn rustdoc(package: &Package, target: &Target,
           cx: &mut Context) -> CargoResult<(Work, String)> {
    let kind = KindTarget;
    let pkg_root = package.get_root();
    let cx_root = cx.layout(package, kind).proxy().dest().join("doc");
    let rustdoc = process("rustdoc", package, cx).cwd(pkg_root.clone());
    let rustdoc = rustdoc.arg(target.get_src_path())
                         .arg("-o").arg(cx_root)
                         .arg("--crate-name").arg(target.get_name());
    let rustdoc = try!(build_deps_args(rustdoc, target, package, cx, kind));

    log!(5, "commands={}", rustdoc);

    let primary = package.get_package_id() == cx.resolve.root();
    let name = package.get_name().to_string();
    let desc = rustdoc.to_string();
    Ok((proc() {
        if primary {
            try!(rustdoc.exec().chain_error(|| {
                human(format!("Could not document `{}`.", name))
            }))
        } else {
            try!(rustdoc.exec_with_output().and(Ok(())).map_err(|err| {
                match err.output() {
                    Some(output) => {
                        caused_human(format!("Could not document `{}`.\n{}",
                                             name, output), err)
                    }
                    None => {
                        caused_human("Failed to run rustdoc", err)
                    }
                }
            }))
        }
        Ok(())
    }, desc))
}

fn build_base_args(cx: &Context,
                   mut cmd: ProcessBuilder,
                   pkg: &Package,
                   target: &Target,
                   crate_types: &[&str]) -> ProcessBuilder {
    let metadata = target.get_metadata();

    // TODO: Handle errors in converting paths into args
    cmd = cmd.arg(target.get_src_path());

    cmd = cmd.arg("--crate-name").arg(target.get_name());

    for crate_type in crate_types.iter() {
        cmd = cmd.arg("--crate-type").arg(*crate_type);
    }

    // Despite whatever this target's profile says, we need to configure it
    // based off the profile found in the root package's targets.
    let mut profile = target.get_profile().clone();
    let root_package = cx.get_package(cx.resolve.root());
    for target in root_package.get_manifest().get_targets().iter() {
        let root_profile = target.get_profile();
        if root_profile.get_env() != profile.get_env() { continue }
        profile = profile.opt_level(root_profile.get_opt_level())
                         .debug(root_profile.get_debug());
    }

    if profile.get_opt_level() != 0 {
        cmd = cmd.arg("--opt-level").arg(profile.get_opt_level().to_string());
    }

    match profile.get_codegen_units() {
        Some(n) => cmd = cmd.arg("-C").arg(format!("codegen-units={}", n)),
        None => {},
    }

    if profile.get_debug() {
        cmd = cmd.arg("-g");
    } else {
        cmd = cmd.args(["--cfg", "ndebug"]);
    }

    if profile.is_test() && profile.uses_test_harness() {
        cmd = cmd.arg("--test");
    }

    match cx.resolve.features(pkg.get_package_id()) {
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

    return cmd;
}


fn build_plugin_args(mut cmd: ProcessBuilder, cx: &Context, pkg: &Package,
                     target: &Target, kind: Kind) -> ProcessBuilder {
    cmd = cmd.arg("--out-dir");
    cmd = cmd.arg(cx.layout(pkg, kind).root());

    let (_, dep_info_loc) = fingerprint::dep_info_loc(cx, pkg, target, kind);
    cmd = cmd.arg("--dep-info").arg(dep_info_loc);

    if kind == KindTarget {
        fn opt(cmd: ProcessBuilder, key: &str, prefix: &str,
               val: Option<&str>) -> ProcessBuilder {
            match val {
                Some(val) => {
                    cmd.arg(key)
                       .arg(format!("{}{}", prefix, val))
                }
                None => cmd
            }
        }

        cmd = opt(cmd, "--target", "", cx.config.target());
        cmd = opt(cmd, "-C", "ar=", cx.config.ar());
        cmd = opt(cmd, "-C", "linker=", cx.config.linker());
    }

    return cmd;
}

fn build_deps_args(mut cmd: ProcessBuilder, target: &Target, package: &Package,
                   cx: &Context,
                   kind: Kind) -> CargoResult<ProcessBuilder> {
    let layout = cx.layout(package, kind);
    cmd = cmd.arg("-L").arg(layout.root());
    cmd = cmd.arg("-L").arg(layout.deps());

    // Traverse the entire dependency graph looking for -L paths to pass for
    // native dependencies.
    let mut dirs = Vec::new();
    each_dep(package, cx, |pkg| {
        if pkg.get_manifest().get_build().len() > 0 {
            dirs.push(layout.native(pkg));
        }
    });
    for dir in dirs.into_iter() {
        cmd = cmd.arg("-L").arg(dir);
    }

    for &(pkg, target) in cx.dep_targets(package).iter() {
        cmd = try!(link_to(cmd, pkg, target, cx, kind));
    }

    let mut targets = package.get_targets().iter().filter(|target| {
        target.is_lib() && target.get_profile().is_compile()
    });

    if target.is_bin() {
        for target in targets {
            if target.is_staticlib() {
                continue;
            }

            cmd = try!(link_to(cmd, package, target, cx, kind));
        }
    }

    return Ok(cmd);

    fn link_to(mut cmd: ProcessBuilder, pkg: &Package, target: &Target,
               cx: &Context, kind: Kind) -> CargoResult<ProcessBuilder> {
        // If this target is itself a plugin *or* if it's being linked to a
        // plugin, then we want the plugin directory. Otherwise we want the
        // target directory (hence the || here).
        let layout = cx.layout(pkg, match kind {
            KindPlugin => KindPlugin,
            KindTarget if target.get_profile().is_plugin() => KindPlugin,
            KindTarget => KindTarget,
        });

        for filename in try!(cx.target_filenames(target)).iter() {
            let mut v = Vec::new();
            v.push_all(target.get_name().as_bytes());
            v.push(b'=');
            v.push_all(layout.root().as_vec());
            v.push(b'/');
            v.push_all(filename.as_bytes());
            cmd = cmd.arg("--extern").arg(v.as_slice());
        }
        return Ok(cmd);
    }
}

pub fn process<T: ToCStr>(cmd: T, pkg: &Package, cx: &Context) -> ProcessBuilder {
    // When invoking a tool, we need the *host* deps directory in the dynamic
    // library search path for plugins and such which have dynamic dependencies.
    let layout = cx.layout(pkg, KindPlugin);
    let mut search_path = DynamicLibrary::search_path();
    search_path.push(layout.deps().clone());

    // Also be sure to pick up any native build directories required by plugins
    // or their dependencies
    let mut native_search_paths = HashSet::new();
    for &(dep, target) in cx.dep_targets(pkg).iter() {
        if !target.get_profile().is_plugin() { continue }
        each_dep(dep, cx, |dep| {
            if dep.get_manifest().get_build().len() > 0 {
                native_search_paths.insert(layout.native(dep));
            }
        });
    }
    search_path.extend(native_search_paths.into_iter());

    // We want to use the same environment and such as normal processes, but we
    // want to override the dylib search path with the one we just calculated.
    let search_path = os::join_paths(search_path.as_slice()).unwrap();
    cx.compilation.process(cmd, pkg)
                  .env(DynamicLibrary::envvar(), Some(search_path.as_slice()))
}

fn each_dep<'a>(pkg: &Package, cx: &'a Context, f: |&'a Package|) {
    let mut visited = HashSet::new();
    let pkg = cx.get_package(pkg.get_package_id());
    visit_deps(pkg, cx, &mut visited, f);

    fn visit_deps<'a>(pkg: &'a Package, cx: &'a Context,
                      visited: &mut HashSet<&'a PackageId>,
                      f: |&'a Package|) {
        if !visited.insert(pkg.get_package_id()) { return }
        f(pkg);
        let mut deps = match cx.resolve.deps(pkg.get_package_id()) {
            Some(deps) => deps,
            None => return,
        };
        for dep_id in deps {
            visit_deps(cx.get_package(dep_id), cx, visited, |p| f(p))
        }
    }
}
