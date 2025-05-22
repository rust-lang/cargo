use crate::core::compiler::Unit;
use crate::core::manifest::TargetSourcePath;
use crate::core::{Target, Workspace};
use crate::ops::CompileOptions;
use crate::util::CargoResult;
use anyhow::bail;
use cargo_util::paths::normalize_path;
use cargo_util::ProcessBuilder;
use std::fmt::Write;
use std::path::Path;
use std::path::PathBuf;

const ITEM_INDENT: &str = "    ";

fn get_available_targets<'a>(
    filter_fn: fn(&Target) -> bool,
    ws: &'a Workspace<'_>,
    options: &'a CompileOptions,
) -> CargoResult<Vec<(&'a str, &'a Path)>> {
    let packages = options.spec.get_packages(ws)?;

    let mut targets: Vec<_> = packages
        .into_iter()
        .flat_map(|pkg| {
            pkg.manifest()
                .targets()
                .iter()
                .filter(|target| filter_fn(target))
        })
        .map(|target| {
            (
                target.name(),
                target.src_path().path().expect("Target is not a `Metabuild` but one of `Bin` | `Test` | `Bench` | `ExampleBin`")
            )
        })
        .collect();

    targets.sort();

    Ok(targets)
}

fn print_available_targets(
    filter_fn: fn(&Target) -> bool,
    ws: &Workspace<'_>,
    options: &CompileOptions,
    option_name: &str,
    plural_name: &str,
) -> CargoResult<()> {
    let targets = get_available_targets(filter_fn, ws, options)?;

    let mut output = String::new();
    writeln!(output, "\"{}\" takes one argument.", option_name)?;

    if targets.is_empty() {
        writeln!(output, "No {} available.", plural_name)?;
    } else {
        writeln!(output, "Available {}:", plural_name)?;
        let mut shell = ws.gctx().shell();
        for (name, src_path) in targets {
            let link = shell.err_file_hyperlink(src_path);
            writeln!(output, "{ITEM_INDENT}{link}{}{link:#}", name)?;
        }
    }
    bail!("{}", output)
}

pub fn print_available_packages(ws: &Workspace<'_>) -> CargoResult<()> {
    let packages = ws
        .members()
        .map(|pkg| (pkg.name().as_str(), pkg.manifest_path()))
        .collect::<Vec<_>>();

    let mut output = "\"--package <SPEC>\" requires a SPEC format value, \
        which can be any package ID specifier in the dependency graph.\n\
        Run `cargo help pkgid` for more information about SPEC format.\n\n"
        .to_string();

    if packages.is_empty() {
        // This would never happen.
        // Just in case something regresses we covers it here.
        writeln!(output, "No packages available.")?;
    } else {
        writeln!(output, "Possible packages/workspace members:")?;
        let mut shell = ws.gctx().shell();
        for (name, manifest_path) in packages {
            let link = shell.err_file_hyperlink(manifest_path);
            writeln!(output, "{ITEM_INDENT}{link}{}{link:#}", name)?;
        }
    }
    bail!("{}", output)
}

pub fn print_available_examples(ws: &Workspace<'_>, options: &CompileOptions) -> CargoResult<()> {
    print_available_targets(Target::is_example, ws, options, "--example", "examples")
}

pub fn print_available_binaries(ws: &Workspace<'_>, options: &CompileOptions) -> CargoResult<()> {
    print_available_targets(Target::is_bin, ws, options, "--bin", "binaries")
}

pub fn print_available_benches(ws: &Workspace<'_>, options: &CompileOptions) -> CargoResult<()> {
    print_available_targets(Target::is_bench, ws, options, "--bench", "bench targets")
}

pub fn print_available_tests(ws: &Workspace<'_>, options: &CompileOptions) -> CargoResult<()> {
    print_available_targets(Target::is_test, ws, options, "--test", "test targets")
}

/// The source path and its current dir for use in compilation.
///
/// The path that we pass to rustc is actually fairly important because it will
/// show up in error messages (important for readability), debug information
/// (important for caching), etc. As a result we need to be pretty careful how we
/// actually invoke rustc.
///
/// In general users don't expect `cargo build` to cause rebuilds if you change
/// directories. That could be if you just change directories in the package or
/// if you literally move the whole package wholesale to a new directory. As a
/// result we mostly don't factor in `cwd` to this calculation. Instead we try to
/// track the workspace as much as possible and we update the current directory
/// of rustc/rustdoc where appropriate.
///
/// The first returned value here is the argument to pass to rustc, and the
/// second is the cwd that rustc should operate in.
pub fn path_args(ws: &Workspace<'_>, unit: &Unit) -> (PathBuf, PathBuf) {
    let src = match unit.target.src_path() {
        TargetSourcePath::Path(path) => path.to_path_buf(),
        TargetSourcePath::Metabuild => unit.pkg.manifest().metabuild_path(ws.build_dir()),
    };
    assert!(src.is_absolute());
    if unit.pkg.package_id().source_id().is_path() {
        // Determine which path we make this relative to: usually it's the workspace root,
        // but this can be overwritten with a `-Z` flag.
        let root = match &ws.gctx().cli_unstable().root_dir {
            None => ws.root().to_owned(),
            Some(root_dir) => normalize_path(&ws.gctx().cwd().join(root_dir)),
        };
        if let Ok(path) = src.strip_prefix(&root) {
            return (path.to_path_buf(), root);
        }
    }
    (src, unit.pkg.root().to_path_buf())
}

pub fn add_path_args(ws: &Workspace<'_>, unit: &Unit, cmd: &mut ProcessBuilder) {
    let (arg, cwd) = path_args(ws, unit);
    cmd.arg(arg);
    cmd.cwd(cwd);
}
