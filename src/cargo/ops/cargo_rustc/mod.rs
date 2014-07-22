use std::collections::HashSet;
use std::dynamic_lib::DynamicLibrary;
use std::io::{fs, UserRWX};
use std::os;
use semver::Version;

use core::{SourceMap, Package, PackageId, PackageSet, Target, Resolve};
use util;
use util::{CargoResult, ProcessBuilder, CargoError, human, caused_human};
use util::{Config, Freshness, internal, ChainError};

use self::job::Job;
use self::job_queue::JobQueue;
use self::context::{Context, PlatformRequirement, Target, Plugin, PluginAndTarget};

mod context;
mod fingerprint;
mod job;
mod job_queue;
mod layout;

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
                           deps: &PackageSet, resolve: &'a Resolve, sources: &'a SourceMap,
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

    let mut cx = try!(Context::new(env, resolve, sources, deps, config,
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
    for (i, build_cmd) in pkg.get_manifest().get_build().iter().enumerate() {
        build_cmds.push(try!(compile_custom(pkg, build_cmd.as_slice(),
                                            cx, i == 0)));
    }

    // After the custom command has run, execute rustc for all targets of our
    // package.
    //
    // Note that bins can all be built in parallel because they all depend on
    // one another, but libs must be built sequentially because they may have
    // interdependencies.
    let (mut libs, mut bins) = (Vec::new(), Vec::new());
    for &target in targets.iter() {
        let jobs = if target.get_profile().is_doc() {
            vec![rustdoc(pkg, target, cx)]
        } else {
            let req = cx.get_requirement(pkg, target);
            rustc(pkg, target, cx, req)
        };
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
                  cx: &Context, first: bool) -> CargoResult<Job> {
    // TODO: this needs to be smarter about splitting
    let mut cmd = cmd.split(' ');
    // TODO: this shouldn't explicitly pass `false` for dest/deps_dir, we may
    //       be building a C lib for a plugin
    let layout = cx.layout(false);
    let output = layout.native(pkg);
    let mut p = process(cmd.next().unwrap(), pkg, cx)
                     .env("OUT_DIR", Some(&output))
                     .env("DEPS_DIR", Some(&output))
                     .env("TARGET", cx.config.target());
    for arg in cmd {
        p = p.arg(arg);
    }
    Ok(Job::new(proc() {
        if first {
            try!(fs::mkdir(&output, UserRWX).chain_error(|| {
                internal("failed to create output directory for build command")
            }));
        }
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
        let name = package.get_name().to_string();

        Job::new(proc() {
            if primary {
                log!(5, "executing primary");
                try!(rustc.exec().chain_error(|| human(format!("Could not compile `{}`.", name))))
            } else {
                log!(5, "executing deps");
                try!(rustc.exec_with_output().and(Ok(())).map_err(|err| {
                    caused_human(format!("Could not compile `{}`.\n{}",
                                         name, err.output().unwrap()), err)
                }))
            }
            Ok(Vec::new())
        })
    }).collect()
}

fn prepare_rustc(package: &Package, target: &Target, crate_types: Vec<&str>,
                 cx: &Context, req: PlatformRequirement) -> Vec<ProcessBuilder> {
    let base = process("rustc", package, cx);
    let base = build_base_args(base, target, crate_types.as_slice());

    let target_cmd = build_plugin_args(base.clone(), cx, false);
    let plugin_cmd = build_plugin_args(base, cx, true);
    let target_cmd = build_deps_args(target_cmd, target, package, cx, false);
    let plugin_cmd = build_deps_args(plugin_cmd, target, package, cx, true);

    match req {
        Target => vec![target_cmd],
        Plugin => vec![plugin_cmd],
        PluginAndTarget if cx.config.target().is_none() => vec![target_cmd],
        PluginAndTarget => vec![target_cmd, plugin_cmd],
    }
}


fn rustdoc(package: &Package, target: &Target, cx: &mut Context) -> Job {
    // Can't document binaries, but they have a doc target listed so we can
    // build documentation of dependencies even when `cargo doc` is run.
    if target.is_bin() {
        return Job::new(proc() Ok(Vec::new()))
    }

    let pkg_root = package.get_root();
    let cx_root = cx.layout(false).proxy().dest().dir_path().join("doc");
    let rustdoc = util::process("rustdoc").cwd(pkg_root.clone());
    let rustdoc = rustdoc.arg(target.get_src_path())
                         .arg("-o").arg(cx_root)
                         .arg("--crate-name").arg(target.get_name());
    let rustdoc = build_deps_args(rustdoc, target, package, cx, false);

    log!(5, "commands={}", rustdoc);

    let _ = cx.config.shell().verbose(|shell| {
        shell.status("Running", rustdoc.to_string())
    });

    let primary = cx.primary;
    let name = package.get_name().to_string();
    Job::new(proc() {
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
        Ok(Vec::new())
    })
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


fn build_plugin_args(mut cmd: ProcessBuilder, cx: &Context,
                     plugin: bool) -> ProcessBuilder {
    cmd = cmd.arg("--out-dir");
    cmd = cmd.arg(cx.layout(plugin).root());

    if !plugin {
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
                   cx: &Context, plugin: bool) -> ProcessBuilder {
    let layout = cx.layout(plugin);
    cmd = cmd.arg("-L").arg(layout.root());
    cmd = cmd.arg("-L").arg(layout.deps());

    // Traverse the entire dependency graph looking for -L paths to pass for
    // native dependencies.
    cmd = push_native_dirs(cmd, &layout, package, cx, &mut HashSet::new());

    for &(_, target) in cx.dep_targets(package).iter() {
        cmd = link_to(cmd, target, cx, true);
    }

    let mut targets = package.get_targets().iter().filter(|target| {
        target.is_lib() && target.get_profile().is_compile()
    });

    if target.is_bin() {
        for target in targets {
            cmd = link_to(cmd, target, cx, false);
        }
    }

    return cmd;

    fn link_to(mut cmd: ProcessBuilder, target: &Target,
               cx: &Context, is_dep_lib: bool) -> ProcessBuilder {
        let layout = cx.layout(target.get_profile().is_plugin());
        for filename in cx.target_filenames(target).iter() {
            let mut v = Vec::new();
            v.push_all(target.get_name().as_bytes());
            v.push(b'=');
            if is_dep_lib {
                v.push_all(layout.deps().as_vec());
            } else {
                v.push_all(layout.root().as_vec());
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
    search_path.push(cx.layout(false).deps().clone());
    let search_path = os::join_paths(search_path.as_slice()).unwrap();

    util::process(cmd)
        .cwd(pkg.get_root())
        .env(DynamicLibrary::envvar(), Some(search_path.as_slice()))
        .env("CARGO_PKG_VERSION_MAJOR", Some(pkg.get_version().major.to_string()))
        .env("CARGO_PKG_VERSION_MINOR", Some(pkg.get_version().minor.to_string()))
        .env("CARGO_PKG_VERSION_PATCH", Some(pkg.get_version().patch.to_string()))
        .env("CARGO_PKG_VERSION_PRE", pre_version_component(pkg.get_version()))
}

fn pre_version_component(v: &Version) -> Option<String> {
    if v.pre.is_empty() {
        return None;
    }

    let mut ret = String::new();

    for (i, x) in v.pre.iter().enumerate() {
        if i != 0 { ret.push_char('.') };
        ret.push_str(x.to_string().as_slice());
    }

    Some(ret)
}
