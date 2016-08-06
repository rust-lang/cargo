use std::str::{self, FromStr};
use std::iter;
use std::fmt;

use util::{CargoError, CargoResult, human};

#[derive(Clone, PartialEq, Debug)]
pub enum Cfg {
    Name(String),
    KeyPair(String, String),
}

#[derive(Clone, PartialEq, Debug)]
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
    type Err = Box<CargoError>;

    fn from_str(s: &str) -> CargoResult<Cfg> {
        let mut p = Parser::new(s);
        let e = try!(p.cfg());
        if p.t.next().is_some() {
            bail!("malformed cfg value or key/value pair")
        }
        Ok(e)
    }
}

impl fmt::Display for Cfg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Cfg::Name(ref s) => s.fmt(f),
            Cfg::KeyPair(ref k, ref v) => write!(f, "{} = \"{}\"", k, v),
        }
    }
}

impl CfgExpr {
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
    type Err = Box<CargoError>;

    fn from_str(s: &str) -> CargoResult<CfgExpr> {
        let mut p = Parser::new(s);
        let e = try!(p.expr());
        if p.t.next().is_some() {
            bail!("can only have one cfg-expression, consider using all() or \
                   any() explicitly")
        }
        Ok(e)
    }
}

impl fmt::Display for CfgExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CfgExpr::Not(ref e) => write!(f, "not({})", e),
            CfgExpr::All(ref e) => write!(f, "all({})", CommaSep(e)),
            CfgExpr::Any(ref e) => write!(f, "any({})", CommaSep(e)),
            CfgExpr::Value(ref e) => write!(f, "{}", e),
        }
    }
}

struct CommaSep<'a, T: 'a>(&'a [T]);

impl<'a, T: fmt::Display> fmt::Display for CommaSep<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, v) in self.0.iter().enumerate() {
            if i > 0 {
                try!(write!(f, ", "));
            }
            try!(write!(f, "{}", v));
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
            }.peekable(),
        }
    }

    fn expr(&mut self) -> CargoResult<CfgExpr> {
        match self.t.peek() {
            Some(&Ok(Token::Ident(op @ "all"))) |
            Some(&Ok(Token::Ident(op @ "any"))) => {
                self.t.next();
                let mut e = Vec::new();
                try!(self.eat(Token::LeftParen));
                while !self.try(Token::RightParen) {
                    e.push(try!(self.expr()));
                    if !self.try(Token::Comma) {
                        try!(self.eat(Token::RightParen));
                        break
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
                try!(self.eat(Token::LeftParen));
                let e = try!(self.expr());
                try!(self.eat(Token::RightParen));
                Ok(CfgExpr::Not(Box::new(e)))
            }
            Some(&Ok(..)) => self.cfg().map(CfgExpr::Value),
            Some(&Err(..)) => {
                Err(self.t.next().unwrap().err().unwrap())
            }
            None => bail!("expected start of a cfg expression, \
                           found nothing"),
        }
    }

    fn cfg(&mut self) -> CargoResult<Cfg> {
        match self.t.next() {
            Some(Ok(Token::Ident(name))) => {
                let e = if self.try(Token::Equals) {
                    let val = match self.t.next() {
                        Some(Ok(Token::String(s))) => s,
                        Some(Ok(t)) => bail!("expected a string, found {}",
                                             t.classify()),
                        Some(Err(e)) => return Err(e),
                        None => bail!("expected a string, found nothing"),
                    };
                    Cfg::KeyPair(name.to_string(), val.to_string())
                } else {
                    Cfg::Name(name.to_string())
                };
                Ok(e)
            }
            Some(Ok(t)) => bail!("expected identifier, found {}", t.classify()),
            Some(Err(e)) => Err(e),
            None => bail!("expected identifier, found nothing"),
        }
    }

    fn try(&mut self, token: Token<'a>) -> bool {
        match self.t.peek() {
            Some(&Ok(ref t)) if token == *t => {}
            _ => return false,
        }
        self.t.next();
        true
    }

    fn eat(&mut self, token: Token<'a>) -> CargoResult<()> {
        match self.t.next() {
            Some(Ok(ref t)) if token == *t => Ok(()),
            Some(Ok(t)) => bail!("expected {}, found {}", token.classify(),
                                 t.classify()),
            Some(Err(e)) => Err(e),
            None => bail!("expected {}, but cfg expr ended", token.classify()),
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
                            return Some(Ok(Token::String(&self.orig[start+1..end])))
                        }
                    }
                    return Some(Err(human("unterminated string in cfg".to_string())))
                }
                Some((start, ch)) if is_ident_start(ch) => {
                    while let Some(&(end, ch)) = self.s.peek() {
                        if !is_ident_rest(ch) {
                            return Some(Ok(Token::Ident(&self.orig[start..end])))
                        } else {
                            self.s.next();
                        }
                    }
                    return Some(Ok(Token::Ident(&self.orig[start..])))
                }
                Some((_, ch)) => {
                    return Some(Err(human(format!("unexpected character in \
                                                   cfg `{}`, expected parens, \
                                                   a comma, an identifier, or \
                                                   a string", ch))))
                }
                None => return None
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
