use anstyle::*;

pub const NOP: Style = Style::new();
pub const HEADER: Style = AnsiColor::BrightGreen.on_default().effects(Effects::BOLD);
pub const USAGE: Style = AnsiColor::BrightGreen.on_default().effects(Effects::BOLD);
pub const LITERAL: Style = AnsiColor::BrightCyan.on_default().effects(Effects::BOLD);
pub const PLACEHOLDER: Style = AnsiColor::Cyan.on_default();
pub const ERROR: Style = annotate_snippets::renderer::DEFAULT_ERROR_STYLE;
pub const WARN: Style = annotate_snippets::renderer::DEFAULT_WARNING_STYLE;
pub const NOTE: Style = annotate_snippets::renderer::DEFAULT_NOTE_STYLE;
pub const GOOD: Style = AnsiColor::BrightGreen.on_default().effects(Effects::BOLD);
pub const VALID: Style = AnsiColor::BrightCyan.on_default().effects(Effects::BOLD);
pub const INVALID: Style = annotate_snippets::renderer::DEFAULT_WARNING_STYLE;
pub const TRANSIENT: Style = annotate_snippets::renderer::DEFAULT_HELP_STYLE;
