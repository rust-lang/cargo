//! Routines for comparing and diffing output.
//!
//! # Patterns
//!
//! Many of these functions support special markup to assist with comparing
//! text that may vary or is otherwise uninteresting for the test at hand. The
//! supported patterns are:
//!
//! - `[..]` is a wildcard that matches 0 or more characters on the same line
//!   (similar to `.*` in a regex). It is non-greedy.
//! - `[EXE]` optionally adds `.exe` on Windows (empty string on other
//!   platforms).
//! - `[ROOT]` is the path to the test directory's root.
//! - `[CWD]` is the working directory of the process that was run.
//! - There is a wide range of substitutions (such as `[COMPILING]` or
//!   `[WARNING]`) to match cargo's "status" output and allows you to ignore
//!   the alignment. See the source of `substitute_macros` for a complete list
//!   of substitutions.
//!
//! # Normalization
//!
//! In addition to the patterns described above, the strings are normalized
//! in such a way to avoid unwanted differences. The normalizations are:
//!
//! - Raw tab characters are converted to the string `<tab>`. This is helpful
//!   so that raw tabs do not need to be written in the expected string, and
//!   to avoid confusion of tabs vs spaces.
//! - Backslashes are converted to forward slashes to deal with Windows paths.
//!   This helps so that all tests can be written assuming forward slashes.
//!   Other heuristics are applied to try to ensure Windows-style paths aren't
//!   a problem.
//! - Carriage returns are removed, which can help when running on Windows.

use crate::paths;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::env;
use std::path::Path;
use std::str;
use url::Url;

/// Normalizes the output so that it can be compared against the expected value.
fn normalize_actual(actual: &str, cwd: Option<&Path>) -> String {
    // It's easier to read tabs in outputs if they don't show up as literal
    // hidden characters
    let actual = actual.replace('\t', "<tab>");
    // Let's not deal with \r\n vs \n on windows...
    let actual = actual.replace('\r', "");
    normalize_common(&actual, cwd)
}

/// Normalizes the expected string so that it can be compared against the actual output.
fn normalize_expected(expected: &str, cwd: Option<&Path>) -> String {
    let expected = substitute_macros(expected);
    normalize_common(&expected, cwd)
}

/// Normalizes text for both actual and expected strings.
fn normalize_common(text: &str, cwd: Option<&Path>) -> String {
    // Let's not deal with / vs \ (windows...)
    // First replace backslash-escaped backslashes with forward slashes
    // which can occur in, for example, JSON output
    let text = text.replace("\\\\", "/").replace('\\', "/");

    // Weirdness for paths on Windows extends beyond `/` vs `\` apparently.
    // Namely paths like `c:\` and `C:\` are equivalent and that can cause
    // issues. The return value of `env::current_dir()` may return a
    // lowercase drive name, but we round-trip a lot of values through `Url`
    // which will auto-uppercase the drive name. To just ignore this
    // distinction we try to canonicalize as much as possible, taking all
    // forms of a path and canonicalizing them to one.
    let replace_path = |s: &str, path: &Path, with: &str| {
        let path_through_url = Url::from_file_path(path).unwrap().to_file_path().unwrap();
        let path1 = path.display().to_string().replace('\\', "/");
        let path2 = path_through_url.display().to_string().replace('\\', "/");
        s.replace(&path1, with)
            .replace(&path2, with)
            .replace(with, &path1)
    };

    let text = match cwd {
        None => text,
        Some(p) => replace_path(&text, p, "[CWD]"),
    };

    // Similar to cwd above, perform similar treatment to the root path
    // which in theory all of our paths should otherwise get rooted at.
    let root = paths::root();
    let text = replace_path(&text, &root, "[ROOT]");

    text
}

fn substitute_macros(input: &str) -> String {
    let macros = [
        ("[RUNNING]", "     Running"),
        ("[COMPILING]", "   Compiling"),
        ("[CHECKING]", "    Checking"),
        ("[COMPLETED]", "   Completed"),
        ("[CREATED]", "     Created"),
        ("[FINISHED]", "    Finished"),
        ("[ERROR]", "error:"),
        ("[WARNING]", "warning:"),
        ("[NOTE]", "note:"),
        ("[HELP]", "help:"),
        ("[DOCUMENTING]", " Documenting"),
        ("[FRESH]", "       Fresh"),
        ("[UPDATING]", "    Updating"),
        ("[ADDING]", "      Adding"),
        ("[REMOVING]", "    Removing"),
        ("[DOCTEST]", "   Doc-tests"),
        ("[PACKAGING]", "   Packaging"),
        ("[DOWNLOADING]", " Downloading"),
        ("[DOWNLOADED]", "  Downloaded"),
        ("[UPLOADING]", "   Uploading"),
        ("[VERIFYING]", "   Verifying"),
        ("[ARCHIVING]", "   Archiving"),
        ("[INSTALLING]", "  Installing"),
        ("[REPLACING]", "   Replacing"),
        ("[UNPACKING]", "   Unpacking"),
        ("[SUMMARY]", "     Summary"),
        ("[FIXED]", "       Fixed"),
        ("[FIXING]", "      Fixing"),
        ("[EXE]", env::consts::EXE_SUFFIX),
        ("[IGNORED]", "     Ignored"),
        ("[INSTALLED]", "   Installed"),
        ("[REPLACED]", "    Replaced"),
        ("[BUILDING]", "    Building"),
        ("[LOGIN]", "       Login"),
        ("[LOGOUT]", "      Logout"),
        ("[YANK]", "        Yank"),
        ("[OWNER]", "       Owner"),
        ("[MIGRATING]", "   Migrating"),
    ];
    let mut result = input.to_owned();
    for &(pat, subst) in &macros {
        result = result.replace(pat, subst)
    }
    result
}

/// Compares one string against another, checking that they both match.
///
/// See [Patterns](index.html#patterns) for more information on pattern matching.
///
/// - `description` explains where the output is from (usually "stdout" or "stderr").
/// - `other_output` is other output to display in the error (usually stdout or stderr).
pub fn match_exact(
    expected: &str,
    actual: &str,
    description: &str,
    other_output: &str,
    cwd: Option<&Path>,
) -> Result<()> {
    let expected = normalize_expected(expected, cwd);
    let actual = normalize_actual(actual, cwd);
    let e = expected.lines();
    let a = actual.lines();

    let diffs = diff_lines(a, e, false);
    if diffs.is_empty() {
        Ok(())
    } else {
        bail!(
            "{} did not match:\n\
             {}\n\n\
             other output:\n\
             `{}`",
            description,
            diffs.join("\n"),
            other_output,
        )
    }
}

/// Checks that the given string contains the given lines, ignoring the order
/// of the lines.
///
/// See [Patterns](index.html#patterns) for more information on pattern matching.
pub fn match_unordered(expected: &str, actual: &str, cwd: Option<&Path>) -> Result<()> {
    let expected = normalize_expected(expected, cwd);
    let actual = normalize_actual(actual, cwd);
    let mut a = actual.lines().collect::<Vec<_>>();
    // match more-constrained lines first, although in theory we'll
    // need some sort of recursive match here. This handles the case
    // that you expect "a\n[..]b" and two lines are printed out,
    // "ab\n"a", where technically we do match unordered but a naive
    // search fails to find this. This simple sort at least gets the
    // test suite to pass for now, but we may need to get more fancy
    // if tests start failing again.
    a.sort_by_key(|s| s.len());
    let mut failures = Vec::new();

    for e_line in expected.lines() {
        match a.iter().position(|a_line| lines_match(e_line, a_line)) {
            Some(index) => {
                a.remove(index);
            }
            None => failures.push(e_line),
        }
    }
    if !failures.is_empty() {
        bail!(
            "Did not find expected line(s):\n{}\n\
             Remaining available output:\n{}\n",
            failures.join("\n"),
            a.join("\n")
        );
    }
    if !a.is_empty() {
        bail!(
            "Output included extra lines:\n\
             {}\n",
            a.join("\n")
        )
    } else {
        Ok(())
    }
}

/// Checks that the given string contains the given contiguous lines
/// somewhere.
///
/// See [Patterns](index.html#patterns) for more information on pattern matching.
pub fn match_contains(expected: &str, actual: &str, cwd: Option<&Path>) -> Result<()> {
    let expected = normalize_expected(expected, cwd);
    let actual = normalize_actual(actual, cwd);
    let e = expected.lines();
    let mut a = actual.lines();

    let mut diffs = diff_lines(a.clone(), e.clone(), true);
    while a.next().is_some() {
        let a = diff_lines(a.clone(), e.clone(), true);
        if a.len() < diffs.len() {
            diffs = a;
        }
    }
    if diffs.is_empty() {
        Ok(())
    } else {
        bail!(
            "expected to find:\n\
             {}\n\n\
             did not find in output:\n\
             {}",
            expected,
            actual
        )
    }
}

/// Checks that the given string does not contain the given contiguous lines
/// anywhere.
///
/// See [Patterns](index.html#patterns) for more information on pattern matching.
pub fn match_does_not_contain(expected: &str, actual: &str, cwd: Option<&Path>) -> Result<()> {
    if match_contains(expected, actual, cwd).is_ok() {
        bail!(
            "expected not to find:\n\
             {}\n\n\
             but found in output:\n\
             {}",
            expected,
            actual
        );
    } else {
        Ok(())
    }
}

/// Checks that the given string contains the given contiguous lines
/// somewhere, and should be repeated `number` times.
///
/// See [Patterns](index.html#patterns) for more information on pattern matching.
pub fn match_contains_n(
    expected: &str,
    number: usize,
    actual: &str,
    cwd: Option<&Path>,
) -> Result<()> {
    let expected = normalize_expected(expected, cwd);
    let actual = normalize_actual(actual, cwd);
    let e = expected.lines();
    let mut a = actual.lines();

    let mut matches = 0;

    while let Some(..) = {
        if diff_lines(a.clone(), e.clone(), true).is_empty() {
            matches += 1;
        }
        a.next()
    } {}

    if matches == number {
        Ok(())
    } else {
        bail!(
            "expected to find {} occurrences:\n\
             {}\n\n\
             did not find in output:\n\
             {}",
            number,
            expected,
            actual
        )
    }
}

/// Checks that the given string has a line that contains the given patterns,
/// and that line also does not contain the `without` patterns.
///
/// See [Patterns](index.html#patterns) for more information on pattern matching.
///
/// See [`crate::Execs::with_stderr_line_without`] for an example and cautions
/// against using.
pub fn match_with_without(
    actual: &str,
    with: &[String],
    without: &[String],
    cwd: Option<&Path>,
) -> Result<()> {
    let actual = normalize_actual(actual, cwd);
    let contains = |s, line| {
        let mut s = normalize_expected(s, cwd);
        s.insert_str(0, "[..]");
        s.push_str("[..]");
        lines_match(&s, line)
    };
    let matches: Vec<&str> = actual
        .lines()
        .filter(|line| with.iter().all(|with| contains(with, line)))
        .filter(|line| !without.iter().any(|without| contains(without, line)))
        .collect();
    match matches.len() {
        0 => bail!(
            "Could not find expected line in output.\n\
             With contents: {:?}\n\
             Without contents: {:?}\n\
             Actual stderr:\n\
             {}\n",
            with,
            without,
            actual
        ),
        1 => Ok(()),
        _ => bail!(
            "Found multiple matching lines, but only expected one.\n\
             With contents: {:?}\n\
             Without contents: {:?}\n\
             Matching lines:\n\
             {}\n",
            with,
            without,
            matches.join("\n")
        ),
    }
}

/// Checks that the given string of JSON objects match the given set of
/// expected JSON objects.
///
/// See [`crate::Execs::with_json`] for more details.
pub fn match_json(expected: &str, actual: &str, cwd: Option<&Path>) -> Result<()> {
    let (exp_objs, act_objs) = collect_json_objects(expected, actual)?;
    if exp_objs.len() != act_objs.len() {
        bail!(
            "expected {} json lines, got {}, stdout:\n{}",
            exp_objs.len(),
            act_objs.len(),
            actual
        );
    }
    for (exp_obj, act_obj) in exp_objs.iter().zip(act_objs) {
        find_json_mismatch(exp_obj, &act_obj, cwd)?;
    }
    Ok(())
}

/// Checks that the given string of JSON objects match the given set of
/// expected JSON objects, ignoring their order.
///
/// See [`crate::Execs::with_json_contains_unordered`] for more details and
/// cautions when using.
pub fn match_json_contains_unordered(
    expected: &str,
    actual: &str,
    cwd: Option<&Path>,
) -> Result<()> {
    let (exp_objs, mut act_objs) = collect_json_objects(expected, actual)?;
    for exp_obj in exp_objs {
        match act_objs
            .iter()
            .position(|act_obj| find_json_mismatch(&exp_obj, act_obj, cwd).is_ok())
        {
            Some(index) => act_objs.remove(index),
            None => {
                bail!(
                    "Did not find expected JSON:\n\
                     {}\n\
                     Remaining available output:\n\
                     {}\n",
                    serde_json::to_string_pretty(&exp_obj).unwrap(),
                    itertools::join(
                        act_objs.iter().map(|o| serde_json::to_string(o).unwrap()),
                        "\n"
                    )
                );
            }
        };
    }
    Ok(())
}

fn collect_json_objects(
    expected: &str,
    actual: &str,
) -> Result<(Vec<serde_json::Value>, Vec<serde_json::Value>)> {
    let expected_objs: Vec<_> = expected
        .split("\n\n")
        .map(|expect| {
            expect
                .parse()
                .with_context(|| format!("failed to parse expected JSON object:\n{}", expect))
        })
        .collect::<Result<_>>()?;
    let actual_objs: Vec<_> = actual
        .lines()
        .filter(|line| line.starts_with('{'))
        .map(|line| {
            line.parse()
                .with_context(|| format!("failed to parse JSON object:\n{}", line))
        })
        .collect::<Result<_>>()?;
    Ok((expected_objs, actual_objs))
}

fn diff_lines<'a>(actual: str::Lines<'a>, expected: str::Lines<'a>, partial: bool) -> Vec<String> {
    let actual = actual.take(if partial {
        expected.clone().count()
    } else {
        usize::MAX
    });
    zip_all(actual, expected)
        .enumerate()
        .filter_map(|(i, (a, e))| match (a, e) {
            (Some(a), Some(e)) => {
                if lines_match(e, a) {
                    None
                } else {
                    Some(format!("{:3} - |{}|\n    + |{}|\n", i, e, a))
                }
            }
            (Some(a), None) => Some(format!("{:3} -\n    + |{}|\n", i, a)),
            (None, Some(e)) => Some(format!("{:3} - |{}|\n    +\n", i, e)),
            (None, None) => unreachable!(),
        })
        .collect()
}

struct ZipAll<I1: Iterator, I2: Iterator> {
    first: I1,
    second: I2,
}

impl<T, I1: Iterator<Item = T>, I2: Iterator<Item = T>> Iterator for ZipAll<I1, I2> {
    type Item = (Option<T>, Option<T>);
    fn next(&mut self) -> Option<(Option<T>, Option<T>)> {
        let first = self.first.next();
        let second = self.second.next();

        match (first, second) {
            (None, None) => None,
            (a, b) => Some((a, b)),
        }
    }
}

/// Returns an iterator, similar to `zip`, but exhausts both iterators.
///
/// Each element is `(Option<T>, Option<T>)` where `None` indicates an
/// iterator ended early.
fn zip_all<T, I1: Iterator<Item = T>, I2: Iterator<Item = T>>(a: I1, b: I2) -> ZipAll<I1, I2> {
    ZipAll {
        first: a,
        second: b,
    }
}

/// Compares a line with an expected pattern.
/// - Use `[..]` as a wildcard to match 0 or more characters on the same line
///   (similar to `.*` in a regex). It is non-greedy.
/// - Use `[EXE]` to optionally add `.exe` on Windows (empty string on other
///   platforms).
/// - There is a wide range of macros (such as `[COMPILING]` or `[WARNING]`)
///   to match cargo's "status" output and allows you to ignore the alignment.
///   See `substitute_macros` for a complete list of macros.
/// - `[ROOT]` the path to the test directory's root
/// - `[CWD]` is the working directory of the process that was run.
pub fn lines_match(expected: &str, mut actual: &str) -> bool {
    for (i, part) in expected.split("[..]").enumerate() {
        match actual.find(part) {
            Some(j) => {
                if i == 0 && j != 0 {
                    return false;
                }
                actual = &actual[j + part.len()..];
            }
            None => return false,
        }
    }
    actual.is_empty() || expected.ends_with("[..]")
}

/// Compares JSON object for approximate equality.
/// You can use `[..]` wildcard in strings (useful for OS-dependent things such
/// as paths). You can use a `"{...}"` string literal as a wildcard for
/// arbitrary nested JSON (useful for parts of object emitted by other programs
/// (e.g., rustc) rather than Cargo itself).
pub fn find_json_mismatch(expected: &Value, actual: &Value, cwd: Option<&Path>) -> Result<()> {
    match find_json_mismatch_r(expected, actual, cwd) {
        Some((expected_part, actual_part)) => bail!(
            "JSON mismatch\nExpected:\n{}\nWas:\n{}\nExpected part:\n{}\nActual part:\n{}\n",
            serde_json::to_string_pretty(expected).unwrap(),
            serde_json::to_string_pretty(&actual).unwrap(),
            serde_json::to_string_pretty(expected_part).unwrap(),
            serde_json::to_string_pretty(actual_part).unwrap(),
        ),
        None => Ok(()),
    }
}

fn find_json_mismatch_r<'a>(
    expected: &'a Value,
    actual: &'a Value,
    cwd: Option<&Path>,
) -> Option<(&'a Value, &'a Value)> {
    use serde_json::Value::*;
    match (expected, actual) {
        (&Number(ref l), &Number(ref r)) if l == r => None,
        (&Bool(l), &Bool(r)) if l == r => None,
        (&String(ref l), _) if l == "{...}" => None,
        (&String(ref l), &String(ref r)) => {
            let l = normalize_expected(l, cwd);
            let r = normalize_actual(r, cwd);
            if lines_match(&l, &r) {
                None
            } else {
                Some((expected, actual))
            }
        }
        (&Array(ref l), &Array(ref r)) => {
            if l.len() != r.len() {
                return Some((expected, actual));
            }

            l.iter()
                .zip(r.iter())
                .filter_map(|(l, r)| find_json_mismatch_r(l, r, cwd))
                .next()
        }
        (&Object(ref l), &Object(ref r)) => {
            let same_keys = l.len() == r.len() && l.keys().all(|k| r.contains_key(k));
            if !same_keys {
                return Some((expected, actual));
            }

            l.values()
                .zip(r.values())
                .filter_map(|(l, r)| find_json_mismatch_r(l, r, cwd))
                .next()
        }
        (&Null, &Null) => None,
        // Magic string literal `"{...}"` acts as wildcard for any sub-JSON.
        _ => Some((expected, actual)),
    }
}

#[test]
fn lines_match_works() {
    assert!(lines_match("a b", "a b"));
    assert!(lines_match("a[..]b", "a b"));
    assert!(lines_match("a[..]", "a b"));
    assert!(lines_match("[..]", "a b"));
    assert!(lines_match("[..]b", "a b"));

    assert!(!lines_match("[..]b", "c"));
    assert!(!lines_match("b", "c"));
    assert!(!lines_match("b", "cb"));
}
