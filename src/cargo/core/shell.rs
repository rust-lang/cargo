use std::fmt;
use std::io::IsTerminal;
use std::io::prelude::*;

use annotate_snippets::{Renderer, Report};
use anstream::AutoStream;
use anstyle::Style;

use crate::util::errors::CargoResult;
use crate::util::hostname;
use crate::util::style::*;

/// An abstraction around console output that remembers preferences for output
/// verbosity and color.
pub struct Shell {
    /// Wrapper around stdout/stderr. This helps with supporting sending
    /// output to a memory buffer which is useful for tests.
    output: ShellOut,
    /// How verbose messages should be.
    verbosity: Verbosity,
    /// Flag that indicates the current line needs to be cleared before
    /// printing. Used when a progress bar is currently displayed.
    needs_clear: bool,
    hostname: Option<String>,
}

impl fmt::Debug for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.output {
            ShellOut::Write(_) => f
                .debug_struct("Shell")
                .field("verbosity", &self.verbosity)
                .finish(),
            ShellOut::Stream { color_choice, .. } => f
                .debug_struct("Shell")
                .field("verbosity", &self.verbosity)
                .field("color_choice", &color_choice)
                .finish(),
        }
    }
}

impl Shell {
    /// Creates a new shell (color choice and verbosity), defaulting to 'auto' color and verbose
    /// output.
    pub fn new() -> Shell {
        let auto_clr = ColorChoice::CargoAuto;
        let stdout_choice = auto_clr.to_anstream_color_choice();
        let stderr_choice = auto_clr.to_anstream_color_choice();
        Shell {
            output: ShellOut::Stream {
                stdout: AutoStream::new(std::io::stdout(), stdout_choice),
                stderr: AutoStream::new(std::io::stderr(), stderr_choice),
                color_choice: auto_clr,
                hyperlinks: supports_hyperlinks(),
                stderr_tty: std::io::stderr().is_terminal(),
                stdout_unicode: supports_unicode(&std::io::stdout()),
                stderr_unicode: supports_unicode(&std::io::stderr()),
                stderr_term_integration: supports_term_integration(&std::io::stderr()),
            },
            verbosity: Verbosity::Verbose,
            needs_clear: false,
            hostname: None,
        }
    }

    /// Creates a shell from a plain writable object, with no color, and max verbosity.
    pub fn from_write(out: Box<dyn Write + Send + Sync>) -> Shell {
        Shell {
            output: ShellOut::Write(AutoStream::never(out)), // strip all formatting on write
            verbosity: Verbosity::Verbose,
            needs_clear: false,
            hostname: None,
        }
    }

    /// Prints a message, where the status will have `color` color, and can be justified. The
    /// messages follows without color.
    fn print(
        &mut self,
        status: &dyn fmt::Display,
        message: Option<&dyn fmt::Display>,
        color: &Style,
        justified: bool,
    ) -> CargoResult<()> {
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => {
                if self.needs_clear {
                    self.err_erase_line();
                }
                self.output
                    .message_stderr(status, message, color, justified)
            }
        }
    }

    /// Sets whether the next print should clear the current line.
    pub fn set_needs_clear(&mut self, needs_clear: bool) {
        self.needs_clear = needs_clear;
    }

    /// Returns `true` if the `needs_clear` flag is unset.
    pub fn is_cleared(&self) -> bool {
        !self.needs_clear
    }

    /// Returns the width of the terminal in spaces, if any.
    pub fn err_width(&self) -> TtyWidth {
        match self.output {
            ShellOut::Stream {
                stderr_tty: true, ..
            } => imp::stderr_width(),
            _ => TtyWidth::NoTty,
        }
    }

    /// Returns `true` if stderr is a tty.
    pub fn is_err_tty(&self) -> bool {
        match self.output {
            ShellOut::Stream { stderr_tty, .. } => stderr_tty,
            _ => false,
        }
    }

    pub fn is_err_term_integration_available(&self) -> bool {
        if let ShellOut::Stream {
            stderr_term_integration,
            ..
        } = self.output
        {
            stderr_term_integration
        } else {
            false
        }
    }

    /// Gets a reference to the underlying stdout writer.
    pub fn out(&mut self) -> &mut dyn Write {
        if self.needs_clear {
            self.err_erase_line();
        }
        self.output.stdout()
    }

    /// Gets a reference to the underlying stderr writer.
    pub fn err(&mut self) -> &mut dyn Write {
        if self.needs_clear {
            self.err_erase_line();
        }
        self.output.stderr()
    }

    /// Erase from cursor to end of line.
    pub fn err_erase_line(&mut self) {
        if self.err_supports_color() {
            imp::err_erase_line(self);
            self.needs_clear = false;
        }
    }

    /// Shortcut to right-align and color green a status message.
    pub fn status<T, U>(&mut self, status: T, message: U) -> CargoResult<()>
    where
        T: fmt::Display,
        U: fmt::Display,
    {
        self.print(&status, Some(&message), &HEADER, true)
    }

    pub fn transient_status<T>(&mut self, status: T) -> CargoResult<()>
    where
        T: fmt::Display,
    {
        self.print(&status, None, &TRANSIENT, true)
    }

    /// Shortcut to right-align a status message.
    pub fn status_with_color<T, U>(
        &mut self,
        status: T,
        message: U,
        color: &Style,
    ) -> CargoResult<()>
    where
        T: fmt::Display,
        U: fmt::Display,
    {
        self.print(&status, Some(&message), color, true)
    }

    /// Runs the callback only if we are in verbose mode.
    pub fn verbose<F>(&mut self, mut callback: F) -> CargoResult<()>
    where
        F: FnMut(&mut Shell) -> CargoResult<()>,
    {
        match self.verbosity {
            Verbosity::Verbose => callback(self),
            _ => Ok(()),
        }
    }

    /// Runs the callback if we are not in verbose mode.
    pub fn concise<F>(&mut self, mut callback: F) -> CargoResult<()>
    where
        F: FnMut(&mut Shell) -> CargoResult<()>,
    {
        match self.verbosity {
            Verbosity::Verbose => Ok(()),
            _ => callback(self),
        }
    }

    /// Prints a red 'error' message.
    pub fn error<T: fmt::Display>(&mut self, message: T) -> CargoResult<()> {
        if self.needs_clear {
            self.err_erase_line();
        }
        self.output
            .message_stderr(&"error", Some(&message), &ERROR, false)
    }

    /// Prints an amber 'warning' message.
    pub fn warn<T: fmt::Display>(&mut self, message: T) -> CargoResult<()> {
        self.print(&"warning", Some(&message), &WARN, false)
    }

    /// Prints a cyan 'note' message.
    pub fn note<T: fmt::Display>(&mut self, message: T) -> CargoResult<()> {
        let report = &[annotate_snippets::Group::with_title(
            annotate_snippets::Level::NOTE.secondary_title(message.to_string()),
        )];
        self.print_report(report, false)
    }

    /// Updates the verbosity of the shell.
    pub fn set_verbosity(&mut self, verbosity: Verbosity) {
        self.verbosity = verbosity;
    }

    /// Gets the verbosity of the shell.
    pub fn verbosity(&self) -> Verbosity {
        self.verbosity
    }

    /// Updates the color choice (always, never, or auto) from a string..
    pub fn set_color_choice(&mut self, color: Option<&str>) -> CargoResult<()> {
        if let ShellOut::Stream {
            stdout,
            stderr,
            color_choice,
            ..
        } = &mut self.output
        {
            let cfg = color
                .map(|c| c.parse())
                .transpose()?
                .unwrap_or(ColorChoice::CargoAuto);
            *color_choice = cfg;
            let stdout_choice = cfg.to_anstream_color_choice();
            let stderr_choice = cfg.to_anstream_color_choice();
            *stdout = AutoStream::new(std::io::stdout(), stdout_choice);
            *stderr = AutoStream::new(std::io::stderr(), stderr_choice);
        }
        Ok(())
    }

    pub fn set_unicode(&mut self, yes: bool) -> CargoResult<()> {
        if let ShellOut::Stream {
            stdout_unicode,
            stderr_unicode,
            ..
        } = &mut self.output
        {
            *stdout_unicode = yes;
            *stderr_unicode = yes;
        }
        Ok(())
    }

    pub fn set_hyperlinks(&mut self, yes: bool) -> CargoResult<()> {
        if let ShellOut::Stream { hyperlinks, .. } = &mut self.output {
            *hyperlinks = yes;
        }
        Ok(())
    }

    pub fn out_unicode(&self) -> bool {
        match &self.output {
            ShellOut::Write(_) => true,
            ShellOut::Stream { stdout_unicode, .. } => *stdout_unicode,
        }
    }

    pub fn err_unicode(&self) -> bool {
        match &self.output {
            ShellOut::Write(_) => true,
            ShellOut::Stream { stderr_unicode, .. } => *stderr_unicode,
        }
    }

    /// Gets the current color choice.
    ///
    /// If we are not using a color stream, this will always return `Never`, even if the color
    /// choice has been set to something else.
    pub fn color_choice(&self) -> ColorChoice {
        match self.output {
            ShellOut::Stream { color_choice, .. } => color_choice,
            ShellOut::Write(_) => ColorChoice::Never,
        }
    }

    /// Whether the shell supports color.
    pub fn err_supports_color(&self) -> bool {
        match &self.output {
            ShellOut::Write(_) => false,
            ShellOut::Stream { stderr, .. } => supports_color(stderr.current_choice()),
        }
    }

    pub fn out_supports_color(&self) -> bool {
        match &self.output {
            ShellOut::Write(_) => false,
            ShellOut::Stream { stdout, .. } => supports_color(stdout.current_choice()),
        }
    }

    pub fn out_hyperlink<D: fmt::Display>(&self, url: D) -> Hyperlink<D> {
        let supports_hyperlinks = match &self.output {
            ShellOut::Write(_) => false,
            ShellOut::Stream {
                stdout, hyperlinks, ..
            } => stdout.current_choice() == anstream::ColorChoice::AlwaysAnsi && *hyperlinks,
        };
        Hyperlink {
            url: supports_hyperlinks.then_some(url),
        }
    }

    pub fn err_hyperlink<D: fmt::Display>(&self, url: D) -> Hyperlink<D> {
        let supports_hyperlinks = match &self.output {
            ShellOut::Write(_) => false,
            ShellOut::Stream {
                stderr, hyperlinks, ..
            } => stderr.current_choice() == anstream::ColorChoice::AlwaysAnsi && *hyperlinks,
        };
        if supports_hyperlinks {
            Hyperlink { url: Some(url) }
        } else {
            Hyperlink { url: None }
        }
    }

    pub fn out_file_hyperlink(&mut self, path: &std::path::Path) -> Hyperlink<url::Url> {
        let url = self.file_hyperlink(path);
        url.map(|u| self.out_hyperlink(u)).unwrap_or_default()
    }

    pub fn err_file_hyperlink(&mut self, path: &std::path::Path) -> Hyperlink<url::Url> {
        let url = self.file_hyperlink(path);
        url.map(|u| self.err_hyperlink(u)).unwrap_or_default()
    }

    fn file_hyperlink(&mut self, path: &std::path::Path) -> Option<url::Url> {
        let mut url = url::Url::from_file_path(path).ok()?;
        // Do a best-effort of setting the host in the URL to avoid issues with opening a link
        // scoped to the computer you've SSHed into
        let hostname = if cfg!(windows) {
            // Not supported correctly on windows
            None
        } else {
            if let Some(hostname) = self.hostname.as_deref() {
                Some(hostname)
            } else {
                self.hostname = hostname().ok().and_then(|h| h.into_string().ok());
                self.hostname.as_deref()
            }
        };
        let _ = url.set_host(hostname);
        Some(url)
    }

    /// Prints a message to stderr and translates ANSI escape code into console colors.
    pub fn print_ansi_stderr(&mut self, message: &[u8]) -> CargoResult<()> {
        if self.needs_clear {
            self.err_erase_line();
        }
        self.err().write_all(message)?;
        Ok(())
    }

    /// Prints a message to stdout and translates ANSI escape code into console colors.
    pub fn print_ansi_stdout(&mut self, message: &[u8]) -> CargoResult<()> {
        if self.needs_clear {
            self.err_erase_line();
        }
        self.out().write_all(message)?;
        Ok(())
    }

    pub fn print_json<T: serde::ser::Serialize>(&mut self, obj: &T) -> CargoResult<()> {
        // Path may fail to serialize to JSON ...
        let encoded = serde_json::to_string(obj)?;
        // ... but don't fail due to a closed pipe.
        drop(writeln!(self.out(), "{}", encoded));
        Ok(())
    }

    /// Prints the passed in [`Report`] to stderr
    pub fn print_report(&mut self, report: Report<'_>, force: bool) -> CargoResult<()> {
        if !force && matches!(self.verbosity, Verbosity::Quiet) {
            return Ok(());
        }

        if self.needs_clear {
            self.err_erase_line();
        }
        let term_width = self
            .err_width()
            .diagnostic_terminal_width()
            .unwrap_or(annotate_snippets::renderer::DEFAULT_TERM_WIDTH);
        let rendered = Renderer::styled().term_width(term_width).render(report);
        self.err().write_all(rendered.as_bytes())?;
        self.err().write_all(b"\n")?;
        Ok(())
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

/// A `Write`able object, either with or without color support
enum ShellOut {
    /// A plain write object without color support
    Write(AutoStream<Box<dyn Write + Send + Sync>>),
    /// Color-enabled stdio, with information on whether color should be used
    Stream {
        stdout: AutoStream<std::io::Stdout>,
        stderr: AutoStream<std::io::Stderr>,
        stderr_tty: bool,
        color_choice: ColorChoice,
        hyperlinks: bool,
        stdout_unicode: bool,
        stderr_unicode: bool,
        stderr_term_integration: bool,
    },
}

impl ShellOut {
    /// Prints out a message with a status. The status comes first, and is bold plus the given
    /// color. The status can be justified, in which case the max width that will right align is
    /// 12 chars.
    fn message_stderr(
        &mut self,
        status: &dyn fmt::Display,
        message: Option<&dyn fmt::Display>,
        style: &Style,
        justified: bool,
    ) -> CargoResult<()> {
        let mut buffer = Vec::new();
        if justified {
            write!(&mut buffer, "{style}{status:>12}{style:#}")?;
        } else {
            write!(&mut buffer, "{style}{status}{style:#}:")?;
        }
        match message {
            Some(message) => writeln!(buffer, " {message}")?,
            None => write!(buffer, " ")?,
        }
        self.stderr().write_all(&buffer)?;
        Ok(())
    }

    /// Gets stdout as a `io::Write`.
    fn stdout(&mut self) -> &mut dyn Write {
        match self {
            ShellOut::Stream { stdout, .. } => stdout,
            ShellOut::Write(w) => w,
        }
    }

    /// Gets stderr as a `io::Write`.
    fn stderr(&mut self) -> &mut dyn Write {
        match self {
            ShellOut::Stream { stderr, .. } => stderr,
            ShellOut::Write(w) => w,
        }
    }
}

pub enum TtyWidth {
    NoTty,
    Known(usize),
    Guess(usize),
}

impl TtyWidth {
    /// Returns the width of the terminal to use for diagnostics (which is
    /// relayed to rustc via `--diagnostic-width`).
    pub fn diagnostic_terminal_width(&self) -> Option<usize> {
        // ALLOWED: For testing cargo itself only.
        #[allow(clippy::disallowed_methods)]
        if let Ok(width) = std::env::var("__CARGO_TEST_TTY_WIDTH_DO_NOT_USE_THIS") {
            return Some(width.parse().unwrap());
        }
        match *self {
            TtyWidth::NoTty | TtyWidth::Guess(_) => None,
            TtyWidth::Known(width) => Some(width),
        }
    }

    /// Returns the width used by progress bars for the tty.
    pub fn progress_max_width(&self) -> Option<usize> {
        match *self {
            TtyWidth::NoTty => None,
            TtyWidth::Known(width) | TtyWidth::Guess(width) => Some(width),
        }
    }
}

/// The requested verbosity of output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet,
}

/// Whether messages should use color output
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ColorChoice {
    /// Force color output
    Always,
    /// Force disable color output
    Never,
    /// Intelligently guess whether to use color output
    CargoAuto,
}

impl ColorChoice {
    /// Converts our color choice to anstream's version.
    fn to_anstream_color_choice(self) -> anstream::ColorChoice {
        match self {
            ColorChoice::Always => anstream::ColorChoice::Always,
            ColorChoice::Never => anstream::ColorChoice::Never,
            ColorChoice::CargoAuto => anstream::ColorChoice::Auto,
        }
    }
}

impl std::str::FromStr for ColorChoice {
    type Err = anyhow::Error;
    fn from_str(color: &str) -> Result<Self, Self::Err> {
        let cfg = match color {
            "always" => ColorChoice::Always,
            "never" => ColorChoice::Never,

            "auto" => ColorChoice::CargoAuto,

            arg => anyhow::bail!(
                "argument for --color must be auto, always, or \
                     never, but found `{}`",
                arg
            ),
        };
        Ok(cfg)
    }
}

fn supports_color(choice: anstream::ColorChoice) -> bool {
    match choice {
        anstream::ColorChoice::Always
        | anstream::ColorChoice::AlwaysAnsi
        | anstream::ColorChoice::Auto => true,
        anstream::ColorChoice::Never => false,
    }
}

fn supports_unicode(stream: &dyn IsTerminal) -> bool {
    !stream.is_terminal() || supports_unicode::supports_unicode()
}

fn supports_hyperlinks() -> bool {
    #[allow(clippy::disallowed_methods)] // We are reading the state of the system, not config
    if std::env::var_os("TERM_PROGRAM").as_deref() == Some(std::ffi::OsStr::new("iTerm.app")) {
        // Override `supports_hyperlinks` as we have an unknown incompatibility with iTerm2
        return false;
    }

    supports_hyperlinks::supports_hyperlinks()
}

/// Determines whether the terminal supports ANSI OSC 9;4.
#[allow(clippy::disallowed_methods)] // Read environment variables to detect terminal
fn supports_term_integration(stream: &dyn IsTerminal) -> bool {
    let windows_terminal = std::env::var("WT_SESSION").is_ok();
    let conemu = std::env::var("ConEmuANSI").ok() == Some("ON".into());
    let wezterm = std::env::var("TERM_PROGRAM").ok() == Some("WezTerm".into());
    let ghostty = std::env::var("TERM_PROGRAM").ok() == Some("ghostty".into());

    (windows_terminal || conemu || wezterm || ghostty) && stream.is_terminal()
}

pub struct Hyperlink<D: fmt::Display> {
    url: Option<D>,
}

impl<D: fmt::Display> Default for Hyperlink<D> {
    fn default() -> Self {
        Self { url: None }
    }
}

impl<D: fmt::Display> fmt::Display for Hyperlink<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(url) = self.url.as_ref() else {
            return Ok(());
        };
        if f.alternate() {
            write!(f, "\x1B]8;;\x1B\\")
        } else {
            write!(f, "\x1B]8;;{url}\x1B\\")
        }
    }
}

#[cfg(unix)]
mod imp {
    use super::{Shell, TtyWidth};
    use std::mem;

    pub fn stderr_width() -> TtyWidth {
        unsafe {
            let mut winsize: libc::winsize = mem::zeroed();
            // The .into() here is needed for FreeBSD which defines TIOCGWINSZ
            // as c_uint but ioctl wants c_ulong.
            if libc::ioctl(libc::STDERR_FILENO, libc::TIOCGWINSZ.into(), &mut winsize) < 0 {
                return TtyWidth::NoTty;
            }
            if winsize.ws_col > 0 {
                TtyWidth::Known(winsize.ws_col as usize)
            } else {
                TtyWidth::NoTty
            }
        }
    }

    pub fn err_erase_line(shell: &mut Shell) {
        // This is the "EL - Erase in Line" sequence. It clears from the cursor
        // to the end of line.
        // https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_sequences
        let _ = shell.output.stderr().write_all(b"\x1B[K");
    }
}

#[cfg(windows)]
mod imp {
    use std::{cmp, mem, ptr};

    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileA, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        CONSOLE_SCREEN_BUFFER_INFO, GetConsoleScreenBufferInfo, GetStdHandle, STD_ERROR_HANDLE,
    };
    use windows_sys::core::PCSTR;

    pub(super) use super::{TtyWidth, default_err_erase_line as err_erase_line};

    pub fn stderr_width() -> TtyWidth {
        unsafe {
            let stdout = GetStdHandle(STD_ERROR_HANDLE);
            let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = mem::zeroed();
            if GetConsoleScreenBufferInfo(stdout, &mut csbi) != 0 {
                return TtyWidth::Known((csbi.srWindow.Right - csbi.srWindow.Left) as usize);
            }

            // On mintty/msys/cygwin based terminals, the above fails with
            // INVALID_HANDLE_VALUE. Use an alternate method which works
            // in that case as well.
            let h = CreateFileA(
                "CONOUT$\0".as_ptr() as PCSTR,
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            );
            if h == INVALID_HANDLE_VALUE {
                return TtyWidth::NoTty;
            }

            let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = mem::zeroed();
            let rc = GetConsoleScreenBufferInfo(h, &mut csbi);
            CloseHandle(h);
            if rc != 0 {
                let width = (csbi.srWindow.Right - csbi.srWindow.Left) as usize;
                // Unfortunately cygwin/mintty does not set the size of the
                // backing console to match the actual window size. This
                // always reports a size of 80 or 120 (not sure what
                // determines that). Use a conservative max of 60 which should
                // work in most circumstances. ConEmu does some magic to
                // resize the console correctly, but there's no reasonable way
                // to detect which kind of terminal we are running in, or if
                // GetConsoleScreenBufferInfo returns accurate information.
                return TtyWidth::Guess(cmp::min(60, width));
            }

            TtyWidth::NoTty
        }
    }
}

#[cfg(windows)]
fn default_err_erase_line(shell: &mut Shell) {
    match imp::stderr_width() {
        TtyWidth::Known(max_width) | TtyWidth::Guess(max_width) => {
            let blank = " ".repeat(max_width);
            drop(write!(shell.output.stderr(), "{}\r", blank));
        }
        _ => (),
    }
}
