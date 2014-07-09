use std::fmt;
use std::str::CharOffsets;
use semver::Version;
use util::{CargoResult, internal};

#[deriving(PartialEq,Clone)]
pub struct VersionReq {
    predicates: Vec<Predicate>
}

#[deriving(PartialEq,Clone)]
enum Op {
    Ex,   // Exact
    Gt,   // Greater than
    GtEq, // Greater than or equal to
    Lt,   // Less than
    LtEq  // Less than or equal to
}

#[deriving(PartialEq,Clone)]
struct Predicate {
    op: Op,
    major: uint,
    minor: Option<uint>,
    patch: Option<uint>
}

struct PredBuilder {
    op: Option<Op>,
    major: Option<uint>,
    minor: Option<uint>,
    patch: Option<uint>
}


impl VersionReq {
    pub fn any() -> VersionReq {
        VersionReq { predicates: vec!() }
    }

    pub fn parse(input: &str) -> CargoResult<VersionReq> {
        let mut lexer = Lexer::new(input);
        let mut builder = PredBuilder::new();
        let mut predicates = Vec::new();

        for token in lexer {
            match token {
                Sigil(x) => try!(builder.set_sigil(x)),
                AlphaNum(x) => try!(builder.set_version_part(x)),
                Dot => (), // Nothing to do for now
                _ => unimplemented!()
            }
        }

        if lexer.is_error() {
            return Err(internal("invalid version requirement"));
        }

        predicates.push(try!(builder.build()));

        Ok(VersionReq { predicates: predicates })
    }

    pub fn exact(version: &Version) -> VersionReq {
        VersionReq { predicates: vec!(Predicate::exact(version)) }
    }

    pub fn matches(&self, version: &Version) -> bool {
        self.predicates.iter().all(|p| p.matches(version))
    }
}

impl Predicate {
    pub fn exact(version: &Version) -> Predicate {
        Predicate {
            op: Ex,
            major: version.major,
            minor: Some(version.minor),
            patch: Some(version.patch)
        }
    }

    pub fn matches(&self, ver: &Version) -> bool {
        match self.op {
            Ex => self.is_exact(ver),
            Gt => self.is_greater(ver),
            GtEq => self.is_exact(ver) || self.is_greater(ver),
            _ => false // not implemented
        }
    }

    fn is_exact(&self, ver: &Version) -> bool {
        if self.major != ver.major {
            return false;
        }

        match self.minor {
            Some(minor) => {
                if minor != ver.minor {
                    return false;
                }
            }
            None => return true
        }

        match self.patch {
            Some(patch) => {
                if patch != ver.patch {
                    return false;
                }
            }
            None => return true
        }

        true
    }

    fn is_greater(self, ver: &Version) -> bool {
        if self.major != ver.major {
            return self.major > ver.major;
        }

        match self.minor {
            Some(minor) => {
                if minor != ver.minor {
                    return minor > ver.minor
                }
            }
            None => return false
        }

        match self.patch {
            Some(patch) => {
                if patch != ver.patch {
                    return patch > ver.patch
                }
            }

            None => return false
        }

        false
    }
}

impl PredBuilder {
    fn new() -> PredBuilder {
        PredBuilder {
            op: None,
            major: None,
            minor: None,
            patch: None
        }
    }

    fn set_sigil(&mut self, sigil: &str) -> CargoResult<()> {
        if self.op.is_some() {
            return Err(internal("op already set"));
        }

        match Op::from_sigil(sigil) {
            Some(op) => self.op = Some(op),
            _ => return Err(internal("invalid sigil"))
        }

        Ok(())
    }

    fn set_version_part(&mut self, part: &str) -> CargoResult<()> {
        if self.op.is_none() {
            // If no op is specified, then the predicate is an exact match on
            // the version
            self.op = Some(Ex);
        }

        if self.major.is_none() {
            self.major = Some(try!(parse_version_part(part)));
        }
        else if self.minor.is_none() {
            self.minor = Some(try!(parse_version_part(part)));
        }
        else if self.patch.is_none() {
            self.patch = Some(try!(parse_version_part(part)));
        }

        Ok(())
    }

    /// Validates that a version predicate can be created given the present
    /// information.
    fn build(&self) -> CargoResult<Predicate> {
        let op = match self.op {
            Some(x) => x,
            None => return Err(internal("op required"))
        };

        let major = match self.major {
            Some(x) => x,
            None => return Err(internal("major version required"))
        };

        Ok(Predicate {
            op: op,
            major: major,
            minor: self.minor,
            patch: self.patch
        })
    }
}

struct Lexer<'a> {
    c: char,
    idx: uint,
    iter: CharOffsets<'a>,
    mark: Option<uint>,
    input: &'a str,
    state: LexState
}

#[deriving(Show,PartialEq)]
enum LexState {
    LexInit,
    LexStart,
    LexAlphaNum,
    LexSigil,
    LexErr,
    LexWin
}

#[deriving(Show)]
enum Token<'a> {
    Sigil(&'a str),
    AlphaNum(&'a str),
    Comma,
    Dot
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Lexer<'a> {
        Lexer {
            c: '\0',
            idx: 0,
            iter: input.char_indices(),
            mark: None,
            input: input,
            state: LexInit
        }
    }

    fn is_error(&self) -> bool {
        self.state == LexErr
    }

    fn mark(&mut self, at: uint) {
        self.mark = Some(at)
    }

    fn flush(&mut self, to: uint, kind: LexState) -> Option<Token<'a>> {
        match self.mark {
            Some(mark) => {
                if to <= mark {
                    return None;
                }

                let s = self.input.slice(mark, to);

                self.mark = None;

                match kind {
                    LexAlphaNum => Some(AlphaNum(s)),
                    LexSigil => Some(Sigil(s)),
                    _ => None // bug
                }
            }
            None => None
        }
    }
}

impl<'a> Iterator<Token<'a>> for Lexer<'a> {
    fn next(&mut self) -> Option<Token<'a>> {
        let mut c;
        let mut idx = 0;

        macro_rules! next(
            () => (
                match self.iter.next() {
                    Some((n_idx, n_char)) => {
                        c = n_char;
                        idx = n_idx;
                    }
                    _ => {
                      let s = self.state;
                      return self.flush(idx + 1, s)
                    }
                }
            ))

        macro_rules! flush(
            ($s:expr) => ({
                self.c = c;
                self.idx = idx;
                self.flush(idx, $s)
            }))


        if self.state == LexInit {
            self.state = LexStart;
            next!();
        }
        else {
            c = self.c;
            idx = self.idx;
        }

        loop {
            match self.state {
                LexStart => {
                    if c.is_whitespace() {
                        next!(); // Ignore
                    }
                    else if c.is_alphanumeric() {
                        self.mark(idx);
                        self.state = LexAlphaNum;
                        next!();
                    }
                    else if is_sigil(c) {
                        self.mark(idx);
                        self.state = LexSigil;
                        next!();
                    }
                    else if c == '.' {
                        self.state = LexInit;
                        return Some(Dot);
                    }
                    else if c == ',' {
                        self.state = LexInit;
                        return Some(Comma);
                    }
                    else {
                        self.state = LexErr;
                        return None;
                    }
                }
                LexAlphaNum => {
                    if c.is_alphanumeric() {
                        next!();
                    }
                    else {
                        self.state = LexStart;
                        return flush!(LexAlphaNum);
                    }
                }
                LexSigil => {
                    if is_sigil(c) {
                        next!();
                    }
                    else {
                        self.state = LexStart;
                        return flush!(LexSigil);
                    }
                }
                LexErr | LexWin => return None,
                LexInit => return None // bug
            }
        }
    }
}

impl Op {
    fn from_sigil(sigil: &str) -> Option<Op> {
        match sigil {
            "=" => Some(Ex),
            ">" => Some(Gt),
            ">=" => Some(GtEq),
            "<" => Some(Lt),
            "<=" => Some(LtEq),
            _ => None
        }
    }
}

fn parse_version_part(s: &str) -> CargoResult<uint> {
    let mut ret = 0;

    for c in s.chars() {
        let n = (c as uint) - ('0' as uint);

        if n > 9 {
            return Err(internal("version components must be numeric"));
        }

        ret *= 10;
        ret +=  n;
    }

    Ok(ret)
}

fn is_sigil(c: char) -> bool {
    match c {
        '>' | '<' | '=' | '~' | '^' => true,
        _ => false
    }
}

impl fmt::Show for VersionReq {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.predicates.is_empty() {
            try!(write!(fmt, "*"));
        }
        else {
            for (i, ref pred) in self.predicates.iter().enumerate() {
                if i == 0 {
                    try!(write!(fmt, "{}", pred));
                }
                else {
                    try!(write!(fmt, ", {}", pred));
                }
            }
        }

        Ok(())
    }
}

impl fmt::Show for Predicate {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(fmt, "{} {}", self.op, self.major));

        match self.minor {
            Some(v) => try!(write!(fmt, ".{}", v)),
            None => ()
        }

        match self.patch {
            Some(v) => try!(write!(fmt, ".{}", v)),
            None => ()
        }

        Ok(())
    }
}

impl fmt::Show for Op {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Ex => try!(write!(fmt, "=")),
            Gt => try!(write!(fmt, ">")),
            GtEq => try!(write!(fmt, ">=")),
            Lt => try!(write!(fmt, "<")),
            LtEq => try!(write!(fmt, "<="))
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::VersionReq;
    use semver;

    fn req(s: &str) -> VersionReq {
        VersionReq::parse(s).unwrap()
    }

    fn version(s: &str) -> semver::Version {
        match semver::parse(s) {
            Some(v) => v,
            None => fail!("`{}` is not a valid version", s)
        }
    }

    fn assert_match(req: &VersionReq, vers: &[&str]) {
        for ver in vers.iter() {
            assert!(req.matches(&version(*ver)), "did not match {}", ver);
        }
    }

    fn assert_not_match(req: &VersionReq, vers: &[&str]) {
        for ver in vers.iter() {
            assert!(!req.matches(&version(*ver)), "matched {}", ver);
        }
    }

    #[test]
    pub fn test_parsing_exact() {
        let r = req("1.0.0");

        assert!(r.to_string() == "= 1.0.0".to_string());

        assert_match(&r, ["1.0.0"]);
        assert_not_match(&r, ["1.0.1", "0.9.9", "0.10.0", "0.1.0"]);

        let r = req("0.9.0");

        assert!(r.to_string() == "= 0.9.0".to_string());

        assert_match(&r, ["0.9.0"]);
        assert_not_match(&r, ["0.9.1", "1.9.0", "0.0.9"]);
    }

    #[test]
    pub fn test_parsing_greater_than() {
        let r = req(">= 1.0.0");

        assert!(r.to_string() == ">= 1.0.0".to_string());

        assert_match(&r, ["1.0.0"]);
    }

    /* TODO:
     * - Test parse errors
     * - Handle pre releases
     */
}
