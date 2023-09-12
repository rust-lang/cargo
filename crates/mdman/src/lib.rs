//! mdman markdown to man converter.

use anyhow::{bail, Context, Error};
use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};
use std::ops::Range;
use std::path::Path;
use url::Url;

mod format;
mod hbs;
mod util;

use format::Formatter;

/// Mapping of `(name, section)` of a man page to a URL.
pub type ManMap = HashMap<(String, u8), String>;

/// A man section.
pub type Section = u8;

/// The output formats supported by mdman.
#[derive(Copy, Clone)]
pub enum Format {
    Man,
    Md,
    Text,
}

impl Format {
    /// The filename extension for the format.
    pub fn extension(&self, section: Section) -> String {
        match self {
            Format::Man => section.to_string(),
            Format::Md => "md".to_string(),
            Format::Text => "txt".to_string(),
        }
    }
}

/// Converts the handlebars markdown file at the given path into the given
/// format, returning the translated result.
pub fn convert(
    file: &Path,
    format: Format,
    url: Option<Url>,
    man_map: ManMap,
) -> Result<String, Error> {
    let formatter: Box<dyn Formatter + Send + Sync> = match format {
        Format::Man => Box::new(format::man::ManFormatter::new(url)),
        Format::Md => Box::new(format::md::MdFormatter::new(man_map)),
        Format::Text => Box::new(format::text::TextFormatter::new(url)),
    };
    let expanded = hbs::expand(file, &*formatter)?;
    // pulldown-cmark can behave a little differently with Windows newlines,
    // just normalize it.
    let expanded = expanded.replace("\r\n", "\n");
    formatter.render(&expanded)
}

/// Pulldown-cmark iterator yielding an `(event, range)` tuple.
type EventIter<'a> = Box<dyn Iterator<Item = (Event<'a>, Range<usize>)> + 'a>;

/// Creates a new markdown parser with the given input.
pub(crate) fn md_parser(input: &str, url: Option<Url>) -> EventIter<'_> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    let parser = Parser::new_ext(input, options);
    let parser = parser.into_offset_iter();
    // Translate all links to include the base url.
    let parser = parser.map(move |(event, range)| match event {
        Event::Start(Tag::Link(lt, dest_url, title)) if !matches!(lt, LinkType::Email) => (
            Event::Start(Tag::Link(lt, join_url(url.as_ref(), dest_url), title)),
            range,
        ),
        Event::End(Tag::Link(lt, dest_url, title)) if !matches!(lt, LinkType::Email) => (
            Event::End(Tag::Link(lt, join_url(url.as_ref(), dest_url), title)),
            range,
        ),
        _ => (event, range),
    });
    Box::new(parser)
}

fn join_url<'a>(base: Option<&Url>, dest: CowStr<'a>) -> CowStr<'a> {
    match base {
        Some(base_url) => {
            // Absolute URL or page-relative anchor doesn't need to be translated.
            if dest.contains(':') || dest.starts_with('#') {
                dest
            } else {
                let joined = base_url.join(&dest).unwrap_or_else(|e| {
                    panic!("failed to join URL `{}` to `{}`: {}", dest, base_url, e)
                });
                String::from(joined).into()
            }
        }
        None => dest,
    }
}

pub fn extract_section(file: &Path) -> Result<Section, Error> {
    let f = fs::File::open(file).with_context(|| format!("could not open `{}`", file.display()))?;
    let mut f = io::BufReader::new(f);
    let mut line = String::new();
    f.read_line(&mut line)?;
    if !line.starts_with("# ") {
        bail!("expected input file to start with # header");
    }
    let (_name, section) = util::parse_name_and_section(&line[2..].trim()).with_context(|| {
        format!(
            "expected input file to have header with the format `# command-name(1)`, found: `{}`",
            line
        )
    })?;
    Ok(section)
}
