use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use cargo_util::paths;
use regex::Regex;
use serde_json::json;

use crate::util::errors::CargoResult;

pub(crate) struct PublicDependencyManifest {
    path: PathBuf,
    contents: String,
    document: toml::Spanned<toml::de::DeTable<'static>>,
}

pub(crate) struct PublicDependencySuggestion {
    file_name: String,
    span: Range<usize>,
    replacement: String,
    line_start: usize,
    column_start: usize,
    column_end: usize,
    line_text: String,
}

impl PublicDependencyManifest {
    pub(crate) fn load(path: PathBuf) -> CargoResult<Self> {
        let contents = paths::read(&path)?;
        let document = crate::util::toml::parse_document(&contents)?;
        Ok(Self {
            path,
            contents,
            document,
        })
    }

    pub(crate) fn find_public_suggestion(
        &self,
        unrenamed: &str,
    ) -> Option<PublicDependencySuggestion> {
        let mut candidates = Vec::new();
        if let Some(deps) = self
            .document
            .get_ref()
            .get("dependencies")
            .and_then(|d| d.as_ref().as_table())
        {
            self.find_in_dependency_table(deps, unrenamed, &mut candidates);
        }

        if let Some(target) = self
            .document
            .get_ref()
            .get("target")
            .and_then(|t| t.as_ref().as_table())
        {
            for (_, platform_table) in target.iter() {
                let Some(platform_table) = platform_table.as_ref().as_table() else {
                    continue;
                };
                let Some(deps) = platform_table
                    .get("dependencies")
                    .and_then(|d| d.as_ref().as_table())
                else {
                    continue;
                };
                self.find_in_dependency_table(deps, unrenamed, &mut candidates);
            }
        }

        candidates.sort_by_key(|candidate| {
            (
                candidate.file_name.clone(),
                candidate.span.start,
                candidate.span.end,
            )
        });
        candidates.dedup_by(|a, b| a.file_name == b.file_name && a.span == b.span);
        if candidates.len() == 1 {
            candidates.pop()
        } else {
            None
        }
    }

    fn find_in_dependency_table(
        &self,
        deps: &toml::de::DeTable<'static>,
        unrenamed: &str,
        candidates: &mut Vec<PublicDependencySuggestion>,
    ) {
        for (key, value) in deps.iter() {
            if dependency_matches(key, value, unrenamed) {
                if let Some(suggestion) =
                    public_dependency_suggestion_from_value(&self.path, &self.contents, value)
                {
                    candidates.push(suggestion);
                }
            }
        }
    }
}

impl PublicDependencySuggestion {
    pub(crate) fn to_diagnostic_child(&self, crate_name: &str) -> serde_json::Value {
        json!({
            "message": format!("mark dependency `{crate_name}` as public"),
            "code": null,
            "level": "help",
            "spans": [{
                "file_name": self.file_name.clone(),
                "byte_start": self.span.start,
                "byte_end": self.span.end,
                "line_start": self.line_start,
                "line_end": self.line_start,
                "column_start": self.column_start,
                "column_end": self.column_end,
                "is_primary": true,
                "text": [{
                    "text": self.line_text.clone(),
                    "highlight_start": self.column_start,
                    "highlight_end": self.column_end,
                }],
                "label": "mark as public",
                "suggested_replacement": self.replacement.clone(),
                "suggestion_applicability": "MachineApplicable",
                "expansion": null,
            }],
            "children": [],
            "rendered": null,
        })
    }
}

pub(crate) fn exported_private_dependency_name(message: &str) -> Option<&str> {
    // rustc currently emits messages like:
    // - type `FromPriv` from private dependency 'priv_dep' in public interface
    // - struct `FromPriv` from private dependency 'priv_dep' is re-exported
    static PRIV_DEP_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new("from private dependency '([A-Za-z0-9-_]+)'").unwrap());
    PRIV_DEP_REGEX
        .captures(message)
        .and_then(|captures| captures.get(1))
        .map(|matched| matched.as_str())
}

pub(crate) fn add_public_dependency_suggestion_to_diagnostic(
    diagnostic: &mut serde_json::Value,
    manifest: &PublicDependencyManifest,
) -> bool {
    if !is_exported_private_dependencies(diagnostic) {
        return false;
    }

    let Some(crate_name) = diagnostic
        .get("message")
        .and_then(|message| message.as_str())
        .and_then(exported_private_dependency_name)
        .map(str::to_owned)
    else {
        return false;
    };

    let Some(suggestion) = manifest.find_public_suggestion(&crate_name) else {
        return false;
    };

    push_public_dependency_suggestion_to_diagnostic(diagnostic, &crate_name, suggestion)
}

pub(crate) fn push_public_dependency_suggestion_to_diagnostic(
    diagnostic: &mut serde_json::Value,
    crate_name: &str,
    suggestion: PublicDependencySuggestion,
) -> bool {
    if diagnostic.get("children").is_none() {
        diagnostic["children"] = json!([]);
    }
    let Some(children) = diagnostic
        .get_mut("children")
        .and_then(|children| children.as_array_mut())
    else {
        return false;
    };
    children.push(suggestion.to_diagnostic_child(crate_name));
    true
}

pub(crate) fn public_dependency_suggestion_from_value(
    path: &Path,
    contents: &str,
    value: &toml::Spanned<toml::de::DeValue<'static>>,
) -> Option<PublicDependencySuggestion> {
    match value.get_ref() {
        toml::de::DeValue::String(_) => {
            let span = value.span();
            let version = contents.get(span.clone())?;
            suggestion_from_replacement(
                path,
                contents,
                span,
                format!("{{ version = {version}, public = true }}"),
            )
        }
        _ => {
            let table = value.get_ref().as_table()?;
            if let Some(public) = table.get("public") {
                return suggestion_from_replacement(path, contents, public.span(), "true".into());
            }
            if table.get("workspace").is_some() {
                return None;
            }

            let span = value.span();
            let value_text = contents.get(span.clone())?;
            if !value_text.trim_start().starts_with('{') || !value_text.trim_end().ends_with('}') {
                return None;
            }

            let brace_pos = value_text.rfind('}')?;
            let insert_at = value_text[..brace_pos].trim_end().len();
            let before = &value_text[..insert_at];
            let after = &value_text[insert_at..];
            let separator = if before.trim_end().ends_with('{') {
                " public = true"
            } else {
                ", public = true"
            };
            suggestion_from_replacement(path, contents, span, format!("{before}{separator}{after}"))
        }
    }
}

fn is_exported_private_dependencies(diagnostic: &serde_json::Value) -> bool {
    diagnostic
        .get("code")
        .and_then(|code| code.get("code"))
        .and_then(|code| code.as_str())
        == Some("exported_private_dependencies")
}

fn dependency_matches(
    key: &toml::Spanned<std::borrow::Cow<'_, str>>,
    value: &toml::Spanned<toml::de::DeValue<'static>>,
    unrenamed: &str,
) -> bool {
    if key.as_ref() == unrenamed {
        return true;
    }
    value
        .get_ref()
        .as_table()
        .and_then(|table| table.get("package"))
        .and_then(spanned_string)
        == Some(unrenamed)
}

fn spanned_string<'a>(value: &'a toml::Spanned<toml::de::DeValue<'static>>) -> Option<&'a str> {
    match value.get_ref() {
        toml::de::DeValue::String(value) => Some(value.as_ref()),
        _ => None,
    }
}

fn suggestion_from_replacement(
    path: &Path,
    contents: &str,
    span: Range<usize>,
    replacement: String,
) -> Option<PublicDependencySuggestion> {
    let line = line_info(contents, span.clone())?;
    Some(PublicDependencySuggestion {
        file_name: path.display().to_string(),
        span,
        replacement,
        line_start: line.line_start,
        column_start: line.column_start,
        column_end: line.column_end,
        line_text: line.text,
    })
}

struct LineInfo {
    line_start: usize,
    column_start: usize,
    column_end: usize,
    text: String,
}

fn line_info(contents: &str, span: Range<usize>) -> Option<LineInfo> {
    if span.start > span.end
        || span.end > contents.len()
        || !contents.is_char_boundary(span.start)
        || !contents.is_char_boundary(span.end)
    {
        return None;
    }

    let line_start_byte = contents[..span.start]
        .rfind('\n')
        .map(|pos| pos + 1)
        .unwrap_or(0);
    let line_end_byte = contents[span.end..]
        .find('\n')
        .map(|pos| span.end + pos)
        .unwrap_or(contents.len());
    if !contents.is_char_boundary(line_start_byte) || !contents.is_char_boundary(line_end_byte) {
        return None;
    }

    let line_start = contents[..line_start_byte]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let column_start = contents[line_start_byte..span.start].chars().count() + 1;
    let column_end = contents[line_start_byte..span.end].chars().count() + 1;
    let text = contents[line_start_byte..line_end_byte]
        .trim_end_matches('\r')
        .to_string();

    Some(LineInfo {
        line_start,
        column_start,
        column_end,
        text,
    })
}
