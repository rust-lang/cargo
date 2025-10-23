//! Messages for logging.

use jiff::Timestamp;
use serde::Serialize;

/// A log message.
///
/// Each variant represents a different type of event.
#[derive(Serialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum LogMessage {}

impl LogMessage {
    /// Converts this message to a JSON log line with run_id and timestamp.
    pub fn to_json_log(&self, run_id: &str) -> String {
        #[derive(Serialize)]
        struct LogEntry<'a> {
            run_id: &'a str,
            timestamp: Timestamp,
            #[serde(flatten)]
            msg: &'a LogMessage,
        }

        let entry = LogEntry {
            run_id,
            timestamp: Timestamp::now(),
            msg: self,
        };

        serde_json::to_string(&entry).unwrap()
    }
}
