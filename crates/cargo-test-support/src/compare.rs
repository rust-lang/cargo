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
//! - `[DIRTY-MSVC]` (only when the line starts with it) would be replaced by
//!   `[DIRTY]` when `cfg(target_env = "msvc")` or the line will be ignored otherwise.
//!   Tests that work around [issue 7358](https://github.com/rust-lang/cargo/issues/7358)
//!   can use this to avoid duplicating the `with_stderr` call like:
//!   `if cfg!(target_env = "msvc") {e.with_stderr("...[DIRTY]...");} else {e.with_stderr("...");}`.
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

use crate::diff;
use crate::paths;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::env;
use std::fmt;
use std::path::Path;
use std::str;
use url::Url;

/// Default `snapbox` Assertions
///
/// # Snapshots
///
/// Updating of snapshots is controlled with the `SNAPSHOTS` environment variable:
///
/// - `skip`: do not run the tests
/// - `ignore`: run the tests but ignore their failure
/// - `verify`: run the tests
/// - `overwrite`: update the snapshots based on the output of the tests
///
/// # Patterns
///
/// - `[..]` is a character wildcard, stopping at line breaks
/// - `\n...\n` is a multi-line wildcard
/// - `[EXE]` matches the exe suffix for the current platform
/// - `[ROOT]` matches [`paths::root()`][crate::paths::root]
/// - `[ROOTURL]` matches [`paths::root()`][crate::paths::root] as a URL
///
/// # Normalization
///
/// In addition to the patterns described above, text is normalized
/// in such a way to avoid unwanted differences. The normalizations are:
///
/// - Backslashes are converted to forward slashes to deal with Windows paths.
///   This helps so that all tests can be written assuming forward slashes.
///   Other heuristics are applied to try to ensure Windows-style paths aren't
///   a problem.
/// - Carriage returns are removed, which can help when running on Windows.
pub fn assert_ui() -> snapbox::Assert {
    let root = paths::root();
    // Use `from_file_path` instead of `from_dir_path` so the trailing slash is
    // put in the users output, rather than hidden in the variable
    let root_url = url::Url::from_file_path(&root).unwrap().to_string();
    let root = root.display().to_string();

    let mut subs = snapbox::Substitutions::new();
    subs.extend([
        (
            "[EXE]",
            std::borrow::Cow::Borrowed(std::env::consts::EXE_SUFFIX),
        ),
        ("[ROOT]", std::borrow::Cow::Owned(root)),
        ("[ROOTURL]", std::borrow::Cow::Owned(root_url)),
    ])
    .unwrap();
    snapbox::Assert::new()
        .action_env(snapbox::DEFAULT_ACTION_ENV)
        .substitutions(subs)
}

/// Normalizes the output so that it can be compared against the expected value.
fn normalize_actual(actual: &str, cwd: Option<&Path>) -> String {
    // It's easier to read tabs in outputs if they don't show up as literal
    // hidden characters
    let actual = actual.replace('\t', "<tab>");
    if cfg!(windows) {
        // Let's not deal with \r\n vs \n on windows...
        let actual = actual.replace('\r', "");
        normalize_windows(&actual, cwd)
    } else {
        actual
    }
}

/// Normalizes the expected string so that it can be compared against the actual output.
fn normalize_expected(expected: &str, cwd: Option<&Path>) -> String {
    let expected = replace_dirty_msvc(expected);
    let expected = substitute_macros(&expected);

    if cfg!(windows) {
        normalize_windows(&expected, cwd)
    } else {
        let expected = match cwd {
            None => expected,
            Some(cwd) => expected.replace("[CWD]", &cwd.display().to_string()),
        };
        let expected = expected.replace("[ROOT]", &paths::root().display().to_string());
        expected
    }
}

fn replace_dirty_msvc_impl(s: &str, is_msvc: bool) -> String {
    if is_msvc {
        s.replace("[DIRTY-MSVC]", "[DIRTY]")
    } else {
        use itertools::Itertools;

        let mut new = s
            .lines()
            .filter(|it| !it.starts_with("[DIRTY-MSVC]"))
            .join("\n");

        if s.ends_with("\n") {
            new.push_str("\n");
        }

        new
    }
}

fn replace_dirty_msvc(s: &str) -> String {
    replace_dirty_msvc_impl(s, cfg!(target_env = "msvc"))
}

/// Normalizes text for both actual and expected strings on Windows.
fn normalize_windows(text: &str, cwd: Option<&Path>) -> String {
    // Let's not deal with / vs \ (windows...)
    let text = text.replace('\\', "/");

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
        ("[CREDENTIAL]", "  Credential"),
        ("[DOWNGRADING]", " Downgrading"),
        ("[FINISHED]", "    Finished"),
        ("[ERROR]", "error:"),
        ("[WARNING]", "warning:"),
        ("[NOTE]", "note:"),
        ("[HELP]", "help:"),
        ("[DOCUMENTING]", " Documenting"),
        ("[SCRAPING]", "    Scraping"),
        ("[FRESH]", "       Fresh"),
        ("[DIRTY]", "       Dirty"),
        ("[UPDATING]", "    Updating"),
        ("[ADDING]", "      Adding"),
        ("[REMOVING]", "    Removing"),
        ("[REMOVED]", "     Removed"),
        ("[DOCTEST]", "   Doc-tests"),
        ("[PACKAGING]", "   Packaging"),
        ("[PACKAGED]", "    Packaged"),
        ("[DOWNLOADING]", " Downloading"),
        ("[DOWNLOADED]", "  Downloaded"),
        ("[UPLOADING]", "   Uploading"),
        ("[UPLOADED]", "    Uploaded"),
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
        ("[EXECUTABLE]", "  Executable"),
        ("[SKIPPING]", "    Skipping"),
        ("[WAITING]", "     Waiting"),
        ("[PUBLISHED]", "   Published"),
        ("[BLOCKING]", "    Blocking"),
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
    let e: Vec<_> = expected.lines().map(WildStr::new).collect();
    let a: Vec<_> = actual.lines().map(WildStr::new).collect();
    if e == a {
        return Ok(());
    }
    let diff = diff::colored_diff(&e, &a);
    bail!(
        "{} did not match:\n\
         {}\n\n\
         other output:\n\
         {}\n",
        description,
        diff,
        other_output,
    );
}

/// Convenience wrapper around [`match_exact`] which will panic on error.
#[track_caller]
pub fn assert_match_exact(expected: &str, actual: &str) {
    if let Err(e) = match_exact(expected, actual, "", "", None) {
        crate::panic_error("", e);
    }
}

/// Checks that the given string contains the given lines, ignoring the order
/// of the lines.
///
/// See [Patterns](index.html#patterns) for more information on pattern matching.
pub fn match_unordered(expected: &str, actual: &str, cwd: Option<&Path>) -> Result<()> {
    let expected = normalize_expected(expected, cwd);
    let actual = normalize_actual(actual, cwd);
    let e: Vec<_> = expected.lines().map(|line| WildStr::new(line)).collect();
    let mut a: Vec<_> = actual.lines().map(|line| WildStr::new(line)).collect();
    // match more-constrained lines first, although in theory we'll
    // need some sort of recursive match here. This handles the case
    // that you expect "a\n[..]b" and two lines are printed out,
    // "ab\n"a", where technically we do match unordered but a naive
    // search fails to find this. This simple sort at least gets the
    // test suite to pass for now, but we may need to get more fancy
    // if tests start failing again.
    a.sort_by_key(|s| s.line.len());
    let mut changes = Vec::new();
    let mut a_index = 0;
    let mut failure = false;

    use crate::diff::Change;
    for (e_i, e_line) in e.into_iter().enumerate() {
        match a.iter().position(|a_line| e_line == *a_line) {
            Some(index) => {
                let a_line = a.remove(index);
                changes.push(Change::Keep(e_i, index, a_line));
                a_index += 1;
            }
            None => {
                failure = true;
                changes.push(Change::Remove(e_i, e_line));
            }
        }
    }
    for unmatched in a {
        failure = true;
        changes.push(Change::Add(a_index, unmatched));
        a_index += 1;
    }
    if failure {
        bail!(
            "Expected lines did not match (ignoring order):\n{}\n",
            diff::render_colored_changes(&changes)
        );
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
    let e: Vec<_> = expected.lines().map(|line| WildStr::new(line)).collect();
    let a: Vec<_> = actual.lines().map(|line| WildStr::new(line)).collect();
    if e.len() == 0 {
        bail!("expected length must not be zero");
    }
    for window in a.windows(e.len()) {
        if window == e {
            return Ok(());
        }
    }
    bail!(
        "expected to find:\n\
         {}\n\n\
         did not find in output:\n\
         {}",
        expected,
        actual
    );
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
    let e: Vec<_> = expected.lines().map(|line| WildStr::new(line)).collect();
    let a: Vec<_> = actual.lines().map(|line| WildStr::new(line)).collect();
    if e.len() == 0 {
        bail!("expected length must not be zero");
    }
    let matches = a.windows(e.len()).filter(|window| *window == e).count();
    if matches == number {
        Ok(())
    } else {
        bail!(
            "expected to find {} occurrences of:\n\
             {}\n\n\
             but found {} matches in the output:\n\
             {}",
            number,
            expected,
            matches,
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
    let norm = |s: &String| format!("[..]{}[..]", normalize_expected(s, cwd));
    let with: Vec<_> = with.iter().map(norm).collect();
    let without: Vec<_> = without.iter().map(norm).collect();
    let with_wild: Vec<_> = with.iter().map(|w| WildStr::new(w)).collect();
    let without_wild: Vec<_> = without.iter().map(|w| WildStr::new(w)).collect();

    let matches: Vec<_> = actual
        .lines()
        .map(WildStr::new)
        .filter(|line| with_wild.iter().all(|with| with == line))
        .filter(|line| !without_wild.iter().any(|without| without == line))
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
            itertools::join(matches, "\n")
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
            if match_exact(l, r, "", "", cwd).is_err() {
                Some((expected, actual))
            } else {
                None
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

/// A single line string that supports `[..]` wildcard matching.
pub struct WildStr<'a> {
    has_meta: bool,
    line: &'a str,
}

impl<'a> WildStr<'a> {
    pub fn new(line: &'a str) -> WildStr<'a> {
        WildStr {
            has_meta: line.contains("[..]"),
            line,
        }
    }
}

impl<'a> PartialEq for WildStr<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self.has_meta, other.has_meta) {
            (false, false) => self.line == other.line,
            (true, false) => meta_cmp(self.line, other.line),
            (false, true) => meta_cmp(other.line, self.line),
            (true, true) => panic!("both lines cannot have [..]"),
        }
    }
}

fn meta_cmp(a: &str, mut b: &str) -> bool {
    for (i, part) in a.split("[..]").enumerate() {
        match b.find(part) {
            Some(j) => {
                if i == 0 && j != 0 {
                    return false;
                }
                b = &b[j + part.len()..];
            }
            None => return false,
        }
    }
    b.is_empty() || a.ends_with("[..]")
}

impl fmt::Display for WildStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.line)
    }
}

impl fmt::Debug for WildStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.line)
    }
}

#[test]
fn wild_str_cmp() {
    for (a, b) in &[
        ("a b", "a b"),
        ("a[..]b", "a b"),
        ("a[..]", "a b"),
        ("[..]", "a b"),
        ("[..]b", "a b"),
    ] {
        assert_eq!(WildStr::new(a), WildStr::new(b));
    }
    for (a, b) in &[("[..]b", "c"), ("b", "c"), ("b", "cb")] {
        assert_ne!(WildStr::new(a), WildStr::new(b));
    }
}

#[test]
fn dirty_msvc() {
    let case = |expected: &str, wild: &str, msvc: bool| {
        assert_eq!(expected, &replace_dirty_msvc_impl(wild, msvc));
    };

    // no replacements
    case("aa", "aa", false);
    case("aa", "aa", true);

    // with replacements
    case(
        "\
[DIRTY] a",
        "\
[DIRTY-MSVC] a",
        true,
    );
    case(
        "",
        "\
[DIRTY-MSVC] a",
        false,
    );
    case(
        "\
[DIRTY] a
[COMPILING] a",
        "\
[DIRTY-MSVC] a
[COMPILING] a",
        true,
    );
    case(
        "\
[COMPILING] a",
        "\
[DIRTY-MSVC] a
[COMPILING] a",
        false,
    );

    // test trailing newline behavior
    case(
        "\
A
B
", "\
A
B
", true,
    );

    case(
        "\
A
B
", "\
A
B
", false,
    );

    case(
        "\
A
B", "\
A
B", true,
    );

    case(
        "\
A
B", "\
A
B", false,
    );

    case(
        "\
[DIRTY] a
",
        "\
[DIRTY-MSVC] a
",
        true,
    );
    case(
        "\n",
        "\
[DIRTY-MSVC] a
",
        false,
    );

    case(
        "\
[DIRTY] a",
        "\
[DIRTY-MSVC] a",
        true,
    );
    case(
        "",
        "\
[DIRTY-MSVC] a",
        false,
    );
}
