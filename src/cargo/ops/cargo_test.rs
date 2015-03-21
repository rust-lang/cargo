use std::ffi::OsString;
use std::path::Path;

use core::Source;
use sources::PathSource;
use ops::{self, ExecEngine, ProcessEngine, Compilation};
use util::{CargoResult, ProcessError};

pub struct TestOptions<'a, 'b: 'a> {
    pub compile_opts: ops::CompileOptions<'a, 'b>,
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

    if options.no_run { return Ok(None) }

    let libs = compile.package.targets().iter()
                      .filter(|t| t.doctested())
                      .map(|t| (t.src_path(), t.name()));

    for (lib, name) in libs {
        try!(config.shell().status("Doc-tests", name));
        let mut p = try!(compile.rustdoc_process(&compile.package));
        p.arg("--test").arg(lib)
         .arg("--crate-name").arg(name)
         .arg("-L").arg(&compile.root_output)
         .arg("-L").arg(&compile.deps_output)
         .cwd(compile.package.root());

        if test_args.len() > 0 {
            p.arg("--test-args").arg(&test_args.connect(" "));
        }

        for feat in compile.features.iter() {
            p.arg("--cfg").arg(&format!("feature=\"{}\"", feat));
        }

        for (pkg, libs) in compile.libraries.iter() {
            for lib in libs.iter() {
                let mut arg = OsString::from_str(pkg.name());
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

fn build_and_run(manifest_path: &Path,
                 options: &TestOptions,
                 test_args: &[String])
                 -> CargoResult<Result<Compilation, ProcessError>> {
    let config = options.compile_opts.config;
    let mut source = try!(PathSource::for_path(&manifest_path.parent().unwrap(),
                                               config));
    try!(source.update());

    let mut compile = try!(ops::compile(manifest_path, &options.compile_opts));
    if options.no_run { return Ok(Ok(compile)) }
    compile.tests.sort();

    let cwd = config.cwd();
    for &(_, ref exe) in &compile.tests {
        let to_display = match exe.relative_from(&cwd) {
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
