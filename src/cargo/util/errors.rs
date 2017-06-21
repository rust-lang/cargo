use std::error::Error;
use std::fmt;
use std::io;
use std::num;
use std::process::{Output, ExitStatus};
use std::str;
use std::string;

use core::TargetKind;

use curl;
use git2;
use semver;
use serde_json;
use toml;
use registry;

error_chain! {
    types {
        CargoError, CargoErrorKind, CargoResultExt, CargoResult;
    }

    links {
        CrateRegistry(registry::Error, registry::ErrorKind);
    }

    foreign_links {
        ParseSemver(semver::ReqParseError);
        Semver(semver::SemVerError);
        Io(io::Error);
        SerdeJson(serde_json::Error);
        TomlSer(toml::ser::Error);
        TomlDe(toml::de::Error);
        ParseInt(num::ParseIntError);
        ParseBool(str::ParseBoolError);
        Parse(string::ParseError);
        Git(git2::Error);
        Curl(curl::Error);
    }

    errors {
        Internal(err: Box<CargoErrorKind>) {
            description(err.description())
            display("{}", *err)
        }
        ProcessErrorKind(proc_err: ProcessError) {
            description(&proc_err.desc)
            display("{}", &proc_err.desc)
        }
        CargoTestErrorKind(test_err: CargoTestError) {
            description(&test_err.desc)
            display("{}", &test_err.desc)
        }
        HttpNot200(code: u32, url: String) {
            description("failed to get a 200 response")
            display("failed to get 200 response from `{}`, got {}", url, code)
        }
    }
}

impl CargoError {
    pub fn into_internal(self) -> Self {
        CargoError(CargoErrorKind::Internal(Box::new(self.0)), self.1)
    }

    fn is_human(&self) -> bool {
        match &self.0 {
            &CargoErrorKind::Msg(_) => true,
            &CargoErrorKind::TomlSer(_) => true,
            &CargoErrorKind::TomlDe(_) => true,
            &CargoErrorKind::Curl(_) => true,
            &CargoErrorKind::HttpNot200(..) => true,
            &CargoErrorKind::ProcessErrorKind(_) => true,
            &CargoErrorKind::CrateRegistry(_) => true,
            &CargoErrorKind::ParseSemver(_) |
            &CargoErrorKind::Semver(_) |
            &CargoErrorKind::Io(_) |
            &CargoErrorKind::SerdeJson(_) |
            &CargoErrorKind::ParseInt(_) |
            &CargoErrorKind::ParseBool(_) |
            &CargoErrorKind::Parse(_) |
            &CargoErrorKind::Git(_) |
            &CargoErrorKind::Internal(_) |
            &CargoErrorKind::CargoTestErrorKind(_) => false
        }
    }
}


// =============================================================================
// Process errors
#[derive(Debug)]
pub struct ProcessError {
    pub desc: String,
    pub exit: Option<ExitStatus>,
    pub output: Option<Output>,
}

// =============================================================================
// Cargo test errors.

/// Error when testcases fail
#[derive(Debug)]
pub struct CargoTestError {
    pub test: Test,
    pub desc: String,
    pub exit: Option<ExitStatus>,
    pub causes: Vec<ProcessError>,
}

#[derive(Debug)]
pub enum Test {
    Multiple,
    Doc,
    UnitTest(TargetKind, String)
}

impl CargoTestError {
    pub fn new(test: Test, errors: Vec<ProcessError>) -> Self {
        if errors.is_empty() {
            panic!("Cannot create CargoTestError from empty Vec")
        }
        let desc = errors.iter().map(|error| error.desc.clone())
                                .collect::<Vec<String>>()
                                .join("\n");
        CargoTestError {
            test: test,
            desc: desc,
            exit: errors[0].exit,
            causes: errors,
        }
    }

    pub fn hint(&self) -> String {
        match &self.test {
            &Test::UnitTest(ref kind, ref name) => {
                match *kind {
                    TargetKind::Bench => format!("test failed, to rerun pass '--bench {}'", name),
                    TargetKind::Bin => format!("test failed, to rerun pass '--bin {}'", name),
                    TargetKind::Lib(_) => "test failed, to rerun pass '--lib'".into(),
                    TargetKind::Test => format!("test failed, to rerun pass '--test {}'", name),
                    TargetKind::ExampleBin | TargetKind::ExampleLib(_) =>
                        format!("test failed, to rerun pass '--example {}", name),
                    _ => "test failed.".into()
                }
            },
            &Test::Doc => "test failed, to rerun pass '--doc'".into(),
            _ => "test failed.".into()
        }
    }
}

// =============================================================================
// CLI errors

pub type CliResult = Result<(), CliError>;

#[derive(Debug)]
pub struct CliError {
    pub error: Option<CargoError>,
    pub unknown: bool,
    pub exit_code: i32
}

impl Error for CliError {
    fn description(&self) -> &str {
        self.error.as_ref().map(|e| e.description())
            .unwrap_or("unknown cli error")
    }

    fn cause(&self) -> Option<&Error> {
        self.error.as_ref().and_then(|e| e.cause())
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref error) = self.error {
            error.fmt(f)
        } else {
            self.description().fmt(f)
        }
    }
}

impl CliError {
    pub fn new(error: CargoError, code: i32) -> CliError {
        let human = &error.is_human();
        CliError { error: Some(error), exit_code: code, unknown: !human }
    }

    pub fn code(code: i32) -> CliError {
        CliError { error: None, exit_code: code, unknown: false }
    }
}

impl From<CargoError> for CliError {
    fn from(err: CargoError) -> CliError {
        CliError::new(err, 101)
    }
}


// =============================================================================
// Construction helpers

pub fn process_error(msg: &str,
                     status: Option<&ExitStatus>,
                     output: Option<&Output>) -> ProcessError
{
    let exit = match status {
        Some(s) => status_to_string(s),
        None => "never executed".to_string(),
    };
    let mut desc = format!("{} ({})", &msg, exit);

    if let Some(out) = output {
        match str::from_utf8(&out.stdout) {
            Ok(s) if !s.trim().is_empty() => {
                desc.push_str("\n--- stdout\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
        match str::from_utf8(&out.stderr) {
            Ok(s) if !s.trim().is_empty() => {
                desc.push_str("\n--- stderr\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
    }

    return ProcessError {
        desc: desc,
        exit: status.cloned(),
        output: output.cloned(),
    };

    #[cfg(unix)]
    fn status_to_string(status: &ExitStatus) -> String {
        use std::os::unix::process::*;
        use libc;

        if let Some(signal) = status.signal() {
            let name = match signal as libc::c_int {
                libc::SIGABRT => ", SIGABRT: process abort signal",
                libc::SIGALRM => ", SIGALRM: alarm clock",
                libc::SIGFPE => ", SIGFPE: erroneous arithmetic operation",
                libc::SIGHUP => ", SIGHUP: hangup",
                libc::SIGILL => ", SIGILL: illegal instruction",
                libc::SIGINT => ", SIGINT: terminal interrupt signal",
                libc::SIGKILL => ", SIGKILL: kill",
                libc::SIGPIPE => ", SIGPIPE: write on a pipe with no one to read",
                libc::SIGQUIT => ", SIGQUIT: terminal quite signal",
                libc::SIGSEGV => ", SIGSEGV: invalid memory reference",
                libc::SIGTERM => ", SIGTERM: termination signal",
                libc::SIGBUS => ", SIGBUS: access to undefined memory",
                #[cfg(not(target_os = "haiku"))]
                libc::SIGSYS => ", SIGSYS: bad system call",
                libc::SIGTRAP => ", SIGTRAP: trace/breakpoint trap",
                _ => "",
            };
            format!("signal: {}{}", signal, name)
        } else {
            status.to_string()
        }
    }

    #[cfg(windows)]
    fn status_to_string(status: &ExitStatus) -> String {
        status.to_string()
    }
}

pub fn internal<S: fmt::Display>(error: S) -> CargoError {
    _internal(&error)
}

fn _internal(error: &fmt::Display) -> CargoError {
    CargoError::from_kind(error.to_string().into()).into_internal()
}
