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
    let source = split_source(content)?;
    if let Some(frontmatter) = source.frontmatter {
        match source.info {
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
        if let Some(shebang) = source.shebang {
            writeln!(hacked_source, "{shebang}")?;
        }
        writeln!(hacked_source)?; // open
        for _ in 0..frontmatter.lines().count() {
            writeln!(hacked_source)?;
        }
        writeln!(hacked_source)?; // close
        writeln!(hacked_source, "{}", source.content)?;
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

struct Source<'s> {
    shebang: Option<&'s str>,
    info: Option<&'s str>,
    frontmatter: Option<&'s str>,
    content: &'s str,
}

fn split_source(input: &str) -> CargoResult<Source<'_>> {
    let mut source = Source {
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
        let (shebang, content) = source
            .content
            .split_once('\n')
            .unwrap_or((source.content, ""));
        source.shebang = Some(shebang);
        source.content = content;
    }

    // Experiment: let us try which char works better
    let tick_char = '-';

    let tick_end = source
        .content
        .char_indices()
        .find_map(|(i, c)| (c != tick_char).then_some(i))
        .unwrap_or(source.content.len());
    let (fence_pattern, rest) = match tick_end {
        0 => {
            return Ok(source);
        }
        1 | 2 => {
            anyhow::bail!("found {tick_end} `{tick_char}` in rust frontmatter, expected at least 3")
        }
        _ => source.content.split_at(tick_end),
    };
    let (info, content) = rest.split_once("\n").unwrap_or((rest, ""));
    if !info.is_empty() {
        source.info = Some(info.trim_end());
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

#[cfg(test)]
mod test_expand {
    use snapbox::str;

    use super::*;

    macro_rules! si {
        ($i:expr) => {{
            let shell = crate::Shell::from_write(Box::new(Vec::new()));
            let cwd = std::env::current_dir().unwrap();
            let home = home::cargo_home_with_cwd(&cwd).unwrap();
            let gctx = GlobalContext::new(shell, cwd, home);
            expand_manifest($i, std::path::Path::new("/home/me/test.rs"), &gctx)
                .unwrap_or_else(|err| panic!("{}", err))
        }};
    }

    #[test]
    fn test_default() {
        snapbox::assert_data_eq!(
            si!(r#"fn main() {}"#),
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
edition = "2021"
name = "test-"

[profile.release]
strip = true

[workspace]

"#]]
        );
    }

    #[test]
    fn test_dependencies() {
        snapbox::assert_data_eq!(
            si!(r#"---cargo
[dependencies]
time="0.1.25"
---
fn main() {}
"#),
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
edition = "2021"
name = "test-"

[profile.release]
strip = true

[workspace]

"#]]
        );
    }

    #[test]
    fn test_no_infostring() {
        snapbox::assert_data_eq!(
            si!(r#"---
[dependencies]
time="0.1.25"
---
fn main() {}
"#),
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
edition = "2021"
name = "test-"

[profile.release]
strip = true

[workspace]

"#]]
        );
    }
}
