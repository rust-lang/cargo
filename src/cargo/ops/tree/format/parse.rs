use std::iter;
use std::str;

pub enum RawChunk<'a> {
    Text(&'a str),
    Argument(&'a str),
    Error(&'static str),
}

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
                '{' | '}' | ')' => return RawChunk::Text(&self.s[start..pos]),
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
                Some(RawChunk::Error("unexpected '}'"))
            }
            Some(&(i, _)) => Some(self.text(i)),
            None => None,
        }
    }
}
