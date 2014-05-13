use std::fmt;
use std::fmt::{Show,Formatter};
use std::os;
use std::path::Path;
use std::io::process::{Process,ProcessConfig,ProcessOutput,InheritFd};
use util::{CargoResult,io_error,process_error};
use collections::HashMap;

#[deriving(Clone,Eq)]
pub struct ProcessBuilder {
    program: ~str,
    args: Vec<~str>,
    path: Vec<~str>,
    env: HashMap<~str, ~str>,
    cwd: Path
}

impl Show for ProcessBuilder {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f.buf, "`{}", self.program));

        if self.args.len() > 0 {
            try!(write!(f.buf, " {}", self.args.connect(" ")));
        }

        write!(f.buf, "`")
    }
}

// TODO: Upstream a Windows/Posix branch to Rust proper
static PATH_SEP : &'static str = ":";

impl ProcessBuilder {
    pub fn args(mut self, arguments: &[~str]) -> ProcessBuilder {
        self.args = Vec::from_slice(arguments);
        self
    }

    pub fn get_args<'a>(&'a self) -> &'a [~str] {
        self.args.as_slice()
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
    pub fn exec(&self) -> CargoResult<()> {
        let mut config = try!(self.build_config());
        let env = self.build_env();

        // Set where the output goes
        config.env = Some(env.as_slice());
        config.stdout = InheritFd(1);
        config.stderr = InheritFd(2);

        let mut process = try!(Process::configure(config).map_err(io_error));
        let exit = process.wait();

        if exit.success() {
            Ok(())
        } else {
            let msg = format!("Could not execute process `{}`", self.debug_string());
            Err(process_error(msg, exit, None))
        }
    }

    pub fn exec_with_output(&self) -> CargoResult<ProcessOutput> {
        let mut config = try!(self.build_config());
        let env = self.build_env();

        config.env = Some(env.as_slice());

        let output = try!(Process::configure(config).map(|mut ok| ok.wait_with_output()).map_err(io_error));

        if output.status.success() {
            Ok(output)
        } else {
            let msg = format!("Could not execute process `{}`", self.debug_string());
            Err(process_error(msg, output.status.clone(), Some(output)))
        }
    }

    fn build_config<'a>(&'a self) -> CargoResult<ProcessConfig<'a>> {
        let mut config = ProcessConfig::new();

        config.program = self.program.as_slice();
        config.args = self.args.as_slice();
        config.cwd = Some(&self.cwd);

        Ok(config)
    }

    fn debug_string(&self) -> ~str {
        format!("{} {}", self.program, self.args.connect(" "))
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
