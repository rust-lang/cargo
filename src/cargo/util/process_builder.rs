use std::collections::HashMap;
use std::env;
use std::ffi::{OsString, OsStr};
use std::fmt;
use std::path::Path;
use std::process::{Command, Stdio, Output};

use util::{CargoResult, ProcessError, process_error, read2};
use util::shell_escape::escape;

#[derive(Clone, PartialEq, Debug)]
pub struct ProcessBuilder {
    program: OsString,
    args: Vec<OsString>,
    env: HashMap<String, Option<OsString>>,
    cwd: Option<OsString>,
}

impl fmt::Display for ProcessBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "`{}", self.program.to_string_lossy()));

        for arg in self.args.iter() {
            try!(write!(f, " {}", escape(arg.to_string_lossy())));
        }

        write!(f, "`")
    }
}

impl ProcessBuilder {
    pub fn arg<T: AsRef<OsStr>>(&mut self, arg: T) -> &mut ProcessBuilder {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn args<T: AsRef<OsStr>>(&mut self, arguments: &[T]) -> &mut ProcessBuilder {
        self.args.extend(arguments.iter().map(|t| {
            t.as_ref().to_os_string()
        }));
        self
    }

    pub fn cwd<T: AsRef<OsStr>>(&mut self, path: T) -> &mut ProcessBuilder {
        self.cwd = Some(path.as_ref().to_os_string());
        self
    }

    pub fn env<T: AsRef<OsStr>>(&mut self, key: &str,
                                val: T) -> &mut ProcessBuilder {
        self.env.insert(key.to_string(), Some(val.as_ref().to_os_string()));
        self
    }

    pub fn env_remove(&mut self, key: &str) -> &mut ProcessBuilder {
        self.env.insert(key.to_string(), None);
        self
    }

    pub fn get_args(&self) -> &[OsString] {
        &self.args
    }

    pub fn get_cwd(&self) -> Option<&Path> {
        self.cwd.as_ref().map(Path::new)
    }

    pub fn get_env(&self, var: &str) -> Option<OsString> {
        self.env.get(var).cloned().or_else(|| Some(env::var_os(var)))
            .and_then(|s| s)
    }

    pub fn get_envs(&self) -> &HashMap<String, Option<OsString>> { &self.env }

    pub fn exec(&self) -> Result<(), ProcessError> {
        let mut command = self.build_command();
        let exit = try!(command.status().map_err(|e| {
            process_error(&format!("could not execute process `{}`",
                                   self.debug_string()),
                          Some(Box::new(e)), None, None)
        }));

        if exit.success() {
            Ok(())
        } else {
            Err(process_error(&format!("process didn't exit successfully: `{}`",
                                       self.debug_string()),
                              None, Some(&exit), None))
        }
    }

    #[cfg(unix)]
    pub fn exec_replace(&self) -> Result<(), ProcessError> {
        use std::os::unix::process::CommandExt;

        let mut command = self.build_command();
        let error = command.exec();
        Err(process_error(&format!("could not execute process `{}`",
                                   self.debug_string()),
                          Some(Box::new(error)), None, None))
    }

    #[cfg(windows)]
    pub fn exec_replace(&self) -> Result<(), ProcessError> {
        self.exec()
    }

    pub fn exec_with_output(&self) -> Result<Output, ProcessError> {
        let mut command = self.build_command();

        let output = try!(command.output().map_err(|e| {
            process_error(&format!("could not execute process `{}`",
                                   self.debug_string()),
                          Some(Box::new(e)), None, None)
        }));

        if output.status.success() {
            Ok(output)
        } else {
            Err(process_error(&format!("process didn't exit successfully: `{}`",
                                       self.debug_string()),
                              None, Some(&output.status), Some(&output)))
        }
    }

    pub fn exec_with_streaming(&self,
                               on_stdout_line: &mut FnMut(&str) -> CargoResult<()>,
                               on_stderr_line: &mut FnMut(&str) -> CargoResult<()>)
                               -> Result<Output, ProcessError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let mut cmd = self.build_command();
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        let mut callback_error = None;
        let status = try!((|| {
            let mut child = try!(cmd.spawn());
            let out = child.stdout.take().unwrap();
            let err = child.stderr.take().unwrap();
            try!(read2(out, err, &mut |is_out, data, eof| {
                let idx = if eof {
                    data.len()
                } else {
                    match data.iter().rposition(|b| *b == b'\n') {
                        Some(i) => i + 1,
                        None => return,
                    }
                };
                let data = data.drain(..idx);
                let dst = if is_out {&mut stdout} else {&mut stderr};
                let start = dst.len();
                dst.extend(data);
                for line in String::from_utf8_lossy(&dst[start..]).lines() {
                    if callback_error.is_some() { break }
                    let callback_result = if is_out {
                        on_stdout_line(line)
                    } else {
                        on_stderr_line(line)
                    };
                    if let Err(e) = callback_result {
                        callback_error = Some(e);
                    }
                }
            }));
            child.wait()
        })().map_err(|e| {
            process_error(&format!("could not execute process `{}`",
                                   self.debug_string()),
                          Some(Box::new(e)), None, None)
        }));
        let output = Output {
            stdout: stdout,
            stderr: stderr,
            status: status,
        };
        if !output.status.success() {
            Err(process_error(&format!("process didn't exit successfully: `{}`",
                                       self.debug_string()),
                              None, Some(&output.status), Some(&output)))
        } else if let Some(e) = callback_error {
            Err(process_error(&format!("failed to parse process output: `{}`",
                                       self.debug_string()),
                              Some(Box::new(e)), Some(&output.status), Some(&output)))
        } else {
            Ok(output)
        }
    }

    pub fn build_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        if let Some(cwd) = self.get_cwd() {
            command.current_dir(cwd);
        }
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
        let mut program = format!("{}", self.program.to_string_lossy());
        for arg in self.args.iter() {
            program.push(' ');
            program.push_str(&format!("{}", arg.to_string_lossy()));
        }
        program
    }
}

pub fn process<T: AsRef<OsStr>>(cmd: T) -> ProcessBuilder {
    ProcessBuilder {
        program: cmd.as_ref().to_os_string(),
        args: Vec::new(),
        cwd: None,
        env: HashMap::new(),
    }
}
