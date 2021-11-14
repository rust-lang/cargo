use crate::core::compiler::{Compilation, CompileKind, Doctest, UnitOutput};
use crate::core::shell::Verbosity;
use crate::core::{TargetKind, Workspace};
use crate::ops;
use crate::util::errors::CargoResult;
use crate::util::{add_path_args, CargoTestError, Config, Progress, ProgressStyle, Test};
use cargo_util::ProcessBuilder;
use cargo_util::ProcessError;
use crossbeam_utils::thread;
use std::ffi::OsString;
use std::sync::{
    mpsc::{Receiver, Sender},
    Arc, Mutex,
};
use std::thread::ThreadId;

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
    /// Test process finished with an error.
    Error(TestError),
}

type TestError = (TargetKind, String, String, anyhow::Error);

fn run_unit_tests(
    config: &Config,
    options: &TestOptions,
    test_args: &[&str],
    compilation: &Compilation<'_>,
) -> CargoResult<(Test, Vec<ProcessError>)> {
    let cwd = config.cwd();

    let mut jobs: Vec<Job> = vec![]; // jobs to run.

    for UnitOutput {
        unit,
        path,
        script_meta,
    } in compilation.tests.iter()
    {
        let test_path = unit.target.src_path().path().unwrap();
        let path_display = path.strip_prefix(cwd).unwrap_or(path).display();
        let exe = if let TargetKind::Test = unit.target.kind() {
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

        let pkg_name = unit.pkg.name().to_string();
        let target = &unit.target;
        let (tx, rx) = std::sync::mpsc::channel();
        jobs.push(Job {
            state: JobState::NotStarted,
            cmd,
            name: target.name().to_string(),
            exe,
            target_kind: target.kind().clone(),
            pkg_name,
            rx,
            tx: Some(tx),
        });
    }

    let mut errors = execute_tests(jobs, config, options, compilation.tests.len(), false)?;

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

fn run_doc_tests(
    ws: &Workspace<'_>,
    options: &TestOptions,
    test_args: &[&str],
    compilation: &Compilation<'_>,
) -> CargoResult<(Test, Vec<ProcessError>)> {
    let config = ws.config();
    let doctest_xcompile = config.cli_unstable().doctest_xcompile;
    let doctest_in_workspace = config.cli_unstable().doctest_in_workspace;

    let mut jobs = vec![];
    let mut total = 0;
    for doctest_info in &compilation.to_doc_test {
        total += 1;
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

        // exec_with_streaming doesn't look like a tty so we have to be explicit
        if !test_args.contains(&"--color=never") && config.shell().err_supports_color() {
            p.arg("--color=always");
        }

        let exe = unit.target.name().to_string();
        let pkg_name = unit.pkg.name().to_string();

        let (tx, rx) = std::sync::mpsc::channel();
        jobs.push(Job {
            state: JobState::NotStarted,
            cmd: p,
            name: unit.target.name().to_string(),
            exe,
            target_kind: unit.target.kind().clone(),
            pkg_name,
            rx,
            tx: Some(tx),
        });
    }
    let errors = execute_tests(jobs, config, options, total, true)?;

    let mut res = vec![];
    for (_, _, _, e) in errors.into_iter() {
        res.push(e.downcast::<ProcessError>()?);
    }
    Ok((Test::Doc, res))
}

fn execute_tests(
    jobs: Vec<Job>,
    config: &Config,
    options: &TestOptions,
    total: usize,
    doc_tests: bool,
) -> CargoResult<Vec<TestError>> {
    thread::scope(|s| {
        let mut errors: Vec<TestError> = Vec::new();
        let jobs = Arc::new(Mutex::new(jobs));
        let mut progress = Progress::with_style("Testing", ProgressStyle::Ratio, config);

        // Run n test crates in parallel
        for _ in 0..options.compile_opts.build_config.test_jobs {
            let jobs = jobs.clone();
            s.spawn(move |_| {
                loop {
                    let tx;
                    let cmd;
                    let name;
                    let target_kind;
                    let pkg_name;
                    // Transition job to in progress and put rx in job.
                    {
                        let mut jobs = jobs.lock().unwrap();
                        if let Some(mut job) = jobs
                            .iter_mut()
                            .filter(|job| matches!(job.state, JobState::NotStarted))
                            .nth(0)
                        {
                            cmd = job.cmd.clone();
                            name = job.name.clone();
                            target_kind = job.target_kind.clone();
                            pkg_name = job.pkg_name.clone();
                            job.state = JobState::InProgress(std::thread::current().id());
                            tx = job.tx.take().expect("tx to exist");
                        } else {
                            break;
                        }
                    }

                    let result = cmd
                        .exec_with_streaming(
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
                        .map_err(|e| (target_kind, name, pkg_name, e));
                    drop(cmd);
                    if let Err(err) = result {
                        tx.send(OutOrErr::Error(err)).unwrap();
                    }

                    for mut job in &mut *jobs.lock().unwrap() {
                        if let JobState::InProgress(thread_id) = job.state {
                            if thread_id == std::thread::current().id() {
                                job.state = JobState::Finished;
                                break;
                            }
                        }
                    }
                }
            });
        }

        std::thread::sleep(std::time::Duration::from_millis(100));

        // Report results in the standard order...
        for _ in 0..total {
            let active_names: Vec<String>;
            let done_count;
            let job: Job = {
                let mut jobs = jobs.lock().unwrap();
                done_count = total
                    - jobs
                        .iter()
                        .filter(|job| {
                            matches!(job.state, JobState::NotStarted | JobState::InProgress(_))
                        })
                        .count();
                active_names = jobs
                    .iter()
                    .filter(|job| matches!(job.state, JobState::InProgress(_)))
                    .map(|job| job.name.clone())
                    .collect();
                jobs.remove(0)
            };

            progress.clear();
            config.shell().concise(|shell| {
                shell.status(if doc_tests { "Doc-tests" } else { "Running" }, &job.exe)
            })?;
            config
                .shell()
                .verbose(|shell| shell.status("Running", &job.cmd))?;

            for line in job.rx.iter() {
                progress.clear();
                match line {
                    OutOrErr::Out(line) => writeln!(config.shell().out(), "{}", line).unwrap(),
                    OutOrErr::Err(line) => writeln!(config.shell().err(), "{}", line).unwrap(),
                    OutOrErr::Error(err) => {
                        errors.push(err);
                        if !options.no_fail_fast {
                            break;
                        }
                    }
                }
                drop(progress.tick_now(
                    done_count,
                    total,
                    &format!(": {}", active_names.join(", ")),
                ));
            }
        }
        let out: Result<_, anyhow::Error> = Ok(errors);
        out
    })
    .unwrap()
}

#[derive(Debug)]
struct Job {
    name: String,
    cmd: ProcessBuilder,
    exe: String,
    target_kind: TargetKind,
    pkg_name: String,
    state: JobState,
    rx: Receiver<OutOrErr>,
    tx: Option<Sender<OutOrErr>>,
}

#[derive(Debug)]
enum JobState {
    NotStarted,
    InProgress(ThreadId),
    Finished,
}
