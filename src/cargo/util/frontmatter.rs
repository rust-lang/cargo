type Span = std::ops::Range<usize>;

#[derive(Debug)]
pub struct ScriptSource<'s> {
    /// The full file
    raw: &'s str,
    /// The `#!/usr/bin/env cargo` line, if present
    shebang: Option<Span>,
    /// The code fence opener (`---`)
    open: Option<Span>,
    /// Trailing text after `ScriptSource::open` that identifies the meaning of
    /// `ScriptSource::frontmatter`
    info: Option<Span>,
    /// The lines between `ScriptSource::open` and `ScriptSource::close`
    frontmatter: Option<Span>,
    /// The code fence closer (`---`)
    close: Option<Span>,
    /// All content after the frontmatter and shebang
    content: Span,
}

impl<'s> ScriptSource<'s> {
    pub fn parse(raw: &'s str) -> Result<Self, FrontmatterError> {
        use winnow::stream::FindSlice as _;
        use winnow::stream::Location as _;
        use winnow::stream::Offset as _;
        use winnow::stream::Stream as _;

        let content_end = raw.len();
        let mut source = Self {
            raw,
            shebang: None,
            open: None,
            info: None,
            frontmatter: None,
            close: None,
            content: 0..content_end,
        };

        let mut input = winnow::stream::LocatingSlice::new(raw);

        if let Some(shebang_end) = strip_shebang(input.as_ref()) {
            let shebang_start = input.current_token_start();
            let _ = input.next_slice(shebang_end);
            let shebang_end = input.current_token_start();
            source.shebang = Some(shebang_start..shebang_end);
            source.content = shebang_end..content_end;
        }

        // Whitespace may precede a frontmatter but must end with a newline
        if let Some(nl_end) = strip_ws_lines(input.as_ref()) {
            let _ = input.next_slice(nl_end);
        }

        // Opens with a line that starts with 3 or more `-` followed by an optional identifier
        const FENCE_CHAR: char = '-';
        let fence_length = input
            .as_ref()
            .char_indices()
            .find_map(|(i, c)| (c != FENCE_CHAR).then_some(i))
            .unwrap_or_else(|| input.eof_offset());
        let open_start = input.current_token_start();
        let fence_pattern = input.next_slice(fence_length);
        let open_end = input.current_token_start();
        match fence_length {
            0 => {
                return Ok(source);
            }
            1 | 2 => {
                // either not a frontmatter or invalid frontmatter opening
                return Err(FrontmatterError::new(
                    format!(
                        "found {fence_length} `{FENCE_CHAR}` in rust frontmatter, expected at least 3"
                    ),
                    raw.len()..raw.len(),
                ).push_visible_span(open_start..open_end));
            }
            _ => {}
        }
        source.open = Some(open_start..open_end);
        let Some(info_nl) = input.find_slice("\n") else {
            return Err(FrontmatterError::new(
                format!("unclosed frontmatter; expected `{fence_pattern}`"),
                raw.len()..raw.len(),
            )
            .push_visible_span(open_start..open_end));
        };
        let info = input.next_slice(info_nl.start);
        let info = info.strip_suffix('\r').unwrap_or(info); // already excludes `\n`
        let info = info.trim_matches(is_horizontal_whitespace);
        if !info.is_empty() {
            let info_start = info.offset_from(&raw);
            let info_end = info_start + info.len();
            source.info = Some(info_start..info_end);
        }

        // Ends with a line that starts with a matching number of `-` only followed by whitespace
        let nl_fence_pattern = format!("\n{fence_pattern}");
        let Some(frontmatter_nl) = input.find_slice(nl_fence_pattern.as_str()) else {
            for len in (2..(nl_fence_pattern.len() - 1)).rev() {
                let Some(frontmatter_nl) = input.find_slice(&nl_fence_pattern[0..len]) else {
                    continue;
                };
                let _ = input.next_slice(frontmatter_nl.start + 1);
                let close_start = input.current_token_start();
                let _ = input.next_slice(len);
                let close_end = input.current_token_start();
                let fewer_dashes = fence_length - len;
                return Err(FrontmatterError::new(
                    format!(
                        "closing code fence has {fewer_dashes} less `-` than the opening fence"
                    ),
                    close_start..close_end,
                )
                .push_visible_span(open_start..open_end));
            }
            return Err(FrontmatterError::new(
                format!("unclosed frontmatter; expected `{fence_pattern}`"),
                raw.len()..raw.len(),
            )
            .push_visible_span(open_start..open_end));
        };
        let frontmatter_start = input.current_token_start() + 1; // skip nl from infostring
        let _ = input.next_slice(frontmatter_nl.start + 1);
        let frontmatter_end = input.current_token_start();
        source.frontmatter = Some(frontmatter_start..frontmatter_end);
        let close_start = input.current_token_start();
        let _ = input.next_slice(fence_length);
        let close_end = input.current_token_start();
        source.close = Some(close_start..close_end);

        let nl = input.find_slice("\n");
        let after_closing_fence = input.next_slice(
            nl.map(|span| span.end)
                .unwrap_or_else(|| input.eof_offset()),
        );
        let content_start = input.current_token_start();
        let extra_dashes = after_closing_fence
            .chars()
            .take_while(|b| *b == FENCE_CHAR)
            .count();
        if 0 < extra_dashes {
            let extra_start = close_end;
            let extra_end = extra_start + extra_dashes;
            return Err(FrontmatterError::new(
                format!("closing code fence has {extra_dashes} more `-` than the opening fence"),
                extra_start..extra_end,
            )
            .push_visible_span(open_start..open_end));
        } else {
            let after_closing_fence = strip_newline(after_closing_fence);
            let after_closing_fence = after_closing_fence.trim_matches(is_horizontal_whitespace);
            if !after_closing_fence.is_empty() {
                // extra characters beyond the original fence pattern
                let after_start = after_closing_fence.offset_from(&raw);
                let after_end = after_start + after_closing_fence.len();
                return Err(FrontmatterError::new(
                    format!("unexpected characters after frontmatter close"),
                    after_start..after_end,
                )
                .push_visible_span(open_start..open_end));
            }
        }

        source.content = content_start..content_end;

        if let Some(nl_end) = strip_ws_lines(input.as_ref()) {
            let _ = input.next_slice(nl_end);
        }
        let fence_length = input
            .as_ref()
            .char_indices()
            .find_map(|(i, c)| (c != FENCE_CHAR).then_some(i))
            .unwrap_or_else(|| input.eof_offset());
        if 0 < fence_length {
            let fence_start = input.current_token_start();
            let fence_end = fence_start + fence_length;
            return Err(FrontmatterError::new(
                format!("only one frontmatter is supported"),
                fence_start..fence_end,
            )
            .push_visible_span(open_start..open_end)
            .push_visible_span(close_start..close_end));
        }

        Ok(source)
    }

    pub fn shebang(&self) -> Option<&'s str> {
        self.shebang.clone().map(|span| &self.raw[span])
    }

    pub fn shebang_span(&self) -> Option<Span> {
        self.shebang.clone()
    }

    pub fn open_span(&self) -> Option<Span> {
        self.open.clone()
    }

    pub fn info(&self) -> Option<&'s str> {
        self.info.clone().map(|span| &self.raw[span])
    }

    pub fn info_span(&self) -> Option<Span> {
        self.info.clone()
    }

    pub fn frontmatter(&self) -> Option<&'s str> {
        self.frontmatter.clone().map(|span| &self.raw[span])
    }

    pub fn frontmatter_span(&self) -> Option<Span> {
        self.frontmatter.clone()
    }

    pub fn close_span(&self) -> Option<Span> {
        self.close.clone()
    }

    pub fn content(&self) -> &'s str {
        &self.raw[self.content.clone()]
    }

    pub fn content_span(&self) -> Span {
        self.content.clone()
    }
}

/// Returns the index after the shebang line, if present
pub fn strip_shebang(input: &str) -> Option<usize> {
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

/// Returns the index after any lines with only whitespace, if present
pub fn strip_ws_lines(input: &str) -> Option<usize> {
    let ws_end = input.find(|c| !is_whitespace(c)).unwrap_or(input.len());
    if ws_end == 0 {
        return None;
    }

    let nl_start = input[0..ws_end].rfind('\n')?;
    let nl_end = nl_start + 1;
    Some(nl_end)
}

/// True if `c` is considered a whitespace according to Rust language definition.
/// See [Rust language reference](https://doc.rust-lang.org/reference/whitespace.html)
/// for definitions of these classes.
fn is_whitespace(c: char) -> bool {
    // This is Pattern_White_Space.
    //
    // Note that this set is stable (ie, it doesn't change with different
    // Unicode versions), so it's ok to just hard-code the values.

    matches!(
        c,
        // End-of-line characters
        | '\u{000A}' // line feed (\n)
        | '\u{000B}' // vertical tab
        | '\u{000C}' // form feed
        | '\u{000D}' // carriage return (\r)
        | '\u{0085}' // next line (from latin1)
        | '\u{2028}' // LINE SEPARATOR
        | '\u{2029}' // PARAGRAPH SEPARATOR

        // `Default_Ignorable_Code_Point` characters
        | '\u{200E}' // LEFT-TO-RIGHT MARK
        | '\u{200F}' // RIGHT-TO-LEFT MARK

        // Horizontal space characters
        | '\u{0009}'   // tab (\t)
        | '\u{0020}' // space
    )
}

/// True if `c` is considered horizontal whitespace according to Rust language definition.
fn is_horizontal_whitespace(c: char) -> bool {
    // This is Pattern_White_Space.
    //
    // Note that this set is stable (ie, it doesn't change with different
    // Unicode versions), so it's ok to just hard-code the values.

    matches!(
        c,
        // Horizontal space characters
        '\u{0009}'   // tab (\t)
        | '\u{0020}' // space
    )
}

fn strip_newline(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

#[derive(Debug)]
pub struct FrontmatterError {
    message: String,
    primary_span: Span,
    visible_spans: Vec<Span>,
}

impl FrontmatterError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            primary_span: span,
            visible_spans: Vec::new(),
        }
    }

    pub fn push_visible_span(mut self, span: Span) -> Self {
        self.visible_spans.push(span);
        self
    }

    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    pub fn primary_span(&self) -> Span {
        self.primary_span.clone()
    }

    pub fn visible_spans(&self) -> &[Span] {
        &self.visible_spans
    }
}

impl std::fmt::Display for FrontmatterError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(fmt)
    }
}

impl std::error::Error for FrontmatterError {}

#[cfg(test)]
mod test {
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

"##]],
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
            str!["closing code fence has 2 more `-` than the opening fence"],
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
            str!["closing code fence has 1 more `-` than the opening fence"],
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
            str!["unclosed frontmatter; expected `---`"],
        );
    }
}
