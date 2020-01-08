use crate::core::{Target, Workspace};
use crate::ops::CompileOptions;
use crate::util::CargoResult;
use anyhow::bail;
use std::fmt::Write;

fn get_available_targets<'a>(
    filter_fn: fn(&Target) -> bool,
    ws: &'a Workspace<'_>,
    options: &'a CompileOptions<'_>,
) -> CargoResult<Vec<&'a Target>> {
    let packages = options.spec.get_packages(ws)?;

    let mut targets: Vec<_> = packages
        .into_iter()
        .flat_map(|pkg| {
            pkg.manifest()
                .targets()
                .iter()
                .filter(|target| filter_fn(target))
        })
        .collect();

    targets.sort();

    Ok(targets)
}

fn print_available(
    filter_fn: fn(&Target) -> bool,
    ws: &Workspace<'_>,
    options: &CompileOptions<'_>,
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
        for target in targets {
            writeln!(output, "    {}", target.name())?;
        }
    }
    bail!("{}", output)
}

pub fn print_available_examples(
    ws: &Workspace<'_>,
    options: &CompileOptions<'_>,
) -> CargoResult<()> {
    print_available(Target::is_example, ws, options, "--example", "examples")
}

pub fn print_available_binaries(
    ws: &Workspace<'_>,
    options: &CompileOptions<'_>,
) -> CargoResult<()> {
    print_available(Target::is_bin, ws, options, "--bin", "binaries")
}

pub fn print_available_benches(
    ws: &Workspace<'_>,
    options: &CompileOptions<'_>,
) -> CargoResult<()> {
    print_available(Target::is_bench, ws, options, "--bench", "benches")
}

pub fn print_available_tests(ws: &Workspace<'_>, options: &CompileOptions<'_>) -> CargoResult<()> {
    print_available(Target::is_test, ws, options, "--test", "tests")
}
