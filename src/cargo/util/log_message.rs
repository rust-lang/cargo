//! Messages for logging.

use std::path::Path;

use jiff::Timestamp;
use serde::Serialize;

/// A log message.
///
/// Each variant represents a different type of event.
#[derive(Serialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum LogMessage<'a> {
    /// Emitted when a build starts.
    BuildStarted {
        cwd: &'a Path,
        host: &'a str,
        jobs: u32,
        profile: &'a str,
        rustc_version: &'a str,
        rustc_version_verbose: &'a str,
        target_dir: &'a Path,
        workspace_root: &'a Path,
    },
}

impl LogMessage<'_> {
    /// Converts this message to a JSON log line with run_id and timestamp.
    pub fn to_json_log(&self, run_id: &str) -> String {
        #[derive(Serialize)]
        struct LogEntry<'a> {
            run_id: &'a str,
            timestamp: Timestamp,
            #[serde(flatten)]
            msg: &'a LogMessage<'a>,
        }

        let entry = LogEntry {
            run_id,
            timestamp: Timestamp::now(),
            msg: self,
        };

        serde_json::to_string(&entry).unwrap()
    }
}
