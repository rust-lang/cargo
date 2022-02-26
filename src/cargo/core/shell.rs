use std::fmt;
use std::io::prelude::*;

use termcolor::Color::{Cyan, Green, Red, Yellow};
use termcolor::{self, Color, ColorSpec, StandardStream, WriteColor};

use crate::util::errors::CargoResult;

pub enum TtyWidth {
    NoTty,
    Known(usize),
    Guess(usize),
}

impl TtyWidth {
    /// Returns the width provided with `-Z terminal-width` to rustc to truncate diagnostics with
    /// long lines.
    pub fn diagnostic_terminal_width(&self) -> Option<usize> {
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

/// A `Write`able object, either with or without color support
enum ShellOut {
    /// A plain write object without color support
    Write(Box<dyn Write>),
    /// Color-enabled stdio, with information on whether color should be used
    Stream {
        stdout: StandardStream,
        stderr: StandardStream,
        stderr_tty: bool,
        color_choice: ColorChoice,
    },
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

impl Shell {
    /// Creates a new shell (color choice and verbosity), defaulting to 'auto' color and verbose
    /// output.
    pub fn new() -> Shell {
        let auto_clr = ColorChoice::CargoAuto;
        Shell {
            output: ShellOut::Stream {
                stdout: StandardStream::stdout(
                    auto_clr.to_termcolor_color_choice(atty::Stream::Stdout),
                ),
                stderr: StandardStream::stderr(
                    auto_clr.to_termcolor_color_choice(atty::Stream::Stderr),
                ),
                color_choice: ColorChoice::CargoAuto,
                stderr_tty: atty::is(atty::Stream::Stderr),
            },
            verbosity: Verbosity::Verbose,
            needs_clear: false,
        }
    }

    /// Creates a shell from a plain writable object, with no color, and max verbosity.
    pub fn from_write(out: Box<dyn Write>) -> Shell {
        Shell {
            output: ShellOut::Write(out),
            verbosity: Verbosity::Verbose,
            needs_clear: false,
        }
    }

    /// Prints a message, where the status will have `color` color, and can be justified. The
    /// messages follows without color.
    fn print(
        &mut self,
        status: &dyn fmt::Display,
        message: Option<&dyn fmt::Display>,
        color: Color,
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
        self.print(&status, Some(&message), Green, true)
    }

    pub fn status_header<T>(&mut self, status: T) -> CargoResult<()>
    where
        T: fmt::Display,
    {
        self.print(&status, None, Cyan, true)
    }

    /// Shortcut to right-align a status message.
    pub fn status_with_color<T, U>(
        &mut self,
        status: T,
        message: U,
        color: Color,
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
            .message_stderr(&"error", Some(&message), Red, false)
    }

    /// Prints an amber 'warning' message.
    pub fn warn<T: fmt::Display>(&mut self, message: T) -> CargoResult<()> {
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => self.print(&"warning", Some(&message), Yellow, false),
        }
    }

    /// Prints a cyan 'note' message.
    pub fn note<T: fmt::Display>(&mut self, message: T) -> CargoResult<()> {
        self.print(&"note", Some(&message), Cyan, false)
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
            ref mut stdout,
            ref mut stderr,
            ref mut color_choice,
            ..
        } = self.output
        {
            let cfg = match color {
                Some("always") => ColorChoice::Always,
                Some("never") => ColorChoice::Never,

                Some("auto") | None => ColorChoice::CargoAuto,

                Some(arg) => anyhow::bail!(
                    "argument for --color must be auto, always, or \
                     never, but found `{}`",
                    arg
                ),
            };
            *color_choice = cfg;
            *stdout = StandardStream::stdout(cfg.to_termcolor_color_choice(atty::Stream::Stdout));
            *stderr = StandardStream::stderr(cfg.to_termcolor_color_choice(atty::Stream::Stderr));
        }
        Ok(())
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
            ShellOut::Stream { stderr, .. } => stderr.supports_color(),
        }
    }

    pub fn out_supports_color(&self) -> bool {
        match &self.output {
            ShellOut::Write(_) => false,
            ShellOut::Stream { stdout, .. } => stdout.supports_color(),
        }
    }

    /// Write a styled fragment
    ///
    /// Caller is responsible for deciding whether [`Shell::verbosity`] is affects output.
    pub fn write_stdout(
        &mut self,
        fragment: impl fmt::Display,
        color: &ColorSpec,
    ) -> CargoResult<()> {
        self.output.write_stdout(fragment, color)
    }

    /// Prints a message to stderr and translates ANSI escape code into console colors.
    pub fn print_ansi_stderr(&mut self, message: &[u8]) -> CargoResult<()> {
        if self.needs_clear {
            self.err_erase_line();
        }
        #[cfg(windows)]
        {
            if let ShellOut::Stream { stderr, .. } = &mut self.output {
                ::fwdansi::write_ansi(stderr, message)?;
                return Ok(());
            }
        }
        self.err().write_all(message)?;
        Ok(())
    }

    /// Prints a message to stdout and translates ANSI escape code into console colors.
    pub fn print_ansi_stdout(&mut self, message: &[u8]) -> CargoResult<()> {
        if self.needs_clear {
            self.err_erase_line();
        }
        #[cfg(windows)]
        {
            if let ShellOut::Stream { stdout, .. } = &mut self.output {
                ::fwdansi::write_ansi(stdout, message)?;
                return Ok(());
            }
        }
        self.out().write_all(message)?;
        Ok(())
    }

    pub fn print_json<T: serde::ser::Serialize>(&mut self, obj: &T) -> CargoResult<()> {
        // Path may fail to serialize to JSON ...
        let encoded = serde_json::to_string(&obj)?;
        // ... but don't fail due to a closed pipe.
        drop(writeln!(self.out(), "{}", encoded));
        Ok(())
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellOut {
    /// Prints out a message with a status. The status comes first, and is bold plus the given
    /// color. The status can be justified, in which case the max width that will right align is
    /// 12 chars.
    fn message_stderr(
        &mut self,
        status: &dyn fmt::Display,
        message: Option<&dyn fmt::Display>,
        color: Color,
        justified: bool,
    ) -> CargoResult<()> {
        match *self {
            ShellOut::Stream { ref mut stderr, .. } => {
                stderr.reset()?;
                stderr.set_color(ColorSpec::new().set_bold(true).set_fg(Some(color)))?;
                if justified {
                    write!(stderr, "{:>12}", status)?;
                } else {
                    write!(stderr, "{}", status)?;
                    stderr.set_color(ColorSpec::new().set_bold(true))?;
                    write!(stderr, ":")?;
                }
                stderr.reset()?;
                match message {
                    Some(message) => writeln!(stderr, " {}", message)?,
                    None => write!(stderr, " ")?,
                }
            }
            ShellOut::Write(ref mut w) => {
                if justified {
                    write!(w, "{:>12}", status)?;
                } else {
                    write!(w, "{}:", status)?;
                }
                match message {
                    Some(message) => writeln!(w, " {}", message)?,
                    None => write!(w, " ")?,
                }
            }
        }
        Ok(())
    }

    /// Write a styled fragment
    fn write_stdout(&mut self, fragment: impl fmt::Display, color: &ColorSpec) -> CargoResult<()> {
        match *self {
            ShellOut::Stream { ref mut stdout, .. } => {
                stdout.reset()?;
                stdout.set_color(&color)?;
                write!(stdout, "{}", fragment)?;
                stdout.reset()?;
            }
            ShellOut::Write(ref mut w) => {
                write!(w, "{}", fragment)?;
            }
        }
        Ok(())
    }

    /// Gets stdout as a `io::Write`.
    fn stdout(&mut self) -> &mut dyn Write {
        match *self {
            ShellOut::Stream { ref mut stdout, .. } => stdout,
            ShellOut::Write(ref mut w) => w,
        }
    }

    /// Gets stderr as a `io::Write`.
    fn stderr(&mut self) -> &mut dyn Write {
        match *self {
            ShellOut::Stream { ref mut stderr, .. } => stderr,
            ShellOut::Write(ref mut w) => w,
        }
    }
}

impl ColorChoice {
    /// Converts our color choice to termcolor's version.
    fn to_termcolor_color_choice(self, stream: atty::Stream) -> termcolor::ColorChoice {
        match self {
            ColorChoice::Always => termcolor::ColorChoice::Always,
            ColorChoice::Never => termcolor::ColorChoice::Never,
            ColorChoice::CargoAuto => {
                if atty::is(stream) {
                    termcolor::ColorChoice::Auto
                } else {
                    termcolor::ColorChoice::Never
                }
            }
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
    use winapi::um::fileapi::*;
    use winapi::um::handleapi::*;
    use winapi::um::processenv::*;
    use winapi::um::winbase::*;
    use winapi::um::wincon::*;
    use winapi::um::winnt::*;

    pub(super) use super::{default_err_erase_line as err_erase_line, TtyWidth};

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
                "CONOUT$\0".as_ptr() as *const CHAR,
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
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
