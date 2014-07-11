use core::{Package, PackageSet, Target, Resolve};
use util;
use util::{CargoResult, ProcessBuilder, CargoError, human};
use util::{Config, Freshness};

use self::job::Job;
use self::job_queue::JobQueue;
use self::context::Context;

mod context;
mod fingerprint;
mod job;
mod job_queue;

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

pub fn compile_targets<'a>(env: &str, targets: &[&Target], pkg: &Package,
                           deps: &PackageSet, resolve: &'a Resolve,
                           config: &'a mut Config<'a>) -> CargoResult<()>
{
    if targets.is_empty() {
        return Ok(());
    }

    debug!("compile_targets; targets={}; pkg={}; deps={}", targets, pkg, deps);

    let target_dir = pkg.get_absolute_target_dir()
                        .join(config.target().unwrap_or(""))
                        .join(uniq_target_dest(targets).unwrap_or(""));
    let deps_target_dir = target_dir.join("deps");

    let mut cx = try!(Context::new(resolve, deps, config,
                                   target_dir, deps_target_dir));

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
            target.is_lib() && match env {
                "test" => target.get_profile().is_compile(),
                _ => target.get_profile().get_env() == env,
            }
        }).collect::<Vec<&Target>>();

        try!(compile(targets.as_slice(), dep, &mut cx, &mut jobs));
    }

    cx.primary();
    try!(compile(targets, pkg, &mut cx, &mut jobs));

    // Now that we've figured out everything that we're going to do, do it!
    JobQueue::new(cx.config, jobs).execute()
}

fn compile<'a>(targets: &[&Target], pkg: &'a Package, cx: &mut Context,
               jobs: &mut Vec<(&'a Package, Freshness, Job)>) -> CargoResult<()> {
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
        build_cmds.push(compile_custom(pkg, build_cmd.as_slice(), cx));
    }

    // After the custom command has run, execute rustc for all targets of our
    // package.
    //
    // Note that bins can all be built in parallel because they all depend on
    // one another, but libs must be built sequentially because they may have
    // interdependencies.
    let (mut libs, mut bins) = (Vec::new(), Vec::new());
    for &target in targets.iter() {
        let job = rustc(pkg, target, cx);
        if target.is_lib() {
            libs.push(job);
        } else {
            bins.push(job);
        }
    }

    // Only after all the binaries have been built can we actually write the
    // fingerprint. Currently fingerprints are transactionally done per package,
    // not per-target.
    //
    // TODO: Can a fingerprint be per-target instead of per-package? Doing so
    //       would likely involve altering the granularity of key for the
    //       dependency queue that is later used to run jobs.
    let (freshness, write_fingerprint) =
        try!(fingerprint::prepare(cx, pkg, targets));

    // Note that we build the job backwards because each job will produce more
    // work.
    let bins = Job::after(bins, write_fingerprint);
    let build_libs = Job::all(libs, bins);
    let job = Job::all(build_cmds, vec![build_libs]);

    jobs.push((pkg, freshness, job));
    Ok(())
}

fn compile_custom(pkg: &Package, cmd: &str,
                  cx: &Context) -> Job {
    // FIXME: this needs to be smarter about splitting
    let mut cmd = cmd.split(' ');
    let mut p = util::process(cmd.next().unwrap())
                     .cwd(pkg.get_root())
                     .env("OUT_DIR", Some(cx.dest().as_str()
                                            .expect("non-UTF8 dest path")))
                     .env("DEPS_DIR", Some(cx.deps_dir.as_str()
                                             .expect("non-UTF8 deps path")))
                     .env("TARGET", cx.config.target());
    for arg in cmd {
        p = p.arg(arg);
    }
    Job::new(proc() {
        try!(p.exec_with_output().map(|_| ()).map_err(|e| e.mark_human()));
        Ok(Vec::new())
    })
}

fn rustc(package: &Package, target: &Target, cx: &mut Context) -> Job {
    let crate_types = target.rustc_crate_types();
    let root = package.get_root();

    log!(5, "root={}; target={}; crate_types={}; dest={}; deps={}; verbose={}",
         root.display(), target, crate_types, cx.dest().display(),
         cx.deps_dir.display(), cx.primary);

    let primary = cx.primary;
    let rustc = prepare_rustc(package, target, crate_types, cx);

    log!(5, "command={}", rustc);

    let _ = cx.config.shell().verbose(|shell| {
        shell.status("Running", rustc.to_string())
    });

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
}

fn prepare_rustc(package: &Package, target: &Target, crate_types: Vec<&str>,
                 cx: &Context) -> ProcessBuilder
{
    let root = package.get_root();
    let mut args = Vec::new();

    build_base_args(&mut args, target, crate_types, cx);
    build_deps_args(&mut args, package, cx);

    util::process("rustc")
        .cwd(root.clone())
        .args(args.as_slice())
        .env("RUST_LOG", None) // rustc is way too noisy
}

fn build_base_args(into: &mut Args,
                   target: &Target,
                   crate_types: Vec<&str>,
                   cx: &Context)
{
    let metadata = target.get_metadata();

    // TODO: Handle errors in converting paths into args
    into.push(target.get_src_path().display().to_string());

    into.push("--crate-name".to_string());
    into.push(target.get_name().to_string());

    for crate_type in crate_types.iter() {
        into.push("--crate-type".to_string());
        into.push(crate_type.to_string());
    }

    let out = cx.dest().clone();
    let profile = target.get_profile();

    if profile.get_opt_level() != 0 {
        into.push("--opt-level".to_string());
        into.push(profile.get_opt_level().to_string());
    }

    // Right now -g is a little buggy, so we're not passing -g just yet
    // if profile.get_debug() {
    //     into.push("-g".to_string());
    // }

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

    if target.is_lib() {
        into.push("--out-dir".to_string());
        into.push(out.display().to_string());
    } else {
        into.push("-o".to_string());
        into.push(out.join(target.get_name()).display().to_string());
    }

    match cx.config.target() {
        Some(target) if !profile.is_plugin() => {
            into.push("--target".to_string());
            into.push(target.to_string());
        }
        _ => {}
    }
}

fn build_deps_args(dst: &mut Args, package: &Package, cx: &Context) {
    dst.push("-L".to_string());
    dst.push(cx.dest().display().to_string());
    dst.push("-L".to_string());
    dst.push(cx.deps_dir.display().to_string());

    for target in cx.dep_targets(package).iter() {
        dst.push("--extern".to_string());
        dst.push(format!("{}={}/{}",
                 target.get_name(),
                 cx.deps_dir.display(),
                 cx.target_filename(target)));
    }
}
