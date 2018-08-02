//! A small TCP server to handle collection of diagnostics information in a
//! cross-platform way for the `cargo fix` command.

use std::collections::HashSet;
use std::env;
use std::io::{BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use failure::{Error, ResultExt};
use serde_json;

use util::{Config, ProcessBuilder};
use util::errors::CargoResult;

const DIAGNOSICS_SERVER_VAR: &str = "__CARGO_FIX_DIAGNOSTICS_SERVER";
const PLEASE_REPORT_THIS_BUG: &str =
    "\
     This likely indicates a bug in either rustc or cargo itself,\n\
     and we would appreciate a bug report! You're likely to see \n\
     a number of compiler warnings after this message which cargo\n\
     attempted to fix but failed. If you could open an issue at\n\
     https://github.com/rust-lang/cargo/issues\n\
     quoting the full output of this command we'd be very appreciative!\n\n\
     ";

#[derive(Deserialize, Serialize)]
pub enum Message {
    Fixing {
        file: String,
        fixes: usize,
    },
    FixFailed {
        files: Vec<String>,
        krate: Option<String>,
    },
    ReplaceFailed {
        file: String,
        message: String,
    },
    PreviewNotFound {
        file: String,
        edition: String,
    },
    EditionAlreadyEnabled {
        file: String,
        edition: String,
    },
}

impl Message {
    pub fn fixing(file: &str, num: usize) -> Message {
        Message::Fixing {
            file: file.into(),
            fixes: num,
        }
    }

    pub fn post(&self) -> Result<(), Error> {
        let addr = env::var(DIAGNOSICS_SERVER_VAR)
            .context("diagnostics collector misconfigured")?;
        let mut client =
            TcpStream::connect(&addr).context("failed to connect to parent diagnostics target")?;

        let s = serde_json::to_string(self).context("failed to serialize message")?;
        client
            .write_all(s.as_bytes())
            .context("failed to write message to diagnostics target")?;
        client
            .shutdown(Shutdown::Write)
            .context("failed to shutdown")?;

        let mut tmp = Vec::new();
        client
            .read_to_end(&mut tmp)
            .context("failed to receive a disconnect")?;

        Ok(())
    }
}

pub struct DiagnosticPrinter<'a> {
    config: &'a Config,
    preview_not_found: HashSet<String>,
    edition_already_enabled: HashSet<String>,
}

impl<'a> DiagnosticPrinter<'a> {
    pub fn new(config: &'a Config) -> DiagnosticPrinter<'a> {
        DiagnosticPrinter {
            config,
            preview_not_found: HashSet::new(),
            edition_already_enabled: HashSet::new(),
        }
    }

    pub fn print(&mut self, msg: &Message) -> CargoResult<()> {
        match msg {
            Message::Fixing { file, fixes } => {
                let msg = if *fixes == 1 { "fix" } else { "fixes" };
                let msg = format!("{} ({} {})", file, fixes, msg);
                self.config.shell().status("Fixing", msg)
            }
            Message::ReplaceFailed { file, message } => {
                let msg = format!("error applying suggestions to `{}`\n", file);
                self.config.shell().warn(&msg)?;
                write!(
                    self.config.shell().err(),
                    "The full error message was:\n\n> {}\n\n",
                    message,
                )?;
                write!(self.config.shell().err(), "{}", PLEASE_REPORT_THIS_BUG)?;
                Ok(())
            }
            Message::FixFailed { files, krate } => {
                if let Some(ref krate) = *krate {
                    self.config.shell().warn(&format!(
                        "failed to automatically apply fixes suggested by rustc \
                         to crate `{}`",
                        krate,
                    ))?;
                } else {
                    self.config.shell().warn(
                        "failed to automatically apply fixes suggested by rustc"
                    )?;
                }
                if !files.is_empty() {
                    writeln!(
                        self.config.shell().err(),
                        "\nafter fixes were automatically applied the compiler \
                         reported errors within these files:\n"
                    )?;
                    for file in files {
                        writeln!(self.config.shell().err(), "  * {}", file)?;
                    }
                    writeln!(self.config.shell().err())?;
                }
                write!(self.config.shell().err(), "{}", PLEASE_REPORT_THIS_BUG)?;
                Ok(())
            }
            Message::PreviewNotFound { file, edition } => {
                // By default we're fixing a lot of things concurrently, don't
                // warn about the same file multiple times.
                if !self.preview_not_found.insert(file.clone()) {
                    return Ok(())
                }
                self.config.shell().warn(&format!(
                    "failed to find `#![feature(rust_{}_preview)]` in `{}`\n\
                     this may cause `cargo fix` to not be able to fix all\n\
                     issues in preparation for the {0} edition",
                    edition,
                    file,
                ))?;
                Ok(())
            }
            Message::EditionAlreadyEnabled { file, edition } => {
                // Like above, only warn once per file
                if !self.edition_already_enabled.insert(file.clone()) {
                    return Ok(())
                }

                let msg = format!(
                    "\
cannot prepare for the {} edition when it is enabled, so cargo cannot
automatically fix errors in `{}`

To prepare for the {0} edition you should first remove `edition = '{0}'` from
your `Cargo.toml` and then rerun this command. Once all warnings have been fixed
then you can re-enable the `edition` key in `Cargo.toml`. For some more
information about transitioning to the {0} edition see:

  https://rust-lang-nursery.github.io/edition-guide/editions/transitioning.html
",
                    edition,
                    file,
                );
                self.config.shell().error(&msg)?;
                Ok(())
            }
        }

    }
}

#[derive(Debug)]
pub struct RustfixDiagnosticServer {
    listener: TcpListener,
    addr: SocketAddr,
}

pub struct StartedServer {
    addr: SocketAddr,
    done: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl RustfixDiagnosticServer {
    pub fn new() -> Result<Self, Error> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .with_context(|_| "failed to bind TCP listener to manage locking")?;
        let addr = listener.local_addr()?;

        Ok(RustfixDiagnosticServer { listener, addr })
    }

    pub fn configure(&self, process: &mut ProcessBuilder) {
        process.env(DIAGNOSICS_SERVER_VAR, self.addr.to_string());
    }

    pub fn start<F>(self, on_message: F) -> Result<StartedServer, Error>
    where
        F: Fn(Message) + Send + 'static,
    {
        let addr = self.addr;
        let done = Arc::new(AtomicBool::new(false));
        let done2 = done.clone();
        let thread = thread::spawn(move || {
            self.run(&on_message, &done2);
        });

        Ok(StartedServer {
            addr,
            thread: Some(thread),
            done,
        })
    }

    fn run(self, on_message: &Fn(Message), done: &AtomicBool) {
        while let Ok((client, _)) = self.listener.accept() {
            let client = BufReader::new(client);
            match serde_json::from_reader(client) {
                Ok(message) => on_message(message),
                Err(e) => warn!("invalid diagnostics message: {}", e),
            }
            if done.load(Ordering::SeqCst) {
                break
            }
        }
    }
}

impl Drop for StartedServer {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        // Ignore errors here as this is largely best-effort
        if TcpStream::connect(&self.addr).is_err() {
            return;
        }
        drop(self.thread.take().unwrap().join());
    }
}
