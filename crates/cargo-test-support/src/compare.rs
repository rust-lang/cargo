//! Routines for comparing and diffing output.
//!
//! # Deprecated comparisons
//!
//! Cargo's tests are in transition from internal-only pattern and normalization routines used in
//! asserts like [`crate::Execs::with_stdout_contains`] to [`assert_e2e`] and [`assert_ui`].
//!
//! ## Patterns
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
//! ## Normalization
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

use crate::cross_compile::try_alternate;
use crate::paths;
use crate::{diff, rustc_host};
use anyhow::{bail, Result};
use std::fmt;
use std::path::Path;
use std::str;
use url::Url;

/// This makes it easier to write regex replacements that are guaranteed to only
/// get compiled once
macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

/// Assertion policy for UI tests
///
/// This emphasizes showing as much content as possible at the cost of more brittleness
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
///
/// # Example
///
/// ```no_run
/// # use cargo_test_support::compare::assert_e2e;
/// # use cargo_test_support::file;
/// # let p = cargo_test_support::project().build();
/// # let stdout = "";
/// assert_e2e().eq(stdout, file!["stderr.term.svg"]);
/// ```
/// ```console
/// $ SNAPSHOTS=overwrite cargo test
/// ```
pub fn assert_ui() -> snapbox::Assert {
    let mut subs = snapbox::Redactions::new();
    subs.extend(MIN_LITERAL_REDACTIONS.into_iter().cloned())
        .unwrap();
    add_test_support_redactions(&mut subs);
    add_regex_redactions(&mut subs);

    snapbox::Assert::new()
        .action_env(snapbox::assert::DEFAULT_ACTION_ENV)
        .redact_with(subs)
}

/// Assertion policy for functional end-to-end tests
///
/// This emphasizes showing as much content as possible at the cost of more brittleness
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
///
/// # Example
///
/// ```no_run
/// # use cargo_test_support::compare::assert_e2e;
/// # use cargo_test_support::str;
/// # let p = cargo_test_support::project().build();
/// assert_e2e().eq(p.read_lockfile(), str![]);
/// ```
/// ```console
/// $ SNAPSHOTS=overwrite cargo test
/// ```
pub fn assert_e2e() -> snapbox::Assert {
    let mut subs = snapbox::Redactions::new();
    subs.extend(MIN_LITERAL_REDACTIONS.into_iter().cloned())
        .unwrap();
    subs.extend(E2E_LITERAL_REDACTIONS.into_iter().cloned())
        .unwrap();
    add_test_support_redactions(&mut subs);
    add_regex_redactions(&mut subs);

    snapbox::Assert::new()
        .action_env(snapbox::assert::DEFAULT_ACTION_ENV)
        .redact_with(subs)
}

fn add_test_support_redactions(subs: &mut snapbox::Redactions) {
    let root = paths::root();
    // Use `from_file_path` instead of `from_dir_path` so the trailing slash is
    // put in the users output, rather than hidden in the variable
    let root_url = url::Url::from_file_path(&root).unwrap().to_string();

    subs.insert("[ROOT]", root).unwrap();
    subs.insert("[ROOTURL]", root_url).unwrap();
    subs.insert("[HOST_TARGET]", rustc_host()).unwrap();
    if let Some(alt_target) = try_alternate() {
        subs.insert("[ALT_TARGET]", alt_target).unwrap();
    }
}

fn add_regex_redactions(subs: &mut snapbox::Redactions) {
    // For e2e tests
    subs.insert(
        "[ELAPSED]",
        regex!(r"\[FINISHED\].*in (?<redacted>[0-9]+(\.[0-9]+)?(m [0-9]+)?)s"),
    )
    .unwrap();
    // for UI tests
    subs.insert(
        "[ELAPSED]",
        regex!(r"Finished.*in (?<redacted>[0-9]+(\.[0-9]+)?(m [0-9]+)?)s"),
    )
    .unwrap();
    // output from libtest
    subs.insert(
        "[ELAPSED]",
        regex!(r"; finished in (?<redacted>[0-9]+(\.[0-9]+)?(m [0-9]+)?)s"),
    )
    .unwrap();
    subs.insert(
        "[FILE_NUM]",
        regex!(r"\[(REMOVED|SUMMARY)\] (?<redacted>[1-9][0-9]*) files"),
    )
    .unwrap();
    subs.insert(
        "[FILE_SIZE]",
        regex!(r"(?<redacted>[0-9]+(\.[0-9]+)?([a-zA-Z]i)?)B\s"),
    )
    .unwrap();
    subs.insert(
        "[HASH]",
        regex!(r"home/\.cargo/registry/(cache|index|src)/-(?<redacted>[a-z0-9]+)"),
    )
    .unwrap();
    subs.insert(
        "[HASH]",
        regex!(r"\.cargo/target/(?<redacted>[0-9a-f]{2}/[0-9a-f]{14})"),
    )
    .unwrap();
    subs.insert("[HASH]", regex!(r"/[a-z0-9\-_]+-(?<redacted>[0-9a-f]{16})"))
        .unwrap();
    subs.insert(
        "[AVG_ELAPSED]",
        regex!(r"(?<redacted>[0-9]+(\.[0-9]+)?) ns/iter"),
    )
    .unwrap();
    subs.insert(
        "[JITTER]",
        regex!(r"ns/iter \(\+/- (?<redacted>[0-9]+(\.[0-9]+)?)\)"),
    )
    .unwrap();

    // Following 3 subs redact:
    //   "1719325877.527949100s, 61549498ns after last build at 1719325877.466399602s"
    //   "1719503592.218193216s, 1h 1s after last build at 1719499991.982681034s"
    // into "[DIRTY_REASON_NEW_TIME], [DIRTY_REASON_DIFF] after last build at [DIRTY_REASON_OLD_TIME]"
    subs.insert(
        "[TIME_DIFF_AFTER_LAST_BUILD]",
        regex!(r"(?<redacted>[0-9]+(\.[0-9]+)?s, (\s?[0-9]+(\.[0-9]+)?(s|ns|h))+ after last build at [0-9]+(\.[0-9]+)?s)"),
       )
       .unwrap();
}

static MIN_LITERAL_REDACTIONS: &[(&str, &str)] = &[
    ("[EXE]", std::env::consts::EXE_SUFFIX),
    ("[BROKEN_PIPE]", "Broken pipe (os error 32)"),
    ("[BROKEN_PIPE]", "The pipe is being closed. (os error 232)"),
    // Unix message for an entity was not found
    ("[NOT_FOUND]", "No such file or directory (os error 2)"),
    // Windows message for an entity was not found
    (
        "[NOT_FOUND]",
        "The system cannot find the file specified. (os error 2)",
    ),
    (
        "[NOT_FOUND]",
        "The system cannot find the path specified. (os error 3)",
    ),
    ("[NOT_FOUND]", "Access is denied. (os error 5)"),
    ("[NOT_FOUND]", "program not found"),
    // Unix message for exit status
    ("[EXIT_STATUS]", "exit status"),
    // Windows message for exit status
    ("[EXIT_STATUS]", "exit code"),
];
static E2E_LITERAL_REDACTIONS: &[(&str, &str)] = &[
    ("[RUNNING]", "     Running"),
    ("[COMPILING]", "   Compiling"),
    ("[CHECKING]", "    Checking"),
    ("[COMPLETED]", "   Completed"),
    ("[CREATED]", "     Created"),
    ("[CREATING]", "    Creating"),
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
    ("[LOCKING]", "     Locking"),
    ("[UPDATING]", "    Updating"),
    ("[UPGRADING]", "   Upgrading"),
    ("[ADDING]", "      Adding"),
    ("[REMOVING]", "    Removing"),
    ("[REMOVED]", "     Removed"),
    ("[UNCHANGED]", "   Unchanged"),
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
    ("[GENERATED]", "   Generated"),
    ("[OPENING]", "     Opening"),
];

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
    let mut result = input.to_owned();
    for &(pat, subst) in MIN_LITERAL_REDACTIONS {
        result = result.replace(pat, subst)
    }
    for &(pat, subst) in E2E_LITERAL_REDACTIONS {
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
pub(crate) fn match_exact(
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
pub(crate) fn assert_match_exact(expected: &str, actual: &str) {
    if let Err(e) = match_exact(expected, actual, "", "", None) {
        crate::panic_error("", e);
    }
}

/// Checks that the given string contains the given lines, ignoring the order
/// of the lines.
///
/// See [Patterns](index.html#patterns) for more information on pattern matching.
pub(crate) fn match_unordered(expected: &str, actual: &str, cwd: Option<&Path>) -> Result<()> {
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
pub(crate) fn match_contains(expected: &str, actual: &str, cwd: Option<&Path>) -> Result<()> {
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
pub(crate) fn match_does_not_contain(
    expected: &str,
    actual: &str,
    cwd: Option<&Path>,
) -> Result<()> {
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
pub(crate) fn match_contains_n(
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
pub(crate) fn match_with_without(
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

/// A single line string that supports `[..]` wildcard matching.
pub(crate) struct WildStr<'a> {
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

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::prelude::*;
    use snapbox::str;

    use super::*;

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

    #[test]
    fn redact_elapsed_time() {
        let mut subs = snapbox::Redactions::new();
        add_regex_redactions(&mut subs);

        assert_data_eq!(
            subs.redact("[FINISHED] `release` profile [optimized] target(s) in 5.5s"),
            str!["[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s"].raw()
        );
        assert_data_eq!(
            subs.redact("[FINISHED] `release` profile [optimized] target(s) in 1m 05s"),
            str!["[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s"].raw()
        );
    }
}
