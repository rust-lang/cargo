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
    args: Vec<CString>,
    env: HashMap<String, Option<CString>>,
    cwd: Path,
}

impl Show for ProcessBuilder {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "`{}", String::from_utf8_lossy(self.program.as_bytes_no_nul())));

        for arg in self.args.iter() {
            try!(write!(f, " {}", String::from_utf8_lossy(arg.as_bytes_no_nul())));
        }

        write!(f, "`")
    }
}

impl ProcessBuilder {
    pub fn arg<T: ToCStr>(mut self, arg: T) -> ProcessBuilder {
        self.args.push(arg.to_c_str());
        self
    }

    pub fn args<T: ToCStr>(mut self, arguments: &[T]) -> ProcessBuilder {
        self.args.extend(arguments.iter().map(|t| t.to_c_str()));
        self
    }

    pub fn get_args(&self) -> &[CString] {
        self.args.as_slice()
    }

    pub fn cwd(mut self, path: Path) -> ProcessBuilder {
        self.cwd = path;
        self
    }

    pub fn env<T: ToCStr>(mut self, key: &str, val: Option<T>) -> ProcessBuilder {
        self.env.insert(key.to_string(), val.map(|t| t.to_c_str()));
        self
    }

    // TODO: should InheritFd be hardcoded?
    pub fn exec(&self) -> Result<(), ProcessError> {
        let mut command = self.build_command();
        command.stdout(InheritFd(1))
               .stderr(InheritFd(2))
               .stdin(InheritFd(0));

        let exit = try!(command.status().map_err(|e| {
            process_error(format!("Could not execute process `{}`",
                                  self.debug_string()),
                          Some(e), None, None)
        }));

        if exit.success() {
            Ok(())
        } else {
            Err(process_error(format!("Process didn't exit successfully: `{}`",
                                      self.debug_string()),
                              None, Some(&exit), None))
        }
    }

    pub fn exec_with_output(&self) -> Result<ProcessOutput, ProcessError> {
        let command = self.build_command();

        let output = try!(command.output().map_err(|e| {
            process_error(format!("Could not execute process `{}`",
                                  self.debug_string()),
                          Some(e), None, None)
        }));

        if output.status.success() {
            Ok(output)
        } else {
            Err(process_error(format!("Process didn't exit successfully: `{}`",
                                      self.debug_string()),
                              None, Some(&output.status), Some(&output)))
        }
    }

    pub fn build_command(&self) -> Command {
        let mut command = Command::new(self.program.as_bytes_no_nul());
        command.cwd(&self.cwd);
        for arg in self.args.iter() {
            command.arg(arg.as_bytes_no_nul());
        }
        for (k, v) in self.env.iter() {
            let k = k.as_slice();
            match *v {
                Some(ref v) => { command.env(k, v.as_bytes_no_nul()); }
                None => { command.env_remove(k); }
            }
        }
        command
    }

    fn debug_string(&self) -> String {
        let program = String::from_utf8_lossy(self.program.as_bytes_no_nul());
        let mut program = program.into_string();
        for arg in self.args.iter() {
            program.push_char(' ');
            let s = String::from_utf8_lossy(arg.as_bytes_no_nul());
            program.push_str(s.as_slice());
        }
        program
    }
}

pub fn process<T: ToCStr>(cmd: T) -> ProcessBuilder {
    ProcessBuilder {
        program: cmd.to_c_str(),
        args: Vec::new(),
        cwd: os::getcwd(),
        env: HashMap::new(),
    }
}
