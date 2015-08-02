use std::fmt;
use std::io::prelude::*;
use std::io;

use term::Attr;
use term::color::{Color, BLACK, RED, GREEN, YELLOW};
use term::{Terminal, TerminfoTerminal, color};

use self::AdequateTerminal::{NoColor, Colored};
use self::Verbosity::{Verbose, Normal, Quiet};
use self::ColorConfig::{Auto, Always, Never};

use util::errors::{human, CargoResult};

#[derive(Clone, Copy, PartialEq)]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet
}

#[derive(Clone, Copy, PartialEq)]
pub enum ColorConfig {
    Auto,
    Always,
    Never
}

#[derive(Clone, Copy)]
pub struct ShellConfig {
    pub color_config: ColorConfig,
    pub tty: bool
}

enum AdequateTerminal {
    NoColor(Box<Write + Send>),
    Colored(Box<Terminal<Output=Box<Write + Send>> + Send>)
}

pub struct Shell {
    terminal: AdequateTerminal,
    config: ShellConfig,
}

pub struct MultiShell {
    out: Shell,
    err: Shell,
    verbosity: Verbosity
}

impl MultiShell {
    pub fn new(out: Shell, err: Shell, verbosity: Verbosity) -> MultiShell {
        MultiShell { out: out, err: err, verbosity: verbosity }
    }

    pub fn out(&mut self) -> &mut Shell {
        &mut self.out
    }

    pub fn err(&mut self) -> &mut Shell {
        &mut self.err
    }

    pub fn say<T: ToString>(&mut self, message: T, color: Color) -> io::Result<()> {
        match self.verbosity {
            Quiet => Ok(()),
            _ => self.out().say(message, color)
        }
    }

    pub fn status<T, U>(&mut self, status: T, message: U) -> io::Result<()>
        where T: fmt::Display, U: fmt::Display
    {
        match self.verbosity {
            Quiet => Ok(()),
            _ => self.out().say_status(status, message, GREEN)
        }
    }

    pub fn verbose<F>(&mut self, mut callback: F) -> io::Result<()>
        where F: FnMut(&mut MultiShell) -> io::Result<()>
    {
        match self.verbosity {
            Verbose => return callback(self),
            _ => Ok(())
        }
    }

    pub fn concise<F>(&mut self, mut callback: F) -> io::Result<()>
        where F: FnMut(&mut MultiShell) -> io::Result<()>
    {
        match self.verbosity {
            Verbose => Ok(()),
            _ => return callback(self)
        }
    }

    pub fn error<T: ToString>(&mut self, message: T) -> io::Result<()> {
        self.err().say(message, RED)
    }

    pub fn warn<T: ToString>(&mut self, message: T) -> io::Result<()> {
        self.err().say(message, YELLOW)
    }

    pub fn set_verbosity(&mut self, verbose: bool, quiet: bool) -> CargoResult<()> {
        self.verbosity = match (verbose, quiet) {
            (true, true) => return Err(human("cannot set both --verbose and --quiet")),
            (true, false) => Verbose,
            (false, true) => Quiet,
            (false, false) => Normal
        };
        Ok(())
    }

    /// shortcut for commands that don't have both --verbose and --quiet
    pub fn set_verbose(&mut self, verbose: bool) {
        if verbose {
            self.verbosity = Verbose;
        } else {
            self.verbosity = Normal;
        }
    }

    pub fn set_color_config(&mut self, color: Option<&str>) -> CargoResult<()> {
        self.out.set_color_config(match color {
            Some("auto") => Auto,
            Some("always") => Always,
            Some("never") => Never,

            None => Auto,

            Some(arg) => return Err(human(format!("argument for --color must be auto, always, or \
                                                   never, but found `{}`",
                                                  arg))),
        });
        Ok(())
    }

    pub fn get_verbose(&self) -> Verbosity {
        self.verbosity
    }
}

impl Shell {
    pub fn create(out: Box<Write + Send>, config: ShellConfig) -> Shell {
        match ::term::terminfo::TermInfo::from_env() {
            Ok(ti) => {
                // Color output is possible.
                Shell {
                    terminal: Colored(Box::new(TerminfoTerminal::new_with_terminfo(out, ti))),
                    config: config
                }
            }
            _ if config.tty => {
                // Color output is expected but not available, fall back to stderr.
                Shell { terminal: NoColor(Box::new(io::stderr())), config: config }
            }
            _ => {
                // No color output.
                Shell { terminal: NoColor(out), config: config }
            }
        }
    }

    pub fn set_color_config(&mut self, color_config: ColorConfig) {
        self.config.color_config = color_config;
    }

    pub fn say<T: ToString>(&mut self, message: T, color: Color) -> io::Result<()> {
        try!(self.reset());
        if color != BLACK { try!(self.fg(color)); }
        try!(write!(self, "{}\n", message.to_string()));
        try!(self.reset());
        try!(self.flush());
        Ok(())
    }

    pub fn say_status<T, U>(&mut self, status: T, message: U, color: Color)
                            -> io::Result<()>
        where T: fmt::Display, U: fmt::Display
    {
        try!(self.reset());
        if color != BLACK { try!(self.fg(color)); }
        if self.supports_attr(Attr::Bold) { try!(self.attr(Attr::Bold)); }
        try!(write!(self, "{:>12}", status.to_string()));
        try!(self.reset());
        try!(write!(self, " {}\n", message));
        try!(self.flush());
        Ok(())
    }

    fn fg(&mut self, color: color::Color) -> io::Result<bool> {
        let colored = self.colored();

        match self.terminal {
            Colored(ref mut c) if colored => c.fg(color),
            _ => Ok(false)
        }
    }

    fn attr(&mut self, attr: Attr) -> io::Result<bool> {
        let colored = self.colored();

        match self.terminal {
            Colored(ref mut c) if colored => c.attr(attr),
            _ => Ok(false)
        }
    }

    fn supports_attr(&self, attr: Attr) -> bool {
        let colored = self.colored();

        match self.terminal {
            Colored(ref c) if colored => c.supports_attr(attr),
            _ => false
        }
    }

    fn reset(&mut self) -> io::Result<()> {
        let colored = self.colored();

        match self.terminal {
            Colored(ref mut c) if colored => c.reset().map(|_| ()),
            _ => Ok(())
        }
    }

    fn colored(&self) -> bool {
        self.config.tty && Auto == self.config.color_config
            || Always == self.config.color_config
    }
}

impl Write for Shell {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.terminal {
            Colored(ref mut c) => c.write(buf),
            NoColor(ref mut n) => n.write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.terminal {
            Colored(ref mut c) => c.flush(),
            NoColor(ref mut n) => n.flush()
        }
    }
}
