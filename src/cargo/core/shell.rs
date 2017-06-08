use std::fmt;
use std::io::prelude::*;

use termcolor::{self, StandardStream, Color, ColorSpec, WriteColor};
use termcolor::Color::{Green, Red, Yellow};

use util::errors::CargoResult;

#[derive(Clone, Copy, PartialEq)]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet
}

pub struct Shell {
    err: StandardStream,
    verbosity: Verbosity,
    choice: ColorChoice,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ColorChoice {
    Always,
    Never,
    CargoAuto,
}

impl Shell {
    pub fn new() -> Shell {
        Shell {
            err: StandardStream::stderr(ColorChoice::CargoAuto.to_termcolor_color_choice()),
            verbosity: Verbosity::Verbose,
            choice: ColorChoice::CargoAuto,
        }
    }

    fn print(&mut self,
             status: &fmt::Display,
             message: &fmt::Display,
             color: Color,
             justified: bool) -> CargoResult<()> {
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => {
                self.err.reset()?;
                self.err.set_color(ColorSpec::new()
                                        .set_bold(true)
                                        .set_fg(Some(color)))?;
                if justified {
                    write!(self.err, "{:>12}", status)?;
                } else {
                    write!(self.err, "{}", status)?;
                }
                self.err.reset()?;
                write!(self.err, " {}\n", message)?;
                Ok(())
            }
        }
    }

    pub fn err(&mut self) -> &mut StandardStream {
        &mut self.err
    }

    pub fn status<T, U>(&mut self, status: T, message: U) -> CargoResult<()>
        where T: fmt::Display, U: fmt::Display
    {
        self.print(&status, &message, Green, true)
    }

    pub fn status_with_color<T, U>(&mut self,
                                   status: T,
                                   message: U,
                                   color: Color) -> CargoResult<()>
        where T: fmt::Display, U: fmt::Display
    {
        self.print(&status, &message, color, true)
    }

    pub fn verbose<F>(&mut self, mut callback: F) -> CargoResult<()>
        where F: FnMut(&mut Shell) -> CargoResult<()>
    {
        match self.verbosity {
            Verbosity::Verbose => callback(self),
            _ => Ok(())
        }
    }

    pub fn concise<F>(&mut self, mut callback: F) -> CargoResult<()>
        where F: FnMut(&mut Shell) -> CargoResult<()>
    {
        match self.verbosity {
            Verbosity::Verbose => Ok(()),
            _ => callback(self)
        }
    }

    pub fn error<T: fmt::Display>(&mut self, message: T) -> CargoResult<()> {
        self.print(&"error:", &message, Red, false)
    }

    pub fn warn<T: fmt::Display>(&mut self, message: T) -> CargoResult<()> {
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => self.print(&"warning:", &message, Yellow, false),
        }
    }

    pub fn set_verbosity(&mut self, verbosity: Verbosity) {
        self.verbosity = verbosity;
    }

    pub fn verbosity(&self) -> Verbosity {
        self.verbosity
    }

    pub fn set_color_choice(&mut self, color: Option<&str>) -> CargoResult<()> {
        let cfg = match color {
            Some("always") => ColorChoice::Always,
            Some("never") => ColorChoice::Never,

            Some("auto") |
            None => ColorChoice::CargoAuto,

            Some(arg) => bail!("argument for --color must be auto, always, or \
                                never, but found `{}`", arg),
        };
        self.choice = cfg;
        self.err = StandardStream::stderr(cfg.to_termcolor_color_choice());
        return Ok(());
    }

    pub fn color_choice(&self) -> ColorChoice {
        self.choice
    }
}

impl ColorChoice {
    fn to_termcolor_color_choice(&self) -> termcolor::ColorChoice {
        return match *self {
            ColorChoice::Always => termcolor::ColorChoice::Always,
            ColorChoice::Never => termcolor::ColorChoice::Never,
            ColorChoice::CargoAuto if isatty() => termcolor::ColorChoice::Auto,
            ColorChoice::CargoAuto => termcolor::ColorChoice::Never,
        };

        #[cfg(unix)]
        fn isatty() -> bool {
            extern crate libc;

            unsafe { libc::isatty(libc::STDERR_FILENO) != 0 }
        }

        #[cfg(windows)]
        fn isatty() -> bool {
            extern crate kernel32;
            extern crate winapi;

            unsafe {
                let handle = kernel32::GetStdHandle(winapi::STD_ERROR_HANDLE);
                let mut out = 0;
                kernel32::GetConsoleMode(handle, &mut out) != 0
            }
        }
    }
}
