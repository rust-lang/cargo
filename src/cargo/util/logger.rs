//! Build analysis logging infrastructure.

use std::cell::RefCell;
use std::hash::Hash;
use std::io::{BufWriter, Write};
use std::mem::ManuallyDrop;
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use anyhow::Context as _;
use cargo_util::paths;

use crate::CargoResult;
use crate::core::Workspace;
use crate::core::compiler::BuildConfig;
use crate::util::log_message::LogMessage;
use crate::util::short_hash;

// for newer `cargo report` commands
struct FileLogger {
    tx: ManuallyDrop<Sender<LogMessage>>,
    handle: Option<JoinHandle<()>>,
}

impl FileLogger {
    /// Creates a logger if `-Zbuild-analysis` is enabled
    fn maybe_new(ws: &Workspace<'_>, run_id: &RunId) -> CargoResult<Option<FileLogger>> {
        let analysis = ws.gctx().build_config()?.analysis.as_ref();
        match (analysis, ws.gctx().cli_unstable().build_analysis) {
            (Some(analysis), true) if analysis.enabled => {
                let log_dir = ws.gctx().home().join("log");
                paths::create_dir_all(log_dir.as_path_unlocked())?;

                let filename = format!("{run_id}.jsonl");
                let log_file = log_dir.open_rw_exclusive_create(
                    Path::new(&filename),
                    ws.gctx(),
                    "build analysis log",
                )?;

                let (tx, rx) = mpsc::channel::<LogMessage>();

                let run_id_str = run_id.to_string();
                let handle = std::thread::spawn(move || {
                    let mut writer = BufWriter::new(log_file);
                    for msg in rx {
                        let _ = msg.write_json_log(&mut writer, &run_id_str);
                    }
                    let _ = writer.flush();
                });

                Ok(Some(Self {
                    tx: ManuallyDrop::new(tx),
                    handle: Some(handle),
                }))
            }
            (Some(_), false) => {
                ws.gctx().shell().warn(
                    "ignoring 'build.analysis' config, pass `-Zbuild-analysis` to enable it",
                )?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

impl Drop for FileLogger {
    fn drop(&mut self) {
        // SAFETY: tx is dropped exactly once here to signal thread shutdown.
        // ManuallyDrop prevents automatic drop after this impl runs.
        unsafe {
            ManuallyDrop::drop(&mut self.tx);
        }

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// For legacy `cargo build --timings` flag
struct InMemoryLogger {
    // using mutex to hide mutability
    logs: RefCell<Vec<LogMessage>>,
}

impl InMemoryLogger {
    fn maybe_new(options: &BuildConfig) -> Option<Self> {
        if options.timing_report {
            Some(Self {
                logs: RefCell::new(Vec::new()),
            })
        } else {
            None
        }
    }
}

/// Logger for `-Zbuild-analysis`.
pub struct BuildLogger {
    run_id: RunId,
    file_logger: Option<FileLogger>,
    in_memory_logger: Option<InMemoryLogger>,
}

impl BuildLogger {
    pub fn maybe_new(ws: &Workspace<'_>, options: &BuildConfig) -> CargoResult<Option<Self>> {
        let run_id = Self::generate_run_id(ws);
        let file_logger = FileLogger::maybe_new(ws, &run_id)?;
        let in_memory_logger = InMemoryLogger::maybe_new(options);

        if file_logger.is_none() && in_memory_logger.is_none() {
            return Ok(None);
        }

        Ok(Some(Self {
            run_id,
            file_logger,
            in_memory_logger,
        }))
    }

    /// Generates a unique run ID.
    pub fn generate_run_id(ws: &Workspace<'_>) -> RunId {
        RunId::new(&ws.root())
    }

    /// Returns the run ID for this build session.
    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    /// Logs a message.
    pub fn log(&self, msg: LogMessage) {
        if let Some(ref logger) = self.in_memory_logger {
            let mut borrowed = logger.logs.try_borrow_mut().expect(
                "Unable to get a mutable reference to in-memory logger; please file a bug report",
            );
            borrowed.push(msg.clone());
        };

        if let Some(ref logger) = self.file_logger {
            let _ = logger.tx.send(msg);
        };
    }

    pub fn get_logs(&self) -> Option<Vec<LogMessage>> {
        self.in_memory_logger.as_ref().map(|l| {
            l.logs
                .try_borrow()
                .expect("Unable to get a reference to in-memory logger; please file a bug report")
                .clone()
        })
    }
}

/// A unique identifier for a Cargo invocation.
#[derive(Clone)]
pub struct RunId {
    timestamp: jiff::Timestamp,
    hash: String,
}

impl RunId {
    const FORMAT: &str = "%Y%m%dT%H%M%S%3fZ";

    pub fn new<H: Hash>(h: &H) -> RunId {
        RunId {
            timestamp: jiff::Timestamp::now(),
            hash: short_hash(h),
        }
    }

    pub fn timestamp(&self) -> &jiff::Timestamp {
        &self.timestamp
    }

    /// Checks whether ID was generated from the same workspace.
    pub fn same_workspace(&self, other: &RunId) -> bool {
        self.hash == other.hash
    }
}

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hash = &self.hash;
        let timestamp = self.timestamp.strftime(Self::FORMAT);
        write!(f, "{timestamp}-{hash}")
    }
}

impl std::str::FromStr for RunId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let msg =
            || format!("expect run ID in format `20060724T012128000Z-<16-char-hex>`, got `{s}`");
        let Some((timestamp, hash)) = s.rsplit_once('-') else {
            anyhow::bail!(msg());
        };

        if hash.len() != 16 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!(msg());
        }
        let timestamp = jiff::civil::DateTime::strptime(Self::FORMAT, timestamp)
            .and_then(|dt| dt.to_zoned(jiff::tz::TimeZone::UTC))
            .map(|zoned| zoned.timestamp())
            .with_context(msg)?;

        Ok(RunId {
            timestamp,
            hash: hash.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_id_round_trip() {
        let id = "20060724T012128000Z-b0fd440798ab3cfb";
        assert_eq!(id, &id.parse::<RunId>().unwrap().to_string());
    }
}
