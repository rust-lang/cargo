//! Job management (mostly for windows)
//!
//! Most of the time when you're running cargo you expect Ctrl-C to actually
//! terminate the entire tree of processes in play, not just the one at the top
//! (cago). This currently works "by default" on Unix platforms because Ctrl-C
//! actually sends a signal to the *process group* rather than the parent
//! process, so everything will get torn down. On Windows, however, this does
//! not happen and Ctrl-C just kills cargo.
//!
//! To achieve the same semantics on Windows we use Job Objects to ensure that
//! all processes die at the same time. Job objects have a mode of operation
//! where when all handles to the object are closed it causes all child
//! processes associated with the object to be terminated immediately.
//! Conveniently whenever a process in the job object spawns a new process the
//! child will be associated with the job object as well. This means if we add
//! ourselves to the job object we create then everything will get torn down!

pub fn setup() {
    unsafe { imp::setup() }
}

#[cfg(unix)]
mod imp {
    use std::env;
    use libc;

    pub unsafe fn setup() {
        // There's a test case for the behavior of
        // when-cargo-is-killed-subprocesses-are-also-killed, but that requires
        // one cargo spawned to become its own session leader, so we do that
        // here.
        if env::var("__CARGO_TEST_SETSID_PLEASE_DONT_USE_ELSEWHERE").is_ok() {
            libc::setsid();
        }
    }
}

#[cfg(windows)]
mod imp {
    extern crate kernel32;
    extern crate winapi;

    use std::mem;

    pub unsafe fn setup() {
        // Creates a new job object for us to use and then adds ourselves to it.
        // Note that all errors are basically ignored in this function,
        // intentionally. Job objects are "relatively new" in Windows,
        // particularly the ability to support nested job objects. Older
        // Windows installs don't support this ability. We probably don't want
        // to force Cargo to abort in this situation or force others to *not*
        // use job objects, so we instead just ignore errors and assume that
        // we're otherwise part of someone else's job object in this case.

        let job = kernel32::CreateJobObjectW(0 as *mut _, 0 as *const _);
        if job.is_null() {
            return
        }

        // Indicate that when all handles to the job object are gone that all
        // process in the object should be killed. Note that this includes our
        // entire process tree by default because we've added ourselves and and
        // our children will reside in the job once we spawn a process.
        let mut info: winapi::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
        info = mem::zeroed();
        info.BasicLimitInformation.LimitFlags =
            winapi::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let r = kernel32::SetInformationJobObject(job,
                        winapi::JobObjectExtendedLimitInformation,
                        &mut info as *mut _ as winapi::LPVOID,
                        mem::size_of_val(&info) as winapi::DWORD);
        if r == 0 {
            kernel32::CloseHandle(job);
            return
        }

        // Assign our process to this job object, meaning that our children will
        // now live or die based on our existence.
        let me = kernel32::GetCurrentProcess();
        let r = kernel32::AssignProcessToJobObject(job, me);
        if r == 0 {
            kernel32::CloseHandle(job);
            return
        }

        // Intentionally leak the `job` handle here. We've got the only
        // reference to this job, so once it's gone we and all our children will
        // be killed. This typically won't happen unless Cargo itself is
        // ctrl-c'd.
    }
}
