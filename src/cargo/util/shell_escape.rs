// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::borrow::Cow;

pub fn escape(s: Cow<str>) -> Cow<str> {
    if cfg!(unix) {
        unix::escape(s)
    } else {
        windows::escape(s)
    }
}

pub mod windows {
    use std::borrow::Cow;

    const SPACE: char = ' ';
    const ESCAPE_CHAR: char = '\\';
    const QUOTE_CHAR: char = '"';
    const BACKSLASH: char = '\\';
    const SLASH: char = '/';

    /// Escape characters that may have special meaning in a shell,
    /// including spaces.
    ///
    /// Escape double quotes and spaces by wrapping the string in double quotes.
    ///
    /// Turn all backslashes into forward slashes.
    pub fn escape(s: Cow<str>) -> Cow<str> {
        // check if string needs to be escaped
        let mut has_spaces = false;
        let mut has_backslashes = false;
        let mut has_quotes = false;
        for ch in s.chars() {
            match ch {
                QUOTE_CHAR => has_quotes = true,
                SPACE => has_spaces = true,
                BACKSLASH => has_backslashes = true,
                _ => {}
            }
        }
        if !has_spaces && !has_backslashes && !has_quotes {
            return s
        }
        let mut es = String::with_capacity(s.len());
        if has_spaces {
            es.push(QUOTE_CHAR);
        }
        for ch in s.chars() {
            match ch {
                BACKSLASH => { es.push(SLASH); continue }
                QUOTE_CHAR => es.push(ESCAPE_CHAR),
                _ => {}
            }
            es.push(ch)
        }
        if has_spaces {
            es.push(QUOTE_CHAR);
        }
        es.into()
    }

    #[test]
    fn test_escape() {
        assert_eq!(escape("--aaa=bbb-ccc".into()), "--aaa=bbb-ccc");
        assert_eq!(escape("linker=gcc -L/foo -Wl,bar".into()),
                          r#""linker=gcc -L/foo -Wl,bar""#);
        assert_eq!(escape(r#"--features="default""#.into()),
                          r#"--features=\"default\""#);
        assert_eq!(escape(r#"\path\to\my documents\"#.into()),
                          r#""/path/to/my documents/""#);
    }
}

pub mod unix {
    use std::borrow::Cow;

    const SHELL_SPECIAL: &'static str = r#" \$'"`!"#;

    /// Escape characters that may have special meaning in a shell,
    /// including spaces.
    pub fn escape(s: Cow<str>) -> Cow<str> {
        let escape_char = '\\';
        // check if string needs to be escaped
        let clean = SHELL_SPECIAL.chars().all(|sp_char| !s.contains(sp_char));
        if clean {
            return s
        }
        let mut es = String::with_capacity(s.len());
        for ch in s.chars() {
            if SHELL_SPECIAL.contains(ch) {
                es.push(escape_char);
            }
            es.push(ch)
        }
        es.into()
    }

    #[test]
    fn test_escape() {
        assert_eq!(escape("--aaa=bbb-ccc".into()), "--aaa=bbb-ccc");
        assert_eq!(escape("linker=gcc -L/foo -Wl,bar".into()),
                          r#"linker=gcc\ -L/foo\ -Wl,bar"#);
        assert_eq!(escape(r#"--features="default""#.into()),
                          r#"--features=\"default\""#);
        assert_eq!(escape(r#"'!\$`\\\n "#.into()),
                          r#"\'\!\\\$\`\\\\\\n\ "#);
    }
}
