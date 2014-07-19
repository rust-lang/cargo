use std::io::{fs, UserRWX};
use std::collections::HashSet;

use core::{Package, PackageId, PackageSet, Target, Resolve};
use util;
use util::{CargoResult, ProcessBuilder, CargoError, human};
use util::{Config, Freshness, internal, ChainError};

use self::job::Job;
use self::job_queue::JobQueue;
use self::context::{Context, PlatformRequirement, Target, Plugin, PluginAndTarget};

mod context;
mod fingerprint;
mod job;
mod job_queue;
mod layout;

type Args = Vec<String>;

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

pub fn compile_targets<'a>(env: &str, targets: &[&'a Target], pkg: &'a Package,
                           deps: &PackageSet, resolve: &'a Resolve,
                           config: &'a mut Config<'a>) -> CargoResult<()> {
    if targets.is_empty() {
        return Ok(());
    }

    debug!("compile_targets; targets={}; pkg={}; deps={}", targets, pkg, deps);

    let root = pkg.get_absolute_target_dir();
    let dest = uniq_target_dest(targets).unwrap_or("");
    let host_layout = layout::Layout::new(root.join(dest));
    let target_layout = config.target().map(|target| {
        layout::Layout::new(root.join(target).join(dest))
    });

    let mut cx = try!(Context::new(env, resolve, deps, config,
                                   host_layout, target_layout));

    // First ensure that the destination directory exists
    try!(cx.prepare(pkg));

    // Build up a list of pending jobs, each of which represent compiling a
    // particular package. No actual work is executed as part of this, that's
    // all done later as part of the `execute` function which will run
    // everything in order with proper parallelism.
    let mut jobs = Vec::new();
    for dep in deps.iter() {
        if dep == pkg { continue; }

        // Only compile lib targets for dependencies
        let targets = dep.get_targets().iter().filter(|target| {
            cx.is_relevant_target(*target)
        }).collect::<Vec<&Target>>();

        try!(compile(targets.as_slice(), dep, &mut cx, &mut jobs));
    }

    cx.primary();
    try!(compile(targets, pkg, &mut cx, &mut jobs));

    // Now that we've figured out everything that we're going to do, do it!
    JobQueue::new(cx.config, cx.resolve, jobs).execute()
}

fn compile<'a, 'b>(targets: &[&'a Target], pkg: &'a Package,
                   cx: &mut Context<'a, 'b>,
                   jobs: &mut Vec<(&'a Package, Freshness, (Job, Job))>)
                   -> CargoResult<()> {
    debug!("compile_pkg; pkg={}; targets={}", pkg, targets);

    if targets.is_empty() {
        return Ok(())
    }

    // First part of the build step of a target is to execute all of the custom
    // build commands.
    //
    // TODO: Should this be on the target or the package?
    let mut build_cmds = Vec::new();
    for build_cmd in pkg.get_manifest().get_build().iter() {
        build_cmds.push(try!(compile_custom(pkg, build_cmd.as_slice(), cx)));
    }

    // After the custom command has run, execute rustc for all targets of our
    // package.
    //
    // Note that bins can all be built in parallel because they all depend on
    // one another, but libs must be built sequentially because they may have
    // interdependencies.
    let (mut libs, mut bins) = (Vec::new(), Vec::new());
    for &target in targets.iter() {
        let req = cx.get_requirement(pkg, target);
        let jobs = rustc(pkg, target, cx, req);
        if target.is_lib() {
            libs.push_all_move(jobs);
        } else {
            bins.push_all_move(jobs);
        }
    }

    // Only after all the binaries have been built can we actually write the
    // fingerprint. Currently fingerprints are transactionally done per package,
    // not per-target.
    //
    // TODO: Can a fingerprint be per-target instead of per-package? Doing so
    //       would likely involve altering the granularity of key for the
    //       dependency queue that is later used to run jobs.
    let (freshness, write_fingerprint, copy_old) =
        try!(fingerprint::prepare(cx, pkg, targets));

    // Note that we build the job backwards because each job will produce more
    // work.
    let bins = Job::after(bins, write_fingerprint);
    let build_libs = Job::all(libs, bins);
    let job = Job::all(build_cmds, vec![build_libs]);

    jobs.push((pkg, freshness, (job, copy_old)));
    Ok(())
}

fn compile_custom(pkg: &Package, cmd: &str,
                  cx: &Context) -> CargoResult<Job> {
    // TODO: this needs to be smarter about splitting
    let mut cmd = cmd.split(' ');
    // TODO: this shouldn't explicitly pass `false` for dest/deps_dir, we may
    //       be building a C lib for a plugin
    let layout = cx.layout(false);
    let output = layout.native(pkg);
    if !output.exists() {
        try!(fs::mkdir(&output, UserRWX).chain_error(|| {
            internal("failed to create output directory for build command")
        }));
    }
    let mut p = util::process(cmd.next().unwrap())
                     .cwd(pkg.get_root())
                     .env("OUT_DIR", Some(output.as_str()
                                                .expect("non-UTF8 dest path")))
                     .env("DEPS_DIR", Some(output.as_str()
                                                 .expect("non-UTF8 dest path")))
                     .env("TARGET", cx.config.target());
    for arg in cmd {
        p = p.arg(arg);
    }
    Ok(Job::new(proc() {
        try!(p.exec_with_output().map(|_| ()).map_err(|e| e.mark_human()));
        Ok(Vec::new())
    }))
}

fn rustc(package: &Package, target: &Target,
         cx: &mut Context, req: PlatformRequirement) -> Vec<Job> {
    let crate_types = target.rustc_crate_types();
    let root = package.get_root();

    log!(5, "root={}; target={}; crate_types={}; verbose={}; req={}",
         root.display(), target, crate_types, cx.primary, req);

    let primary = cx.primary;
    let rustcs = prepare_rustc(package, target, crate_types, cx, req);

    log!(5, "commands={}", rustcs);

    let _ = cx.config.shell().verbose(|shell| {
        for rustc in rustcs.iter() {
            try!(shell.status("Running", rustc.to_string()));
        }
        Ok(())
    });

    rustcs.move_iter().map(|rustc| {
        Job::new(proc() {
            if primary {
                log!(5, "executing primary");
                try!(rustc.exec().map_err(|err| human(err.to_string())))
            } else {
                log!(5, "executing deps");
                try!(rustc.exec_with_output().and(Ok(())).map_err(|err| {
                    human(err.to_string())
                }))
            }
            Ok(Vec::new())
        })
    }).collect()
}

fn prepare_rustc(package: &Package, target: &Target, crate_types: Vec<&str>,
                 cx: &Context, req: PlatformRequirement) -> Vec<ProcessBuilder> {
    let root = package.get_root();
    let mut target_args = Vec::new();
    build_base_args(&mut target_args, target, crate_types.as_slice(), cx, false);
    build_deps_args(&mut target_args, package, cx, false);

    let mut plugin_args = Vec::new();
    build_base_args(&mut plugin_args, target, crate_types.as_slice(), cx, true);
    build_deps_args(&mut plugin_args, package, cx, true);

    let base = util::process("rustc").cwd(root.clone());

    match req {
        Target => vec![base.args(target_args.as_slice())],
        Plugin => vec![base.args(plugin_args.as_slice())],
        PluginAndTarget if cx.config.target().is_none() =>
            vec![base.args(target_args.as_slice())],
        PluginAndTarget =>
            vec![base.clone().args(target_args.as_slice()),
                 base.args(plugin_args.as_slice())],
    }
}

fn build_base_args(into: &mut Args,
                   target: &Target,
                   crate_types: &[&str],
                   cx: &Context,
                   plugin: bool) {
    let metadata = target.get_metadata();

    // TODO: Handle errors in converting paths into args
    into.push(target.get_src_path().display().to_string());

    into.push("--crate-name".to_string());
    into.push(target.get_name().to_string());

    for crate_type in crate_types.iter() {
        into.push("--crate-type".to_string());
        into.push(crate_type.to_string());
    }

    let profile = target.get_profile();

    if profile.get_opt_level() != 0 {
        into.push("--opt-level".to_string());
        into.push(profile.get_opt_level().to_string());
    }

    // Right now -g is a little buggy, so we're not passing -g just yet
    // if profile.get_debug() {
    //     into.push("-g".to_string());
    // }

    if !profile.get_debug() {
        into.push("--cfg".to_string());
        into.push("ndebug".to_string());
    }

    if profile.is_test() {
        into.push("--test".to_string());
    }

    match metadata {
        Some(m) => {
            into.push("-C".to_string());
            into.push(format!("metadata={}", m.metadata));

            into.push("-C".to_string());
            into.push(format!("extra-filename={}", m.extra_filename));
        }
        None => {}
    }

    into.push("--out-dir".to_string());
    into.push(cx.layout(plugin).root().display().to_string());

    if !plugin {
        fn opt(into: &mut Vec<String>, key: &str, prefix: &str,
               val: Option<&str>) {
            match val {
                Some(val) => {
                    into.push(key.to_string());
                    into.push(format!("{}{}", prefix, val));
                }
                None => {}
            }
        }

        opt(into, "--target", "", cx.config.target());
        opt(into, "-C", "ar=", cx.config.ar());
        opt(into, "-C", "linker=", cx.config.linker());
    }
}

fn build_deps_args(dst: &mut Args, package: &Package, cx: &Context,
                   plugin: bool) {
    let layout = cx.layout(plugin);
    dst.push("-L".to_string());
    dst.push(layout.root().display().to_string());
    dst.push("-L".to_string());
    dst.push(layout.deps().display().to_string());

    // Traverse the entire dependency graph looking for -L paths to pass for
    // native dependencies.
    push_native_dirs(dst, &layout, package, cx, &mut HashSet::new());

    for &(_, target) in cx.dep_targets(package).iter() {
        let layout = cx.layout(target.get_profile().is_plugin());
        for filename in cx.target_filenames(target).iter() {
            dst.push("--extern".to_string());
            dst.push(format!("{}={}/{}",
                     target.get_name(),
                     layout.deps().display(),
                     filename));
        }
    }

    fn push_native_dirs(dst: &mut Args, layout: &layout::LayoutProxy,
                        pkg: &Package, cx: &Context,
                        visited: &mut HashSet<PackageId>) {
        if !visited.insert(pkg.get_package_id().clone()) { return }

        if pkg.get_manifest().get_build().len() > 0 {
            dst.push("-L".to_string());
            dst.push(layout.native(pkg).display().to_string());
        }

        match cx.resolve.deps(pkg.get_package_id()) {
            Some(mut pkgids) => {
                for dep_id in pkgids {
                    let dep = cx.get_package(dep_id);
                    push_native_dirs(dst, layout, dep, cx, visited);
                }
            }
            None => {}
        }
    }
}
