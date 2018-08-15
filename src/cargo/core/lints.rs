use std::collections::BTreeMap;
use std::collections::HashSet;
use std::str::FromStr;

use util::{Cfg, CfgExpr, ProcessBuilder};
use util::errors::CargoResult;

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
    cfg: Option<CfgExpr>,
}

impl Lints {
    pub fn new(
        cfg: Option<&String>,
        manifest_lints: &BTreeMap<String, String>,
        warnings: &mut Vec<String>,
    ) -> CargoResult<Lints> {
        let cfg = if let Some(t) = cfg {
            if t.starts_with("cfg(") && t.ends_with(')') {
                Some(CfgExpr::from_str(&t[4..t.len() - 1])?)
            } else {
                bail!("expected `cfg(...)`, found {}", t)
            }
        } else {
            None
        };

        let mut lints = vec![];
        for (lint_name, lint_state) in manifest_lints.iter() {
            if let Some(state) = LintKind::try_from_string(lint_state) {
                lints.push((lint_name.to_string(), state));
            } else {
                warnings.push(format!(
                    "invalid lint state for `{}` (expected `warn`, `allow`, `deny` or `forbid`)",
                    lint_name
                ));
            }
        }
        Ok(Lints { lints, cfg })
    }

    pub fn set_lint_flags(&self, unit_cfg: &[Cfg], features: &HashSet<String>, cmd: &mut ProcessBuilder) {
        match self.cfg {
            None => self.set_flags(cmd),
            Some(CfgExpr::Value(Cfg::KeyPair(ref key, ref value)))
                if key == "feature" && features.contains(value) => self.set_flags(cmd),
            Some(ref cfg) if cfg.matches(unit_cfg) => self.set_flags(cmd),
            _ => (),
        }
    }

    fn set_flags(&self, cmd: &mut ProcessBuilder) {
        for (lint_name, state) in self.lints.iter() {
            cmd.arg(format!("-{}", state.flag())).arg(lint_name);
        }
    }
}
