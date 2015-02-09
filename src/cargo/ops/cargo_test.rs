
use core::Source;
use sources::PathSource;
use ops::{self, ExecEngine, ProcessEngine};
use util::{CargoResult, ProcessError};

pub struct TestOptions<'a, 'b: 'a> {
    pub compile_opts: ops::CompileOptions<'a, 'b>,
    pub no_run: bool,
    pub name: Option<&'a str>,
}

pub fn run_tests(manifest_path: &Path,
                 options: &TestOptions,
                 test_args: &[String]) -> CargoResult<Option<ProcessError>> {
    let config = options.compile_opts.config;
    let mut source = try!(PathSource::for_path(&manifest_path.dir_path(),
                                               config));
    try!(source.update());

    let mut compile = try!(ops::compile(manifest_path, &options.compile_opts));
    if options.no_run { return Ok(None) }
    compile.tests.sort();

    let tarname = options.name;
    let tests_to_run = compile.tests.iter().filter(|&&(ref test_name, _)| {
        tarname.map_or(true, |tarname| tarname == *test_name)
    });

    let cwd = config.cwd();
    for &(_, ref exe) in tests_to_run {
        let to_display = match exe.path_relative_from(&cwd) {
            Some(path) => path,
            None => exe.clone(),
        };
        let cmd = try!(compile.target_process(exe, &compile.package))
                  .args(test_args);
        try!(config.shell().concise(|shell| {
            shell.status("Running", to_display.display().to_string())
        }));
        try!(config.shell().verbose(|shell| {
            shell.status("Running", cmd.to_string())
        }));
        match ExecEngine::exec(&mut ProcessEngine, cmd) {
            Ok(()) => {}
            Err(e) => return Ok(Some(e))
        }
    }

    if options.name.is_some() { return Ok(None) }

    if options.compile_opts.env == "bench" { return Ok(None) }

    let libs = compile.package.targets().iter().filter_map(|target| {
        if !target.profile().is_doctest() || !target.is_lib() {
            return None
        }
        Some((target.src_path(), target.name()))
    });

    for (lib, name) in libs {
        try!(config.shell().status("Doc-tests", name));
        let mut p = try!(compile.rustdoc_process(&compile.package))
                           .arg("--test").arg(lib)
                           .arg("--crate-name").arg(name)
                           .arg("-L").arg(&compile.root_output)
                           .arg("-L").arg(&compile.deps_output)
                           .cwd(compile.package.root());

        // FIXME(rust-lang/rust#16272): this should just always be passed.
        if test_args.len() > 0 {
            p = p.arg("--test-args").arg(test_args.connect(" "));
        }

        for (pkg, libs) in compile.libraries.iter() {
            for lib in libs.iter() {
                let mut arg = pkg.name().as_bytes().to_vec();
                arg.push(b'=');
                arg.push_all(lib.as_vec());
                p = p.arg("--extern").arg(arg);
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

    run_tests(manifest_path, options, &args)
}
