//! Code for building the standard library.

use super::layout::Layout;
use crate::core::compiler::{BuildContext, CompileKind, CompileMode, Context, FileFlavor, Unit};
use crate::core::profiles::UnitFor;
use crate::core::resolver::ResolveOpts;
use crate::core::{Dependency, PackageId, PackageSet, Resolve, SourceId, Workspace};
use crate::ops::{self, Packages};
use crate::util::errors::CargoResult;
use crate::util::paths;
use std::collections::{HashMap, HashSet};
use std::env;
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
    crates: &[String],
) -> CargoResult<(PackageSet<'cfg>, Resolve)> {
    let src_path = detect_sysroot_src_path(ws)?;
    let to_patch = [
        "rustc-std-workspace-core",
        "rustc-std-workspace-alloc",
        "rustc-std-workspace-std",
    ];
    let patches = to_patch
        .iter()
        .map(|&name| {
            let source_path = SourceId::for_path(&src_path.join("src").join("tools").join(name))?;
            let dep = Dependency::parse_no_deprecated(name, None, source_path)?;
            Ok(dep)
        })
        .collect::<CargoResult<Vec<_>>>()?;
    let crates_io_url = crate::sources::CRATES_IO_INDEX.parse().unwrap();
    let mut patch = HashMap::new();
    patch.insert(crates_io_url, patches);
    let members = vec![
        String::from("src/libstd"),
        String::from("src/libcore"),
        String::from("src/liballoc"),
        String::from("src/libtest"),
    ];
    let ws_config = crate::core::WorkspaceConfig::Root(crate::core::WorkspaceRootConfig::new(
        &src_path,
        &Some(members),
        /*default_members*/ &None,
        /*exclude*/ &None,
    ));
    let virtual_manifest = crate::core::VirtualManifest::new(
        /*replace*/ Vec::new(),
        patch,
        ws_config,
        // Profiles are not used here, but we need something to pass in.
        ws.profiles().clone(),
        crate::core::Features::default(),
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
    let current_manifest = src_path.join("src/libtest/Cargo.toml");
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
    let features = vec!["panic-unwind".to_string(), "backtrace".to_string()];
    // dev_deps setting shouldn't really matter here.
    let opts = ResolveOpts::new(
        /*dev_deps*/ false, &features, /*all_features*/ false,
        /*uses_default_features*/ true,
    );
    let resolve = ops::resolve_ws_with_opts(&std_ws, opts, &specs)?;
    Ok((resolve.pkg_set, resolve.targeted_resolve))
}

/// Generate a list of root `Unit`s for the standard library.
///
/// The given slice of crate names is the root set.
pub fn generate_std_roots<'a>(
    bcx: &BuildContext<'a, '_>,
    crates: &[String],
    std_resolve: &'a Resolve,
    kind: CompileKind,
) -> CargoResult<Vec<Unit<'a>>> {
    // Generate the root Units for the standard library.
    let std_ids = crates
        .iter()
        .map(|crate_name| std_resolve.query(crate_name))
        .collect::<CargoResult<Vec<PackageId>>>()?;
    // Convert PackageId to Package.
    let std_pkgs = bcx.packages.get_many(std_ids)?;
    // Generate a list of Units.
    std_pkgs
        .into_iter()
        .map(|pkg| {
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
            let profile = bcx.profiles.get_profile(
                pkg.package_id(),
                /*is_member*/ false,
                unit_for,
                mode,
                bcx.build_config.profile_kind.clone(),
            );
            let features = std_resolve.features_sorted(pkg.package_id());
            Ok(bcx.units.intern(
                pkg, lib, profile, kind, mode, features, /*is_std*/ true,
            ))
        })
        .collect::<CargoResult<Vec<_>>>()
}

fn detect_sysroot_src_path(ws: &Workspace<'_>) -> CargoResult<PathBuf> {
    if let Some(s) = env::var_os("__CARGO_TESTS_ONLY_SRC_ROOT") {
        return Ok(s.into());
    }

    // NOTE: This is temporary until we figure out how to acquire the source.
    // If we decide to keep the sysroot probe, then BuildConfig will need to
    // be restructured so that the TargetInfo is created earlier and passed
    // in, so we don't have this extra call to rustc.
    let rustc = ws.config().load_global_rustc(Some(ws))?;
    let output = rustc.process().arg("--print=sysroot").exec_with_output()?;
    let s = String::from_utf8(output.stdout)
        .map_err(|e| failure::format_err!("rustc didn't return utf8 output: {:?}", e))?;
    let sysroot = PathBuf::from(s.trim());
    let src_path = sysroot.join("lib").join("rustlib").join("src").join("rust");
    let lock = src_path.join("Cargo.lock");
    if !lock.exists() {
        failure::bail!(
            "{:?} does not exist, unable to build with the standard \
             library, try:\n        rustup component add rust-src",
            lock
        );
    }
    Ok(src_path)
}

/// Prepare the output directory for the local sysroot.
pub fn prepare_sysroot(layout: &Layout) -> CargoResult<()> {
    if let Some(libdir) = layout.sysroot_libdir() {
        if libdir.exists() {
            paths::remove_dir_all(libdir)?;
        }
        paths::create_dir_all(libdir)?;
    }
    Ok(())
}

/// Copy an artifact to the sysroot.
pub fn add_sysroot_artifact<'a>(
    cx: &Context<'a, '_>,
    unit: &Unit<'a>,
    rmeta: bool,
) -> CargoResult<()> {
    let outputs = cx.outputs(unit)?;
    let outputs = outputs
        .iter()
        .filter(|output| output.flavor == FileFlavor::Linkable { rmeta })
        .map(|output| &output.path);
    for path in outputs {
        let libdir = cx.files().layout(unit.kind).sysroot_libdir().unwrap();
        let dst = libdir.join(path.file_name().unwrap());
        paths::link_or_copy(path, dst)?;
    }
    Ok(())
}
