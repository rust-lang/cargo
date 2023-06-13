use anyhow::Context as _;

use crate::core::Workspace;
use crate::CargoResult;
use crate::Config;

const DEFAULT_EDITION: crate::core::features::Edition =
    crate::core::features::Edition::LATEST_STABLE;
const DEFAULT_VERSION: &str = "0.0.0";
const DEFAULT_PUBLISH: bool = false;

pub struct RawScript {
    manifest: String,
    body: String,
    path: std::path::PathBuf,
}

pub fn parse_from(path: &std::path::Path) -> CargoResult<RawScript> {
    let body = std::fs::read_to_string(path)
        .with_context(|| format!("failed to script at {}", path.display()))?;
    parse(&body, path)
}

pub fn parse(body: &str, path: &std::path::Path) -> CargoResult<RawScript> {
    let comment = match extract_comment(body) {
        Ok(manifest) => Some(manifest),
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
    let body = body.to_owned();
    let path = path.to_owned();
    Ok(RawScript {
        manifest,
        body,
        path,
    })
}

pub fn to_workspace<'cfg>(
    script: &RawScript,
    config: &'cfg Config,
) -> CargoResult<Workspace<'cfg>> {
    let target_dir = config
        .target_dir()
        .transpose()
        .unwrap_or_else(|| default_target_dir().map(crate::util::Filesystem::new))?;
    // HACK: without cargo knowing about embedded manifests, the only way to create a
    // `Workspace` is either
    // - Create a temporary one on disk
    // - Create an "ephemeral" workspace **but** compilation re-loads ephemeral workspaces
    //   from the registry rather than what we already have on memory, causing it to fail
    //   because the registry doesn't know about embedded manifests.
    let manifest_path = write(script, config, target_dir.as_path_unlocked())?;
    let workspace = Workspace::new(&manifest_path, config)?;
    Ok(workspace)
}

fn write(
    script: &RawScript,
    config: &Config,
    target_dir: &std::path::Path,
) -> CargoResult<std::path::PathBuf> {
    let hash = hash(script).to_string();
    assert_eq!(hash.len(), 64);
    let mut workspace_root = target_dir.to_owned();
    workspace_root.push("eval");
    workspace_root.push(&hash[0..2]);
    workspace_root.push(&hash[2..4]);
    workspace_root.push(&hash[4..]);
    workspace_root.push(package_name(script)?);
    std::fs::create_dir_all(&workspace_root).with_context(|| {
        format!(
            "failed to create temporary workspace at {}",
            workspace_root.display()
        )
    })?;
    let manifest_path = workspace_root.join("Cargo.toml");
    let manifest = expand_manifest(script, config)?;
    write_if_changed(&manifest_path, &manifest)?;
    Ok(manifest_path)
}

pub fn expand_manifest(script: &RawScript, config: &Config) -> CargoResult<String> {
    let manifest = expand_manifest_(script, config)
        .with_context(|| format!("failed to parse manifest at {}", script.path.display()))?;
    let manifest = remap_paths(
        manifest,
        script.path.parent().ok_or_else(|| {
            anyhow::format_err!("no parent directory for {}", script.path.display())
        })?,
    )?;
    let manifest = toml::to_string_pretty(&manifest)?;
    Ok(manifest)
}

fn expand_manifest_(script: &RawScript, config: &Config) -> CargoResult<toml::Table> {
    let mut manifest: toml::Table = toml::from_str(&script.manifest)?;

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
    let name = package_name(script)?;
    let hash = hash(script);
    let bin_name = format!("{name}_{hash}");
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
        toml::Value::String(
            script
                .path
                .to_str()
                .ok_or_else(|| anyhow::format_err!("path is not valid UTF-8"))?
                .into(),
        ),
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

fn package_name(script: &RawScript) -> CargoResult<String> {
    let name = script
        .path
        .file_stem()
        .ok_or_else(|| anyhow::format_err!("no file name"))?
        .to_string_lossy();
    let mut slug = String::new();
    for (i, c) in name.chars().enumerate() {
        match (i, c) {
            (0, '0'..='9') => {
                slug.push('_');
                slug.push(c);
            }
            (_, '0'..='9') | (_, 'a'..='z') | (_, '_') | (_, '-') => {
                slug.push(c);
            }
            (_, 'A'..='Z') => {
                // Convert uppercase characters to lowercase to avoid `non_snake_case` warnings.
                slug.push(c.to_ascii_lowercase());
            }
            (_, _) => {
                slug.push('_');
            }
        }
    }
    Ok(slug)
}

fn hash(script: &RawScript) -> blake3::Hash {
    blake3::hash(script.body.as_bytes())
}

fn default_target_dir() -> CargoResult<std::path::PathBuf> {
    let mut cargo_home = home::cargo_home()?;
    cargo_home.push("eval");
    cargo_home.push("target");
    Ok(cargo_home)
}

fn write_if_changed(path: &std::path::Path, new: &str) -> CargoResult<()> {
    let write_needed = match std::fs::read_to_string(path) {
        Ok(current) => current != new,
        Err(_) => true,
    };
    if write_needed {
        std::fs::write(path, new).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
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
            let script = parse($i, std::path::Path::new("/home/me/test.rs"))
                .unwrap_or_else(|err| panic!("{}", err));
            expand_manifest(&script, &Config::default().unwrap())
                .unwrap_or_else(|err| panic!("{}", err))
        }};
    }

    #[test]
    fn test_default() {
        snapbox::assert_eq(
            r#"[[bin]]
name = "test_a472c7a31645d310613df407eab80844346938a3b8fe4f392cae059cb181aa85"
path = "/home/me/test.rs"

[package]
edition = "2021"
name = "test"
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
name = "test_3a1fa07700654ea2e893f70bb422efa7884eb1021ccacabc5466efe545da8a0b"
path = "/home/me/test.rs"

[dependencies]
time = "0.1.25"

[package]
edition = "2021"
name = "test"
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

/// Given a Cargo manifest, attempts to rewrite relative file paths to absolute ones, allowing the manifest to be relocated.
fn remap_paths(
    mani: toml::Table,
    package_root: &std::path::Path,
) -> anyhow::Result<toml::value::Table> {
    // Values that need to be rewritten:
    let paths: &[&[&str]] = &[
        &["build-dependencies", "*", "path"],
        &["dependencies", "*", "path"],
        &["dev-dependencies", "*", "path"],
        &["package", "build"],
        &["target", "*", "dependencies", "*", "path"],
    ];

    let mut mani = toml::Value::Table(mani);

    for path in paths {
        iterate_toml_mut_path(&mut mani, path, &mut |v| {
            if let toml::Value::String(s) = v {
                if std::path::Path::new(s).is_relative() {
                    let p = package_root.join(&*s);
                    if let Some(p) = p.to_str() {
                        *s = p.into()
                    }
                }
            }
            Ok(())
        })?
    }

    match mani {
        toml::Value::Table(mani) => Ok(mani),
        _ => unreachable!(),
    }
}

/// Iterates over the specified TOML values via a path specification.
fn iterate_toml_mut_path<F>(
    base: &mut toml::Value,
    path: &[&str],
    on_each: &mut F,
) -> anyhow::Result<()>
where
    F: FnMut(&mut toml::Value) -> anyhow::Result<()>,
{
    if path.is_empty() {
        return on_each(base);
    }

    let cur = path[0];
    let tail = &path[1..];

    if cur == "*" {
        if let toml::Value::Table(tab) = base {
            for (_, v) in tab {
                iterate_toml_mut_path(v, tail, on_each)?;
            }
        }
    } else if let toml::Value::Table(tab) = base {
        if let Some(v) = tab.get_mut(cur) {
            iterate_toml_mut_path(v, tail, on_each)?;
        }
    }

    Ok(())
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
