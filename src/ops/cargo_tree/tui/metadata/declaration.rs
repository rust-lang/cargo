//! Format the selected dependency as a simplified manifest declaration.

use std::borrow::Cow;

use crate::ops::cargo_tree::tui::dependency::DependencyUsage;
use crate::util::style::VALID;
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(super) fn toml_wrapped_line_count(line: &Line<'_>, width: usize) -> usize {
    let width = width.max(1);
    let line_width = line.width();
    line_width.max(1).div_ceil(width) + usize::from(line_width > width)
}

pub(super) fn push_compact_field<'a>(
    lines: &mut Vec<Line<'a>>,
    label: &'static str,
    value: impl Into<Cow<'a, str>>,
) {
    lines.push(Line::from(vec![
        Span::styled(label, Style::from(VALID)),
        Span::raw(": "),
        Span::raw(value),
    ]));
}

pub(super) fn format_usage_declaration(usage: &DependencyUsage, version: &str) -> (String, String) {
    let dependency = &usage.dependency;
    let section = usage.kind.kind_table();
    let section = match dependency.platform() {
        Some(target) => format!("[target.'{target}'.{section}]"),
        None => format!("[{section}]"),
    };
    let mut fields = Vec::with_capacity(5);
    fields.push(format!("version = {version:?}"));
    if dependency.package_name() != dependency.name_in_toml() {
        fields.push(format!(
            "package = {:?}",
            dependency.package_name().as_str()
        ));
    }
    if !dependency.features().is_empty() {
        let features = dependency
            .features()
            .iter()
            .map(|feature| format!("{:?}", feature.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        fields.push(format!("features = [{features}]"));
    }
    if dependency.is_optional() {
        fields.push("optional = true".to_owned());
    }
    if !dependency.uses_default_features() {
        fields.push("default-features = false".to_owned());
    }
    let declaration = format!(
        "{} = {{ {} }}",
        dependency.name_in_toml(),
        fields.join(", ")
    );
    (section, declaration)
}
