use anyhow::Context as _;

use crate::util::restricted_names;
use crate::CargoResult;
use crate::Config;

const DEFAULT_EDITION: crate::core::features::Edition =
    crate::core::features::Edition::LATEST_STABLE;
const DEFAULT_VERSION: &str = "0.0.0";
const DEFAULT_PUBLISH: bool = false;

pub fn expand_manifest(
    content: &str,
    path: &std::path::Path,
    config: &Config,
) -> CargoResult<String> {
    let comment = match extract_comment(content) {
        Ok(comment) => Some(comment),
        Err(err) => {
            log::trace!("failed to extract doc comment: {err}");
            None
        }
    }
    .unwrap_or_default();
    let manifest = match extract_manifest(&comment)? {
        Some(manifest) => Some(manifest),
        None => {
            log::trace!("failed to extract manifest");
            None
        }
    }
    .unwrap_or_default();
    let manifest = expand_manifest_(&manifest, path, config)
        .with_context(|| format!("failed to parse manifest at {}", path.display()))?;
    let manifest = toml::to_string_pretty(&manifest)?;
    Ok(manifest)
}

fn expand_manifest_(
    manifest: &str,
    path: &std::path::Path,
    config: &Config,
) -> CargoResult<toml::Table> {
    let mut manifest: toml::Table = toml::from_str(&manifest)?;

    for key in ["workspace", "lib", "bin", "example", "test", "bench"] {
        if manifest.contains_key(key) {
            anyhow::bail!("`{key}` is not allowed in embedded manifests")
        }
    }

    // Prevent looking for a workspace by `read_manifest_from_str`
    manifest.insert("workspace".to_owned(), toml::Table::new().into());

    let package = manifest
        .entry("package".to_owned())
        .or_insert_with(|| toml::Table::new().into())
        .as_table_mut()
        .ok_or_else(|| anyhow::format_err!("`package` must be a table"))?;
    for key in ["workspace", "build", "links"] {
        if package.contains_key(key) {
            anyhow::bail!("`package.{key}` is not allowed in embedded manifests")
        }
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::format_err!("no file name"))?
        .to_string_lossy();
    let file_stem = path
        .file_stem()
        .ok_or_else(|| anyhow::format_err!("no file name"))?
        .to_string_lossy();
    let name = sanitize_name(file_stem.as_ref());
    let bin_name = name.clone();
    package
        .entry("name".to_owned())
        .or_insert(toml::Value::String(name));
    package
        .entry("version".to_owned())
        .or_insert_with(|| toml::Value::String(DEFAULT_VERSION.to_owned()));
    package.entry("edition".to_owned()).or_insert_with(|| {
        let _ = config.shell().warn(format_args!(
            "`package.edition` is unspecifiead, defaulting to `{}`",
            DEFAULT_EDITION
        ));
        toml::Value::String(DEFAULT_EDITION.to_string())
    });
    package
        .entry("publish".to_owned())
        .or_insert_with(|| toml::Value::Boolean(DEFAULT_PUBLISH));

    let mut bin = toml::Table::new();
    bin.insert("name".to_owned(), toml::Value::String(bin_name));
    bin.insert(
        "path".to_owned(),
        toml::Value::String(file_name.into_owned()),
    );
    manifest.insert(
        "bin".to_owned(),
        toml::Value::Array(vec![toml::Value::Table(bin)]),
    );

    let release = manifest
        .entry("profile".to_owned())
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or_else(|| anyhow::format_err!("`profile` must be a table"))?
        .entry("release".to_owned())
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or_else(|| anyhow::format_err!("`profile.release` must be a table"))?;
    release
        .entry("strip".to_owned())
        .or_insert_with(|| toml::Value::Boolean(true));

    Ok(manifest)
}

/// Ensure the package name matches the validation from `ops::cargo_new::check_name`
fn sanitize_name(name: &str) -> String {
    let placeholder = if name.contains('_') {
        '_'
    } else {
        // Since embedded manifests only support `[[bin]]`s, prefer arrow-case as that is the
        // more common convention for CLIs
        '-'
    };

    let mut name = restricted_names::sanitize_package_name(name, placeholder);

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

/// Locates a "code block manifest" in Rust source.
fn extract_comment(input: &str) -> CargoResult<String> {
    let re_crate_comment = regex::Regex::new(
        // We need to find the first `/*!` or `//!` that *isn't* preceded by something that would
        // make it apply to anything other than the crate itself.  Because we can't do this
        // accurately, we'll just require that the doc-comment is the *first* thing in the file
        // (after the optional shebang).
        r"(?x)(^\s*|^\#![^\[].*?(\r\n|\n))(/\*!|//(!|/))",
    )
    .unwrap();
    let re_margin = regex::Regex::new(r"^\s*\*( |$)").unwrap();
    let re_space = regex::Regex::new(r"^(\s+)").unwrap();
    let re_nesting = regex::Regex::new(r"/\*|\*/").unwrap();
    let re_comment = regex::Regex::new(r"^\s*//(!|/)").unwrap();

    fn n_leading_spaces(s: &str, n: usize) -> anyhow::Result<()> {
        if !s.chars().take(n).all(|c| c == ' ') {
            anyhow::bail!("leading {n:?} chars aren't all spaces: {s:?}")
        }
        Ok(())
    }

    /// Returns a slice of the input string with the leading shebang, if there is one, omitted.
    fn strip_shebang(s: &str) -> &str {
        let re_shebang = regex::Regex::new(r"^#![^\[].*?(\r\n|\n)").unwrap();
        re_shebang.find(s).map(|m| &s[m.end()..]).unwrap_or(s)
    }

    // First, we will look for and slice out a contiguous, inner doc-comment which must be *the
    // very first thing* in the file.  `#[doc(...)]` attributes *are not supported*.  Multiple
    // single-line comments cannot have any blank lines between them.
    let input = strip_shebang(input); // `re_crate_comment` doesn't work with shebangs
    let start = re_crate_comment
        .captures(input)
        .ok_or_else(|| anyhow::format_err!("no doc-comment found"))?
        .get(3)
        .ok_or_else(|| anyhow::format_err!("no doc-comment found"))?
        .start();

    let input = &input[start..];

    if let Some(input) = input.strip_prefix("/*!") {
        // On every line:
        //
        // - update nesting level and detect end-of-comment
        // - if margin is None:
        //     - if there appears to be a margin, set margin.
        // - strip off margin marker
        // - update the leading space counter
        // - strip leading space
        // - append content
        let mut r = String::new();

        let mut leading_space = None;
        let mut margin = None;
        let mut depth: u32 = 1;

        for line in input.lines() {
            if depth == 0 {
                break;
            }

            // Update nesting and look for end-of-comment.
            let mut end_of_comment = None;

            for (end, marker) in re_nesting.find_iter(line).map(|m| (m.start(), m.as_str())) {
                match (marker, depth) {
                    ("/*", _) => depth += 1,
                    ("*/", 1) => {
                        end_of_comment = Some(end);
                        depth = 0;
                        break;
                    }
                    ("*/", _) => depth -= 1,
                    _ => panic!("got a comment marker other than /* or */"),
                }
            }

            let line = end_of_comment.map(|end| &line[..end]).unwrap_or(line);

            // Detect and strip margin.
            margin = margin.or_else(|| re_margin.find(line).map(|m| m.as_str()));

            let line = if let Some(margin) = margin {
                let end = line
                    .char_indices()
                    .take(margin.len())
                    .map(|(i, c)| i + c.len_utf8())
                    .last()
                    .unwrap_or(0);
                &line[end..]
            } else {
                line
            };

            // Detect and strip leading indentation.
            leading_space = leading_space.or_else(|| re_space.find(line).map(|m| m.end()));

            // Make sure we have only leading spaces.
            //
            // If we see a tab, fall over.  I *would* expand them, but that gets into the question of how *many* spaces to expand them to, and *where* is the tab, because tabs are tab stops and not just N spaces.
            n_leading_spaces(line, leading_space.unwrap_or(0))?;

            let strip_len = line.len().min(leading_space.unwrap_or(0));
            let line = &line[strip_len..];

            // Done.
            r.push_str(line);

            // `lines` removes newlines.  Ideally, it wouldn't do that, but hopefully this shouldn't cause any *real* problems.
            r.push('\n');
        }

        Ok(r)
    } else if input.starts_with("//!") || input.starts_with("///") {
        let mut r = String::new();

        let mut leading_space = None;

        for line in input.lines() {
            // Strip leading comment marker.
            let content = match re_comment.find(line) {
                Some(m) => &line[m.end()..],
                None => break,
            };

            // Detect and strip leading indentation.
            leading_space = leading_space.or_else(|| {
                re_space
                    .captures(content)
                    .and_then(|c| c.get(1))
                    .map(|m| m.end())
            });

            // Make sure we have only leading spaces.
            //
            // If we see a tab, fall over.  I *would* expand them, but that gets into the question of how *many* spaces to expand them to, and *where* is the tab, because tabs are tab stops and not just N spaces.
            n_leading_spaces(content, leading_space.unwrap_or(0))?;

            let strip_len = content.len().min(leading_space.unwrap_or(0));
            let content = &content[strip_len..];

            // Done.
            r.push_str(content);

            // `lines` removes newlines.  Ideally, it wouldn't do that, but hopefully this shouldn't cause any *real* problems.
            r.push('\n');
        }

        Ok(r)
    } else {
        Err(anyhow::format_err!("no doc-comment found"))
    }
}

/// Extracts the first `Cargo` fenced code block from a chunk of Markdown.
fn extract_manifest(comment: &str) -> CargoResult<Option<String>> {
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};

    // To match librustdoc/html/markdown.rs, opts.
    let exts = Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES;

    let md = Parser::new_ext(comment, exts);

    let mut inside = false;
    let mut output = None;

    for item in md {
        match item {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref info)))
                if info.to_lowercase() == "cargo" =>
            {
                if output.is_some() {
                    anyhow::bail!("multiple `cargo` manifests present")
                } else {
                    output = Some(String::new());
                }
                inside = true;
            }
            Event::Text(ref text) if inside => {
                let s = output.get_or_insert(String::new());
                s.push_str(text);
            }
            Event::End(Tag::CodeBlock(_)) if inside => {
                inside = false;
            }
            _ => (),
        }
    }

    Ok(output)
}

#[cfg(test)]
mod test_expand {
    use super::*;

    macro_rules! si {
        ($i:expr) => {{
            expand_manifest(
                $i,
                std::path::Path::new("/home/me/test.rs"),
                &Config::default().unwrap(),
            )
            .unwrap_or_else(|err| panic!("{}", err))
        }};
    }

    #[test]
    fn test_default() {
        snapbox::assert_eq(
            r#"[[bin]]
name = "test-"
path = "test.rs"

[package]
edition = "2021"
name = "test-"
publish = false
version = "0.0.0"

[profile.release]
strip = true

[workspace]
"#,
            si!(r#"fn main() {}"#),
        );
    }

    #[test]
    fn test_dependencies() {
        snapbox::assert_eq(
            r#"[[bin]]
name = "test-"
path = "test.rs"

[dependencies]
time = "0.1.25"

[package]
edition = "2021"
name = "test-"
publish = false
version = "0.0.0"

[profile.release]
strip = true

[workspace]
"#,
            si!(r#"
//! ```cargo
//! [dependencies]
//! time="0.1.25"
//! ```
fn main() {}
"#),
        );
    }
}

#[cfg(test)]
mod test_comment {
    use super::*;

    macro_rules! ec {
        ($s:expr) => {
            extract_comment($s).unwrap_or_else(|err| panic!("{}", err))
        };
    }

    #[test]
    fn test_no_comment() {
        snapbox::assert_eq(
            "no doc-comment found",
            extract_comment(
                r#"
fn main () {
}
"#,
            )
            .unwrap_err()
            .to_string(),
        );
    }

    #[test]
    fn test_no_comment_she_bang() {
        snapbox::assert_eq(
            "no doc-comment found",
            extract_comment(
                r#"#!/usr/bin/env cargo-eval

fn main () {
}
"#,
            )
            .unwrap_err()
            .to_string(),
        );
    }

    #[test]
    fn test_comment() {
        snapbox::assert_eq(
            r#"Here is a manifest:

```cargo
[dependencies]
time = "*"
```
"#,
            ec!(r#"//! Here is a manifest:
//!
//! ```cargo
//! [dependencies]
//! time = "*"
//! ```
fn main() {}
"#),
        );
    }

    #[test]
    fn test_comment_shebang() {
        snapbox::assert_eq(
            r#"Here is a manifest:

```cargo
[dependencies]
time = "*"
```
"#,
            ec!(r#"#!/usr/bin/env cargo-eval

//! Here is a manifest:
//!
//! ```cargo
//! [dependencies]
//! time = "*"
//! ```
fn main() {}
"#),
        );
    }

    #[test]
    fn test_multiline_comment() {
        snapbox::assert_eq(
            r#"
Here is a manifest:

```cargo
[dependencies]
time = "*"
```

"#,
            ec!(r#"/*!
Here is a manifest:

```cargo
[dependencies]
time = "*"
```
*/

fn main() {
}
"#),
        );
    }

    #[test]
    fn test_multiline_comment_shebang() {
        snapbox::assert_eq(
            r#"
Here is a manifest:

```cargo
[dependencies]
time = "*"
```

"#,
            ec!(r#"#!/usr/bin/env cargo-eval

/*!
Here is a manifest:

```cargo
[dependencies]
time = "*"
```
*/

fn main() {
}
"#),
        );
    }

    #[test]
    fn test_multiline_block_comment() {
        snapbox::assert_eq(
            r#"
Here is a manifest:

```cargo
[dependencies]
time = "*"
```

"#,
            ec!(r#"/*!
 * Here is a manifest:
 *
 * ```cargo
 * [dependencies]
 * time = "*"
 * ```
 */
fn main() {}
"#),
        );
    }

    #[test]
    fn test_multiline_block_comment_shebang() {
        snapbox::assert_eq(
            r#"
Here is a manifest:

```cargo
[dependencies]
time = "*"
```

"#,
            ec!(r#"#!/usr/bin/env cargo-eval

/*!
 * Here is a manifest:
 *
 * ```cargo
 * [dependencies]
 * time = "*"
 * ```
 */
fn main() {}
"#),
        );
    }
}

#[cfg(test)]
mod test_manifest {
    use super::*;

    macro_rules! smm {
        ($c:expr) => {
            extract_manifest($c)
        };
    }

    #[test]
    fn test_no_code_fence() {
        assert_eq!(
            smm!(
                r#"There is no manifest in this comment.
"#
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn test_no_cargo_code_fence() {
        assert_eq!(
            smm!(
                r#"There is no manifest in this comment.

```
This is not a manifest.
```

```rust
println!("Nor is this.");
```

    Or this.
"#
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn test_cargo_code_fence() {
        assert_eq!(
            smm!(
                r#"This is a manifest:

```cargo
dependencies = { time = "*" }
```
"#
            )
            .unwrap(),
            Some(
                r#"dependencies = { time = "*" }
"#
                .into()
            )
        );
    }

    #[test]
    fn test_mixed_code_fence() {
        assert_eq!(
            smm!(
                r#"This is *not* a manifest:

```
He's lying, I'm *totally* a manifest!
```

This *is*:

```cargo
dependencies = { time = "*" }
```
"#
            )
            .unwrap(),
            Some(
                r#"dependencies = { time = "*" }
"#
                .into()
            )
        );
    }

    #[test]
    fn test_two_cargo_code_fence() {
        assert!(smm!(
            r#"This is a manifest:

```cargo
dependencies = { time = "*" }
```

So is this, but it doesn't count:

```cargo
dependencies = { explode = true }
```
"#
        )
        .is_err());
    }
}
