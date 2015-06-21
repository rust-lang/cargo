use std::fmt;
use std::io::prelude::*;
use std::io;

use term::Attr;
use term::color::{Color, BLACK, RED, GREEN, YELLOW};
use term::{Terminal, TerminfoTerminal, color};

use self::AdequateTerminal::{NoColor, Colored};

#[derive(Clone, Copy)]
pub struct ShellConfig {
    pub color: bool,
    pub verbose: bool,
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
    verbose: bool
}

impl MultiShell {
    pub fn new(out: Shell, err: Shell, verbose: bool) -> MultiShell {
        MultiShell { out: out, err: err, verbose: verbose }
    }

    pub fn out(&mut self) -> &mut Shell {
        &mut self.out
    }

    pub fn err(&mut self) -> &mut Shell {
        &mut self.err
    }

    pub fn say<T: ToString>(&mut self, message: T, color: Color) -> io::Result<()> {
        self.out().say(message, color)
    }

    pub fn status<T, U>(&mut self, status: T, message: U) -> io::Result<()>
        where T: fmt::Display, U: fmt::Display
    {
        self.out().say_status(status, message, GREEN)
    }

    pub fn verbose<F>(&mut self, mut callback: F) -> io::Result<()>
        where F: FnMut(&mut MultiShell) -> io::Result<()>
    {
        if self.verbose { return callback(self) }
        Ok(())
    }

    pub fn concise<F>(&mut self, mut callback: F) -> io::Result<()>
        where F: FnMut(&mut MultiShell) -> io::Result<()>
    {
        if !self.verbose { return callback(self) }
        Ok(())
    }

    pub fn error<T: ToString>(&mut self, message: T) -> io::Result<()> {
        self.err().say(message, RED)
    }

    pub fn warn<T: ToString>(&mut self, message: T) -> io::Result<()> {
        self.err().say(message, YELLOW)
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    pub fn get_verbose(&self) -> bool {
        self.verbose
    }
}

impl Shell {
    pub fn create(out: Box<Write + Send>, config: ShellConfig) -> Shell {
        if config.tty && config.color {
            let term = TerminfoTerminal::new(out);
            term.map(|t| Shell {
                terminal: Colored(Box::new(t)),
                config: config
            }).unwrap_or_else(|| {
                Shell { terminal: NoColor(Box::new(io::stderr())), config: config }
            })
        } else {
            Shell { terminal: NoColor(out), config: config }
        }
    }

    pub fn verbose<F>(&mut self, mut callback: F) -> io::Result<()>
        where F: FnMut(&mut Shell) -> io::Result<()>
    {
        if self.config.verbose { return callback(self) }
        Ok(())
    }

    pub fn concise<F>(&mut self, mut callback: F) -> io::Result<()>
        where F: FnMut(&mut Shell) -> io::Result<()>
    {
        if !self.config.verbose { return callback(self) }
        Ok(())
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
        match self.terminal {
            Colored(ref mut c) => c.fg(color),
            NoColor(_) => Ok(false)
        }
    }

    fn attr(&mut self, attr: Attr) -> io::Result<bool> {
        match self.terminal {
            Colored(ref mut c) => c.attr(attr),
            NoColor(_) => Ok(false)
        }
    }

    fn supports_attr(&self, attr: Attr) -> bool {
        match self.terminal {
            Colored(ref c) => c.supports_attr(attr),
            NoColor(_) => false
        }
    }

    fn reset(&mut self) -> io::Result<()> {
        match self.terminal {
            Colored(ref mut c) => c.reset().map(|_| ()),
            NoColor(_) => Ok(())
        }
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
