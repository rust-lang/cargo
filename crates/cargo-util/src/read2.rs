pub use self::imp::read2;

#[cfg(unix)]
mod imp {
    use io_lifetimes::AsFilelike;
    use rustix::io::{PollFd, PollFlags};
    use std::fs::File;
    use std::io;
    use std::io::prelude::*;
    use std::process::{ChildStderr, ChildStdout};

    pub fn read2(
        out_pipe: ChildStdout,
        err_pipe: ChildStderr,
        data: &mut dyn FnMut(bool, &mut Vec<u8>, bool),
    ) -> io::Result<()> {
        rustix::fs::fcntl_setfl(&out_pipe, rustix::fs::OFlags::NONBLOCK)?;
        rustix::fs::fcntl_setfl(&err_pipe, rustix::fs::OFlags::NONBLOCK)?;

        let mut out_done = false;
        let mut err_done = false;
        let mut out = Vec::new();
        let mut err = Vec::new();

        let mut fds = [
            PollFd::new(&out_pipe, PollFlags::IN),
            PollFd::new(&err_pipe, PollFlags::IN),
        ];
        let mut nfds = 2;
        let mut errfd = 1;

        while nfds > 0 {
            // wait for either pipe to become readable using `poll`
            match rustix::io::poll(&mut fds, -1) {
                Ok(_num) => (),
                Err(rustix::io::Error::INTR) => continue,
                Err(err) => return Err(err.into()),
            };

            // Read as much as we can from each pipe, ignoring EWOULDBLOCK or
            // EAGAIN. If we hit EOF, then this will happen because the underlying
            // reader will return Ok(0), in which case we'll see `Ok` ourselves. In
            // this case we flip the other fd back into blocking mode and read
            // whatever's leftover on that file descriptor.
            let handle = |poll_fd: &PollFd, buf: &mut Vec<u8>| match poll_fd
                .as_filelike_view::<File>()
                .read_to_end(buf)
            {
                Ok(_) => Ok(true),
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        Ok(false)
                    } else {
                        Err(e)
                    }
                }
            };
            if !err_done && !fds[errfd].revents().is_empty() && handle(&fds[errfd], &mut err)? {
                err_done = true;
                nfds -= 1;
            }
            data(false, &mut err, err_done);
            if !out_done && !fds[0].revents().is_empty() && handle(&fds[0], &mut out)? {
                out_done = true;
                fds[0].set_fd(&err_pipe);
                errfd = 0;
                nfds -= 1;
            }
            data(true, &mut out, out_done);
        }
        Ok(())
    }
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::os::windows::prelude::*;
    use std::process::{ChildStderr, ChildStdout};
    use std::slice;

    use miow::iocp::{CompletionPort, CompletionStatus};
    use miow::pipe::NamedPipe;
    use miow::Overlapped;
    use winapi::shared::winerror::ERROR_BROKEN_PIPE;

    struct Pipe<'a> {
        dst: &'a mut Vec<u8>,
        overlapped: Overlapped,
        pipe: NamedPipe,
        done: bool,
    }

    pub fn read2(
        out_pipe: ChildStdout,
        err_pipe: ChildStderr,
        data: &mut dyn FnMut(bool, &mut Vec<u8>, bool),
    ) -> io::Result<()> {
        let mut out = Vec::new();
        let mut err = Vec::new();

        let port = CompletionPort::new(1)?;
        port.add_handle(0, &out_pipe)?;
        port.add_handle(1, &err_pipe)?;

        unsafe {
            let mut out_pipe = Pipe::new(out_pipe, &mut out);
            let mut err_pipe = Pipe::new(err_pipe, &mut err);

            out_pipe.read()?;
            err_pipe.read()?;

            let mut status = [CompletionStatus::zero(), CompletionStatus::zero()];

            while !out_pipe.done || !err_pipe.done {
                for status in port.get_many(&mut status, None)? {
                    if status.token() == 0 {
                        out_pipe.complete(status);
                        data(true, out_pipe.dst, out_pipe.done);
                        out_pipe.read()?;
                    } else {
                        err_pipe.complete(status);
                        data(false, err_pipe.dst, err_pipe.done);
                        err_pipe.read()?;
                    }
                }
            }

            Ok(())
        }
    }

    impl<'a> Pipe<'a> {
        unsafe fn new<P: IntoRawHandle>(p: P, dst: &'a mut Vec<u8>) -> Pipe<'a> {
            Pipe {
                dst,
                pipe: NamedPipe::from_raw_handle(p.into_raw_handle()),
                overlapped: Overlapped::zero(),
                done: false,
            }
        }

        unsafe fn read(&mut self) -> io::Result<()> {
            let dst = slice_to_end(self.dst);
            match self.pipe.read_overlapped(dst, self.overlapped.raw()) {
                Ok(_) => Ok(()),
                Err(e) => {
                    if e.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) {
                        self.done = true;
                        Ok(())
                    } else {
                        Err(e)
                    }
                }
            }
        }

        unsafe fn complete(&mut self, status: &CompletionStatus) {
            let prev = self.dst.len();
            self.dst.set_len(prev + status.bytes_transferred() as usize);
            if status.bytes_transferred() == 0 {
                self.done = true;
            }
        }
    }

    unsafe fn slice_to_end(v: &mut Vec<u8>) -> &mut [u8] {
        if v.capacity() == 0 {
            v.reserve(16);
        }
        if v.capacity() == v.len() {
            v.reserve(1);
        }
        slice::from_raw_parts_mut(v.as_mut_ptr().add(v.len()), v.capacity() - v.len())
    }
}
