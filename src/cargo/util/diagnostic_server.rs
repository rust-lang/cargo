//! A small TCP server to handle collection of diagnostics information in a
//! cross-platform way for the `cargo fix` command.

use std::collections::HashSet;
use std::io::{BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use anyhow::{Context as _, Error};
use cargo_util::ProcessBuilder;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Origin;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::core::Edition;
use crate::util::GlobalContext;
use crate::util::errors::CargoResult;
use crate::util::network::LOCALHOST;

const DIAGNOSTICS_SERVER_VAR: &str = "__CARGO_FIX_DIAGNOSTICS_SERVER";

#[derive(Deserialize, Serialize, Hash, Eq, PartialEq, Clone)]
pub enum Message {
    Migrating {
        file: String,
        from_edition: Edition,
        to_edition: Edition,
    },
    Fixing {
        file: String,
    },
    Fixed {
        file: String,
        fixes: u32,
    },
    FixFailed {
        files: Vec<String>,
        krate: Option<String>,
        errors: Vec<String>,
        abnormal_exit: Option<String>,
    },
    ReplaceFailed {
        file: String,
        message: String,
    },
    EditionAlreadyEnabled {
        message: String,
        edition: Edition,
    },
}

impl Message {
    pub fn post(&self, gctx: &GlobalContext) -> Result<(), Error> {
        let addr = gctx
            .get_env(DIAGNOSTICS_SERVER_VAR)
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

        client
            .read_to_end(&mut Vec::new())
            .context("failed to receive a disconnect")?;

        Ok(())
    }
}

/// A printer that will print diagnostics messages to the shell.
pub struct DiagnosticPrinter<'a> {
    /// The context to get the shell to print to.
    gctx: &'a GlobalContext,
    /// An optional wrapper to be used in addition to `rustc.wrapper` for workspace crates.
    /// This is used to get the correct bug report URL. For instance,
    /// if `clippy-driver` is set as the value for the wrapper,
    /// then the correct bug report URL for `clippy` can be obtained.
    workspace_wrapper: &'a Option<PathBuf>,
    // A set of messages that have already been printed.
    dedupe: HashSet<Message>,
}

impl<'a> DiagnosticPrinter<'a> {
    pub fn new(
        gctx: &'a GlobalContext,
        workspace_wrapper: &'a Option<PathBuf>,
    ) -> DiagnosticPrinter<'a> {
        DiagnosticPrinter {
            gctx,
            workspace_wrapper,
            dedupe: HashSet::new(),
        }
    }

    pub fn print(&mut self, msg: &Message) -> CargoResult<()> {
        match msg {
            Message::Migrating {
                file,
                from_edition,
                to_edition,
            } => {
                if !self.dedupe.insert(msg.clone()) {
                    return Ok(());
                }
                self.gctx.shell().status(
                    "Migrating",
                    &format!("{file} from {from_edition} edition to {to_edition}"),
                )
            }
            Message::Fixing { file } => self
                .gctx
                .shell()
                .verbose(|shell| shell.status("Fixing", file)),
            Message::Fixed { file, fixes } => {
                let msg = if *fixes == 1 { "fix" } else { "fixes" };
                let msg = format!("{file} ({fixes} {msg})");
                self.gctx.shell().status("Fixed", msg)
            }
            Message::ReplaceFailed { file, message } => {
                let issue_link = get_bug_report_url(self.workspace_wrapper);

                let report = &[
                    Level::ERROR
                        .secondary_title("error applying suggestions")
                        .element(Origin::path(file))
                        .element(Level::ERROR.with_name("cause").message(message)),
                    gen_please_report_this_bug_group(issue_link),
                    gen_suggest_broken_code_group(),
                ];
                self.gctx.shell().print_report(report, false)?;
                Ok(())
            }
            Message::FixFailed {
                files,
                krate,
                errors,
                abnormal_exit,
            } => {
                let to_crate = if let Some(ref krate) = *krate {
                    format!(" to crate `{krate}`",)
                } else {
                    "".to_owned()
                };
                let issue_link = get_bug_report_url(self.workspace_wrapper);

                let cause_message = if !errors.is_empty() {
                    Some(errors.join("\n").trim().to_owned())
                } else {
                    None
                };

                let report = &[
                    Level::ERROR
                        .secondary_title(format!("errors present after applying fixes{to_crate}"))
                        .elements(files.iter().map(|f| Origin::path(f)))
                        .elements(
                            cause_message
                                .into_iter()
                                .map(|err| Level::ERROR.with_name("cause").message(err)),
                        )
                        .elements(abnormal_exit.iter().map(|exit| {
                            Level::ERROR
                                .with_name("cause")
                                .message(format!("rustc exited abnormally: {exit}"))
                        })),
                    gen_please_report_this_bug_group(issue_link),
                    gen_suggest_broken_code_group(),
                    Group::with_title(
                        Level::NOTE.secondary_title("original diagnostics will follow:"),
                    ),
                ];

                self.gctx.shell().print_report(report, false)?;
                Ok(())
            }
            Message::EditionAlreadyEnabled { message, edition } => {
                if !self.dedupe.insert(msg.clone()) {
                    return Ok(());
                }
                // Don't give a really verbose warning if it has already been issued.
                if self.dedupe.insert(Message::EditionAlreadyEnabled {
                    message: "".to_string(), // Dummy, so that this only long-warns once.
                    edition: *edition,
                }) {
                    self.gctx.shell().warn(&format!("\
{message}

If you are trying to migrate from the previous edition ({prev_edition}), the
process requires following these steps:

1. Start with `edition = \"{prev_edition}\"` in `Cargo.toml`
2. Run `cargo fix --edition`
3. Modify `Cargo.toml` to set `edition = \"{this_edition}\"`
4. Run `cargo build` or `cargo test` to verify the fixes worked

More details may be found at
https://doc.rust-lang.org/edition-guide/editions/transitioning-an-existing-project-to-a-new-edition.html
",
                        this_edition=edition, prev_edition=edition.previous().unwrap()
                    ))
                } else {
                    self.gctx.shell().warn(message)
                }
            }
        }
    }
}

fn gen_please_report_this_bug_group(url: &str) -> Group<'static> {
    Group::with_title(Level::HELP.secondary_title(format!(
        "to report this as a bug, open an issue at {url}, quoting the full output of this command"
    )))
}

fn gen_suggest_broken_code_group() -> Group<'static> {
    Group::with_title(
        Level::HELP
            .secondary_title("to possibly apply more fixes, pass in the `--broken-code` flag"),
    )
}

fn get_bug_report_url(rustc_workspace_wrapper: &Option<PathBuf>) -> &str {
    let clippy = std::ffi::OsStr::new("clippy-driver");
    let issue_link = match rustc_workspace_wrapper.as_ref().and_then(|x| x.file_stem()) {
        Some(wrapper) if wrapper == clippy => "https://github.com/rust-lang/rust-clippy/issues",
        _ => "https://github.com/rust-lang/rust/issues",
    };

    issue_link
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
        let listener = TcpListener::bind(&LOCALHOST[..])
            .context("failed to bind TCP listener to manage locking")?;
        let addr = listener.local_addr()?;

        Ok(RustfixDiagnosticServer { listener, addr })
    }

    pub fn configure(&self, process: &mut ProcessBuilder) {
        process.env(DIAGNOSTICS_SERVER_VAR, self.addr.to_string());
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

    fn run(self, on_message: &dyn Fn(Message), done: &AtomicBool) {
        while let Ok((client, _)) = self.listener.accept() {
            if done.load(Ordering::SeqCst) {
                break;
            }
            let mut client = BufReader::new(client);
            let mut s = String::new();
            if let Err(e) = client.read_to_string(&mut s) {
                warn!("diagnostic server failed to read: {e}");
            } else {
                match serde_json::from_str(&s) {
                    Ok(message) => on_message(message),
                    Err(e) => warn!("invalid diagnostics message: {e}"),
                }
            }
            // The client should be kept alive until after `on_message` is
            // called to ensure that the client doesn't exit too soon (and
            // Message::Finish getting posted before Message::FixDiagnostic).
            drop(client);
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
