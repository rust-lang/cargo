use cargo_util_schemas::manifest::PackageName;

use crate::util::frontmatter::FrontmatterError;
use crate::util::frontmatter::ScriptSource;
use crate::util::restricted_names;

pub(super) fn expand_manifest(content: &str) -> Result<String, FrontmatterError> {
    let source = ScriptSource::parse(content)?;
    if let Some(span) = source.frontmatter_span() {
        match source.info() {
            Some("cargo") | None => {}
            Some(other) => {
                if let Some(remainder) = other.strip_prefix("cargo,") {
                    return Err(FrontmatterError::new(
                        format!(
                            "cargo does not support frontmatter infostring attributes like `{remainder}` at this time"
                        ),
                        source.info_span().unwrap(),
                    ));
                } else {
                    return Err(FrontmatterError::new(
                        format!(
                            "frontmatter infostring `{other}` is unsupported by cargo; specify `cargo` for embedding a manifest"
                        ),
                        source.info_span().unwrap(),
                    ));
                }
            }
        }

        // Include from file start to frontmatter end when we parse the TOML to get line numbers
        // correct and so if a TOML error says "entire file", it shows the existing content, rather
        // than blank lines.
        //
        // HACK: Since frontmatter open isn't valid TOML, we insert a comment
        let mut frontmatter = content[0..span.end].to_owned();
        let open_span = source.open_span().unwrap();
        frontmatter.insert(open_span.start, '#');
        Ok(frontmatter)
    } else {
        // Consider the shebang to be part of the frontmatter
        // so if a TOML error says "entire file", it shows the existing content, rather
        // than blank lines.
        let span = source.shebang_span().unwrap_or(0..0);
        Ok(content[span].to_owned())
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

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;

    use super::*;

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
            str![[r##"
#---cargo
[dependencies]
time="0.1.25"

"##]]
        );
    }
}
