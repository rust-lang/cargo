use crate::process_error::ProcessError;
use crate::read2;
use anyhow::{bail, Context, Result};
use jobserver::Client;
use shell_escape::escape;
use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::iter::once;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// A builder object for an external process, similar to [`std::process::Command`].
#[derive(Clone, Debug)]
pub struct ProcessBuilder {
    /// The program to execute.
    program: OsString,
    /// A list of arguments to pass to the program.
    args: Vec<OsString>,
    /// Any environment variables that should be set for the program.
    env: BTreeMap<String, Option<OsString>>,
    /// The directory to run the program from.
    cwd: Option<OsString>,
    /// The `make` jobserver. See the [jobserver crate] for
    /// more information.
    ///
    /// [jobserver crate]: https://docs.rs/jobserver/
    jobserver: Option<Client>,
    /// `true` to include environment variable in display.
    display_env_vars: bool,
}

impl fmt::Display for ProcessBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`")?;

        if self.display_env_vars {
            for (key, val) in self.env.iter() {
                if let Some(val) = val {
                    let val = escape(val.to_string_lossy());
                    if cfg!(windows) {
                        write!(f, "set {}={}&& ", key, val)?;
                    } else {
                        write!(f, "{}={} ", key, val)?;
                    }
                }
            }
        }

        write!(f, "{}", self.program.to_string_lossy())?;

        for arg in &self.args {
            write!(f, " {}", escape(arg.to_string_lossy()))?;
        }

        write!(f, "`")
    }
}

impl ProcessBuilder {
    /// Creates a new [`ProcessBuilder`] with the given executable path.
    pub fn new<T: AsRef<OsStr>>(cmd: T) -> ProcessBuilder {
        ProcessBuilder {
            program: cmd.as_ref().to_os_string(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
            jobserver: None,
            display_env_vars: false,
        }
    }

    /// (chainable) Sets the executable for the process.
    pub fn program<T: AsRef<OsStr>>(&mut self, program: T) -> &mut ProcessBuilder {
        self.program = program.as_ref().to_os_string();
        self
    }

    /// (chainable) Adds `arg` to the args list.
    pub fn arg<T: AsRef<OsStr>>(&mut self, arg: T) -> &mut ProcessBuilder {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    /// (chainable) Adds multiple `args` to the args list.
    pub fn args<T: AsRef<OsStr>>(&mut self, args: &[T]) -> &mut ProcessBuilder {
        self.args
            .extend(args.iter().map(|t| t.as_ref().to_os_string()));
        self
    }

    /// (chainable) Replaces the args list with the given `args`.
    pub fn args_replace<T: AsRef<OsStr>>(&mut self, args: &[T]) -> &mut ProcessBuilder {
        self.args = args.iter().map(|t| t.as_ref().to_os_string()).collect();
        self
    }

    /// (chainable) Sets the current working directory of the process.
    pub fn cwd<T: AsRef<OsStr>>(&mut self, path: T) -> &mut ProcessBuilder {
        self.cwd = Some(path.as_ref().to_os_string());
        self
    }

    /// (chainable) Sets an environment variable for the process.
    pub fn env<T: AsRef<OsStr>>(&mut self, key: &str, val: T) -> &mut ProcessBuilder {
        self.env
            .insert(key.to_string(), Some(val.as_ref().to_os_string()));
        self
    }

    /// (chainable) Unsets an environment variable for the process.
    pub fn env_remove(&mut self, key: &str) -> &mut ProcessBuilder {
        self.env.insert(key.to_string(), None);
        self
    }

    /// Gets the executable name.
    pub fn get_program(&self) -> &OsString {
        &self.program
    }

    /// Gets the program arguments.
    pub fn get_args(&self) -> &[OsString] {
        &self.args
    }

    /// Gets the current working directory for the process.
    pub fn get_cwd(&self) -> Option<&Path> {
        self.cwd.as_ref().map(Path::new)
    }

    /// Gets an environment variable as the process will see it (will inherit from environment
    /// unless explicitally unset).
    pub fn get_env(&self, var: &str) -> Option<OsString> {
        self.env
            .get(var)
            .cloned()
            .or_else(|| Some(env::var_os(var)))
            .and_then(|s| s)
    }

    /// Gets all environment variables explicitly set or unset for the process (not inherited
    /// vars).
    pub fn get_envs(&self) -> &BTreeMap<String, Option<OsString>> {
        &self.env
    }

    /// Sets the `make` jobserver. See the [jobserver crate][jobserver_docs] for
    /// more information.
    ///
    /// [jobserver_docs]: https://docs.rs/jobserver/0.1.6/jobserver/
    pub fn inherit_jobserver(&mut self, jobserver: &Client) -> &mut Self {
        self.jobserver = Some(jobserver.clone());
        self
    }

    /// Enables environment variable display.
    pub fn display_env_vars(&mut self) -> &mut Self {
        self.display_env_vars = true;
        self
    }

    /// Runs the process, waiting for completion, and mapping non-success exit codes to an error.
    pub fn exec(&self) -> Result<()> {
        let mut command = self.build_command();
        let exit = command.status().with_context(|| {
            ProcessError::new(&format!("could not execute process {}", self), None, None)
        })?;

        if exit.success() {
            Ok(())
        } else {
            Err(ProcessError::new(
                &format!("process didn't exit successfully: {}", self),
                Some(exit),
                None,
            )
            .into())
        }
    }

    /// Replaces the current process with the target process.
    ///
    /// On Unix, this executes the process using the Unix syscall `execvp`, which will block
    /// this process, and will only return if there is an error.
    ///
    /// On Windows this isn't technically possible. Instead we emulate it to the best of our
    /// ability. One aspect we fix here is that we specify a handler for the Ctrl-C handler.
    /// In doing so (and by effectively ignoring it) we should emulate proxying Ctrl-C
    /// handling to the application at hand, which will either terminate or handle it itself.
    /// According to Microsoft's documentation at
    /// <https://docs.microsoft.com/en-us/windows/console/ctrl-c-and-ctrl-break-signals>.
    /// the Ctrl-C signal is sent to all processes attached to a terminal, which should
    /// include our child process. If the child terminates then we'll reap them in Cargo
    /// pretty quickly, and if the child handles the signal then we won't terminate
    /// (and we shouldn't!) until the process itself later exits.
    pub fn exec_replace(&self) -> Result<()> {
        imp::exec_replace(self)
    }

    /// Executes the process, returning the stdio output, or an error if non-zero exit status.
    pub fn exec_with_output(&self) -> Result<Output> {
        let mut command = self.build_command();

        let output = command.output().with_context(|| {
            ProcessError::new(&format!("could not execute process {}", self), None, None)
        })?;

        if output.status.success() {
            Ok(output)
        } else {
            Err(ProcessError::new(
                &format!("process didn't exit successfully: {}", self),
                Some(output.status),
                Some(&output),
            )
            .into())
        }
    }

    /// Executes a command, passing each line of stdout and stderr to the supplied callbacks, which
    /// can mutate the string data.
    ///
    /// If any invocations of these function return an error, it will be propagated.
    ///
    /// If `capture_output` is true, then all the output will also be buffered
    /// and stored in the returned `Output` object. If it is false, no caching
    /// is done, and the callbacks are solely responsible for handling the
    /// output.
    pub fn exec_with_streaming(
        &self,
        on_stdout_line: &mut dyn FnMut(&str) -> Result<()>,
        on_stderr_line: &mut dyn FnMut(&str) -> Result<()>,
        capture_output: bool,
    ) -> Result<Output> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let mut cmd = self.build_command();
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        let mut callback_error = None;
        let mut stdout_pos = 0;
        let mut stderr_pos = 0;
        let status = (|| {
            let mut child = cmd.spawn()?;
            let out = child.stdout.take().unwrap();
            let err = child.stderr.take().unwrap();
            read2(out, err, &mut |is_out, data, eof| {
                let pos = if is_out {
                    &mut stdout_pos
                } else {
                    &mut stderr_pos
                };
                let idx = if eof {
                    data.len()
                } else {
                    match data[*pos..].iter().rposition(|b| *b == b'\n') {
                        Some(i) => *pos + i + 1,
                        None => {
                            *pos = data.len();
                            return;
                        }
                    }
                };

                let new_lines = &data[..idx];

                for line in String::from_utf8_lossy(new_lines).lines() {
                    if callback_error.is_some() {
                        break;
                    }
                    let callback_result = if is_out {
                        on_stdout_line(line)
                    } else {
                        on_stderr_line(line)
                    };
                    if let Err(e) = callback_result {
                        callback_error = Some(e);
                        break;
                    }
                }

                if capture_output {
                    let dst = if is_out { &mut stdout } else { &mut stderr };
                    dst.extend(new_lines);
                }

                data.drain(..idx);
                *pos = 0;
            })?;
            child.wait()
        })()
        .with_context(|| {
            ProcessError::new(&format!("could not execute process {}", self), None, None)
        })?;
        let output = Output {
            status,
            stdout,
            stderr,
        };

        {
            let to_print = if capture_output { Some(&output) } else { None };
            if let Some(e) = callback_error {
                let cx = ProcessError::new(
                    &format!("failed to parse process output: {}", self),
                    Some(output.status),
                    to_print,
                );
                bail!(anyhow::Error::new(cx).context(e));
            } else if !output.status.success() {
                bail!(ProcessError::new(
                    &format!("process didn't exit successfully: {}", self),
                    Some(output.status),
                    to_print,
                ));
            }
        }

        Ok(output)
    }

    /// Converts `ProcessBuilder` into a `std::process::Command`, and handles the jobserver, if
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
                Some(ref v) => {
                    command.env(k, v);
                }
                None => {
                    command.env_remove(k);
                }
            }
        }
        if let Some(ref c) = self.jobserver {
            c.configure(&mut command);
        }
        command
    }

    /// Wraps an existing command with the provided wrapper, if it is present and valid.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cargo_util::ProcessBuilder;
    /// // Running this would execute `rustc`
    /// let cmd = ProcessBuilder::new("rustc");
    ///
    /// // Running this will execute `sccache rustc`
    /// let cmd = cmd.wrapped(Some("sccache"));
    /// ```
    pub fn wrapped(mut self, wrapper: Option<impl AsRef<OsStr>>) -> Self {
        let wrapper = if let Some(wrapper) = wrapper.as_ref() {
            wrapper.as_ref()
        } else {
            return self;
        };

        if wrapper.is_empty() {
            return self;
        }

        let args = once(self.program).chain(self.args.into_iter()).collect();

        self.program = wrapper.to_os_string();
        self.args = args;

        self
    }
}

#[cfg(unix)]
mod imp {
    use super::{ProcessBuilder, ProcessError};
    use anyhow::Result;
    use std::os::unix::process::CommandExt;

    pub fn exec_replace(process_builder: &ProcessBuilder) -> Result<()> {
        let mut command = process_builder.build_command();
        let error = command.exec();
        Err(anyhow::Error::from(error).context(ProcessError::new(
            &format!("could not execute process {}", process_builder),
            None,
            None,
        )))
    }
}

#[cfg(windows)]
mod imp {
    use super::{ProcessBuilder, ProcessError};
    use anyhow::Result;
    use winapi::shared::minwindef::{BOOL, DWORD, FALSE, TRUE};
    use winapi::um::consoleapi::SetConsoleCtrlHandler;

    unsafe extern "system" fn ctrlc_handler(_: DWORD) -> BOOL {
        // Do nothing; let the child process handle it.
        TRUE
    }

    pub fn exec_replace(process_builder: &ProcessBuilder) -> Result<()> {
        unsafe {
            if SetConsoleCtrlHandler(Some(ctrlc_handler), TRUE) == FALSE {
                return Err(ProcessError::new("Could not set Ctrl-C handler.", None, None).into());
            }
        }

        // Just execute the process as normal.
        process_builder.exec()
    }
}
