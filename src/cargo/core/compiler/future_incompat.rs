use serde::{Deserialize, Serialize};

/// The future incompatibility report, emitted by the compiler as a JSON message.
#[derive(serde::Deserialize)]
pub struct FutureIncompatReport {
    pub future_incompat_report: Vec<FutureBreakageItem>,
}

#[derive(Serialize, Deserialize)]
pub struct FutureBreakageItem {
    /// The date at which this lint will become an error.
    /// Currently unused
    pub future_breakage_date: Option<String>,
    /// The original diagnostic emitted by the compiler
    pub diagnostic: Diagnostic,
}

/// A diagnostic emitted by the compiler as a JSON message.
/// We only care about the 'rendered' field
#[derive(Serialize, Deserialize)]
pub struct Diagnostic {
    pub rendered: String,
}

/// The filename in the top-level `target` directory where we store
/// the report
pub const FUTURE_INCOMPAT_FILE: &str = ".future-incompat-report.json";

#[derive(Serialize, Deserialize)]
pub struct OnDiskReport {
    // A Cargo-generated id used to detect when a report has been overwritten
    pub id: String,
    // Cannot be a &str, since Serde needs
    // to be able to un-escape the JSON
    pub report: String,
}
