use std::path::PathBuf;

use util::{self, CargoResult, internal, ProcessBuilder};

/// Information on the `rustc` executable
#[derive(Debug)]
pub struct Rustc {
    /// The location of the exe
    pub path: PathBuf,
    /// An optional program that will be passed the path of the rust exe as its first argument, and
    /// rustc args following this.
    pub wrapper: Option<PathBuf>,
    /// Verbose version information (the output of `rustc -vV`)
    pub verbose_version: String,
    /// The host triple (arch-platform-OS), this comes from verbose_version.
    pub host: String,
}

impl Rustc {
    /// Run the compiler at `path` to learn various pieces of information about
    /// it, with an optional wrapper.
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
            }).map(|l| &l[6..]).ok_or_else(|| internal("rustc -v didn't have a line for `host:`"))?;
            triple.to_string()
        };

        Ok(Rustc {
            path: path,
            wrapper: wrapper,
            verbose_version: verbose_version,
            host: host,
        })
    }

    /// Get a process builder set up to use the found rustc version, with a wrapper if Some
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
