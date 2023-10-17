use crate::core::compiler::{Compilation, CompileKind, Doctest, Metadata, Unit, UnitOutput};
use crate::core::shell::Verbosity;
use crate::core::{TargetKind, Workspace};
use crate::ops;
use crate::util::errors::CargoResult;
use crate::util::{add_path_args, CliError, CliResult, Config};
use anyhow::format_err;
use cargo_util::{ProcessBuilder, ProcessError};
use std::ffi::OsString;
use std::fmt::Write;
use std::path::{Path, PathBuf};

pub struct TestOptions {
    pub compile_opts: ops::CompileOptions,
    pub no_run: bool,
    pub no_fail_fast: bool,
}

/// The kind of test.
///
/// This is needed because `Unit` does not track whether or not something is a
/// benchmark.
#[derive(Copy, Clone)]
enum TestKind {
    Test,
    Bench,
    Doctest,
}

/// A unit that failed to run.
struct UnitTestError {
    unit: Unit,
    kind: TestKind,
}

impl UnitTestError {
    /// Returns the CLI args needed to target this unit.
    fn cli_args(&self, ws: &Workspace<'_>, opts: &ops::CompileOptions) -> String {
        let mut args = if opts.spec.needs_spec_flag(ws) {
            format!("-p {} ", self.unit.pkg.name())
        } else {
            String::new()
        };
        let mut add = |which| write!(args, "--{which} {}", self.unit.target.name()).unwrap();

        match self.kind {
            TestKind::Test | TestKind::Bench => match self.unit.target.kind() {
                TargetKind::Lib(_) => args.push_str("--lib"),
                TargetKind::Bin => add("bin"),
                TargetKind::Test => add("test"),
                TargetKind::Bench => add("bench"),
                TargetKind::ExampleLib(_) | TargetKind::ExampleBin => add("example"),
                TargetKind::CustomBuild => panic!("unexpected CustomBuild kind"),
            },
            TestKind::Doctest => args.push_str("--doc"),
        }
        args
    }
}

/// Compiles and runs tests.
///
/// On error, the returned [`CliError`] will have the appropriate process exit
/// code that Cargo should use.
pub fn run_tests(ws: &Workspace<'_>, options: &TestOptions, test_args: &[&str]) -> CliResult {
    let compilation = compile_tests(ws, options)?;

    if options.no_run {
        if !options.compile_opts.build_config.emit_json() {
            display_no_run_information(ws, test_args, &compilation, "unittests")?;
        }
        return Ok(());
    }
    let mut errors = run_unit_tests(ws, options, test_args, &compilation, TestKind::Test)?;

    let doctest_errors = run_doc_tests(ws, options, test_args, &compilation)?;
    errors.extend(doctest_errors);
    no_fail_fast_err(ws, &options.compile_opts, &errors)
}

/// Compiles and runs benchmarks.
///
/// On error, the returned [`CliError`] will have the appropriate process exit
/// code that Cargo should use.
pub fn run_benches(ws: &Workspace<'_>, options: &TestOptions, args: &[&str]) -> CliResult {
    let compilation = compile_tests(ws, options)?;

    if options.no_run {
        if !options.compile_opts.build_config.emit_json() {
            display_no_run_information(ws, args, &compilation, "benches")?;
        }
        return Ok(());
    }

    let mut args = args.to_vec();
    args.push("--bench");

    let errors = run_unit_tests(ws, options, &args, &compilation, TestKind::Bench)?;
    no_fail_fast_err(ws, &options.compile_opts, &errors)
}

fn compile_tests<'a>(ws: &Workspace<'a>, options: &TestOptions) -> CargoResult<Compilation<'a>> {
    let mut compilation = ops::compile(ws, &options.compile_opts)?;
    compilation.tests.sort();
    Ok(compilation)
}

/// Runs the unit and integration tests of a package.
///
/// Returns a `Vec` of tests that failed when `--no-fail-fast` is used.
/// If `--no-fail-fast` is *not* used, then this returns an `Err`.
fn run_unit_tests(
    ws: &Workspace<'_>,
    options: &TestOptions,
    test_args: &[&str],
    compilation: &Compilation<'_>,
    test_kind: TestKind,
) -> Result<Vec<UnitTestError>, CliError> {
    let config = ws.config();
    let cwd = config.cwd();
    let mut errors = Vec::new();

    for UnitOutput {
        unit,
        path,
        script_meta,
    } in compilation.tests.iter()
    {
        let (exe_display, mut cmd) = cmd_builds(
            config,
            cwd,
            unit,
            path,
            script_meta,
            test_args,
            compilation,
            "unittests",
        )?;

        if config.extra_verbose() {
            cmd.display_env_vars();
        }

        config
            .shell()
            .concise(|shell| shell.status("Running", &exe_display))?;
        config
            .shell()
            .verbose(|shell| shell.status("Running", &cmd))?;

        if let Err(e) = cmd.exec() {
            let code = fail_fast_code(&e);
            let unit_err = UnitTestError {
                unit: unit.clone(),
                kind: test_kind,
            };
            report_test_error(ws, test_args, &options.compile_opts, &unit_err, e);
            errors.push(unit_err);
            if !options.no_fail_fast {
                return Err(CliError::code(code));
            }
        }
    }
    Ok(errors)
}

/// Runs doc tests.
///
/// Returns a `Vec` of tests that failed when `--no-fail-fast` is used.
/// If `--no-fail-fast` is *not* used, then this returns an `Err`.
fn run_doc_tests(
    ws: &Workspace<'_>,
    options: &TestOptions,
    test_args: &[&str],
    compilation: &Compilation<'_>,
) -> Result<Vec<UnitTestError>, CliError> {
    let config = ws.config();
    let mut errors = Vec::new();
    let doctest_xcompile = config.cli_unstable().doctest_xcompile;

    for doctest_info in &compilation.to_doc_test {
        let Doctest {
            args,
            unstable_opts,
            unit,
            linker,
            script_meta,
            env,
        } = doctest_info;

        if !doctest_xcompile {
            match unit.kind {
                CompileKind::Host => {}
                CompileKind::Target(target) => {
                    if target.short_name() != compilation.host {
                        // Skip doctests, -Zdoctest-xcompile not enabled.
                        config.shell().verbose(|shell| {
                            shell.note(format!(
                                "skipping doctests for {} ({}), \
                                 cross-compilation doctests are not yet supported\n\
                                 See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#doctest-xcompile \
                                 for more information.",
                                unit.pkg,
                                unit.target.description_named()
                            ))
                        })?;
                        continue;
                    }
                }
            }
        }

        config.shell().status("Doc-tests", unit.target.name())?;
        let mut p = compilation.rustdoc_process(unit, *script_meta)?;

        for (var, value) in env {
            p.env(var, value);
        }
        p.arg("--crate-name").arg(&unit.target.crate_name());
        p.arg("--test");

        add_path_args(ws, unit, &mut p);
        p.arg("--test-run-directory")
            .arg(unit.pkg.root().to_path_buf());

        if let CompileKind::Target(target) = unit.kind {
            // use `rustc_target()` to properly handle JSON target paths
            p.arg("--target").arg(target.rustc_target());
        }

        if doctest_xcompile {
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

        p.args(unit.pkg.manifest().lint_rustflags());

        p.args(args);

        if *unstable_opts {
            p.arg("-Zunstable-options");
        }

        if config.extra_verbose() {
            p.display_env_vars();
        }

        config
            .shell()
            .verbose(|shell| shell.status("Running", p.to_string()))?;

        if let Err(e) = p.exec() {
            let code = fail_fast_code(&e);
            let unit_err = UnitTestError {
                unit: unit.clone(),
                kind: TestKind::Doctest,
            };
            report_test_error(ws, test_args, &options.compile_opts, &unit_err, e);
            errors.push(unit_err);
            if !options.no_fail_fast {
                return Err(CliError::code(code));
            }
        }
    }
    Ok(errors)
}

/// Displays human-readable descriptions of the test executables.
///
/// This is used when `cargo test --no-run` is used.
fn display_no_run_information(
    ws: &Workspace<'_>,
    test_args: &[&str],
    compilation: &Compilation<'_>,
    exec_type: &str,
) -> CargoResult<()> {
    let config = ws.config();
    let cwd = config.cwd();
    for UnitOutput {
        unit,
        path,
        script_meta,
    } in compilation.tests.iter()
    {
        let (exe_display, cmd) = cmd_builds(
            config,
            cwd,
            unit,
            path,
            script_meta,
            test_args,
            compilation,
            exec_type,
        )?;
        config
            .shell()
            .concise(|shell| shell.status("Executable", &exe_display))?;
        config
            .shell()
            .verbose(|shell| shell.status("Executable", &cmd))?;
    }

    return Ok(());
}

/// Creates a [`ProcessBuilder`] for executing a single test.
///
/// Returns a tuple `(exe_display, process)` where `exe_display` is a string
/// to display that describes the executable path in a human-readable form.
/// `process` is the `ProcessBuilder` to use for executing the test.
fn cmd_builds(
    config: &Config,
    cwd: &Path,
    unit: &Unit,
    path: &PathBuf,
    script_meta: &Option<Metadata>,
    test_args: &[&str],
    compilation: &Compilation<'_>,
    exec_type: &str,
) -> CargoResult<(String, ProcessBuilder)> {
    let test_path = unit.target.src_path().path().unwrap();
    let short_test_path = test_path
        .strip_prefix(unit.pkg.root())
        .unwrap_or(test_path)
        .display();

    let exe_display = match unit.target.kind() {
        TargetKind::Test | TargetKind::Bench => format!(
            "{} ({})",
            short_test_path,
            path.strip_prefix(cwd).unwrap_or(path).display()
        ),
        _ => format!(
            "{} {} ({})",
            exec_type,
            short_test_path,
            path.strip_prefix(cwd).unwrap_or(path).display()
        ),
    };

    let mut cmd = compilation.target_process(path, unit.kind, &unit.pkg, *script_meta)?;
    cmd.args(test_args);
    if unit.target.harness() && config.shell().verbosity() == Verbosity::Quiet {
        cmd.arg("--quiet");
    }

    Ok((exe_display, cmd))
}

/// Returns the error code to use when *not* using `--no-fail-fast`.
///
/// Cargo will return the error code from the test process itself. If some
/// other error happened (like a failure to launch the process), then it will
/// return a standard 101 error code.
///
/// When using `--no-fail-fast`, Cargo always uses the 101 exit code (since
/// there may not be just one process to report).
fn fail_fast_code(error: &anyhow::Error) -> i32 {
    if let Some(proc_err) = error.downcast_ref::<ProcessError>() {
        if let Some(code) = proc_err.code {
            return code;
        }
    }
    101
}

/// Returns the `CliError` when using `--no-fail-fast` and there is at least
/// one error.
fn no_fail_fast_err(
    ws: &Workspace<'_>,
    opts: &ops::CompileOptions,
    errors: &[UnitTestError],
) -> CliResult {
    // TODO: This could be improved by combining the flags on a single line when feasible.
    let args: Vec<_> = errors
        .iter()
        .map(|unit_err| format!("    `{}`", unit_err.cli_args(ws, opts)))
        .collect();
    let message = match errors.len() {
        0 => return Ok(()),
        1 => format!("1 target failed:\n{}", args.join("\n")),
        n => format!("{n} targets failed:\n{}", args.join("\n")),
    };
    Err(anyhow::Error::msg(message).into())
}

/// Displays an error on the console about a test failure.
fn report_test_error(
    ws: &Workspace<'_>,
    test_args: &[&str],
    opts: &ops::CompileOptions,
    unit_err: &UnitTestError,
    test_error: anyhow::Error,
) {
    let which = match unit_err.kind {
        TestKind::Test => "test failed",
        TestKind::Bench => "bench failed",
        TestKind::Doctest => "doctest failed",
    };

    let mut err = format_err!("{}, to rerun pass `{}`", which, unit_err.cli_args(ws, opts));
    // Don't show "process didn't exit successfully" for simple errors.
    // libtest exits with 101 for normal errors.
    let (is_simple, executed) = test_error
        .downcast_ref::<ProcessError>()
        .and_then(|proc_err| proc_err.code)
        .map_or((false, false), |code| (code == 101, true));

    if !is_simple {
        err = test_error.context(err);
    }

    crate::display_error(&err, &mut ws.config().shell());

    let harness: bool = unit_err.unit.target.harness();
    let nocapture: bool = test_args.contains(&"--nocapture");

    if !is_simple && executed && harness && !nocapture {
        drop(ws.config().shell().note(
            "test exited abnormally; to see the full output pass --nocapture to the harness.",
        ));
    }
}
