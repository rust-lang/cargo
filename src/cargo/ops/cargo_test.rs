// Mod this command to use the new program
pub mod mutation_iabr;
pub mod ast_iabr;
pub mod mutators;


use crate::core::compiler::{Compilation, CompileKind, Doctest, Unit, UnitHash, UnitOutput};
use crate::core::profiles::PanicStrategy;
use crate::core::shell::ColorChoice;
use crate::core::shell::Verbosity;
use crate::core::{TargetKind, Workspace};
use crate::ops;
use crate::util::errors::CargoResult;
use serde::Serialize;
use crate::util::{CliError, CliResult, GlobalContext, add_path_args};
use anyhow::format_err;
use cargo_util::{ProcessBuilder, ProcessError};
use std::ffi::OsString;
use std::fmt::Write;
use std::io::{self, Write as IoWrite};
use std::time::Instant;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct TestOptions {
    pub compile_opts: ops::CompileOptions,
    pub no_run: bool,
    pub no_fail_fast: bool,
    pub mutation: bool,
    pub mutation_long: bool,
    pub mutation_json: bool,
    pub mutation_json_dir: Option<PathBuf>,
}

impl Clone for TestOptions {
    fn clone(&self) -> Self {
        Self {
            compile_opts: self.compile_opts.clone(),
            no_run: self.no_run,
            no_fail_fast: self.no_fail_fast,
            mutation: self.mutation,
            mutation_long: self.mutation_long,
            mutation_json: self.mutation_json,
            mutation_json_dir: self.mutation_json_dir.clone(),
        }
    }
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
    // Mutation flow: do not pre-compile; each mutation compiles separately.
    if options.mutation {
        return run_mutation_campaign(ws, options, test_args);
    }

    let compilation = compile_tests(ws, options)?;

    if options.no_run {
        if !options.compile_opts.build_config.emit_json() {
            display_no_run_information(ws, test_args, &compilation, "unittests")?;
        }
        return Ok(());
    }
    let mut errors = run_unit_tests(ws, options, test_args, &compilation, TestKind::Test, false)?;

    let doctest_errors = run_doc_tests(ws, options, test_args, &compilation, false)?;
    errors.extend(doctest_errors);
    no_fail_fast_err(ws, &options.compile_opts, &errors)
}

// Runs the full test suite once (compile + unit + doctest) and returns
// Ok(true) if tests passed, Ok(false) if tests failed; Err for infra errors.
fn run_tests_once(ws: &Workspace<'_>, options: &TestOptions, test_args: &[&str], quiet: bool) -> CargoResult<bool> {
    let _guard = if quiet { Some(ShellVerbosityGuard::set(ws.gctx(), Verbosity::Quiet)) } else { None };
    let compilation = compile_tests(ws, options)?;

    if options.mutation_long
    {
        for test in &compilation.tests 
        {
            eprintln!("[DEBUG] Compiling test binary for: {}", test.unit.target.name());
            eprintln!("[DEBUG] Path: {}", test.path.display());
        }
    }      

    if options.no_run {
        if !options.compile_opts.build_config.emit_json() {
            display_no_run_information(ws, test_args, &compilation, "unittests")?;
        }
        return Ok(true);
    }

    let mut failed = false;
    let mut all_errors: Vec<UnitTestError> = Vec::new();

    match run_unit_tests(ws, options, test_args, &compilation, TestKind::Test, quiet) {
        Ok(unit_errs) => all_errors.extend(unit_errs),
        Err(_) => failed = true,
    }
    match run_doc_tests(ws, options, test_args, &compilation, quiet) {
        Ok(doc_errs) => all_errors.extend(doc_errs),
        Err(_) => failed = true,
    }

    if failed || !all_errors.is_empty() { Ok(false) } else { Ok(true) }
}

struct ShellVerbosityGuard<'a> {
    gctx: &'a GlobalContext,
    prev: Verbosity,
}

impl<'a> ShellVerbosityGuard<'a> {
    fn set(gctx: &'a GlobalContext, v: Verbosity) -> Self {
        let mut sh = gctx.shell();
        let prev = sh.verbosity();
        sh.set_verbosity(v);
        drop(sh);
        Self { gctx, prev }
    }
}

impl<'a> Drop for ShellVerbosityGuard<'a> {
    fn drop(&mut self) {
        let mut sh = self.gctx.shell();
        sh.set_verbosity(self.prev);
    }
}

// Replace a file's contents during a scope and restore original on drop.
struct FileReplacer {
    path: std::path::PathBuf,
    original: String,
}

impl FileReplacer {
    fn replace(path: &std::path::Path, new_contents: &str) -> std::io::Result<Self> {
        let original = std::fs::read_to_string(path)?;
        std::fs::write(path, new_contents)?;
        Ok(Self { path: path.to_path_buf(), original })
    }
}

impl Drop for FileReplacer {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.path, &self.original);
    }
}

// Mutation campaign: flip one operator at a time and run tests; report killed/survivors.
fn run_mutation_campaign(ws: &Workspace<'_>, options: &TestOptions, test_args: &[&str]) -> CliResult {
    use self::mutators::{AddSubMutator, DivMulMutator, Mutator};

    // Select mutators to run (can be parameterized later).
    let mutators: Vec<Box<dyn Mutator>> = vec![Box::new(AddSubMutator), Box::new(DivMulMutator)];

    // Prepare non-mutation test options to avoid recursion.
    let mut plain_opts = options.clone();
    plain_opts.mutation = false;

    // Compact header and initial progress line will be shown per-mutator below.
    let bar_width: usize = 20;
    let render_bar = |done: usize, total: usize| -> String {
        let filled = if total == 0 { 0 } else { done * bar_width / total };
        let mut s = String::with_capacity(bar_width);
        for _ in 0..filled { s.push('#'); }
        for _ in filled..bar_width { s.push('-'); }
        s
    };
    let start = Instant::now();

    #[derive(Serialize)]
    struct MutResultEntry { file: String, id: u32, line: u32, column: u32, outcome: &'static str }
    #[derive(Serialize)]
    struct MutJson<'a> {
        kind: &'a str,
        files: usize,
        targets: usize,
        cached: usize,
        results: Vec<MutResultEntry>,
        total: usize,
        killed: usize,
        survived: usize,
        mode: &'a str,
    }

    if !options.mutation_long {
        eprintln!("Mutators: add_sub, mul_div\n");
    }
    // Run each mutator sequentially and emit per-mutator summaries.
    for mutator in mutators.into_iter() {
        if options.mutation_long {
            eprintln!("MUT start kind={} mode=one-at-a-time", mutator.name());
        }

        // Build index + selective AST cache (threshold policy inside).
        let ctx = match mutator.build_context(ws) {
            Ok(c) => c,
            Err(e) => return Err(anyhow::format_err!("indexing failed: {e}").into()),
        };

        let mut total: usize;
        let mut killed = 0usize;
        let mut survived = 0usize;
        let targets = mutator.enumerate_targets(&ctx);
        total = targets.len();

        if options.mutation_long {
            eprintln!(
                "MUT indexed kind={} files={} targets={} cached={}",
                mutator.name(),
                ctx.index.len(),
                total,
                ctx.cached_asts.len()
            );
        }

        if !options.mutation_long {
            // Brief human header per mutator
            let header = match mutator.name() {
                "add_sub" => "Addition <-> Subtraction",
                "mul_div" => "Multiplication <-> Division",
                other => other,
            };
            eprintln!("Mutations:\n{}\n", header);
            eprint!("Progress {}/{} [{}] ({:.2}s)", 0, total, render_bar(0, total), 0.0);
            let _ = io::stderr().flush();
        }

        let mut results_vec: Vec<MutResultEntry> = Vec::new();
        let mut processed: usize = 0;

        for target in targets {
            // Produce mutated source.
            let mutated = match mutator.mutate(&ctx, &target) 
            {
                Ok(m) => 
                {
                    if options.mutation_long
                    {
                        eprintln!("[DEBUG] Mutating file: {:?}, id: {}", target.path, target.id);
                        eprintln!("[DEBUG] Mutated source:\n{}", m);
                    }
                    m
                },
                Err(e) => return Err(anyhow::format_err!
                (
                    "mutation failed for {:?} #{:?}: {e}",
                    target.path, target.id
                ).into()
                ),
            };

            // Replace file on disk; restore back automatically.
            let _guard = match FileReplacer::replace(&target.path, &mutated) {
                Ok(g) => g,
                Err(e) => return Err(anyhow::format_err!(
                    "failed to write mutated file {:?}: {e}",
                    &target.path
                ).into()),
            };

            // Run tests once (compile + run). If they fail, mutation is killed.
            match run_tests_once(ws, &plain_opts, test_args, !options.mutation_long) {
                Ok(true) => {
                    survived += 1;
                    results_vec.push(MutResultEntry { file: target.path.display().to_string(), id: target.id, line: target.line, column: target.column, outcome: "survived" });
                    if options.mutation_long {
                        eprintln!("MUT result kind={} outcome=survived file={:?} id={}", mutator.name(), &target.path, target.id);
                    } else {
                        processed += 1;
                        let secs = start.elapsed().as_secs_f32();
                        eprint!("\rProgress {}/{} [{}] ({:.2}s)", processed, total, render_bar(processed, total), secs);
                        let _ = io::stderr().flush();
                    }
                }
                Ok(false) => {
                    killed += 1;
                    results_vec.push(MutResultEntry { file: target.path.display().to_string(), id: target.id, line: target.line, column: target.column, outcome: "killed" });
                    if options.mutation_long {
                        eprintln!("MUT result kind={} outcome=killed file={:?} id={}", mutator.name(), &target.path, target.id);
                    } else {
                        processed += 1;
                        let secs = start.elapsed().as_secs_f32();
                        eprint!("\rProgress {}/{} [{}] ({:.2}s)", processed, total, render_bar(processed, total), secs);
                        let _ = io::stderr().flush();
                    }
                }
                Err(_) => {
                    // Any error while running tests is treated as killing the mutant
                    killed += 1;
                    results_vec.push(MutResultEntry {
                        file: target.path.display().to_string(),
                        id: target.id,
                        line: target.line,
                        column: target.column,
                        outcome: "killed",
                    });
                    if options.mutation_long {
                        eprintln!(
                            "MUT result kind={} outcome=killed (error while running tests) file={:?} id={}",
                            mutator.name(),
                            &target.path,
                            target.id
                        );
                    } else {
                        processed += 1;
                        let secs = start.elapsed().as_secs_f32();
                        eprint!(
                            "\rProgress {}/{} [{}] ({:.2}s)",
                            processed,
                            total,
                            render_bar(processed, total),
                            secs
                        );
                        let _ = io::stderr().flush();
                    }
                }

            }
        }

        if options.mutation_long {
            eprintln!("MUT summary kind={} total={} killed={} survived={}", mutator.name(), total, killed, survived);
        } else {
            // Final summary block (single header printed earlier; do not reprint bar)
            eprintln!("\n");
            eprintln!("Total {}", total);
            eprintln!("Killed {}", killed);
            eprintln!("Survived {}", survived);
            eprintln!("Test {}", if survived == 0 && killed == total { "Passed" } else { "Failed" });
        }

        // JSON output if requested: write mutator JSON file by kind
        if options.mutation_json {
            #[derive(Serialize)]
            struct MutJsonShort<'a> {
                kind: &'a str,
                files: usize,
                targets: usize,
                cached: usize,
                total: usize,
                killed: usize,
                survived: usize,
                mode: &'a str,
            }

            let s = if options.mutation_long {
                let json = MutJson {
                    kind: mutator.name(),
                    files: ctx.index.len(),
                    targets: total,
                    cached: ctx.cached_asts.len(),
                    results: results_vec,
                    total,
                    killed,
                    survived,
                    mode: "long",
                };
                serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
            } else {
                let json = MutJsonShort {
                    kind: mutator.name(),
                    files: ctx.index.len(),
                    targets: total,
                    cached: ctx.cached_asts.len(),
                    total,
                    killed,
                    survived,
                    mode: "short",
                };
                serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
            };

            // Determine output path, include mutator kind in filename
            let out_dir = if let Some(ref dir) = options.mutation_json_dir {
                dir.clone()
            } else {
                ws.gctx().cwd().to_path_buf()
            };
            let out_path = out_dir.join(format!("mutation-results-{}.json", mutator.name()));
            // Ensure dir exists
            if let Err(e) = std::fs::create_dir_all(&out_dir) {
                eprintln!("failed to create output dir {}: {}", out_dir.display(), e);
            }
            match std::fs::write(&out_path, &s) {
                Ok(_) => {
                    // Print the directory where the JSON was written (at the end).
                    eprintln!("Mutation JSON file: {}", out_path.display());
                }
                Err(e) => {
                    eprintln!("failed to write mutation JSON to {}: {}", out_path.display(), e);
                }
            }
        }
    }
    Ok(())
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

    let errors = run_unit_tests(ws, options, &args, &compilation, TestKind::Bench, false)?;
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
    suppress_output: bool,
) -> Result<Vec<UnitTestError>, CliError> {
    let gctx = ws.gctx();
    let cwd = gctx.cwd();
    let mut errors = Vec::new();

    for UnitOutput {
        unit,
        path,
        script_metas,
    } in compilation.tests.iter()
    {
        let (exe_display, mut cmd) = cmd_builds(
            gctx,
            cwd,
            unit,
            path,
            script_metas.as_ref(),
            test_args,
            compilation,
            "unittests",
        )?;

        if gctx.extra_verbose() {
            cmd.display_env_vars();
        }

        if !suppress_output {
            gctx.shell()
                .concise(|shell| shell.status("Running", &exe_display))?;
            gctx.shell()
                .verbose(|shell| shell.status("Running", &cmd))?;
        }

        let exec_result = if suppress_output { cmd.exec_with_output().map(|_| ()) } else { cmd.exec() };
        if let Err(e) = exec_result {
            let code = fail_fast_code(&e);
            let unit_err = UnitTestError {
                unit: unit.clone(),
                kind: test_kind,
            };
            if !suppress_output {
                report_test_error(ws, test_args, &options.compile_opts, &unit_err, e);
            }
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
    suppress_output: bool,
) -> Result<Vec<UnitTestError>, CliError> {
    let gctx = ws.gctx();
    let mut errors = Vec::new();
    let color = gctx.shell().color_choice();

    for doctest_info in &compilation.to_doc_test {
        let Doctest {
            args,
            unstable_opts,
            unit,
            linker,
            script_metas,
            env,
        } = doctest_info;

        if !suppress_output {
            gctx.shell().status("Doc-tests", unit.target.name())?;
        }
        let mut p = compilation.rustdoc_process(unit, script_metas.as_ref())?;

        for (var, value) in env {
            p.env(var, value);
        }

        let color_arg = match color {
            ColorChoice::Always => "always",
            ColorChoice::Never => "never",
            ColorChoice::CargoAuto => "auto",
        };
        p.arg("--color").arg(color_arg);

        p.arg("--crate-name").arg(&unit.target.crate_name());
        p.arg("--test");

        add_path_args(ws, unit, &mut p);
        p.arg("--test-run-directory").arg(unit.pkg.root());

        if let CompileKind::Target(target) = unit.kind {
            // use `rustc_target()` to properly handle JSON target paths
            p.arg("--target").arg(target.rustc_target());
        }

        if let Some((runtool, runtool_args)) = compilation.target_runner(unit.kind) {
            p.arg("--test-runtool").arg(runtool);
            for arg in runtool_args {
                p.arg("--test-runtool-arg").arg(arg);
            }
        }
        if let Some(linker) = linker {
            let mut joined = OsString::from("linker=");
            joined.push(linker);
            p.arg("-C").arg(joined);
        }

        if unit.profile.panic != PanicStrategy::Unwind {
            p.arg("-C").arg(format!("panic={}", unit.profile.panic));
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

        if gctx.shell().verbosity() == Verbosity::Quiet {
            p.arg("--test-args").arg("--quiet");
        }

        p.args(unit.pkg.manifest().lint_rustflags());

        p.args(args);

        if *unstable_opts {
            p.arg("-Zunstable-options");
        }

        if gctx.extra_verbose() {
            p.display_env_vars();
        }

        if !suppress_output {
            gctx.shell()
                .verbose(|shell| shell.status("Running", p.to_string()))?;
        }

        let exec_result = if suppress_output { p.exec_with_output().map(|_| ()) } else { p.exec() };
        if let Err(e) = exec_result {
            let code = fail_fast_code(&e);
            let unit_err = UnitTestError {
                unit: unit.clone(),
                kind: TestKind::Doctest,
            };
            if !suppress_output {
                report_test_error(ws, test_args, &options.compile_opts, &unit_err, e);
            }
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
    let gctx = ws.gctx();
    let cwd = gctx.cwd();
    for UnitOutput {
        unit,
        path,
        script_metas,
    } in compilation.tests.iter()
    {
        let (exe_display, cmd) = cmd_builds(
            gctx,
            cwd,
            unit,
            path,
            script_metas.as_ref(),
            test_args,
            compilation,
            exec_type,
        )?;
        gctx.shell()
            .concise(|shell| shell.status("Executable", &exe_display))?;
        gctx.shell()
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
    gctx: &GlobalContext,
    cwd: &Path,
    unit: &Unit,
    path: &PathBuf,
    script_metas: Option<&Vec<UnitHash>>,
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

    let mut cmd = compilation.target_process(path, unit.kind, &unit.pkg, script_metas)?;
    cmd.args(test_args);
    if unit.target.harness() && gctx.shell().verbosity() == Verbosity::Quiet {
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

    crate::display_error(&err, &mut ws.gctx().shell());

    let harness: bool = unit_err.unit.target.harness();
    let nocapture: bool = test_args.contains(&"--nocapture") || test_args.contains(&"--no-capture");

    if !is_simple && executed && harness && !nocapture {
        drop(ws.gctx().shell().note(
            "test exited abnormally; to see the full output pass --no-capture to the harness.",
        ));
    }
}
