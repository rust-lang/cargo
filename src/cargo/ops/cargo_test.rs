use std::ffi::OsString;

use crate::core::compiler::{Compilation, Doctest};
use crate::core::shell::Verbosity;
use crate::core::Workspace;
use crate::ops;
use crate::util::errors::CargoResult;
use crate::util::{CargoTestError, ProcessError, Test};

pub struct TestOptions<'a> {
    pub compile_opts: ops::CompileOptions<'a>,
    pub no_run: bool,
    pub no_fail_fast: bool,
}

pub fn run_tests(
    ws: &Workspace<'_>,
    options: &TestOptions<'_>,
    test_args: &[&str],
) -> CargoResult<Option<CargoTestError>> {
    let compilation = compile_tests(ws, options)?;

    if options.no_run {
        return Ok(None);
    }
    let (test, mut errors) = run_unit_tests(options, test_args, &compilation)?;

    // If we have an error and want to fail fast, then return.
    if !errors.is_empty() && !options.no_fail_fast {
        return Ok(Some(CargoTestError::new(test, errors)));
    }

    let (doctest, docerrors) = run_doc_tests(options, test_args, &compilation)?;
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
    options: &TestOptions<'_>,
    args: &[&str],
) -> CargoResult<Option<CargoTestError>> {
    let compilation = compile_tests(ws, options)?;

    if options.no_run {
        return Ok(None);
    }

    let mut args = args.to_vec();
    args.push("--bench");

    let (test, errors) = run_unit_tests(options, &args, &compilation)?;

    match errors.len() {
        0 => Ok(None),
        _ => Ok(Some(CargoTestError::new(test, errors))),
    }
}

fn compile_tests<'a>(
    ws: &Workspace<'a>,
    options: &TestOptions<'a>,
) -> CargoResult<Compilation<'a>> {
    let mut compilation = ops::compile(ws, &options.compile_opts)?;
    compilation
        .tests
        .sort_by(|a, b| (a.0.package_id(), &a.1, &a.2).cmp(&(b.0.package_id(), &b.1, &b.2)));
    Ok(compilation)
}

/// Runs the unit and integration tests of a package.
fn run_unit_tests(
    options: &TestOptions<'_>,
    test_args: &[&str],
    compilation: &Compilation<'_>,
) -> CargoResult<(Test, Vec<ProcessError>)> {
    let config = options.compile_opts.config;
    let cwd = options.compile_opts.config.cwd();

    let mut errors = Vec::new();

    for &(ref pkg, ref target, ref exe) in &compilation.tests {
        let kind = target.kind();
        let test = target.name().to_string();
        let exe_display = exe.strip_prefix(cwd).unwrap_or(exe).display();
        let mut cmd = compilation.target_process(exe, pkg)?;
        cmd.args(test_args);
        if target.harness() && config.shell().verbosity() == Verbosity::Quiet {
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
                errors.push((kind.clone(), test.clone(), pkg.name().to_string(), e));
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
    options: &TestOptions<'_>,
    test_args: &[&str],
    compilation: &Compilation<'_>,
) -> CargoResult<(Test, Vec<ProcessError>)> {
    let mut errors = Vec::new();
    let config = options.compile_opts.config;

    // The unstable doctest-xcompile feature enables both per-target-ignores and
    // cross-compiling doctests. As a side effect, this feature also gates running
    // doctests with runtools when target == host.
    let doctest_xcompile = config.cli_unstable().doctest_xcompile;
    let mut runtool: &Option<(std::path::PathBuf, Vec<String>)> = &None;
    if doctest_xcompile {
        runtool = compilation.target_runner();
    } else if compilation.host != compilation.target {
        return Ok((Test::Doc, errors));
    }

    for doctest_info in &compilation.to_doc_test {
        let Doctest {
            package,
            target,
            deps,
        } = doctest_info;
        config.shell().status("Doc-tests", target.name())?;
        let mut p = compilation.rustdoc_process(package, target)?;
        p.arg("--test")
            .arg(target.src_path().path().unwrap())
            .arg("--crate-name")
            .arg(&target.crate_name());

        if doctest_xcompile {
            p.arg("--target").arg(&compilation.target);
            p.arg("-Zunstable-options");
            p.arg("--enable-per-target-ignores");
        }

        runtool.as_ref().map(|(runtool, runtool_args)| {
            p.arg("--runtool").arg(runtool);
            for arg in runtool_args {
                p.arg("--runtool-arg").arg(arg);
            }
        });

        for &rust_dep in &[&compilation.deps_output] {
            let mut arg = OsString::from("dependency=");
            arg.push(rust_dep);
            p.arg("-L").arg(arg);
        }

        for native_dep in compilation.native_dirs.iter() {
            p.arg("-L").arg(native_dep);
        }

        for &host_rust_dep in &[&compilation.host_deps_output] {
            let mut arg = OsString::from("dependency=");
            arg.push(host_rust_dep);
            p.arg("-L").arg(arg);
        }

        for arg in test_args {
            p.arg("--test-args").arg(arg);
        }

        if let Some(cfgs) = compilation.cfgs.get(&package.package_id()) {
            for cfg in cfgs.iter() {
                p.arg("--cfg").arg(cfg);
            }
        }

        for &(ref extern_crate_name, ref lib) in deps.iter() {
            let mut arg = OsString::from(extern_crate_name.as_str());
            arg.push("=");
            arg.push(lib);
            p.arg("--extern").arg(&arg);
        }

        if let Some(flags) = compilation.rustdocflags.get(&package.package_id()) {
            p.args(flags);
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
