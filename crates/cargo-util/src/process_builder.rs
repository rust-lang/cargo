use crate::process_error::ProcessError;
use crate::read2;

use anyhow::{bail, Context, Result};
use jobserver::Client;
use shell_escape::escape;
use tempfile::NamedTempFile;

use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::iter::once;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Output, Stdio};

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
    /// A list of wrappers that wrap the original program when calling
    /// [`ProcessBuilder::wrapped`]. The last one is the outermost one.
    wrappers: Vec<OsString>,
    /// The `make` jobserver. See the [jobserver crate] for
    /// more information.
    ///
    /// [jobserver crate]: https://docs.rs/jobserver/
    jobserver: Option<Client>,
    /// `true` to include environment variable in display.
    display_env_vars: bool,
    /// `true` to retry with an argfile if hitting "command line too big" error.
    retry_with_argfile: bool,
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

        write!(f, "{}", self.get_program().to_string_lossy())?;

        for arg in self.get_args() {
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
            wrappers: Vec::new(),
            jobserver: None,
            display_env_vars: false,
            retry_with_argfile: false,
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
        if let Some(program) = self.wrappers.pop() {
            // User intend to replace all args, so we
            // - use the outermost wrapper as the main program, and
            // - cleanup other inner wrappers.
            self.program = program;
            self.wrappers = Vec::new();
        }
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
        self.wrappers.last().unwrap_or(&self.program)
    }

    /// Gets the program arguments.
    pub fn get_args(&self) -> impl Iterator<Item = &OsString> {
        self.wrappers
            .iter()
            .rev()
            .chain(once(&self.program))
            .chain(self.args.iter())
            .skip(1) // Skip the main `program
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

    /// Enables retrying with an argfile if hitting "command line too big" error
    pub fn retry_with_argfile(&mut self, enabled: bool) -> &mut Self {
        self.retry_with_argfile = enabled;
        self
    }

    fn should_retry_with_argfile(&self, err: &io::Error) -> bool {
        self.retry_with_argfile && imp::command_line_too_big(err)
    }

    /// Like [`Command::status`] but with a better error message.
    pub fn status(&self) -> Result<ExitStatus> {
        self.build_and_spawn(|_| {})
            .and_then(|mut child| child.wait())
            .with_context(|| {
                ProcessError::new(&format!("could not execute process {self}"), None, None)
            })
    }

    /// Runs the process, waiting for completion, and mapping non-success exit codes to an error.
    pub fn exec(&self) -> Result<()> {
        let exit = self.status()?;
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

    /// Like [`Command::output`] but with a better error message.
    pub fn output(&self) -> Result<Output> {
        self.build_and_spawn(|cmd| {
            cmd.stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null());
        })
        .and_then(|child| child.wait_with_output())
        .with_context(|| {
            ProcessError::new(&format!("could not execute process {self}"), None, None)
        })
    }

    /// Executes the process, returning the stdio output, or an error if non-zero exit status.
    pub fn exec_with_output(&self) -> Result<Output> {
        let output = self.output()?;
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

        let mut callback_error = None;
        let mut stdout_pos = 0;
        let mut stderr_pos = 0;
        let status = (|| {
            let mut child = self.build_and_spawn(|cmd| {
                cmd.stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .stdin(Stdio::null());
            })?;
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

    /// Builds a command from `ProcessBuilder` and spawn it.
    ///
    /// There is a risk when spawning a process, it might hit "command line
    /// too big" OS error. To handle those kind of OS errors, this method try
    /// to reinvoke the command with a `@<path>` argfile that contains all the
    /// arguments.
    ///
    /// * `apply`: Modify the command before invoking. Useful for updating [`Stdio`].
    fn build_and_spawn(&self, apply: impl Fn(&mut Command)) -> io::Result<Child> {
        let mut cmd = self.build_command();
        apply(&mut cmd);

        match cmd.spawn() {
            Err(ref e) if self.should_retry_with_argfile(e) => {
                let (mut cmd, _argfile) = self.build_command_with_argfile()?;
                apply(&mut cmd);
                cmd.spawn()
            }
            res => res,
        }
    }

    /// Builds the command with an `@<path>` argfile that contains all the
    /// arguments. This is primarily served for rustc/rustdoc command family.
    ///
    /// Ref:
    ///
    /// - https://doc.rust-lang.org/rustdoc/command-line-arguments.html#path-load-command-line-flags-from-a-path
    /// - https://doc.rust-lang.org/rustc/command-line-arguments.html#path-load-command-line-flags-from-a-path>
    fn build_command_with_argfile(&self) -> io::Result<(Command, NamedTempFile)> {
        use std::io::Write as _;

        let mut tmp = tempfile::Builder::new()
            .prefix("cargo-argfile.")
            .tempfile()?;

        let path = tmp.path().display();
        let mut cmd = self.build_command_without_args();
        cmd.arg(format!("@{path}"));
        log::debug!("created argfile at {path} for `{self}`");

        let cap = self.get_args().map(|arg| arg.len() + 1).sum::<usize>();
        let mut buf = String::with_capacity(cap);
        for arg in &self.args {
            let arg = arg
                .to_str()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        "argument contains invalid UTF-8 characters",
                    )
                })?;
            // TODO: Shall we escape line feed?
            buf.push_str(arg);
            buf.push('\n');
        }
        tmp.write_all(buf.as_bytes())?;
        Ok((cmd, tmp))
    }

    /// Builds a command from `ProcessBuilder` for everythings but not `args`.
    fn build_command_without_args(&self) -> Command {
        let mut command = {
            let mut iter = self.wrappers.iter().rev().chain(once(&self.program));
            let mut cmd = Command::new(iter.next().expect("at least one `program` exists"));
            cmd.args(iter);
            cmd
        };
        if let Some(cwd) = self.get_cwd() {
            command.current_dir(cwd);
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

    /// Converts `ProcessBuilder` into a `std::process::Command`, and handles
    /// the jobserver, if present.
    ///
    /// Note that this method doesn't take argfile fallback into account. The
    /// caller should handle it by themselves.
    pub fn build_command(&self) -> Command {
        let mut command = self.build_command_without_args();
        for arg in &self.args {
            command.arg(arg);
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
        if let Some(wrapper) = wrapper.as_ref() {
            let wrapper = wrapper.as_ref();
            if !wrapper.is_empty() {
                self.wrappers.push(wrapper.to_os_string());
            }
        }
        self
    }
}

#[cfg(unix)]
mod imp {
    use super::{ProcessBuilder, ProcessError};
    use anyhow::Result;
    use std::io;
    use std::os::unix::process::CommandExt;

    pub fn exec_replace(process_builder: &ProcessBuilder) -> Result<()> {
        let mut command = process_builder.build_command();

        let mut error = command.exec();
        if process_builder.should_retry_with_argfile(&error) {
            let (mut command, _argfile) = process_builder.build_command_with_argfile()?;
            error = command.exec()
        }

        Err(anyhow::Error::from(error).context(ProcessError::new(
            &format!("could not execute process {}", process_builder),
            None,
            None,
        )))
    }

    pub fn command_line_too_big(err: &io::Error) -> bool {
        err.raw_os_error() == Some(libc::E2BIG)
    }
}

#[cfg(windows)]
mod imp {
    use super::{ProcessBuilder, ProcessError};
    use anyhow::Result;
    use std::io;
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

    pub fn command_line_too_big(err: &io::Error) -> bool {
        use winapi::shared::winerror::ERROR_FILENAME_EXCED_RANGE;
        err.raw_os_error() == Some(ERROR_FILENAME_EXCED_RANGE as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn test_argfile() {
        let mut cmd = ProcessBuilder::new("echo");
        cmd.args(["foo", "bar"].as_slice());
        let (cmd, argfile) = cmd.build_command_with_argfile().unwrap();

        assert_eq!(cmd.get_program(), "echo");
        let cmd_args: Vec<_> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(cmd_args.len(), 1);
        assert!(cmd_args[0].starts_with("@"));
        assert!(cmd_args[0].contains("cargo-argfile."));

        let buf = fs::read_to_string(argfile.path()).unwrap();
        assert_eq!(buf, "foo\nbar\n");
    }
}
