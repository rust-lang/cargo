use std::io::{fs, BufReader, USER_RWX};
use std::io::fs::PathExtensions;

use core::{Package, Target};
use util::{CargoResult, CargoError, human};
use util::{internal, ChainError};

use super::job::Work;
use super::{process, KindHost, Context};

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

/// Prepares a `Work` that executes the target as a custom build script.
pub fn prepare_execute_custom_build(pkg: &Package, target: &Target,
                                    cx: &mut Context)
                                    -> CargoResult<Work> {
    let layout = cx.layout(pkg, KindHost);
    let script_output = layout.build(pkg);
    let build_output = layout.build_out(pkg);

    // Building the command to execute
    let to_exec = try!(cx.target_filenames(target))[0].clone();
    let to_exec = script_output.join(to_exec);

    // Filling environment variables
    let profile = target.get_profile();
    let mut p = process(to_exec, pkg, cx)
                     .env("OUT_DIR", Some(&build_output))
                     .env("CARGO_MANIFEST_DIR", Some(pkg.get_manifest_path()
                                                     .display().to_string()))
                     .env("NUM_JOBS", profile.get_codegen_units().map(|n| n.to_string()))
                     .env("TARGET", Some(cx.target_triple()))
                     .env("DEBUG", Some(profile.get_debug().to_string()))
                     .env("OPT_LEVEL", Some(profile.get_opt_level().to_string()))
                     .env("PROFILE", Some(profile.get_env()));

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

    // Gather the set of native dependencies that this package has
    let lib_deps = {
        cx.dep_targets(pkg).iter().filter_map(|&(pkg, _)| {
            pkg.get_manifest().get_links()
        }).map(|s| s.to_string()).collect::<Vec<_>>()
    };

    let native_libs = cx.native_libs.clone();

    // Building command
    let pkg = pkg.to_string();
    let work = proc(desc_tx: Sender<String>) {

        if !build_output.exists() {
            try!(fs::mkdir(&build_output, USER_RWX).chain_error(|| {
                internal("failed to create build output directory for \
                          build command")
            }))
        }

        // loading each possible custom build output file in order to get their
        // metadata
        let mut p = p;
        {
            let native_libs = native_libs.lock();
            for dep in lib_deps.iter() {
                for &(ref key, ref value) in (*native_libs)[*dep].metadata.iter() {
                    p = p.env(format!("DEP_{}_{}",
                                      super::envify(dep.as_slice()),
                                      super::envify(key.as_slice())).as_slice(),
                              Some(value.as_slice()));
                }
            }
        }

        desc_tx.send_opt(p.to_string()).ok();
        let output = try!(p.exec_with_output().map_err(|mut e| {
            e.msg = format!("Failed to run custom build command for `{}`\n{}",
                            pkg, e.msg);
            e.concrete().mark_human()
        }));

        // parsing the output of the custom build script to check that it's correct
        try!(BuildOutput::parse(BufReader::new(output.output.as_slice()),
                                             pkg.as_slice()));

        // writing the output to the right directory
        try!(fs::File::create(&script_output.join("output")).write(output.output.as_slice())
            .map_err(|e| {
                human(format!("failed to write output of custom build command: {}", e))
            }));

        Ok(())
    };

    Ok(work)
}

impl BuildOutput {
    // Parses the output of a script.
    // The `pkg_name` is used for error messages.
    pub fn parse<B: Buffer>(mut input: B, pkg_name: &str) -> CargoResult<BuildOutput> {
        let mut library_paths = Vec::new();
        let mut library_links = Vec::new();
        let mut metadata = Vec::new();
        let whence = format!("build script of `{}`", pkg_name);

        for line in input.lines() {
            // unwrapping the IoResult
            let line = try!(line.map_err(|e| human(format!("Error while reading\
                                                            custom build output: {}", e))));

            let mut iter = line.as_slice().splitn(1, |c: char| c == ':');
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
                (Some(a), Some(b)) => (a, b),
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
