//! Support for CLI progress bars.

use std::cmp;
use std::time::{Duration, Instant};

use crate::core::shell::Verbosity;
use crate::util::context::ProgressWhen;
use crate::util::{CargoResult, GlobalContext};
use cargo_util::is_ci;
use unicode_width::UnicodeWidthChar;

/// CLI progress bar.
///
/// The `Progress` object can be in an enabled or disabled state. When
/// disabled, calling any of the methods to update it will not display
/// anything. Disabling is typically done by the user with options such as
/// `--quiet` or the `term.progress` config option.
///
/// There are several methods to update the progress bar and to cause it to
/// update its display.
///
/// The bar will be removed from the display when the `Progress` object is
/// dropped or [`Progress::clear`] is called.
///
/// The progress bar has built-in rate limiting to avoid updating the display
/// too fast. It should usually be fine to call [`Progress::tick`] as often as
/// needed, though be cautious if the tick rate is very high or it is
/// expensive to compute the progress value.
pub struct Progress<'gctx> {
    state: Option<State<'gctx>>,
}

/// Indicates the style of information for displaying the amount of progress.
///
/// See also [`Progress::print_now`] for displaying progress without a bar.
pub enum ProgressStyle {
    /// Displays progress as a percentage.
    ///
    /// Example: `Fetch [=====================>   ]  88.15%`
    ///
    /// This is good for large values like number of bytes downloaded.
    Percentage,
    /// Displays progress as a ratio.
    ///
    /// Example: `Building [===>                      ] 35/222`
    ///
    /// This is good for smaller values where the exact number is useful to see.
    Ratio,
    /// Does not display an exact value of how far along it is.
    ///
    /// Example: `Fetch [===========>                     ]`
    ///
    /// This is good for situations where the exact value is an approximation,
    /// and thus there isn't anything accurate to display to the user.
    Indeterminate,
}

struct Throttle {
    first: bool,
    last_update: Instant,
}

struct State<'gctx> {
    gctx: &'gctx GlobalContext,
    format: Format,
    name: String,
    done: bool,
    throttle: Throttle,
    last_line: Option<String>,
    fixed_width: Option<usize>,
}

struct Format {
    style: ProgressStyle,
    max_width: usize,
    max_print: usize,
    term_integration: TerminalIntegration,
    unicode: bool,
}

/// Controls terminal progress integration via OSC sequences.
struct TerminalIntegration {
    enabled: bool,
    error: bool,
}

/// A progress status value printable as an ANSI OSC 9;4 escape code.
#[cfg_attr(test, derive(PartialEq, Debug))]
enum StatusValue {
    /// No output.
    None,
    /// Remove progress.
    Remove,
    /// Progress value (0-100).
    Value(f64),
    /// Indeterminate state (no bar, just animation)
    Indeterminate,
    /// Progress value in an error state (0-100).
    Error(f64),
}

enum ProgressOutput {
    /// Print progress without a message
    PrintNow,
    /// Progress, message and progress report
    TextAndReport(String, StatusValue),
    /// Only progress report, no message and no text progress
    Report(StatusValue),
}

impl TerminalIntegration {
    #[cfg(test)]
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            error: false,
        }
    }

    /// Creates a `TerminalIntegration` from Cargo's configuration.
    /// Autodetect support if not explicitly enabled or disabled.
    fn from_config(gctx: &GlobalContext) -> Self {
        let enabled = gctx
            .progress_config()
            .term_integration
            .unwrap_or_else(|| gctx.shell().is_err_term_integration_available());

        Self {
            enabled,
            error: false,
        }
    }

    fn progress_state(&self, value: StatusValue) -> StatusValue {
        match (self.enabled, self.error) {
            (true, false) => value,
            (true, true) => match value {
                StatusValue::Value(v) => StatusValue::Error(v),
                _ => StatusValue::Error(100.0),
            },
            (false, _) => StatusValue::None,
        }
    }

    pub fn remove(&self) -> StatusValue {
        self.progress_state(StatusValue::Remove)
    }

    pub fn value(&self, percent: f64) -> StatusValue {
        self.progress_state(StatusValue::Value(percent))
    }

    pub fn indeterminate(&self) -> StatusValue {
        self.progress_state(StatusValue::Indeterminate)
    }

    pub fn error(&mut self) {
        self.error = true;
    }
}

impl std::fmt::Display for StatusValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // From https://conemu.github.io/en/AnsiEscapeCodes.html#ConEmu_specific_OSC
        // ESC ] 9 ; 4 ; st ; pr ST
        // When st is 0: remove progress.
        // When st is 1: set progress value to pr (number, 0-100).
        // When st is 2: set error state in taskbar, pr is optional.
        // When st is 3: set indeterminate state, pr is ignored.
        // When st is 4: set paused state, pr is optional.
        let (state, progress) = match self {
            Self::None => return Ok(()), // No output
            Self::Remove => (0, 0.0),
            Self::Value(v) => (1, *v),
            Self::Indeterminate => (3, 0.0),
            Self::Error(v) => (2, *v),
        };
        write!(f, "\x1b]9;4;{state};{progress:.0}\x1b\\")
    }
}

impl<'gctx> Progress<'gctx> {
    /// Creates a new progress bar.
    ///
    /// The first parameter is the text displayed to the left of the bar, such
    /// as "Fetching".
    ///
    /// The progress bar is not displayed until explicitly updated with one if
    /// its methods.
    ///
    /// The progress bar may be created in a disabled state if the user has
    /// disabled progress display (such as with the `--quiet` option).
    pub fn with_style(
        name: &str,
        style: ProgressStyle,
        gctx: &'gctx GlobalContext,
    ) -> Progress<'gctx> {
        // report no progress when -q (for quiet) or TERM=dumb are set
        // or if running on Continuous Integration service like Travis where the
        // output logs get mangled.
        let dumb = match gctx.get_env("TERM") {
            Ok(term) => term == "dumb",
            Err(_) => false,
        };
        let progress_config = gctx.progress_config();
        match progress_config.when {
            ProgressWhen::Always => return Progress::new_priv(name, style, gctx),
            ProgressWhen::Never => return Progress { state: None },
            ProgressWhen::Auto => {}
        }
        if gctx.shell().verbosity() == Verbosity::Quiet || dumb || is_ci() {
            return Progress { state: None };
        }
        Progress::new_priv(name, style, gctx)
    }

    fn new_priv(name: &str, style: ProgressStyle, gctx: &'gctx GlobalContext) -> Progress<'gctx> {
        let progress_config = gctx.progress_config();
        let width = progress_config
            .width
            .or_else(|| gctx.shell().err_width().progress_max_width());

        Progress {
            state: width.map(|n| State {
                gctx,
                format: Format {
                    style,
                    max_width: n,
                    // 50 gives some space for text after the progress bar,
                    // even on narrow (e.g. 80 char) terminals.
                    max_print: 50,
                    term_integration: TerminalIntegration::from_config(gctx),
                    unicode: gctx.shell().err_unicode(),
                },
                name: name.to_string(),
                done: false,
                throttle: Throttle::new(),
                last_line: None,
                fixed_width: progress_config.width,
            }),
        }
    }

    /// Disables the progress bar, ensuring it won't be displayed.
    pub fn disable(&mut self) {
        self.state = None;
    }

    /// Returns whether or not the progress bar is allowed to be displayed.
    pub fn is_enabled(&self) -> bool {
        self.state.is_some()
    }

    /// Creates a new `Progress` with the [`ProgressStyle::Percentage`] style.
    ///
    /// See [`Progress::with_style`] for more information.
    pub fn new(name: &str, gctx: &'gctx GlobalContext) -> Progress<'gctx> {
        Self::with_style(name, ProgressStyle::Percentage, gctx)
    }

    /// Updates the state of the progress bar.
    ///
    /// * `cur` should be how far along the progress is.
    /// * `max` is the maximum value for the progress bar.
    /// * `msg` is a small piece of text to display at the end of the progress
    ///   bar. It will be truncated with `…` if it does not fit on the terminal.
    ///
    /// This may not actually update the display if `tick` is being called too
    /// quickly.
    pub fn tick(&mut self, cur: usize, max: usize, msg: &str) -> CargoResult<()> {
        let Some(s) = &mut self.state else {
            return Ok(());
        };

        // Don't update too often as it can cause excessive performance loss
        // just putting stuff onto the terminal. We also want to avoid
        // flickering by not drawing anything that goes away too quickly. As a
        // result we've got two branches here:
        //
        // 1. If we haven't drawn anything, we wait for a period of time to
        //    actually start drawing to the console. This ensures that
        //    short-lived operations don't flicker on the console. Currently
        //    there's a 500ms delay to when we first draw something.
        // 2. If we've drawn something, then we rate limit ourselves to only
        //    draw to the console every so often. Currently there's a 100ms
        //    delay between updates.
        if !s.throttle.allowed() {
            return Ok(());
        }

        s.tick(cur, max, msg)
    }

    /// Updates the state of the progress bar.
    ///
    /// This is the same as [`Progress::tick`], but ignores rate throttling
    /// and forces the display to be updated immediately.
    ///
    /// This may be useful for situations where you know you aren't calling
    /// `tick` too fast, and accurate information is more important than
    /// limiting the console update rate.
    pub fn tick_now(&mut self, cur: usize, max: usize, msg: &str) -> CargoResult<()> {
        match self.state {
            Some(ref mut s) => s.tick(cur, max, msg),
            None => Ok(()),
        }
    }

    /// Returns whether or not updates are currently being throttled.
    ///
    /// This can be useful if computing the values for calling the
    /// [`Progress::tick`] function may require some expensive work.
    pub fn update_allowed(&mut self) -> bool {
        match &mut self.state {
            Some(s) => s.throttle.allowed(),
            None => false,
        }
    }

    /// Displays progress without a bar.
    ///
    /// The given `msg` is the text to display after the status message.
    ///
    /// Example: `Downloading 61 crates, remaining bytes: 28.0 MB`
    ///
    /// This does not have any rate limit throttling, so be careful about
    /// calling it too often.
    pub fn print_now(&mut self, msg: &str) -> CargoResult<()> {
        match &mut self.state {
            Some(s) => s.print(ProgressOutput::PrintNow, msg),
            None => Ok(()),
        }
    }

    /// Clears the progress bar from the console.
    pub fn clear(&mut self) {
        if let Some(ref mut s) = self.state {
            s.clear();
        }
    }

    /// Sets the progress reporter to the error state.
    pub fn indicate_error(&mut self) {
        if let Some(s) = &mut self.state {
            s.format.term_integration.error()
        }
    }
}

impl Throttle {
    fn new() -> Throttle {
        Throttle {
            first: true,
            last_update: Instant::now(),
        }
    }

    fn allowed(&mut self) -> bool {
        if self.first {
            let delay = Duration::from_millis(500);
            if self.last_update.elapsed() < delay {
                return false;
            }
        } else {
            let interval = Duration::from_millis(100);
            if self.last_update.elapsed() < interval {
                return false;
            }
        }
        self.update();
        true
    }

    fn update(&mut self) {
        self.first = false;
        self.last_update = Instant::now();
    }
}

impl<'gctx> State<'gctx> {
    fn tick(&mut self, cur: usize, max: usize, msg: &str) -> CargoResult<()> {
        if self.done {
            write!(
                self.gctx.shell().err(),
                "{}",
                self.format.term_integration.remove()
            )?;
            return Ok(());
        }

        if max > 0 && cur == max {
            self.done = true;
        }

        // Write out a pretty header, then the progress bar itself, and then
        // return back to the beginning of the line for the next print.
        self.try_update_max_width();
        if let Some(pbar) = self.format.progress(cur, max) {
            self.print(pbar, msg)?;
        }
        Ok(())
    }

    fn print(&mut self, progress: ProgressOutput, msg: &str) -> CargoResult<()> {
        self.throttle.update();
        self.try_update_max_width();

        let (mut line, report) = match progress {
            ProgressOutput::PrintNow => (String::new(), None),
            ProgressOutput::TextAndReport(prefix, report) => (prefix, Some(report)),
            ProgressOutput::Report(report) => (String::new(), Some(report)),
        };

        // make sure we have enough room for the header
        if self.format.max_width < 15 {
            // even if we don't have space we can still output progress report
            if let Some(tb) = report {
                write!(self.gctx.shell().err(), "{tb}\r")?;
            }
            return Ok(());
        }

        self.format.render(&mut line, msg);
        while line.len() < self.format.max_width - 15 {
            line.push(' ');
        }

        // Only update if the line has changed.
        if self.gctx.shell().is_cleared() || self.last_line.as_ref() != Some(&line) {
            let mut shell = self.gctx.shell();
            shell.set_needs_clear(false);
            shell.transient_status(&self.name)?;
            if let Some(tb) = report {
                write!(shell.err(), "{line}{tb}\r")?;
            } else {
                write!(shell.err(), "{line}\r")?;
            }
            self.last_line = Some(line);
            shell.set_needs_clear(true);
        }

        Ok(())
    }

    fn clear(&mut self) {
        // Always clear the progress report
        let _ = write!(
            self.gctx.shell().err(),
            "{}",
            self.format.term_integration.remove()
        );
        // No need to clear if the progress is not currently being displayed.
        if self.last_line.is_some() && !self.gctx.shell().is_cleared() {
            self.gctx.shell().err_erase_line();
            self.last_line = None;
        }
    }

    fn try_update_max_width(&mut self) {
        if self.fixed_width.is_none() {
            if let Some(n) = self.gctx.shell().err_width().progress_max_width() {
                self.format.max_width = n;
            }
        }
    }
}

impl Format {
    fn progress(&self, cur: usize, max: usize) -> Option<ProgressOutput> {
        assert!(cur <= max);
        // Render the percentage at the far right and then figure how long the
        // progress bar is
        let pct = (cur as f64) / (max as f64);
        let pct = if !pct.is_finite() { 0.0 } else { pct };
        let stats = match self.style {
            ProgressStyle::Percentage => format!(" {:6.02}%", pct * 100.0),
            ProgressStyle::Ratio => format!(" {cur}/{max}"),
            ProgressStyle::Indeterminate => String::new(),
        };
        let report = match self.style {
            ProgressStyle::Percentage | ProgressStyle::Ratio => {
                self.term_integration.value(pct * 100.0)
            }
            ProgressStyle::Indeterminate => self.term_integration.indeterminate(),
        };

        let extra_len = stats.len() + 2 /* [ and ] */ + 15 /* status header */;
        let Some(display_width) = self.width().checked_sub(extra_len) else {
            if self.term_integration.enabled {
                return Some(ProgressOutput::Report(report));
            }
            return None;
        };

        let mut string = String::with_capacity(self.max_width);
        string.push('[');
        let hashes = display_width as f64 * pct;
        let hashes = hashes as usize;

        // Draw the `===>`
        if hashes > 0 {
            for _ in 0..hashes - 1 {
                string.push('=');
            }
            if cur == max {
                string.push('=');
            } else {
                string.push('>');
            }
        }

        // Draw the empty space we have left to do
        for _ in 0..(display_width - hashes) {
            string.push(' ');
        }
        string.push(']');
        string.push_str(&stats);

        Some(ProgressOutput::TextAndReport(string, report))
    }

    fn render(&self, string: &mut String, msg: &str) {
        let mut avail_msg_len = self.max_width - string.len() - 15;
        let mut ellipsis_pos = 0;

        let (ellipsis, ellipsis_width) = if self.unicode { ("…", 1) } else { ("...", 3) };

        if avail_msg_len <= ellipsis_width {
            return;
        }
        for c in msg.chars() {
            let display_width = c.width().unwrap_or(0);
            if avail_msg_len >= display_width {
                avail_msg_len -= display_width;
                string.push(c);
                if avail_msg_len >= ellipsis_width {
                    ellipsis_pos = string.len();
                }
            } else {
                string.truncate(ellipsis_pos);
                string.push_str(ellipsis);
                break;
            }
        }
    }

    #[cfg(test)]
    fn progress_status(&self, cur: usize, max: usize, msg: &str) -> Option<String> {
        let mut ret = match self.progress(cur, max)? {
            // Check only the variant that contains text.
            ProgressOutput::TextAndReport(text, _) => text,
            _ => return None,
        };
        self.render(&mut ret, msg);
        Some(ret)
    }

    fn width(&self) -> usize {
        cmp::min(self.max_width, self.max_print)
    }
}

impl<'gctx> Drop for State<'gctx> {
    fn drop(&mut self) {
        self.clear();
    }
}

#[test]
fn test_progress_status() {
    let format = Format {
        style: ProgressStyle::Ratio,
        max_print: 40,
        max_width: 60,
        term_integration: TerminalIntegration::new(false),
        unicode: true,
    };
    assert_eq!(
        format.progress_status(0, 4, ""),
        Some("[                   ] 0/4".to_string())
    );
    assert_eq!(
        format.progress_status(1, 4, ""),
        Some("[===>               ] 1/4".to_string())
    );
    assert_eq!(
        format.progress_status(2, 4, ""),
        Some("[========>          ] 2/4".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, ""),
        Some("[=============>     ] 3/4".to_string())
    );
    assert_eq!(
        format.progress_status(4, 4, ""),
        Some("[===================] 4/4".to_string())
    );

    assert_eq!(
        format.progress_status(3999, 4000, ""),
        Some("[===========> ] 3999/4000".to_string())
    );
    assert_eq!(
        format.progress_status(4000, 4000, ""),
        Some("[=============] 4000/4000".to_string())
    );

    assert_eq!(
        format.progress_status(3, 4, ": short message"),
        Some("[=============>     ] 3/4: short message".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, ": msg thats just fit"),
        Some("[=============>     ] 3/4: msg thats just fit".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, ": msg that's just fit"),
        Some("[=============>     ] 3/4: msg that's just f…".to_string())
    );

    // combining diacritics have width zero and thus can fit max_width.
    let zalgo_msg = "z̸̧̢̗͉̝̦͍̱ͧͦͨ̑̅̌ͥ́͢a̢ͬͨ̽ͯ̅̑ͥ͋̏̑ͫ̄͢͏̫̝̪̤͎̱̣͍̭̞̙̱͙͍̘̭͚l̶̡̛̥̝̰̭̹̯̯̞̪͇̱̦͙͔̘̼͇͓̈ͨ͗ͧ̓͒ͦ̀̇ͣ̈ͭ͊͛̃̑͒̿̕͜g̸̷̢̩̻̻͚̠͓̞̥͐ͩ͌̑ͥ̊̽͋͐̐͌͛̐̇̑ͨ́ͅo͙̳̣͔̰̠̜͕͕̞̦̙̭̜̯̹̬̻̓͑ͦ͋̈̉͌̃ͯ̀̂͠ͅ ̸̡͎̦̲̖̤̺̜̮̱̰̥͔̯̅̏ͬ̂ͨ̋̃̽̈́̾̔̇ͣ̚͜͜h̡ͫ̐̅̿̍̀͜҉̛͇̭̹̰̠͙̞ẽ̶̙̹̳̖͉͎̦͂̋̓ͮ̔ͬ̐̀͂̌͑̒͆̚͜͠ ͓͓̟͍̮̬̝̝̰͓͎̼̻ͦ͐̾̔͒̃̓͟͟c̮̦͍̺͈͚̯͕̄̒͐̂͊̊͗͊ͤͣ̀͘̕͝͞o̶͍͚͍̣̮͌ͦ̽̑ͩ̅ͮ̐̽̏͗́͂̅ͪ͠m̷̧͖̻͔̥̪̭͉͉̤̻͖̩̤͖̘ͦ̂͌̆̂ͦ̒͊ͯͬ͊̉̌ͬ͝͡e̵̹̣͍̜̺̤̤̯̫̹̠̮͎͙̯͚̰̼͗͐̀̒͂̉̀̚͝͞s̵̲͍͙͖̪͓͓̺̱̭̩̣͖̣ͤͤ͂̎̈͗͆ͨͪ̆̈͗͝͠";
    assert_eq!(
        format.progress_status(3, 4, zalgo_msg),
        Some("[=============>     ] 3/4".to_string() + zalgo_msg)
    );

    // some non-ASCII ellipsize test
    assert_eq!(
        format.progress_status(3, 4, "_123456789123456e\u{301}\u{301}8\u{301}90a"),
        Some("[=============>     ] 3/4_123456789123456e\u{301}\u{301}8\u{301}9…".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, "：每個漢字佔據了兩個字元"),
        Some("[=============>     ] 3/4：每個漢字佔據了兩…".to_string())
    );
    assert_eq!(
        // handle breaking at middle of character
        format.progress_status(3, 4, "：-每個漢字佔據了兩個字元"),
        Some("[=============>     ] 3/4：-每個漢字佔據了兩…".to_string())
    );
}

#[test]
fn test_progress_status_percentage() {
    let format = Format {
        style: ProgressStyle::Percentage,
        max_print: 40,
        max_width: 60,
        term_integration: TerminalIntegration::new(false),
        unicode: true,
    };
    assert_eq!(
        format.progress_status(0, 77, ""),
        Some("[               ]   0.00%".to_string())
    );
    assert_eq!(
        format.progress_status(1, 77, ""),
        Some("[               ]   1.30%".to_string())
    );
    assert_eq!(
        format.progress_status(76, 77, ""),
        Some("[=============> ]  98.70%".to_string())
    );
    assert_eq!(
        format.progress_status(77, 77, ""),
        Some("[===============] 100.00%".to_string())
    );
}

#[test]
fn test_progress_status_too_short() {
    let format = Format {
        style: ProgressStyle::Percentage,
        max_print: 25,
        max_width: 25,
        term_integration: TerminalIntegration::new(false),
        unicode: true,
    };
    assert_eq!(
        format.progress_status(1, 1, ""),
        Some("[] 100.00%".to_string())
    );

    let format = Format {
        style: ProgressStyle::Percentage,
        max_print: 24,
        max_width: 24,
        term_integration: TerminalIntegration::new(false),
        unicode: true,
    };
    assert_eq!(format.progress_status(1, 1, ""), None);
}

#[test]
fn test_term_integration_disabled() {
    let report = TerminalIntegration::new(false);
    let mut out = String::new();
    out.push_str(&report.remove().to_string());
    out.push_str(&report.value(10.0).to_string());
    out.push_str(&report.indeterminate().to_string());
    assert!(out.is_empty());
}

#[test]
fn test_term_integration_error_state() {
    let mut report = TerminalIntegration::new(true);
    assert_eq!(report.value(10.0), StatusValue::Value(10.0));
    report.error();
    assert_eq!(report.value(50.0), StatusValue::Error(50.0));
}
