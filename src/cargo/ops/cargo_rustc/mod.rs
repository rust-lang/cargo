use std::collections::HashSet;
use std::dynamic_lib::DynamicLibrary;
use std::io::{fs, UserRWX};
use std::os;

use core::{SourceMap, Package, PackageId, PackageSet, Target, Resolve};
use util::{CargoResult, ProcessBuilder, CargoError, human, caused_human};
use util::{Config, internal, ChainError, Fresh, profile};

use self::job::{Job, Work};
use self::job_queue::{JobQueue, StageStart, StageCustomBuild, StageLibraries};
use self::job_queue::{StageBinaries, StageEnd};
use self::context::{Context, PlatformRequirement, Target, Plugin, PluginAndTarget};

pub use self::compilation::Compilation;

mod context;
mod compilation;
mod fingerprint;
mod job;
mod job_queue;
mod layout;

#[deriving(PartialEq, Eq)]
enum Kind { KindPlugin, KindTarget }

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
        return Ok(Compilation::new())
    }

    debug!("compile_targets; targets={}; pkg={}; deps={}", targets, pkg, deps);

    let root = pkg.get_absolute_target_dir();
    let dest = uniq_target_dest(targets).unwrap_or("");
    let host_layout = layout::Layout::new(root.join(dest));
    let target_layout = config.target().map(|target| {
        layout::Layout::new(root.join(target).join(dest))
    });

    let mut cx = try!(Context::new(env, resolve, sources, deps, config,
                                   host_layout, target_layout));
    let mut queue = JobQueue::new(cx.resolve, cx.config);

    // First ensure that the destination directory exists
    try!(cx.prepare(pkg));

    // Build up a list of pending jobs, each of which represent compiling a
    // particular package. No actual work is executed as part of this, that's
    // all done later as part of the `execute` function which will run
    // everything in order with proper parallelism.
    for dep in deps.iter() {
        if dep == pkg { continue }

        // Only compile lib targets for dependencies
        let targets = dep.get_targets().iter().filter(|target| {
            cx.is_relevant_target(*target)
        }).collect::<Vec<&Target>>();

        try!(compile(targets.as_slice(), dep, &mut cx, &mut queue));
    }

    cx.primary();
    try!(compile(targets, pkg, &mut cx, &mut queue));

    // Now that we've figured out everything that we're going to do, do it!
    try!(queue.execute(cx.config));

    Ok(cx.compilation)
}

fn compile<'a, 'b>(targets: &[&'a Target], pkg: &'a Package,
                   cx: &mut Context<'a, 'b>,
                   jobs: &mut JobQueue<'a, 'b>) -> CargoResult<()> {
    debug!("compile_pkg; pkg={}; targets={}", pkg, targets);
    let _p = profile::start(format!("preparing: {}", pkg));

    if targets.is_empty() {
        return Ok(())
    }

    // Prepare the fingerprint directory as the first step of building a package
    let (target1, target2) = fingerprint::prepare_init(cx, pkg, KindTarget);
    let mut init = vec![(Job::new(target1, target2), Fresh)];
    if cx.config.target().is_some() {
        let (plugin1, plugin2) = fingerprint::prepare_init(cx, pkg, KindPlugin);
        init.push((Job::new(plugin1, plugin2), Fresh));
    }
    jobs.enqueue(pkg, StageStart, init);

    // First part of the build step of a target is to execute all of the custom
    // build commands.
    let mut build_cmds = Vec::new();
    for (i, build_cmd) in pkg.get_manifest().get_build().iter().enumerate() {
        let work = try!(compile_custom(pkg, build_cmd.as_slice(), cx, i == 0));
        build_cmds.push(work);
    }
    let (freshness, dirty, fresh) =
        try!(fingerprint::prepare_build_cmd(cx, pkg));
    let dirty = proc() {
        for cmd in build_cmds.move_iter() { try!(cmd()) }
        dirty()
    };
    jobs.enqueue(pkg, StageCustomBuild, vec![(Job::new(dirty, fresh), freshness)]);

    // After the custom command has run, execute rustc for all targets of our
    // package.
    //
    // Each target has its own concept of freshness to ensure incremental
    // rebuilds on the *target* granularity, not the *package* granularity.
    let (mut libs, mut bins) = (Vec::new(), Vec::new());
    for &target in targets.iter() {
        let work = if target.get_profile().is_doc() {
            vec![(rustdoc(pkg, target, cx), KindTarget)]
        } else {
            let req = cx.get_requirement(pkg, target);
            rustc(pkg, target, cx, req)
        };

        let dst = if target.is_lib() {&mut libs} else {&mut bins};
        for (work, kind) in work.move_iter() {
            let (freshness, dirty, fresh) =
                try!(fingerprint::prepare_target(cx, pkg, target, kind));

            let dirty = proc() { try!(work()); dirty() };
            dst.push((Job::new(dirty, fresh), freshness));
        }
    }
    jobs.enqueue(pkg, StageLibraries, libs);
    jobs.enqueue(pkg, StageBinaries, bins);
    jobs.enqueue(pkg, StageEnd, Vec::new());
    Ok(())
}

fn compile_custom(pkg: &Package, cmd: &str,
                  cx: &Context, first: bool) -> CargoResult<Work> {
    // TODO: this needs to be smarter about splitting
    let mut cmd = cmd.split(' ');
    // TODO: this shouldn't explicitly pass `KindTarget` for dest/deps_dir, we
    //       may be building a C lib for a plugin
    let layout = cx.layout(KindTarget);
    let output = layout.native(pkg);
    let mut p = process(cmd.next().unwrap(), pkg, cx)
                     .env("OUT_DIR", Some(&output))
                     .env("DEPS_DIR", Some(&output))
                     .env("TARGET", Some(cx.target_triple()));
    for arg in cmd {
        p = p.arg(arg);
    }
    Ok(proc() {
        if first {
            try!(fs::mkdir(&output, UserRWX).chain_error(|| {
                internal("failed to create output directory for build command")
            }));
        }
        try!(p.exec_with_output().map(|_| ()).map_err(|e| e.mark_human()));
        Ok(())
    })
}

fn rustc(package: &Package, target: &Target,
         cx: &mut Context, req: PlatformRequirement) -> Vec<(Work, Kind)> {
    let crate_types = target.rustc_crate_types();
    let root = package.get_root();

    log!(5, "root={}; target={}; crate_types={}; verbose={}; req={}",
         root.display(), target, crate_types, cx.primary, req);

    let primary = cx.primary;
    let rustcs = prepare_rustc(package, target, crate_types, cx, req);

    let _ = cx.config.shell().verbose(|shell| {
        for &(ref rustc, _) in rustcs.iter() {
            try!(shell.status("Running", rustc.to_string()));
        }
        Ok(())
    });

    rustcs.move_iter().map(|(rustc, kind)| {
        let name = package.get_name().to_string();

        (proc() {
            if primary {
                log!(5, "executing primary");
                try!(rustc.exec().chain_error(|| {
                    human(format!("Could not compile `{}`.", name))
                }))
            } else {
                log!(5, "executing deps");
                try!(rustc.exec_with_output().and(Ok(())).map_err(|err| {
                    caused_human(format!("Could not compile `{}`.\n{}",
                                         name, err.output().unwrap()), err)
                }))
            }
            Ok(())
        }, kind)
    }).collect()
}

fn prepare_rustc(package: &Package, target: &Target, crate_types: Vec<&str>,
                 cx: &Context, req: PlatformRequirement)
                 -> Vec<(ProcessBuilder, Kind)> {
    let base = process("rustc", package, cx);
    let base = build_base_args(base, target, crate_types.as_slice());

    let target_cmd = build_plugin_args(base.clone(), cx, package, target, KindTarget);
    let plugin_cmd = build_plugin_args(base, cx, package, target, KindPlugin);
    let target_cmd = build_deps_args(target_cmd, target, package, cx, KindTarget);
    let plugin_cmd = build_deps_args(plugin_cmd, target, package, cx, KindPlugin);

    match req {
        Target => vec![(target_cmd, KindTarget)],
        Plugin => vec![(plugin_cmd, KindPlugin)],
        PluginAndTarget if cx.config.target().is_none() =>
            vec![(target_cmd, KindTarget)],
        PluginAndTarget => vec![(target_cmd, KindTarget),
                                (plugin_cmd, KindPlugin)],
    }
}


fn rustdoc(package: &Package, target: &Target, cx: &mut Context) -> Work {
    let kind = KindTarget;
    let pkg_root = package.get_root();
    let cx_root = cx.layout(kind).proxy().dest().dir_path().join("doc");
    let rustdoc = process("rustdoc", package, cx).cwd(pkg_root.clone());
    let rustdoc = rustdoc.arg(target.get_src_path())
                         .arg("-o").arg(cx_root)
                         .arg("--crate-name").arg(target.get_name());
    let rustdoc = build_deps_args(rustdoc, target, package, cx, kind);

    log!(5, "commands={}", rustdoc);

    let _ = cx.config.shell().verbose(|shell| {
        shell.status("Running", rustdoc.to_string())
    });

    let primary = cx.primary;
    let name = package.get_name().to_string();
    proc() {
        if primary {
            try!(rustdoc.exec().chain_error(|| {
                human(format!("Could not document `{}`.", name))
            }))
        } else {
            try!(rustdoc.exec_with_output().and(Ok(())).map_err(|err| {
                caused_human(format!("Could not document `{}`.\n{}",
                                     name, err.output().unwrap()), err)
            }))
        }
        Ok(())
    }
}

fn build_base_args(mut cmd: ProcessBuilder,
                   target: &Target,
                   crate_types: &[&str]) -> ProcessBuilder {
    let metadata = target.get_metadata();

    // TODO: Handle errors in converting paths into args
    cmd = cmd.arg(target.get_src_path());

    cmd = cmd.arg("--crate-name").arg(target.get_name());

    for crate_type in crate_types.iter() {
        cmd = cmd.arg("--crate-type").arg(*crate_type);
    }

    let profile = target.get_profile();

    if profile.get_opt_level() != 0 {
        cmd = cmd.arg("--opt-level").arg(profile.get_opt_level().to_string());
    }

    // Right now -g is a little buggy, so we're not passing -g just yet
    // if profile.get_debug() {
    //     into.push("-g".to_string());
    // }

    if !profile.get_debug() {
        cmd = cmd.args(["--cfg", "ndebug"]);
    }

    if profile.is_test() {
        cmd = cmd.arg("--test");
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
    cmd = cmd.arg(cx.layout(kind).root());

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
                   cx: &Context, kind: Kind) -> ProcessBuilder {
    enum LinkReason { Dependency, LocalLib }

    let layout = cx.layout(kind);
    cmd = cmd.arg("-L").arg(layout.root());
    cmd = cmd.arg("-L").arg(layout.deps());

    // Traverse the entire dependency graph looking for -L paths to pass for
    // native dependencies.
    cmd = push_native_dirs(cmd, &layout, package, cx, &mut HashSet::new());

    for &(_, target) in cx.dep_targets(package).iter() {
        cmd = link_to(cmd, target, cx, kind, Dependency);
    }

    let mut targets = package.get_targets().iter().filter(|target| {
        target.is_lib() && target.get_profile().is_compile()
    });

    if target.is_bin() {
        for target in targets {
            cmd = link_to(cmd, target, cx, kind, LocalLib);
        }
    }

    return cmd;

    fn link_to(mut cmd: ProcessBuilder, target: &Target,
               cx: &Context, kind: Kind, reason: LinkReason) -> ProcessBuilder {
        // If this target is itself a plugin *or* if it's being linked to a
        // plugin, then we want the plugin directory. Otherwise we want the
        // target directory (hence the || here).
        let layout = cx.layout(match kind {
            KindPlugin => KindPlugin,
            KindTarget if target.get_profile().is_plugin() => KindPlugin,
            KindTarget => KindTarget,
        });

        for filename in cx.target_filenames(target).iter() {
            let mut v = Vec::new();
            v.push_all(target.get_name().as_bytes());
            v.push(b'=');
            match reason {
                Dependency => v.push_all(layout.deps().as_vec()),
                LocalLib => v.push_all(layout.root().as_vec()),
            }
            v.push(b'/');
            v.push_all(filename.as_bytes());
            cmd = cmd.arg("--extern").arg(v.as_slice());
        }
        return cmd;
    }

    fn push_native_dirs(mut cmd: ProcessBuilder, layout: &layout::LayoutProxy,
                        pkg: &Package, cx: &Context,
                        visited: &mut HashSet<PackageId>) -> ProcessBuilder {
        if !visited.insert(pkg.get_package_id().clone()) { return cmd }

        if pkg.get_manifest().get_build().len() > 0 {
            cmd = cmd.arg("-L").arg(layout.native(pkg));
        }

        match cx.resolve.deps(pkg.get_package_id()) {
            Some(mut pkgids) => {
                pkgids.fold(cmd, |cmd, dep_id| {
                    let dep = cx.get_package(dep_id);
                    push_native_dirs(cmd, layout, dep, cx, visited)
                })
            }
            None => cmd
        }
    }
}

pub fn process<T: ToCStr>(cmd: T, pkg: &Package, cx: &Context) -> ProcessBuilder {
    // When invoking a tool, we need the *host* deps directory in the dynamic
    // library search path for plugins and such which have dynamic dependencies.
    let mut search_path = DynamicLibrary::search_path();
    search_path.push(cx.layout(KindPlugin).deps().clone());
    let search_path = os::join_paths(search_path.as_slice()).unwrap();

    // We want to use the same environment and such as normal processes, but we
    // want to override the dylib search path with the one we just calculated.
    cx.compilation.process(cmd).cwd(pkg.get_root())
                               .env(DynamicLibrary::envvar(),
                                    Some(search_path.as_slice()))
}
