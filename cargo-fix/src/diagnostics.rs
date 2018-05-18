//! A small TCP server to handle collection of diagnostics information in a
//! cross-platform way.

use std::env;
use std::io::{BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::thread::{self, JoinHandle};

use atty;
use failure::{Error, ResultExt};
use serde_json;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

static DIAGNOSICS_SERVER_VAR: &str = "__CARGO_FIX_DIAGNOSTICS_SERVER";

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
}

impl Message {
    pub fn fixing(file: &str, num: usize) -> Message {
        Message::Fixing {
            file: file.into(),
            fixes: num,
        }
    }

    pub fn post(&self) -> Result<(), Error> {
        let addr = env::var(DIAGNOSICS_SERVER_VAR).context("diagnostics collector misconfigured")?;
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

pub struct Server {
    listener: TcpListener,
}

pub struct StartedServer {
    _addr: SocketAddr,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn new() -> Result<Self, Error> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .with_context(|_| "failed to bind TCP listener to manage locking")?;
        env::set_var(DIAGNOSICS_SERVER_VAR, listener.local_addr()?.to_string());

        Ok(Server { listener })
    }

    pub fn start<F>(self, on_message: F) -> Result<StartedServer, Error>
    where
        F: Fn(Message, &mut StandardStream) + Send + 'static,
    {
        let _addr = self.listener.local_addr()?;
        let thread = thread::spawn(move || {
            self.run(on_message);
        });

        Ok(StartedServer {
            _addr,
            thread: Some(thread),
        })
    }

    fn run<F>(self, on_message: F)
    where
        F: Fn(Message, &mut StandardStream),
    {
        let mut stream = output_stream();
        while let Ok((client, _)) = self.listener.accept() {
            let mut client = BufReader::new(client);
            match serde_json::from_reader(client) {
                Ok(message) => on_message(message, &mut stream),
                Err(e) => {
                    warn!("invalid diagnostics message: {}", e);
                }
            }
        }
    }
}

impl Drop for StartedServer {
    fn drop(&mut self) {
        drop(self.thread.take().unwrap().join());
    }
}

pub fn output_stream() -> StandardStream {
    let color_choice = if atty::is(atty::Stream::Stderr) {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };
    StandardStream::stderr(color_choice)
}

pub fn write_warning(stream: &mut StandardStream) -> Result<(), Error> {
    stream.set_color(ColorSpec::new().set_bold(true).set_fg(Some(Color::Yellow)))?;
    write!(stream, "warning")?;
    stream.reset()?;
    stream.set_color(ColorSpec::new().set_bold(true))?;
    write!(stream, ": ")?;
    Ok(())
}

pub fn log_for_human(kind: &str, msg: &str, stream: &mut StandardStream) -> Result<(), Error> {
    stream.set_color(ColorSpec::new().set_bold(true).set_fg(Some(Color::Cyan)))?;
    // Justify to 12 chars just like cargo
    write!(stream, "{:>12}", kind)?;
    stream.reset()?;
    write!(stream, " {}\n", msg)?;
    Ok(())
}
