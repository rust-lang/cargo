use std::collections::BTreeMap;
use std::collections::HashMap;

use util::{CargoResult, ProcessBuilder};

#[derive(Clone, PartialEq, Debug)]
enum LintKind {
    Allow,
    Warn,
    Deny,
}

#[derive(Clone, Debug)]
pub struct Lints {
    lints: HashMap<String, LintKind>,
}

impl Lints {
    pub fn new(
        manifest_lints: Option<&BTreeMap<String, String>>,
        warnings: &mut Vec<String>,
    ) -> CargoResult<Lints> {
        let mut lints = HashMap::new();
        if let Some(lint_section) = manifest_lints {
            for (lint_name, lint_state) in lint_section.iter() {
                match lint_state.as_ref() {
                    "allow" => { lints.insert(lint_name.to_string(), LintKind::Allow); },
                    "warn" => { lints.insert(lint_name.to_string(), LintKind::Warn); },
                    "deny" => { lints.insert(lint_name.to_string(), LintKind::Deny); },
                    _ => warnings.push(format!(
                        "invalid lint state for `{}` (expected `warn`, `allow` or `deny`)",
                        lint_name
                    )),
                }
            }
        }
        Ok(Lints { lints })
    }

    pub fn set_flags(&self, cmd: &mut ProcessBuilder, package_lints: &Lints) {
        let get_kind = |kind: LintKind| {
            self.lints.iter()
                .filter(|l| *l.1 == kind)
                .chain(package_lints.lints.iter()
                    .filter(|l| *l.1 == kind && !self.lints.contains_key(l.0)))
                .map(|l| l.0.to_string())
                .collect::<Vec<String>>()
                .join(",")
        };

        let allow = get_kind(LintKind::Allow);
        if !allow.is_empty() {
            cmd.arg("-A").arg(allow);
        }
        let warn = get_kind(LintKind::Warn);
        if !warn.is_empty() {
            cmd.arg("-W").arg(warn);
        }
        let deny = get_kind(LintKind::Deny);
        if !deny.is_empty() {
            cmd.arg("-D").arg(deny);
        }
    }
}
