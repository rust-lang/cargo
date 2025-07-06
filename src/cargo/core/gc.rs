//! Support for garbage collecting unused files from downloaded files or
//! artifacts from the target directory.
//!
//! The [`Gc`] type provides the high-level interface for the
//! garbage-collection system.
//!
//! Garbage collection can be done "automatically" by cargo, which it does by
//! default once a day when running any command that does a lot of work (like
//! `cargo build`). The entry point for this is the [`auto_gc`] function,
//! which handles some basic setup, creating the [`Gc`], and calling
//! [`Gc::auto`].
//!
//! Garbage collection can also be done manually via the `cargo clean` command
//! by passing any option that requests deleting unused files. That is
//! implemented by calling the [`Gc::gc`] method.
//!
//! Garbage collection for the global cache is guided by the last-use tracking
//! implemented in the [`crate::core::global_cache_tracker`] module. See that
//! module documentation for an in-depth explanation of how global cache
//! tracking works.

use crate::core::global_cache_tracker::{self, GlobalCacheTracker};
use crate::ops::CleanContext;
use crate::util::cache_lock::{CacheLock, CacheLockMode};
use crate::{CargoResult, GlobalContext};
use anyhow::{Context as _, format_err};
use serde::Deserialize;
use std::time::Duration;

/// Default max age to auto-clean extracted sources, which can be recovered
/// without downloading anything.
const DEFAULT_MAX_AGE_EXTRACTED: &str = "1 month";
/// Default max ago to auto-clean cache data, which must be downloaded to
/// recover.
const DEFAULT_MAX_AGE_DOWNLOADED: &str = "3 months";
/// How often auto-gc will run by default unless overridden in the config.
const DEFAULT_AUTO_FREQUENCY: &str = "1 day";

/// Performs automatic garbage collection.
///
/// This is called in various places in Cargo where garbage collection should
/// be performed automatically based on the config settings. The default
/// behavior is to only clean once a day.
///
/// This should only be called in code paths for commands that are already
/// doing a lot of work. It should only be called *after* crates are
/// downloaded so that the last-use data is updated first.
///
/// It should be cheap to call this multiple times (subsequent calls are
/// ignored), but try not to abuse that.
pub fn auto_gc(gctx: &GlobalContext) {
    if !gctx.network_allowed() {
        // As a conservative choice, auto-gc is disabled when offline. If the
        // user is indefinitely offline, we don't want to delete things they
        // may later depend on.
        tracing::trace!(target: "gc", "running offline, auto gc disabled");
        return;
    }

    if let Err(e) = auto_gc_inner(gctx) {
        if global_cache_tracker::is_silent_error(&e) && !gctx.extra_verbose() {
            tracing::warn!(target: "gc", "failed to auto-clean cache data: {e:?}");
        } else {
            crate::display_warning_with_error(
                "failed to auto-clean cache data",
                &e,
                &mut gctx.shell(),
            );
        }
    }
}

fn auto_gc_inner(gctx: &GlobalContext) -> CargoResult<()> {
    let _lock = match gctx.try_acquire_package_cache_lock(CacheLockMode::MutateExclusive)? {
        Some(lock) => lock,
        None => {
            tracing::debug!(target: "gc", "unable to acquire mutate lock, auto gc disabled");
            return Ok(());
        }
    };
    // This should not be called when there are pending deferred entries, so check that.
    let deferred = gctx.deferred_global_last_use()?;
    debug_assert!(deferred.is_empty());
    let mut global_cache_tracker = gctx.global_cache_tracker()?;
    let mut gc = Gc::new(gctx, &mut global_cache_tracker)?;
    let mut clean_ctx = CleanContext::new(gctx);
    gc.auto(&mut clean_ctx)?;
    Ok(())
}

/// Cache cleaning settings from the `cache.global-clean` config table.
///
/// NOTE: Not all of these options may get stabilized. Some of them are very
/// low-level details, and may not be something typical users need.
///
/// If any of these options are `None`, the built-in default is used.
#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct GlobalCleanConfig {
    /// Anything older than this duration will be deleted in the source cache.
    max_src_age: Option<String>,
    /// Anything older than this duration will be deleted in the compressed crate cache.
    max_crate_age: Option<String>,
    /// Any index older than this duration will be deleted from the index cache.
    max_index_age: Option<String>,
    /// Any git checkout older than this duration will be deleted from the checkout cache.
    max_git_co_age: Option<String>,
    /// Any git clone older than this duration will be deleted from the git cache.
    max_git_db_age: Option<String>,
}

/// Options to use for garbage collection.
#[derive(Clone, Debug, Default)]
pub struct GcOpts {
    /// The `--max-src-age` CLI option.
    pub max_src_age: Option<Duration>,
    // The `--max-crate-age` CLI option.
    pub max_crate_age: Option<Duration>,
    /// The `--max-index-age` CLI option.
    pub max_index_age: Option<Duration>,
    /// The `--max-git-co-age` CLI option.
    pub max_git_co_age: Option<Duration>,
    /// The `--max-git-db-age` CLI option.
    pub max_git_db_age: Option<Duration>,
    /// The `--max-src-size` CLI option.
    pub max_src_size: Option<u64>,
    /// The `--max-crate-size` CLI option.
    pub max_crate_size: Option<u64>,
    /// The `--max-git-size` CLI option.
    pub max_git_size: Option<u64>,
    /// The `--max-download-size` CLI option.
    pub max_download_size: Option<u64>,
}

impl GcOpts {
    /// Returns whether any download cache cleaning options are set.
    pub fn is_download_cache_opt_set(&self) -> bool {
        self.max_src_age.is_some()
            || self.max_crate_age.is_some()
            || self.max_index_age.is_some()
            || self.max_git_co_age.is_some()
            || self.max_git_db_age.is_some()
            || self.max_src_size.is_some()
            || self.max_crate_size.is_some()
            || self.max_git_size.is_some()
            || self.max_download_size.is_some()
    }

    /// Returns whether any download cache cleaning options based on size are set.
    pub fn is_download_cache_size_set(&self) -> bool {
        self.max_src_size.is_some()
            || self.max_crate_size.is_some()
            || self.max_git_size.is_some()
            || self.max_download_size.is_some()
    }

    /// Updates the `GcOpts` to incorporate the specified max download age.
    ///
    /// "Download" means any cached data that can be re-downloaded.
    pub fn set_max_download_age(&mut self, max_download_age: Duration) {
        self.max_src_age = Some(maybe_newer_span(max_download_age, self.max_src_age));
        self.max_crate_age = Some(maybe_newer_span(max_download_age, self.max_crate_age));
        self.max_index_age = Some(maybe_newer_span(max_download_age, self.max_index_age));
        self.max_git_co_age = Some(maybe_newer_span(max_download_age, self.max_git_co_age));
        self.max_git_db_age = Some(maybe_newer_span(max_download_age, self.max_git_db_age));
    }

    /// Updates the configuration of this [`GcOpts`] to incorporate the
    /// settings from config.
    pub fn update_for_auto_gc(&mut self, gctx: &GlobalContext) -> CargoResult<()> {
        let config = gctx
            .get::<Option<GlobalCleanConfig>>("cache.global-clean")?
            .unwrap_or_default();
        self.update_for_auto_gc_config(&config, gctx.cli_unstable().gc)
    }

    fn update_for_auto_gc_config(
        &mut self,
        config: &GlobalCleanConfig,
        unstable_allowed: bool,
    ) -> CargoResult<()> {
        macro_rules! config_default {
            ($config:expr, $field:ident, $default:expr, $unstable_allowed:expr) => {
                if !unstable_allowed {
                    // These config options require -Zgc
                    $default
                } else {
                    $config.$field.as_deref().unwrap_or($default)
                }
            };
        }

        self.max_src_age = newer_time_span_for_config(
            self.max_src_age,
            "gc.auto.max-src-age",
            config_default!(
                config,
                max_src_age,
                DEFAULT_MAX_AGE_EXTRACTED,
                unstable_allowed
            ),
        )?;
        self.max_crate_age = newer_time_span_for_config(
            self.max_crate_age,
            "gc.auto.max-crate-age",
            config_default!(
                config,
                max_crate_age,
                DEFAULT_MAX_AGE_DOWNLOADED,
                unstable_allowed
            ),
        )?;
        self.max_index_age = newer_time_span_for_config(
            self.max_index_age,
            "gc.auto.max-index-age",
            config_default!(
                config,
                max_index_age,
                DEFAULT_MAX_AGE_DOWNLOADED,
                unstable_allowed
            ),
        )?;
        self.max_git_co_age = newer_time_span_for_config(
            self.max_git_co_age,
            "gc.auto.max-git-co-age",
            config_default!(
                config,
                max_git_co_age,
                DEFAULT_MAX_AGE_EXTRACTED,
                unstable_allowed
            ),
        )?;
        self.max_git_db_age = newer_time_span_for_config(
            self.max_git_db_age,
            "gc.auto.max-git-db-age",
            config_default!(
                config,
                max_git_db_age,
                DEFAULT_MAX_AGE_DOWNLOADED,
                unstable_allowed
            ),
        )?;
        Ok(())
    }
}

/// Garbage collector.
///
/// See the module docs at [`crate::core::gc`] for more information on GC.
pub struct Gc<'a, 'gctx> {
    gctx: &'gctx GlobalContext,
    global_cache_tracker: &'a mut GlobalCacheTracker,
    /// A lock on the package cache.
    ///
    /// This is important to be held, since we don't want multiple cargos to
    /// be allowed to write to the cache at the same time, or for others to
    /// read while we are modifying the cache.
    #[allow(dead_code)] // Held for drop.
    lock: CacheLock<'gctx>,
}

impl<'a, 'gctx> Gc<'a, 'gctx> {
    pub fn new(
        gctx: &'gctx GlobalContext,
        global_cache_tracker: &'a mut GlobalCacheTracker,
    ) -> CargoResult<Gc<'a, 'gctx>> {
        let lock = gctx.acquire_package_cache_lock(CacheLockMode::MutateExclusive)?;
        Ok(Gc {
            gctx,
            global_cache_tracker,
            lock,
        })
    }

    /// Performs automatic garbage cleaning.
    ///
    /// This returns immediately without doing work if garbage collection has
    /// been performed recently (since `cache.auto-clean-frequency`).
    fn auto(&mut self, clean_ctx: &mut CleanContext<'gctx>) -> CargoResult<()> {
        let freq = self
            .gctx
            .get::<Option<String>>("cache.auto-clean-frequency")?;
        let Some(freq) = parse_frequency(freq.as_deref().unwrap_or(DEFAULT_AUTO_FREQUENCY))? else {
            tracing::trace!(target: "gc", "auto gc disabled");
            return Ok(());
        };
        if !self.global_cache_tracker.should_run_auto_gc(freq)? {
            return Ok(());
        }
        let config = self
            .gctx
            .get::<Option<GlobalCleanConfig>>("cache.global-clean")?
            .unwrap_or_default();

        let mut gc_opts = GcOpts::default();
        gc_opts.update_for_auto_gc_config(&config, self.gctx.cli_unstable().gc)?;
        self.gc(clean_ctx, &gc_opts)?;
        if !clean_ctx.dry_run {
            self.global_cache_tracker.set_last_auto_gc()?;
        }
        Ok(())
    }

    /// Performs garbage collection based on the given options.
    pub fn gc(&mut self, clean_ctx: &mut CleanContext<'gctx>, gc_opts: &GcOpts) -> CargoResult<()> {
        self.global_cache_tracker.clean(clean_ctx, gc_opts)?;
        // In the future, other gc operations go here, such as target cleaning.
        Ok(())
    }
}

/// Returns the shorter duration from `cur_span` versus `config_span`.
///
/// This is used because the user may specify multiple options which overlap,
/// and this will pick whichever one is shorter.
///
/// * `cur_span` is the span we are comparing against (the value from the CLI
///   option). If None, just returns the config duration.
/// * `config_name` is the name of the config option the span is loaded from.
/// * `config_span` is the span value loaded from config.
fn newer_time_span_for_config(
    cur_span: Option<Duration>,
    config_name: &str,
    config_span: &str,
) -> CargoResult<Option<Duration>> {
    let config_span = parse_time_span_for_config(config_name, config_span)?;
    Ok(Some(maybe_newer_span(config_span, cur_span)))
}

/// Returns whichever [`Duration`] is shorter.
fn maybe_newer_span(a: Duration, b: Option<Duration>) -> Duration {
    match b {
        Some(b) => {
            if b < a {
                b
            } else {
                a
            }
        }
        None => a,
    }
}

/// Parses a frequency string.
///
/// Returns `Ok(None)` if the frequency is "never".
fn parse_frequency(frequency: &str) -> CargoResult<Option<Duration>> {
    if frequency == "always" {
        return Ok(Some(Duration::new(0, 0)));
    } else if frequency == "never" {
        return Ok(None);
    }
    let duration = maybe_parse_time_span(frequency).ok_or_else(|| {
        format_err!(
            "config option `cache.auto-clean-frequency` expected a value of \"always\", \"never\", \
             or \"N seconds/minutes/days/weeks/months\", got: {frequency:?}"
        )
    })?;
    Ok(Some(duration))
}

/// Parses a time span value fetched from config.
///
/// This is here to provide better error messages specific to reading from
/// config.
fn parse_time_span_for_config(config_name: &str, span: &str) -> CargoResult<Duration> {
    maybe_parse_time_span(span).ok_or_else(|| {
        format_err!(
            "config option `{config_name}` expected a value of the form \
             \"N seconds/minutes/days/weeks/months\", got: {span:?}"
        )
    })
}

/// Parses a time span string.
///
/// Returns None if the value is not valid. See [`parse_time_span`] if you
/// need a variant that generates an error message.
fn maybe_parse_time_span(span: &str) -> Option<Duration> {
    let Some(right_i) = span.find(|c: char| !c.is_ascii_digit()) else {
        return None;
    };
    let (left, mut right) = span.split_at(right_i);
    if right.starts_with(' ') {
        right = &right[1..];
    }
    let count: u64 = left.parse().ok()?;
    let factor = match right {
        "second" | "seconds" => 1,
        "minute" | "minutes" => 60,
        "hour" | "hours" => 60 * 60,
        "day" | "days" => 24 * 60 * 60,
        "week" | "weeks" => 7 * 24 * 60 * 60,
        "month" | "months" => 2_629_746, // average is 30.436875 days
        _ => return None,
    };
    Some(Duration::from_secs(factor * count))
}

/// Parses a time span string.
pub fn parse_time_span(span: &str) -> CargoResult<Duration> {
    maybe_parse_time_span(span).ok_or_else(|| {
        format_err!(
            "expected a value of the form \
             \"N seconds/minutes/days/weeks/months\", got: {span:?}"
        )
    })
}

/// Parses a file size using metric or IEC units.
pub fn parse_human_size(input: &str) -> CargoResult<u64> {
    let re = regex::Regex::new(r"(?i)^([0-9]+(\.[0-9])?) ?(b|kb|mb|gb|kib|mib|gib)?$").unwrap();
    let cap = re.captures(input).ok_or_else(|| {
        format_err!(
            "invalid size `{input}`, \
             expected a number with an optional B, kB, MB, GB, kiB, MiB, or GiB suffix"
        )
    })?;
    let factor = match cap.get(3) {
        Some(suffix) => match suffix.as_str().to_lowercase().as_str() {
            "b" => 1.0,
            "kb" => 1_000.0,
            "mb" => 1_000_000.0,
            "gb" => 1_000_000_000.0,
            "kib" => 1024.0,
            "mib" => 1024.0 * 1024.0,
            "gib" => 1024.0 * 1024.0 * 1024.0,
            s => unreachable!("suffix `{s}` out of sync with regex"),
        },
        None => {
            return cap[1]
                .parse()
                .with_context(|| format!("expected an integer size, got `{}`", &cap[1]));
        }
    };
    let num = cap[1]
        .parse::<f64>()
        .with_context(|| format!("expected an integer or float, found `{}`", &cap[1]))?;
    Ok((num * factor) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn time_spans() {
        let d = |x| Some(Duration::from_secs(x));
        assert_eq!(maybe_parse_time_span("0 seconds"), d(0));
        assert_eq!(maybe_parse_time_span("1second"), d(1));
        assert_eq!(maybe_parse_time_span("23 seconds"), d(23));
        assert_eq!(maybe_parse_time_span("5 minutes"), d(60 * 5));
        assert_eq!(maybe_parse_time_span("2 hours"), d(60 * 60 * 2));
        assert_eq!(maybe_parse_time_span("1 day"), d(60 * 60 * 24));
        assert_eq!(maybe_parse_time_span("2 weeks"), d(60 * 60 * 24 * 14));
        assert_eq!(maybe_parse_time_span("6 months"), d(2_629_746 * 6));

        assert_eq!(parse_frequency("5 seconds").unwrap(), d(5));
        assert_eq!(parse_frequency("always").unwrap(), d(0));
        assert_eq!(parse_frequency("never").unwrap(), None);
    }

    #[test]
    fn time_span_errors() {
        assert_eq!(maybe_parse_time_span(""), None);
        assert_eq!(maybe_parse_time_span("1"), None);
        assert_eq!(maybe_parse_time_span("second"), None);
        assert_eq!(maybe_parse_time_span("+2 seconds"), None);
        assert_eq!(maybe_parse_time_span("day"), None);
        assert_eq!(maybe_parse_time_span("-1 days"), None);
        assert_eq!(maybe_parse_time_span("1.5 days"), None);
        assert_eq!(maybe_parse_time_span("1 dayz"), None);
        assert_eq!(maybe_parse_time_span("always"), None);
        assert_eq!(maybe_parse_time_span("never"), None);
        assert_eq!(maybe_parse_time_span("1 day "), None);
        assert_eq!(maybe_parse_time_span(" 1 day"), None);
        assert_eq!(maybe_parse_time_span("1  second"), None);

        let e =
            parse_time_span_for_config("cache.global-clean.max-src-age", "-1 days").unwrap_err();
        assert_eq!(
            e.to_string(),
            "config option `cache.global-clean.max-src-age` \
             expected a value of the form \"N seconds/minutes/days/weeks/months\", \
             got: \"-1 days\""
        );
        let e = parse_frequency("abc").unwrap_err();
        assert_eq!(
            e.to_string(),
            "config option `cache.auto-clean-frequency` \
             expected a value of \"always\", \"never\", or \"N seconds/minutes/days/weeks/months\", \
             got: \"abc\""
        );
    }

    #[test]
    fn human_sizes() {
        assert_eq!(parse_human_size("0").unwrap(), 0);
        assert_eq!(parse_human_size("123").unwrap(), 123);
        assert_eq!(parse_human_size("123b").unwrap(), 123);
        assert_eq!(parse_human_size("123B").unwrap(), 123);
        assert_eq!(parse_human_size("123 b").unwrap(), 123);
        assert_eq!(parse_human_size("123 B").unwrap(), 123);
        assert_eq!(parse_human_size("1kb").unwrap(), 1_000);
        assert_eq!(parse_human_size("5kb").unwrap(), 5_000);
        assert_eq!(parse_human_size("1mb").unwrap(), 1_000_000);
        assert_eq!(parse_human_size("1gb").unwrap(), 1_000_000_000);
        assert_eq!(parse_human_size("1kib").unwrap(), 1_024);
        assert_eq!(parse_human_size("1mib").unwrap(), 1_048_576);
        assert_eq!(parse_human_size("1gib").unwrap(), 1_073_741_824);
        assert_eq!(parse_human_size("1.5kb").unwrap(), 1_500);
        assert_eq!(parse_human_size("1.7b").unwrap(), 1);

        assert!(parse_human_size("").is_err());
        assert!(parse_human_size("x").is_err());
        assert!(parse_human_size("1x").is_err());
        assert!(parse_human_size("1 2").is_err());
        assert!(parse_human_size("1.5").is_err());
        assert!(parse_human_size("+1").is_err());
        assert!(parse_human_size("123  b").is_err());
    }
}
