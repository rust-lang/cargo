//! Rustc Diagnostic JSON Output
//! 
//! The following data types are copied from [rust-lang/rust](https://github.com/rust-lang/rust/blob/de78655bca47cac8e783dbb563e7e5c25c1fae40/src/libsyntax/json.rs)

#[derive(Serialize, Deserialize, Debug)]
pub struct Diagnostic {
    /// The primary error message.
    message: String,
    code: Option<DiagnosticCode>,
    /// "error: internal compiler error", "error", "warning", "note", "help".
    level: String,
    spans: Vec<DiagnosticSpan>,
    /// Associated diagnostic messages.
    children: Vec<Diagnostic>,
    /// The message as rustc would render it. Currently this is only
    /// `Some` for "suggestions", but eventually it will include all
    /// snippets.
    rendered: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct DiagnosticSpan {
    file_name: String,
    byte_start: u32,
    byte_end: u32,
    /// 1-based.
    line_start: usize,
    line_end: usize,
    /// 1-based, character offset.
    column_start: usize,
    column_end: usize,
    /// Is this a "primary" span -- meaning the point, or one of the points,
    /// where the error occurred?
    is_primary: bool,
    /// Source text from the start of line_start to the end of line_end.
    text: Vec<DiagnosticSpanLine>,
    /// Label that should be placed at this location (if any)
    label: Option<String>,
    /// If we are suggesting a replacement, this will contain text
    /// that should be sliced in atop this span. You may prefer to
    /// load the fully rendered version from the parent `Diagnostic`,
    /// however.
    suggested_replacement: Option<String>,
    /// Macro invocations that created the code at this span, if any.
    expansion: Option<Box<DiagnosticSpanMacroExpansion>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct DiagnosticSpanLine {
    text: String,

    /// 1-based, character offset in self.text.
    highlight_start: usize,

    highlight_end: usize,
}

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
struct DiagnosticCode {
    /// The code itself.
    code: String,
    /// An explanation for the code.
    explanation: Option<String>,
}
