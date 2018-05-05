//! A small TCP server to handle collection of diagnostics information in a
//! cross-platform way.

use std::env;
use std::io::BufReader;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread::{self, JoinHandle};

use failure::{Error, ResultExt};
use serde_json;

static DIAGNOSICS_SERVER_VAR: &str = "__CARGO_FIX_DIAGNOSTICS_SERVER";

#[derive(Deserialize, Serialize)]
pub enum Message {
    Fixing { file: String, fixes: usize },
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

        serde_json::to_writer(&mut client, &self)
            .context("failed to write message to diagnostics target")?;

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

    pub fn start<F: Fn(Message) + Send + 'static>(
        self,
        on_message: F,
    ) -> Result<StartedServer, Error> {
        let _addr = self.listener.local_addr()?;
        let thread = thread::spawn(move || {
            self.run(on_message);
        });

        Ok(StartedServer {
            _addr,
            thread: Some(thread),
        })
    }

    fn run<F: Fn(Message)>(self, on_message: F) {
        while let Ok((client, _)) = self.listener.accept() {
            let mut client = BufReader::new(client);
            match serde_json::from_reader(client) {
                Ok(message) => on_message(message),
                Err(e) => { warn!("invalid diagnostics message: {}", e); }
            }
        }
    }
}

impl Drop for StartedServer {
    fn drop(&mut self) {
        drop(self.thread.take().unwrap().join());
    }
}
