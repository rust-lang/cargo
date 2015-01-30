use std::fmt;
use std::old_io::{IoResult, stderr};

use term::Attr;
use term::color::{Color, BLACK, RED, GREEN, YELLOW};
use term::{Terminal, TerminfoTerminal, color};

use self::AdequateTerminal::{NoColor, Colored};

#[derive(Copy)]
pub struct ShellConfig {
    pub color: bool,
    pub verbose: bool,
    pub tty: bool
}

enum AdequateTerminal {
    NoColor(Box<Writer + Send>),
    Colored(Box<Terminal<UghWhyIsThisNecessary> + Send>)
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

struct UghWhyIsThisNecessary {
    inner: Box<Writer + Send>,
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

    pub fn say<T: ToString>(&mut self, message: T, color: Color) -> IoResult<()> {
        self.out().say(message, color)
    }

    pub fn status<T, U>(&mut self, status: T, message: U) -> IoResult<()>
        where T: fmt::Display, U: fmt::Display
    {
        self.out().say_status(status, message, GREEN)
    }

    pub fn verbose<F>(&mut self, mut callback: F) -> IoResult<()>
        where F: FnMut(&mut MultiShell) -> IoResult<()>
    {
        if self.verbose { return callback(self) }
        Ok(())
    }

    pub fn concise<F>(&mut self, mut callback: F) -> IoResult<()>
        where F: FnMut(&mut MultiShell) -> IoResult<()>
    {
        if !self.verbose { return callback(self) }
        Ok(())
    }

    pub fn error<T: ToString>(&mut self, message: T) -> IoResult<()> {
        self.err().say(message, RED)
    }

    pub fn warn<T: ToString>(&mut self, message: T) -> IoResult<()> {
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
    pub fn create(out: Box<Writer + Send>, config: ShellConfig) -> Shell {
        let out = UghWhyIsThisNecessary { inner: out };
        if config.tty && config.color {
            let term = TerminfoTerminal::new(out);
            term.map(|t| Shell {
                terminal: Colored(Box::new(t)),
                config: config
            }).unwrap_or_else(|| {
                Shell { terminal: NoColor(Box::new(stderr())), config: config }
            })
        } else {
            Shell { terminal: NoColor(out.inner), config: config }
        }
    }

    pub fn verbose<F>(&mut self, mut callback: F) -> IoResult<()>
        where F: FnMut(&mut Shell) -> IoResult<()>
    {
        if self.config.verbose { return callback(self) }
        Ok(())
    }

    pub fn concise<F>(&mut self, mut callback: F) -> IoResult<()>
        where F: FnMut(&mut Shell) -> IoResult<()>
    {
        if !self.config.verbose { return callback(self) }
        Ok(())
    }

    pub fn say<T: ToString>(&mut self, message: T, color: Color) -> IoResult<()> {
        try!(self.reset());
        if color != BLACK { try!(self.fg(color)); }
        try!(self.write_line(message.to_string().as_slice()));
        try!(self.reset());
        try!(self.flush());
        Ok(())
    }

    pub fn say_status<T, U>(&mut self, status: T, message: U, color: Color)
                            -> IoResult<()>
        where T: fmt::Display, U: fmt::Display
    {
        try!(self.reset());
        if color != BLACK { try!(self.fg(color)); }
        if self.supports_attr(Attr::Bold) { try!(self.attr(Attr::Bold)); }
        try!(self.write_str(format!("{:>12}", status).as_slice()));
        try!(self.reset());
        try!(self.write_line(format!(" {}", message).as_slice()));
        try!(self.flush());
        Ok(())
    }

    fn fg(&mut self, color: color::Color) -> IoResult<bool> {
        match self.terminal {
            Colored(ref mut c) => c.fg(color),
            NoColor(_) => Ok(false)
        }
    }

    fn attr(&mut self, attr: Attr) -> IoResult<bool> {
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

    fn reset(&mut self) -> IoResult<()> {
        match self.terminal {
            Colored(ref mut c) => c.reset().map(|_| ()),
            NoColor(_) => Ok(())
        }
    }
}

impl Writer for Shell {
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        match self.terminal {
            Colored(ref mut c) => c.write_all(buf),
            NoColor(ref mut n) => n.write_all(buf)
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        match self.terminal {
            Colored(ref mut c) => c.flush(),
            NoColor(ref mut n) => n.flush()
        }
    }
}

impl Writer for UghWhyIsThisNecessary {
    fn write_all(&mut self, bytes: &[u8]) -> IoResult<()> {
        self.inner.write_all(bytes)
    }
}
