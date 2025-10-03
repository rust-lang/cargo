//! This module implements Cargo conventions for directory layout:
//!
//!  * `src/lib.rs` is a library
//!  * `src/main.rs` is a binary
//!  * `src/bin/*.rs` are binaries
//!  * `examples/*.rs` are examples
//!  * `tests/*.rs` are integration tests
//!  * `benches/*.rs` are benchmarks
//!
//! It is a bit tricky because we need match explicit information from `Cargo.toml`
//! with implicit info in directory layout.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs::{self, DirEntry};
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use cargo_util::paths;
use cargo_util_schemas::manifest::{
    PathValue, StringOrVec, TomlBenchTarget, TomlBinTarget, TomlExampleTarget, TomlLibTarget,
    TomlManifest, TomlPackageBuild, TomlTarget, TomlTestTarget,
};

use crate::core::compiler::{CrateType, rustdoc::RustdocScrapeExamples};
use crate::core::{Edition, Feature, Features, Target};
use crate::util::{
    closest_msg, errors::CargoResult, restricted_names, toml::deprecated_underscore,
};

const DEFAULT_TEST_DIR_NAME: &'static str = "tests";
const DEFAULT_BENCH_DIR_NAME: &'static str = "benches";
const DEFAULT_EXAMPLE_DIR_NAME: &'static str = "examples";

const TARGET_KIND_HUMAN_LIB: &str = "library";
const TARGET_KIND_HUMAN_BIN: &str = "binary";
const TARGET_KIND_HUMAN_EXAMPLE: &str = "example";
const TARGET_KIND_HUMAN_TEST: &str = "test";
const TARGET_KIND_HUMAN_BENCH: &str = "benchmark";

const TARGET_KIND_LIB: &str = "lib";
const TARGET_KIND_BIN: &str = "bin";
const TARGET_KIND_EXAMPLE: &str = "example";
const TARGET_KIND_TEST: &str = "test";
const TARGET_KIND_BENCH: &str = "bench";

#[tracing::instrument(skip_all)]
pub(super) fn to_targets(
    features: &Features,
    original_toml: &TomlManifest,
    normalized_toml: &TomlManifest,
    package_root: &Path,
    edition: Edition,
    metabuild: &Option<StringOrVec>,
    warnings: &mut Vec<String>,
) -> CargoResult<Vec<Target>> {
    let mut targets = Vec::new();

    if let Some(target) = to_lib_target(
        original_toml.lib.as_ref(),
        normalized_toml.lib.as_ref(),
        package_root,
        edition,
        warnings,
    )? {
        targets.push(target);
    }

    let package = normalized_toml
        .package
        .as_ref()
        .ok_or_else(|| anyhow::format_err!("manifest has no `package` (or `project`)"))?;

    targets.extend(to_bin_targets(
        features,
        normalized_toml.bin.as_deref().unwrap_or_default(),
        package_root,
        edition,
        warnings,
    )?);

    targets.extend(to_example_targets(
        normalized_toml.example.as_deref().unwrap_or_default(),
        package_root,
        edition,
        warnings,
    )?);

    targets.extend(to_test_targets(
        normalized_toml.test.as_deref().unwrap_or_default(),
        package_root,
        edition,
        warnings,
    )?);

    targets.extend(to_bench_targets(
        normalized_toml.bench.as_deref().unwrap_or_default(),
        package_root,
        edition,
        warnings,
    )?);

    // processing the custom build script
    if let Some(custom_build) = package.normalized_build().expect("previously normalized") {
        if metabuild.is_some() {
            anyhow::bail!("cannot specify both `metabuild` and `build`");
        }
        validate_unique_build_scripts(custom_build)?;
        for script in custom_build {
            let script_path = Path::new(script);
            let name = format!(
                "build-script-{}",
                script_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
            );
            targets.push(Target::custom_build_target(
                &name,
                package_root.join(script_path),
                edition,
            ));
        }
    }
    if let Some(metabuild) = metabuild {
        // Verify names match available build deps.
        let bdeps = normalized_toml.build_dependencies.as_ref();
        for name in &metabuild.0 {
            if !bdeps.map_or(false, |bd| bd.contains_key(name.as_str())) {
                anyhow::bail!(
                    "metabuild package `{}` must be specified in `build-dependencies`",
                    name
                );
            }
        }

        targets.push(Target::metabuild_target(&format!(
            "metabuild-{}",
            package.normalized_name().expect("previously normalized")
        )));
    }

    Ok(targets)
}

#[tracing::instrument(skip_all)]
pub fn normalize_lib(
    original_lib: Option<&TomlLibTarget>,
    package_root: &Path,
    package_name: &str,
    edition: Edition,
    autodiscover: Option<bool>,
    warnings: &mut Vec<String>,
) -> CargoResult<Option<TomlLibTarget>> {
    if is_normalized(original_lib, autodiscover) {
        let Some(mut lib) = original_lib.cloned() else {
            return Ok(None);
        };

        // Check early to improve error messages
        validate_lib_name(&lib, warnings)?;

        validate_proc_macro(&lib, TARGET_KIND_HUMAN_LIB, edition, warnings)?;
        validate_crate_types(&lib, TARGET_KIND_HUMAN_LIB, edition, warnings)?;

        if let Some(PathValue(path)) = &lib.path {
            lib.path = Some(PathValue(paths::normalize_path(path).into()));
        }

        Ok(Some(lib))
    } else {
        let inferred = inferred_lib(package_root);
        let lib = original_lib.cloned().or_else(|| {
            inferred.as_ref().map(|lib| TomlTarget {
                path: Some(PathValue(lib.clone())),
                ..TomlTarget::new()
            })
        });
        let Some(mut lib) = lib else { return Ok(None) };
        lib.name
            .get_or_insert_with(|| package_name.replace("-", "_"));

        // Check early to improve error messages
        validate_lib_name(&lib, warnings)?;

        validate_proc_macro(&lib, TARGET_KIND_HUMAN_LIB, edition, warnings)?;
        validate_crate_types(&lib, TARGET_KIND_HUMAN_LIB, edition, warnings)?;

        if lib.path.is_none() {
            if let Some(inferred) = inferred {
                lib.path = Some(PathValue(inferred));
            } else {
                let name = name_or_panic(&lib);
                let legacy_path = Path::new("src").join(format!("{name}.rs"));
                if edition == Edition::Edition2015 && package_root.join(&legacy_path).exists() {
                    warnings.push(format!(
                        "path `{}` was erroneously implicitly accepted for library `{name}`,\n\
                     please rename the file to `src/lib.rs` or set lib.path in Cargo.toml",
                        legacy_path.display(),
                    ));
                    lib.path = Some(PathValue(legacy_path));
                } else {
                    anyhow::bail!(
                        "can't find library `{name}`, \
                     rename file to `src/lib.rs` or specify lib.path",
                    )
                }
            }
        }

        if let Some(PathValue(path)) = lib.path.as_ref() {
            lib.path = Some(PathValue(paths::normalize_path(&path).into()));
        }

        Ok(Some(lib))
    }
}

#[tracing::instrument(skip_all)]
fn to_lib_target(
    original_lib: Option<&TomlLibTarget>,
    normalized_lib: Option<&TomlLibTarget>,
    package_root: &Path,
    edition: Edition,
    warnings: &mut Vec<String>,
) -> CargoResult<Option<Target>> {
    let Some(lib) = normalized_lib else {
        return Ok(None);
    };

    let path = lib.path.as_ref().expect("previously normalized");
    let path = package_root.join(&path.0);

    // Per the Macros 1.1 RFC:
    //
    // > Initially if a crate is compiled with the `proc-macro` crate type
    // > (and possibly others) it will forbid exporting any items in the
    // > crate other than those functions tagged #[proc_macro_derive] and
    // > those functions must also be placed at the crate root.
    //
    // A plugin requires exporting plugin_registrar so a crate cannot be
    // both at once.
    let crate_types = match (lib.crate_types(), lib.proc_macro()) {
        (Some(kinds), _)
            if kinds.contains(&CrateType::Dylib.as_str().to_owned())
                && kinds.contains(&CrateType::Cdylib.as_str().to_owned()) =>
        {
            anyhow::bail!(format!(
                "library `{}` cannot set the crate type of both `dylib` and `cdylib`",
                name_or_panic(lib)
            ));
        }
        (Some(kinds), _) if kinds.contains(&"proc-macro".to_string()) => {
            warnings.push(format!(
                "library `{}` should only specify `proc-macro = true` instead of setting `crate-type`",
                name_or_panic(lib)
            ));
            if kinds.len() > 1 {
                anyhow::bail!("cannot mix `proc-macro` crate type with others");
            }
            vec![CrateType::ProcMacro]
        }
        (Some(kinds), _) => kinds.iter().map(|s| s.into()).collect(),
        (None, Some(true)) => vec![CrateType::ProcMacro],
        (None, _) => vec![CrateType::Lib],
    };

    let mut target = Target::lib_target(name_or_panic(lib), crate_types, path, edition);
    configure(lib, &mut target, TARGET_KIND_HUMAN_LIB, warnings)?;
    target.set_name_inferred(original_lib.map_or(true, |v| v.name.is_none()));
    Ok(Some(target))
}

#[tracing::instrument(skip_all)]
pub fn normalize_bins(
    toml_bins: Option<&Vec<TomlBinTarget>>,
    package_root: &Path,
    package_name: &str,
    edition: Edition,
    autodiscover: Option<bool>,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
    has_lib: bool,
) -> CargoResult<Vec<TomlBinTarget>> {
    if are_normalized(toml_bins, autodiscover) {
        let mut toml_bins = toml_bins.cloned().unwrap_or_default();
        for bin in toml_bins.iter_mut() {
            validate_bin_name(bin, warnings)?;
            validate_bin_crate_types(bin, edition, warnings, errors)?;
            validate_bin_proc_macro(bin, edition, warnings, errors)?;

            if let Some(PathValue(path)) = &bin.path {
                bin.path = Some(PathValue(paths::normalize_path(path).into()));
            }
        }
        Ok(toml_bins)
    } else {
        let inferred = inferred_bins(package_root, package_name);

        let mut bins = toml_targets_and_inferred(
            toml_bins,
            &inferred,
            package_root,
            autodiscover,
            edition,
            warnings,
            TARGET_KIND_HUMAN_BIN,
            TARGET_KIND_BIN,
            "autobins",
        );

        for bin in &mut bins {
            // Check early to improve error messages
            validate_bin_name(bin, warnings)?;

            validate_bin_crate_types(bin, edition, warnings, errors)?;
            validate_bin_proc_macro(bin, edition, warnings, errors)?;

            let path = target_path(
                bin,
                &inferred,
                TARGET_KIND_BIN,
                package_root,
                edition,
                &mut |_| {
                    if let Some(legacy_path) =
                        legacy_bin_path(package_root, name_or_panic(bin), has_lib)
                    {
                        warnings.push(format!(
                            "path `{}` was erroneously implicitly accepted for binary `{}`,\n\
                     please set bin.path in Cargo.toml",
                            legacy_path.display(),
                            name_or_panic(bin)
                        ));
                        Some(legacy_path)
                    } else {
                        None
                    }
                },
            );
            let path = match path {
                Ok(path) => paths::normalize_path(&path).into(),
                Err(e) => anyhow::bail!("{}", e),
            };
            bin.path = Some(PathValue(path));
        }

        Ok(bins)
    }
}

#[tracing::instrument(skip_all)]
fn to_bin_targets(
    features: &Features,
    bins: &[TomlBinTarget],
    package_root: &Path,
    edition: Edition,
    warnings: &mut Vec<String>,
) -> CargoResult<Vec<Target>> {
    // This loop performs basic checks on each of the TomlTarget in `bins`.
    for bin in bins {
        // For each binary, check if the `filename` parameter is populated. If it is,
        // check if the corresponding cargo feature has been activated.
        if bin.filename.is_some() {
            features.require(Feature::different_binary_name())?;
        }
    }

    validate_unique_names(&bins, TARGET_KIND_HUMAN_BIN)?;

    let mut result = Vec::new();
    for bin in bins {
        let path = package_root.join(&bin.path.as_ref().expect("previously normalized").0);
        let mut target = Target::bin_target(
            name_or_panic(bin),
            bin.filename.clone(),
            path,
            bin.required_features.clone(),
            edition,
        );

        configure(bin, &mut target, TARGET_KIND_HUMAN_BIN, warnings)?;
        result.push(target);
    }
    Ok(result)
}

fn legacy_bin_path(package_root: &Path, name: &str, has_lib: bool) -> Option<PathBuf> {
    if !has_lib {
        let rel_path = Path::new("src").join(format!("{}.rs", name));
        if package_root.join(&rel_path).exists() {
            return Some(rel_path);
        }
    }

    let rel_path = Path::new("src").join("main.rs");
    if package_root.join(&rel_path).exists() {
        return Some(rel_path);
    }

    let default_bin_dir_name = Path::new("src").join("bin");
    let rel_path = default_bin_dir_name.join("main.rs");
    if package_root.join(&rel_path).exists() {
        return Some(rel_path);
    }
    None
}

#[tracing::instrument(skip_all)]
pub fn normalize_examples(
    toml_examples: Option<&Vec<TomlExampleTarget>>,
    package_root: &Path,
    edition: Edition,
    autodiscover: Option<bool>,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) -> CargoResult<Vec<TomlExampleTarget>> {
    let mut inferred = || infer_from_directory(&package_root, Path::new(DEFAULT_EXAMPLE_DIR_NAME));

    let targets = normalize_targets(
        TARGET_KIND_HUMAN_EXAMPLE,
        TARGET_KIND_EXAMPLE,
        toml_examples,
        &mut inferred,
        package_root,
        edition,
        autodiscover,
        warnings,
        errors,
        "autoexamples",
    )?;

    Ok(targets)
}

#[tracing::instrument(skip_all)]
fn to_example_targets(
    targets: &[TomlExampleTarget],
    package_root: &Path,
    edition: Edition,
    warnings: &mut Vec<String>,
) -> CargoResult<Vec<Target>> {
    validate_unique_names(&targets, TARGET_KIND_EXAMPLE)?;

    let mut result = Vec::new();
    for toml in targets {
        let path = package_root.join(&toml.path.as_ref().expect("previously normalized").0);
        let crate_types = match toml.crate_types() {
            Some(kinds) => kinds.iter().map(|s| s.into()).collect(),
            None => Vec::new(),
        };

        let mut target = Target::example_target(
            name_or_panic(&toml),
            crate_types,
            path,
            toml.required_features.clone(),
            edition,
        );
        configure(&toml, &mut target, TARGET_KIND_HUMAN_EXAMPLE, warnings)?;
        result.push(target);
    }

    Ok(result)
}

#[tracing::instrument(skip_all)]
pub fn normalize_tests(
    toml_tests: Option<&Vec<TomlTestTarget>>,
    package_root: &Path,
    edition: Edition,
    autodiscover: Option<bool>,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) -> CargoResult<Vec<TomlTestTarget>> {
    let mut inferred = || infer_from_directory(&package_root, Path::new(DEFAULT_TEST_DIR_NAME));

    let targets = normalize_targets(
        TARGET_KIND_HUMAN_TEST,
        TARGET_KIND_TEST,
        toml_tests,
        &mut inferred,
        package_root,
        edition,
        autodiscover,
        warnings,
        errors,
        "autotests",
    )?;

    Ok(targets)
}

#[tracing::instrument(skip_all)]
fn to_test_targets(
    targets: &[TomlTestTarget],
    package_root: &Path,
    edition: Edition,
    warnings: &mut Vec<String>,
) -> CargoResult<Vec<Target>> {
    validate_unique_names(&targets, TARGET_KIND_TEST)?;

    let mut result = Vec::new();
    for toml in targets {
        let path = package_root.join(&toml.path.as_ref().expect("previously normalized").0);
        let mut target = Target::test_target(
            name_or_panic(&toml),
            path,
            toml.required_features.clone(),
            edition,
        );
        configure(&toml, &mut target, TARGET_KIND_HUMAN_TEST, warnings)?;
        result.push(target);
    }
    Ok(result)
}

#[tracing::instrument(skip_all)]
pub fn normalize_benches(
    toml_benches: Option<&Vec<TomlBenchTarget>>,
    package_root: &Path,
    edition: Edition,
    autodiscover: Option<bool>,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) -> CargoResult<Vec<TomlBenchTarget>> {
    let mut legacy_warnings = vec![];
    let mut legacy_bench_path = |bench: &TomlTarget| {
        let legacy_path = Path::new("src").join("bench.rs");
        if !(name_or_panic(bench) == "bench" && package_root.join(&legacy_path).exists()) {
            return None;
        }
        legacy_warnings.push(format!(
            "path `{}` was erroneously implicitly accepted for benchmark `{}`,\n\
                 please set bench.path in Cargo.toml",
            legacy_path.display(),
            name_or_panic(bench)
        ));
        Some(legacy_path)
    };

    let mut inferred = || infer_from_directory(&package_root, Path::new(DEFAULT_BENCH_DIR_NAME));

    let targets = normalize_targets_with_legacy_path(
        TARGET_KIND_HUMAN_BENCH,
        TARGET_KIND_BENCH,
        toml_benches,
        &mut inferred,
        package_root,
        edition,
        autodiscover,
        warnings,
        errors,
        &mut legacy_bench_path,
        "autobenches",
    )?;
    warnings.append(&mut legacy_warnings);

    Ok(targets)
}

#[tracing::instrument(skip_all)]
fn to_bench_targets(
    targets: &[TomlBenchTarget],
    package_root: &Path,
    edition: Edition,
    warnings: &mut Vec<String>,
) -> CargoResult<Vec<Target>> {
    validate_unique_names(&targets, TARGET_KIND_BENCH)?;

    let mut result = Vec::new();
    for toml in targets {
        let path = package_root.join(&toml.path.as_ref().expect("previously normalized").0);
        let mut target = Target::bench_target(
            name_or_panic(&toml),
            path,
            toml.required_features.clone(),
            edition,
        );
        configure(&toml, &mut target, TARGET_KIND_HUMAN_BENCH, warnings)?;
        result.push(target);
    }

    Ok(result)
}

fn is_normalized(toml_target: Option<&TomlTarget>, autodiscover: Option<bool>) -> bool {
    are_normalized_(toml_target.map(std::slice::from_ref), autodiscover)
}

fn are_normalized(toml_targets: Option<&Vec<TomlTarget>>, autodiscover: Option<bool>) -> bool {
    are_normalized_(toml_targets.map(|v| v.as_slice()), autodiscover)
}

fn are_normalized_(toml_targets: Option<&[TomlTarget]>, autodiscover: Option<bool>) -> bool {
    if autodiscover != Some(false) {
        return false;
    }

    let Some(toml_targets) = toml_targets else {
        return true;
    };
    toml_targets
        .iter()
        .all(|t| t.name.is_some() && t.path.is_some())
}

fn normalize_targets(
    target_kind_human: &str,
    target_kind: &str,
    toml_targets: Option<&Vec<TomlTarget>>,
    inferred: &mut dyn FnMut() -> Vec<(String, PathBuf)>,
    package_root: &Path,
    edition: Edition,
    autodiscover: Option<bool>,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
    autodiscover_flag_name: &str,
) -> CargoResult<Vec<TomlTarget>> {
    normalize_targets_with_legacy_path(
        target_kind_human,
        target_kind,
        toml_targets,
        inferred,
        package_root,
        edition,
        autodiscover,
        warnings,
        errors,
        &mut |_| None,
        autodiscover_flag_name,
    )
}

fn normalize_targets_with_legacy_path(
    target_kind_human: &str,
    target_kind: &str,
    toml_targets: Option<&Vec<TomlTarget>>,
    inferred: &mut dyn FnMut() -> Vec<(String, PathBuf)>,
    package_root: &Path,
    edition: Edition,
    autodiscover: Option<bool>,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
    legacy_path: &mut dyn FnMut(&TomlTarget) -> Option<PathBuf>,
    autodiscover_flag_name: &str,
) -> CargoResult<Vec<TomlTarget>> {
    if are_normalized(toml_targets, autodiscover) {
        let mut toml_targets = toml_targets.cloned().unwrap_or_default();
        for target in toml_targets.iter_mut() {
            // Check early to improve error messages
            validate_target_name(target, target_kind_human, target_kind, warnings)?;

            validate_proc_macro(target, target_kind_human, edition, warnings)?;
            validate_crate_types(target, target_kind_human, edition, warnings)?;

            if let Some(PathValue(path)) = &target.path {
                target.path = Some(PathValue(paths::normalize_path(path).into()));
            }
        }
        Ok(toml_targets)
    } else {
        let inferred = inferred();
        let toml_targets = toml_targets_and_inferred(
            toml_targets,
            &inferred,
            package_root,
            autodiscover,
            edition,
            warnings,
            target_kind_human,
            target_kind,
            autodiscover_flag_name,
        );

        for target in &toml_targets {
            // Check early to improve error messages
            validate_target_name(target, target_kind_human, target_kind, warnings)?;

            validate_proc_macro(target, target_kind_human, edition, warnings)?;
            validate_crate_types(target, target_kind_human, edition, warnings)?;
        }

        let mut result = Vec::new();
        for mut target in toml_targets {
            let path = target_path(
                &target,
                &inferred,
                target_kind,
                package_root,
                edition,
                legacy_path,
            );
            let path = match path {
                Ok(path) => path,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            target.path = Some(PathValue(paths::normalize_path(&path).into()));
            result.push(target);
        }
        Ok(result)
    }
}

fn inferred_lib(package_root: &Path) -> Option<PathBuf> {
    let lib = Path::new("src").join("lib.rs");
    if package_root.join(&lib).exists() {
        Some(lib)
    } else {
        None
    }
}

fn inferred_bins(package_root: &Path, package_name: &str) -> Vec<(String, PathBuf)> {
    let main = "src/main.rs";
    let mut result = Vec::new();
    if package_root.join(main).exists() {
        let main = PathBuf::from(main);
        result.push((package_name.to_string(), main));
    }
    let default_bin_dir_name = Path::new("src").join("bin");
    result.extend(infer_from_directory(package_root, &default_bin_dir_name));

    result
}

fn infer_from_directory(package_root: &Path, relpath: &Path) -> Vec<(String, PathBuf)> {
    let directory = package_root.join(relpath);
    let entries = match fs::read_dir(directory) {
        Err(_) => return Vec::new(),
        Ok(dir) => dir,
    };

    entries
        .filter_map(|e| e.ok())
        .filter(is_not_dotfile)
        .filter_map(|d| infer_any(package_root, &d))
        .collect()
}

fn infer_any(package_root: &Path, entry: &DirEntry) -> Option<(String, PathBuf)> {
    if entry.file_type().map_or(false, |t| t.is_dir()) {
        infer_subdirectory(package_root, entry)
    } else if entry.path().extension().and_then(|p| p.to_str()) == Some("rs") {
        infer_file(package_root, entry)
    } else {
        None
    }
}

fn infer_file(package_root: &Path, entry: &DirEntry) -> Option<(String, PathBuf)> {
    let path = entry.path();
    let stem = path.file_stem()?.to_str()?.to_owned();
    let path = path
        .strip_prefix(package_root)
        .map(|p| p.to_owned())
        .unwrap_or(path);
    Some((stem, path))
}

fn infer_subdirectory(package_root: &Path, entry: &DirEntry) -> Option<(String, PathBuf)> {
    let path = entry.path();
    let main = path.join("main.rs");
    let name = path.file_name()?.to_str()?.to_owned();
    if main.exists() {
        let main = main
            .strip_prefix(package_root)
            .map(|p| p.to_owned())
            .unwrap_or(main);
        Some((name, main))
    } else {
        None
    }
}

fn is_not_dotfile(entry: &DirEntry) -> bool {
    entry.file_name().to_str().map(|s| s.starts_with('.')) == Some(false)
}

fn toml_targets_and_inferred(
    toml_targets: Option<&Vec<TomlTarget>>,
    inferred: &[(String, PathBuf)],
    package_root: &Path,
    autodiscover: Option<bool>,
    edition: Edition,
    warnings: &mut Vec<String>,
    target_kind_human: &str,
    target_kind: &str,
    autodiscover_flag_name: &str,
) -> Vec<TomlTarget> {
    let inferred_targets = inferred_to_toml_targets(inferred);
    let mut toml_targets = match toml_targets {
        None => {
            if let Some(false) = autodiscover {
                vec![]
            } else {
                inferred_targets
            }
        }
        Some(targets) => {
            let mut targets = targets.clone();

            let target_path =
                |target: &TomlTarget| target.path.clone().map(|p| package_root.join(p.0));

            let mut seen_names = HashSet::new();
            let mut seen_paths = HashSet::new();
            for target in targets.iter() {
                seen_names.insert(target.name.clone());
                seen_paths.insert(target_path(target));
            }

            let mut rem_targets = vec![];
            for target in inferred_targets {
                if !seen_names.contains(&target.name) && !seen_paths.contains(&target_path(&target))
                {
                    rem_targets.push(target);
                }
            }

            let autodiscover = match autodiscover {
                Some(autodiscover) => autodiscover,
                None => {
                    if edition == Edition::Edition2015 {
                        if !rem_targets.is_empty() {
                            let mut rem_targets_str = String::new();
                            for t in rem_targets.iter() {
                                if let Some(p) = t.path.clone() {
                                    rem_targets_str.push_str(&format!("* {}\n", p.0.display()))
                                }
                            }
                            warnings.push(format!(
                                "\
An explicit [[{section}]] section is specified in Cargo.toml which currently
disables Cargo from automatically inferring other {target_kind_human} targets.
This inference behavior will change in the Rust 2018 edition and the following
files will be included as a {target_kind_human} target:

{rem_targets_str}
This is likely to break cargo build or cargo test as these files may not be
ready to be compiled as a {target_kind_human} target today. You can future-proof yourself
and disable this warning by adding `{autodiscover_flag_name} = false` to your [package]
section. You may also move the files to a location where Cargo would not
automatically infer them to be a target, such as in subfolders.

For more information on this warning you can consult
https://github.com/rust-lang/cargo/issues/5330",
                                section = target_kind,
                                target_kind_human = target_kind_human,
                                rem_targets_str = rem_targets_str,
                                autodiscover_flag_name = autodiscover_flag_name,
                            ));
                        };
                        false
                    } else {
                        true
                    }
                }
            };

            if autodiscover {
                targets.append(&mut rem_targets);
            }

            targets
        }
    };
    // Ensure target order is deterministic, particularly for `cargo vendor` where re-vendoring
    // should not cause changes.
    //
    // `unstable` should be deterministic because we enforce that `t.name` is unique
    toml_targets.sort_unstable_by_key(|t| t.name.clone());
    toml_targets
}

fn inferred_to_toml_targets(inferred: &[(String, PathBuf)]) -> Vec<TomlTarget> {
    inferred
        .iter()
        .map(|(name, path)| TomlTarget {
            name: Some(name.clone()),
            path: Some(PathValue(path.clone())),
            ..TomlTarget::new()
        })
        .collect()
}

/// Will check a list of toml targets, and make sure the target names are unique within a vector.
fn validate_unique_names(targets: &[TomlTarget], target_kind: &str) -> CargoResult<()> {
    let mut seen = HashSet::new();
    for name in targets.iter().map(|e| name_or_panic(e)) {
        if !seen.insert(name) {
            anyhow::bail!(
                "found duplicate {target_kind} name {name}, \
                 but all {target_kind} targets must have a unique name",
                target_kind = target_kind,
                name = name
            );
        }
    }
    Ok(())
}

/// Will check a list of build scripts, and make sure script file stems are unique within a vector.
fn validate_unique_build_scripts(scripts: &[String]) -> CargoResult<()> {
    let mut seen = HashMap::new();
    for script in scripts {
        let stem = Path::new(script).file_stem().unwrap().to_str().unwrap();
        seen.entry(stem)
            .or_insert_with(Vec::new)
            .push(script.as_str());
    }
    let mut conflict_file_stem = false;
    let mut err_msg = String::from(
        "found build scripts with duplicate file stems, but all build scripts must have a unique file stem",
    );
    for (stem, paths) in seen {
        if paths.len() > 1 {
            conflict_file_stem = true;
            write!(&mut err_msg, "\n  for stem `{stem}`: {}", paths.join(", "))?;
        }
    }
    if conflict_file_stem {
        anyhow::bail!(err_msg);
    }
    Ok(())
}

fn configure(
    toml: &TomlTarget,
    target: &mut Target,
    target_kind_human: &str,
    warnings: &mut Vec<String>,
) -> CargoResult<()> {
    let t2 = target.clone();
    target
        .set_tested(toml.test.unwrap_or_else(|| t2.tested()))
        .set_doc(toml.doc.unwrap_or_else(|| t2.documented()))
        .set_doctest(toml.doctest.unwrap_or_else(|| t2.doctested()))
        .set_benched(toml.bench.unwrap_or_else(|| t2.benched()))
        .set_harness(toml.harness.unwrap_or_else(|| t2.harness()))
        .set_proc_macro(toml.proc_macro().unwrap_or_else(|| t2.proc_macro()))
        .set_doc_scrape_examples(match toml.doc_scrape_examples {
            None => RustdocScrapeExamples::Unset,
            Some(false) => RustdocScrapeExamples::Disabled,
            Some(true) => RustdocScrapeExamples::Enabled,
        })
        .set_for_host(toml.proc_macro().unwrap_or_else(|| t2.for_host()));

    if let Some(edition) = toml.edition.clone() {
        let name = target.name();
        warnings.push(format!(
            "`edition` is set on {target_kind_human} `{name}` which is deprecated"
        ));
        target.set_edition(
            edition
                .parse()
                .context("failed to parse the `edition` key")?,
        );
    }
    Ok(())
}

/// Build an error message for a target path that cannot be determined either
/// by auto-discovery or specifying.
///
/// This function tries to detect commonly wrong paths for targets:
///
/// test -> tests/*.rs, tests/*/main.rs
/// bench -> benches/*.rs, benches/*/main.rs
/// example -> examples/*.rs, examples/*/main.rs
/// bin -> src/bin/*.rs, src/bin/*/main.rs
///
/// Note that the logic need to sync with [`infer_from_directory`] if changes.
fn target_path_not_found_error_message(
    package_root: &Path,
    target: &TomlTarget,
    target_kind: &str,
    inferred: &[(String, PathBuf)],
) -> String {
    fn possible_target_paths(name: &str, kind: &str, commonly_wrong: bool) -> [PathBuf; 2] {
        let mut target_path = PathBuf::new();
        match (kind, commonly_wrong) {
            // commonly wrong paths
            ("test" | "bench" | "example", true) => target_path.push(kind),
            ("bin", true) => target_path.extend(["src", "bins"]),
            // default inferred paths
            ("test", false) => target_path.push(DEFAULT_TEST_DIR_NAME),
            ("bench", false) => target_path.push(DEFAULT_BENCH_DIR_NAME),
            ("example", false) => target_path.push(DEFAULT_EXAMPLE_DIR_NAME),
            ("bin", false) => target_path.extend(["src", "bin"]),
            _ => unreachable!("invalid target kind: {}", kind),
        }

        let target_path_file = {
            let mut path = target_path.clone();
            path.push(format!("{name}.rs"));
            path
        };
        let target_path_subdir = {
            target_path.extend([name, "main.rs"]);
            target_path
        };
        return [target_path_file, target_path_subdir];
    }

    let target_name = name_or_panic(target);

    let commonly_wrong_paths = possible_target_paths(&target_name, target_kind, true);
    let possible_paths = possible_target_paths(&target_name, target_kind, false);

    let msg = closest_msg(target_name, inferred.iter(), |(n, _p)| n, target_kind);
    if let Some((wrong_path, possible_path)) = commonly_wrong_paths
        .iter()
        .zip(possible_paths.iter())
        .filter(|(wp, _)| package_root.join(wp).exists())
        .next()
    {
        let [wrong_path, possible_path] = [wrong_path, possible_path].map(|p| p.display());
        format!(
            "can't find `{target_name}` {target_kind} at default paths, but found a file at `{wrong_path}`.\n\
             Perhaps rename the file to `{possible_path}` for target auto-discovery, \
             or specify {target_kind}.path if you want to use a non-default path.{msg}",
        )
    } else {
        let [path_file, path_dir] = possible_paths.each_ref().map(|p| p.display());
        format!(
            "can't find `{target_name}` {target_kind} at `{path_file}` or `{path_dir}`. \
             Please specify {target_kind}.path if you want to use a non-default path.{msg}"
        )
    }
}

fn target_path(
    target: &TomlTarget,
    inferred: &[(String, PathBuf)],
    target_kind: &str,
    package_root: &Path,
    edition: Edition,
    legacy_path: &mut dyn FnMut(&TomlTarget) -> Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(ref path) = target.path {
        // Should we verify that this path exists here?
        return Ok(path.0.clone());
    }
    let name = name_or_panic(target).to_owned();

    let mut matching = inferred
        .iter()
        .filter(|(n, _)| n == &name)
        .map(|(_, p)| p.clone());

    let first = matching.next();
    let second = matching.next();
    match (first, second) {
        (Some(path), None) => Ok(path),
        (None, None) => {
            if edition == Edition::Edition2015 {
                if let Some(path) = legacy_path(target) {
                    return Ok(path);
                }
            }
            Err(target_path_not_found_error_message(
                package_root,
                target,
                target_kind,
                inferred,
            ))
        }
        (Some(p0), Some(p1)) => {
            if edition == Edition::Edition2015 {
                if let Some(path) = legacy_path(target) {
                    return Ok(path);
                }
            }
            Err(format!(
                "\
cannot infer path for `{}` {}
Cargo doesn't know which to use because multiple target files found at `{}` and `{}`.",
                name_or_panic(target),
                target_kind,
                p0.strip_prefix(package_root).unwrap_or(&p0).display(),
                p1.strip_prefix(package_root).unwrap_or(&p1).display(),
            ))
        }
        (None, Some(_)) => unreachable!(),
    }
}

/// Returns the path to the build script if one exists for this crate.
#[tracing::instrument(skip_all)]
pub fn normalize_build(
    build: Option<&TomlPackageBuild>,
    package_root: &Path,
) -> CargoResult<Option<TomlPackageBuild>> {
    const BUILD_RS: &str = "build.rs";
    match build {
        None => {
            // If there is a `build.rs` file next to the `Cargo.toml`, assume it is
            // a build script.
            let build_rs = package_root.join(BUILD_RS);
            if build_rs.is_file() {
                Ok(Some(TomlPackageBuild::SingleScript(BUILD_RS.to_owned())))
            } else {
                Ok(Some(TomlPackageBuild::Auto(false)))
            }
        }
        // Explicitly no build script.
        Some(TomlPackageBuild::Auto(false)) => Ok(build.cloned()),
        Some(TomlPackageBuild::SingleScript(build_file)) => {
            let build_file = paths::normalize_path(Path::new(build_file));
            let build = build_file.into_os_string().into_string().expect(
                "`build_file` started as a String and `normalize_path` shouldn't have changed that",
            );
            Ok(Some(TomlPackageBuild::SingleScript(build)))
        }
        Some(TomlPackageBuild::Auto(true)) => {
            Ok(Some(TomlPackageBuild::SingleScript(BUILD_RS.to_owned())))
        }
        Some(TomlPackageBuild::MultipleScript(_scripts)) => Ok(build.cloned()),
    }
}

fn name_or_panic(target: &TomlTarget) -> &str {
    target
        .name
        .as_deref()
        .unwrap_or_else(|| panic!("target name is required"))
}

fn validate_lib_name(target: &TomlTarget, warnings: &mut Vec<String>) -> CargoResult<()> {
    validate_target_name(target, TARGET_KIND_HUMAN_LIB, TARGET_KIND_LIB, warnings)?;
    let name = name_or_panic(target);
    if name.contains('-') {
        anyhow::bail!("library target names cannot contain hyphens: {}", name)
    }

    Ok(())
}

fn validate_bin_name(bin: &TomlTarget, warnings: &mut Vec<String>) -> CargoResult<()> {
    validate_target_name(bin, TARGET_KIND_HUMAN_BIN, TARGET_KIND_BIN, warnings)?;
    let name = name_or_panic(bin).to_owned();
    if restricted_names::is_conflicting_artifact_name(&name) {
        anyhow::bail!(
            "the binary target name `{name}` is forbidden, \
                 it conflicts with cargo's build directory names",
        )
    }

    Ok(())
}

fn validate_target_name(
    target: &TomlTarget,
    target_kind_human: &str,
    target_kind: &str,
    warnings: &mut Vec<String>,
) -> CargoResult<()> {
    match target.name {
        Some(ref name) => {
            if name.trim().is_empty() {
                anyhow::bail!("{} target names cannot be empty", target_kind_human)
            }
            if cfg!(windows) && restricted_names::is_windows_reserved(name) {
                warnings.push(format!(
                    "{} target `{}` is a reserved Windows filename, \
                        this target will not work on Windows platforms",
                    target_kind_human, name
                ));
            }
        }
        None => anyhow::bail!(
            "{} target {}.name is required",
            target_kind_human,
            target_kind
        ),
    }

    Ok(())
}

fn validate_bin_proc_macro(
    target: &TomlTarget,
    edition: Edition,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) -> CargoResult<()> {
    if target.proc_macro() == Some(true) {
        let name = name_or_panic(target);
        errors.push(format!(
            "the target `{}` is a binary and can't have `proc-macro` \
                 set `true`",
            name
        ));
    } else {
        validate_proc_macro(target, TARGET_KIND_HUMAN_BIN, edition, warnings)?;
    }
    Ok(())
}

fn validate_proc_macro(
    target: &TomlTarget,
    kind: &str,
    edition: Edition,
    warnings: &mut Vec<String>,
) -> CargoResult<()> {
    deprecated_underscore(
        &target.proc_macro2,
        &target.proc_macro,
        "proc-macro",
        name_or_panic(target),
        format!("{kind} target").as_str(),
        edition,
        warnings,
    )
}

fn validate_bin_crate_types(
    target: &TomlTarget,
    edition: Edition,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) -> CargoResult<()> {
    if let Some(crate_types) = target.crate_types() {
        if !crate_types.is_empty() {
            let name = name_or_panic(target);
            errors.push(format!(
                "the target `{}` is a binary and can't have any \
                     crate-types set (currently \"{}\")",
                name,
                crate_types.join(", ")
            ));
        } else {
            validate_crate_types(target, TARGET_KIND_HUMAN_BIN, edition, warnings)?;
        }
    }
    Ok(())
}

fn validate_crate_types(
    target: &TomlTarget,
    kind: &str,
    edition: Edition,
    warnings: &mut Vec<String>,
) -> CargoResult<()> {
    deprecated_underscore(
        &target.crate_type2,
        &target.crate_type,
        "crate-type",
        name_or_panic(target),
        format!("{kind} target").as_str(),
        edition,
        warnings,
    )
}
