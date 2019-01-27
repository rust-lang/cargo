use std::fmt;
use std::iter;
use std::str::{self, FromStr};

use crate::util::CargoResult;

#[derive(Eq, PartialEq, Hash, Ord, PartialOrd, Clone, Debug)]
pub enum Cfg {
    Name(String),
    KeyPair(String, String),
}

#[derive(Eq, PartialEq, Hash, Ord, PartialOrd, Clone, Debug)]
pub enum CfgExpr {
    Not(Box<CfgExpr>),
    All(Vec<CfgExpr>),
    Any(Vec<CfgExpr>),
    Value(Cfg),
}

#[derive(PartialEq)]
enum Token<'a> {
    LeftParen,
    RightParen,
    Ident(&'a str),
    Comma,
    Equals,
    String(&'a str),
}

struct Tokenizer<'a> {
    s: iter::Peekable<str::CharIndices<'a>>,
    orig: &'a str,
}

struct Parser<'a> {
    t: iter::Peekable<Tokenizer<'a>>,
}

impl FromStr for Cfg {
    type Err = failure::Error;

    fn from_str(s: &str) -> CargoResult<Cfg> {
        let mut p = Parser::new(s);
        let e = p.cfg()?;
        if p.t.next().is_some() {
            failure::bail!("malformed cfg value or key/value pair: `{}`", s)
        }
        Ok(e)
    }
}

impl fmt::Display for Cfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Cfg::Name(ref s) => s.fmt(f),
            Cfg::KeyPair(ref k, ref v) => write!(f, "{} = \"{}\"", k, v),
        }
    }
}

impl CfgExpr {
    /// Utility function to check if the key, "cfg(..)" matches the `target_cfg`
    pub fn matches_key(key: &str, target_cfg: &[Cfg]) -> bool {
        if key.starts_with("cfg(") && key.ends_with(')') {
            let cfg = &key[4..key.len() - 1];

            CfgExpr::from_str(cfg)
                .ok()
                .map(|ce| ce.matches(target_cfg))
                .unwrap_or(false)
        } else {
            false
        }
    }

    pub fn matches(&self, cfg: &[Cfg]) -> bool {
        match *self {
            CfgExpr::Not(ref e) => !e.matches(cfg),
            CfgExpr::All(ref e) => e.iter().all(|e| e.matches(cfg)),
            CfgExpr::Any(ref e) => e.iter().any(|e| e.matches(cfg)),
            CfgExpr::Value(ref e) => cfg.contains(e),
        }
    }
}

impl FromStr for CfgExpr {
    type Err = failure::Error;

    fn from_str(s: &str) -> CargoResult<CfgExpr> {
        let mut p = Parser::new(s);
        let e = p.expr()?;
        if p.t.next().is_some() {
            failure::bail!(
                "can only have one cfg-expression, consider using all() or \
                 any() explicitly"
            )
        }
        Ok(e)
    }
}

impl fmt::Display for CfgExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            CfgExpr::Not(ref e) => write!(f, "not({})", e),
            CfgExpr::All(ref e) => write!(f, "all({})", CommaSep(e)),
            CfgExpr::Any(ref e) => write!(f, "any({})", CommaSep(e)),
            CfgExpr::Value(ref e) => write!(f, "{}", e),
        }
    }
}

struct CommaSep<'a, T>(&'a [T]);

impl<'a, T: fmt::Display> fmt::Display for CommaSep<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, v) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", v)?;
        }
        Ok(())
    }
}

impl<'a> Parser<'a> {
    fn new(s: &'a str) -> Parser<'a> {
        Parser {
            t: Tokenizer {
                s: s.char_indices().peekable(),
                orig: s,
            }
            .peekable(),
        }
    }

    fn expr(&mut self) -> CargoResult<CfgExpr> {
        match self.t.peek() {
            Some(&Ok(Token::Ident(op @ "all"))) | Some(&Ok(Token::Ident(op @ "any"))) => {
                self.t.next();
                let mut e = Vec::new();
                self.eat(&Token::LeftParen)?;
                while !self.r#try(&Token::RightParen) {
                    e.push(self.expr()?);
                    if !self.r#try(&Token::Comma) {
                        self.eat(&Token::RightParen)?;
                        break;
                    }
                }
                if op == "all" {
                    Ok(CfgExpr::All(e))
                } else {
                    Ok(CfgExpr::Any(e))
                }
            }
            Some(&Ok(Token::Ident("not"))) => {
                self.t.next();
                self.eat(&Token::LeftParen)?;
                let e = self.expr()?;
                self.eat(&Token::RightParen)?;
                Ok(CfgExpr::Not(Box::new(e)))
            }
            Some(&Ok(..)) => self.cfg().map(CfgExpr::Value),
            Some(&Err(..)) => Err(self.t.next().unwrap().err().unwrap()),
            None => failure::bail!(
                "expected start of a cfg expression, \
                 found nothing"
            ),
        }
    }

    fn cfg(&mut self) -> CargoResult<Cfg> {
        match self.t.next() {
            Some(Ok(Token::Ident(name))) => {
                let e = if self.r#try(&Token::Equals) {
                    let val = match self.t.next() {
                        Some(Ok(Token::String(s))) => s,
                        Some(Ok(t)) => failure::bail!("expected a string, found {}", t.classify()),
                        Some(Err(e)) => return Err(e),
                        None => failure::bail!("expected a string, found nothing"),
                    };
                    Cfg::KeyPair(name.to_string(), val.to_string())
                } else {
                    Cfg::Name(name.to_string())
                };
                Ok(e)
            }
            Some(Ok(t)) => failure::bail!("expected identifier, found {}", t.classify()),
            Some(Err(e)) => Err(e),
            None => failure::bail!("expected identifier, found nothing"),
        }
    }

    fn r#try(&mut self, token: &Token<'a>) -> bool {
        match self.t.peek() {
            Some(&Ok(ref t)) if token == t => {}
            _ => return false,
        }
        self.t.next();
        true
    }

    fn eat(&mut self, token: &Token<'a>) -> CargoResult<()> {
        match self.t.next() {
            Some(Ok(ref t)) if token == t => Ok(()),
            Some(Ok(t)) => failure::bail!("expected {}, found {}", token.classify(), t.classify()),
            Some(Err(e)) => Err(e),
            None => failure::bail!("expected {}, but cfg expr ended", token.classify()),
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = CargoResult<Token<'a>>;

    fn next(&mut self) -> Option<CargoResult<Token<'a>>> {
        loop {
            match self.s.next() {
                Some((_, ' ')) => {}
                Some((_, '(')) => return Some(Ok(Token::LeftParen)),
                Some((_, ')')) => return Some(Ok(Token::RightParen)),
                Some((_, ',')) => return Some(Ok(Token::Comma)),
                Some((_, '=')) => return Some(Ok(Token::Equals)),
                Some((start, '"')) => {
                    while let Some((end, ch)) = self.s.next() {
                        if ch == '"' {
                            return Some(Ok(Token::String(&self.orig[start + 1..end])));
                        }
                    }
                    return Some(Err(failure::format_err!("unterminated string in cfg")));
                }
                Some((start, ch)) if is_ident_start(ch) => {
                    while let Some(&(end, ch)) = self.s.peek() {
                        if !is_ident_rest(ch) {
                            return Some(Ok(Token::Ident(&self.orig[start..end])));
                        } else {
                            self.s.next();
                        }
                    }
                    return Some(Ok(Token::Ident(&self.orig[start..])));
                }
                Some((_, ch)) => {
                    return Some(Err(failure::format_err!(
                        "unexpected character in \
                         cfg `{}`, expected parens, \
                         a comma, an identifier, or \
                         a string",
                        ch
                    )));
                }
                None => return None,
            }
        }
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ('a' <= ch && ch <= 'z') || ('A' <= ch && ch <= 'Z')
}

fn is_ident_rest(ch: char) -> bool {
    is_ident_start(ch) || ('0' <= ch && ch <= '9')
}

impl<'a> Token<'a> {
    fn classify(&self) -> &str {
        match *self {
            Token::LeftParen => "`(`",
            Token::RightParen => "`)`",
            Token::Ident(..) => "an identifier",
            Token::Comma => "`,`",
            Token::Equals => "`=`",
            Token::String(..) => "a string",
        }
    }
}
