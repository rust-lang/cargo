use anyhow::Error;

pub mod man;
pub mod md;
pub mod text;

pub trait Formatter {
    /// Renders the given markdown to the formatter's output.
    fn render(&self, input: &str) -> Result<String, Error>;
    /// Renders the start of a block of options (triggered by `{{#options}}`).
    fn render_options_start(&self) -> &'static str;
    /// Renders the end of a block of options (triggered by `{{/options}}`).
    fn render_options_end(&self) -> &'static str;
    /// Renders an option (triggered by `{{#option}}`).
    fn render_option(&self, params: &[&str], block: &str, man_name: &str) -> Result<String, Error>;
    /// Converts a man page reference into markdown that is appropriate for this format.
    ///
    /// Triggered by `{{man name section}}`.
    fn linkify_man_to_md(&self, name: &str, section: u8) -> Result<String, Error>;
}
