//! Parser for the `--format` string for `cargo tree`.

use std::iter;
use std::str;

pub enum RawChunk<'a> {
    /// Raw text to include in the output.
    Text(&'a str),
    /// A substitution to place in the output. For example, the argument "p"
    /// emits the package name.
    Argument(&'a str),
    /// Indicates an error in the format string. The given string is a
    /// human-readable message explaining the error.
    Error(&'static str),
}

/// `cargo tree` format parser.
///
/// The format string indicates how each package should be displayed. It
/// includes simple markers surrounded in curly braces that will be
/// substituted with their corresponding values. For example, the text
/// "{p} license:{l}" will substitute the `{p}` with the package name/version
/// (and optionally source), and the `{l}` will be the license from
/// `Cargo.toml`.
///
/// Substitutions are alphabetic characters between curly braces, like `{p}`
/// or `{foo}`. The actual interpretation of these are done in the `Pattern`
/// struct.
///
/// Bare curly braces can be included in the output with double braces like
/// `{{` will include a single `{`, similar to Rust's format strings.
pub struct Parser<'a> {
    s: &'a str,
    it: iter::Peekable<str::CharIndices<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(s: &'a str) -> Parser<'a> {
        Parser {
            s,
            it: s.char_indices().peekable(),
        }
    }

    fn consume(&mut self, ch: char) -> bool {
        match self.it.peek() {
            Some(&(_, c)) if c == ch => {
                self.it.next();
                true
            }
            _ => false,
        }
    }

    fn argument(&mut self) -> RawChunk<'a> {
        RawChunk::Argument(self.name())
    }

    fn name(&mut self) -> &'a str {
        let start = match self.it.peek() {
            Some(&(pos, ch)) if ch.is_alphabetic() => {
                self.it.next();
                pos
            }
            _ => return "",
        };

        loop {
            match self.it.peek() {
                Some(&(_, ch)) if ch.is_alphanumeric() => {
                    self.it.next();
                }
                Some(&(end, _)) => return &self.s[start..end],
                None => return &self.s[start..],
            }
        }
    }

    fn text(&mut self, start: usize) -> RawChunk<'a> {
        while let Some(&(pos, ch)) = self.it.peek() {
            match ch {
                '{' | '}' => return RawChunk::Text(&self.s[start..pos]),
                _ => {
                    self.it.next();
                }
            }
        }
        RawChunk::Text(&self.s[start..])
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = RawChunk<'a>;

    fn next(&mut self) -> Option<RawChunk<'a>> {
        match self.it.peek() {
            Some(&(_, '{')) => {
                self.it.next();
                if self.consume('{') {
                    Some(RawChunk::Text("{"))
                } else {
                    let chunk = self.argument();
                    if self.consume('}') {
                        Some(chunk)
                    } else {
                        for _ in &mut self.it {}
                        Some(RawChunk::Error("expected '}'"))
                    }
                }
            }
            Some(&(_, '}')) => {
                self.it.next();
                if self.consume('}') {
                    Some(RawChunk::Text("}"))
                } else {
                    Some(RawChunk::Error("unexpected '}'"))
                }
            }
            Some(&(i, _)) => Some(self.text(i)),
            None => None,
        }
    }
}
