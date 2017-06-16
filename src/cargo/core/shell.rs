use std::fmt;
use std::io::prelude::*;

use atty;
use termcolor::Color::{Green, Red, Yellow};
use termcolor::{self, StandardStream, Color, ColorSpec, WriteColor};

use util::errors::CargoResult;

#[derive(Clone, Copy, PartialEq)]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet
}

pub struct Shell {
    err: ShellOut,
    verbosity: Verbosity,
}

enum ShellOut {
    Write(Box<Write>),
    Stream(StandardStream, ColorChoice),
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
            err: ShellOut::Stream(
                StandardStream::stderr(ColorChoice::CargoAuto.to_termcolor_color_choice()),
                ColorChoice::CargoAuto,
            ),
            verbosity: Verbosity::Verbose,
        }
    }

    pub fn from_write(out: Box<Write>) -> Shell {
        Shell {
            err: ShellOut::Write(out),
            verbosity: Verbosity::Verbose,
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
                self.err.print(status, message, color, justified)
            }
        }
    }

    pub fn err(&mut self) -> &mut Write {
        self.err.as_write()
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

    pub fn color_choice(&self) -> ColorChoice {
        match self.err {
            ShellOut::Stream(_, cc) => cc,
            ShellOut::Write(_) => ColorChoice::Never,
        }
    }
}

impl ShellOut {
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

    fn as_write(&mut self) -> &mut Write {
        match *self {
            ShellOut::Stream(ref mut err, _) => err,
            ShellOut::Write(ref mut w) => w,
        }
    }
}

impl ColorChoice {
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
