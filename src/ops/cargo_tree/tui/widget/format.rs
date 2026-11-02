use anyhow::{Result, bail};
use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

use crate::ops::cargo_tree::format::parse::{Parser, RawChunk};
use crate::ops::cargo_tree::tui::dependency::{Dependency, PackageFormatRequirements};

#[derive(Debug, Clone)]
enum Chunk {
    Text(String),
    Package,
    License,
    Repository,
    Features,
    Library,
}

/// Parsed Cargo-style package display format.
#[derive(Debug, Clone)]
pub struct TreeFormat {
    raw: String,
    chunks: Vec<Chunk>,
    requirements: PackageFormatRequirements,
}

#[derive(Default)]
pub(super) struct FormatStyles {
    pub name: Style,
    pub license: Style,
    pub feature: Style,
    pub version: Style,
    pub suffix: Style,
}

impl TreeFormat {
    pub fn parse(format: &str) -> Result<Self> {
        let mut chunks = Vec::new();
        for raw in Parser::new(format) {
            let chunk = match raw {
                RawChunk::Text(text) => Chunk::Text(text.to_owned()),
                RawChunk::Argument("p" | "package") => Chunk::Package,
                RawChunk::Argument("l" | "license") => Chunk::License,
                RawChunk::Argument("r" | "repository") => Chunk::Repository,
                RawChunk::Argument("f" | "features") => Chunk::Features,
                RawChunk::Argument("lib") => Chunk::Library,
                RawChunk::Argument(name) => {
                    bail!("tree format `{format}` not valid: unsupported pattern `{name}`")
                }
                RawChunk::Error(error) => bail!("tree format `{format}` not valid: {error}"),
            };
            chunks.push(chunk);
        }

        let requirements = PackageFormatRequirements {
            license: chunks.iter().any(|chunk| matches!(chunk, Chunk::License)),
            repository: chunks
                .iter()
                .any(|chunk| matches!(chunk, Chunk::Repository)),
            features: chunks.iter().any(|chunk| matches!(chunk, Chunk::Features)),
            library: chunks.iter().any(|chunk| matches!(chunk, Chunk::Library)),
            source: chunks.iter().any(|chunk| matches!(chunk, Chunk::Package)),
            metadata: false,
        };

        Ok(Self {
            raw: format.to_owned(),
            chunks,
            requirements,
        })
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn fields(&self) -> [bool; 5] {
        [
            self.chunks
                .iter()
                .any(|chunk| matches!(chunk, Chunk::Package)),
            self.chunks
                .iter()
                .any(|chunk| matches!(chunk, Chunk::License)),
            self.chunks
                .iter()
                .any(|chunk| matches!(chunk, Chunk::Repository)),
            self.chunks
                .iter()
                .any(|chunk| matches!(chunk, Chunk::Features)),
            self.chunks
                .iter()
                .any(|chunk| matches!(chunk, Chunk::Library)),
        ]
    }

    pub fn requirements(&self) -> PackageFormatRequirements {
        self.requirements
    }

    pub(super) fn append_spans<'a>(
        &'a self,
        dependency: &'a Dependency,
        spans: &mut Vec<Span<'a>>,
        styles: FormatStyles,
    ) {
        for chunk in &self.chunks {
            match chunk {
                Chunk::Text(text) => {
                    spans.push(Span::styled(text.as_str(), styles.name));
                }
                Chunk::Package => {
                    spans.push(Span::styled(dependency.name.as_str(), styles.name));
                    spans.push(Span::styled(" v", styles.version));
                    spans.push(Span::styled(dependency.version.as_str(), styles.version));
                    if dependency.is_proc_macro {
                        spans.push(Span::styled(" (proc-macro)", styles.suffix));
                    }
                    if let Some(source) = dependency
                        .format_metadata
                        .as_ref()
                        .and_then(|metadata| metadata.source.as_deref())
                    {
                        spans.push(Span::styled(" (", styles.suffix));
                        spans.push(Span::styled(source, styles.suffix));
                        spans.push(Span::styled(")", styles.suffix));
                    }
                }
                Chunk::License => push_value(
                    spans,
                    dependency
                        .format_metadata
                        .as_ref()
                        .and_then(|metadata| metadata.license.as_deref()),
                    styles.license,
                ),
                Chunk::Repository => push_value(
                    spans,
                    dependency
                        .format_metadata
                        .as_ref()
                        .and_then(|metadata| metadata.repository.as_deref()),
                    styles.name.add_modifier(Modifier::ITALIC),
                ),
                Chunk::Features => push_value(
                    spans,
                    dependency
                        .format_metadata
                        .as_ref()
                        .and_then(|metadata| metadata.features.as_deref()),
                    styles.feature,
                ),
                Chunk::Library => push_value(
                    spans,
                    dependency
                        .format_metadata
                        .as_ref()
                        .and_then(|metadata| metadata.library.as_deref()),
                    styles.name,
                ),
            }
        }
    }
}

fn push_value<'a>(spans: &mut Vec<Span<'a>>, value: Option<&'a str>, style: Style) {
    if let Some(value) = value {
        spans.push(Span::styled(value, style));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::ops::cargo_tree::tui::dependency::{Dependency, PackageFormatMetadata};

    use super::TreeFormat;

    fn dependency(format_metadata: Option<PackageFormatMetadata>) -> Dependency {
        Dependency {
            name: "demo".to_owned(),
            version: "1.2.3".to_owned(),
            package_id: None,
            manifest_dir: None,
            is_proc_macro: false,
            format_metadata: format_metadata.map(Arc::new),
            children: Vec::new(),
        }
    }

    fn render(format: &TreeFormat, dependency: &Dependency) -> String {
        let mut spans = Vec::new();
        format.append_spans(dependency, &mut spans, super::FormatStyles::default());
        spans.iter().fold(String::new(), |mut output, span| {
            output.push_str(span.content.as_ref());
            output
        })
    }

    #[test]
    fn aliases_request_their_cargo_metadata() {
        let format =
            TreeFormat::parse("{p} {package} {l} {license} {r} {repository} {f} {features} {lib}")
                .unwrap();

        let requirements = format.requirements();
        assert!(requirements.source);
        assert!(requirements.license);
        assert!(requirements.repository);
        assert!(requirements.features);
        assert!(requirements.library);

        let license_only = TreeFormat::parse("{l}").unwrap().requirements();
        assert!(license_only.license);
        assert!(!license_only.source);
        assert!(!license_only.repository);
        assert!(!license_only.features);
        assert!(!license_only.library);
    }

    #[test]
    fn formats_package_and_metadata_values() {
        let format = TreeFormat::parse("{p}|{l}|{r}|{f}|{lib}").unwrap();
        let mut dependency = dependency(Some(PackageFormatMetadata {
            license: Some("MIT".to_owned()),
            repository: Some("https://example.com/demo".to_owned()),
            features: Some("default,serde".to_owned()),
            library: Some("demo_lib".to_owned()),
            source: Some("git+https://example.com/demo".to_owned()),
            ..PackageFormatMetadata::default()
        }));
        dependency.is_proc_macro = true;

        assert_eq!(
            render(&format, &dependency),
            "demo v1.2.3 (proc-macro) (git+https://example.com/demo)|MIT|https://example.com/demo|default,serde|demo_lib"
        );
    }

    #[test]
    fn escaped_braces_are_literal_text() {
        let format = TreeFormat::parse("{{literal}} {p} }}").unwrap();
        let dependency = dependency(None);

        assert_eq!(render(&format, &dependency), "{literal} demo v1.2.3 }");
    }

    #[test]
    fn absent_metadata_expands_to_empty_text() {
        let format = TreeFormat::parse("{l}|{r}|{f}|{lib}|{p}").unwrap();
        let dependency = dependency(None);

        assert_eq!(render(&format, &dependency), "||||demo v1.2.3");
    }

    #[test]
    fn rejects_unsupported_or_malformed_patterns() {
        for input in ["{unknown}", "{p", "}"] {
            assert!(TreeFormat::parse(input).is_err(), "accepted {input:?}");
        }
    }
}
