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

        const FENCE_CHAR: char = '-';

        let mut rest = source.content;
        while !rest.is_empty() {
            let without_spaces = rest.trim_start_matches([' ', '\t']);
            let without_nl = without_spaces.trim_start_matches(['\r', '\n']);
            if without_nl == rest {
                // nothing trimmed
                break;
            } else if without_nl == without_spaces {
                // frontmatter must come after a newline
                return Ok(source);
            }
            rest = without_nl;
        }
        let fence_end = rest
            .char_indices()
            .find_map(|(i, c)| (c != FENCE_CHAR).then_some(i))
            .unwrap_or(source.content.len());
        let (fence_pattern, rest) = match fence_end {
            0 => {
                return Ok(source);
            }
            1 | 2 => {
                anyhow::bail!(
                    "found {fence_end} `{FENCE_CHAR}` in rust frontmatter, expected at least 3"
                )
            }
            _ => rest.split_at(fence_end),
        };
        let nl_fence_pattern = format!("\n{fence_pattern}");
        let (info, content) = rest.split_once("\n").unwrap_or((rest, ""));
        let info = info.trim();
        if !info.is_empty() {
            source.info = Some(info);
        }
        source.content = content;

        let Some(frontmatter_nl) = source.content.find(&nl_fence_pattern) else {
            anyhow::bail!("no closing `{fence_pattern}` found for frontmatter");
        };
        source.frontmatter = Some(&source.content[..frontmatter_nl + 1]);
        source.content = &source.content[frontmatter_nl + nl_fence_pattern.len()..];

        let (line, content) = source
            .content
            .split_once("\n")
            .unwrap_or((source.content, ""));
        let line = line.trim();
        if !line.is_empty() {
            anyhow::bail!("unexpected trailing content on closing fence: `{line}`");
        }
        source.content = content;

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
            str!["unexpected trailing content on closing fence: `--`"],
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
            str!["unexpected trailing content on closing fence: `-`"],
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
