//! check-cfg

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Display,
};

/// Check Config (aka `--check-cfg`/`--print=check-cfg` representation)
#[derive(Debug, Default, Clone)]
pub struct CheckCfg {
    /// Is `--check-cfg` activated
    pub exhaustive: bool,
    /// List of expected cfgs
    pub expecteds: HashMap<String, ExpectedValues>,
}

/// List of expected check-cfg values
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedValues {
    /// List of expected values
    ///
    ///  - `#[cfg(foo)]` value is `None`
    ///  - `#[cfg(foo = "")]` value is `Some("")`
    ///  - `#[cfg(foo = "bar")]` value is `Some("bar")`
    Some(HashSet<Option<String>>),
    /// All values expected
    Any,
}

/// Error when parse a line from `--print=check-cfg`
#[derive(Debug)]
#[non_exhaustive]
pub struct PrintCheckCfgParsingError;

impl Error for PrintCheckCfgParsingError {}

impl Display for PrintCheckCfgParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("error when parsing a `--print=check-cfg` line")
    }
}

impl CheckCfg {
    /// Parse a line from `--print=check-cfg`
    pub fn parse_print_check_cfg_line(
        &mut self,
        line: &str,
    ) -> Result<(), PrintCheckCfgParsingError> {
        if line == "any()=any()" || line == "any()" {
            self.exhaustive = false;
            return Ok(());
        }

        let mut value: HashSet<Option<String>> = HashSet::default();
        let mut value_any_specified = false;
        let name: String;

        if let Some((n, val)) = line.split_once('=') {
            name = n.to_string();

            if val == "any()" {
                value_any_specified = true;
            } else if val.is_empty() {
                // no value, nothing to add
            } else if let Some(val) = maybe_quoted_value(val) {
                value.insert(Some(val.to_string()));
            } else {
                // missing quotes and non-empty
                return Err(PrintCheckCfgParsingError);
            }
        } else {
            name = line.to_string();
            value.insert(None);
        }

        self.expecteds
            .entry(name)
            .and_modify(|v| match v {
                ExpectedValues::Some(_) if value_any_specified => *v = ExpectedValues::Any,
                ExpectedValues::Some(v) => v.extend(value.clone()),
                ExpectedValues::Any => {}
            })
            .or_insert_with(|| {
                if value_any_specified {
                    ExpectedValues::Any
                } else {
                    ExpectedValues::Some(value)
                }
            });
        Ok(())
    }
}

fn maybe_quoted_value<'a>(v: &'a str) -> Option<&'a str> {
    // strip "" around the value, e.g. "linux" -> linux
    v.strip_prefix('"').and_then(|v| v.strip_suffix('"'))
}
