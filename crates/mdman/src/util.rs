///! General utilities.
use crate::EventIter;
use anyhow::{bail, format_err, Context, Error};
use pulldown_cmark::{CowStr, Event, Tag};

/// Splits the text `foo(1)` into "foo" and `1`.
pub fn parse_name_and_section(text: &str) -> Result<(&str, u8), Error> {
    let mut i = text.split_terminator(&['(', ')'][..]);
    let name = i
        .next()
        .ok_or_else(|| format_err!("man reference must have a name"))?;
    let section = i
        .next()
        .ok_or_else(|| format_err!("man reference must have a section such as mycommand(1)"))?;
    if let Some(s) = i.next() {
        bail!(
            "man reference must have the form mycommand(1), got extra part `{}`",
            s
        );
    }
    let section: u8 = section
        .parse()
        .with_context(|| format!("section must be a number, got {}", section))?;
    Ok((name, section))
}

/// Extracts the text from a header after Tag::Heading has been received.
pub fn header_text<'e>(parser: &mut EventIter<'e>) -> Result<CowStr<'e>, Error> {
    let text = match parser.next() {
        Some((Event::Text(t), _range)) => t,
        e => bail!("expected plain text in man header, got {:?}", e),
    };
    match parser.next() {
        Some((Event::End(Tag::Heading(..)), _range)) => {
            return Ok(text);
        }
        e => bail!("expected plain text in man header, got {:?}", e),
    }
}

/// Removes tags from the front and back of a string.
pub fn unwrap<'t>(text: &'t str, front: &str, back: &str) -> &'t str {
    text.trim().trim_start_matches(front).trim_end_matches(back)
}
