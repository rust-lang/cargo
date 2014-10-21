use term::{Terminal, TerminfoTerminal, color};
use term::color::{Color, BLACK, RED, GREEN, YELLOW};
use term::attr::{Attr, Bold};
use std::io::{IoResult, stderr};
use std::fmt::Show;

pub enum Verbosity {
    Verbose,
    Normal,
    Quiet
}

pub struct ShellConfig {
    pub color: bool,
    pub verbosity: Verbosity,
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
    verbose: Verbosity
}

pub type Callback<'a> = |&mut MultiShell|:'a -> IoResult<()>;

struct UghWhyIsThisNecessary {
    inner: Box<Writer + Send>,
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

    pub fn say<T: ToString>(&mut self, message: T, color: Color) -> IoResult<()> {
        match self.verbosity {
            Quiet => Ok(()),
            _ => self.out().say(message, color)
        }
    }

    pub fn status<T: Show, U: Show>(&mut self, status: T, message: U) -> IoResult<()> {
        match self.verbosity {
            Quiet => Ok(()),
            _ => self.out().say_status(status, message, GREEN)
        }
    }

    pub fn verbose(&mut self, callback: Callback) -> IoResult<()> {
        match self.verbosity {
            Verbose => return callback(self),
            _ => Ok(())
        }
    }

    pub fn concise(&mut self, callback: Callback) -> IoResult<()> {
        match self.verbosity {
            Verbose => Ok(()),
            _ => return callback(self)
        }
    }

    pub fn error<T: ToString>(&mut self, message: T) -> IoResult<()> {
        self.err().say(message, RED)
    }

    pub fn warn<T: ToString>(&mut self, message: T) -> IoResult<()> {
        self.err().say(message, YELLOW)
    }

    pub fn set_verbosity(&mut self, verbosity: Verbosity) {
        self.verbosity = verbosity;
    }

    /// shortcut for commands that don't have both --verbose and --quiet
    pub fn set_verbose(&mut self, verbose: bool) {
        if verbose {
            self.verbosity = Verbose;
        } else {
            self.verbosity = Normal;
        }
    }
}

pub type ShellCallback<'a> = |&mut Shell|:'a -> IoResult<()>;

impl Shell {
    pub fn create(out: Box<Writer + Send>, config: ShellConfig) -> Shell {
        let out = UghWhyIsThisNecessary { inner: out };
        if config.tty && config.color {
            let term = TerminfoTerminal::new(out);
            term.map(|t| Shell {
                terminal: Colored(t),
                config: config
            }).unwrap_or_else(|| {
                Shell { terminal: NoColor(box stderr()), config: config }
            })
        } else {
            Shell { terminal: NoColor(out.inner), config: config }
        }
    }

    pub fn verbose(&mut self, callback: ShellCallback) -> IoResult<()> {
        match self.config.verbosity {
            Verbose => return callback(self),
            _ => Ok(())
        }
    }

    pub fn concise(&mut self, callback: ShellCallback) -> IoResult<()> {
        match self.config.verbosity {
            Verbose => Ok(()),
            _ => return callback(self)
        }
    }

    pub fn say<T: ToString>(&mut self, message: T, color: Color) -> IoResult<()> {
        try!(self.reset());
        if color != BLACK { try!(self.fg(color)); }
        try!(self.write_line(message.to_string().as_slice()));
        try!(self.reset());
        try!(self.flush());
        Ok(())
    }

    pub fn say_status<T: Show, U: Show>(&mut self, status: T, message: U,
                                        color: Color) -> IoResult<()> {
        try!(self.reset());
        if color != BLACK { try!(self.fg(color)); }
        if self.supports_attr(Bold) { try!(self.attr(Bold)); }
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
            Colored(ref mut c) => c.reset(),
            NoColor(_) => Ok(())
        }
    }
}

impl Writer for Shell {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        match self.terminal {
            Colored(ref mut c) => c.write(buf),
            NoColor(ref mut n) => n.write(buf)
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
    fn write(&mut self, bytes: &[u8]) -> IoResult<()> {
        self.inner.write(bytes)
    }
}
