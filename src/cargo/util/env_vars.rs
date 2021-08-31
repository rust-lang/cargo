//! Expands environment variable references in strings.

use crate::util::CargoResult;
use std::borrow::Cow;

/// Expands a string, replacing references to variables with values provided by
/// the caller.
///
/// This function looks for references to variables, similar to environment
/// variable references in command-line shells or Makefiles, and replaces the
/// references with values.  The caller provides a `query` function which gives
/// the values of the variables.
///
/// The syntax used for variable references is `${name}` or `${name?default}` if
/// a default value is provided. The curly braces are always required;
/// `$FOO` will not be interpreted as a variable reference (and will be copied
/// to the output).
///
/// If a variable is referenced, then it must have a value (`query` must return
/// `Some`) or the variable reference must provide a default value (using the
/// `...?default` syntax). If `query` returns `None` and the variable reference
/// does not provide a default value, then the expansion of the entire string
/// will fail and the function will return `Err`.
///
/// Most strings processed by Cargo will not contain variable references.
/// Hence, this function uses `Cow<str>` for its return type; it will return
/// its input string as `Cow::Borrowed` if no variable references were found.
pub fn expand_vars_with<'a, Q>(s: &'a str, query: Q) -> CargoResult<Cow<'a, str>>
where
    Q: Fn(&str) -> CargoResult<Option<String>>,
{
    let mut rest: &str;
    let mut result: String;
    if let Some(pos) = s.find('$') {
        result = String::with_capacity(s.len() + 50);
        result.push_str(&s[..pos]);
        rest = &s[pos..];
    } else {
        // Most strings do not contain environment variable references.
        // We optimize for the case where there is no reference, and
        // return the same (borrowed) string.
        return Ok(Cow::Borrowed(s));
    };

    while let Some(pos) = rest.find('$') {
        result.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let mut chars = rest.chars();
        let c0 = chars.next();
        debug_assert_eq!(c0, Some('$')); // because rest.find()
        match chars.next() {
            Some('{') => {
                // the expected case, which is handled below.
            }
            Some(c) => {
                // We found '$' that was not paired with '{'.
                // This is not a variable reference.
                // Output the $ and continue.
                result.push('$');
                result.push(c);
                rest = chars.as_str();
                continue;
            }
            None => {
                // We found '$' at the end of the string.
                result.push('$');
                break;
            }
        }
        let name_start = chars.as_str();
        let name: &str;
        let default_value: Option<&str>;
        // Look for '}' or '?'
        loop {
            let pos = name_start.len() - chars.as_str().len();
            match chars.next() {
                None => {
                    anyhow::bail!("environment variable reference is missing closing brace.")
                }
                Some('}') => {
                    name = &name_start[..pos];
                    default_value = None;
                    break;
                }
                Some('?') => {
                    name = &name_start[..pos];
                    let default_value_start = chars.as_str();
                    loop {
                        let pos = chars.as_str();
                        if let Some(c) = chars.next() {
                            if c == '}' {
                                default_value = Some(
                                    &default_value_start[..default_value_start.len() - pos.len()],
                                );
                                break;
                            }
                        } else {
                            anyhow::bail!(
                                "environment variable reference is missing closing brace."
                            );
                        }
                    }
                    break;
                }
                Some(_) => {
                    // consume this character (as part of var name)
                }
            }
        }

        if name.is_empty() {
            anyhow::bail!("environment variable reference has invalid empty name");
        }
        // We now have the environment variable name, and have parsed the end of the
        // name reference.
        match (query(name)?, default_value) {
            (Some(value), _) => result.push_str(&value),
            (None, Some(value)) => result.push_str(value),
            (None, None) => anyhow::bail!(format!(
                "environment variable '{}' is not set and has no default value",
                name
            )),
        }
        rest = chars.as_str();
    }
    result.push_str(rest);
    Ok(Cow::Owned(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let query = |name: &str| {
            Ok(Some(
                match name {
                    "FOO" => "/foo",
                    "BAR" => "/bar",
                    "FOO(ZAP)" => "/foo/zap",
                    "WINKING_FACE" => "\u{1F609}",
                    "\u{1F916}" => "ROBOT FACE",
                    _ => return Ok(None),
                }
                .to_string(),
            ))
        };

        let expand = |s| expand_vars_with(s, query);

        macro_rules! case {
            ($input:expr, $expected_output:expr) => {{
                let input = $input;
                let expected_output = $expected_output;
                match expand(input) {
                    Ok(output) => {
                        assert_eq!(output, expected_output, "input = {:?}", input);
                    }
                    Err(e) => {
                        panic!(
                            "Expected string {:?} to expand successfully, but it failed: {:?}",
                            input, e
                        );
                    }
                }
            }};
        }

        macro_rules! err_case {
            ($input:expr, $expected_error:expr) => {{
                let input = $input;
                let expected_error = $expected_error;
                match expand(input) {
                    Ok(output) => {
                        panic!("Expected expansion of string {:?} to fail, but it succeeded with value {:?}", input, output);
                    }
                    Err(e) => {
                        let message = e.to_string();
                        assert_eq!(message, expected_error, "input = {:?}", input);
                    }
                }
            }}
        }

        // things without references should not change.
        case!("", "");
        case!("identity", "identity");

        // we require ${...} (braces), so we ignore $FOO.
        case!("$FOO/some_package", "$FOO/some_package");

        // make sure variable references at the beginning, middle, and end
        // of a string all work correctly.
        case!("${FOO}", "/foo");
        case!("${FOO} one", "/foo one");
        case!("one ${FOO}", "one /foo");
        case!("one ${FOO} two", "one /foo two");
        case!("one ${FOO} two ${BAR} three", "one /foo two /bar three");

        // variable names can contain most characters, except for '}' or '?'
        // (Windows sets "ProgramFiles(x86)", for example.)
        case!("${FOO(ZAP)}", "/foo/zap");

        // variable is set, and has a default (which goes unused)
        case!("${FOO?/default}", "/foo");

        // variable is not set, but does have default
        case!("${VAR_NOT_SET?/default}", "/default");

        // variable is not set and has no default
        err_case!(
            "${VAR_NOT_SET}",
            "environment variable 'VAR_NOT_SET' is not set and has no default value"
        );

        // environment variables with unicode values are ok
        case!("${WINKING_FACE}", "\u{1F609}");

        // strings with unicode in them are ok
        case!("\u{1F609}${FOO}", "\u{1F609}/foo");

        // environment variable names with unicode in them are ok
        case!("${\u{1F916}}", "ROBOT FACE");

        // default values with unicode in them are ok
        case!("${VAR_NOT_SET?\u{1F916}}", "\u{1F916}");

        // invalid names
        err_case!(
            "${}",
            "environment variable reference has invalid empty name"
        );
        err_case!(
            "${?default}",
            "environment variable reference has invalid empty name"
        );
        err_case!(
            "${",
            "environment variable reference is missing closing brace."
        );
        err_case!(
            "${FOO",
            "environment variable reference is missing closing brace."
        );
        err_case!(
            "${FOO?default",
            "environment variable reference is missing closing brace."
        );
    }
}
