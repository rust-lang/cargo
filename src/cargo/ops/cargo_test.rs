use crate::core::compiler::{Compilation, CompileKind, Doctest, UnitOutput};
use crate::core::shell::Verbosity;
use crate::core::{TargetKind, Workspace};
use crate::ops;
use crate::util::errors::CargoResult;
use crate::util::{add_path_args, CargoTestError, Config, Test};
use cargo_util::ProcessError;
use std::ffi::OsString;

pub struct TestOptions {
    pub compile_opts: ops::CompileOptions,
    pub no_run: bool,
    pub no_fail_fast: bool,
}

pub fn run_tests(
    ws: &Workspace<'_>,
    options: &TestOptions,
    test_args: &[&str],
) -> CargoResult<Option<CargoTestError>> {
    let compilation = compile_tests(ws, options)?;

    if options.no_run {
        return Ok(None);
    }
    let (test, mut errors) = run_unit_tests(ws.config(), options, test_args, &compilation)?;

    // If we have an error and want to fail fast, then return.
    if !errors.is_empty() && !options.no_fail_fast {
        return Ok(Some(CargoTestError::new(test, errors)));
    }

    let (doctest, docerrors) = run_doc_tests(ws, options, test_args, &compilation)?;
    let test = if docerrors.is_empty() { test } else { doctest };
    errors.extend(docerrors);
    if errors.is_empty() {
        Ok(None)
    } else {
        Ok(Some(CargoTestError::new(test, errors)))
    }
}

pub fn run_benches(
    ws: &Workspace<'_>,
    options: &TestOptions,
    args: &[&str],
) -> CargoResult<Option<CargoTestError>> {
    let compilation = compile_tests(ws, options)?;

    if options.no_run {
        return Ok(None);
    }

    let mut args = args.to_vec();
    args.push("--bench");

    let (test, errors) = run_unit_tests(ws.config(), options, &args, &compilation)?;

    match errors.len() {
        0 => Ok(None),
        _ => Ok(Some(CargoTestError::new(test, errors))),
    }
}

fn compile_tests<'a>(ws: &Workspace<'a>, options: &TestOptions) -> CargoResult<Compilation<'a>> {
    let mut compilation = ops::compile(ws, &options.compile_opts)?;
    compilation.tests.sort();
    Ok(compilation)
}

/// Runs the unit and integration tests of a package.
fn run_unit_tests(
    config: &Config,
    options: &TestOptions,
    test_args: &[&str],
    compilation: &Compilation<'_>,
) -> CargoResult<(Test, Vec<ProcessError>)> {
    let cwd = config.cwd();
    let mut errors = Vec::new();

    for UnitOutput {
        unit,
        path,
        script_meta,
    } in compilation.tests.iter()
    {
        let test = unit.target.name().to_string();

        let test_path = unit.target.src_path().path().unwrap();
        let exe_display = if let TargetKind::Test = unit.target.kind() {
            format!(
                "{} ({})",
                test_path
                    .strip_prefix(unit.pkg.root())
                    .unwrap_or(test_path)
                    .display(),
                path.strip_prefix(cwd).unwrap_or(path).display()
            )
        } else {
            format!(
                "unittests ({})",
                path.strip_prefix(cwd).unwrap_or(path).display()
            )
        };

        let mut cmd = compilation.target_process(path, unit.kind, &unit.pkg, *script_meta)?;
        cmd.args(test_args);
        if unit.target.harness() && config.shell().verbosity() == Verbosity::Quiet {
            cmd.arg("--quiet");
        }
        config
            .shell()
            .concise(|shell| shell.status("Running", &exe_display))?;
        config
            .shell()
            .verbose(|shell| shell.status("Running", &cmd))?;

        let result = cmd.exec();

        match result {
            Err(e) => {
                let e = e.downcast::<ProcessError>()?;
                errors.push((
                    unit.target.kind().clone(),
                    test.clone(),
                    unit.pkg.name().to_string(),
                    e,
                ));
                if !options.no_fail_fast {
                    break;
                }
            }
            Ok(()) => {}
        }
    }

    if errors.len() == 1 {
        let (kind, name, pkg_name, e) = errors.pop().unwrap();
        Ok((
            Test::UnitTest {
                kind,
                name,
                pkg_name,
            },
            vec![e],
        ))
    } else {
        Ok((
            Test::Multiple,
            errors.into_iter().map(|(_, _, _, e)| e).collect(),
        ))
    }
}

fn run_doc_tests(
    ws: &Workspace<'_>,
    options: &TestOptions,
    test_args: &[&str],
    compilation: &Compilation<'_>,
) -> CargoResult<(Test, Vec<ProcessError>)> {
    let config = ws.config();
    let mut errors = Vec::new();
    let doctest_xcompile = config.cli_unstable().doctest_xcompile;
    let doctest_in_workspace = config.cli_unstable().doctest_in_workspace;

    for doctest_info in &compilation.to_doc_test {
        let Doctest { unit, linker, .. } = doctest_info;

        if !doctest_xcompile {
            match unit.kind {
                CompileKind::Host => {}
                CompileKind::Target(target) => {
                    if target.short_name() != compilation.host {
                        // Skip doctests, -Zdoctest-xcompile not enabled.
                        continue;
                    }
                }
            }
        }

        config.shell().status("Doc-tests", unit.target.name())?;
        let mut p = doctest_info.rustdoc_process(compilation)?;

        if doctest_in_workspace {
            add_path_args(ws, unit, &mut p);
            // FIXME(swatinem): remove the `unstable-options` once rustdoc stabilizes the `test-run-directory` option
            p.arg("-Z").arg("unstable-options");
            p.arg("--test-run-directory")
                .arg(unit.pkg.root().to_path_buf());
        } else {
            p.arg(unit.target.src_path().path().unwrap());
        }

        if doctest_xcompile {
            if let CompileKind::Target(target) = unit.kind {
                // use `rustc_target()` to properly handle JSON target paths
                p.arg("--target").arg(target.rustc_target());
            }
            p.arg("-Zunstable-options");
            p.arg("--enable-per-target-ignores");
            if let Some((runtool, runtool_args)) = compilation.target_runner(unit.kind) {
                p.arg("--runtool").arg(runtool);
                for arg in runtool_args {
                    p.arg("--runtool-arg").arg(arg);
                }
            }
            if let Some(linker) = linker {
                let mut joined = OsString::from("linker=");
                joined.push(linker);
                p.arg("-C").arg(joined);
            }
        }

        for arg in test_args {
            p.arg("--test-args").arg(arg);
        }

        config
            .shell()
            .verbose(|shell| shell.status("Running", p.to_string()))?;
        if let Err(e) = p.exec() {
            let e = e.downcast::<ProcessError>()?;
            errors.push(e);
            if !options.no_fail_fast {
                return Ok((Test::Doc, errors));
            }
        }
    }
    Ok((Test::Doc, errors))
}
