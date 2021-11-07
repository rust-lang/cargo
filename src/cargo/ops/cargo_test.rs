use crate::core::compiler::{Compilation, CompileKind, Doctest, UnitOutput};
use crate::core::shell::Verbosity;
use crate::core::{TargetKind, Workspace};
use crate::ops;
use crate::util::errors::CargoResult;
use crate::util::{add_path_args, CargoTestError, Config, Test};
use cargo_util::ProcessError;
use crossbeam_utils::thread::{self, ScopedJoinHandle};
use std::ffi::OsString;
use std::fmt::Write;

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

enum OutOrErr {
    Out(String),
    Err(String),
}

struct Context {
    exe: String,
    cmd: String,
}

type TestError = (TargetKind, String, String, anyhow::Error);

fn run_unit_tests(
    config: &Config,
    options: &TestOptions,
    test_args: &[&str],
    compilation: &Compilation<'_>,
) -> CargoResult<(Test, Vec<ProcessError>)> {
    let cwd = config.cwd();
    let mut errors: Vec<TestError> = Vec::new();

    thread::scope(|s| {
        let mut handles = vec![];
        let parallel = std::env::var("CARGO_TEST_PARALLEL")
            .unwrap_or("TRUE".into())
            .to_uppercase()
            .eq("TRUE");

        for UnitOutput {
            unit,
            path,
            script_meta,
        } in compilation.tests.iter()
        {
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

            let mut cmd = compilation.target_process(&path, unit.kind, &unit.pkg, *script_meta)?;
            cmd.args(test_args);
            if unit.target.harness() && config.shell().verbosity() == Verbosity::Quiet {
                cmd.arg("--quiet");
            }

            let mut ctx = Context {
                exe: exe_display,
                cmd: String::new(),
            };
            write!(ctx.cmd, "{}", &cmd).unwrap();

            let pkg_name = unit.pkg.name().to_string();
            let target = &unit.target;
            let (tx, rx) = std::sync::mpsc::channel();
            let handle = s.spawn(move |_| {
                cmd.exec_with_streaming(
                    &mut |line| {
                        tx.send(OutOrErr::Out(line.to_string())).unwrap();
                        Ok(())
                    },
                    &mut |line| {
                        tx.send(OutOrErr::Err(line.to_string())).unwrap();
                        Ok(())
                    },
                    false,
                )
                .map_err(|e| {
                    (
                        target.kind().clone(),
                        target.name().to_string(),
                        pkg_name,
                        e,
                    )
                })
            });
            if parallel {
                handles.push((handle, rx, ctx));
            } else if !process_output(handle, &mut errors, ctx, config, options, rx)? {
                break;
            }
        }

        for (handle, rx, ctx) in handles {
            if !process_output(handle, &mut errors, ctx, config, options, rx)? {
                break;
            }
        }
        let out: Result<(), anyhow::Error> = Ok(());
        out
    })
    .unwrap()?;

    if errors.len() == 1 {
        let (kind, name, pkg_name, e) = errors.pop().unwrap();
        Ok((
            Test::UnitTest {
                kind,
                name,
                pkg_name,
            },
            vec![e.downcast::<ProcessError>()?],
        ))
    } else {
        let mut res = vec![];
        for (_, _, _, e) in errors.into_iter() {
            res.push(e.downcast::<ProcessError>()?);
        }
        Ok((Test::Multiple, res))
    }
}

/// Puts test output on the sceen.
/// Returns false if we should early exit due to test failures.
fn process_output<'scope>(
    handle: ScopedJoinHandle<'scope, Result<std::process::Output, TestError>>,
    errors: &mut Vec<TestError>,
    ctx: Context,
    config: &Config,
    options: &TestOptions,
    rx: std::sync::mpsc::Receiver<OutOrErr>,
) -> CargoResult<bool> {
    let result = handle.join().unwrap();
    config
        .shell()
        .concise(|shell| shell.status("Running", &ctx.exe))?;
    config
        .shell()
        .verbose(|shell| shell.status("Running", &ctx.cmd))?;

    for line in &rx {
        match line {
            OutOrErr::Out(line) => writeln!(config.shell().out(), "{}", line).unwrap(),
            OutOrErr::Err(line) => writeln!(config.shell().err(), "{}", line).unwrap(),
        }
    }
    if let Err(err) = result {
        errors.push(err);
        if !options.no_fail_fast {
            return Ok(false);
        }
    }
    Ok(true)
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
        let Doctest {
            args,
            unstable_opts,
            unit,
            linker,
            script_meta,
        } = doctest_info;

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
        let mut p = compilation.rustdoc_process(unit, *script_meta)?;
        p.arg("--crate-name").arg(&unit.target.crate_name());
        p.arg("--test");

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

        for &rust_dep in &[
            &compilation.deps_output[&unit.kind],
            &compilation.deps_output[&CompileKind::Host],
        ] {
            let mut arg = OsString::from("dependency=");
            arg.push(rust_dep);
            p.arg("-L").arg(arg);
        }

        for native_dep in compilation.native_dirs.iter() {
            p.arg("-L").arg(native_dep);
        }

        for arg in test_args {
            p.arg("--test-args").arg(arg);
        }

        if config.shell().verbosity() == Verbosity::Quiet {
            p.arg("--test-args").arg("--quiet");
        }

        p.args(args);

        if *unstable_opts {
            p.arg("-Zunstable-options");
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
