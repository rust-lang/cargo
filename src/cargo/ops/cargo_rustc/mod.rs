use std::collections::HashSet;
use std::dynamic_lib::DynamicLibrary;
use std::io::{fs, BufferedReader, USER_RWX};
use std::io::fs::{File, PathExtensions};
use std::os;

use core::{SourceMap, Package, PackageId, PackageSet, Target, Resolve};
use util::{mod, CargoResult, ProcessBuilder, CargoError, human, caused_human};
use util::{Require, Config, internal, ChainError, Fresh, profile, join_paths};

use self::custom_build::CustomBuildCommandOutput;
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
mod custom_build;
mod fingerprint;
mod job;
mod job_queue;
mod layout;

#[deriving(PartialEq, Eq)]
pub enum Kind { KindHost, KindTarget }

/// Run `rustc` to figure out what its current version string is.
///
/// The second element of the tuple returned is the target triple that rustc
/// is a host for.
pub fn rustc_version() -> CargoResult<(String, String)> {
    let output = try!(util::process("rustc").arg("-v").arg("verbose")
                           .exec_with_output());
    let output = try!(String::from_utf8(output.output).map_err(|_| {
        internal("rustc -v didn't return utf8 output")
    }));
    let triple = {
        let triple = output.as_slice().lines().filter(|l| {
            l.starts_with("host: ")
        }).map(|l| l.slice_from(6)).next();
        let triple = try!(triple.require(|| {
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
                           config: &'a Config<'a>)
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
    let mut init = vec![(Job::new(target1, target2), Fresh)];
    if cx.config.target().is_some() {
        let (plugin1, plugin2) = fingerprint::prepare_init(cx, pkg, KindHost);
        init.push((Job::new(plugin1, plugin2), Fresh));
    }
    jobs.enqueue(pkg, jq::StageStart, init);

    // After the custom command has run, execute rustc for all targets of our
    // package.
    //
    // Each target has its own concept of freshness to ensure incremental
    // rebuilds on the *target* granularity, not the *package* granularity.
    let (mut builds, mut libs, mut bins, mut tests) = (Vec::new(), Vec::new(),
                                                       Vec::new(), Vec::new());
    for &target in targets.iter() {
        let work = if target.get_profile().is_doc() {
            let rustdoc = try!(rustdoc(pkg, target, cx));
            vec![(rustdoc, KindTarget)]
        } else {
            let req = cx.get_requirement(pkg, target);
            let mut rustc = try!(rustc(pkg, target, cx, req));

            if target.get_profile().is_custom_build() {
                for &(ref mut work, _) in rustc.iter_mut() {
                    use std::mem;

                    let (old_build, script_output) = {
                        let layout = cx.layout(pkg, KindHost);
                        let old_build = layout.proxy().old_build(pkg);
                        let script_output = layout.build(pkg);
                        (old_build, script_output)
                    };

                    let execute_cmd = try!(custom_build::prepare_execute_custom_build(pkg,
                                                                                      target,
                                                                                      cx));

                    // building a `Work` that creates the directory where the compiled script
                    // must be placed
                    let create_directory = proc() {
                        if old_build.exists() {
                            fs::rename(&old_build, &script_output)
                        } else {
                            fs::mkdir_recursive(&script_output, USER_RWX)
                        }.chain_error(|| {
                            internal("failed to create script output directory for build command")
                        })
                    };

                    // replacing the simple rustc compilation by three steps:
                    // 1 - create the output directory
                    // 2 - call rustc
                    // 3 - execute the command
                    let rustc_cmd = mem::replace(work, proc(_) Ok(()));
                    let replacement = proc(desc_tx: Sender<String>) {
                        try!(create_directory());
                        try!(rustc_cmd(desc_tx.clone()));
                        execute_cmd(desc_tx)
                    };
                    mem::replace(work, replacement);
                }
            }

            rustc
        };

        let dst = match (target.is_lib(),
                         target.get_profile().is_test(),
                         target.get_profile().is_custom_build()) {
            (_, _, true) => &mut builds,
            (_, true, _) => &mut tests,
            (true, _, _) => &mut libs,
            (false, false, _) if target.get_profile().get_env() == "test" => &mut tests,
            (false, false, _) => &mut bins,
        };
        for (work, kind) in work.into_iter() {
            let (freshness, dirty, fresh) =
                try!(fingerprint::prepare_target(cx, pkg, target, kind));

            let dirty = proc(desc_tx: Sender<String>) {
                try!(work(desc_tx.clone()));
                dirty(desc_tx)
            };
            dst.push((job(dirty, fresh), freshness));
        }
    }

    if builds.len() >= 1 {
        // New custom build system
        jobs.enqueue(pkg, jq::StageCustomBuild, builds);

    } else {
        // Old custom build system
        // TODO: deprecated, remove
        let mut build_cmds = Vec::new();
        for (i, build_cmd) in pkg.get_manifest().get_build().iter().enumerate() {
            let work = try!(compile_custom_old(pkg, build_cmd.as_slice(), cx, i == 0));
            build_cmds.push(work);
        }
        let (freshness, dirty, fresh) =
            try!(fingerprint::prepare_build_cmd(cx, pkg));
        let desc = match build_cmds.len() {
            0 => String::new(),
            1 => pkg.get_manifest().get_build()[0].to_string(),
            _ => format!("custom build commands"),
        };
        let dirty = proc(desc_tx: Sender<String>) {
            desc_tx.send_opt(desc).ok();
            for cmd in build_cmds.into_iter() { try!(cmd(desc_tx.clone())) }
            dirty(desc_tx)
        };
        jobs.enqueue(pkg, jq::StageCustomBuild, vec![(job(dirty, fresh),
                                                      freshness)]);
    }

    jobs.enqueue(pkg, jq::StageLibraries, libs);
    jobs.enqueue(pkg, jq::StageBinaries, bins);
    jobs.enqueue(pkg, jq::StageTests, tests);
    Ok(())
}

// TODO: deprecated, remove
fn compile_custom_old(pkg: &Package, cmd: &str,
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
    let mut p = try!(process(cmd.next().unwrap(), pkg, cx))
                     .env("OUT_DIR", Some(&output))
                     .env("DEPS_DIR", Some(&output))
                     .env("TARGET", Some(cx.target_triple()))
                     .env("DEBUG", Some(profile.get_debug().to_string()))
                     .env("OPT_LEVEL", Some(profile.get_opt_level().to_string()))
                     .env("PROFILE", Some(profile.get_env()));
    for arg in cmd {
        p = p.arg(arg);
    }
    match cx.resolve.features(pkg.get_package_id()) {
        Some(features) => {
            for feat in features.iter() {
                let feat = feat.as_slice().chars()
                               .map(|c| c.to_uppercase())
                               .map(|c| if c == '-' {'_'} else {c})
                               .collect::<String>();
                p = p.env(format!("CARGO_FEATURE_{}", feat).as_slice(), Some("1"));
            }
        }
        None => {}
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

    Ok(proc(desc_tx: Sender<String>) {
        desc_tx.send_opt(p.to_string()).ok();
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
            e.concrete().mark_human()
        }));
        Ok(())
    })
}

fn rustc(package: &Package, target: &Target,
         cx: &mut Context, req: PlatformRequirement)
         -> CargoResult<Vec<(Work, Kind)> >{
    let crate_types = target.rustc_crate_types();
    let rustcs = try!(prepare_rustc(package, target, crate_types, cx, req));

    Ok(rustcs.into_iter().map(|(rustc, kind)| {
        let name = package.get_name().to_string();
        let is_path_source = package.get_package_id().get_source_id().is_path();
        let show_warnings = package.get_package_id() == cx.resolve.root() ||
                            is_path_source;
        let rustc = if show_warnings {rustc} else {rustc.arg("-Awarnings")};
        let build_cmd_layout = cx.layout(package, KindHost);

        // building the possible `build/$pkg/output` file for this local package
        let command_output_file = build_cmd_layout.build(package).join("output");

        // building the list of all possible `build/$pkg/output` files
        // whether they exist or not will be checked during the work
        let command_output_files = cx.dep_targets(package).iter().map(|&(pkg, _)| {
            build_cmd_layout.build(pkg).join("output")
        }).collect::<Vec<_>>();

        (proc(desc_tx: Sender<String>) {
            let mut rustc = rustc;

            let mut additional_library_paths = Vec::new();

            // list of `-l` flags to pass to rustc coming from custom build scripts
            let additional_library_links = match File::open(&command_output_file) {
                Ok(f) => {
                    let flags = try!(CustomBuildCommandOutput::parse(
                        BufferedReader::new(f), name.as_slice()));

                    additional_library_paths.extend(flags.library_paths.iter().map(|p| p.clone()));
                    flags.library_links.clone()
                },
                Err(_) => Vec::new()
            };

            // loading each possible custom build output file to fill `additional_library_paths`
            for flags_file in command_output_files.into_iter() {
                let flags = match File::open(&flags_file) {
                    Ok(f) => f,
                    Err(_) => continue  // the file doesn't exist, probably means that this pkg
                                        // doesn't have a build command
                };

                let flags = try!(CustomBuildCommandOutput::parse(
                    BufferedReader::new(flags), name.as_slice()));
                additional_library_paths.extend(flags.library_paths.iter().map(|p| p.clone()));
            }

            for p in additional_library_paths.into_iter() {
                rustc = rustc.arg("-L").arg(p);
            }
            for lib in additional_library_links.into_iter() {
                rustc = rustc.arg("-l").arg(lib);
            }

            desc_tx.send_opt(rustc.to_string()).ok();
            try!(rustc.exec().chain_error(|| {
                human(format!("Could not compile `{}`.", name))
            }));

            Ok(())

        }, kind)
    }).collect())
}

fn prepare_rustc(package: &Package, target: &Target, crate_types: Vec<&str>,
                 cx: &Context, req: PlatformRequirement)
                 -> CargoResult<Vec<(ProcessBuilder, Kind)>> {
    let base = try!(process("rustc", package, cx));
    let base = build_base_args(cx, base, package, target, crate_types.as_slice());

    let target_cmd = build_plugin_args(base.clone(), cx, package, target, KindTarget);
    let plugin_cmd = build_plugin_args(base, cx, package, target, KindHost);
    let target_cmd = try!(build_deps_args(target_cmd, target, package, cx,
                                          KindTarget));
    let plugin_cmd = try!(build_deps_args(plugin_cmd, target, package, cx,
                                          KindHost));

    Ok(match req {
        PlatformTarget => vec![(target_cmd, KindTarget)],
        PlatformPlugin => vec![(plugin_cmd, KindHost)],
        PlatformPluginAndTarget if cx.config.target().is_none() =>
            vec![(target_cmd, KindTarget)],
        PlatformPluginAndTarget => vec![(target_cmd, KindTarget),
                                        (plugin_cmd, KindHost)],
    })
}


fn rustdoc(package: &Package, target: &Target,
           cx: &mut Context) -> CargoResult<Work> {
    let kind = KindTarget;
    let pkg_root = package.get_root();
    let cx_root = cx.layout(package, kind).proxy().dest().join("doc");
    let rustdoc = try!(process("rustdoc", package, cx)).cwd(pkg_root.clone());
    let mut rustdoc = rustdoc.arg(target.get_src_path())
                         .arg("-o").arg(cx_root)
                         .arg("--crate-name").arg(target.get_name());

    match cx.resolve.features(package.get_package_id()) {
        Some(features) => {
            for feat in features.iter() {
                rustdoc = rustdoc.arg("--cfg").arg(format!("feature=\"{}\"", feat));
            }
        }
        None => {}
    }

    let rustdoc = try!(build_deps_args(rustdoc, target, package, cx, kind));

    log!(5, "commands={}", rustdoc);

    let primary = package.get_package_id() == cx.resolve.root();
    let name = package.get_name().to_string();
    let desc = rustdoc.to_string();
    Ok(proc(desc_tx: Sender<String>) {
        desc_tx.send(desc);
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
    })
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
                         .debug(root_profile.get_debug())
                         .rpath(root_profile.get_rpath())
    }

    if profile.is_plugin() {
        cmd = cmd.arg("-C").arg("prefer-dynamic");
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

    if profile.get_rpath() {
        cmd = cmd.arg("-C").arg("rpath");
    }

    return cmd;
}


fn build_plugin_args(mut cmd: ProcessBuilder, cx: &Context, pkg: &Package,
                     target: &Target, kind: Kind) -> ProcessBuilder {
    let out_dir = cx.layout(pkg, kind);
    let out_dir = if target.get_profile().is_custom_build() {
        out_dir.build(pkg)
    } else if target.is_example() {
        out_dir.examples().clone()
    } else {
        out_dir.root().clone()
    };

    cmd = cmd.arg("--out-dir");
    cmd = cmd.arg(out_dir);

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
        cmd = opt(cmd, "-C", "ar=", cx.config.ar().as_ref()
                                             .map(|s| s.as_slice()));
        cmd = opt(cmd, "-C", "linker=", cx.config.linker().as_ref()
                                                 .map(|s| s.as_slice()));
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
    // TODO: deprecated, remove
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
            KindHost => KindHost,
            KindTarget if target.get_profile().is_for_host() => KindHost,
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

pub fn process<T: ToCStr>(cmd: T, pkg: &Package,
                          cx: &Context) -> CargoResult<ProcessBuilder> {
    // When invoking a tool, we need the *host* deps directory in the dynamic
    // library search path for plugins and such which have dynamic dependencies.
    let layout = cx.layout(pkg, KindHost);
    let mut search_path = DynamicLibrary::search_path();
    search_path.push(layout.deps().clone());

    // TODO: deprecated, remove
    // Also be sure to pick up any native build directories required by plugins
    // or their dependencies
    let mut native_search_paths = HashSet::new();
    for &(dep, target) in cx.dep_targets(pkg).iter() {
        if !target.get_profile().is_for_host() { continue }
        each_dep(dep, cx, |dep| {
            if dep.get_manifest().get_build().len() > 0 {
                native_search_paths.insert(layout.native(dep));
            }
        });
    }
    search_path.extend(native_search_paths.into_iter());

    // We want to use the same environment and such as normal processes, but we
    // want to override the dylib search path with the one we just calculated.
    let search_path = try!(join_paths(search_path.as_slice(),
                                      DynamicLibrary::envvar()));
    Ok(try!(cx.compilation.process(cmd, pkg))
              .env(DynamicLibrary::envvar(), Some(search_path.as_slice())))
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
