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
use std::env;

pub fn escape(s: Cow<str>) -> Cow<str> {
    if cfg!(unix) || env::var("MSYSTEM").is_ok() {
        unix::escape(s)
    } else {
        windows::escape(s)
    }
}

pub mod windows {
    use std::borrow::Cow;
    use std::iter::repeat;

    /// Escape for the windows cmd.exe shell, for more info see this url:
    ///
    /// http://blogs.msdn.com/b/twistylittlepassagesallalike/archive/2011/04/23
    ///                      /everyone-quotes-arguments-the-wrong-way.aspx
    pub fn escape(s: Cow<str>) -> Cow<str> {
        let mut needs_escape = false;
        for ch in s.chars() {
            match ch {
                '"' | '\t' | '\n' | ' ' => needs_escape = true,
                _ => {}
            }
        }
        if !needs_escape {
            return s
        }
        let mut es = String::with_capacity(s.len());
        es.push('"');
        let mut chars = s.chars().peekable();
        loop {
            let mut nslashes = 0;
            while let Some(&'\\') = chars.peek() {
                chars.next();
                nslashes += 1;
            }

            match chars.next() {
                Some('"') => {
                    es.extend(repeat('\\').take(nslashes * 2 + 1));
                    es.push('"');
                }
                Some(c) => {
                    es.extend(repeat('\\').take(nslashes));
                    es.push(c);
                }
                None => {
                    es.extend(repeat('\\').take(nslashes * 2));
                    break;
                }
            }

        }
        es.push('"');
        es.into()
    }

    #[test]
    fn test_escape() {
        assert_eq!(escape("--aaa=bbb-ccc".into()), "--aaa=bbb-ccc");
        assert_eq!(escape("linker=gcc -L/foo -Wl,bar".into()),
                          r#""linker=gcc -L/foo -Wl,bar""#);
        assert_eq!(escape(r#"--features="default""#.into()),
                          r#""--features=\"default\"""#);
        assert_eq!(escape(r#"\path\to\my documents\"#.into()),
                          r#""\path\to\my documents\\""#);
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
