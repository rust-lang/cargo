// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[cfg(not(target_os = "windows"))]
pub use self::windows::shell_escape;


mod windows {
    use std::borrow::Cow;

    pub fn shell_escape(s: Cow<str>) -> Cow<str> {
        panic!()
    }
}

#[cfg(target_os = "windows")]
pub use self::other::shell_escape;

mod other {
    use std::borrow::Cow;

    static SHELL_SPECIAL: &'static str = r#" \$'"`!"#;

    #[cfg(not(target_os = "windows"))]
    /// Escape characters that may have special meaning in a shell,
    /// including spaces.
    pub fn shell_escape(s: Cow<str>) -> Cow<str> {
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
    fn test_shell_escape() {
        assert_eq!(shell_escape("--aaa=bbb-ccc".into()), "--aaa=bbb-ccc");
        assert_eq!(shell_escape("linker=gcc -L/foo -Wl,bar".into()),
                                r#"linker=gcc\ -L/foo\ -Wl,bar"#);
        assert_eq!(shell_escape(r#"--features="default""#.into()),
                                r#"--features=\"default\""#);
        assert_eq!(shell_escape(r#"'!\$`\\\n "#.into()),
                                r#"\'\!\\\$\`\\\\\\n\ "#);
    }
}
