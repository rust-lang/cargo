use std::fmt;
use std::fmt::{Show, Formatter};
use std::os;
use std::c_str::CString;
use std::io::process::{Command, ProcessOutput, InheritFd};
use std::collections::HashMap;

use util::{ProcessError, process_error};

#[deriving(Clone,PartialEq)]
pub struct ProcessBuilder {
    program: CString,
    args: Vec<String>,
    path: Vec<String>,
    env: HashMap<String, String>,
    cwd: Path
}

impl Show for ProcessBuilder {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "`{}", self.program.as_str().unwrap_or("<not-utf8>")));

        if self.args.len() > 0 {
            try!(write!(f, " {}", self.args.connect(" ")));
        }

        write!(f, "`")
    }
}

// TODO: Upstream a Windows/Posix branch to Rust proper
#[cfg(unix)]
static PATH_SEP : &'static str = ":";
#[cfg(windows)]
static PATH_SEP : &'static str = ";";

impl ProcessBuilder {
    pub fn arg<T: Str>(mut self, arg: T) -> ProcessBuilder {
        self.args.push(arg.as_slice().to_string());
        self
    }

    pub fn args<T: Str>(mut self, arguments: &[T]) -> ProcessBuilder {
        self.args = arguments.iter().map(|a| a.as_slice().to_string()).collect();
        self
    }

    pub fn get_args<'a>(&'a self) -> &'a [String] {
        self.args.as_slice()
    }

    pub fn extra_path(mut self, path: Path) -> ProcessBuilder {
        // For now, just convert to a string, but we should do something better
        self.path.unshift(path.display().to_string());
        self
    }

    pub fn cwd(mut self, path: Path) -> ProcessBuilder {
        self.cwd = path;
        self
    }

    pub fn env(mut self, key: &str, val: Option<&str>) -> ProcessBuilder {
        match val {
            Some(v) => {
                self.env.insert(key.to_string(), v.to_string());
            },
            None => {
                self.env.remove(&key.to_string());
            }
        }

        self
    }

    // TODO: should InheritFd be hardcoded?
    pub fn exec(&self) -> Result<(), ProcessError> {
        let mut command = self.build_command();
        command
            .env(self.build_env().as_slice())
            .stdout(InheritFd(1))
            .stderr(InheritFd(2));

        let msg = || format!("Could not execute process `{}`",
                             self.debug_string());

        let exit = try!(command.status().map_err(|e|
            process_error(msg(), Some(e), None, None)));

        if exit.success() {
            Ok(())
        } else {
            Err(process_error(msg(), None, Some(&exit), None))
        }
    }

    pub fn exec_with_output(&self) -> Result<ProcessOutput, ProcessError> {
        let mut command = self.build_command();
        command.env(self.build_env().as_slice());

        let msg = || format!("Could not execute process `{}`",
                             self.debug_string());

        let output = try!(command.output().map_err(|e| {
            process_error(msg(), Some(e), None, None)
        }));

        if output.status.success() {
            Ok(output)
        } else {
            Err(process_error(msg(), None, Some(&output.status),
                              Some(&output)))
        }
    }

    pub fn build_command(&self) -> Command {
        let mut command = Command::new(self.program.as_bytes_no_nul());
        command.args(self.args.as_slice()).cwd(&self.cwd);
        command
    }

    fn debug_string(&self) -> String {
        let program = self.program.as_str().unwrap_or("<not-utf8>");
        if self.args.len() == 0 {
            program.to_string()
        } else {
            format!("{} {}", program, self.args.connect(" "))
        }
    }

    fn build_env(&self) -> Vec<(String, String)> {
        let mut ret = Vec::new();

        for (key, val) in self.env.iter() {
            // Skip path
            if key.as_slice() != "PATH" {
                ret.push((key.clone(), val.clone()));
            }
        }

        match self.build_path() {
            Some(path) => ret.push(("PATH".to_string(), path)),
            _ => ()
        }

        ret.as_slice().to_owned()
    }

    fn build_path(&self) -> Option<String> {
        let path = self.path.connect(PATH_SEP);

        match self.env.find_equiv(&("PATH")) {
            Some(existing) => {
                if self.path.is_empty() {
                    Some(existing.clone())
                } else {
                    Some(format!("{}{}{}", existing, PATH_SEP, path))
                }
            },
            None => {
                if self.path.is_empty() {
                    None
                } else {
                    Some(path)
                }
            }
        }
    }
}

pub fn process<T: ToCStr>(cmd: T) -> ProcessBuilder {
    ProcessBuilder {
        program: cmd.to_c_str(),
        args: vec!(),
        path: vec!(),
        cwd: os::getcwd(),
        env: os::env().move_iter().collect()
    }
}
