use std::collections::HashMap;
use std::env;
use std::ffi::CString;
use std::fmt::{self, Formatter};
use std::old_io::process::{Command, ProcessOutput, InheritFd};
use std::old_path::BytesContainer;

use util::{CargoResult, ProcessError, process_error};

#[derive(Clone, PartialEq, Debug)]
pub struct ProcessBuilder {
    program: CString,
    args: Vec<CString>,
    env: HashMap<String, Option<CString>>,
    cwd: Path,
}

impl fmt::Display for ProcessBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "`{}", String::from_utf8_lossy(self.program.as_bytes())));

        for arg in self.args.iter() {
            try!(write!(f, " {}", String::from_utf8_lossy(arg.as_bytes())));
        }

        write!(f, "`")
    }
}

impl ProcessBuilder {
    pub fn arg<T: BytesContainer>(mut self, arg: T) -> ProcessBuilder {
        self.args.push(CString::from_slice(arg.container_as_bytes()));
        self
    }

    pub fn args<T: BytesContainer>(mut self, arguments: &[T]) -> ProcessBuilder {
        self.args.extend(arguments.iter().map(|t| {
            CString::from_slice(t.container_as_bytes())
        }));
        self
    }

    pub fn get_args(&self) -> &[CString] {
        &self.args
    }

    pub fn cwd(mut self, path: Path) -> ProcessBuilder {
        self.cwd = path;
        self
    }

    pub fn env<T: BytesContainer>(mut self, key: &str,
                                  val: Option<T>) -> ProcessBuilder {
        let val = val.map(|t| CString::from_slice(t.container_as_bytes()));
        self.env.insert(key.to_string(), val);
        self
    }

    // TODO: should InheritFd be hardcoded?
    pub fn exec(&self) -> Result<(), ProcessError> {
        let mut command = self.build_command();
        command.stdout(InheritFd(1))
               .stderr(InheritFd(2))
               .stdin(InheritFd(0));

        let exit = try!(command.status().map_err(|e| {
            process_error(&format!("Could not execute process `{}`",
                                   self.debug_string()),
                          Some(e), None, None)
        }));

        if exit.success() {
            Ok(())
        } else {
            Err(process_error(&format!("Process didn't exit successfully: `{}`",
                                       self.debug_string()),
                              None, Some(&exit), None))
        }
    }

    pub fn exec_with_output(&self) -> Result<ProcessOutput, ProcessError> {
        let command = self.build_command();

        let output = try!(command.output().map_err(|e| {
            process_error(&format!("Could not execute process `{}`",
                               self.debug_string()),
                          Some(e), None, None)
        }));

        if output.status.success() {
            Ok(output)
        } else {
            Err(process_error(&format!("Process didn't exit successfully: `{}`",
                                       self.debug_string()),
                              None, Some(&output.status), Some(&output)))
        }
    }

    pub fn build_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.cwd(&self.cwd);
        for arg in self.args.iter() {
            command.arg(arg);
        }
        for (k, v) in self.env.iter() {
            match *v {
                Some(ref v) => { command.env(k, v); }
                None => { command.env_remove(k); }
            }
        }
        command
    }

    fn debug_string(&self) -> String {
        let mut program = format!("{}", String::from_utf8_lossy(self.program.as_bytes()));
        for arg in self.args.iter() {
            program.push(' ');
            program.push_str(&format!("{}", String::from_utf8_lossy(arg.as_bytes()))[]);
        }
        program
    }
}

pub fn process<T: BytesContainer>(cmd: T) -> CargoResult<ProcessBuilder> {
    Ok(ProcessBuilder {
        program: CString::from_slice(cmd.container_as_bytes()),
        args: Vec::new(),
        cwd: try!(env::current_dir()),
        env: HashMap::new(),
    })
}
