//! Messages for logging.

use std::io::Write;
use std::path::PathBuf;

use cargo_util_schemas::core::PackageIdSpec;
use jiff::Timestamp;
use serde::Serialize;

use crate::core::Target;
use crate::core::compiler::CompilationSection;
use crate::core::compiler::CompileMode;
use crate::core::compiler::fingerprint::DirtyReason;

/// A log message.
///
/// Each variant represents a different type of event.
#[derive(Serialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum LogMessage {
    /// Emitted when a build starts.
    BuildStarted {
        cwd: PathBuf,
        host: String,
        jobs: u32,
        profile: String,
        rustc_version: String,
        rustc_version_verbose: String,
        target_dir: PathBuf,
        workspace_root: PathBuf,
    },
    /// Emitted when a compilation unit finishes.
    TimingInfo {
        package_id: PackageIdSpec,
        target: Target,
        mode: CompileMode,
        duration: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        rmeta_time: Option<f64>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        sections: Vec<(String, CompilationSection)>,
    },
    /// Emitted when a unit needs to be rebuilt.
    Rebuild {
        package_id: PackageIdSpec,
        target: Target,
        mode: CompileMode,
        cause: DirtyReason,
    },
}

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
