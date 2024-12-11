use anyhow::Context as _;

use cargo_util_schemas::manifest::PackageName;

use crate::util::restricted_names;
use crate::CargoResult;
use crate::GlobalContext;

const DEFAULT_EDITION: crate::core::features::Edition =
    crate::core::features::Edition::LATEST_STABLE;
const AUTO_FIELDS: &[&str] = &[
    "autolib",
    "autobins",
    "autoexamples",
    "autotests",
    "autobenches",
];

pub(super) fn expand_manifest(
    content: &str,
    path: &std::path::Path,
    gctx: &GlobalContext,
) -> CargoResult<String> {
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

        // HACK: until rustc has native support for this syntax, we have to remove it from the
        // source file
        use std::fmt::Write as _;
        let hash = crate::util::hex::short_hash(&path.to_string_lossy());
        let mut rel_path = std::path::PathBuf::new();
        rel_path.push("target");
        rel_path.push(&hash[0..2]);
        rel_path.push(&hash[2..]);
        let target_dir = gctx.home().join(rel_path);
        let hacked_path = target_dir
            .join(
                path.file_name()
                    .expect("always a name for embedded manifests"),
            )
            .into_path_unlocked();
        let mut hacked_source = String::new();
        if let Some(shebang) = source.shebang() {
            writeln!(hacked_source, "{shebang}")?;
        }
        writeln!(hacked_source)?; // open
        for _ in 0..frontmatter.lines().count() {
            writeln!(hacked_source)?;
        }
        writeln!(hacked_source)?; // close
        writeln!(hacked_source, "{}", source.content())?;
        if let Some(parent) = hacked_path.parent() {
            cargo_util::paths::create_dir_all(parent)?;
        }
        cargo_util::paths::write_if_changed(&hacked_path, hacked_source)?;

        let manifest = expand_manifest_(&frontmatter, &hacked_path, gctx)
            .with_context(|| format!("failed to parse manifest at {}", path.display()))?;
        let manifest = toml::to_string_pretty(&manifest)?;
        Ok(manifest)
    } else {
        let frontmatter = "";
        let manifest = expand_manifest_(frontmatter, path, gctx)
            .with_context(|| format!("failed to parse manifest at {}", path.display()))?;
        let manifest = toml::to_string_pretty(&manifest)?;
        Ok(manifest)
    }
}

fn expand_manifest_(
    manifest: &str,
    path: &std::path::Path,
    gctx: &GlobalContext,
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
    for key in ["workspace", "build", "links"]
        .iter()
        .chain(AUTO_FIELDS.iter())
    {
        if package.contains_key(*key) {
            anyhow::bail!("`package.{key}` is not allowed in embedded manifests")
        }
    }
    // HACK: Using an absolute path while `hacked_path` is in use
    let bin_path = path.to_string_lossy().into_owned();
    let file_stem = path
        .file_stem()
        .ok_or_else(|| anyhow::format_err!("no file name"))?
        .to_string_lossy();
    let name = sanitize_name(file_stem.as_ref());
    let bin_name = name.clone();
    package
        .entry("name".to_owned())
        .or_insert(toml::Value::String(name));
    package.entry("edition".to_owned()).or_insert_with(|| {
        let _ = gctx.shell().warn(format_args!(
            "`package.edition` is unspecified, defaulting to `{}`",
            DEFAULT_EDITION
        ));
        toml::Value::String(DEFAULT_EDITION.to_string())
    });
    package
        .entry("build".to_owned())
        .or_insert_with(|| toml::Value::Boolean(false));
    for field in AUTO_FIELDS {
        package
            .entry(field.to_owned())
            .or_insert_with(|| toml::Value::Boolean(false));
    }

    let mut bin = toml::Table::new();
    bin.insert("name".to_owned(), toml::Value::String(bin_name));
    bin.insert("path".to_owned(), toml::Value::String(bin_path));
    manifest.insert(
        "bin".to_owned(),
        toml::Value::Array(vec![toml::Value::Table(bin)]),
    );

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

        // See rust-lang/rust's compiler/rustc_lexer/src/lib.rs's `strip_shebang`
        // Shebang must start with `#!` literally, without any preceding whitespace.
        // For simplicity we consider any line starting with `#!` a shebang,
        // regardless of restrictions put on shebangs by specific platforms.
        if let Some(rest) = source.content.strip_prefix("#!") {
            // Ok, this is a shebang but if the next non-whitespace token is `[`,
            // then it may be valid Rust code, so consider it Rust code.
            if rest.trim_start().starts_with('[') {
                return Ok(source);
            }

            // No other choice than to consider this a shebang.
            let newline_end = source
                .content
                .find('\n')
                .map(|pos| pos + 1)
                .unwrap_or(source.content.len());
            let (shebang, content) = source.content.split_at(newline_end);
            source.shebang = Some(shebang);
            source.content = content;
        }

        const FENCE_CHAR: char = '-';

        let mut trimmed_content = source.content;
        while !trimmed_content.is_empty() {
            let c = trimmed_content;
            let c = c.trim_start_matches([' ', '\t']);
            let c = c.trim_start_matches(['\r', '\n']);
            if c == trimmed_content {
                break;
            }
            trimmed_content = c;
        }
        let fence_end = trimmed_content
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
            _ => trimmed_content.split_at(fence_end),
        };
        let (info, content) = rest.split_once("\n").unwrap_or((rest, ""));
        let info = info.trim();
        if !info.is_empty() {
            source.info = Some(info);
        }
        source.content = content;

        let Some((frontmatter, content)) = source.content.split_once(fence_pattern) else {
            anyhow::bail!("no closing `{fence_pattern}` found for frontmatter");
        };
        source.frontmatter = Some(frontmatter);
        source.content = content;

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
        let shell = crate::Shell::from_write(Box::new(Vec::new()));
        let cwd = std::env::current_dir().unwrap();
        let home = home::cargo_home_with_cwd(&cwd).unwrap();
        let gctx = GlobalContext::new(shell, cwd, home);
        expand_manifest(source, std::path::Path::new("/home/me/test.rs"), &gctx)
            .unwrap_or_else(|err| panic!("{}", err))
    }

    #[test]
    fn expand_default() {
        assert_data_eq!(
            expand(r#"fn main() {}"#),
            str![[r#"
[[bin]]
name = "test-"
path = "/home/me/test.rs"

[package]
autobenches = false
autobins = false
autoexamples = false
autolib = false
autotests = false
build = false
edition = "2024"
name = "test-"

[workspace]

"#]]
        );
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
[[bin]]
name = "test-"
path = [..]

[dependencies]
time = "0.1.25"

[package]
autobenches = false
autobins = false
autoexamples = false
autolib = false
autotests = false
build = false
edition = "2024"
name = "test-"

[workspace]

"#]]
        );
    }
}
