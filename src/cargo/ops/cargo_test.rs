use std::ffi::{OsString, OsStr};
use std::path::Path;

use core::Source;
use sources::PathSource;
use ops::{self, ExecEngine, ProcessEngine, Compilation};
use util::{self, CargoResult, ProcessError};

pub struct TestOptions<'a> {
    pub compile_opts: ops::CompileOptions<'a>,
    pub no_run: bool,
}

pub fn run_tests(manifest_path: &Path,
                 options: &TestOptions,
                 test_args: &[String]) -> CargoResult<Option<ProcessError>> {
    let config = options.compile_opts.config;
    let compile = match try!(build_and_run(manifest_path, options, test_args)) {
        Ok(compile) => compile,
        Err(e) => return Ok(Some(e)),
    };

    // If a specific test was requested or we're not running any tests at all,
    // don't run any doc tests.
    if let ops::CompileFilter::Only { .. } = options.compile_opts.filter {
        return Ok(None)
    }
    if options.no_run {
        return Ok(None)
    }

    let libs = compile.package.targets().iter()
                      .filter(|t| t.doctested())
                      .map(|t| (t.src_path(), t.name(), t.crate_name()));

    for (lib, name, crate_name) in libs {
        try!(config.shell().status("Doc-tests", name));
        let mut p = try!(compile.rustdoc_process(&compile.package));
        p.arg("--test").arg(lib)
         .arg("--crate-name").arg(&crate_name)
         .cwd(compile.package.root());

        for &rust_dep in &[&compile.deps_output, &compile.root_output] {
            let mut arg = OsString::from("dependency=");
            arg.push(rust_dep);
            p.arg("-L").arg(arg);
        }
        for native_dep in compile.native_dirs.values() {
            p.arg("-L").arg(native_dep);
        }

        if test_args.len() > 0 {
            p.arg("--test-args").arg(&test_args.connect(" "));
        }

        for feat in compile.features.iter() {
            p.arg("--cfg").arg(&format!("feature=\"{}\"", feat));
        }

        for (_, libs) in compile.libraries.iter() {
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
        match ExecEngine::exec(&mut ProcessEngine, p) {
            Ok(()) => {}
            Err(e) => return Ok(Some(e)),
        }
    }

    Ok(None)
}

pub fn run_benches(manifest_path: &Path,
                   options: &TestOptions,
                   args: &[String]) -> CargoResult<Option<ProcessError>> {
    let mut args = args.to_vec();
    args.push("--bench".to_string());

    Ok(try!(build_and_run(manifest_path, options, &args)).err())
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
    compile.tests.sort();

    let cwd = config.cwd();
    for &(_, ref exe) in &compile.tests {
        let to_display = match util::without_prefix(exe, &cwd) {
            Some(path) => path,
            None => &**exe,
        };
        let mut cmd = try!(compile.target_process(exe, &compile.package));
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

    Ok(Ok(compile))
}
