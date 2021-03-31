//! Error value for [`crate::ProcessBuilder`] when a process fails.

use std::fmt;
use std::process::{ExitStatus, Output};
use std::str;

#[derive(Debug)]
pub struct ProcessError {
    /// A detailed description to show to the user why the process failed.
    pub desc: String,

    /// The exit status of the process.
    ///
    /// This can be `None` if the process failed to launch (like process not
    /// found) or if the exit status wasn't a code but was instead something
    /// like termination via a signal.
    pub code: Option<i32>,

    /// The stdout from the process.
    ///
    /// This can be `None` if the process failed to launch, or the output was
    /// not captured.
    pub stdout: Option<Vec<u8>>,

    /// The stderr from the process.
    ///
    /// This can be `None` if the process failed to launch, or the output was
    /// not captured.
    pub stderr: Option<Vec<u8>>,
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.desc.fmt(f)
    }
}

impl std::error::Error for ProcessError {}

impl ProcessError {
    /// Creates a new [`ProcessError`].
    ///
    /// * `status` can be `None` if the process did not launch.
    /// * `output` can be `None` if the process did not launch, or output was not captured.
    pub fn new(msg: &str, status: Option<ExitStatus>, output: Option<&Output>) -> ProcessError {
        let exit = match status {
            Some(s) => exit_status_to_string(s),
            None => "never executed".to_string(),
        };

        Self::new_raw(
            msg,
            status.and_then(|s| s.code()),
            &exit,
            output.map(|s| s.stdout.as_slice()),
            output.map(|s| s.stderr.as_slice()),
        )
    }

    /// Creates a new [`ProcessError`] with the raw output data.
    ///
    /// * `code` can be `None` for situations like being killed by a signal on unix.
    pub fn new_raw(
        msg: &str,
        code: Option<i32>,
        status: &str,
        stdout: Option<&[u8]>,
        stderr: Option<&[u8]>,
    ) -> ProcessError {
        let mut desc = format!("{} ({})", msg, status);

        if let Some(out) = stdout {
            match str::from_utf8(out) {
                Ok(s) if !s.trim().is_empty() => {
                    desc.push_str("\n--- stdout\n");
                    desc.push_str(s);
                }
                Ok(..) | Err(..) => {}
            }
        }
        if let Some(out) = stderr {
            match str::from_utf8(out) {
                Ok(s) if !s.trim().is_empty() => {
                    desc.push_str("\n--- stderr\n");
                    desc.push_str(s);
                }
                Ok(..) | Err(..) => {}
            }
        }

        ProcessError {
            desc,
            code,
            stdout: stdout.map(|s| s.to_vec()),
            stderr: stderr.map(|s| s.to_vec()),
        }
    }
}

/// Converts an [`ExitStatus`]  to a human-readable string suitable for
/// displaying to a user.
pub fn exit_status_to_string(status: ExitStatus) -> String {
    return status_to_string(status);

    #[cfg(unix)]
    fn status_to_string(status: ExitStatus) -> String {
        use std::os::unix::process::*;

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
                libc::SIGQUIT => ", SIGQUIT: terminal quit signal",
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
    fn status_to_string(status: ExitStatus) -> String {
        use winapi::shared::minwindef::DWORD;
        use winapi::um::winnt::*;

        let mut base = status.to_string();
        let extra = match status.code().unwrap() as DWORD {
            STATUS_ACCESS_VIOLATION => "STATUS_ACCESS_VIOLATION",
            STATUS_IN_PAGE_ERROR => "STATUS_IN_PAGE_ERROR",
            STATUS_INVALID_HANDLE => "STATUS_INVALID_HANDLE",
            STATUS_INVALID_PARAMETER => "STATUS_INVALID_PARAMETER",
            STATUS_NO_MEMORY => "STATUS_NO_MEMORY",
            STATUS_ILLEGAL_INSTRUCTION => "STATUS_ILLEGAL_INSTRUCTION",
            STATUS_NONCONTINUABLE_EXCEPTION => "STATUS_NONCONTINUABLE_EXCEPTION",
            STATUS_INVALID_DISPOSITION => "STATUS_INVALID_DISPOSITION",
            STATUS_ARRAY_BOUNDS_EXCEEDED => "STATUS_ARRAY_BOUNDS_EXCEEDED",
            STATUS_FLOAT_DENORMAL_OPERAND => "STATUS_FLOAT_DENORMAL_OPERAND",
            STATUS_FLOAT_DIVIDE_BY_ZERO => "STATUS_FLOAT_DIVIDE_BY_ZERO",
            STATUS_FLOAT_INEXACT_RESULT => "STATUS_FLOAT_INEXACT_RESULT",
            STATUS_FLOAT_INVALID_OPERATION => "STATUS_FLOAT_INVALID_OPERATION",
            STATUS_FLOAT_OVERFLOW => "STATUS_FLOAT_OVERFLOW",
            STATUS_FLOAT_STACK_CHECK => "STATUS_FLOAT_STACK_CHECK",
            STATUS_FLOAT_UNDERFLOW => "STATUS_FLOAT_UNDERFLOW",
            STATUS_INTEGER_DIVIDE_BY_ZERO => "STATUS_INTEGER_DIVIDE_BY_ZERO",
            STATUS_INTEGER_OVERFLOW => "STATUS_INTEGER_OVERFLOW",
            STATUS_PRIVILEGED_INSTRUCTION => "STATUS_PRIVILEGED_INSTRUCTION",
            STATUS_STACK_OVERFLOW => "STATUS_STACK_OVERFLOW",
            STATUS_DLL_NOT_FOUND => "STATUS_DLL_NOT_FOUND",
            STATUS_ORDINAL_NOT_FOUND => "STATUS_ORDINAL_NOT_FOUND",
            STATUS_ENTRYPOINT_NOT_FOUND => "STATUS_ENTRYPOINT_NOT_FOUND",
            STATUS_CONTROL_C_EXIT => "STATUS_CONTROL_C_EXIT",
            STATUS_DLL_INIT_FAILED => "STATUS_DLL_INIT_FAILED",
            STATUS_FLOAT_MULTIPLE_FAULTS => "STATUS_FLOAT_MULTIPLE_FAULTS",
            STATUS_FLOAT_MULTIPLE_TRAPS => "STATUS_FLOAT_MULTIPLE_TRAPS",
            STATUS_REG_NAT_CONSUMPTION => "STATUS_REG_NAT_CONSUMPTION",
            STATUS_HEAP_CORRUPTION => "STATUS_HEAP_CORRUPTION",
            STATUS_STACK_BUFFER_OVERRUN => "STATUS_STACK_BUFFER_OVERRUN",
            STATUS_ASSERTION_FAILURE => "STATUS_ASSERTION_FAILURE",
            _ => return base,
        };
        base.push_str(", ");
        base.push_str(extra);
        base
    }
}

/// Returns `true` if the given process exit code is something a normal
/// process would exit with.
///
/// This helps differentiate from abnormal termination codes, such as
/// segmentation faults or signals.
pub fn is_simple_exit_code(code: i32) -> bool {
    // Typical unix exit codes are 0 to 127.
    // Windows doesn't have anything "typical", and is a
    // 32-bit number (which appears signed here, but is really
    // unsigned). However, most of the interesting NTSTATUS
    // codes are very large. This is just a rough
    // approximation of which codes are "normal" and which
    // ones are abnormal termination.
    code >= 0 && code <= 127
}
