use std::collections::{HashMap, BTreeSet};
use std::fs;
use std::io::prelude::*;
use std::path::PathBuf;
use std::str;
use std::sync::{Mutex, Arc};

use core::{PackageId, PackageSet};
use util::{CargoResult, human, Human};
use util::{internal, ChainError, profile, paths};
use util::Freshness;

use super::job::Work;
use super::{fingerprint, process, Kind, Context, Unit};
use super::CommandType;

/// Contains the parsed output of a custom build script.
#[derive(Clone, Debug, Hash)]
pub struct BuildOutput {
    /// Paths to pass to rustc with the `-L` flag
    pub library_paths: Vec<PathBuf>,
    /// Names and link kinds of libraries, suitable for the `-l` flag
    pub library_links: Vec<String>,
    /// Various `--cfg` flags to pass to the compiler
    pub cfgs: Vec<String>,
    /// Metadata to pass to the immediate dependencies
    pub metadata: Vec<(String, String)>,
}

pub type BuildMap = HashMap<(PackageId, Kind), BuildOutput>;

pub struct BuildState {
    pub outputs: Mutex<BuildMap>,
}

#[derive(Default)]
pub struct BuildScripts {
    pub to_link: BTreeSet<(PackageId, Kind)>,
    pub plugins: BTreeSet<PackageId>,
}

/// Prepares a `Work` that executes the target as a custom build script.
///
/// The `req` given is the requirement which this run of the build script will
/// prepare work for. If the requirement is specified as both the target and the
/// host platforms it is assumed that the two are equal and the build script is
/// only run once (not twice).
pub fn prepare(cx: &mut Context, unit: &Unit)
               -> CargoResult<(Work, Work, Freshness)> {
    let _p = profile::start(format!("build script prepare: {}/{}",
                                    unit.pkg, unit.target.name()));
    let key = (unit.pkg.package_id().clone(), unit.kind);
    let overridden = cx.build_state.outputs.lock().unwrap().contains_key(&key);
    let (work_dirty, work_fresh) = if overridden {
        (Work::new(|_| Ok(())), Work::new(|_| Ok(())))
    } else {
        try!(build_work(cx, unit))
    };

    // Now that we've prep'd our work, build the work needed to manage the
    // fingerprint and then start returning that upwards.
    let (freshness, dirty, fresh) =
            try!(fingerprint::prepare_build_cmd(cx, unit));

    Ok((work_dirty.then(dirty), work_fresh.then(fresh), freshness))
}

fn build_work(cx: &mut Context, unit: &Unit) -> CargoResult<(Work, Work)> {
    let (script_output, build_output) = {
        (cx.layout(unit.pkg, Kind::Host).build(unit.pkg),
         cx.layout(unit.pkg, unit.kind).build_out(unit.pkg))
    };

    // Building the command to execute
    let to_exec = script_output.join(unit.target.name());

    // Start preparing the process to execute, starting out with some
    // environment variables. Note that the profile-related environment
    // variables are not set with this the build script's profile but rather the
    // package's library profile.
    let profile = cx.lib_profile(unit.pkg.package_id());
    let to_exec = to_exec.into_os_string();
    let mut p = try!(super::process(CommandType::Host(to_exec), unit.pkg, cx));
    p.env("OUT_DIR", &build_output)
     .env("CARGO_MANIFEST_DIR", unit.pkg.root())
     .env("NUM_JOBS", &cx.jobs().to_string())
     .env("TARGET", &match unit.kind {
         Kind::Host => &cx.config.rustc_info().host[..],
         Kind::Target => cx.target_triple(),
     })
     .env("DEBUG", &profile.debuginfo.to_string())
     .env("OPT_LEVEL", &profile.opt_level.to_string())
     .env("PROFILE", if cx.build_config.release {"release"} else {"debug"})
     .env("HOST", &cx.config.rustc_info().host);

    // Be sure to pass along all enabled features for this package, this is the
    // last piece of statically known information that we have.
    if let Some(features) = cx.resolve.features(unit.pkg.package_id()) {
        for feat in features.iter() {
            p.env(&format!("CARGO_FEATURE_{}", super::envify(feat)), "1");
        }
    }

    // Gather the set of native dependencies that this package has along with
    // some other variables to close over.
    //
    // This information will be used at build-time later on to figure out which
    // sorts of variables need to be discovered at that time.
    let lib_deps = {
        cx.dep_run_custom_build(unit).iter().filter_map(|unit| {
            if unit.profile.run_custom_build {
                Some((unit.pkg.manifest().links().unwrap().to_string(),
                      unit.pkg.package_id().clone()))
            } else {
                None
            }
        }).collect::<Vec<_>>()
    };
    let pkg_name = unit.pkg.to_string();
    let build_state = cx.build_state.clone();
    let id = unit.pkg.package_id().clone();
    let all = (id.clone(), pkg_name.clone(), build_state.clone(),
               build_output.clone());
    let build_scripts = super::load_build_deps(cx, unit);
    let kind = unit.kind;

    try!(fs::create_dir_all(&cx.layout(unit.pkg, Kind::Host).build(unit.pkg)));
    try!(fs::create_dir_all(&cx.layout(unit.pkg, unit.kind).build(unit.pkg)));

    let exec_engine = cx.exec_engine.clone();

    // Prepare the unit of "dirty work" which will actually run the custom build
    // command.
    //
    // Note that this has to do some extra work just before running the command
    // to determine extra environment variables and such.
    let dirty = Work::new(move |desc_tx| {
        // Make sure that OUT_DIR exists.
        //
        // If we have an old build directory, then just move it into place,
        // otherwise create it!
        if fs::metadata(&build_output).is_err() {
            try!(fs::create_dir(&build_output).chain_error(|| {
                internal("failed to create script output directory for \
                          build command")
            }));
        }

        // For all our native lib dependencies, pick up their metadata to pass
        // along to this custom build command. We're also careful to augment our
        // dynamic library search path in case the build script depended on any
        // native dynamic libraries.
        {
            let build_state = build_state.outputs.lock().unwrap();
            for (name, id) in lib_deps {
                let key = (id.clone(), kind);
                let state = try!(build_state.get(&key).chain_error(|| {
                    internal(format!("failed to locate build state for env \
                                      vars: {}/{:?}", id, kind))
                }));
                let data = &state.metadata;
                for &(ref key, ref value) in data.iter() {
                    p.env(&format!("DEP_{}_{}", super::envify(&name),
                                   super::envify(key)), value);
                }
            }
            if let Some(build_scripts) = build_scripts {
                try!(super::add_plugin_deps(&mut p, &build_state,
                                            &build_scripts));
            }
        }

        // And now finally, run the build command itself!
        desc_tx.send(p.to_string()).ok();
        let output = try!(exec_engine.exec_with_output(p).map_err(|mut e| {
            e.desc = format!("failed to run custom build command for `{}`\n{}",
                             pkg_name, e.desc);
            Human(e)
        }));
        try!(paths::write(&build_output.parent().unwrap().join("output"),
                          &output.stdout));

        // After the build command has finished running, we need to be sure to
        // remember all of its output so we can later discover precisely what it
        // was, even if we don't run the build command again (due to freshness).
        //
        // This is also the location where we provide feedback into the build
        // state informing what variables were discovered via our script as
        // well.
        let output = try!(str::from_utf8(&output.stdout).map_err(|_| {
            human("build script output was not valid utf-8")
        }));
        let parsed_output = try!(BuildOutput::parse(output, &pkg_name));
        build_state.insert(id, kind, parsed_output);
        Ok(())
    });

    // Now that we've prepared our work-to-do, we need to prepare the fresh work
    // itself to run when we actually end up just discarding what we calculated
    // above.
    let fresh = Work::new(move |_tx| {
        let (id, pkg_name, build_state, build_output) = all;
        let contents = try!(paths::read(&build_output.parent().unwrap()
                                                     .join("output")));
        let output = try!(BuildOutput::parse(&contents, &pkg_name));
        build_state.insert(id, kind, output);
        Ok(())
    });

    Ok((dirty, fresh))
}

impl BuildState {
    pub fn new(config: &super::BuildConfig,
               packages: &PackageSet) -> BuildState {
        let mut sources = HashMap::new();
        for package in packages.iter() {
            match package.manifest().links() {
                Some(links) => {
                    sources.insert(links.to_string(),
                                   package.package_id().clone());
                }
                None => {}
            }
        }
        let mut outputs = HashMap::new();
        let i1 = config.host.overrides.iter().map(|p| (p, Kind::Host));
        let i2 = config.target.overrides.iter().map(|p| (p, Kind::Target));
        for ((name, output), kind) in i1.chain(i2) {
            // If no package is using the library named `name`, then this is
            // just an override that we ignore.
            if let Some(id) = sources.get(name) {
                outputs.insert((id.clone(), kind), output.clone());
            }
        }
        BuildState { outputs: Mutex::new(outputs) }
    }

    fn insert(&self, id: PackageId, kind: Kind, output: BuildOutput) {
        self.outputs.lock().unwrap().insert((id, kind), output);
    }
}

impl BuildOutput {
    // Parses the output of a script.
    // The `pkg_name` is used for error messages.
    pub fn parse(input: &str, pkg_name: &str) -> CargoResult<BuildOutput> {
        let mut library_paths = Vec::new();
        let mut library_links = Vec::new();
        let mut cfgs = Vec::new();
        let mut metadata = Vec::new();
        let whence = format!("build script of `{}`", pkg_name);

        for line in input.lines() {
            let mut iter = line.splitn(2, ':');
            if iter.next() != Some("cargo") {
                // skip this line since it doesn't start with "cargo:"
                continue;
            }
            let data = match iter.next() {
                Some(val) => val,
                None => continue
            };

            // getting the `key=value` part of the line
            let mut iter = data.splitn(2, '=');
            let key = iter.next();
            let value = iter.next();
            let (key, value) = match (key, value) {
                (Some(a), Some(b)) => (a, b.trim_right()),
                // line started with `cargo:` but didn't match `key=value`
                _ => bail!("Wrong output in {}: `{}`", whence, line),
            };

            match key {
                "rustc-flags" => {
                    let (libs, links) = try!(
                        BuildOutput::parse_rustc_flags(value, &whence)
                    );
                    library_links.extend(links.into_iter());
                    library_paths.extend(libs.into_iter());
                }
                "rustc-link-lib" => library_links.push(value.to_string()),
                "rustc-link-search" => library_paths.push(PathBuf::from(value)),
                "rustc-cfg" => cfgs.push(value.to_string()),
                _ => metadata.push((key.to_string(), value.to_string())),
            }
        }

        Ok(BuildOutput {
            library_paths: library_paths,
            library_links: library_links,
            cfgs: cfgs,
            metadata: metadata,
        })
    }

    pub fn parse_rustc_flags(value: &str, whence: &str)
                             -> CargoResult<(Vec<PathBuf>, Vec<String>)> {
        let value = value.trim();
        let mut flags_iter = value.split(|c: char| c.is_whitespace())
                                  .filter(|w| w.chars().any(|c| !c.is_whitespace()));
        let (mut library_links, mut library_paths) = (Vec::new(), Vec::new());
        loop {
            let flag = match flags_iter.next() {
                Some(f) => f,
                None => break
            };
            if flag != "-l" && flag != "-L" {
                bail!("Only `-l` and `-L` flags are allowed in {}: `{}`",
                      whence, value)
            }
            let value = match flags_iter.next() {
                Some(v) => v,
                None => bail!("Flag in rustc-flags has no value in {}: `{}`",
                              whence, value)
            };
            match flag {
                "-l" => library_links.push(value.to_string()),
                "-L" => library_paths.push(PathBuf::from(value)),

                // was already checked above
                _ => bail!("only -l and -L flags are allowed")
            };
        }
        Ok((library_paths, library_links))
    }
}

/// Compute the `build_scripts` map in the `Context` which tracks what build
/// scripts each package depends on.
///
/// The global `build_scripts` map lists for all (package, kind) tuples what set
/// of packages' build script outputs must be considered. For example this lists
/// all dependencies' `-L` flags which need to be propagated transitively.
///
/// The given set of targets to this function is the initial set of
/// targets/profiles which are being built.
pub fn build_map<'b, 'cfg>(cx: &mut Context<'b, 'cfg>,
                           units: &[Unit<'b>]) {
    let mut ret = HashMap::new();
    for unit in units {
        build(&mut ret, cx, unit);
    }
    cx.build_scripts.extend(ret.into_iter().map(|(k, v)| {
        (k, Arc::new(v))
    }));

    // Recursive function to build up the map we're constructing. This function
    // memoizes all of its return values as it goes along.
    fn build<'a, 'b, 'cfg>(out: &'a mut HashMap<Unit<'b>, BuildScripts>,
                           cx: &Context<'b, 'cfg>,
                           unit: &Unit<'b>)
                           -> &'a BuildScripts {
        // Do a quick pre-flight check to see if we've already calculated the
        // set of dependencies.
        if out.contains_key(unit) {
            return &out[unit]
        }

        let mut to_link = BTreeSet::new();
        let mut plugins = BTreeSet::new();

        if !unit.target.is_custom_build() && unit.pkg.has_custom_build() {
            to_link.insert((unit.pkg.package_id().clone(), unit.kind));
        }
        for unit in cx.dep_targets(unit).iter() {
            let dep_scripts = build(out, cx, unit);

            if unit.target.for_host() {
                plugins.extend(dep_scripts.to_link.iter()
                                          .map(|p| &p.0).cloned());
            } else if unit.target.linkable() {
                to_link.extend(dep_scripts.to_link.iter().cloned());
            }
        }

        let prev = out.entry(*unit).or_insert(BuildScripts::default());
        prev.to_link.extend(to_link);
        prev.plugins.extend(plugins);
        return prev
    }
}
