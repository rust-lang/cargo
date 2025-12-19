//! Build analysis logging infrastructure.

use std::hash::Hash;
use std::io::BufWriter;
use std::io::Write as _;
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
pub struct BuildLogger<'gctx> {
    gctx: &'gctx crate::util::context::GlobalContext,
    /// Whether to write to console.
    to_console: bool,
    /// Channel to background thread writing log file.
    tx: Option<ManuallyDrop<Sender<LogMessage>>>,
    run_id: RunId,
    run_id_str: String,
    handle: Option<JoinHandle<()>>,
}

impl<'gctx> BuildLogger<'gctx> {
    /// Creates a logger if `-Zbuild-analysis` is enabled.
    pub fn maybe_new(ws: &Workspace<'gctx>, emit_json_messages: bool) -> CargoResult<Option<Self>> {
        let analysis = ws.gctx().build_config()?.analysis.as_ref();
        match (analysis, ws.gctx().cli_unstable().build_analysis) {
            (Some(analysis), true) if analysis.enabled.unwrap_or_default() => {
                let to_console = analysis.console.unwrap_or(false);
                let to_file = analysis.file.unwrap_or(true);

                if to_console && !emit_json_messages {
                    ws.gctx().shell().warn(
                        "ignoring `build.analysis.console` config, pass a JSON `--message-format` option to enable it"
                    )?;
                }
                let to_console = to_console && emit_json_messages;

                if to_file || to_console{
                    Ok(Some(Self::new(ws, to_console, to_file)?))
                } else {
                    Ok(None)
                }
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

    fn new(ws: &Workspace<'gctx>, to_console: bool, to_file: bool) -> CargoResult<Self> {
        let run_id = Self::generate_run_id(ws);
        let run_id_str = run_id.to_string();
        let gctx = ws.gctx();

        let (tx, handle) = if to_file {
            let log_dir = gctx.home().join("log");
            paths::create_dir_all(log_dir.as_path_unlocked())?;

            let filename = format!("{run_id}.jsonl");
            let log_file = log_dir.open_rw_exclusive_create(
                Path::new(&filename),
                gctx,
                "build analysis log",
            )?;

            let (tx, rx) = mpsc::channel::<LogMessage>();

            let run_id_str = run_id_str.clone();
            let handle = std::thread::spawn(move || {
                let mut writer = BufWriter::new(log_file);
                for msg in rx {
                    let _ = msg.write_json_log(&mut writer, &run_id_str);
                }
                let _ = writer.flush();
            });

            (Some(ManuallyDrop::new(tx)), Some(handle))
        } else {
            (None, None)
        };

        Ok(Self {
            gctx,
            to_console,
            tx,
            run_id,
            run_id_str,
            handle,
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
        if self.to_console {
            let mut shell = self.gctx.shell();
            let _ = msg.write_json_log(&mut shell.out(), &self.run_id_str);
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(msg);
        }
    }

    /// Batch-Logs multiple messages.
    ///
    /// This should be used when logging many messages in a tight loop.
    ///
    /// Avoids allocation when only one output destination (console or file) is enabled.
    pub fn log_batch(&self, messages: impl IntoIterator<Item = LogMessage>) {
        if let Some(tx) = &self.tx {
            if self.to_console {
                let mut shell = self.gctx.shell();
                for msg in messages {
                    let _ = msg.write_json_log(&mut shell.out(), &self.run_id_str);
                    let _ = tx.send(msg);
                }
            } else {
                for msg in messages {
                    let _ = tx.send(msg);
                }
            }
        } else if self.to_console {
            let mut shell = self.gctx.shell();
            for msg in messages {
                let _ = msg.write_json_log(&mut shell.out(), &self.run_id_str);
            }
        }
    }
}

impl Drop for BuildLogger<'_> {
    fn drop(&mut self) {
        // SAFETY: tx is dropped exactly once here to signal thread shutdown.
        // ManuallyDrop prevents automatic drop after this impl runs.
        if let Some(tx) = &mut self.tx {
            unsafe {
                ManuallyDrop::drop(tx);
            }
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
