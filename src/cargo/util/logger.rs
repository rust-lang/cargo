//! Build analysis logging infrastructure.

use std::hash::Hash;
use std::io::{BufWriter, Write};
use std::mem::ManuallyDrop;
use std::path::Path;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

use anyhow::Context as _;
use cargo_util::paths;

use crate::CargoResult;
use crate::core::Workspace;
use crate::util::log_message::LogMessage;
use crate::util::short_hash;

/// Logger for `-Zbuild-analysis`.
pub struct BuildLogger {
    tx: ManuallyDrop<Sender<LogMessage>>,
    run_id: RunId,
    handle: Option<JoinHandle<()>>,
}

impl BuildLogger {
    /// Creates a logger if `-Zbuild-analysis` is enabled.
    pub fn maybe_new(ws: &Workspace<'_>) -> CargoResult<Option<Self>> {
        let analysis = ws.gctx().build_config()?.analysis.as_ref();
        match (analysis, ws.gctx().cli_unstable().build_analysis) {
            (Some(analysis), true) if analysis.enabled => Ok(Some(Self::new(ws)?)),
            (Some(_), false) => {
                ws.gctx().shell().warn(
                    "ignoring 'build.analysis' config, pass `-Zbuild-analysis` to enable it",
                )?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn new(ws: &Workspace<'_>) -> CargoResult<Self> {
        let run_id = Self::generate_run_id(ws);

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

        Ok(Self {
            tx: ManuallyDrop::new(tx),
            run_id,
            handle: Some(handle),
        })
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
        let _ = self.tx.send(msg);
    }
}

impl Drop for BuildLogger {
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
