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
use std::fs::{self, DirEntry};
use std::collections::HashSet;

use core::Target;
use ops::is_bad_artifact_name;
use util::errors::CargoResult;
use util::paths::without_prefix;
use super::{TomlTarget, LibKind, PathValue, TomlManifest, StringOrBool,
            TomlLibTarget, TomlBinTarget, TomlBenchTarget, TomlExampleTarget, TomlTestTarget};


pub fn targets(manifest: &TomlManifest,
               package_name: &str,
               package_root: &Path,
               custom_build: &Option<StringOrBool>,
               warnings: &mut Vec<String>)
               -> CargoResult<Vec<Target>> {
    let mut targets = Vec::new();

    let has_lib;

    if let Some(target) = clean_lib(manifest.lib.as_ref(), package_root, package_name, warnings)? {
        targets.push(target);
        has_lib = true;
    } else {
        has_lib = false;
    }

    targets.extend(
        clean_bins(manifest.bin.as_ref(), package_root, package_name, warnings, has_lib)?
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

fn infer_from_directory(directory: &Path) -> Vec<(String, PathBuf)> {
    let entries = match fs::read_dir(directory) {
        Err(_) => return Vec::new(),
        Ok(dir) => dir
    };

    entries
        .filter_map(|e| e.ok())
        .filter(is_not_dotfile)
        .map(|e| e.path())
        .filter(|f| f.extension().and_then(|s| s.to_str()) == Some("rs"))
        .filter_map(|f| {
            f.file_stem().and_then(|s| s.to_str())
                .map(|s| (s.to_owned(), f.clone()))
        })
        .collect()
}

fn inferred_lib(package_root: &Path) -> Option<PathBuf> {
    let lib = package_root.join("src").join("lib.rs");
    if fs::metadata(&lib).is_ok() {
        Some(lib)
    } else {
        None
    }
}

fn inferred_bins(package_root: &Path, package_name: &str) -> Vec<(String, PathBuf)> {
    let main = package_root.join("src").join("main.rs");
    let mut result = Vec::new();
    if main.exists() {
        result.push((package_name.to_string(), main));
    }
    result.extend(infer_from_directory(&package_root.join("src").join("bin")));

    if let Ok(entries) = fs::read_dir(&package_root.join("src").join("bin")) {
        let multifile_bins = entries
            .filter_map(|e| e.ok())
            .filter(is_not_dotfile)
            .filter(|e| match e.file_type() {
                Ok(t) if t.is_dir() => true,
                _ => false
            })
            .filter_map(|entry| {
                let dir = entry.path();
                let main = dir.join("main.rs");
                let name = dir.file_name().and_then(|n| n.to_str());
                match (main.exists(), name) {
                    (true, Some(name)) => Some((name.to_owned(), main)),
                    _ => None
                }
            });
        result.extend(multifile_bins);
    }

    result
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
             package_name: &str,
             warnings: &mut Vec<String>) -> CargoResult<Option<Target>> {
    let inferred = inferred_lib(package_root);
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
                        || inferred.as_ref().map(|p| PathValue(p.clone()))
                    ),
                    ..lib.clone()
                }
            )
        }
        None => inferred.as_ref().map(|lib| {
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

    let path = match (lib.path.as_ref(), inferred) {
        (Some(path), _) => package_root.join(&path.0),
        (None, Some(path)) => path,
        (None, None) => {
            let legacy_path = package_root.join("src").join(format!("{}.rs", lib.name()));
            if legacy_path.exists() {
                {
                    let short_path = without_prefix(&legacy_path, package_root)
                        .unwrap_or(&legacy_path);

                    warnings.push(format!(
                        "path `{}` was erroneously implicitly accepted for library {},\n\
                         please rename the file to `src/lib.rs` or set lib.path in Cargo.toml",
                        short_path.display(), lib.name()
                    ));
                }
                legacy_path
            } else {
                bail!("can't find library `{}`, \
                       rename file to `src/lib.rs` or specify lib.path", lib.name())
            }
        }
    };

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

    let mut target = Target::lib_target(&lib.name(), crate_types, path);
    configure(lib, &mut target);
    Ok(Some(target))
}

fn clean_bins(toml_bins: Option<&Vec<TomlBinTarget>>,
              package_root: &Path,
              package_name: &str,
              warnings: &mut Vec<String>,
              has_lib: bool) -> CargoResult<Vec<Target>> {
    let inferred = inferred_bins(package_root, package_name);
    let bins = match toml_bins {
        Some(bins) => bins.clone(),
        None => inferred.iter().map(|&(ref name, ref path)| {
            TomlTarget {
                name: Some(name.clone()),
                path: Some(PathValue(path.clone())),
                ..TomlTarget::new()
            }
        }).collect()
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
        let path = match target_path(bin, &inferred, "bin", package_root) {
            Ok(path) => path,
            Err(e) => {
                if let Some(legacy_path) = legacy_bin_path(package_root, &bin.name(), has_lib) {
                    {
                        let short_path = without_prefix(&legacy_path, package_root)
                            .unwrap_or(&legacy_path);

                        warnings.push(format!(
                            "path `{}` was erroneously implicitly accepted for binary {},\n\
                             please set bin.path in Cargo.toml",
                            short_path.display(), bin.name()
                        ));
                    }
                    legacy_path
                } else {
                    return Err(e);
                }
            }
        };

        let mut target = Target::bin_target(&bin.name(), path,
                                            bin.required_features.clone());
        configure(bin, &mut target);
        result.push(target);
    }
    return Ok(result);

    fn legacy_bin_path(package_root: &Path, name: &str, has_lib: bool) -> Option<PathBuf> {
        if !has_lib {
            let path = package_root.join("src").join(format!("{}.rs", name));
            if path.exists() {
                return Some(path);
            }
        }
        let path = package_root.join("src").join("main.rs");
        if path.exists() {
            return Some(path);
        }

        let path = package_root.join("src").join("bin").join("main.rs");
        if path.exists() {
            return Some(path);
        }
        return None;
    }
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
    let inferred = inferred_benches(package_root);
    let benches = match toml_benches {
        Some(benches) => benches.clone(),
        None => inferred.iter().map(|&(ref name, ref path)| {
            TomlTarget {
                name: Some(name.clone()),
                path: Some(PathValue(path.clone())),
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
        let path = target_path(bench, &inferred, "bench", package_root)?;

        let mut target = Target::bench_target(&bench.name(), path,
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

fn is_not_dotfile(entry: &DirEntry) -> bool {
    entry.file_name().to_str().map(|s| s.starts_with('.')) == Some(false)
}
