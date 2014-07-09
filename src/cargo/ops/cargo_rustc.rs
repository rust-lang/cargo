use std::hash::Hasher;
use std::hash::sip::SipHasher;
use std::io::{File, IoError};
use std::io;
use std::os::args;
use std::str;
use term::color::YELLOW;

use core::{Package, PackageSet, Target};
use util;
use util::{CargoResult, ChainError, ProcessBuilder, internal, human, CargoError};
use util::{Config, TaskPool, DependencyQueue, Fresh, Dirty, Freshness};

type Args = Vec<String>;

struct Context<'a, 'b> {
    dest: &'a Path,
    deps_dir: &'a Path,
    primary: bool,
    rustc_version: &'a str,
    config: &'b mut Config<'b>
}

type Job = proc():Send -> CargoResult<()>;

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

pub fn compile_targets<'a>(targets: &[&Target], pkg: &Package, deps: &PackageSet,
                           config: &'a mut Config<'a>) -> CargoResult<()> {

    if targets.is_empty() {
        return Ok(());
    }

    debug!("compile_targets; targets={}; pkg={}; deps={}", targets, pkg, deps);

    let path_fragment = uniq_target_dest(targets);
    let target_dir = pkg.get_absolute_target_dir().join(path_fragment.unwrap_or(""));
    let deps_target_dir = target_dir.join("deps");

    let output = try!(util::process("rustc").arg("-v").exec_with_output());
    let rustc_version = str::from_utf8(output.output.as_slice()).unwrap();

    // First ensure that the destination directory exists
    debug!("creating target dir; path={}", target_dir.display());

    try!(mk_target(&target_dir).chain_error(||
        internal(format!("Couldn't create the target directory for {} at {}",
                 pkg.get_name(), target_dir.display()))));

    try!(mk_target(&deps_target_dir).chain_error(||
        internal(format!("Couldn't create the directory for dependencies for {} at {}",
                 pkg.get_name(), deps_target_dir.display()))));

    let mut cx = Context {
        dest: &deps_target_dir,
        deps_dir: &deps_target_dir,
        primary: false,
        rustc_version: rustc_version.as_slice(),
        config: config
    };

    // Build up a list of pending jobs, each of which represent compiling a
    // particular package. No actual work is executed as part of this, that's
    // all done later as part of the `execute` function which will run
    // everything in order with proper parallelism.
    let mut jobs = Vec::new();
    for dep in deps.iter() {
        // Only compile lib targets for dependencies
        let targets = dep.get_targets().iter().filter(|target| {
            target.is_lib() && target.get_profile().is_compile()
        }).collect::<Vec<&Target>>();

        jobs.push((dep,
                   try!(compile(targets.as_slice(), dep, &mut cx))));
    }

    cx.primary = true;
    cx.dest = &target_dir;
    jobs.push((pkg, try!(compile(targets, pkg, &mut cx))));

    // Now that we've figured out everything that we're going to do, do it!
    execute(cx.config, jobs)
}

fn compile(targets: &[&Target], pkg: &Package,
           cx: &mut Context) -> CargoResult<(Freshness, Job)> {
    debug!("compile_pkg; pkg={}; targets={}", pkg, pkg.get_targets());

    if targets.is_empty() {
        return Ok((Fresh, proc() Ok(())))
    }

    // First check to see if this package is fresh.
    //
    // Note that we're compiling things in topological order, so if nothing has
    // been built up to this point and we're fresh, then we can safely skip
    // recompilation. If anything has previously been rebuilt, it may have been
    // a dependency of ours, so just go ahead and rebuild ourselves.
    //
    // This is not quite accurate, we should only trigger forceful
    // recompilations for downstream dependencies of ourselves, not everyone
    // compiled afterwards.a
    //
    // TODO: Figure out how this works with targets
    let fingerprint_loc = cx.dest.join(format!(".{}.fingerprint",
                                               pkg.get_name()));

    let (is_fresh, fingerprint) = try!(is_fresh(pkg, &fingerprint_loc, cx,
                                                targets));

    let mut cmds = Vec::new();

    // TODO: Should this be on the target or the package?
    for build_cmd in pkg.get_manifest().get_build().iter() {
        cmds.push(compile_custom(pkg, build_cmd.as_slice(), cx));
    }

    // After the custom command has run, execute rustc for all targets of our
    // package.
    for &target in targets.iter() {
        cmds.push(rustc(&pkg.get_root(), target, cx));
    }

    cmds.push(proc() {
        // If this job runs, then everything has successfully compiled, so write
        // our new fingerprint to the relevant location to prevent
        // recompilations in the future.
        try!(File::create(&fingerprint_loc).write_str(fingerprint.as_slice()));
        Ok(())
    });

    // TODO: this job itself may internally be parallel, but we're hiding that
    //       currently. How to expose the parallelism among a single target?
    Ok((if is_fresh {Fresh} else {Dirty}, proc() {
        for cmd in cmds.move_iter() {
            try!(cmd());
        }
        Ok(())
    }))
}

fn is_fresh(dep: &Package, loc: &Path,
            cx: &mut Context, targets: &[&Target]) -> CargoResult<(bool, String)>
{
    let new_pkg_fingerprint = format!("{}{}", cx.rustc_version,
                                  try!(dep.get_fingerprint(cx.config)));

    let new_fingerprint = fingerprint(new_pkg_fingerprint, hash_targets(targets));

    let mut file = match File::open(loc) {
        Ok(file) => file,
        Err(..) => return Ok((false, new_fingerprint)),
    };

    let old_fingerprint = try!(file.read_to_string());

    log!(5, "old fingerprint: {}", old_fingerprint);
    log!(5, "new fingerprint: {}", new_fingerprint);

    Ok((old_fingerprint == new_fingerprint, new_fingerprint))
}

fn hash_targets(targets: &[&Target]) -> u64 {
    let hasher = SipHasher::new_with_keys(0,0);
    let targets = targets.iter().map(|t| (*t).clone()).collect::<Vec<Target>>();
    hasher.hash(&targets)
}

fn fingerprint(package: String, profiles: u64) -> String {
    let hasher = SipHasher::new_with_keys(0,0);
    util::to_hex(hasher.hash(&(package, profiles)))
}

fn mk_target(target: &Path) -> Result<(), IoError> {
    io::fs::mkdir_recursive(target, io::UserRWX)
}

fn compile_custom(pkg: &Package, cmd: &str,
                  cx: &Context) -> Job {
    // FIXME: this needs to be smarter about splitting
    let mut cmd = cmd.split(' ');
    let mut p = util::process(cmd.next().unwrap())
                     .cwd(pkg.get_root())
                     .env("OUT_DIR", Some(cx.dest.as_str().expect("non-UTF8 dest path")))
                     .env("DEPS_DIR", Some(cx.dest.join(cx.deps_dir)
                                             .as_str().expect("non-UTF8 deps path")));
    for arg in cmd {
        p = p.arg(arg);
    }
    proc() p.exec_with_output().map(|_| ()).map_err(|e| e.mark_human())
}

fn rustc(root: &Path, target: &Target, cx: &mut Context) -> Job {
    let crate_types = target.rustc_crate_types();

    log!(5, "root={}; target={}; crate_types={}; dest={}; deps={}; verbose={}",
         root.display(), target, crate_types, cx.dest.display(),
         cx.deps_dir.display(), cx.primary);

    let primary = cx.primary;
    let rustc = prepare_rustc(root, target, crate_types, cx);

    log!(5, "command={}", rustc);

    let _ = cx.config.shell().verbose(|shell| shell.status("Running", rustc.to_string()));

    proc() {
        if primary {
            rustc.exec().map_err(|err| human(err.to_string()))
        } else {
            rustc.exec_with_output().and(Ok(())).map_err(|err| {
                human(err.to_string())
            })
        }
    }
}

fn prepare_rustc(root: &Path, target: &Target, crate_types: Vec<&str>,
                 cx: &Context) -> ProcessBuilder {
    let mut args = Vec::new();

    build_base_args(&mut args, target, crate_types, cx);
    build_deps_args(&mut args, cx);

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

    let out = cx.dest.clone();
    let profile = target.get_profile();

    if profile.get_opt_level() != 0 {
        into.push("--opt-level".to_string());
        into.push(profile.get_opt_level().to_string());
    }

    if profile.get_debug() {
        into.push("-g".to_string());
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

    if target.is_lib() {
        into.push("--out-dir".to_string());
        into.push(out.display().to_string());
    } else {
        into.push("-o".to_string());
        into.push(out.join(target.get_name()).display().to_string());
    }
}

fn build_deps_args(dst: &mut Args, cx: &Context) {
    dst.push("-L".to_string());
    dst.push(cx.dest.display().to_string());
    dst.push("-L".to_string());
    dst.push(cx.deps_dir.display().to_string());
}

/// Execute all jobs necessary to build the dependency graph.
///
/// This function will spawn off `config.jobs()` workers to build all of the
/// necessary dependencies, in order. Freshness is propagated as far as possible
/// along each dependency chain.
fn execute(config: &mut Config,
           jobs: Vec<(&Package, (Freshness, Job))>) -> CargoResult<()> {
    let pool = TaskPool::new(config.jobs());
    let (tx, rx) = channel();
    let mut queue = DependencyQueue::new();
    for &(pkg, _) in jobs.iter() {
        queue.register(pkg);
    }
    for (pkg, (fresh, job)) in jobs.move_iter() {
        queue.enqueue(pkg, fresh, (pkg, job));
    }

    // Iteratively execute the dependency graph. Each turn of this loop will
    // schedule as much work as possible and then wait for one job to finish,
    // possibly scheduling more work afterwards.
    let mut active = 0i;
    while queue.len() > 0 {
        loop {
            match queue.dequeue() {
                Some((name, Fresh, (pkg, _))) => {
                    try!(config.shell().status("Fresh", pkg));
                    tx.send((name, Fresh, Ok(())));
                }
                Some((name, Dirty, (pkg, job))) => {
                    try!(config.shell().status("Compiling", pkg));
                    let my_tx = tx.clone();
                    pool.execute(proc() my_tx.send((name, Dirty, job())));
                }
                None => break,
            }
        }

        // Now that all possible work has been scheduled, wait for a piece of
        // work to finish. If any package fails to build then we stop scheduling
        // work as quickly as possibly.
        active -= 1;
        match rx.recv() {
            (name, fresh, Ok(())) => queue.finish(&name, fresh),
            (_, _, Err(e)) => {
                if active > 0 && config.jobs() > 1 {
                    try!(config.shell().say("Build failed, waiting for other \
                                             jobs to finish...", YELLOW));
                    for _ in rx.iter() {}
                }
                return Err(e)
            }
        }
    }

    log!(5, "rustc jobs completed");

    Ok(())
}
