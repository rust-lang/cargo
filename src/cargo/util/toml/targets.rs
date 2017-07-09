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


pub fn targets(manifest: &TomlManifest,
               package_name: &str,
               package_root: &Path,
               custom_build: &Option<StringOrBool>)
               -> CargoResult<Vec<Target>> {
    let layout = Layout::from_package_path(package_root);

    let mut targets = Vec::new();

    let has_lib;

    if let Some(target) = clean_lib(manifest.lib.as_ref(), package_root, &layout, package_name)? {
        targets.push(target);
        has_lib = true;
    } else {
        has_lib = false;
    }

    targets.extend(
        clean_bins(manifest.bin.as_ref(), package_root, &layout, package_name, has_lib)?
    );

    targets.extend(
        clean_examples(manifest.example.as_ref(), package_root)?
    );

    targets.extend(
        clean_tests(manifest.test.as_ref(), package_root)?
    );

    targets.extend(
        clean_benches(manifest.bench.as_ref(), package_root)?
    );

    // processing the custom build script
    if let Some(custom_build) = manifest.maybe_custom_build(custom_build, package_root) {
        let name = format!("build-script-{}",
                           custom_build.file_stem().and_then(|s| s.to_str()).unwrap_or(""));
        targets.push(Target::custom_build_target(&name, package_root.join(custom_build)));
    }

    Ok(targets)
}


/// Implicit Cargo targets, defined by conventions.
struct Layout {
    lib: Option<PathBuf>,
    bins: Vec<PathBuf>,
}

impl Layout {
    /// Returns a new `Layout` for a given root path.
    /// The `package_root` represents the directory that contains the `Cargo.toml` file.
    fn from_package_path(package_root: &Path) -> Layout {
        let mut lib = None;
        let mut bins = vec![];

        let lib_candidate = package_root.join("src").join("lib.rs");
        if fs::metadata(&lib_candidate).is_ok() {
            lib = Some(lib_candidate);
        }

        try_add_file(&mut bins, package_root.join("src").join("main.rs"));
        try_add_files(&mut bins, package_root.join("src").join("bin"));
        try_add_mains_from_dirs(&mut bins, package_root.join("src").join("bin"));

        return Layout {
            lib: lib,
            bins: bins,
        };

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
    }
}

fn infer_from_directory(directory: &Path) -> Vec<(String, PathBuf)> {
    let entries = match fs::read_dir(directory) {
        Err(_) => return Vec::new(),
        Ok(dir) => dir
    };

    entries.filter_map(|entry| entry.map(|d| d.path()).ok())
        .filter(|f| f.extension().and_then(|s| s.to_str()) == Some("rs"))
        .filter_map(|f| {
            if f.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with('.')) != Some(false) {
                return None;
            };
            f.file_stem().and_then(|s| s.to_str())
                .map(|s| (s.to_owned(), f.clone()))
        })
        .collect()
}


fn inferred_tests(package_root: &Path) -> Vec<(String, PathBuf)> {
    infer_from_directory(&package_root.join("tests"))
}

fn inferred_benches(package_root: &Path) -> Vec<(String, PathBuf)> {
    infer_from_directory(&package_root.join("benches"))
}

fn inferred_examples(package_root: &Path) -> Vec<(String, PathBuf)> {
    infer_from_directory(&package_root.join("examples"))
}

impl TomlTarget {
    fn validate_crate_type(&self) -> CargoResult<()> {
        // Per the Macros 1.1 RFC:
        //
        // > Initially if a crate is compiled with the proc-macro crate type
        // > (and possibly others) it will forbid exporting any items in the
        // > crate other than those functions tagged #[proc_macro_derive] and
        // > those functions must also be placed at the crate root.
        //
        // A plugin requires exporting plugin_registrar so a crate cannot be
        // both at once.
        if self.plugin == Some(true) && self.proc_macro() == Some(true) {
            Err("lib.plugin and lib.proc-macro cannot both be true".into())
        } else {
            Ok(())
        }
    }
}

fn clean_lib(toml_lib: Option<&TomlLibTarget>,
             package_root: &Path,
             layout: &Layout,
             package_name: &str) -> CargoResult<Option<Target>> {
    let lib = match toml_lib {
        Some(lib) => {
            if let Some(ref name) = lib.name {
                // XXX: other code paths dodge this validation
                if name.contains('-') {
                    bail!("library target names cannot contain hyphens: {}", name)
                }
            }

            lib.validate_crate_type()?;
            Some(
                TomlTarget {
                    name: lib.name.clone().or(Some(package_name.to_owned())),
                    path: lib.path.clone().or_else(
                        || layout.lib.as_ref().map(|p| PathValue(p.clone()))
                    ),
                    ..lib.clone()
                }
            )
        }
        None => layout.lib.as_ref().map(|lib| {
            TomlTarget {
                name: Some(package_name.to_string()),
                path: Some(PathValue(lib.clone())),
                ..TomlTarget::new()
            }
        })
    };

    let lib = match lib {
        Some(ref lib) => lib,
        None => return Ok(None)
    };

    validate_has_name(&lib, "library", "lib")?;

    let path = lib.path.clone().unwrap_or_else(
        || PathValue(Path::new("src").join(&format!("{}.rs", lib.name())))
    );

    let crate_types = match lib.crate_types() {
        Some(kinds) => kinds.iter().map(|s| LibKind::from_str(s)).collect(),
        None => {
            let lib_kind = match (lib.plugin, lib.proc_macro()) {
                (Some(true), _) => LibKind::Dylib,
                (_, Some(true)) => LibKind::ProcMacro,
                _ => LibKind::Lib
            };
            vec![lib_kind]
        }
    };

    let mut target = Target::lib_target(&lib.name(), crate_types, package_root.join(&path.0));
    configure(lib, &mut target);
    Ok(Some(target))
}

fn clean_bins(toml_bins: Option<&Vec<TomlBinTarget>>,
              package_root: &Path,
              layout: &Layout,
              package_name: &str,
              has_lib: bool) -> CargoResult<Vec<Target>> {
    let bins = match toml_bins {
        Some(bins) => bins.clone(),
        None => inferred_bin_targets(package_name, &layout, package_root)
    };

    for bin in bins.iter() {
        validate_has_name(bin, "binary", "bin")?;

        let name = bin.name();
        if is_bad_artifact_name(&name) {
            bail!("the binary target name `{}` is forbidden", name)
        }
    }

    validate_unique_names(&bins, "binary")?;

    let mut result = Vec::new();
    for bin in bins.iter() {
        let path = bin.path.clone().unwrap_or_else(|| {
            PathValue(inferred_bin_path(bin, has_lib, package_root, bins.len()))
        });
        let mut target = Target::bin_target(&bin.name(), package_root.join(&path.0),
                                            bin.required_features.clone());
        configure(bin, &mut target);
        result.push(target);
    }
    Ok(result)
}

fn clean_examples(toml_examples: Option<&Vec<TomlExampleTarget>>,
                  package_root: &Path)
                  -> CargoResult<Vec<Target>> {
    let inferred = inferred_examples(package_root);
    let examples = match toml_examples {
        Some(examples) => examples.clone(),
        None => inferred.iter().map(|&(ref name, ref path)| {
            TomlTarget {
                name: Some(name.clone()),
                path: Some(PathValue(path.clone())),
                ..TomlTarget::new()
            }
        }).collect()
    };

    for target in examples.iter() {
        validate_has_name(target, "example", "example")?;
    }

    validate_unique_names(&examples, "example")?;

    let mut result = Vec::new();
    for ex in examples.iter() {
        let path = target_path(ex, &inferred, "example", package_root)?;

        let crate_types = match ex.crate_types() {
            Some(kinds) => kinds.iter().map(|s| LibKind::from_str(s)).collect(),
            None => Vec::new()
        };

        let mut target = Target::example_target(
            &ex.name(),
            crate_types,
            path,
            ex.required_features.clone()
        );
        configure(ex, &mut target);
        result.push(target);
    }

    Ok(result)
}

fn clean_tests(toml_tests: Option<&Vec<TomlTestTarget>>,
               package_root: &Path) -> CargoResult<Vec<Target>> {
    let inferred = inferred_tests(package_root);
    let tests = match toml_tests {
        Some(tests) => tests.clone(),
        None => inferred.iter().map(|&(ref name, ref path)| {
            TomlTarget {
                name: Some(name.clone()),
                path: Some(PathValue(path.clone())),
                ..TomlTarget::new()
            }
        }).collect()
    };

    for target in tests.iter() {
        validate_has_name(target, "test", "test")?;
    }

    validate_unique_names(&tests, "test")?;

    let mut result = Vec::new();
    for test in tests.iter() {
        let path = target_path(test, &inferred, "test", package_root)?;

        let mut target = Target::test_target(&test.name(), path,
                                             test.required_features.clone());
        configure(test, &mut target);
        result.push(target);
    }
    Ok(result)
}

fn clean_benches(toml_benches: Option<&Vec<TomlBenchTarget>>,
                 package_root: &Path) -> CargoResult<Vec<Target>> {
    let benches = match toml_benches {
        Some(benches) => benches.clone(),
        None => inferred_benches(package_root).into_iter().map(|(name, path)| {
            TomlTarget {
                name: Some(name),
                path: Some(PathValue(path)),
                ..TomlTarget::new()
            }
        }).collect()
    };

    for target in benches.iter() {
        validate_has_name(target, "benchmark", "bench")?;
    }

    validate_unique_names(&benches, "bench")?;

    let mut result = Vec::new();
    for bench in benches.iter() {
        let path = bench.path.clone().unwrap_or_else(|| {
            PathValue(Path::new("benches").join(&format!("{}.rs", bench.name())))
        });

        let mut target = Target::bench_target(&bench.name(), package_root.join(&path.0),
                                              bench.required_features.clone());
        configure(bench, &mut target);
        result.push(target);
    }
    Ok(result)
}

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
fn inferred_bin_targets(name: &str, layout: &Layout, project_root: &Path) -> Vec<TomlTarget> {
    layout.bins.iter().filter_map(|bin| {
        let name = if &**bin == Path::new("src/main.rs") ||
            *bin == project_root.join("src").join("main.rs") {
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

fn target_path(target: &TomlTarget,
               inferred: &[(String, PathBuf)],
               target_kind: &str,
               package_root: &Path) -> CargoResult<PathBuf> {
    if let Some(ref path) = target.path {
        // Should we verify that this path exists here?
        return Ok(package_root.join(&path.0));
    }
    let name = target.name();

    let mut matching = inferred.iter()
        .filter(|&&(ref n, _)| n == &name)
        .map(|&(_, ref p)| p.clone());

    let first = matching.next();
    let second = matching.next();
    match (first, second) {
        (Some(path), None) => Ok(path),
        (None, None) | (Some(_), Some(_)) => {
            bail!("can't find `{name}` {target_kind}, specify {target_kind}.path",
                   name = name, target_kind = target_kind)
        }
        (None, Some(_)) => unreachable!()
    }
}

fn validate_has_name(target: &TomlTarget, target_name: &str, target_kind: &str) -> CargoResult<()> {
    match target.name {
        Some(ref name) => if name.trim().is_empty() {
            bail!("{} target names cannot be empty", target_name)
        },
        None => bail!("{} target {}.name is required", target_name, target_kind)
    }

    Ok(())
}

/// Will check a list of toml targets, and make sure the target names are unique within a vector.
fn validate_unique_names(targets: &[TomlTarget], target_kind: &str) -> CargoResult<()> {
    let mut seen = HashSet::new();
    for name in targets.iter().map(|e| e.name()) {
        if !seen.insert(name.clone()) {
            bail!("found duplicate {target_kind} name {name}, \
                   but all {target_kind} targets must have a unique name",
                   target_kind = target_kind, name = name);
        }
    }
    Ok(())
}
