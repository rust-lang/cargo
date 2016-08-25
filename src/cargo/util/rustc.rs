use std::path::PathBuf;

use util::{self, CargoResult, internal, ChainError, ProcessBuilder};

pub struct Rustc {
    pub path: PathBuf,
    pub verbose_version: String,
    pub host: String,
    pub sysroot: PathBuf,
    /// Backwards compatibility: does this compiler support `--cap-lints` flag?
    pub cap_lints: bool,
}

impl Rustc {
    /// Run the compiler at `path` to learn various pieces of information about
    /// it.
    ///
    /// If successful this function returns a description of the compiler along
    /// with a list of its capabilities.
    pub fn new(path: PathBuf) -> CargoResult<Rustc> {
        let mut cmd = util::process(&path);
        cmd.arg("-vV");

        let mut first = cmd.clone();
        first.arg("--cap-lints").arg("allow");

        let (cap_lints, output) = match first.exec_with_output() {
            Ok(output) => (true, output),
            Err(..) => (false, try!(cmd.exec_with_output())),
        };

        let verbose_version = try!(String::from_utf8(output.stdout).map_err(|_| {
            internal("rustc -v didn't return UTF-8 output")
        }));

        let mut sysroot_raw = try!(util::process(&path).arg("--print").arg("sysroot")
                                   .exec_with_output()).stdout;
        // Trim final newline
        assert_eq!(sysroot_raw.pop(), Some(b'\n'));
        // What about invalid code sequences on Windows?
        let sysroot = From::from(try!(String::from_utf8(sysroot_raw).map_err(|_| {
            internal("rustc --print sysroot didn't not return UTF-8 output")
        })));

        let host = {
            let triple = verbose_version.lines().find(|l| {
                l.starts_with("host: ")
            }).map(|l| &l[6..]);
            let triple = try!(triple.chain_error(|| {
                internal("rustc -v didn't have a line for `host:`")
            }));
            triple.to_string()
        };

        Ok(Rustc {
            path: path,
            verbose_version: verbose_version,
            host: host,
            sysroot: sysroot,
            cap_lints: cap_lints,
        })
    }

    pub fn process(&self) -> ProcessBuilder {
        util::process(&self.path)
    }
}
