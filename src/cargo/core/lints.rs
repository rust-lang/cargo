use std::collections::BTreeMap;

use util::{CargoResult, ProcessBuilder};

#[derive(Clone, PartialEq, Debug)]
enum LintKind {
    Allow,
    Warn,
    Deny,
    Forbid,
}

impl LintKind {
    pub fn try_from_string(lint_state: &str) -> Option<LintKind> {
        match lint_state.as_ref() {
            "allow" => Some(LintKind::Allow),
            "warn" => Some(LintKind::Warn),
            "deny" => Some(LintKind::Deny),
            "forbid" => Some(LintKind::Forbid),
            _ => None,
        }
    }

    pub fn flag(&self) -> char {
        match self {
            LintKind::Allow => 'A',
            LintKind::Warn => 'W',
            LintKind::Deny => 'D',
            LintKind::Forbid => 'F',
        }
    }
}

#[derive(Clone, Debug)]
pub struct Lints {
    lints: Vec<(String, LintKind)>,
}

impl Lints {
    pub fn new(
        manifest_lints: Option<&BTreeMap<String, String>>,
        warnings: &mut Vec<String>,
    ) -> CargoResult<Lints> {
        let mut lints = vec![];
        if let Some(lint_section) = manifest_lints {
            for (lint_name, lint_state) in lint_section.iter() {
                if let Some(state) = LintKind::try_from_string(lint_state) {
                    lints.push((lint_name.to_string(), state));
                } else {
                    warnings.push(format!(
                        "invalid lint state for `{}` (expected `warn`, `allow`, `deny` or `forbid`)",
                        lint_name
                    ));
                }
            }
        }
        Ok(Lints { lints })
    }

    pub fn set_flags(&self, cmd: &mut ProcessBuilder) {
        for (lint_name, state) in self.lints.iter() {
            cmd.arg(format!("-{}", state.flag())).arg(lint_name);
        }
    }
}
