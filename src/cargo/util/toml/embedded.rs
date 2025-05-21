use cargo_util_schemas::manifest::PackageName;

use crate::util::restricted_names;
use crate::CargoResult;

pub(super) fn expand_manifest(content: &str) -> CargoResult<String> {
    let source = ScriptSource::parse(content)?;
    if let Some(frontmatter) = source.frontmatter() {
        match source.info() {
            Some("cargo") | None => {}
            Some(other) => {
                if let Some(remainder) = other.strip_prefix("cargo,") {
                    anyhow::bail!("cargo does not support frontmatter infostring attributes like `{remainder}` at this time")
                } else {
                    anyhow::bail!("frontmatter infostring `{other}` is unsupported by cargo; specify `cargo` for embedding a manifest")
                }
            }
        }

        Ok(frontmatter.to_owned())
    } else {
        let frontmatter = "";
        Ok(frontmatter.to_owned())
    }
}

/// Ensure the package name matches the validation from `ops::cargo_new::check_name`
pub fn sanitize_name(name: &str) -> String {
    let placeholder = if name.contains('_') {
        '_'
    } else {
        // Since embedded manifests only support `[[bin]]`s, prefer arrow-case as that is the
        // more common convention for CLIs
        '-'
    };

    let mut name = PackageName::sanitize(name, placeholder).into_inner();

    loop {
        if restricted_names::is_keyword(&name) {
            name.push(placeholder);
        } else if restricted_names::is_conflicting_artifact_name(&name) {
            // Being an embedded manifest, we always assume it is a `[[bin]]`
            name.push(placeholder);
        } else if name == "test" {
            name.push(placeholder);
        } else if restricted_names::is_windows_reserved(&name) {
            // Go ahead and be consistent across platforms
            name.push(placeholder);
        } else {
            break;
        }
    }

    name
}

#[derive(Debug)]
pub struct ScriptSource<'s> {
    shebang: Option<&'s str>,
    info: Option<&'s str>,
    frontmatter: Option<&'s str>,
    content: &'s str,
}

impl<'s> ScriptSource<'s> {
    pub fn parse(input: &'s str) -> CargoResult<Self> {
        let mut source = Self {
            shebang: None,
            info: None,
            frontmatter: None,
            content: input,
        };

        if let Some(shebang_end) = strip_shebang(source.content) {
            let (shebang, content) = source.content.split_at(shebang_end);
            source.shebang = Some(shebang);
            source.content = content;
        }

        let mut rest = source.content;

        // Whitespace may precede a frontmatter but must end with a newline
        const WHITESPACE: [char; 4] = [' ', '\t', '\r', '\n'];
        let trimmed = rest.trim_start_matches(WHITESPACE);
        if trimmed.len() != rest.len() {
            let trimmed_len = rest.len() - trimmed.len();
            let last_trimmed_index = trimmed_len - 1;
            if rest.as_bytes()[last_trimmed_index] != b'\n' {
                // either not a frontmatter or invalid opening
                return Ok(source);
            }
        }
        rest = trimmed;

        // Opens with a line that starts with 3 or more `-` followed by an optional identifier
        const FENCE_CHAR: char = '-';
        let fence_length = rest
            .char_indices()
            .find_map(|(i, c)| (c != FENCE_CHAR).then_some(i))
            .unwrap_or(rest.len());
        match fence_length {
            0 => {
                return Ok(source);
            }
            1 | 2 => {
                // either not a frontmatter or invalid frontmatter opening
                anyhow::bail!(
                    "found {fence_length} `{FENCE_CHAR}` in rust frontmatter, expected at least 3"
                )
            }
            _ => {}
        }
        let (fence_pattern, rest) = rest.split_at(fence_length);
        let Some(info_end_index) = rest.find('\n') else {
            anyhow::bail!("no closing `{fence_pattern}` found for frontmatter");
        };
        let (info, rest) = rest.split_at(info_end_index);
        let info = info.trim_matches(WHITESPACE);
        if !info.is_empty() {
            source.info = Some(info);
        }
        let rest = rest
            .strip_prefix('\n')
            .expect("earlier `found` + `split_at` left us here");

        // Ends with a line that starts with a matching number of `-` only followed by whitespace
        let nl_fence_pattern = format!("\n{fence_pattern}");
        let Some(frontmatter_nl) = rest.find(&nl_fence_pattern) else {
            anyhow::bail!("no closing `{fence_pattern}` found for frontmatter");
        };
        let frontmatter = &rest[..frontmatter_nl + 1];
        let rest = &rest[frontmatter_nl + nl_fence_pattern.len()..];
        source.frontmatter = Some(frontmatter);

        let (after_closing_fence, rest) = rest.split_once("\n").unwrap_or((rest, ""));
        let after_closing_fence = after_closing_fence.trim_matches(WHITESPACE);
        if !after_closing_fence.is_empty() {
            // extra characters beyond the original fence pattern, even if they are extra `-`
            anyhow::bail!("trailing characters found after frontmatter close");
        }

        let frontmatter_len = input.len() - rest.len();
        source.content = &input[frontmatter_len..];

        Ok(source)
    }

    pub fn shebang(&self) -> Option<&'s str> {
        self.shebang
    }

    pub fn info(&self) -> Option<&'s str> {
        self.info
    }

    pub fn frontmatter(&self) -> Option<&'s str> {
        self.frontmatter
    }

    pub fn content(&self) -> &'s str {
        self.content
    }
}

fn strip_shebang(input: &str) -> Option<usize> {
    // See rust-lang/rust's compiler/rustc_lexer/src/lib.rs's `strip_shebang`
    // Shebang must start with `#!` literally, without any preceding whitespace.
    // For simplicity we consider any line starting with `#!` a shebang,
    // regardless of restrictions put on shebangs by specific platforms.
    if let Some(rest) = input.strip_prefix("#!") {
        // Ok, this is a shebang but if the next non-whitespace token is `[`,
        // then it may be valid Rust code, so consider it Rust code.
        //
        // NOTE: rustc considers line and block comments to be whitespace but to avoid
        // any more awareness of Rust grammar, we are excluding it.
        if !rest.trim_start().starts_with('[') {
            // No other choice than to consider this a shebang.
            let newline_end = input.find('\n').map(|pos| pos + 1).unwrap_or(input.len());
            return Some(newline_end);
        }
    }
    None
}

#[cfg(test)]
mod test_expand {
    use snapbox::assert_data_eq;
    use snapbox::prelude::*;
    use snapbox::str;

    use super::*;

    #[track_caller]
    fn assert_source(source: &str, expected: impl IntoData) {
        use std::fmt::Write as _;

        let actual = match ScriptSource::parse(source) {
            Ok(actual) => actual,
            Err(err) => panic!("unexpected err: {err}"),
        };

        let mut rendered = String::new();
        write_optional_field(&mut rendered, "shebang", actual.shebang());
        write_optional_field(&mut rendered, "info", actual.info());
        write_optional_field(&mut rendered, "frontmatter", actual.frontmatter());
        writeln!(&mut rendered, "content: {:?}", actual.content()).unwrap();
        assert_data_eq!(rendered, expected.raw());
    }

    fn write_optional_field(writer: &mut dyn std::fmt::Write, field: &str, value: Option<&str>) {
        if let Some(value) = value {
            writeln!(writer, "{field}: {value:?}").unwrap();
        } else {
            writeln!(writer, "{field}: None").unwrap();
        }
    }

    #[track_caller]
    fn assert_err(
        result: Result<impl std::fmt::Debug, impl std::fmt::Display>,
        err: impl IntoData,
    ) {
        match result {
            Ok(d) => panic!("unexpected Ok({d:#?})"),
            Err(actual) => snapbox::assert_data_eq!(actual.to_string(), err.raw()),
        }
    }

    #[test]
    fn rustc_dot_in_infostring_leading() {
        assert_source(
            r#"---.toml
//~^ ERROR: invalid infostring for frontmatter
---

// infostrings cannot have leading dots

fn main() {}
"#,
            str![[r#"
shebang: None
info: ".toml"
frontmatter: "//~^ ERROR: invalid infostring for frontmatter\n"
content: "\n// infostrings cannot have leading dots\n\nfn main() {}\n"

"#]],
        );
    }

    #[test]
    fn rustc_dot_in_infostring_non_leading() {
        assert_err(
            ScriptSource::parse(
                r#"---Cargo.toml
---

// infostrings can contain dots as long as a dot isn't the first character.
//@ check-pass

fn main() {}
"#,
            ),
            str!["no closing `---` found for frontmatter"],
        );
    }

    #[test]
    fn rustc_escape() {
        assert_source(
            r#"----

---

----

//@ check-pass

// This test checks that longer dashes for opening and closing can be used to
// escape sequences such as three dashes inside the frontmatter block.

fn main() {}
"#,
            str![[r#"
shebang: None
info: None
frontmatter: "\n---\n\n"
content: "\n//@ check-pass\n\n// This test checks that longer dashes for opening and closing can be used to\n// escape sequences such as three dashes inside the frontmatter block.\n\nfn main() {}\n"

"#]],
        );
    }

    #[test]
    fn rustc_extra_after_end() {
        assert_err(
            ScriptSource::parse(
                r#"---
---cargo
//~^ ERROR: extra characters after frontmatter close are not allowed

fn main() {}
"#,
            ),
            str!["no closing `---` found for frontmatter"],
        );
    }

    #[test]
    fn rustc_frontmatter_after_tokens() {
        assert_source(
            r#"#![feature(frontmatter)]

---
//~^ ERROR: expected item, found `-`
// FIXME(frontmatter): make this diagnostic better
---

// frontmatters must be at the start of a file. This test ensures that.

fn main() {}
"#,
            str![[r##"
shebang: None
info: None
frontmatter: None
content: "#![feature(frontmatter)]\n\n---\n//~^ ERROR: expected item, found `-`\n// FIXME(frontmatter): make this diagnostic better\n---\n\n// frontmatters must be at the start of a file. This test ensures that.\n\nfn main() {}\n"

"##]],
        );
    }

    #[test]
    fn rustc_frontmatter_non_lexible_tokens() {
        assert_source(
            r#"---uwu
🏳️‍⚧️
---

//@ check-pass

// check that frontmatter blocks can have tokens that are otherwise not accepted by
// the lexer as Rust code.

fn main() {}
"#,
            str![[r#"
shebang: None
info: "uwu"
frontmatter: "🏳\u{fe0f}\u{200d}⚧\u{fe0f}\n"
content: "\n//@ check-pass\n\n// check that frontmatter blocks can have tokens that are otherwise not accepted by\n// the lexer as Rust code.\n\nfn main() {}\n"

"#]],
        );
    }

    #[test]
    fn rustc_frontmatter_whitespace_1() {
        assert_source(
            r#"  ---
//~^ ERROR: invalid preceding whitespace for frontmatter opening
  ---
//~^ ERROR: invalid preceding whitespace for frontmatter close

// check that whitespaces should not precede the frontmatter opening or close.

fn main() {}
"#,
            str![[r#"
shebang: None
info: None
frontmatter: None
content: "  ---\n//~^ ERROR: invalid preceding whitespace for frontmatter opening\n  ---\n//~^ ERROR: invalid preceding whitespace for frontmatter close\n\n// check that whitespaces should not precede the frontmatter opening or close.\n\nfn main() {}\n"

"#]],
        );
    }

    #[test]
    fn rustc_frontmatter_whitespace_2() {
        assert_err(
            ScriptSource::parse(
                r#"---cargo

//@ compile-flags: --crate-type lib

fn foo(x: i32) -> i32 {
    ---x
    //~^ ERROR: invalid preceding whitespace for frontmatter close
    //~| ERROR: extra characters after frontmatter close are not allowed
}
//~^ ERROR: unexpected closing delimiter: `}`

// this test is for the weird case that valid Rust code can have three dashes
// within them and get treated as a frontmatter close.
"#,
            ),
            str!["no closing `---` found for frontmatter"],
        );
    }

    #[test]
    fn rustc_frontmatter_whitespace_3() {
        assert_err(
            ScriptSource::parse(
                r#"


---cargo   
---   

// please note the whitespace characters after the first four lines.
// This ensures that we accept whitespaces before the frontmatter, after
// the frontmatter opening and the frontmatter close.

//@ check-pass
// ignore-tidy-end-whitespace
// ignore-tidy-leading-newlines

fn main() {}
"#,
            ),
            str!["no closing `---` found for frontmatter"],
        );
    }

    #[test]
    fn rustc_frontmatter_whitespace_4() {
        assert_err(
            ScriptSource::parse(
                r#"--- cargo
---

//@ check-pass
// A frontmatter infostring can have leading whitespace.

fn main() {}
"#,
            ),
            str!["no closing `---` found for frontmatter"],
        );
    }

    #[test]
    fn rustc_infostring_fail() {
        assert_source(
            r#"
---cargo,clippy
//~^ ERROR: invalid infostring for frontmatter
---

// infostrings can only be a single identifier.

fn main() {}
"#,
            str![[r#"
shebang: None
info: "cargo,clippy"
frontmatter: "//~^ ERROR: invalid infostring for frontmatter\n"
content: "\n// infostrings can only be a single identifier.\n\nfn main() {}\n"

"#]],
        );
    }

    #[test]
    fn rustc_mismatch_1() {
        assert_err(
            ScriptSource::parse(
                r#"---cargo
//~^ ERROR: frontmatter close does not match the opening
----

// there must be the same number of dashes for both the opening and the close
// of the frontmatter.

fn main() {}
"#,
            ),
            str!["trailing characters found after frontmatter close"],
        );
    }

    #[test]
    fn rustc_mismatch_2() {
        assert_err(
            ScriptSource::parse(
                r#"----cargo
//~^ ERROR: frontmatter close does not match the opening
---cargo
//~^ ERROR: extra characters after frontmatter close are not allowed

fn main() {}
"#,
            ),
            str!["no closing `----` found for frontmatter"],
        );
    }

    #[test]
    fn rustc_multifrontmatter_2() {
        assert_source(
            r#"---
 ---
//~^ ERROR: invalid preceding whitespace for frontmatter close

 ---
//~^ ERROR: expected item, found `-`
// FIXME(frontmatter): make this diagnostic better
---

fn main() {}
"#,
            str![[r#"
shebang: None
info: None
frontmatter: " ---\n//~^ ERROR: invalid preceding whitespace for frontmatter close\n\n ---\n//~^ ERROR: expected item, found `-`\n// FIXME(frontmatter): make this diagnostic better\n"
content: "\nfn main() {}\n"

"#]],
        );
    }

    #[test]
    fn rustc_multifrontmatter() {
        assert_source(
            r#"---
---

---
//~^ ERROR: expected item, found `-`
// FIXME(frontmatter): make this diagnostic better
---

// test that we do not parse another frontmatter block after the first one.

fn main() {}
"#,
            str![[r#"
shebang: None
info: None
frontmatter: "---\n\n"
content: "//~^ ERROR: expected item, found `-`\n// FIXME(frontmatter): make this diagnostic better\n---\n\n// test that we do not parse another frontmatter block after the first one.\n\nfn main() {}\n"

"#]],
        );
    }

    #[test]
    fn rustc_shebang() {
        assert_source(
            r#"#!/usr/bin/env -S cargo -Zscript
---
[dependencies]
clap = "4"
---

//@ check-pass

// Shebangs on a file can precede a frontmatter.

fn main () {}
"#,
            str![[r##"
shebang: "#!/usr/bin/env -S cargo -Zscript\n"
info: None
frontmatter: "[dependencies]\nclap = \"4\"\n"
content: "\n//@ check-pass\n\n// Shebangs on a file can precede a frontmatter.\n\nfn main () {}\n"

"##]],
        );
    }

    #[test]
    fn rustc_unclosed_1() {
        assert_err(
            ScriptSource::parse(
                r#"----cargo
//~^ ERROR: unclosed frontmatter

// This test checks that the #! characters can help us recover a frontmatter
// close. There should not be a "missing `main` function" error as the rest
// are properly parsed.

fn main() {}
"#,
            ),
            str!["no closing `----` found for frontmatter"],
        );
    }

    #[test]
    fn rustc_unclosed_2() {
        assert_err(
            ScriptSource::parse(
                r#"----cargo
//~^ ERROR: unclosed frontmatter
//~| ERROR: frontmatters are experimental

//@ compile-flags: --crate-type lib

// Leading whitespace on the feature line prevents recovery. However
// the dashes quoted will not be used for recovery and the entire file
// should be treated as within the frontmatter block.

fn foo() -> &str {
    "----"
}
"#,
            ),
            str!["no closing `----` found for frontmatter"],
        );
    }

    #[test]
    fn rustc_unclosed_3() {
        assert_err(
            ScriptSource::parse(
                r#"----cargo
//~^ ERROR: frontmatter close does not match the opening

//@ compile-flags: --crate-type lib

// Unfortunate recovery situation. Not really preventable with improving the
// recovery strategy, but this type of code is rare enough already.

fn foo(x: i32) -> i32 {
    ---x
    //~^ ERROR: invalid preceding whitespace for frontmatter close
    //~| ERROR: extra characters after frontmatter close are not allowed
}
//~^ ERROR: unexpected closing delimiter: `}`
"#,
            ),
            str!["no closing `----` found for frontmatter"],
        );
    }

    #[test]
    fn rustc_unclosed_4() {
        assert_err(
            ScriptSource::parse(
                r#"
----cargo
//~^ ERROR: unclosed frontmatter

//! Similarly, a module-level content should allow for recovery as well (as
//! per unclosed-1.rs)

fn main() {}
"#,
            ),
            str!["no closing `----` found for frontmatter"],
        );
    }

    #[test]
    fn rustc_unclosed_5() {
        assert_err(
            ScriptSource::parse(
                r#"----cargo
//~^ ERROR: unclosed frontmatter
//~| ERROR: frontmatters are experimental

// Similarly, a use statement should allow for recovery as well (as
// per unclosed-1.rs)

use std::env;

fn main() {}
"#,
            ),
            str!["no closing `----` found for frontmatter"],
        );
    }

    #[test]
    fn split_default() {
        assert_source(
            r#"fn main() {}
"#,
            str![[r#"
shebang: None
info: None
frontmatter: None
content: "fn main() {}\n"

"#]],
        );
    }

    #[test]
    fn split_dependencies() {
        assert_source(
            r#"---
[dependencies]
time="0.1.25"
---
fn main() {}
"#,
            str![[r#"
shebang: None
info: None
frontmatter: "[dependencies]\ntime=\"0.1.25\"\n"
content: "fn main() {}\n"

"#]],
        );
    }

    #[test]
    fn split_infostring() {
        assert_source(
            r#"---cargo
[dependencies]
time="0.1.25"
---
fn main() {}
"#,
            str![[r#"
shebang: None
info: "cargo"
frontmatter: "[dependencies]\ntime=\"0.1.25\"\n"
content: "fn main() {}\n"

"#]],
        );
    }

    #[test]
    fn split_infostring_whitespace() {
        assert_source(
            r#"--- cargo 
[dependencies]
time="0.1.25"
---
fn main() {}
"#,
            str![[r#"
shebang: None
info: "cargo"
frontmatter: "[dependencies]\ntime=\"0.1.25\"\n"
content: "fn main() {}\n"

"#]],
        );
    }

    #[test]
    fn split_shebang() {
        assert_source(
            r#"#!/usr/bin/env cargo
---
[dependencies]
time="0.1.25"
---
fn main() {}
"#,
            str![[r##"
shebang: "#!/usr/bin/env cargo\n"
info: None
frontmatter: "[dependencies]\ntime=\"0.1.25\"\n"
content: "fn main() {}\n"

"##]],
        );
    }

    #[test]
    fn split_crlf() {
        assert_source(
                "#!/usr/bin/env cargo\r\n---\r\n[dependencies]\r\ntime=\"0.1.25\"\r\n---\r\nfn main() {}",
            str![[r##"
shebang: "#!/usr/bin/env cargo\r\n"
info: None
frontmatter: "[dependencies]\r\ntime=\"0.1.25\"\r\n"
content: "fn main() {}"

"##]]
        );
    }

    #[test]
    fn split_leading_newlines() {
        assert_source(
            r#"#!/usr/bin/env cargo
    


---
[dependencies]
time="0.1.25"
---


fn main() {}
"#,
            str![[r##"
shebang: "#!/usr/bin/env cargo\n"
info: None
frontmatter: "[dependencies]\ntime=\"0.1.25\"\n"
content: "\n\nfn main() {}\n"

"##]],
        );
    }

    #[test]
    fn split_attribute() {
        assert_source(
            r#"#[allow(dead_code)]
---
[dependencies]
time="0.1.25"
---
fn main() {}
"#,
            str![[r##"
shebang: None
info: None
frontmatter: None
content: "#[allow(dead_code)]\n---\n[dependencies]\ntime=\"0.1.25\"\n---\nfn main() {}\n"

"##]],
        );
    }

    #[test]
    fn split_extra_dash() {
        assert_source(
            r#"#!/usr/bin/env cargo
----------
[dependencies]
time="0.1.25"
----------

fn main() {}"#,
            str![[r##"
shebang: "#!/usr/bin/env cargo\n"
info: None
frontmatter: "[dependencies]\ntime=\"0.1.25\"\n"
content: "\nfn main() {}"

"##]],
        );
    }

    #[test]
    fn split_too_few_dashes() {
        assert_err(
            ScriptSource::parse(
                r#"#!/usr/bin/env cargo
--
[dependencies]
time="0.1.25"
--
fn main() {}
"#,
            ),
            str!["found 2 `-` in rust frontmatter, expected at least 3"],
        );
    }

    #[test]
    fn split_indent() {
        assert_source(
            r#"#!/usr/bin/env cargo
    ---
    [dependencies]
    time="0.1.25"
    ----

fn main() {}
"#,
            str![[r##"
shebang: "#!/usr/bin/env cargo\n"
info: None
frontmatter: None
content: "    ---\n    [dependencies]\n    time=\"0.1.25\"\n    ----\n\nfn main() {}\n"

"##]],
        );
    }

    #[test]
    fn split_escaped() {
        assert_source(
            r#"#!/usr/bin/env cargo
-----
---
---
-----

fn main() {}
"#,
            str![[r##"
shebang: "#!/usr/bin/env cargo\n"
info: None
frontmatter: "---\n---\n"
content: "\nfn main() {}\n"

"##]],
        );
    }

    #[test]
    fn split_invalid_escaped() {
        assert_err(
            ScriptSource::parse(
                r#"#!/usr/bin/env cargo
---
-----
-----
---

fn main() {}
"#,
            ),
            str!["trailing characters found after frontmatter close"],
        );
    }

    #[test]
    fn split_dashes_in_body() {
        assert_source(
            r#"#!/usr/bin/env cargo
---
Hello---
World
---

fn main() {}
"#,
            str![[r##"
shebang: "#!/usr/bin/env cargo\n"
info: None
frontmatter: "Hello---\nWorld\n"
content: "\nfn main() {}\n"

"##]],
        );
    }

    #[test]
    fn split_mismatched_dashes() {
        assert_err(
            ScriptSource::parse(
                r#"#!/usr/bin/env cargo
---
[dependencies]
time="0.1.25"
----
fn main() {}
"#,
            ),
            str!["trailing characters found after frontmatter close"],
        );
    }

    #[test]
    fn split_missing_close() {
        assert_err(
            ScriptSource::parse(
                r#"#!/usr/bin/env cargo
---
[dependencies]
time="0.1.25"
fn main() {}
"#,
            ),
            str!["no closing `---` found for frontmatter"],
        );
    }

    #[track_caller]
    fn expand(source: &str) -> String {
        expand_manifest(source).unwrap_or_else(|err| panic!("{}", err))
    }

    #[test]
    fn expand_default() {
        assert_data_eq!(expand(r#"fn main() {}"#), str![""]);
    }

    #[test]
    fn expand_dependencies() {
        assert_data_eq!(
            expand(
                r#"---cargo
[dependencies]
time="0.1.25"
---
fn main() {}
"#
            ),
            str![[r#"
[dependencies]
time="0.1.25"

"#]]
        );
    }
}
