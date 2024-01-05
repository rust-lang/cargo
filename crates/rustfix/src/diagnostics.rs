//! Rustc Diagnostic JSON Output.
//!
//! The following data types are copied from [rust-lang/rust](https://github.com/rust-lang/rust/blob/4fd68eb47bad1c121417ac4450b2f0456150db86/compiler/rustc_errors/src/json.rs).
//!
//! For examples of the JSON output, see JSON fixture files under `tests/` directory.

use serde::Deserialize;

/// The root diagnostic JSON output emitted by the compiler.
#[derive(Clone, Deserialize, Debug, Hash, Eq, PartialEq)]
pub struct Diagnostic {
    /// The primary error message.
    pub message: String,
    pub code: Option<DiagnosticCode>,
    /// "error: internal compiler error", "error", "warning", "note", "help".
    level: String,
    pub spans: Vec<DiagnosticSpan>,
    /// Associated diagnostic messages.
    pub children: Vec<Diagnostic>,
    /// The message as rustc would render it.
    pub rendered: Option<String>,
}

/// Span information of a diagnostic item.
#[derive(Clone, Deserialize, Debug, Hash, Eq, PartialEq)]
pub struct DiagnosticSpan {
    pub file_name: String,
    pub byte_start: u32,
    pub byte_end: u32,
    /// 1-based.
    pub line_start: usize,
    pub line_end: usize,
    /// 1-based, character offset.
    pub column_start: usize,
    pub column_end: usize,
    /// Is this a "primary" span -- meaning the point, or one of the points,
    /// where the error occurred?
    pub is_primary: bool,
    /// Source text from the start of line_start to the end of line_end.
    pub text: Vec<DiagnosticSpanLine>,
    /// Label that should be placed at this location (if any)
    label: Option<String>,
    /// If we are suggesting a replacement, this will contain text
    /// that should be sliced in atop this span.
    pub suggested_replacement: Option<String>,
    /// If the suggestion is approximate
    pub suggestion_applicability: Option<Applicability>,
    /// Macro invocations that created the code at this span, if any.
    expansion: Option<Box<DiagnosticSpanMacroExpansion>>,
}

/// Indicates the confidence in the correctness of a suggestion.
///
/// All suggestions are marked with an `Applicability`. Tools use the applicability of a suggestion
/// to determine whether it should be automatically applied or if the user should be consulted
/// before applying the suggestion.
#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Hash, Eq)]
pub enum Applicability {
    /// The suggestion is definitely what the user intended, or maintains the exact meaning of the code.
    /// This suggestion should be automatically applied.
    ///
    /// In case of multiple `MachineApplicable` suggestions (whether as part of
    /// the same `multipart_suggestion` or not), all of them should be
    /// automatically applied.
    MachineApplicable,

    /// The suggestion may be what the user intended, but it is uncertain. The suggestion should
    /// result in valid Rust code if it is applied.
    MaybeIncorrect,

    /// The suggestion contains placeholders like `(...)` or `{ /* fields */ }`. The suggestion
    /// cannot be applied automatically because it will not result in valid Rust code. The user
    /// will need to fill in the placeholders.
    HasPlaceholders,

    /// The applicability of the suggestion is unknown.
    Unspecified,
}

/// Span information of a single line.
#[derive(Clone, Deserialize, Debug, Eq, PartialEq, Hash)]
pub struct DiagnosticSpanLine {
    pub text: String,

    /// 1-based, character offset in self.text.
    pub highlight_start: usize,

    pub highlight_end: usize,
}

/// Span information for macro expansions.
#[derive(Clone, Deserialize, Debug, Eq, PartialEq, Hash)]
struct DiagnosticSpanMacroExpansion {
    /// span where macro was applied to generate this code; note that
    /// this may itself derive from a macro (if
    /// `span.expansion.is_some()`)
    span: DiagnosticSpan,

    /// name of macro that was applied (e.g., "foo!" or "#[derive(Eq)]")
    macro_decl_name: String,

    /// span where macro was defined (if known)
    def_site_span: Option<DiagnosticSpan>,
}

/// The error code emitted by the compiler. See [Rust error codes index].
///
/// [Rust error codes index]: https://doc.rust-lang.org/error_codes/error-index.html
#[derive(Clone, Deserialize, Debug, Eq, PartialEq, Hash)]
pub struct DiagnosticCode {
    /// The code itself.
    pub code: String,
    /// An explanation for the code.
    explanation: Option<String>,
}
