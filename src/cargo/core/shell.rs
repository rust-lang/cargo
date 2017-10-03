use std::fmt;
use std::io::prelude::*;

use atty;
use termcolor::Color::{Green, Red, Yellow};
use termcolor::{self, StandardStream, Color, ColorSpec, WriteColor};

use util::errors::CargoResult;

/// The requested verbosity of output
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet
}

/// An abstraction around a `Write`able object that remembers preferences for output verbosity and
/// color.
pub struct Shell {
    /// the `Write`able object, either with or without color support (represented by different enum
    /// variants)
    err: ShellOut,
    /// How verbose messages should be
    verbosity: Verbosity,
}

impl fmt::Debug for Shell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.err {
            &ShellOut::Write(_) => f.debug_struct("Shell")
                .field("verbosity", &self.verbosity)
                .finish(),
            &ShellOut::Stream(_, color_choice) => f.debug_struct("Shell")
                .field("verbosity", &self.verbosity)
                .field("color_choice", &color_choice)
                .finish()
        }
    }
}

/// A `Write`able object, either with or without color support
enum ShellOut {
    /// A plain write object without color support
    Write(Box<Write>),
    /// Color-enabled stdio, with information on whether color should be used
    Stream(StandardStream, ColorChoice),
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
    /// Create a new shell (color choice and verbosity), defaulting to 'auto' color and verbose
    /// output.
    pub fn new() -> Shell {
        Shell {
            err: ShellOut::Stream(
                StandardStream::stderr(ColorChoice::CargoAuto.to_termcolor_color_choice()),
                ColorChoice::CargoAuto,
            ),
            verbosity: Verbosity::Verbose,
        }
    }

    /// Create a shell from a plain writable object, with no color, and max verbosity.
    pub fn from_write(out: Box<Write>) -> Shell {
        Shell {
            err: ShellOut::Write(out),
            verbosity: Verbosity::Verbose,
        }
    }

    /// Print a message, where the status will have `color` color, and can be justified. The
    /// messages follows without color.
    fn print(&mut self,
             status: &fmt::Display,
             message: &fmt::Display,
             color: Color,
             justified: bool) -> CargoResult<()> {
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => {
                self.err.print(status, message, color, justified)
            }
        }
    }

    /// Get a reference to the underlying writer
    pub fn err(&mut self) -> &mut Write {
        self.err.as_write()
    }

    /// Shortcut to right-align and color green a status message.
    pub fn status<T, U>(&mut self, status: T, message: U) -> CargoResult<()>
        where T: fmt::Display, U: fmt::Display
    {
        self.print(&status, &message, Green, true)
    }

    /// Shortcut to right-align a status message.
    pub fn status_with_color<T, U>(&mut self,
                                   status: T,
                                   message: U,
                                   color: Color) -> CargoResult<()>
        where T: fmt::Display, U: fmt::Display
    {
        self.print(&status, &message, color, true)
    }

    /// Run the callback only if we are in verbose mode
    pub fn verbose<F>(&mut self, mut callback: F) -> CargoResult<()>
        where F: FnMut(&mut Shell) -> CargoResult<()>
    {
        match self.verbosity {
            Verbosity::Verbose => callback(self),
            _ => Ok(())
        }
    }

    /// Run the callback if we are not in verbose mode.
    pub fn concise<F>(&mut self, mut callback: F) -> CargoResult<()>
        where F: FnMut(&mut Shell) -> CargoResult<()>
    {
        match self.verbosity {
            Verbosity::Verbose => Ok(()),
            _ => callback(self)
        }
    }

    /// Print a red 'error' message
    pub fn error<T: fmt::Display>(&mut self, message: T) -> CargoResult<()> {
        self.print(&"error:", &message, Red, false)
    }

    /// Print an amber 'warning' message
    pub fn warn<T: fmt::Display>(&mut self, message: T) -> CargoResult<()> {
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => self.print(&"warning:", &message, Yellow, false),
        }
    }

    /// Update the verbosity of the shell
    pub fn set_verbosity(&mut self, verbosity: Verbosity) {
        self.verbosity = verbosity;
    }

    /// Get the verbosity of the shell
    pub fn verbosity(&self) -> Verbosity {
        self.verbosity
    }

    /// Update the color choice (always, never, or auto) from a string.
    pub fn set_color_choice(&mut self, color: Option<&str>) -> CargoResult<()> {
        if let ShellOut::Stream(ref mut err, ref mut cc) =  self.err {
            let cfg = match color {
                Some("always") => ColorChoice::Always,
                Some("never") => ColorChoice::Never,

                Some("auto") |
                None => ColorChoice::CargoAuto,

                Some(arg) => bail!("argument for --color must be auto, always, or \
                                    never, but found `{}`", arg),
            };
            *cc = cfg;
            *err = StandardStream::stderr(cfg.to_termcolor_color_choice());
        }
        Ok(())
    }

    /// Get the current color choice
    ///
    /// If we are not using a color stream, this will always return Never, even if the color choice
    /// has been set to something else.
    pub fn color_choice(&self) -> ColorChoice {
        match self.err {
            ShellOut::Stream(_, cc) => cc,
            ShellOut::Write(_) => ColorChoice::Never,
        }
    }
}

impl ShellOut {
    /// Print out a message with a status. The status comes first and is bold + the given color.
    /// The status can be justified, in which case the max width that will right align is 12 chars.
    fn print(&mut self,
             status: &fmt::Display,
             message: &fmt::Display,
             color: Color,
             justified: bool) -> CargoResult<()> {
        match *self {
            ShellOut::Stream(ref mut err, _) => {
                err.reset()?;
                err.set_color(ColorSpec::new()
                                    .set_bold(true)
                                    .set_fg(Some(color)))?;
                if justified {
                    write!(err, "{:>12}", status)?;
                } else {
                    write!(err, "{}", status)?;
                }
                err.reset()?;
                write!(err, " {}\n", message)?;
            }
            ShellOut::Write(ref mut w) => {
                if justified {
                    write!(w, "{:>12}", status)?;
                } else {
                    write!(w, "{}", status)?;
                }
                write!(w, " {}\n", message)?;
            }
        }
        Ok(())
    }

    /// Get this object as a `io::Write`.
    fn as_write(&mut self) -> &mut Write {
        match *self {
            ShellOut::Stream(ref mut err, _) => err,
            ShellOut::Write(ref mut w) => w,
        }
    }
}

impl ColorChoice {
    /// Convert our color choice to termcolor's version
    fn to_termcolor_color_choice(&self) -> termcolor::ColorChoice {
        match *self {
            ColorChoice::Always => termcolor::ColorChoice::Always,
            ColorChoice::Never => termcolor::ColorChoice::Never,
            ColorChoice::CargoAuto => {
                if atty::is(atty::Stream::Stderr) {
                    termcolor::ColorChoice::Auto
                } else {
                    termcolor::ColorChoice::Never
                }
            }
        }
    }
}
