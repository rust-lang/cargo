//! This module implements Cargo conventions for directory layout:
//!
//!  * `src/lib.rs` is a library
//!  * `src/main.rs` is a binary
//!  * `src/bin/*.rs` are binaries
//!  * `examples/*.rs` are examples
//!  * `tests/*.rs` are integration tests
//!
//! It is a bit tricky because we need match explicit information from `Cargo.toml`
//! with implicit info in directory layout

use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashSet;

use core::Target;
use ops::is_bad_artifact_name;
use util::errors::CargoResult;
use super::{TomlTarget, LibKind, PathValue, TomlManifest, StringOrBool,
            TomlLibTarget, TomlBinTarget, TomlBenchTarget, TomlExampleTarget, TomlTestTarget};


/// Implicit Cargo targets, defined by conventions.
struct Layout {
    root: PathBuf,
    lib: Option<PathBuf>,
    bins: Vec<PathBuf>,
    examples: Vec<PathBuf>,
    tests: Vec<PathBuf>,
    benches: Vec<PathBuf>,
}

impl Layout {
    /// Returns a new `Layout` for a given root path.
    /// The `root_path` represents the directory that contains the `Cargo.toml` file.
    pub fn from_project_path(root_path: &Path) -> Layout {
        let mut lib = None;
        let mut bins = vec![];
        let mut examples = vec![];
        let mut tests = vec![];
        let mut benches = vec![];

        let lib_candidate = root_path.join("src").join("lib.rs");
        if fs::metadata(&lib_candidate).is_ok() {
            lib = Some(lib_candidate);
        }

        try_add_file(&mut bins, root_path.join("src").join("main.rs"));
        try_add_files(&mut bins, root_path.join("src").join("bin"));
        try_add_mains_from_dirs(&mut bins, root_path.join("src").join("bin"));

        try_add_files(&mut examples, root_path.join("examples"));

        try_add_files(&mut tests, root_path.join("tests"));
        try_add_files(&mut benches, root_path.join("benches"));

        Layout {
            root: root_path.to_path_buf(),
            lib: lib,
            bins: bins,
            examples: examples,
            tests: tests,
            benches: benches,
        }
    }
}

fn try_add_file(files: &mut Vec<PathBuf>, file: PathBuf) {
    if fs::metadata(&file).is_ok() {
        files.push(file);
    }
}

// Add directories form src/bin which contain main.rs file
fn try_add_mains_from_dirs(files: &mut Vec<PathBuf>, root: PathBuf) {
    if let Ok(new) = fs::read_dir(&root) {
        let new: Vec<PathBuf> = new.filter_map(|i| i.ok())
            // Filter only directories
            .filter(|i| {
                i.file_type().map(|f| f.is_dir()).unwrap_or(false)
                // Convert DirEntry into PathBuf and append "main.rs"
            }).map(|i| {
            i.path().join("main.rs")
            // Filter only directories where main.rs is present
        }).filter(|f| {
            f.as_path().exists()
        }).collect();
        files.extend(new);
    }
}

fn try_add_files(files: &mut Vec<PathBuf>, root: PathBuf) {
    if let Ok(new) = fs::read_dir(&root) {
        files.extend(new.filter_map(|dir| {
            dir.map(|d| d.path()).ok()
        }).filter(|f| {
            f.extension().and_then(|s| s.to_str()) == Some("rs")
        }).filter(|f| {
            // Some unix editors may create "dotfiles" next to original
            // source files while they're being edited, but these files are
            // rarely actually valid Rust source files and sometimes aren't
            // even valid UTF-8. Here we just ignore all of them and require
            // that they are explicitly specified in Cargo.toml if desired.
            f.file_name().and_then(|s| s.to_str()).map(|s| {
                !s.starts_with('.')
            }).unwrap_or(true)
        }))
    }
    /* else just don't add anything if the directory doesn't exist, etc. */
}

pub fn targets(me: &TomlManifest, project_root: &Path, project_name: &str, custom_build: &Option<StringOrBool>) -> CargoResult<Vec<Target>> {
    let layout = Layout::from_project_path(project_root);
    let lib = match me.lib {
        Some(ref lib) => {
            lib.validate_library_name()?;
            lib.validate_crate_type()?;
            Some(
                TomlTarget {
                    name: lib.name.clone().or(Some(project_name.to_owned())),
                    path: lib.path.clone().or_else(
                        || layout.lib.as_ref().map(|p| PathValue(p.clone()))
                    ),
                    ..lib.clone()
                }
            )
        }
        None => inferred_lib_target(project_name, &layout),
    };

    let bins = match me.bin {
        Some(ref bins) => {
            for target in bins {
                target.validate_binary_name()?;
            };
            bins.clone()
        }
        None => inferred_bin_targets(project_name, &layout)
    };

    for bin in bins.iter() {
        if is_bad_artifact_name(&bin.name()) {
            bail!("the binary target name `{}` is forbidden",
                      bin.name())
        }
    }

    let examples = match me.example {
        Some(ref examples) => {
            for target in examples {
                target.validate_example_name()?;
            }
            examples.clone()
        }
        None => inferred_example_targets(&layout)
    };

    let tests = match me.test {
        Some(ref tests) => {
            for target in tests {
                target.validate_test_name()?;
            }
            tests.clone()
        }
        None => inferred_test_targets(&layout)
    };

    let benches = match me.bench {
        Some(ref benches) => {
            for target in benches {
                target.validate_bench_name()?;
            }
            benches.clone()
        }
        None => inferred_bench_targets(&layout)
    };

    if let Err(e) = unique_names_in_targets(&bins) {
        bail!("found duplicate binary name {}, but all binary targets \
                   must have a unique name", e);
    }

    if let Err(e) = unique_names_in_targets(&examples) {
        bail!("found duplicate example name {}, but all binary targets \
                   must have a unique name", e);
    }

    if let Err(e) = unique_names_in_targets(&benches) {
        bail!("found duplicate bench name {}, but all binary targets must \
                   have a unique name", e);
    }

    if let Err(e) = unique_names_in_targets(&tests) {
        bail!("found duplicate test name {}, but all binary targets must \
                   have a unique name", e)
    }

    // processing the custom build script
    let new_build = me.maybe_custom_build(custom_build, &layout.root);

    // Get targets
    let targets = normalize(&layout.root,
                            &lib,
                            &bins,
                            new_build,
                            &examples,
                            &tests,
                            &benches);
    Ok(targets)
}

fn normalize(package_root: &Path,
             lib: &Option<TomlLibTarget>,
             bins: &[TomlBinTarget],
             custom_build: Option<PathBuf>,
             examples: &[TomlExampleTarget],
             tests: &[TomlTestTarget],
             benches: &[TomlBenchTarget]) -> Vec<Target> {
    fn configure(toml: &TomlTarget, target: &mut Target) {
        let t2 = target.clone();
        target.set_tested(toml.test.unwrap_or(t2.tested()))
            .set_doc(toml.doc.unwrap_or(t2.documented()))
            .set_doctest(toml.doctest.unwrap_or(t2.doctested()))
            .set_benched(toml.bench.unwrap_or(t2.benched()))
            .set_harness(toml.harness.unwrap_or(t2.harness()))
            .set_for_host(match (toml.plugin, toml.proc_macro()) {
                (None, None) => t2.for_host(),
                (Some(true), _) | (_, Some(true)) => true,
                (Some(false), _) | (_, Some(false)) => false,
            });
    }

    let lib_target = |dst: &mut Vec<Target>, l: &TomlLibTarget| {
        let path = l.path.clone().unwrap_or_else(
            || PathValue(Path::new("src").join(&format!("{}.rs", l.name())))
        );
        let crate_types = l.crate_type.as_ref().or(l.crate_type2.as_ref());
        let crate_types = match crate_types {
            Some(kinds) => kinds.iter().map(|s| LibKind::from_str(s)).collect(),
            None => {
                vec![if l.plugin == Some(true) { LibKind::Dylib } else if l.proc_macro() == Some(true) { LibKind::ProcMacro } else { LibKind::Lib }]
            }
        };

        let mut target = Target::lib_target(&l.name(), crate_types,
                                            package_root.join(&path.0));
        configure(l, &mut target);
        dst.push(target);
    };

    let bin_targets = |dst: &mut Vec<Target>, bins: &[TomlBinTarget],
                       default: &mut FnMut(&TomlBinTarget) -> PathBuf| {
        for bin in bins.iter() {
            let path = bin.path.clone().unwrap_or_else(|| {
                PathValue(default(bin))
            });
            let mut target = Target::bin_target(&bin.name(), package_root.join(&path.0),
                                                bin.required_features.clone());
            configure(bin, &mut target);
            dst.push(target);
        }
    };

    let custom_build_target = |dst: &mut Vec<Target>, cmd: &Path| {
        let name = format!("build-script-{}",
                           cmd.file_stem().and_then(|s| s.to_str()).unwrap_or(""));

        dst.push(Target::custom_build_target(&name, package_root.join(cmd)));
    };

    let example_targets = |dst: &mut Vec<Target>,
                           examples: &[TomlExampleTarget],
                           default: &mut FnMut(&TomlExampleTarget) -> PathBuf| {
        for ex in examples.iter() {
            let path = ex.path.clone().unwrap_or_else(|| {
                PathValue(default(ex))
            });

            let crate_types = ex.crate_type.as_ref().or(ex.crate_type2.as_ref());
            let crate_types = match crate_types {
                Some(kinds) => kinds.iter().map(|s| LibKind::from_str(s)).collect(),
                None => Vec::new()
            };

            let mut target = Target::example_target(
                &ex.name(),
                crate_types,
                package_root.join(&path.0),
                ex.required_features.clone()
            );
            configure(ex, &mut target);
            dst.push(target);
        }
    };

    let test_targets = |dst: &mut Vec<Target>,
                        tests: &[TomlTestTarget],
                        default: &mut FnMut(&TomlTestTarget) -> PathBuf| {
        for test in tests.iter() {
            let path = test.path.clone().unwrap_or_else(|| {
                PathValue(default(test))
            });

            let mut target = Target::test_target(&test.name(), package_root.join(&path.0),
                                                 test.required_features.clone());
            configure(test, &mut target);
            dst.push(target);
        }
    };

    let bench_targets = |dst: &mut Vec<Target>,
                         benches: &[TomlBenchTarget],
                         default: &mut FnMut(&TomlBenchTarget) -> PathBuf| {
        for bench in benches.iter() {
            let path = bench.path.clone().unwrap_or_else(|| {
                PathValue(default(bench))
            });

            let mut target = Target::bench_target(&bench.name(), package_root.join(&path.0),
                                                  bench.required_features.clone());
            configure(bench, &mut target);
            dst.push(target);
        }
    };

    let mut ret = Vec::new();

    if let Some(ref lib) = *lib {
        lib_target(&mut ret, lib);
    }
    bin_targets(&mut ret, bins,
                &mut |bin| inferred_bin_path(bin, lib.is_some(), package_root, bins.len()));


    if let Some(custom_build) = custom_build {
        custom_build_target(&mut ret, &custom_build);
    }

    example_targets(&mut ret, examples,
                    &mut |ex| Path::new("examples")
                        .join(&format!("{}.rs", ex.name())));

    test_targets(&mut ret, tests, &mut |test| {
        Path::new("tests").join(&format!("{}.rs", test.name()))
    });

    bench_targets(&mut ret, benches, &mut |bench| {
        Path::new("benches").join(&format!("{}.rs", bench.name()))
    });

    ret
}


fn inferred_bin_path(bin: &TomlBinTarget,
                     has_lib: bool,
                     package_root: &Path,
                     bin_len: usize) -> PathBuf {
    // here we have a single bin, so it may be located in src/main.rs, src/foo.rs,
    // src/bin/foo.rs, src/bin/foo/main.rs or src/bin/main.rs
    if bin_len == 1 {
        let path = Path::new("src").join("main.rs");
        if package_root.join(&path).exists() {
            return path.to_path_buf();
        }

        if !has_lib {
            let path = Path::new("src").join(&format!("{}.rs", bin.name()));
            if package_root.join(&path).exists() {
                return path.to_path_buf();
            }
        }

        let path = Path::new("src").join("bin").join(&format!("{}.rs", bin.name()));
        if package_root.join(&path).exists() {
            return path.to_path_buf();
        }

        // check for the case where src/bin/foo/main.rs is present
        let path = Path::new("src").join("bin").join(bin.name()).join("main.rs");
        if package_root.join(&path).exists() {
            return path.to_path_buf();
        }

        return Path::new("src").join("bin").join("main.rs").to_path_buf();
    }

    // bin_len > 1
    let path = Path::new("src").join("bin").join(&format!("{}.rs", bin.name()));
    if package_root.join(&path).exists() {
        return path.to_path_buf();
    }

    // we can also have src/bin/foo/main.rs, but the former one is preferred
    let path = Path::new("src").join("bin").join(bin.name()).join("main.rs");
    if package_root.join(&path).exists() {
        return path.to_path_buf();
    }

    if !has_lib {
        let path = Path::new("src").join(&format!("{}.rs", bin.name()));
        if package_root.join(&path).exists() {
            return path.to_path_buf();
        }
    }

    let path = Path::new("src").join("bin").join("main.rs");
    if package_root.join(&path).exists() {
        return path.to_path_buf();
    }

    return Path::new("src").join("main.rs").to_path_buf();
}

// These functions produce the equivalent of specific manifest entries. One
// wrinkle is that certain paths cannot be represented in the manifest due
// to Toml's UTF-8 requirement. This could, in theory, mean that certain
// otherwise acceptable executable names are not used when inside of
// `src/bin/*`, but it seems ok to not build executables with non-UTF8
// paths.
fn inferred_lib_target(name: &str, layout: &Layout) -> Option<TomlTarget> {
    layout.lib.as_ref().map(|lib| {
        TomlTarget {
            name: Some(name.to_string()),
            path: Some(PathValue(lib.clone())),
            ..TomlTarget::new()
        }
    })
}

fn inferred_bin_targets(name: &str, layout: &Layout) -> Vec<TomlTarget> {
    layout.bins.iter().filter_map(|bin| {
        let name = if &**bin == Path::new("src/main.rs") ||
            *bin == layout.root.join("src").join("main.rs") {
            Some(name.to_string())
        } else {
            // bin is either a source file or a directory with main.rs inside.
            if bin.ends_with("main.rs") && !bin.ends_with("src/bin/main.rs") {
                if let Some(parent) = bin.parent() {
                    // Use a name of this directory as a name for binary
                    parent.file_stem().and_then(|s| s.to_str()).map(|f| f.to_string())
                } else {
                    None
                }
            } else {
                // regular case, just a file in the bin directory
                bin.file_stem().and_then(|s| s.to_str()).map(|f| f.to_string())
            }
        };

        name.map(|name| {
            TomlTarget {
                name: Some(name),
                path: Some(PathValue(bin.clone())),
                ..TomlTarget::new()
            }
        })
    }).collect()
}

fn inferred_example_targets(layout: &Layout) -> Vec<TomlTarget> {
    layout.examples.iter().filter_map(|ex| {
        ex.file_stem().and_then(|s| s.to_str()).map(|name| {
            TomlTarget {
                name: Some(name.to_string()),
                path: Some(PathValue(ex.clone())),
                ..TomlTarget::new()
            }
        })
    }).collect()
}

fn inferred_test_targets(layout: &Layout) -> Vec<TomlTarget> {
    layout.tests.iter().filter_map(|ex| {
        ex.file_stem().and_then(|s| s.to_str()).map(|name| {
            TomlTarget {
                name: Some(name.to_string()),
                path: Some(PathValue(ex.clone())),
                ..TomlTarget::new()
            }
        })
    }).collect()
}

fn inferred_bench_targets(layout: &Layout) -> Vec<TomlTarget> {
    layout.benches.iter().filter_map(|ex| {
        ex.file_stem().and_then(|s| s.to_str()).map(|name| {
            TomlTarget {
                name: Some(name.to_string()),
                path: Some(PathValue(ex.clone())),
                ..TomlTarget::new()
            }
        })
    }).collect()
}

/// Will check a list of toml targets, and make sure the target names are unique within a vector.
/// If not, the name of the offending binary target is returned.
fn unique_names_in_targets(targets: &[TomlTarget]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for v in targets.iter().map(|e| e.name()) {
        if !seen.insert(v.clone()) {
            return Err(v);
        }
    }
    Ok(())
}
