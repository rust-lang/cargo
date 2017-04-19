use std::path::PathBuf;

use util::{self, CargoResult, internal, ChainError, ProcessBuilder};

pub struct Rustc {
    pub path: PathBuf,
    pub wrapper: Option<PathBuf>,
    pub verbose_version: String,
    pub host: String,
}

impl Rustc {
    /// Run the compiler at `path` to learn various pieces of information about
    /// it.
    ///
    /// If successful this function returns a description of the compiler along
    /// with a list of its capabilities.
    pub fn new(path: PathBuf, wrapper: Option<PathBuf>) -> CargoResult<Rustc> {
        let mut cmd = util::process(&path);
        cmd.arg("-vV");

        let output = cmd.exec_with_output()?;

        let verbose_version = String::from_utf8(output.stdout).map_err(|_| {
            internal("rustc -v didn't return utf8 output")
        })?;

        let host = {
            let triple = verbose_version.lines().find(|l| {
                l.starts_with("host: ")
            }).map(|l| &l[6..]);
            let triple = triple.chain_error(|| {
                internal("rustc -v didn't have a line for `host:`")
            })?;
            triple.to_string()
        };

        Ok(Rustc {
            path: path,
            wrapper: wrapper,
            verbose_version: verbose_version,
            host: host,
        })
    }

    pub fn process(&self) -> ProcessBuilder {
        if let Some(ref wrapper) = self.wrapper {
            let mut cmd = util::process(wrapper);
            {
                cmd.arg(&self.path);
            }
            cmd
        } else {
            util::process(&self.path)
        }
    }
}
