//! Messages for logging.

use std::borrow::Cow;
use std::io::Write;
use std::path::PathBuf;

use cargo_util_schemas::core::PackageIdSpec;
use jiff::Timestamp;
use serde::Deserialize;
use serde::Serialize;

use crate::core::compiler::CompileMode;
use crate::core::compiler::fingerprint::DirtyReason;

/// A log message.
///
/// Each variant represents a different type of event.
#[derive(Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum LogMessage {
    /// Emitted when a build starts.
    BuildStarted {
        /// Current working directory.
        cwd: PathBuf,
        /// Host triple.
        host: String,
        /// Number of parallel jobs.
        jobs: u32,
        /// Available parallelism of the compilation environment.
        num_cpus: Option<u64>,
        /// Build profile name (e.g., "dev", "release").
        profile: String,
        /// The rustc version (`1.23.4-beta.2`).
        rustc_version: String,
        /// The rustc verbose version information (the output of `rustc -vV`).
        rustc_version_verbose: String,
        /// Target directory for build artifacts.
        target_dir: PathBuf,
        /// Workspace root directory.
        workspace_root: PathBuf,
    },
    /// Emitted when resolving dependencies starts.
    ResolutionStarted {
        /// Seconds elapsed from build start.
        elapsed: f64,
    },
    /// Emitted when resolving dependencies finishes.
    ResolutionFinished {
        /// Seconds elapsed from build start.
        elapsed: f64,
    },
    /// Emitted when unit graph generation starts.
    UnitGraphStarted {
        /// Seconds elapsed from build start.
        elapsed: f64,
    },
    /// Emitted when unit graph generation finishes.
    UnitGraphFinished {
        /// Seconds elapsed from build start.
        elapsed: f64,
    },
    /// Emitted when a compilation unit is registered in the unit graph,
    /// right before [`LogMessage::UnitGraphFinished`] that Cargo finalizes
    /// the unit graph.
    UnitRegistered {
        /// Package ID specification.
        package_id: PackageIdSpec,
        /// Cargo target (lib, bin, example, etc.).
        target: Target,
        /// The compilation action this unit is for (check, build, test, etc.).
        mode: CompileMode,
        /// The target platform this unit builds for.
        ///
        /// It is either a [target triple] the compiler accepts,
        /// or a file name with the `json` extension for a [custom target].
        ///
        /// [target triple]: https://doc.rust-lang.org/nightly/rustc/platform-support.html
        /// [custom target]: https://doc.rust-lang.org/nightly/rustc/targets/custom.html
        platform: String,
        /// Unit index for compact reference in subsequent events.
        index: u64,
        /// Enabled features.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        features: Vec<String>,
        /// Whether this is requested to build by user directly,
        /// like via the `-p` flag or the default workspace members.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        requested: bool,
    },
    /// Emitted when a compilation unit starts.
    UnitStarted {
        /// Unit index from the associated unit-registered event.
        index: u64,
        /// Seconds elapsed from build start.
        elapsed: f64,
    },
    /// Emitted when a section (e.g., rmeta, link) of the compilation unit finishes.
    UnitRmetaFinished {
        /// Unit index from the associated unit-registered event.
        index: u64,
        /// Seconds elapsed from build start.
        elapsed: f64,
        /// Unit indices that were unblocked by this rmeta completion.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        unblocked: Vec<u64>,
    },
    /// Emitted when a section (e.g., rmeta, link) of the compilation unit starts.
    ///
    /// Requires `-Zsection-timings` to be enabled.
    UnitSectionStarted {
        /// Unit index from the associated unit-registered event.
        index: u64,
        /// Seconds elapsed from build start.
        elapsed: f64,
        /// Section name from rustc's `-Zjson=timings` (e.g., "codegen", "link").
        section: String,
    },
    /// Emitted when a section (e.g., rmeta, link) of the compilation unit finishes.
    ///
    /// Requires `-Zsection-timings` to be enabled.
    UnitSectionFinished {
        /// Unit index from the associated unit-registered event.
        index: u64,
        /// Seconds elapsed from build start.
        elapsed: f64,
        /// Section name from rustc's `-Zjson=timings` (e.g., "codegen", "link").
        section: String,
    },
    /// Emitted when a compilation unit finishes.
    UnitFinished {
        /// Unit index from the associated unit-registered event.
        index: u64,
        /// Seconds elapsed from build start.
        elapsed: f64,
        /// Unit indices that were unblocked by this completion.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        unblocked: Vec<u64>,
    },
    /// Emitted when rebuild fingerprint information is determined for a unit.
    UnitFingerprint {
        /// Unit index from the associated unit-registered event.
        index: u64,
        /// Status of the rebuild detection fingerprint of this unit
        status: FingerprintStatus,
        /// Reason why the unit is dirty and needs rebuilding.
        #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
        cause: Option<DirtyReason>,
    },
}

/// Cargo target information.
#[derive(Serialize, Deserialize)]
pub struct Target {
    /// Target name.
    pub name: String,
    /// Target kind (lib, bin, test, bench, example, build-script).
    pub kind: Cow<'static, str>,
}

/// Status of the rebuild detection fingerprint.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FingerprintStatus {
    /// There is no previous fingerprints for this unit.
    /// Might be a brand-new build.
    New,
    /// The current fingerprint doesn't match the previous fingerprints.
    /// Rebuild needed.
    Dirty,
    /// The current fingerprint matches the previous fingerprints.
    /// No rebuild needed.
    Fresh,
}

impl From<&crate::core::Target> for Target {
    fn from(target: &crate::core::Target) -> Self {
        use crate::core::TargetKind;
        Self {
            name: target.name().to_string(),
            kind: match target.kind() {
                TargetKind::Lib(..) => "lib",
                TargetKind::Bin => "bin",
                TargetKind::Test => "test",
                TargetKind::Bench => "bench",
                TargetKind::ExampleLib(..) | TargetKind::ExampleBin => "example",
                TargetKind::CustomBuild => "build-script",
            }
            .into(),
        }
    }
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
