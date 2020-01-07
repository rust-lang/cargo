//! An implementation of IPC locks, guaranteed to be released if a process dies
//!
//! This module implements a locking server/client where the main `cargo fix`
//! process will start up a server and then all the client processes will
//! connect to it. The main purpose of this file is to ensure that each crate
//! (aka file entry point) is only fixed by one process at a time, currently
//! concurrent fixes can't happen.
//!
//! The basic design here is to use a TCP server which is pretty portable across
//! platforms. For simplicity it just uses threads as well. Clients connect to
//! the main server, inform the server what its name is, and then wait for the
//! server to give it the lock (aka write a byte).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Error};

pub struct LockServer {
    listener: TcpListener,
    addr: SocketAddr,
    threads: HashMap<String, ServerClient>,
    done: Arc<AtomicBool>,
}

pub struct LockServerStarted {
    done: Arc<AtomicBool>,
    addr: SocketAddr,
    thread: Option<JoinHandle<()>>,
}

pub struct LockServerClient {
    _socket: TcpStream,
}

struct ServerClient {
    thread: Option<JoinHandle<()>>,
    lock: Arc<Mutex<(bool, Vec<TcpStream>)>>,
}

impl LockServer {
    pub fn new() -> Result<LockServer, Error> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .with_context(|| "failed to bind TCP listener to manage locking")?;
        let addr = listener.local_addr()?;
        Ok(LockServer {
            listener,
            addr,
            threads: HashMap::new(),
            done: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }

    pub fn start(self) -> Result<LockServerStarted, Error> {
        let addr = self.addr;
        let done = self.done.clone();
        let thread = thread::spawn(|| {
            self.run();
        });
        Ok(LockServerStarted {
            addr,
            thread: Some(thread),
            done,
        })
    }

    fn run(mut self) {
        while let Ok((client, _)) = self.listener.accept() {
            if self.done.load(Ordering::SeqCst) {
                break;
            }

            // Learn the name of our connected client to figure out if it needs
            // to wait for another process to release the lock.
            let mut client = BufReader::new(client);
            let mut name = String::new();
            if client.read_line(&mut name).is_err() {
                continue;
            }
            let client = client.into_inner();

            // If this "named mutex" is already registered and the thread is
            // still going, put it on the queue. Otherwise wait on the previous
            // thread and we'll replace it just below.
            if let Some(t) = self.threads.get_mut(&name) {
                let mut state = t.lock.lock().unwrap();
                if state.0 {
                    state.1.push(client);
                    continue;
                }
                drop(t.thread.take().unwrap().join());
            }

            let lock = Arc::new(Mutex::new((true, vec![client])));
            let lock2 = lock.clone();
            let thread = thread::spawn(move || {
                loop {
                    let mut client = {
                        let mut state = lock2.lock().unwrap();
                        if state.1.is_empty() {
                            state.0 = false;
                            break;
                        } else {
                            state.1.remove(0)
                        }
                    };
                    // Inform this client that it now has the lock and wait for
                    // it to disconnect by waiting for EOF.
                    if client.write_all(&[1]).is_err() {
                        continue;
                    }
                    let mut dst = Vec::new();
                    drop(client.read_to_end(&mut dst));
                }
            });

            self.threads.insert(
                name,
                ServerClient {
                    thread: Some(thread),
                    lock,
                },
            );
        }
    }
}

impl Drop for LockServer {
    fn drop(&mut self) {
        for (_, mut client) in self.threads.drain() {
            if let Some(thread) = client.thread.take() {
                drop(thread.join());
            }
        }
    }
}

impl Drop for LockServerStarted {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        // Ignore errors here as this is largely best-effort
        if TcpStream::connect(&self.addr).is_err() {
            return;
        }
        drop(self.thread.take().unwrap().join());
    }
}

impl LockServerClient {
    pub fn lock(addr: &SocketAddr, name: impl AsRef<[u8]>) -> Result<LockServerClient, Error> {
        let mut client =
            TcpStream::connect(&addr).with_context(|| "failed to connect to parent lock server")?;
        client
            .write_all(name.as_ref())
            .and_then(|_| client.write_all(b"\n"))
            .with_context(|| "failed to write to lock server")?;
        let mut buf = [0];
        client
            .read_exact(&mut buf)
            .with_context(|| "failed to acquire lock")?;
        Ok(LockServerClient { _socket: client })
    }
}
