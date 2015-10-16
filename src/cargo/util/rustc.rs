use std::path::Path;

use util::{self, CargoResult, internal, ChainError};

pub struct Rustc {
    pub verbose_version: String,
    pub host: String,
    pub cap_lints: bool,
}

impl Rustc {
    /// Run the compiler at `path` to learn varioues pieces of information about
    /// it.
    ///
    /// If successful this function returns a description of the compiler along
    /// with a list of its capabilities.
    pub fn new<P: AsRef<Path>>(path: P, cwd: &Path) -> CargoResult<Rustc> {
        let mut cmd = try!(util::process(path.as_ref(), cwd));
        cmd.arg("-vV");

        let mut ret = Rustc::blank();
        let mut first = cmd.clone();
        first.arg("--cap-lints").arg("allow");
        let output = match first.exec_with_output() {
            Ok(output) => { ret.cap_lints = true; output }
            Err(..) => try!(cmd.exec_with_output()),
        };
        ret.verbose_version = try!(String::from_utf8(output.stdout).map_err(|_| {
            internal("rustc -v didn't return utf8 output")
        }));
        ret.host = {
            let triple = ret.verbose_version.lines().filter(|l| {
                l.starts_with("host: ")
            }).map(|l| &l[6..]).next();
            let triple = try!(triple.chain_error(|| {
                internal("rustc -v didn't have a line for `host:`")
            }));
            triple.to_string()
        };
        Ok(ret)
    }

    pub fn blank() -> Rustc {
        Rustc {
            verbose_version: String::new(),
            host: String::new(),
            cap_lints: false,
        }
    }
}
