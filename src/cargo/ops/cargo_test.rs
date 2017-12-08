use std::ffi::{OsString, OsStr};

use ops::{self, Compilation};
use util::{self, CargoTestError, Test, ProcessError};
use util::errors::CargoResult;
use core::Workspace;

pub struct TestOptions<'a> {
    pub compile_opts: ops::CompileOptions<'a>,
    pub no_run: bool,
    pub no_fail_fast: bool,
    pub only_doc: bool,
}

pub fn run_tests(ws: &Workspace,
                 options: &TestOptions,
                 test_args: &[String]) -> CargoResult<Option<CargoTestError>> {
    let compilation = compile_tests(ws, options)?;

    if options.no_run {
        return Ok(None)
    }
    let (test, mut errors) = if options.only_doc {
        assert!(options.compile_opts.filter.is_specific());
        run_doc_tests(options, test_args, &compilation)?
    } else {
        run_unit_tests(options, test_args, &compilation)?
    };

    // If we have an error and want to fail fast, return
    if !errors.is_empty() && !options.no_fail_fast {
        return Ok(Some(CargoTestError::new(test, errors)))
    }

    // If a specific test was requested or we're not running any tests at all,
    // don't run any doc tests.
    if options.compile_opts.filter.is_specific() {
        match errors.len() {
            0 => return Ok(None),
            _ => return Ok(Some(CargoTestError::new(test, errors)))
        }
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

pub fn run_benches(ws: &Workspace,
                   options: &TestOptions,
                   args: &[String]) -> CargoResult<Option<CargoTestError>> {
    let mut args = args.to_vec();
    args.push("--bench".to_string());
    let compilation = compile_tests(ws, options)?;

    if options.no_run {
        return Ok(None)
    }
    let (test, errors) = run_unit_tests(options, &args, &compilation)?;
    match errors.len() {
        0 => Ok(None),
        _ => Ok(Some(CargoTestError::new(test, errors))),
    }
}

fn compile_tests<'a>(ws: &Workspace<'a>,
                     options: &TestOptions<'a>)
                     -> CargoResult<Compilation<'a>> {
    let mut compilation = ops::compile(ws, &options.compile_opts)?;
    compilation.tests.sort_by(|a, b| {
        (a.0.package_id(), &a.1, &a.2).cmp(&(b.0.package_id(), &b.1, &b.2))
    });
    Ok(compilation)
}

/// Run the unit and integration tests of a project.
fn run_unit_tests(options: &TestOptions,
                  test_args: &[String],
                  compilation: &Compilation)
                  -> CargoResult<(Test, Vec<ProcessError>)> {
    let config = options.compile_opts.config;
    let cwd = options.compile_opts.config.cwd();

    let mut errors = Vec::new();

    for &(ref pkg, ref kind, ref test, ref exe) in &compilation.tests {
        let to_display = match util::without_prefix(exe, cwd) {
            Some(path) => path,
            None => &**exe,
        };
        let mut cmd = compilation.target_process(exe, pkg)?;
        cmd.args(test_args);
        config.shell().concise(|shell| {
            shell.status("Running", to_display.display().to_string())
        })?;
        config.shell().verbose(|shell| {
            shell.status("Running", cmd.to_string())
        })?;

        let result = cmd.exec();

        match result {
            Err(e) => {
                let e = e.downcast::<ProcessError>()?;
                errors.push((kind.clone(), test.clone(), e));
                if !options.no_fail_fast {
                    break;
                }
            }
            Ok(()) => {}
        }
    }

    if errors.len() == 1 {
        let (kind, test, e) = errors.pop().unwrap();
        Ok((Test::UnitTest(kind, test), vec![e]))
    } else {
        Ok((Test::Multiple, errors.into_iter().map((|(_, _, e)| e)).collect()))
    }
}

fn run_doc_tests(options: &TestOptions,
                 test_args: &[String],
                 compilation: &Compilation)
                 -> CargoResult<(Test, Vec<ProcessError>)> {
    let mut errors = Vec::new();
    let config = options.compile_opts.config;

    // We don't build/rust doctests if target != host
    if config.rustc()?.host != compilation.target {
        return Ok((Test::Doc, errors));
    }

    let libs = compilation.to_doc_test.iter().map(|package| {
        (package, package.targets().iter().filter(|t| t.doctested())
                         .map(|t| (t.src_path(), t.name(), t.crate_name())))
    });

    for (package, tests) in libs {
        for (lib, name, crate_name) in tests {
            config.shell().status("Doc-tests", name)?;
            let mut p = compilation.rustdoc_process(package)?;
            p.arg("--test").arg(lib)
             .arg("--crate-name").arg(&crate_name);

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

            if let Some(cfgs) = compilation.cfgs.get(package.package_id()) {
                for cfg in cfgs.iter() {
                    p.arg("--cfg").arg(cfg);
                }
            }

            let libs = &compilation.libraries[package.package_id()];
            for &(ref target, ref lib) in libs.iter() {
                // Note that we can *only* doctest rlib outputs here.  A
                // staticlib output cannot be linked by the compiler (it just
                // doesn't do that). A dylib output, however, can be linked by
                // the compiler, but will always fail. Currently all dylibs are
                // built as "static dylibs" where the standard library is
                // statically linked into the dylib. The doc tests fail,
                // however, for now as they try to link the standard library
                // dynamically as well, causing problems. As a result we only
                // pass `--extern` for rlib deps and skip out on all other
                // artifacts.
                if lib.extension() != Some(OsStr::new("rlib")) &&
                   !target.for_host() {
                    continue
                }
                let mut arg = OsString::from(target.crate_name());
                arg.push("=");
                arg.push(lib);
                p.arg("--extern").arg(&arg);
            }

            if let Some(flags) = compilation.rustdocflags.get(package.package_id()) {
                p.args(flags);
            }

            config.shell().verbose(|shell| {
                shell.status("Running", p.to_string())
            })?;
            if let Err(e) = p.exec() {
                let e = e.downcast::<ProcessError>()?;
                errors.push(e);
                if !options.no_fail_fast {
                    return Ok((Test::Doc, errors));
                }
            }
        }
    }
    Ok((Test::Doc, errors))
}
