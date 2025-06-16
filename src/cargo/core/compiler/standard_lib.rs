//! Code for building the standard library.

use crate::core::compiler::unit_dependencies::IsArtifact;
use crate::core::compiler::UnitInterner;
use crate::core::compiler::{CompileKind, CompileMode, RustcTargetData, Unit};
use crate::core::profiles::{Profiles, UnitFor};
use crate::core::resolver::features::{CliFeatures, FeaturesFor, ResolvedFeatures};
use crate::core::resolver::HasDevUnits;
use crate::core::{PackageId, PackageSet, Resolve, Workspace};
use crate::ops::{self, Packages};
use crate::util::errors::CargoResult;

use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::path::PathBuf;

use super::BuildConfig;

fn add_default_std_crates<T: Borrow<str> + From<&'static str> + Eq + Hash>(
    crates: &mut HashSet<T>,
    default: &'static str,
    include_libtest: bool,
) {
    // This is a temporary hack until there is a more principled way to
    // declare dependencies in Cargo.toml.
    if crates.is_empty() {
        crates.insert(default.into());
    }
    if crates.contains("std") {
        crates.insert("core".into());
        crates.insert("alloc".into());
        crates.insert("proc_macro".into());
        crates.insert("panic_unwind".into());
        crates.insert("compiler_builtins".into());
        if include_libtest {
            crates.insert("test".into());
        }
    } else if crates.contains("core") {
        crates.insert("compiler_builtins".into());
    }
}

fn std_crates(
    target_data: &RustcTargetData<'_>,
    cli_crates: Option<&[String]>,
    include_libtest: bool,
) -> HashMap<CompileKind, HashSet<String>> {
    let mut map = HashMap::new();
    for kind in target_data.all_kinds().chain([CompileKind::Host]) {
        let requested_crates = if let Some(crates) = &target_data.target_config(kind).build_std {
            crates.val.as_slice()
        } else if let Some(cli_crates) = cli_crates {
            cli_crates
        } else {
            continue;
        };

        let mut actual_crates = requested_crates
            .iter()
            .map(Clone::clone)
            .collect::<HashSet<_>>();
        add_default_std_crates(
            &mut actual_crates,
            if target_data.info(kind).maybe_support_std() {
                "std"
            } else {
                "core"
            },
            include_libtest,
        );
        map.insert(kind, actual_crates);
    }
    map
}

/// Resolve the standard library dependencies.
///
/// * `cli_crates` is the arg value from `-Zbuild-std`.
pub fn resolve_std<'gctx>(
    ws: &Workspace<'gctx>,
    target_data: &mut RustcTargetData<'gctx>,
    build_config: &BuildConfig,
    cli_crates: Option<&[String]>,
) -> CargoResult<(PackageSet<'gctx>, Resolve, ResolvedFeatures)> {
    if build_config.build_plan {
        ws.gctx()
            .shell()
            .warn("-Zbuild-std does not currently fully support --build-plan")?;
    }

    let src_path = detect_sysroot_src_path(target_data)?;
    let std_ws_manifest_path = src_path.join("Cargo.toml");
    let gctx = ws.gctx();
    // TODO: Consider doing something to enforce --locked? Or to prevent the
    // lock file from being written, such as setting ephemeral.
    let mut std_ws = Workspace::new(&std_ws_manifest_path, gctx)?;
    // Don't require optional dependencies in this workspace, aka std's own
    // `[dev-dependencies]`. No need for us to generate a `Resolve` which has
    // those included because we'll never use them anyway.
    std_ws.set_require_optional_deps(false);
    let mut build_std_features = HashSet::new();
    let specs = {
        let mut set = HashSet::new();
        for (compile_kind, new_set) in std_crates(target_data, cli_crates, false) {
            set.extend(new_set);
            if let Some(features) = &target_data.target_config(compile_kind).build_std_features {
                build_std_features.extend(features.val.as_slice().iter().map(String::clone));
            } else if let Some(features) = &gctx.cli_unstable().build_std_features {
                build_std_features.extend(features.iter().map(String::clone));
            } else {
                build_std_features
                    .extend(["panic-unwind", "backtrace", "default"].map(String::from));
            }
        }
        // `sysroot` is not in the default set because it is optional, but it needs
        // to be part of the resolve in case we do need it for `libtest`.
        set.insert("sysroot".into());
        let specs = Packages::Packages(set.into_iter().collect());
        specs.to_package_id_specs(&std_ws)?
    };

    let build_std_features = build_std_features.into_iter().collect::<Vec<_>>();
    let cli_features = CliFeatures::from_command_line(
        &build_std_features,
        /*all_features*/ false,
        /*uses_default_features*/ false,
    )?;
    let dry_run = false;
    let resolve = ops::resolve_ws_with_opts(
        &std_ws,
        target_data,
        &build_config.requested_kinds,
        &cli_features,
        &specs,
        HasDevUnits::No,
        crate::core::resolver::features::ForceAllTargets::No,
        dry_run,
    )?;
    Ok((
        resolve.pkg_set,
        resolve.targeted_resolve,
        resolve.resolved_features,
    ))
}

/// Generates a map of root units for the standard library for each kind requested.
///
/// * `crates` is the arg value from `-Zbuild-std`.
/// * `units` is the root units of the build.
pub fn generate_std_roots(
    cli_crates: Option<&[String]>,
    units: &[Unit],
    std_resolve: &Resolve,
    std_features: &ResolvedFeatures,
    package_set: &PackageSet<'_>,
    interner: &UnitInterner,
    profiles: &Profiles,
    target_data: &RustcTargetData<'_>,
) -> CargoResult<HashMap<CompileKind, Vec<Unit>>> {
    // Generate a map of Units for each kind requested.
    let mut ret: HashMap<CompileKind, Vec<Unit>> = HashMap::new();
    // Only build libtest if it looks like it is needed (libtest depends on libstd)
    // If we know what units we're building, we can filter for libtest depending on the jobs.
    let include_libtest = units
        .iter()
        .any(|unit| unit.mode.is_rustc_test() && unit.target.harness());
    let crates = std_crates(target_data, cli_crates, include_libtest);

    let all_crates = crates
        .values()
        .flat_map(|set| set)
        .map(|s| s.as_str())
        .collect::<HashSet<_>>();
    // collect as `Vec` for stable order
    let all_crates = all_crates.into_iter().collect::<Vec<_>>();
    let std_ids = all_crates
        .iter()
        .map(|crate_name| {
            std_resolve
                .query(crate_name)
                .map(|pkg_id| (pkg_id, *crate_name))
        })
        .collect::<CargoResult<HashMap<PackageId, &str>>>()?;
    let std_pkgs = package_set.get_many(std_ids.keys().copied())?;

    // a map of the requested std crate and its actual package.
    let std_pkgs = std_pkgs
        .iter()
        .map(|pkg| (*std_ids.get(&pkg.package_id()).unwrap(), *pkg))
        .collect::<HashMap<_, _>>();

    for (&kind, crates) in &crates {
        let list = ret.entry(kind).or_default();
        for krate in crates {
            let pkg = std_pkgs.get(krate.as_str()).unwrap();
            let lib = pkg
                .targets()
                .iter()
                .find(|t| t.is_lib())
                .expect("std has a lib");
            // I don't think we need to bother with Check here, the difference
            // in time is minimal, and the difference in caching is
            // significant.
            let mode = CompileMode::Build;
            let features =
                std_features.activated_features(pkg.package_id(), FeaturesFor::NormalOrDev);
            let unit_for = UnitFor::new_normal(kind);
            let profile = profiles.get_profile(
                pkg.package_id(),
                /*is_member*/ false,
                /*is_local*/ false,
                unit_for,
                kind,
            );
            list.push(interner.intern(
                pkg,
                lib,
                profile,
                kind,
                mode,
                features.clone(),
                target_data.info(kind).rustflags.clone(),
                target_data.info(kind).rustdocflags.clone(),
                target_data.target_config(kind).links_overrides.clone(),
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
    if let Some(s) = target_data.gctx.get_env_os("__CARGO_TESTS_ONLY_SRC_ROOT") {
        return Ok(s.into());
    }

    // NOTE: This is temporary until we figure out how to acquire the source.
    let src_path = target_data
        .info(CompileKind::Host)
        .sysroot
        .join("lib")
        .join("rustlib")
        .join("src")
        .join("rust")
        .join("library");
    let lock = src_path.join("Cargo.lock");
    if !lock.exists() {
        let msg = format!(
            "{:?} does not exist, unable to build with the standard \
             library, try:\n        rustup component add rust-src",
            lock
        );
        match target_data.gctx.get_env("RUSTUP_TOOLCHAIN") {
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
