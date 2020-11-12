//! Code for building the standard library.

use crate::core::compiler::UnitInterner;
use crate::core::compiler::{CompileKind, CompileMode, RustcTargetData, Unit};
use crate::core::profiles::{Profiles, UnitFor};
use crate::core::resolver::features::{FeaturesFor, ResolvedFeatures};
use crate::core::resolver::{HasDevUnits, ResolveOpts};
use crate::core::{Dependency, PackageId, PackageSet, Resolve, SourceId, Workspace};
use crate::ops::{self, Packages};
use crate::util::errors::{CargoResult, CargoResultExt};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;

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

/// Resolve the standard library dependencies.
pub fn resolve_std<'cfg>(
    ws: &Workspace<'cfg>,
    target_data: &RustcTargetData,
    requested_targets: &[CompileKind],
    crates: &[String],
) -> CargoResult<(PackageSet<'cfg>, Resolve, ResolvedFeatures)> {
    let src_path = detect_sysroot_src_path(target_data)?;

    // Special std packages should be pulled from `library/` and should be
    // prefixed with `rustc-std-workspace-` in certain places.
    let libs_prefix = "library/";
    let special_std_prefix = "rustc-std-workspace-";
    let libs_path = src_path.join(libs_prefix);

    // Crates in rust-src to build. libtest is in some sense the "root" package
    // of std, as nothing else depends on it, so it must be explicitly added.
    let mut members = vec![format!("{}test", libs_prefix)];

    // If rust-src contains a "vendor" directory, then patch in all the crates it contains.
    let vendor_path = src_path.join("vendor");
    let vendor_dir = fs::read_dir(&vendor_path)
        .chain_err(|| format!("could not read vendor path {}", vendor_path.display()))?;
    let patches = vendor_dir
        .into_iter()
        .map(|entry| {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("package name wasn't utf8"))?;

            // Remap the rustc-std-workspace crates to the actual rust-src libraries
            let path = if let Some(real_name) = name.strip_prefix(special_std_prefix) {
                // Record this crate as something to build in the workspace
                members.push(format!("{}{}", libs_prefix, real_name));
                libs_path.join(&name)
            } else {
                entry.path()
            };
            let source_path = SourceId::for_path(&path)?;
            let dep = Dependency::parse_no_deprecated(&name, None, source_path)?;
            Ok(dep)
        })
        .collect::<CargoResult<Vec<_>>>()
        .chain_err(|| "failed to generate vendor patches")?;

    let crates_io_url = crate::sources::CRATES_IO_INDEX.parse().unwrap();
    let mut patch = HashMap::new();
    patch.insert(crates_io_url, patches);
    let ws_config = crate::core::WorkspaceConfig::Root(crate::core::WorkspaceRootConfig::new(
        &src_path,
        &Some(members),
        /*default_members*/ &None,
        /*exclude*/ &None,
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
    // Thus, in order to set the features for `std`, we need to set `libtest`
    // to be the "current" member. `libtest` is the root, and all other
    // standard library crates are dependencies from there. Since none of the
    // other crates need to alter their features, this should be fine, for
    // now. Perhaps in the future features will be decoupled from the resolver
    // and it will be easier to control feature selection.
    let current_manifest = src_path.join("library/test/Cargo.toml");
    // TODO: Consider doing something to enforce --locked? Or to prevent the
    // lock file from being written, such as setting ephemeral.
    let mut std_ws = Workspace::new_virtual(src_path, current_manifest, virtual_manifest, config)?;
    // Don't require optional dependencies in this workspace, aka std's own
    // `[dev-dependencies]`. No need for us to generate a `Resolve` which has
    // those included because we'll never use them anyway.
    std_ws.set_require_optional_deps(false);
    // `test` is not in the default set because it is optional, but it needs
    // to be part of the resolve in case we do need it.
    let mut spec_pkgs = Vec::from(crates);
    spec_pkgs.push("test".to_string());
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
    // dev_deps setting shouldn't really matter here.
    let opts = ResolveOpts::new(
        /*dev_deps*/ false, &features, /*all_features*/ false,
        /*uses_default_features*/ false,
    );
    let resolve = ops::resolve_ws_with_opts(
        &std_ws,
        target_data,
        requested_targets,
        &opts,
        &specs,
        HasDevUnits::No,
        crate::core::resolver::features::ForceAllTargets::No,
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
        let unit_for = UnitFor::new_normal();
        // I don't think we need to bother with Check here, the difference
        // in time is minimal, and the difference in caching is
        // significant.
        let mode = CompileMode::Build;
        let profile = profiles.get_profile(
            pkg.package_id(),
            /*is_member*/ false,
            /*is_local*/ false,
            unit_for,
            mode,
        );
        let features = std_features.activated_features(pkg.package_id(), FeaturesFor::NormalOrDev);

        for kind in kinds {
            let list = ret.entry(*kind).or_insert_with(Vec::new);
            list.push(interner.intern(
                pkg,
                lib,
                profile,
                *kind,
                mode,
                features.clone(),
                /*is_std*/ true,
                /*dep_hash*/ 0,
            ));
        }
    }
    Ok(ret)
}

fn detect_sysroot_src_path(target_data: &RustcTargetData) -> CargoResult<PathBuf> {
    if let Some(s) = env::var_os("__CARGO_TESTS_ONLY_SRC_ROOT") {
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
        anyhow::bail!(
            "{:?} does not exist, unable to build with the standard \
             library, try:\n        rustup component add rust-src",
            lock
        );
    }
    Ok(src_path)
}
