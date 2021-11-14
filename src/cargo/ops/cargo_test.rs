use crate::core::compiler::{Compilation, CompileKind, Doctest, UnitOutput};
use crate::core::shell::Verbosity;
use crate::core::{TargetKind, Workspace};
use crate::ops;
use crate::util::errors::CargoResult;
use crate::util::{add_path_args, CargoTestError, Config, Progress, ProgressStyle, Test};
use cargo_util::ProcessError;
use crossbeam_utils::thread::{self, ScopedJoinHandle};
use std::ffi::OsString;
use std::fmt::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

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
    exe_display: String,
    cmd: String,
}

type TestError = (TargetKind, String, String, anyhow::Error);
type TestDocError = (crate::util::errors::Test, anyhow::Error);

fn run_unit_tests(
    config: &Config,
    options: &TestOptions,
    test_args: &[&str],
    compilation: &Compilation<'_>,
) -> CargoResult<(Test, Vec<ProcessError>)> {
    let cwd = config.cwd();
    let mut errors: Vec<TestError> = Vec::new();

    let processing: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
    thread::scope(|s| {
        let mut handles = vec![];
        let test_jobs = options.compile_opts.build_config.test_jobs;
        let parallel = test_jobs > 1;

        let mut progress = Progress::with_style("Testing", ProgressStyle::Ratio, config);
        let mut fin = 0;

        for UnitOutput {
            unit,
            path,
            script_meta,
        } in compilation.tests.iter()
        {
            let test_path = unit.target.src_path().path().unwrap();
            let path_display = path.strip_prefix(cwd).unwrap_or(path).display();
            let exe_display = if let TargetKind::Test = unit.target.kind() {
                format!(
                    "{} ({})",
                    test_path
                        .strip_prefix(unit.pkg.root())
                        .unwrap_or(test_path)
                        .display(),
                    path_display
                )
            } else {
                format!("unittests ({})", path_display)
            };

            let mut cmd = compilation.target_process(&path, unit.kind, &unit.pkg, *script_meta)?;
            cmd.args(test_args);
            if unit.target.harness() && config.shell().verbosity() == Verbosity::Quiet {
                cmd.arg("--quiet");
            }
            // exec_with_streaming doesn't look like a tty so we have to be explicit
            if !test_args.contains(&"--color=never") && config.shell().err_supports_color() {
                cmd.arg("--color=always");
            }

            let mut ctx = Context {
                exe_display,
                cmd: String::new(),
            };
            write!(ctx.cmd, "{}", &cmd).unwrap();

            let processing_t = processing.clone();
            let pkg_name = unit.pkg.name().to_string();
            let target = &unit.target;
            let (tx, rx) = std::sync::mpsc::channel();
            let handle = s.spawn(move |_| {
                std::thread::park();

                let result = cmd
                    .exec_with_streaming(
                        &mut |line| {
                            if let Err(_) = tx.send(OutOrErr::Out(line.to_string())) {
                                println!("out-of-order: {}", line);
                            }
                            Ok(())
                        },
                        &mut |line| {
                            if let Err(_) = tx.send(OutOrErr::Err(line.to_string())) {
                                eprintln!("out-of-order: {}", line);
                            };
                            Ok(())
                        },
                        false,
                    )
                    .map_err(|e| {
                        (
                            target.kind().clone(),
                            target.name().to_string(),
                            pkg_name.clone(),
                            e,
                        )
                    });
                drop(cmd);
                let mut list = processing_t.lock().unwrap();
                let idx = list
                    .iter()
                    .position(|i| **i == target.name().to_string())
                    .unwrap();
                list.remove(idx);
                result
            });
            let pkg_name = unit.pkg.name().to_string();
            if parallel {
                handles.push((handle, rx, ctx, target.name().to_string()));
            } else {
                handle.thread().unpark();
                let active_names = vec![pkg_name];
                if !process_output(
                    handle,
                    &mut errors,
                    ctx,
                    config,
                    options,
                    rx,
                    fin,
                    compilation.tests.len(),
                    &active_names[..],
                    &mut progress,
                )? {
                    break;
                }
                fin += 1;
            }
        }

        if parallel {
            let mut threads: Vec<(_, _)> = handles
                .iter()
                .map(|(h, _, _, name)| (h.thread().clone(), name.clone()))
                .collect();

            //Have a thread unparking so that there's no more than n running at once...
            let proc = processing.clone();
            s.spawn(move |_| {
                while !threads.is_empty() {
                    if proc.lock().unwrap().len() < test_jobs as usize {
                        let (thread, name) = threads.pop().unwrap();
                        proc.lock().unwrap().push(name);
                        thread.unpark();
                    } else {
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
            });
            std::thread::sleep(Duration::from_millis(100));
            // Report results in the standard order...
            for (handle, rx, ctx, _name) in handles {
                let active_names;
                {
                    active_names = processing.lock().unwrap().clone();
                }

                if !process_output(
                    handle,
                    &mut errors,
                    ctx,
                    config,
                    options,
                    rx,
                    fin,
                    compilation.tests.len(),
                    &active_names[..],
                    &mut progress,
                )? {
                    break;
                }
                fin += 1;
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
    fin: usize,
    max: usize,
    active_names: &[String],
    progress: &mut Progress<'_>,
) -> CargoResult<bool> {
    progress.clear();
    config
        .shell()
        .concise(|shell| shell.status("Running", &ctx.exe_display))?;
    config
        .shell()
        .verbose(|shell| shell.status("Running", &ctx.cmd))?;

    while let Ok(line) = rx.recv() {
        progress.clear();
        match line {
            OutOrErr::Out(line) => writeln!(config.shell().out(), "{}", line).unwrap(),
            OutOrErr::Err(line) => writeln!(config.shell().err(), "{}", line).unwrap(),
        }
        drop(progress.tick_now(fin, max, &format!(": {}", active_names.join(", "))));
    }
    let result = handle.join().unwrap();

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

    let processing = AtomicU32::new(0);
    thread::scope(|s| {
        let mut handles = vec![];
        let test_jobs = options.compile_opts.build_config.test_jobs;
        let parallel = test_jobs > 1;

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

            let mut ctx = Context {
                exe_display: unit.target.name().to_string(),
                cmd: String::new(),
            };
            write!(ctx.cmd, "{}", &p).unwrap();

            let processing_t = &processing;
            let (tx, rx) = std::sync::mpsc::channel();
            let handle = s.spawn(move |_| {
                std::thread::park();

                let result = p
                    .exec_with_streaming(
                        &mut |line| {
                            if let Err(_) = tx.send(OutOrErr::Out(line.to_string())) {
                                println!("out-of-order: {}", line);
                            }
                            Ok(())
                        },
                        &mut |line| {
                            if let Err(_) = tx.send(OutOrErr::Err(line.to_string())) {
                                eprintln!("out-of-order: {}", line);
                            }
                            Ok(())
                        },
                        false,
                    )
                    .map_err(|e| (Test::Doc, e));
                processing_t.fetch_sub(1, Ordering::Relaxed);
                result
            });
            if parallel {
                handles.push((handle, rx, ctx));
            } else {
                handle.thread().unpark();
                if !process_doc_output(handle, &mut errors, ctx, config, options, rx)? {
                    break;
                }
            }
        }

        if parallel {
            let mut threads: Vec<_> = handles.iter().map(|(h, _, _)| h.thread().clone()).collect();

            //Have a thread unparking so that there's no more than n running at once...
            let proc = &processing;
            s.spawn(move |_| {
                while !threads.is_empty() {
                    if proc.load(Ordering::Relaxed) < test_jobs {
                        proc.fetch_add(1, Ordering::Relaxed);
                        threads.pop().unwrap().unpark();
                    } else {
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
            });

            // Report results in the standard order...
            for (handle, rx, ctx) in handles {
                if !process_doc_output(handle, &mut errors, ctx, config, options, rx)? {
                    break;
                }
            }
        }
        let out: Result<(), anyhow::Error> = Ok(());
        out
    })
    .unwrap()?;
    let mut res = vec![];
    for (_, e) in errors.into_iter() {
        res.push(e.downcast::<ProcessError>()?);
    }
    Ok((Test::Doc, res))
}

/// Puts test output on the sceen.
/// Returns false if we should early exit due to test failures.
fn process_doc_output<'scope>(
    handle: ScopedJoinHandle<'scope, Result<std::process::Output, TestDocError>>,
    errors: &mut Vec<TestDocError>,
    ctx: Context,
    config: &Config,
    options: &TestOptions,
    rx: std::sync::mpsc::Receiver<OutOrErr>,
) -> CargoResult<bool> {
    config.shell().status("Doc-tests", ctx.exe_display)?;

    config
        .shell()
        .verbose(|shell| shell.status("Running", &ctx.cmd))?;

    while let Ok(line) = rx.recv() {
        match line {
            OutOrErr::Out(line) => writeln!(config.shell().out(), "{}", line).unwrap(),
            OutOrErr::Err(line) => writeln!(config.shell().err(), "{}", line).unwrap(),
        }
    }
    let result = handle.join().unwrap();

    if let Err(err) = result {
        errors.push(err);
        if !options.no_fail_fast {
            return Ok(false);
        }
    }
    Ok(true)
}
