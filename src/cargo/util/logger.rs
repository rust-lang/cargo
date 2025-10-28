//! Build analysis logging infrastructure.

use std::io::{BufWriter, Write};
use std::mem::ManuallyDrop;
use std::path::Path;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

use cargo_util::paths;

use crate::CargoResult;
use crate::core::Workspace;
use crate::util::log_message::LogMessage;
use crate::util::short_hash;

/// Logger for `-Zbuild-analysis`.
pub struct BuildLogger {
    tx: ManuallyDrop<Sender<LogMessage>>,
    run_id: String,
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
        let run_id = Self::generate_run_id(ws)?;

        let log_dir = ws.gctx().home().join("log");
        paths::create_dir_all(log_dir.as_path_unlocked())?;

        let filename = format!("{run_id}.jsonl");
        let log_file = log_dir.open_rw_exclusive_create(
            Path::new(&filename),
            ws.gctx(),
            "build analysis log",
        )?;

        let (tx, rx) = mpsc::channel::<LogMessage>();

        let run_id_clone = run_id.clone();
        let handle = std::thread::spawn(move || {
            let mut writer = BufWriter::new(log_file);
            for msg in rx {
                let _ = msg.write_json_log(&mut writer, &run_id_clone);
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
    ///
    /// The format is `{timestamp}-{hash}`, with `:` and `.` in the timestamp
    /// removed to make it safe for filenames.
    /// For example, `20251024T194502773638Z-f891d525d52ecab3`.
    pub fn generate_run_id(ws: &Workspace<'_>) -> CargoResult<String> {
        let hash = short_hash(&ws.root());
        let timestamp = jiff::Timestamp::now().to_string().replace([':', '.'], "");
        Ok(format!("{timestamp}-{hash}"))
    }

    /// Returns the run ID for this build session.
    pub fn run_id(&self) -> &str {
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
