use std::net::TcpListener;
use std::io::{self, Read};
use std::process::{Stdio, Child};

use support::project;

#[cfg(unix)]
fn enabled() -> bool {
    true
}

// On Windows suport for these tests is only enabled through the usage of job
// objects. Support for nested job objects, however, was added in recent-ish
// versions of Windows, so this test may not always be able to succeed.
//
// As a result, we try to add ourselves to a job object here
// can succeed or not.
#[cfg(windows)]
fn enabled() -> bool {
    use kernel32;
    use winapi;
    unsafe {
        // If we're not currently in a job, then we can definitely run these
        // tests.
        let me = kernel32::GetCurrentProcess();
        let mut ret = 0;
        let r = kernel32::IsProcessInJob(me, 0 as *mut _, &mut ret);
        assert!(r != 0);
        if ret == winapi::FALSE {
            return true
        }

        // If we are in a job, then we can run these tests if we can be added to
        // a nested job (as we're going to create a nested job no matter what as
        // part of these tests.
        //
        // If we can't be added to a nested job, then these tests will
        // definitely fail, and there's not much we can do about that.
        let job = kernel32::CreateJobObjectW(0 as *mut _, 0 as *const _);
        assert!(!job.is_null());
        let r = kernel32::AssignProcessToJobObject(job, me);
        kernel32::CloseHandle(job);
        r != 0
    }
}

#[test]
fn ctrl_c_kills_everyone() {
    if !enabled() {
        return
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", &format!(r#"
            use std::net::TcpStream;
            use std::io::Read;

            fn main() {{
                let mut socket = TcpStream::connect("{}").unwrap();
                let _ = socket.read(&mut [0; 10]);
                panic!("that read should never return");
            }}
        "#, addr));
    p.build();

    let mut cargo = p.cargo("build").build_command();
    cargo.stdin(Stdio::piped())
         .stdout(Stdio::piped())
         .stderr(Stdio::piped())
         .env("__CARGO_TEST_SETSID_PLEASE_DONT_USE_ELSEWHERE", "1");
    let mut child = cargo.spawn().unwrap();

    let mut sock = listener.accept().unwrap().0;
    ctrl_c(&mut child);

    assert!(!child.wait().unwrap().success());
    match sock.read(&mut [0; 10]) {
        Ok(n) => assert_eq!(n, 0),
        Err(e) => assert_eq!(e.kind(), io::ErrorKind::ConnectionReset),
    }
}

#[cfg(unix)]
fn ctrl_c(child: &mut Child) {
    use libc;

    let r = unsafe { libc::kill(-(child.id() as i32), libc::SIGINT) };
    if r < 0 {
        panic!("failed to kill: {}", io::Error::last_os_error());
    }
}

#[cfg(windows)]
fn ctrl_c(child: &mut Child) {
    child.kill().unwrap();
}
