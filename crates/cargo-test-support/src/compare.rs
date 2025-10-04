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
use crate::rustc_host;
use anyhow::{Result, bail};
use snapbox::Data;
use snapbox::IntoData;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::str;

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
    // Match multi-part hashes like `06/b451d0d6f88b1d` used in directory paths
    subs.insert("[HASH]", regex!(r"/(?<redacted>[a-f0-9]{2}\/[0-9a-f]{14})"))
        .unwrap();
    // Match file name hashes like `foo-06b451d0d6f88b1d`
    subs.insert("[HASH]", regex!(r"[a-z0-9]+-(?<redacted>[a-f0-9]{16})"))
        .unwrap();
    // Match path hashes like `../06b451d0d6f88b1d/..` used in directory paths
    subs.insert("[HASH]", regex!(r"\/(?<redacted>[0-9a-f]{16})\/"))
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

/// Checks that the given string contains the given contiguous lines
/// somewhere.
///
/// See [Patterns](index.html#patterns) for more information on pattern matching.
pub(crate) fn match_contains(
    expected: &str,
    actual: &str,
    redactions: &snapbox::Redactions,
) -> Result<()> {
    let expected = normalize_expected(expected, redactions);
    let actual = normalize_actual(actual, redactions);
    let e: Vec<_> = expected.lines().map(|line| WildStr::new(line)).collect();
    let a: Vec<_> = actual.lines().collect();
    if e.len() == 0 {
        bail!("expected length must not be zero");
    }
    for window in a.windows(e.len()) {
        if e == window {
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
    redactions: &snapbox::Redactions,
) -> Result<()> {
    if match_contains(expected, actual, redactions).is_ok() {
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
    redactions: &snapbox::Redactions,
) -> Result<()> {
    let actual = normalize_actual(actual, redactions);
    let norm = |s: &String| format!("[..]{}[..]", normalize_expected(s, redactions));
    let with: Vec<_> = with.iter().map(norm).collect();
    let without: Vec<_> = without.iter().map(norm).collect();
    let with_wild: Vec<_> = with.iter().map(|w| WildStr::new(w)).collect();
    let without_wild: Vec<_> = without.iter().map(|w| WildStr::new(w)).collect();

    let matches: Vec<_> = actual
        .lines()
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

/// Normalizes the output so that it can be compared against the expected value.
fn normalize_actual(content: &str, redactions: &snapbox::Redactions) -> String {
    use snapbox::filter::Filter as _;
    let content = snapbox::filter::FilterPaths.filter(content.into_data());
    let content = snapbox::filter::FilterNewlines.filter(content);
    let content = content.render().expect("came in as a String");
    let content = redactions.redact(&content);
    content
}

/// Normalizes the expected string so that it can be compared against the actual output.
fn normalize_expected(content: &str, redactions: &snapbox::Redactions) -> String {
    use snapbox::filter::Filter as _;
    let content = snapbox::filter::FilterPaths.filter(content.into_data());
    let content = snapbox::filter::FilterNewlines.filter(content);
    // Remove any conditionally absent redactions like `[EXE]`
    let content = content.render().expect("came in as a String");
    let content = redactions.clear_unused(&content);
    content.into_owned()
}

/// A single line string that supports `[..]` wildcard matching.
struct WildStr<'a> {
    has_meta: bool,
    line: &'a str,
}

impl<'a> WildStr<'a> {
    fn new(line: &'a str) -> WildStr<'a> {
        WildStr {
            has_meta: line.contains("[..]"),
            line,
        }
    }
}

impl PartialEq<&str> for WildStr<'_> {
    fn eq(&self, other: &&str) -> bool {
        if self.has_meta {
            meta_cmp(self.line, other)
        } else {
            self.line == *other
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

pub struct InMemoryDir {
    files: Vec<(PathBuf, Data)>,
}

impl InMemoryDir {
    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.files.iter().map(|(p, _)| p.as_path())
    }

    #[track_caller]
    pub fn assert_contains(&self, expected: &Self) {
        use std::fmt::Write as _;
        let assert = assert_e2e();
        let mut errs = String::new();
        for (path, expected_data) in &expected.files {
            let actual_data = self
                .files
                .iter()
                .find_map(|(p, d)| (path == p).then(|| d.clone()))
                .unwrap_or_else(|| Data::new());
            if let Err(err) =
                assert.try_eq(Some(&path.display()), actual_data, expected_data.clone())
            {
                let _ = write!(&mut errs, "{err}");
            }
        }
        if !errs.is_empty() {
            panic!("{errs}")
        }
    }
}

impl<P, D> FromIterator<(P, D)> for InMemoryDir
where
    P: Into<std::path::PathBuf>,
    D: IntoData,
{
    fn from_iter<I: IntoIterator<Item = (P, D)>>(files: I) -> Self {
        let files = files
            .into_iter()
            .map(|(p, d)| (p.into(), d.into_data()))
            .collect();
        Self { files }
    }
}

impl<const N: usize, P, D> From<[(P, D); N]> for InMemoryDir
where
    P: Into<PathBuf>,
    D: IntoData,
{
    fn from(files: [(P, D); N]) -> Self {
        let files = files
            .into_iter()
            .map(|(p, d)| (p.into(), d.into_data()))
            .collect();
        Self { files }
    }
}

impl<P, D> From<std::collections::HashMap<P, D>> for InMemoryDir
where
    P: Into<PathBuf>,
    D: IntoData,
{
    fn from(files: std::collections::HashMap<P, D>) -> Self {
        let files = files
            .into_iter()
            .map(|(p, d)| (p.into(), d.into_data()))
            .collect();
        Self { files }
    }
}

impl<P, D> From<std::collections::BTreeMap<P, D>> for InMemoryDir
where
    P: Into<PathBuf>,
    D: IntoData,
{
    fn from(files: std::collections::BTreeMap<P, D>) -> Self {
        let files = files
            .into_iter()
            .map(|(p, d)| (p.into(), d.into_data()))
            .collect();
        Self { files }
    }
}

impl From<()> for InMemoryDir {
    fn from(_files: ()) -> Self {
        let files = Vec::new();
        Self { files }
    }
}

/// Create an `impl _ for InMemoryDir` for a generic tuple
///
/// Must pass in names for each tuple parameter for
/// - internal variable name
/// - `Path` type
/// - `Data` type
macro_rules! impl_from_tuple_for_inmemorydir {
    ($($var:ident $path:ident $data:ident),+) => {
        impl<$($path: Into<PathBuf>, $data: IntoData),+> From<($(($path, $data)),+ ,)> for InMemoryDir {
            fn from(files: ($(($path, $data)),+,)) -> Self {
                let ($($var),+ ,) = files;
                let files = [$(($var.0.into(), $var.1.into_data())),+];
                files.into()
            }
        }
    };
}

/// Extend `impl_from_tuple_for_inmemorydir` to generate for the specified tuple and all smaller
/// tuples
macro_rules! impl_from_tuples_for_inmemorydir {
    ($var1:ident $path1:ident $data1:ident, $($var:ident $path:ident $data:ident),+) => {
        impl_from_tuples_for_inmemorydir!(__impl $var1 $path1 $data1; $($var $path $data),+);
    };
    (__impl $($var:ident $path:ident $data:ident),+; $var1:ident $path1:ident $data1:ident $(,$var2:ident $path2:ident $data2:ident)*) => {
        impl_from_tuple_for_inmemorydir!($($var $path $data),+);
        impl_from_tuples_for_inmemorydir!(__impl $($var $path $data),+, $var1 $path1 $data1; $($var2 $path2 $data2),*);
    };
    (__impl $($var:ident $path:ident $data:ident),+;) => {
        impl_from_tuple_for_inmemorydir!($($var $path $data),+);
    }
}

// Generate for tuples of size `1..=7`
impl_from_tuples_for_inmemorydir!(
    s1 P1 D1,
    s2 P2 D2,
    s3 P3 D3,
    s4 P4 D4,
    s5 P5 D5,
    s6 P6 D6,
    s7 P7 D7
);

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
            assert_eq!(WildStr::new(a), b);
        }
        for (a, b) in &[("[..]b", "c"), ("b", "c"), ("b", "cb")] {
            assert_ne!(WildStr::new(a), b);
        }
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
