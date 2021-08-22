//! Expands environment variable references in strings.

use crate::util::CargoResult;
use std::borrow::Cow;

pub fn expand_env_vars<'a>(s: &'a str) -> CargoResult<Cow<'a, str>> {
    expand_env_vars_with(s, |n| std::env::var(n).ok())
}

pub fn expand_env_vars_with<'a, Q>(s: &'a str, query: Q) -> CargoResult<Cow<'a, str>>
where
    Q: Fn(&str) -> Option<String>,
{
    // Most strings do not contain environment variable references.
    // We optimize for the case where there is no reference, and
    // return the same (borrowed) string.
    if s.contains('$') {
        Ok(Cow::Owned(expand_env_vars_with_slow(s, query)?))
    } else {
        Ok(Cow::Borrowed(s))
    }
}

fn expand_env_vars_with_slow<Q>(s: &str, query: Q) -> CargoResult<String>
where
    Q: Fn(&str) -> Option<String>,
{
    let mut result = String::with_capacity(s.len() + 50);
    let mut rest = s;
    while let Some(pos) = rest.find('$') {
        let (lo, hi) = rest.split_at(pos);
        result.push_str(lo);
        let mut ci = hi.chars();
        let c0 = ci.next();
        debug_assert_eq!(c0, Some('$')); // because rest.find()
        match ci.next() {
            Some('(') => {
                // the expected case, which is handled below.
            }
            Some(c) => {
                // We found '$' that was not paired with '('.
                // This is not a variable reference.
                // Output the $ and continue.
                result.push('$');
                result.push(c);
                rest = ci.as_str();
                continue;
            }
            None => {
                // We found '$ at the end of the string.
                result.push('$');
                break;
            }
        }
        let name_start = ci.as_str();
        let mut name: &str = "";
        let mut default_value: Option<&str> = None;
        // Look for ')' or '?'
        loop {
            let ci_s = ci.as_str();
            let pos = name_start.len() - ci.as_str().len();
            match ci.next() {
                None => {
                    anyhow::bail!("environment variable reference is missing closing parenthesis.")
                }
                Some(')') => {
                    match default_value {
                        Some(d) => default_value = Some(&d[..d.len() - ci_s.len()]),
                        None => name = &name_start[..name_start.len() - ci_s.len()],
                    }
                    rest = ci.as_str();
                    break;
                }
                Some('?') => {
                    if default_value.is_some() {
                        anyhow::bail!("environment variable reference has too many '?'");
                    }
                    name = &name_start[..pos];
                    default_value = Some(ci.as_str());
                }
                Some(_) if default_value.is_some() => {
                    // consume this, for default value
                }
                Some(c) if is_legal_env_var_char(c) => {
                    // continue for next char
                }
                Some(c) => {
                    anyhow::bail!("environment variable reference has invalid character {:?}.", c);
                }
            }
        }

        if name.is_empty() {
            anyhow::bail!("environment variable reference has invalid empty name");
        }
        // We now have the environment variable name, and have parsed the end of the
        // name reference.
        match (query(name), default_value) {
            (Some(value), _) => result.push_str(&value),
            (None, Some(value)) => result.push_str(value),
            (None, None) => anyhow::bail!(format!(
                "environment variable '{}' is not set and has no default value",
                name
            )),
        }
    }
    result.push_str(rest);

    Ok(result)
}

fn is_legal_env_var_char(c: char) -> bool {
    match c {
        'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_str_contains {
        ($haystack:expr, $needle:expr) => {{
            let haystack = $haystack;
            let needle = $needle;
            assert!(
                haystack.contains(needle),
                "expected {:?} to contain {:?}",
                haystack,
                needle
            );
        }};
    }

    #[test]
    fn basic() {
        let query = |name: &str| match name {
            "FOO" => Some("c:/foo".to_string()),
            "BAR" => Some("c:/bar".to_string()),
            _ => None,
        };

        let expand = |s| expand_env_vars_with(s, query);
        let expand_ok = |s| match expand(s) {
            Ok(value) => value,
            Err(e) => panic!(
                "expected '{}' to expand successfully, but failed: {:?}",
                s, e
            ),
        };
        let expand_err = |s| expand(s).unwrap_err().to_string();
        assert_eq!(expand_ok(""), "");
        assert_eq!(expand_ok("identity"), "identity");
        assert_eq!(expand_ok("$(FOO)/some_package"), "c:/foo/some_package");
        assert_eq!(expand_ok("$FOO/some_package"), "$FOO/some_package");

        assert_eq!(expand_ok("alpha $(FOO) beta"), "alpha c:/foo beta");
        assert_eq!(
            expand_ok("one $(FOO) two $(BAR) three"),
            "one c:/foo two c:/bar three"
        );

        assert_eq!(expand_ok("one $(FOO)"), "one c:/foo");

        // has default, but value is present
        assert_eq!(expand_ok("$(FOO?d:/default)"), "c:/foo");
        assert_eq!(expand_ok("$(ZAP?d:/default)"), "d:/default");

        // error cases
        assert_eq!(
            expand_err("$(VAR_NOT_SET)"),
            "environment variable 'VAR_NOT_SET' is not set and has no default value"
        );

        // invalid name
        assert_str_contains!(
            expand("$(111)").unwrap_err().to_string(),
            "" // "environment variable reference has invalid character."
        );

        expand_err("$(");
        expand_err("$(FOO");
    }
}
