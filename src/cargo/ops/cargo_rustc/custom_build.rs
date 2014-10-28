use std::io::{fs, BufferedReader, BufReader, USER_RWX};
use std::io::fs::{File, PathExtensions};

use core::{Package, Target};
use util::{CargoResult, CargoError, human};
use util::{internal, ChainError};

use super::job::Work;
use super::{process, KindHost, Context};

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
                let feat = feat.as_slice().chars()
                               .map(|c| c.to_uppercase())
                               .map(|c| if c == '-' {'_'} else {c})
                               .collect::<String>();
                p = p.env(format!("CARGO_FEATURE_{}", feat).as_slice(), Some("1"));
            }
        }
        None => {}
    }

    // building the list of all possible `build/$pkg/output` files
    // whether they exist or not will be checked during the work
    let command_output_files = {
        let layout = cx.layout(pkg, KindHost);
        cx.dep_targets(pkg).iter().map(|&(pkg, _)| {
            layout.build(pkg).join("output")
        }).collect::<Vec<_>>()
    };

    // Building command
    let pkg = pkg.to_string();
    let work = proc(desc_tx: Sender<String>) {
        desc_tx.send_opt(build_output.display().to_string()).ok();

        if !build_output.exists() {
            try!(fs::mkdir(&build_output, USER_RWX)
                .chain_error(|| {
                    internal("failed to create build output directory for build command")
                }))
        }

        // loading each possible custom build output file in order to get their metadata
        let _metadata = {
            let mut metadata = Vec::new();

            for flags_file in command_output_files.into_iter() {
                match File::open(&flags_file) {
                    Ok(flags) => {
                        let flags = try!(CustomBuildCommandOutput::parse(
                            BufferedReader::new(flags), pkg.as_slice()));
                        metadata.extend(flags.metadata.into_iter());
                    },
                    Err(_) => ()  // the file doesn't exist, probably means that this pkg
                                  // doesn't have a build command
                }
            }

            metadata
        };

        // TODO: ENABLE THIS CODE WHEN `links` IS ADDED
        /*let mut p = p;
        for (key, value) in metadata.into_iter() {
            p = p.env(format!("DEP_{}_{}", PUT LINKS VALUES HERE, value), value);
        }*/

        let output = try!(p.exec_with_output().map_err(|mut e| {
            e.msg = format!("Failed to run custom build command for `{}`\n{}",
                            pkg, e.msg);
            e.mark_human()
        }));

        // parsing the output of the custom build script to check that it's correct
        try!(CustomBuildCommandOutput::parse(BufReader::new(output.output.as_slice()),
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

/// Contains the parsed output of a custom build script.
pub struct CustomBuildCommandOutput {
    /// Paths to pass to rustc with the `-L` flag
    pub library_paths: Vec<Path>,
    /// Names and link kinds of libraries, suitable for the `-l` flag
    pub library_links: Vec<String>,
    /// Metadata to pass to the immediate dependencies
    pub metadata: Vec<(String, String)>,
}

impl CustomBuildCommandOutput {
    // Parses the output of a script.
    // The `pkg_name` is used for error messages.
    pub fn parse<B: Buffer>(mut input: B, pkg_name: &str) -> CargoResult<CustomBuildCommandOutput> {
        let mut library_paths = Vec::new();
        let mut library_links = Vec::new();
        let mut metadata = Vec::new();

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
                _ => return Err(human(format!("Wrong output for the custom\
                                               build script of `{}`:\n`{}`", pkg_name, line)))
            };

            if key == "rustc-flags" {
                // TODO: some arguments (like paths) may contain spaces
                let mut flags_iter = value.words();
                loop {
                    let flag = match flags_iter.next() {
                        Some(f) => f,
                        None => break
                    };
                    if flag != "-l" && flag != "-L" {
                        return Err(human(format!("Only `-l` and `-L` flags are allowed \
                                                 in build script of `{}`:\n`{}`",
                                                 pkg_name, value)))
                    }
                    let value = match flags_iter.next() {
                        Some(v) => v,
                        None => return Err(human(format!("Flag in rustc-flags has no value\
                                                          in build script of `{}`:\n`{}`",
                                                          pkg_name, value)))
                    };
                    match flag {
                        "-l" => library_links.push(value.to_string()),
                        "-L" => library_paths.push(Path::new(value)),

                        // was already checked above
                        _ => return Err(human("only -l and -L flags are allowed"))
                    };
                }
            } else {
                metadata.push((key.to_string(), value.to_string()))
            }
        }

        Ok(CustomBuildCommandOutput {
            library_paths: library_paths,
            library_links: library_links,
            metadata: metadata,
        })
    }
}
