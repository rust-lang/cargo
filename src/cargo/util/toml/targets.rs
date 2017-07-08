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

    if let Some(target) = clean_lib(manifest.lib.as_ref(), package_name, package_root, &layout)? {
        targets.push(target);
        has_lib = true;
    } else {
        has_lib = false;
    }

    targets.extend(
        clean_bins(manifest.bin.as_ref(), package_name, package_root, &layout, has_lib)?
    );

    targets.extend(
        clean_examples(manifest.example.as_ref(), package_root, &layout)?
    );

    targets.extend(
        clean_tests(manifest.test.as_ref(), package_root, &layout)?
    );

    targets.extend(
        clean_benches(manifest.bench.as_ref(), package_root, &layout)?
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
    examples: Vec<PathBuf>,
    tests: Vec<PathBuf>,
    benches: Vec<PathBuf>,
}

impl Layout {
    /// Returns a new `Layout` for a given root path.
    /// The `package_root` represents the directory that contains the `Cargo.toml` file.
    fn from_package_path(package_root: &Path) -> Layout {
        let mut lib = None;
        let mut bins = vec![];
        let mut examples = vec![];
        let mut tests = vec![];
        let mut benches = vec![];

        let lib_candidate = package_root.join("src").join("lib.rs");
        if fs::metadata(&lib_candidate).is_ok() {
            lib = Some(lib_candidate);
        }

        try_add_file(&mut bins, package_root.join("src").join("main.rs"));
        try_add_files(&mut bins, package_root.join("src").join("bin"));
        try_add_mains_from_dirs(&mut bins, package_root.join("src").join("bin"));

        try_add_files(&mut examples, package_root.join("examples"));

        try_add_files(&mut tests, package_root.join("tests"));
        try_add_files(&mut benches, package_root.join("benches"));

        return Layout {
            lib: lib,
            bins: bins,
            examples: examples,
            tests: tests,
            benches: benches,
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

impl TomlTarget {
    fn validate_library_name(&self) -> CargoResult<()> {
        match self.name {
            Some(ref name) => {
                if name.trim().is_empty() {
                    bail!("library target names cannot be empty.")
                }
                if name.contains('-') {
                    bail!("library target names cannot contain hyphens: {}", name)
                }
                Ok(())
            }
            None => Ok(())
        }
    }

    fn validate_binary_name(&self) -> CargoResult<()> {
        match self.name {
            Some(ref name) => {
                if name.trim().is_empty() {
                    bail!("binary target names cannot be empty.")
                }
                if is_bad_artifact_name(name) {
                    bail!("the binary target name `{}` is forbidden", name)
                }
                Ok(())
            }
            None => bail!("binary target bin.name is required")
        }
    }

    fn validate_example_name(&self) -> CargoResult<()> {
        match self.name {
            Some(ref name) => {
                if name.trim().is_empty() {
                    bail!("example target names cannot be empty")
                }
                Ok(())
            }
            None => bail!("example target example.name is required")
        }
    }

    fn validate_test_name(&self) -> CargoResult<()> {
        match self.name {
            Some(ref name) => {
                if name.trim().is_empty() {
                    bail!("test target names cannot be empty")
                }
                Ok(())
            }
            None => bail!("test target test.name is required")
        }
    }

    fn validate_bench_name(&self) -> CargoResult<()> {
        match self.name {
            Some(ref name) => {
                if name.trim().is_empty() {
                    bail!("bench target names cannot be empty")
                }
                Ok(())
            }
            None => bail!("bench target bench.name is required")
        }
    }

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
             package_name: &str,
             package_root: &Path,
             layout: &Layout) -> CargoResult<Option<Target>> {
    let lib = match toml_lib {
        Some(lib) => {
            lib.validate_library_name()?; // XXX: other code paths dodge this validation
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
              package_name: &str,
              package_root: &Path,
              layout: &Layout,
              has_lib: bool) -> CargoResult<Vec<Target>> {
    let bins = match toml_bins {
        Some(bins) => bins.clone(),
        None => inferred_bin_targets(package_name, &layout, package_root)
    };

    for bin in bins.iter() {
        bin.validate_binary_name()?;
    }

    if let Err(e) = unique_names_in_targets(&bins) {
        bail!("found duplicate binary name {}, but all binary targets must have a unique name", e);
    }

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
                  package_root: &Path,
                  layout: &Layout)
                  -> CargoResult<Vec<Target>> {
    let examples = match toml_examples {
        Some(examples) => examples.clone(),
        None => inferred_example_targets(&layout)
    };

    for target in examples.iter() {
        target.validate_example_name()?;
    }

    if let Err(e) = unique_names_in_targets(&examples) {
        bail!("found duplicate example name {}, but all binary targets \
                   must have a unique name", e);
    }

    let mut result = Vec::new();
    for ex in examples.iter() {
        let path = ex.path.clone().unwrap_or_else(|| {
            PathValue(Path::new("examples").join(&format!("{}.rs", ex.name())))
        });

        let crate_types = match ex.crate_types() {
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
        result.push(target);
    }

    Ok(result)
}

fn clean_tests(toml_tests: Option<&Vec<TomlTestTarget>>,
               package_root: &Path,
               layout: &Layout) -> CargoResult<Vec<Target>> {

    let tests = match toml_tests {
        Some(tests) => tests.clone(),
        None => inferred_test_targets(&layout)
    };

    for target in tests.iter() {
        target.validate_test_name()?;
    }
    if let Err(e) = unique_names_in_targets(&tests) {
        bail!("found duplicate test name {}, but all binary targets \
               must have a unique name", e)
    }

    let mut result = Vec::new();
    for test in tests.iter() {
        let path = test.path.clone().unwrap_or_else(|| {
            PathValue(Path::new("tests").join(&format!("{}.rs", test.name())))
        });

        let mut target = Target::test_target(&test.name(), package_root.join(&path.0),
                                             test.required_features.clone());
        configure(test, &mut target);
        result.push(target);
    }
    Ok(result)
}

fn clean_benches(toml_benches: Option<&Vec<TomlBenchTarget>>,
                 package_root: &Path,
                 layout: &Layout) -> CargoResult<Vec<Target>> {
    let benches = match toml_benches {
        Some(benches) => benches.clone(),
        None => inferred_bench_targets(&layout)
    };

    for target in benches.iter() {
        target.validate_bench_name()?;
    }

    if let Err(e) = unique_names_in_targets(&benches) {
        bail!("found duplicate bench name {}, but all binary targets \
               must have a unique name", e);
    }

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
