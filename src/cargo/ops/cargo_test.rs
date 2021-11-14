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

    let mut errors = thread::scope(|s| {
        let mut handles: Vec<Job> = vec![]; // jobs to run.

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

            let pkg_name = unit.pkg.name().to_string();
            let target = &unit.target;

            handles.push(Job::NotStarted {
                cmd,
                name: target.name().to_string(),
                exe_display,
                target_kind: target.kind().clone(),
                pkg_name,
            });
        }

        execute_tests(handles, config, options, s, compilation.tests.len(), false)
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

fn run_doc_tests(
    ws: &Workspace<'_>,
    options: &TestOptions,
    test_args: &[&str],
    compilation: &Compilation<'_>,
) -> CargoResult<(Test, Vec<ProcessError>)> {
    let config = ws.config();
    let doctest_xcompile = config.cli_unstable().doctest_xcompile;
    let doctest_in_workspace = config.cli_unstable().doctest_in_workspace;

    let errors = thread::scope(|s| {
        let mut handles = vec![];
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

            let exe_display = unit.target.name().to_string();
            let pkg_name = unit.pkg.name().to_string();

            handles.push(Job::NotStarted {
                cmd: p,
                name: unit.target.name().to_string(),
                exe_display,
                target_kind: unit.target.kind().clone(),
                pkg_name,
            });
        }

        execute_tests(handles, config, options, s, total, true)
    })
    .unwrap()?;

    let mut res = vec![];
    for (_, _, _, e) in errors.into_iter() {
        res.push(e.downcast::<ProcessError>()?);
    }
    Ok((Test::Doc, res))
}

fn execute_tests(
    handles: Vec<Job>,
    config: &Config,
    options: &TestOptions,
    s: &thread::Scope<'_>,
    total: usize,
    doc_tests: bool,
) -> CargoResult<Vec<TestError>> {
    let mut errors: Vec<TestError> = Vec::new();
    let handles = Arc::new(Mutex::new(handles));
    let mut progress = Progress::with_style("Testing", ProgressStyle::Ratio, config);

    // Run n test crates in parallel
    for _ in 0..options.compile_opts.build_config.test_jobs {
        let handles = handles.clone();
        s.spawn(move |_| {
            loop {
                let tx_a;
                let cmd_a;
                let name_a;
                let target_kind_a;
                let pkg_name_a;
                // Transition job to in progress and put rx in job.
                {
                    let mut jobs = handles.lock().unwrap();
                    let job_idx = jobs
                        .iter()
                        .position(|job| matches!(job, Job::NotStarted { .. }));
                    if let Some(job_idx) = job_idx {
                        let job = std::mem::replace(&mut jobs[job_idx], Job::Placeholder);
                        if let Job::NotStarted {
                            cmd,
                            name,
                            target_kind,
                            pkg_name,
                            ..
                        } = &job
                        {
                            cmd_a = cmd.clone();
                            name_a = name.clone();
                            target_kind_a = target_kind.clone();
                            pkg_name_a = pkg_name.clone();
                        } else {
                            panic!("oh dear");
                        }

                        let (job, tx) = job.start(std::thread::current().id());
                        tx_a = tx;

                        drop(std::mem::replace(&mut jobs[job_idx], job));
                    } else {
                        break;
                    }
                }

                let result = cmd_a
                    .exec_with_streaming(
                        &mut |line| {
                            if let Err(_) = tx_a.send(OutOrErr::Out(line.to_string())) {
                                println!("out-of-order: {}", line);
                            }
                            Ok(())
                        },
                        &mut |line| {
                            if let Err(_) = tx_a.send(OutOrErr::Err(line.to_string())) {
                                eprintln!("out-of-order: {}", line);
                            };
                            Ok(())
                        },
                        false,
                    )
                    .map_err(|e| (target_kind_a, name_a, pkg_name_a, e));
                drop(cmd_a);
                if let Err(err) = result {
                    tx_a.send(OutOrErr::Error(err)).unwrap();
                }

                let mut jobs = handles.lock().unwrap();

                let job_a: Option<&mut Job> = (*jobs).iter_mut().find(|j| match j {
                    Job::InProgress { thread_id, .. }
                        if *thread_id == std::thread::current().id() =>
                    {
                        true
                    }
                    _ => false,
                });

                if let Some(job_a) = job_a {
                    let mut job: Job = Job::Placeholder;
                    std::mem::swap(job_a, &mut &mut job);
                    std::mem::swap(job_a, &mut &mut job.done());
                } else {
                    // NoOp: this job is being reported currently.
                }
            }
        });
    }

    std::thread::sleep(Duration::from_millis(100));

    // Report results in the standard order...
    for _ in 0..total {
        let active_names: Vec<String>;

        // TODO wait for start! - read or timeout
        let rx: Receiver<OutOrErr>;
        let cmd: ProcessBuilder;
        let exe_display: String;

        let done_count;
        {
            let mut jobs = handles.lock().unwrap();
            done_count = total
                - jobs
                    .iter()
                    .filter(|job| matches!(job, Job::NotStarted { .. } | Job::InProgress { .. }))
                    .count();
            active_names = jobs
                .iter()
                .filter_map(|job| match job {
                    Job::InProgress { name, .. } => Some(name.clone()),
                    _ => None,
                })
                .collect();
            let job = jobs.remove(0);
            match job {
                Job::InProgress {
                    rx: rx_a,
                    cmd: cmd_a,
                    exe_display: exe_display_a,
                    ..
                } => {
                    rx = rx_a;
                    cmd = cmd_a;
                    exe_display = exe_display_a;
                }
                Job::Finished {
                    rx: rx_a,
                    cmd: cmd_a,
                    exe_display: exe_display_a,
                    ..
                } => {
                    rx = rx_a;
                    cmd = cmd_a;
                    exe_display = exe_display_a;
                }
                job @ _ => {
                    panic!("not expecting state {:?}", job);
                }
            }
        }

        if !process_output(
            &mut errors,
            cmd,
            exe_display,
            config,
            options,
            rx,
            done_count,
            total,
            &active_names[..],
            &mut progress,
            doc_tests
        )? {
            break;
        }
    }
    let out: Result<_, anyhow::Error> = Ok(errors);
    out
}

#[derive(Debug)]
enum Job {
    NotStarted {
        cmd: ProcessBuilder,
        name: String,
        exe_display: String,
        target_kind: TargetKind,
        pkg_name: String,
    },
    InProgress {
        cmd: ProcessBuilder,
        name: String,
        exe_display: String,
        rx: Receiver<OutOrErr>,
        thread_id: ThreadId,
    },
    Finished {
        cmd: ProcessBuilder,
        exe_display: String,
        rx: Receiver<OutOrErr>,
    },
    Placeholder,
}

impl Job {
    fn start(self, thread_id: ThreadId) -> (Self, Sender<OutOrErr>) {
        if let Self::NotStarted {
            cmd,
            name,
            exe_display,
            ..
        } = self
        {
            let (tx, rx) = std::sync::mpsc::channel();
            (
                Self::InProgress {
                    cmd,
                    name,
                    exe_display,
                    rx,
                    thread_id,
                },
                tx,
            )
        } else {
            panic!("Wrong starting state {:?}", self);
        }
    }

    fn done(self) -> Self {
        if let Self::InProgress {
            rx,
            cmd,
            exe_display,
            ..
        } = self
        {
            Self::Finished {
                cmd,
                rx,
                exe_display,
            }
        } else {
            panic!("Should be in progress: {:?}", self);
        }
    }
}

/// Puts test output on the sceen.
/// Returns false if we should early exit due to test failures.
fn process_output<'scope>(
    errors: &mut Vec<TestError>,
    cmd: ProcessBuilder,
    exe_display: String,
    config: &Config,
    options: &TestOptions,
    rx: std::sync::mpsc::Receiver<OutOrErr>,
    fin: usize,
    max: usize,
    active_names: &[String],
    progress: &mut Progress<'_>,
    doc_tests: bool,
) -> CargoResult<bool> {
    progress.clear();
    config.shell().concise(|shell| {
        shell.status(
            if doc_tests { "Doc-tests" } else { "Running" },
            &exe_display,
        )
    })?;
    config
        .shell()
        .verbose(|shell| shell.status("Running", &cmd))?;

    while let Ok(line) = rx.recv() {
        progress.clear();
        match line {
            OutOrErr::Out(line) => writeln!(config.shell().out(), "{}", line).unwrap(),
            OutOrErr::Err(line) => writeln!(config.shell().err(), "{}", line).unwrap(),
            OutOrErr::Error(err) => {
                errors.push(err);
                if !options.no_fail_fast {
                    return Ok(false);
                }
            }
        }
        drop(progress.tick_now(fin, max, &format!(": {}", active_names.join(", "))));
    }
    Ok(true)
}
