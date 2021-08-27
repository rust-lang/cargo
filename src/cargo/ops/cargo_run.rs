use std::collections::HashSet;
use std::ffi::OsString;
use std::iter;
use std::path::Path;

use crate::core::compiler::UnitOutput;
use crate::core::dependency::DepKind;
use crate::core::resolver::Resolve;
use crate::core::{Package, PackageIdSpec, PackageSet, TargetKind, Workspace};
use crate::ops::{self, Packages};
use crate::util::CargoResult;

pub fn run(
    ws: &Workspace<'_>,
    options: &ops::CompileOptions,
    args: &[OsString],
) -> CargoResult<()> {
    let config = ws.config();

    if options.filter.contains_glob_patterns() {
        anyhow::bail!("`cargo run` does not support glob patterns on target selection")
    }

    // We compute the `bins` here *just for diagnosis*. The actual set of
    // packages to be run is determined by the `ops::compile` call below.
    let packages = packages_eligible_to_run(ws, &options.spec)?;

    let bins: Vec<_> = packages
        .iter()
        .flat_map(|pkg| {
            iter::repeat(pkg).zip(pkg.manifest().targets().iter().filter(|target| {
                !target.is_lib()
                    && !target.is_custom_build()
                    && if !options.filter.is_specific() {
                        target.is_bin()
                    } else {
                        options.filter.target_run(target)
                    }
            }))
        })
        .collect();

    if bins.is_empty() {
        if !options.filter.is_specific() {
            anyhow::bail!("a bin target must be available for `cargo run`")
        } else {
            // This will be verified in `cargo_compile`.
        }
    }

    if bins.len() == 1 {
        let target = bins[0].1;
        if let TargetKind::ExampleLib(..) = target.kind() {
            anyhow::bail!(
                "example target `{}` is a library and cannot be executed",
                target.name()
            )
        }
    }

    if bins.len() > 1 {
        if !options.filter.is_specific() {
            let mut names: Vec<&str> = bins
                .into_iter()
                .map(|(_pkg, target)| target.name())
                .collect();
            names.sort();
            anyhow::bail!(
                "`cargo run` could not determine which binary to run. \
                 Use the `--bin` option to specify a binary, \
                 or the `default-run` manifest key.\n\
                 available binaries: {}",
                names.join(", ")
            )
        } else {
            anyhow::bail!(
                "`cargo run` can run at most one executable, but \
                 multiple were specified"
            )
        }
    }

    // `cargo run` is only compatible with one `--target` flag at most
    options.build_config.single_requested_kind()?;

    let compile = ops::compile(ws, options)?;
    assert_eq!(compile.binaries.len(), 1);
    let UnitOutput {
        unit,
        path,
        script_meta,
    } = &compile.binaries[0];
    let exe = match path.strip_prefix(config.cwd()) {
        Ok(path) if path.file_name() == Some(path.as_os_str()) => Path::new(".").join(path),
        Ok(path) => path.to_path_buf(),
        Err(_) => path.to_path_buf(),
    };
    let pkg = &bins[0].0;
    let mut process = compile.target_process(exe, unit.kind, pkg, *script_meta)?;
    process.args(args).cwd(config.cwd());

    config.shell().status("Running", process.to_string())?;

    process.exec_replace()
}

pub fn packages_eligible_to_run<'a>(
    ws: &Workspace<'a>,
    request: &Packages,
) -> CargoResult<Vec<Package>> {
    let matching_dependencies = if let ops::Packages::Packages(ref pkg_names) = request {
        let specs: HashSet<_> = pkg_names
            .into_iter()
            .flat_map(|s| PackageIdSpec::parse(s))
            .collect();

        let (package_set, resolver): (PackageSet<'a>, Resolve) = ops::resolve_ws(ws)?;

        // Restrict all direct dependencies only to build and development ones.
        // Cargo wouldn't be able to run anything after installation, so
        // normal dependencies are out.
        let direct_dependencies: Vec<_> = ws
            .members()
            .flat_map(|pkg| resolver.deps(pkg.package_id()))
            .filter(|(_, manifest_deps)| {
                manifest_deps.into_iter().any(|dep| match dep.kind() {
                    DepKind::Development | DepKind::Build => true,
                    DepKind::Normal => false,
                })
            })
            .collect();

        specs.into_iter().filter_map(|pkgidspec|
            // Either a workspace match…
            ws.members().find(|pkg| pkgidspec.matches(pkg.package_id()))
                .or_else(|| { // …or a direct dependency as fallback
                    let maybe_dep = direct_dependencies.iter().find(|(dep_pkgid, _)| pkgidspec.matches(*dep_pkgid));
                    maybe_dep.map(|(dep_pkgid, _)| package_set.get_one(*dep_pkgid).unwrap())
                })).cloned().collect()
    } else {
        request.get_packages(ws)?.into_iter().cloned().collect()
    };

    Ok(matching_dependencies)
}
