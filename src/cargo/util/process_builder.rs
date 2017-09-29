use std::collections::HashMap;
use std::env;
use std::ffi::{OsString, OsStr};
use std::fmt;
use std::path::Path;
use std::process::{Command, Stdio, Output};

use jobserver::Client;
use shell_escape::escape;

use util::{CargoResult, CargoResultExt, CargoError, process_error, read2};
use util::errors::CargoErrorKind;

/// A builder object for an external process, similar to `std::process::Command`.
#[derive(Clone, Debug)]
pub struct ProcessBuilder {
    /// The program to execute.
    program: OsString,
    /// A list of arguments to pass to the program.
    args: Vec<OsString>,
    /// Any environment variables that should be set for the program.
    env: HashMap<String, Option<OsString>>,
    /// Which directory to run the program from.
    cwd: Option<OsString>,
    /// The `make` jobserver. See the [jobserver crate][jobserver_docs] for
    /// more information.
    ///
    /// [jobserver_docs]: https://docs.rs/jobserver/0.1.6/jobserver/
    jobserver: Option<Client>,
}

impl fmt::Display for ProcessBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "`{}", self.program.to_string_lossy())?;

        for arg in &self.args {
            write!(f, " {}", escape(arg.to_string_lossy()))?;
        }

        write!(f, "`")
    }
}

impl ProcessBuilder {
    /// (chainable) Set the executable for the process.
    pub fn program<T: AsRef<OsStr>>(&mut self, program: T) -> &mut ProcessBuilder {
        self.program = program.as_ref().to_os_string();
        self
    }

    /// (chainable) Add an arg to the args list.
    pub fn arg<T: AsRef<OsStr>>(&mut self, arg: T) -> &mut ProcessBuilder {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    /// (chainable) Add many args to the args list.
    pub fn args<T: AsRef<OsStr>>(&mut self, arguments: &[T]) -> &mut ProcessBuilder {
        self.args.extend(arguments.iter().map(|t| {
            t.as_ref().to_os_string()
        }));
        self
    }

    /// (chainable) Replace args with new args list
    pub fn args_replace<T: AsRef<OsStr>>(&mut self, arguments: &[T]) -> &mut ProcessBuilder {
        self.args = arguments.iter().map(|t| {
            t.as_ref().to_os_string()
        }).collect();
        self
    }

    /// (chainable) Set the current working directory of the process
    pub fn cwd<T: AsRef<OsStr>>(&mut self, path: T) -> &mut ProcessBuilder {
        self.cwd = Some(path.as_ref().to_os_string());
        self
    }

    /// (chainable) Set an environment variable for the process.
    pub fn env<T: AsRef<OsStr>>(&mut self, key: &str,
                                val: T) -> &mut ProcessBuilder {
        self.env.insert(key.to_string(), Some(val.as_ref().to_os_string()));
        self
    }

    /// (chainable) Unset an environment variable for the process.
    pub fn env_remove(&mut self, key: &str) -> &mut ProcessBuilder {
        self.env.insert(key.to_string(), None);
        self
    }

    /// Get the executable name.
    pub fn get_program(&self) -> &OsString {
        &self.program
    }

    /// Get the program arguments
    pub fn get_args(&self) -> &[OsString] {
        &self.args
    }

    /// Get the current working directory for the process
    pub fn get_cwd(&self) -> Option<&Path> {
        self.cwd.as_ref().map(Path::new)
    }

    /// Get an environment variable as the process will see it (will inherit from environment
    /// unless explicitally unset).
    pub fn get_env(&self, var: &str) -> Option<OsString> {
        self.env.get(var).cloned().or_else(|| Some(env::var_os(var)))
            .and_then(|s| s)
    }

    /// Get all environment variables explicitally set or unset for the process (not inherited
    /// vars).
    pub fn get_envs(&self) -> &HashMap<String, Option<OsString>> { &self.env }

    /// Set the `make` jobserver. See the [jobserver crate][jobserver_docs] for
    /// more information.
    ///
    /// [jobserver_docs]: https://docs.rs/jobserver/0.1.6/jobserver/
    pub fn inherit_jobserver(&mut self, jobserver: &Client) -> &mut Self {
        self.jobserver = Some(jobserver.clone());
        self
    }

    /// Run the process, waiting for completion, and mapping non-success exit codes to an error.
    pub fn exec(&self) -> CargoResult<()> {
        let mut command = self.build_command();
        let exit = command.status().chain_err(|| {
            CargoErrorKind::ProcessErrorKind(
                process_error(&format!("could not execute process `{}`",
                                   self.debug_string()), None, None))
        })?;

        if exit.success() {
            Ok(())
        } else {
            Err(CargoErrorKind::ProcessErrorKind(process_error(
                &format!("process didn't exit successfully: `{}`", self.debug_string()),
                Some(&exit), None)).into())
        }
    }

    /// On unix, executes the process using the unix syscall `execvp`, which will block this
    /// process, and will only return if there is an error. On windows this is a synonym for
    /// `exec`.
    #[cfg(unix)]
    pub fn exec_replace(&self) -> CargoResult<()> {
        use std::os::unix::process::CommandExt;

        let mut command = self.build_command();
        let error = command.exec();
        Err(CargoError::with_chain(error,
            CargoErrorKind::ProcessErrorKind(process_error(
                &format!("could not execute process `{}`", self.debug_string()), None, None))))
    }

    /// On unix, executes the process using the unix syscall `execvp`, which will block this
    /// process, and will only return if there is an error. On windows this is a synonym for
    /// `exec`.
    #[cfg(windows)]
    pub fn exec_replace(&self) -> CargoResult<()> {
        self.exec()
    }

    /// Execute the process, returning the stdio output, or an error if non-zero exit status.
    pub fn exec_with_output(&self) -> CargoResult<Output> {
        let mut command = self.build_command();

        let output = command.output().chain_err(|| {
            CargoErrorKind::ProcessErrorKind(
                process_error(
                    &format!("could not execute process `{}`", self.debug_string()),
                          None, None))
        })?;

        if output.status.success() {
            Ok(output)
        } else {
            Err(CargoErrorKind::ProcessErrorKind(process_error(
                &format!("process didn't exit successfully: `{}`", self.debug_string()),
                Some(&output.status), Some(&output))).into())
        }
    }

    /// Execute a command, passing each line of stdout and stderr to the supplied callbacks, which
    /// can mutate the string data.
    ///
    /// If any invocations of these function return an error, it will be propagated.
    ///
    /// Optionally, output can be passed to errors using `print_output`
    pub fn exec_with_streaming(&self,
                               on_stdout_line: &mut FnMut(&str) -> CargoResult<()>,
                               on_stderr_line: &mut FnMut(&str) -> CargoResult<()>,
                               print_output: bool)
                               -> CargoResult<Output> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let mut cmd = self.build_command();
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        let mut callback_error = None;
        let status = (|| {
            let mut child = cmd.spawn()?;
            let out = child.stdout.take().unwrap();
            let err = child.stderr.take().unwrap();
            read2(out, err, &mut |is_out, data, eof| {
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
            })?;
            child.wait()
        })().chain_err(|| {
            CargoErrorKind::ProcessErrorKind(
                process_error(&format!("could not execute process `{}`",
                    self.debug_string()),
                None, None))
        })?;
        let output = Output {
            stdout: stdout,
            stderr: stderr,
            status: status,
        };

        {
            let to_print = if print_output {
                Some(&output)
            } else {
                None
            };
            if !output.status.success() {
                return Err(CargoErrorKind::ProcessErrorKind(process_error(
                            &format!("process didn't exit successfully: `{}`", self.debug_string()),
                            Some(&output.status), to_print)).into())
            } else if let Some(e) = callback_error {
                return Err(CargoError::with_chain(e,
                        CargoErrorKind::ProcessErrorKind(process_error(
                                &format!("failed to parse process output: `{}`", self.debug_string()),
                                Some(&output.status), to_print))))
            }
        }

        Ok(output)
    }

    /// Converts ProcessBuilder into a `std::process::Command`, and handles the jobserver if
    /// present.
    pub fn build_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        if let Some(cwd) = self.get_cwd() {
            command.current_dir(cwd);
        }
        for arg in &self.args {
            command.arg(arg);
        }
        for (k, v) in &self.env {
            match *v {
                Some(ref v) => { command.env(k, v); }
                None => { command.env_remove(k); }
            }
        }
        if let Some(ref c) = self.jobserver {
            c.configure(&mut command);
        }
        command
    }

    /// Get the command line for the process as a string.
    fn debug_string(&self) -> String {
        let mut program = format!("{}", self.program.to_string_lossy());
        for arg in &self.args {
            program.push(' ');
            program.push_str(&format!("{}", arg.to_string_lossy()));
        }
        program
    }
}

/// A helper function to create a ProcessBuilder.
pub fn process<T: AsRef<OsStr>>(cmd: T) -> ProcessBuilder {
    ProcessBuilder {
        program: cmd.as_ref().to_os_string(),
        args: Vec::new(),
        cwd: None,
        env: HashMap::new(),
        jobserver: None,
    }
}
