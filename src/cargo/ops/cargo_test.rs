use std::ffi::{OsString, OsStr};
use std::path::Path;

use ops::{self, ExecEngine, ProcessEngine, Compilation};
use util::{self, CargoResult, CargoTestError, ProcessError};

pub struct TestOptions<'a> {
    pub compile_opts: ops::CompileOptions<'a>,
    pub no_run: bool,
    pub no_fail_fast: bool,
}



#[allow(deprecated)] // connect => join in 1.3
pub fn run_tests(manifest_path: &Path,
                 options: &TestOptions,
                 test_args: &[String]) -> CargoResult<Option<CargoTestError>> {
    let compilation = try!(compile_tests(manifest_path, options));

    if options.no_run {
        return Ok(None)
    }
    let mut errors = try!(run_unit_tests(options, test_args, &compilation));

    // If we have an error and want to fail fast, return
    if errors.len() > 0 && !options.no_fail_fast {
        return Ok(Some(CargoTestError::new(errors)))
    }

    // If a specific test was requested or we're not running any tests at all,
    // don't run any doc tests.
    if let ops::CompileFilter::Only { .. } = options.compile_opts.filter {
        match errors.len() {
            0 => return Ok(None),
            _ => return Ok(Some(CargoTestError::new(errors)))
        }
    }

    errors.extend(try!(run_doc_tests(options, test_args, &compilation)));
    if errors.len() == 0 {
        Ok(None)
    } else {
        Ok(Some(CargoTestError::new(errors)))
    }
}

pub fn run_benches(manifest_path: &Path,
                   options: &TestOptions,
                   args: &[String]) -> CargoResult<Option<CargoTestError>> {
    let mut args = args.to_vec();
    args.push("--bench".to_string());
    let compilation = try!(compile_tests(manifest_path, options));
    let errors = try!(run_unit_tests(options, &args, &compilation));
    match errors.len() {
        0 => Ok(None),
        _ => Ok(Some(CargoTestError::new(errors))),
    }
}

fn compile_tests<'a>(manifest_path: &Path,
                     options: &TestOptions<'a>)
                     -> CargoResult<Compilation<'a>> {
    let mut compilation = try!(ops::compile(manifest_path, &options.compile_opts));
    compilation.tests.sort();
    Ok(compilation)
}

/// Run the unit and integration tests of a project.
fn run_unit_tests(options: &TestOptions,
                  test_args: &[String],
                  compilation: &Compilation)
                  -> CargoResult<Vec<ProcessError>> {
    let config = options.compile_opts.config;
    let cwd = options.compile_opts.config.cwd();

    let mut errors = Vec::new();

    for &(_, ref exe) in &compilation.tests {
        let to_display = match util::without_prefix(exe, &cwd) {
            Some(path) => path,
            None => &**exe,
        };
        let mut cmd = try!(compilation.target_process(exe, &compilation.package));
        cmd.args(test_args);
        try!(config.shell().concise(|shell| {
            shell.status("Running", to_display.display().to_string())
        }));
        try!(config.shell().verbose(|shell| {
            shell.status("Running", cmd.to_string())
        }));

        if let Err(e) = ExecEngine::exec(&mut ProcessEngine, cmd) {
            errors.push(e);
            if !options.no_fail_fast {
                break
            }
        }
    }
    Ok(errors)
}

#[allow(deprecated)] // connect => join in 1.3
fn run_doc_tests(options: &TestOptions,
                 test_args: &[String],
                 compilation: &Compilation)
                 -> CargoResult<Vec<ProcessError>> {
    let mut errors = Vec::new();
    let config = options.compile_opts.config;
    let libs = compilation.package.targets().iter()
                    .filter(|t| t.doctested())
                    .map(|t| (t.src_path(), t.name(), t.crate_name()));
    for (lib, name, crate_name) in libs {
        try!(config.shell().status("Doc-tests", name));
        let mut p = try!(compilation.rustdoc_process(&compilation.package));
        p.arg("--test").arg(lib)
         .arg("--crate-name").arg(&crate_name)
         .cwd(compilation.package.root());

        for &rust_dep in &[&compilation.deps_output, &compilation.root_output] {
            let mut arg = OsString::from("dependency=");
            arg.push(rust_dep);
            p.arg("-L").arg(arg);
        }
        for native_dep in compilation.native_dirs.values() {
            p.arg("-L").arg(native_dep);
        }

        if test_args.len() > 0 {
            p.arg("--test-args").arg(&test_args.connect(" "));
        }

        for feat in compilation.features.iter() {
            p.arg("--cfg").arg(&format!("feature=\"{}\"", feat));
        }

        for (_, libs) in compilation.libraries.iter() {
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
        }

        try!(config.shell().verbose(|shell| {
            shell.status("Running", p.to_string())
        }));
        if let Err(e) = ExecEngine::exec(&mut ProcessEngine, p) {
            errors.push(e);
            if !options.no_fail_fast {
                break
            }
        }
    }
    Ok(errors)
}

fn build_and_run<'a>(manifest_path: &Path,
                     options: &TestOptions<'a>,
                     test_args: &[String])
                     -> CargoResult<Result<Compilation<'a>, ProcessError>> {
    let config = options.compile_opts.config;
    let mut source = try!(PathSource::for_path(&manifest_path.parent().unwrap(),
                                               config));
    try!(source.update());

    let mut compile = try!(ops::compile(manifest_path, &options.compile_opts));
    if options.no_run { return Ok(Ok(compile)) }
    compile.tests.iter_mut()
                 .map(|&mut (_, ref mut tests)|
                      tests.sort_by(|&(ref n1, _), &(ref n2, _)| n1.cmp(n2)))
                 .collect::<Vec<_>>();

    let cwd = config.cwd();
    for &(ref pkg, ref tests) in &compile.tests {
        for &(_, ref exe) in tests {
            let to_display = match util::without_prefix(exe, &cwd) {
                Some(path) => path,
                None => &**exe,
            };
            let mut cmd = try!(compile.target_process(exe, pkg));
            cmd.args(test_args);
            try!(config.shell().concise(|shell| {
                shell.status("Running", to_display.display().to_string())
            }));
            try!(config.shell().verbose(|shell| {
                shell.status("Running", cmd.to_string())
            }));
            match ExecEngine::exec(&mut ProcessEngine, cmd) {
                Ok(()) => {}
                Err(e) => return Ok(Err(e))
            }
        }
    }

    Ok(Ok(compile))
}
