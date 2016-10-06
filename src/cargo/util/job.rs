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

pub use self::imp::Setup;

pub fn setup() -> Option<Setup> {
    unsafe { imp::setup() }
}

#[cfg(unix)]
mod imp {
    use std::env;
    use libc;

    pub type Setup = ();

    pub unsafe fn setup() -> Option<()> {
        // There's a test case for the behavior of
        // when-cargo-is-killed-subprocesses-are-also-killed, but that requires
        // one cargo spawned to become its own session leader, so we do that
        // here.
        if env::var("__CARGO_TEST_SETSID_PLEASE_DONT_USE_ELSEWHERE").is_ok() {
            libc::setsid();
        }
        Some(())
    }
}

#[cfg(windows)]
mod imp {
    extern crate kernel32;
    extern crate winapi;
    extern crate psapi;

    use std::ffi::OsString;
    use std::io;
    use std::mem;
    use std::os::windows::prelude::*;

    pub struct Setup {
        job: Handle,
    }

    pub struct Handle {
        inner: winapi::HANDLE,
    }

    fn last_err() -> io::Error {
        io::Error::last_os_error()
    }

    pub unsafe fn setup() -> Option<Setup> {
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
            return None
        }
        let job = Handle { inner: job };

        // Indicate that when all handles to the job object are gone that all
        // process in the object should be killed. Note that this includes our
        // entire process tree by default because we've added ourselves and and
        // our children will reside in the job once we spawn a process.
        let mut info: winapi::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
        info = mem::zeroed();
        info.BasicLimitInformation.LimitFlags =
            winapi::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let r = kernel32::SetInformationJobObject(job.inner,
                        winapi::JobObjectExtendedLimitInformation,
                        &mut info as *mut _ as winapi::LPVOID,
                        mem::size_of_val(&info) as winapi::DWORD);
        if r == 0 {
            return None
        }

        // Assign our process to this job object, meaning that our children will
        // now live or die based on our existence.
        let me = kernel32::GetCurrentProcess();
        let r = kernel32::AssignProcessToJobObject(job.inner, me);
        if r == 0 {
            return None
        }

        Some(Setup { job: job })
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            // This is a litte subtle. By default if we are terminated then all
            // processes in our job object are terminated as well, but we
            // intentionally want to whitelist some processes to outlive our job
            // object (see below).
            //
            // To allow for this, we manually kill processes instead of letting
            // the job object kill them for us. We do this in a loop to handle
            // processes spawning other processes.
            //
            // Finally once this is all done we know that the only remaining
            // ones are ourselves and the whitelisted processes. The destructor
            // here then configures our job object to *not* kill everything on
            // close, then closes the job object.
            unsafe {
                while self.kill_remaining() {
                    info!("killed some, going for more");
                }

                let mut info: winapi::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
                info = mem::zeroed();
                let r = kernel32::SetInformationJobObject(
                            self.job.inner,
                            winapi::JobObjectExtendedLimitInformation,
                            &mut info as *mut _ as winapi::LPVOID,
                            mem::size_of_val(&info) as winapi::DWORD);
                if r == 0 {
                    info!("failed to configure job object to defaults: {}",
                          last_err());
                }
            }
        }
    }

    impl Setup {
        unsafe fn kill_remaining(&mut self) -> bool {
            #[repr(C)]
            struct Jobs {
                header: winapi::JOBOBJECT_BASIC_PROCESS_ID_LIST,
                list: [winapi::ULONG_PTR; 1024],
            }

            let mut jobs: Jobs = mem::zeroed();
            let r = kernel32::QueryInformationJobObject(
                            self.job.inner,
                            winapi::JobObjectBasicProcessIdList,
                            &mut jobs as *mut _ as winapi::LPVOID,
                            mem::size_of_val(&jobs) as winapi::DWORD,
                            0 as *mut _);
            if r == 0 {
                info!("failed to query job object: {}", last_err());
                return false
            }

            let mut killed = false;
            let list = &jobs.list[..jobs.header.NumberOfProcessIdsInList as usize];
            assert!(list.len() > 0);
            info!("found {} remaining processes", list.len() - 1);

            let list = list.iter().filter(|&&id| {
                // let's not kill ourselves
                id as winapi::DWORD != kernel32::GetCurrentProcessId()
            }).filter_map(|&id| {
                // Open the process with the necessary rights, and if this
                // fails then we probably raced with the process exiting so we
                // ignore the problem.
                let flags = winapi::PROCESS_QUERY_INFORMATION |
                            winapi::PROCESS_TERMINATE |
                            winapi::SYNCHRONIZE;
                let p = kernel32::OpenProcess(flags,
                                              winapi::FALSE,
                                              id as winapi::DWORD);
                if p.is_null() {
                    None
                } else {
                    Some(Handle { inner: p })
                }
            }).filter(|p| {
                // Test if this process was actually in the job object or not.
                // If it's not then we likely raced with something else
                // recycling this PID, so we just skip this step.
                let mut res = 0;
                let r = kernel32::IsProcessInJob(p.inner, self.job.inner, &mut res);
                if r == 0 {
                    info!("failed to test is process in job: {}", last_err());
                    return false
                }
                res == winapi::TRUE
            });


            for p in list {
                // Load the file which this process was spawned from. We then
                // later use this for identification purposes.
                let mut buf = [0; 1024];
                let r = psapi::GetProcessImageFileNameW(p.inner,
                                                        buf.as_mut_ptr(),
                                                        buf.len() as winapi::DWORD);
                if r == 0 {
                    info!("failed to get image name: {}", last_err());
                    continue
                }
                let s = OsString::from_wide(&buf[..r as usize]);
                info!("found remaining: {:?}", s);

                // And here's where we find the whole purpose for this
                // function!  Currently, our only whitelisted process is
                // `mspdbsrv.exe`, and more details about that can be found
                // here:
                //
                //      https://github.com/rust-lang/rust/issues/33145
                //
                // The gist of it is that all builds on one machine use the
                // same `mspdbsrv.exe` instance. If we were to kill this
                // instance then we could erroneously cause other builds to
                // fail.
                if let Some(s) = s.to_str() {
                    if s.contains("mspdbsrv") {
                        info!("\toops, this is mspdbsrv");
                        continue
                    }
                }

                // Ok, this isn't mspdbsrv, let's kill the process. After we
                // kill it we wait on it to ensure that the next time around in
                // this function we're not going to see it again.
                let r = kernel32::TerminateProcess(p.inner, 1);
                if r == 0 {
                    info!("\tfailed to kill subprocess: {}", last_err());
                    info!("\tassuming subprocess is dead...");
                } else {
                    info!("\tterminated subprocess");
                }
                let r = kernel32::WaitForSingleObject(p.inner, winapi::INFINITE);
                if r != 0 {
                    info!("failed to wait for process to die: {}", last_err());
                    return false
                }
                killed = true;
            }

            return killed
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe { kernel32::CloseHandle(self.inner); }
        }
    }
}
