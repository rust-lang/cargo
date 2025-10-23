//! Messages for logging.

use std::io::Write;

use jiff::Timestamp;
use serde::Serialize;

/// A log message.
///
/// Each variant represents a different type of event.
#[derive(Serialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum LogMessage {}

impl LogMessage {
    /// Serializes this message as a JSON log line directly to the writer.
    pub fn write_json_log<W: Write>(&self, writer: &mut W, run_id: &str) -> std::io::Result<()> {
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

        serde_json::to_writer(&mut *writer, &entry)?;
        writer.write_all(b"\n")?;
        Ok(())
    }
}
