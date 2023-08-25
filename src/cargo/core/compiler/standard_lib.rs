//! Code for building the standard library.

use crate::core::compiler::unit_dependencies::IsArtifact;
use crate::core::compiler::UnitInterner;
use crate::core::compiler::{CompileKind, CompileMode, RustcTargetData, Unit};
use crate::core::profiles::{Profiles, UnitFor};
use crate::core::resolver::features::{CliFeatures, FeaturesFor, ResolvedFeatures};
use crate::core::resolver::HasDevUnits;
use crate::core::{Dependency, PackageId, PackageSet, Resolve, SourceId, Workspace};
use crate::ops::{self, Packages};
use crate::util::errors::CargoResult;
use crate::Config;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::BuildConfig;

/// Parse the `-Zbuild-std` flag.
pub fn parse_unstable_flag(value: Option<&str>) -> Vec<String> {
    // This is a temporary hack until there is a more principled way to
    // declare dependencies in Cargo.toml.
    let value = value.unwrap_or("std");
    let mut crates: HashSet<&str> = value.split(',').collect();
    if crates.contains("std") {
        crates.insert("core");
        crates.insert("alloc");
        crates.insert("proc_macro");
        crates.insert("panic_unwind");
        crates.insert("compiler_builtins");
    } else if crates.contains("core") {
        crates.insert("compiler_builtins");
    }
    crates.into_iter().map(|s| s.to_string()).collect()
}

pub(crate) fn std_crates(config: &Config, units: Option<&[Unit]>) -> Option<Vec<String>> {
    let crates = config.cli_unstable().build_std.as_ref()?.clone();

    // Only build libtest if it looks like it is needed.
    let mut crates = crates.clone();
    // If we know what units we're building, we can filter for libtest depending on the jobs.
    if let Some(units) = units {
        if units
            .iter()
            .any(|unit| unit.mode.is_rustc_test() && unit.target.harness())
        {
            // Only build libtest when libstd is built (libtest depends on libstd)
            if crates.iter().any(|c| c == "std") && !crates.iter().any(|c| c == "test") {
                crates.push("test".to_string());
            }
        }
    } else {
        // We don't know what jobs are going to be run, so download libtest just in case.
        if !crates.iter().any(|c| c == "test") {
            crates.push("test".to_string())
        }
    }

    Some(crates)
}

/// Resolve the standard library dependencies.
pub fn resolve_std<'cfg>(
    ws: &Workspace<'cfg>,
    target_data: &mut RustcTargetData<'cfg>,
    build_config: &BuildConfig,
    crates: &[String],
) -> CargoResult<(PackageSet<'cfg>, Resolve, ResolvedFeatures)> {
    if build_config.build_plan {
        ws.config()
            .shell()
            .warn("-Zbuild-std does not currently fully support --build-plan")?;
    }

    let src_path = detect_sysroot_src_path(target_data)?;
    let to_patch = [
        "rustc-std-workspace-core",
        "rustc-std-workspace-alloc",
        "rustc-std-workspace-std",
    ];
    let patches = to_patch
        .iter()
        .map(|&name| {
            let source_path = SourceId::for_path(&src_path.join("library").join(name))?;
            let dep = Dependency::parse(name, None, source_path)?;
            Ok(dep)
        })
        .collect::<CargoResult<Vec<_>>>()?;
    let crates_io_url = crate::sources::CRATES_IO_INDEX.parse().unwrap();
    let patch = HashMap::from([(crates_io_url, patches)]);
    let members = vec![
        String::from("library/std"),
        String::from("library/core"),
        String::from("library/alloc"),
        String::from("library/sysroot"),
    ];
    let ws_config = crate::core::WorkspaceConfig::Root(crate::core::WorkspaceRootConfig::new(
        &src_path,
        &Some(members),
        /*default_members*/ &None,
        /*exclude*/ &None,
        /*inheritable*/ &None,
        /*custom_metadata*/ &None,
    ));
    let virtual_manifest = crate::core::VirtualManifest::new(
        /*replace*/ Vec::new(),
        patch,
        ws_config,
        /*profiles*/ None,
        crate::core::Features::default(),
        None,
    );

    let config = ws.config();
    // This is a delicate hack. In order for features to resolve correctly,
    // the resolver needs to run a specific "current" member of the workspace.
    // Thus, in order to set the features for `std`, we need to set `sysroot`
    // to be the "current" member. `sysroot` is the root, and all other
    // standard library crates are dependencies from there. Since none of the
    // other crates need to alter their features, this should be fine, for
    // now. Perhaps in the future features will be decoupled from the resolver
    // and it will be easier to control feature selection.
    let current_manifest = src_path.join("library/sysroot/Cargo.toml");
    // TODO: Consider doing something to enforce --locked? Or to prevent the
    // lock file from being written, such as setting ephemeral.
    let mut std_ws = Workspace::new_virtual(src_path, current_manifest, virtual_manifest, config)?;
    // Don't require optional dependencies in this workspace, aka std's own
    // `[dev-dependencies]`. No need for us to generate a `Resolve` which has
    // those included because we'll never use them anyway.
    std_ws.set_require_optional_deps(false);
    // `sysroot` is not in the default set because it is optional, but it needs
    // to be part of the resolve in case we do need it or `libtest`.
    let mut spec_pkgs = Vec::from(crates);
    spec_pkgs.push("sysroot".to_string());
    let spec = Packages::Packages(spec_pkgs);
    let specs = spec.to_package_id_specs(&std_ws)?;
    let features = match &config.cli_unstable().build_std_features {
        Some(list) => list.clone(),
        None => vec![
            "panic-unwind".to_string(),
            "backtrace".to_string(),
            "default".to_string(),
        ],
    };
    let cli_features = CliFeatures::from_command_line(
        &features, /*all_features*/ false, /*uses_default_features*/ false,
    )?;
    let max_rust_version = ws.rust_version();
    let resolve = ops::resolve_ws_with_opts(
        &std_ws,
        target_data,
        &build_config.requested_kinds,
        &cli_features,
        &specs,
        HasDevUnits::No,
        crate::core::resolver::features::ForceAllTargets::No,
        max_rust_version,
    )?;
    Ok((
        resolve.pkg_set,
        resolve.targeted_resolve,
        resolve.resolved_features,
    ))
}

/// Generate a list of root `Unit`s for the standard library.
///
/// The given slice of crate names is the root set.
pub fn generate_std_roots(
    crates: &[String],
    std_resolve: &Resolve,
    std_features: &ResolvedFeatures,
    kinds: &[CompileKind],
    package_set: &PackageSet<'_>,
    interner: &UnitInterner,
    profiles: &Profiles,
) -> CargoResult<HashMap<CompileKind, Vec<Unit>>> {
    // Generate the root Units for the standard library.
    let std_ids = crates
        .iter()
        .map(|crate_name| std_resolve.query(crate_name))
        .collect::<CargoResult<Vec<PackageId>>>()?;
    // Convert PackageId to Package.
    let std_pkgs = package_set.get_many(std_ids)?;
    // Generate a map of Units for each kind requested.
    let mut ret = HashMap::new();
    for pkg in std_pkgs {
        let lib = pkg
            .targets()
            .iter()
            .find(|t| t.is_lib())
            .expect("std has a lib");
        // I don't think we need to bother with Check here, the difference
        // in time is minimal, and the difference in caching is
        // significant.
        let mode = CompileMode::Build;
        let features = std_features.activated_features(pkg.package_id(), FeaturesFor::NormalOrDev);
        for kind in kinds {
            let list = ret.entry(*kind).or_insert_with(Vec::new);
            let unit_for = UnitFor::new_normal(*kind);
            let profile = profiles.get_profile(
                pkg.package_id(),
                /*is_member*/ false,
                /*is_local*/ false,
                unit_for,
                *kind,
            );
            list.push(interner.intern(
                pkg,
                lib,
                profile,
                *kind,
                mode,
                features.clone(),
                /*is_std*/ true,
                /*dep_hash*/ 0,
                IsArtifact::No,
                None,
            ));
        }
    }
    Ok(ret)
}

fn detect_sysroot_src_path(target_data: &RustcTargetData<'_>) -> CargoResult<PathBuf> {
    if let Some(s) = target_data.config.get_env_os("__CARGO_TESTS_ONLY_SRC_ROOT") {
        return Ok(s.into());
    }

    // NOTE: This is temporary until we figure out how to acquire the source.
    let src_path = target_data
        .info(CompileKind::Host)
        .sysroot
        .join("lib")
        .join("rustlib")
        .join("src")
        .join("rust");
    let lock = src_path.join("Cargo.lock");
    if !lock.exists() {
        let msg = format!(
            "{:?} does not exist, unable to build with the standard \
             library, try:\n        rustup component add rust-src",
            lock
        );
        match target_data.config.get_env("RUSTUP_TOOLCHAIN") {
            Ok(rustup_toolchain) => {
                anyhow::bail!("{} --toolchain {}", msg, rustup_toolchain);
            }
            Err(_) => {
                anyhow::bail!(msg);
            }
        }
    }
    Ok(src_path)
}
