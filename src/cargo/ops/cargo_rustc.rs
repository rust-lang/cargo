use std::collections::HashMap;
use std::hash::Hasher;
use std::hash::sip::SipHasher;
use std::io::{File, IoError};
use std::io;
use std::os::args;
use std::str;
use std::sync::{atomics, Arc};
use term::color::YELLOW;

use core::{Package, PackageSet, Target, Resolve};
use util;
use util::{CargoResult, ChainError, ProcessBuilder, CargoError, internal, human};
use util::{Config, TaskPool, DependencyQueue, Fresh, Dirty, Freshness};

type Args = Vec<String>;

struct Context<'a, 'b> {
    dest: &'a Path,
    deps_dir: &'a Path,
    primary: bool,
    rustc_version: &'a str,
    resolve: &'a Resolve,
    package_set: &'a PackageSet,
    config: &'b mut Config<'b>,
    dylib: (String, String)
}

enum Job {
    Work(proc():Send -> CargoResult<Vec<Job>>),
}

impl Job {
    fn all(jobs: Vec<Job>, after: Vec<Job>) -> Job {
        Work(proc() {
            for Work(job) in jobs.move_iter() {
                try!(job());
            }
            Ok(after)
        })
    }
}

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

    let output = try!(util::process("rustc")
                      .arg("-")
                      .arg("--crate-name").arg("-")
                      .arg("--crate-type").arg("dylib")
                      .arg("--print-file-name")
                      .exec_with_output());

    let output = str::from_utf8(output.output.as_slice()).unwrap();

    let parts: Vec<&str> = output.slice_to(output.len() - 1).split('-').collect();
    assert!(parts.len() == 2, "rustc --print-file-name output has changed");

    let mut cx = Context {
        dest: &deps_target_dir,
        deps_dir: &deps_target_dir,
        primary: false,
        rustc_version: rustc_version.as_slice(),
        resolve: resolve,
        package_set: deps,
        config: config,
        dylib: (parts.get(0).to_string(), parts.get(1).to_string())
    };

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

    cx.primary = true;
    cx.dest = &target_dir;
    try!(compile(targets, pkg, &mut cx, &mut jobs));

    // Now that we've figured out everything that we're going to do, do it!
    execute(cx.config, jobs)
}

fn compile<'a>(targets: &[&Target], pkg: &'a Package, cx: &mut Context,
               jobs: &mut Vec<(&'a Package, Freshness, Job)>) -> CargoResult<()> {
    debug!("compile_pkg; pkg={}; targets={}", pkg, targets);

    if targets.is_empty() {
        return Ok(())
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
    let fingerprint_loc = cx.dest.join(format!(".{}.fingerprint",
                                               pkg.get_name()));

    let (is_fresh, fingerprint) = try!(is_fresh(pkg, &fingerprint_loc, cx,
                                                targets));

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
    let mut libs = Vec::new();
    let mut bins = Vec::new();
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
    let state = Arc::new(atomics::AtomicUint::new(bins.len()));
    let write_fingerprint = || {
        let (my_state, fingerprint_loc, fingerprint) =
            (state.clone(), fingerprint_loc.clone(), fingerprint.clone());
        Work(proc() {
            if my_state.load(atomics::SeqCst) == 0 {
                let mut file = try!(File::create(&fingerprint_loc));
                try!(file.write_str(fingerprint.as_slice()));
            }
            Ok(Vec::new())
        })
    };

    // Note that we build the job backwards because each job will produce more
    // work.
    let build_libs = if bins.len() == 0 {
        Job::all(libs, vec![write_fingerprint()])
    } else {
        Job::all(libs, bins.move_iter().map(|Work(bin)| {
            let my_state = state.clone();
            let write = write_fingerprint();
            Work(proc() {
                try!(bin());
                my_state.fetch_sub(1, atomics::SeqCst);
                Ok(vec![write])
            })
        }).collect())
    };
    let job = Job::all(build_cmds, vec![build_libs]);

    jobs.push((pkg, if is_fresh {Fresh} else {Dirty}, job));
    Ok(())
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
                                             .as_str().expect("non-UTF8 deps path")))
                     .env("TARGET", cx.config.target());
    for arg in cmd {
        p = p.arg(arg);
    }
    Work(proc() {
        try!(p.exec_with_output().map(|_| ()).map_err(|e| e.mark_human()));
        Ok(Vec::new())
    })
}

fn rustc(package: &Package, target: &Target, cx: &mut Context) -> Job {
    let crate_types = target.rustc_crate_types();
    let root = package.get_root();

    log!(5, "root={}; target={}; crate_types={}; dest={}; deps={}; verbose={}",
         root.display(), target, crate_types, cx.dest.display(),
         cx.deps_dir.display(), cx.primary);

    let primary = cx.primary;
    let rustc = prepare_rustc(package, target, crate_types, cx);

    log!(5, "command={}", rustc);

    let _ = cx.config.shell().verbose(|shell| {
        shell.status("Running", rustc.to_string())
    });

    Work(proc() {
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

    let out = cx.dest.clone();
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
    dst.push(cx.dest.display().to_string());
    dst.push("-L".to_string());
    dst.push(cx.deps_dir.display().to_string());

    for target in dep_targets(package, cx).iter() {
        dst.push("--extern".to_string());
        dst.push(format!("{}={}/{}",
                 target.get_name(),
                 cx.deps_dir.display(),
                 target_filename(target, cx)));
    }
}

fn target_filename(target: &Target, cx: &Context) -> String {
    let stem = target.file_stem();

    if target.is_dylib() {
        let (ref prefix, ref suffix) = cx.dylib;
        format!("{}{}{}", prefix, stem, suffix)
    } else if target.is_rlib() {
        format!("lib{}.rlib", stem)
    } else {
        unreachable!()
    }
}

fn dep_targets(pkg: &Package, cx: &Context) -> Vec<Target> {
    match cx.resolve.deps(pkg.get_package_id()) {
        None => vec!(),
        Some(deps) => deps
            .map(|pkg_id| {
                cx.package_set.iter()
                  .find(|pkg| pkg_id == pkg.get_package_id())
                  .expect("Should have found package")
            })
            .filter_map(|pkg| {
                pkg.get_targets().iter().find(|&t| t.is_lib() && t.get_profile().is_compile())
            })
            .map(|t| t.clone())
            .collect()
    }
}

/// Execute all jobs necessary to build the dependency graph.
///
/// This function will spawn off `config.jobs()` workers to build all of the
/// necessary dependencies, in order. Freshness is propagated as far as possible
/// along each dependency chain.
fn execute(config: &mut Config,
           jobs: Vec<(&Package, Freshness, Job)>) -> CargoResult<()> {
    let pool = TaskPool::new(config.jobs());
    let (tx, rx) = channel();
    let mut queue = DependencyQueue::new();
    for &(pkg, _, _) in jobs.iter() {
        queue.register(pkg);
    }
    for (pkg, fresh, job) in jobs.move_iter() {
        queue.enqueue(pkg, fresh, (pkg, job));
    }

    // Iteratively execute the dependency graph. Each turn of this loop will
    // schedule as much work as possible and then wait for one job to finish,
    // possibly scheduling more work afterwards.
    let mut active = HashMap::new();
    while queue.len() > 0 {
        loop {
            match queue.dequeue() {
                Some((name, Fresh, (pkg, _))) => {
                    assert!(active.insert(name.clone(), 1u));
                    try!(config.shell().status("Fresh", pkg));
                    tx.send((name, Fresh, Ok(Vec::new())));
                }
                Some((name, Dirty, (pkg, Work(job)))) => {
                    assert!(active.insert(name.clone(), 1));
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
        let (name, fresh, result) = rx.recv();
        *active.get_mut(&name) -= 1;
        match result {
            Ok(v) => {
                for Work(job) in v.move_iter() {
                    *active.get_mut(&name) += 1;
                    let my_tx = tx.clone();
                    let my_name = name.clone();
                    pool.execute(proc() {
                        my_tx.send((my_name, fresh, job()));
                    });
                }
                if *active.get(&name) == 0 {
                    active.remove(&name);
                    queue.finish(&name, fresh);
                }
            }
            Err(e) => {
                if *active.get(&name) == 0 {
                    active.remove(&name);
                }
                if active.len() > 0 && config.jobs() > 1 {
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
