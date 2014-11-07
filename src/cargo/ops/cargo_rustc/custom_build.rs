use std::collections::HashMap;
use std::fmt;
use std::io::fs::PathExtensions;
use std::io::{fs, USER_RWX, File};
use std::str;
use std::sync::Mutex;

use core::{Package, Target, PackageId, PackageSet};
use util::{CargoResult, CargoError, human};
use util::{internal, ChainError, Require};

use super::job::Work;
use super::{fingerprint, process, KindTarget, KindHost, Kind, Context};
use util::Freshness;

/// Contains the parsed output of a custom build script.
#[deriving(Clone)]
pub struct BuildOutput {
    /// Paths to pass to rustc with the `-L` flag
    pub library_paths: Vec<Path>,
    /// Names and link kinds of libraries, suitable for the `-l` flag
    pub library_links: Vec<String>,
    /// Metadata to pass to the immediate dependencies
    pub metadata: Vec<(String, String)>,
}

pub struct BuildState {
    pub outputs: Mutex<HashMap<(PackageId, Kind), BuildOutput>>,
}

/// Prepares a `Work` that executes the target as a custom build script.
pub fn prepare(pkg: &Package, target: &Target, kind: Kind, cx: &mut Context)
               -> CargoResult<(Work, Work, Freshness)> {
    // TODO: this shouldn't explicitly pass `KindTarget` for the layout, we
    //       may be running a build script for a plugin dependency.
    let (script_output, old_script_output, build_output, old_build_output) = {
        let target = cx.layout(pkg, kind);
        let host = cx.layout(pkg, KindHost);
        (host.build(pkg),
         host.proxy().old_build(pkg),
         target.build_out(pkg),
         target.proxy().old_build(pkg).join("out"))
    };

    // Building the command to execute
    let to_exec = try!(cx.target_filenames(target))[0].clone();
    let to_exec = script_output.join(to_exec);

    // Start preparing the process to execute, starting out with some
    // environment variables.
    let profile = target.get_profile();
    let mut p = try!(super::process(to_exec, pkg, target, cx))
                     .env("OUT_DIR", Some(&build_output))
                     .env("CARGO_MANIFEST_DIR", Some(pkg.get_manifest_path()
                                                        .dir_path()
                                                        .display().to_string()))
                     .env("NUM_JOBS", Some(cx.config.jobs().to_string()))
                     .env("TARGET", Some(match kind {
                         KindHost => cx.config.rustc_host(),
                         KindTarget => cx.target_triple(),
                     }))
                     .env("DEBUG", Some(profile.get_debug().to_string()))
                     .env("OPT_LEVEL", Some(profile.get_opt_level().to_string()))
                     .env("PROFILE", Some(profile.get_env()));

    // Be sure to pass along all enabled features for this package, this is the
    // last piece of statically known information that we have.
    match cx.resolve.features(pkg.get_package_id()) {
        Some(features) => {
            for feat in features.iter() {
                p = p.env(format!("CARGO_FEATURE_{}",
                                  super::envify(feat.as_slice())).as_slice(),
                          Some("1"));
            }
        }
        None => {}
    }

    // Gather the set of native dependencies that this package has along with
    // some other variables to close over.
    //
    // This information will be used at build-time later on to figure out which
    // sorts of variables need to be discovered at that time.
    let lib_deps = {
        let non_build_target = pkg.get_targets().iter().find(|t| {
            !t.get_profile().is_custom_build()
        }).unwrap();
        cx.dep_targets(pkg, non_build_target).iter().filter_map(|&(pkg, _)| {
            pkg.get_manifest().get_links().map(|links| {
                (links.to_string(), pkg.get_package_id().clone())
            })
        }).collect::<Vec<_>>()
    };
    let pkg_name = pkg.to_string();
    let build_state = cx.build_state.clone();
    let id = pkg.get_package_id().clone();
    let all = (id.clone(), pkg_name.clone(), build_state.clone(),
               script_output.clone(), old_build_output.clone(),
               build_output.clone());

    try!(fs::mkdir_recursive(&cx.layout(pkg, KindTarget).build(pkg), USER_RWX));
    try!(fs::mkdir_recursive(&cx.layout(pkg, KindHost).build(pkg), USER_RWX));

    // Prepare the unit of "dirty work" which will actually run the custom build
    // command.
    //
    // Note that this has to do some extra work just before running the command
    // to determine extra environment variables and such.
    let work = proc(desc_tx: Sender<String>) {
        // Make sure that OUT_DIR exists.
        //
        // If we have an old build directory, then just move it into place,
        // otherwise create it!
        try!(if old_build_output.exists() {
            fs::rename(&old_build_output, &build_output)
        } else {
            fs::mkdir(&build_output, USER_RWX)
        }.chain_error(|| {
            internal("failed to create script output directory for \
                      build command")
        }));

        // For all our native lib dependencies, pick up their metadata to pass
        // along to this custom build command.
        let mut p = p;
        {
            let build_state = build_state.outputs.lock();
            for &(ref name, ref id) in lib_deps.iter() {
                let data = &build_state[(id.clone(), kind)].metadata;
                for &(ref key, ref value) in data.iter() {
                    p = p.env(format!("DEP_{}_{}",
                                      super::envify(name.as_slice()),
                                      super::envify(key.as_slice())).as_slice(),
                              Some(value.as_slice()));
                }
            }
        }

        // And now finally, run the build command itself!
        desc_tx.send_opt(p.to_string()).ok();
        let output = try!(p.exec_with_output().map_err(|mut e| {
            e.msg = format!("Failed to run custom build command for `{}`\n{}",
                            pkg_name, e.msg);
            e.concrete().mark_human()
        }));

        // After the build command has finished running, we need to be sure to
        // remember all of its output so we can later discover precisely what it
        // was, even if we don't run the build command again (due to freshness).
        //
        // This is also the location where we provide feedback into the build
        // state informing what variables were discovered via our script as
        // well.
        let output = try!(str::from_utf8(output.output.as_slice()).require(|| {
            human("build script output was not valid utf-8")
        }));
        let build_output = try!(BuildOutput::parse(output, pkg_name.as_slice()));
        build_state.outputs.lock().insert((id, kind), build_output);

        try!(File::create(&script_output.join("output"))
                  .write_str(output).map_err(|e| {
            human(format!("failed to write output of custom build command: {}",
                          e))
        }));

        Ok(())
    };

    // Now that we've prepared our work-to-do, we need to prepare the fresh work
    // itself to run when we actually end up just discarding what we calculated
    // above.
    //
    // Note that the freshness calculation here is the build_cmd freshness, not
    // target specific freshness. This is because we don't actually know what
    // the inputs are to this command!
    //
    // Also note that a fresh build command needs to
    let (freshness, dirty, fresh) =
            try!(fingerprint::prepare_build_cmd(cx, pkg, Some(target)));
    let dirty = proc(tx: Sender<String>) { try!(work(tx.clone())); dirty(tx) };
    let fresh = proc(tx) {
        let (id, pkg_name, build_state, script_output,
             old_build_output, build_output) = all;
        let new_loc = script_output.join("output");
        try!(fs::rename(&old_script_output.join("output"), &new_loc));
        try!(fs::rename(&old_build_output, &build_output));
        let mut f = try!(File::open(&new_loc).map_err(|e| {
            human(format!("failed to read cached build command output: {}", e))
        }));
        let contents = try!(f.read_to_string());
        let output = try!(BuildOutput::parse(contents.as_slice(),
                                             pkg_name.as_slice()));
        build_state.outputs.lock().insert((id, kind), output);

        fresh(tx)
    };

    Ok((dirty, fresh, freshness))
}

impl BuildState {
    pub fn new(config: super::BuildConfig,
               packages: &PackageSet) -> BuildState {
        let mut sources = HashMap::new();
        for package in packages.iter() {
            match package.get_manifest().get_links() {
                Some(links) => {
                    sources.insert(links.to_string(),
                                   package.get_package_id().clone());
                }
                None => {}
            }
        }
        let mut outputs = HashMap::new();
        for (name, output) in config.host.overrides.into_iter() {
            outputs.insert((sources[name].clone(), KindHost), output);
        }
        for (name, output) in config.target.overrides.into_iter() {
            outputs.insert((sources[name].clone(), KindTarget), output);
        }
        BuildState { outputs: Mutex::new(outputs) }
    }
}

impl BuildOutput {
    // Parses the output of a script.
    // The `pkg_name` is used for error messages.
    pub fn parse(input: &str, pkg_name: &str) -> CargoResult<BuildOutput> {
        let mut library_paths = Vec::new();
        let mut library_links = Vec::new();
        let mut metadata = Vec::new();
        let whence = format!("build script of `{}`", pkg_name);

        for line in input.lines() {
            let mut iter = line.splitn(1, |c: char| c == ':');
            if iter.next() != Some("cargo") {
                // skip this line since it doesn't start with "cargo:"
                continue;
            }
            let data = match iter.next() {
                Some(val) => val,
                None => continue
            };

            // getting the `key=value` part of the line
            let mut iter = data.splitn(1, |c: char| c == '=');
            let key = iter.next();
            let value = iter.next();
            let (key, value) = match (key, value) {
                (Some(a), Some(b)) => (a, b.trim_right()),
                // line started with `cargo:` but didn't match `key=value`
                _ => return Err(human(format!("Wrong output in {}: `{}`",
                                              whence, line)))
            };

            if key == "rustc-flags" {
                let whence = whence.as_slice();
                let (libs, links) = try!(
                    BuildOutput::parse_rustc_flags(value, whence)
                );
                library_links.extend(links.into_iter());
                library_paths.extend(libs.into_iter());
            } else {
                metadata.push((key.to_string(), value.to_string()))
            }
        }

        Ok(BuildOutput {
            library_paths: library_paths,
            library_links: library_links,
            metadata: metadata,
        })
    }

    pub fn parse_rustc_flags(value: &str, whence: &str)
                             -> CargoResult<(Vec<Path>, Vec<String>)> {
        // TODO: some arguments (like paths) may contain spaces
        let value = value.trim();
        let mut flags_iter = value.words();
        let (mut library_links, mut library_paths) = (Vec::new(), Vec::new());
        loop {
            let flag = match flags_iter.next() {
                Some(f) => f,
                None => break
            };
            if flag != "-l" && flag != "-L" {
                return Err(human(format!("Only `-l` and `-L` flags are allowed \
                                         in {}: `{}`",
                                         whence, value)))
            }
            let value = match flags_iter.next() {
                Some(v) => v,
                None => return Err(human(format!("Flag in rustc-flags has no value\
                                                  in {}: `{}`",
                                                  whence, value)))
            };
            match flag {
                "-l" => library_links.push(value.to_string()),
                "-L" => library_paths.push(Path::new(value)),

                // was already checked above
                _ => return Err(human("only -l and -L flags are allowed"))
            };
        }
        Ok((library_paths, library_links))
    }
}

impl fmt::Show for BuildOutput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BuildOutput {{ paths: [..], libs: {}, metadata: {} }}",
               self.library_links, self.metadata)
    }
}
