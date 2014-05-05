use std::os;
use std::path::Path;
use std::io;
use std::io::process::{Process,ProcessConfig,ProcessOutput,InheritFd};
use collections::HashMap;
use ToCargoError;
use CargoResult;

#[deriving(Clone,Eq)]
pub struct ProcessBuilder {
    program: ~str,
    args: Vec<~str>,
    path: Vec<~str>,
    env: HashMap<~str, ~str>,
    cwd: Path
}

// TODO: Upstream a Windows/Posix branch to Rust proper
static PATH_SEP : &'static str = ":";

impl ProcessBuilder {
    pub fn args(mut self, arguments: &[~str]) -> ProcessBuilder {
        self.args = Vec::from_slice(arguments);
        self
    }

    pub fn extra_path(mut self, path: Path) -> ProcessBuilder {
        // For now, just convert to a string, but we should do something better
        self.path.push(format!("{}", path.display()));
        self
    }

    pub fn cwd(mut self, path: Path) -> ProcessBuilder {
        self.cwd = path;
        self
    }

    // TODO: should InheritFd be hardcoded?
    pub fn exec(&self) -> io::IoResult<()> {
        let mut config = try!(self.build_config());
        let env = self.build_env();

        // Set where the output goes
        config.env = Some(env.as_slice());
        config.stdout = InheritFd(1);
        config.stderr = InheritFd(2);

        let mut process = try!(Process::configure(config));
        let exit = process.wait();

        if exit.success() {
            Ok(())
        }
        else {
            Err(io::IoError {
                kind: io::OtherIoError,
                desc: "process did not exit successfully",
                detail: None
            })
        }
    }

    // TODO: Match exec()
    pub fn exec_with_output(&self) -> CargoResult<ProcessOutput> {
        let mut config = ProcessConfig::new();

        config.program = self.program.as_slice();
        config.args = self.args.as_slice();
        config.cwd = Some(&self.cwd);

        let os_path = try!(os::getenv("PATH").to_cargo_error("Could not find the PATH environment variable".to_owned(), 1));
        let path = os_path + PATH_SEP + self.path.connect(PATH_SEP);

        let path = [("PATH".to_owned(), path)];
        config.env = Some(path.as_slice());

        Process::configure(config).map(|mut ok| ok.wait_with_output()).to_cargo_error("Could not spawn process".to_owned(), 1)
    }

    fn build_config<'a>(&'a self) -> io::IoResult<ProcessConfig<'a>> {
        let mut config = ProcessConfig::new();

        config.program = self.program.as_slice();
        config.args = self.args.as_slice();
        config.cwd = Some(&self.cwd);

        Ok(config)
    }

    fn build_env(&self) -> ~[(~str, ~str)] {
        let mut ret = Vec::new();

        for (key, val) in self.env.iter() {
            // Skip path
            if key.as_slice() != "PATH" {
                ret.push((key.clone(), val.clone()));
            }
        }

        match self.build_path() {
            Some(path) => ret.push(("PATH".to_owned(), path)),
            _ => ()
        }

        ret.as_slice().to_owned()
    }

    fn build_path(&self) -> Option<~str> {
        let path = self.path.connect(PATH_SEP);

        match self.env.find_equiv(&("PATH")) {
            Some(existing) => {
                if self.path.is_empty() {
                    Some(existing.to_owned())
                }
                else {
                    Some(existing.as_slice() + PATH_SEP + path)
                }
            }
            None => {
                if self.path.is_empty() {
                    None
                }
                else {
                    Some(path)
                }
            }
        }
    }
}

pub fn process(cmd: &str) -> ProcessBuilder {
    ProcessBuilder {
        program: cmd.to_owned(),
        args: vec!(),
        path: vec!(),
        cwd: os::getcwd(),
        env: system_env()
    }
}

fn system_env() -> HashMap<~str, ~str> {
    let mut ret = HashMap::new();

    for &(ref key, ref val) in os::env().iter() {
        ret.insert(key.clone(), val.clone());
    }

    ret
}
