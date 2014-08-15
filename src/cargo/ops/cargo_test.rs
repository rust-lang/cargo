use std::os;

use core::Source;
use sources::PathSource;
use ops;
use util::{CargoResult, ProcessError};

pub fn run_tests(manifest_path: &Path,
                 options: &mut ops::CompileOptions,
                 args: &[String]) -> CargoResult<Option<ProcessError>> {
    let mut source = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(source.update());
    let package = try!(source.get_root_package());

    let mut compile = try!(ops::compile(manifest_path, options));
    compile.tests.sort();

    let cwd = os::getcwd();
    for exe in compile.tests.iter() {
        let to_display = match exe.path_relative_from(&cwd) {
            Some(path) => path,
            None => exe.clone(),
        };
        let cmd = compile.process(exe).args(args);
        try!(options.shell.concise(|shell| {
            shell.status("Running", to_display.display().to_string())
        }));
        try!(options.shell.verbose(|shell| {
            shell.status("Running", cmd.to_string())
        }));
        match cmd.exec() {
            Ok(()) => {}
            Err(e) => return Ok(Some(e))
        }
    }

    let mut libs = package.get_targets().iter().filter_map(|target| {
        if !target.get_profile().is_doctest() || !target.is_lib() {
            return None
        }
        Some((target.get_src_path(), target.get_name()))
    });

    for (lib, name) in libs {
        try!(options.shell.status("Doc-tests", name));
        let mut p = compile.process("rustdoc")
                           .arg("--test").arg(lib)
                           .arg("--crate-name").arg(name)
                           .arg("-L").arg("target/test")
                           .arg("-L").arg("target/test/deps")
                           .cwd(package.get_root());

        // FIXME(rust-lang/rust#16272): this should just always be passed.
        if args.len() > 0 {
            p = p.arg("--test-args").arg(args.connect(" "));
        }

        for (pkg, libs) in compile.libraries.iter() {
            for lib in libs.iter() {
                let mut arg = pkg.get_name().as_bytes().to_vec();
                arg.push(b'=');
                arg.push_all(lib.as_vec());
                p = p.arg("--extern").arg(arg.as_slice());
            }
        }

        try!(options.shell.verbose(|shell| {
            shell.status("Running", p.to_string())
        }));
        match p.exec() {
            Ok(()) => {}
            Err(e) => return Ok(Some(e)),
        }
    }

    Ok(None)
}

pub fn run_benches(manifest_path: &Path,
                   options: &mut ops::CompileOptions,
                   args: &[String]) -> CargoResult<Option<ProcessError>> {
    let mut args = args.to_vec();
    args.push("--bench".to_string());

    run_tests(manifest_path, options, args.as_slice())
}
