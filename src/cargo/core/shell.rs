use term;
use term::{Terminal,color};
use term::color::{Color, BLACK, RED, GREEN, YELLOW};
use term::attr::{Attr, Bold};
use std::io::{IoResult, stderr};
use std::fmt::Show;

pub struct ShellConfig {
    pub color: bool,
    pub verbose: bool,
    pub tty: bool
}

enum AdequateTerminal {
    NoColor(Box<Writer>),
    Color(Box<Terminal<Box<Writer>>>)
}

pub struct Shell {
    terminal: AdequateTerminal,
    config: ShellConfig
}

pub struct MultiShell {
    out: Shell,
    err: Shell,
    verbose: bool
}

pub type Callback<'a> = |&mut MultiShell|:'a -> IoResult<()>;

impl MultiShell {
    pub fn new(out: Shell, err: Shell, verbose: bool) -> MultiShell {
        MultiShell { out: out, err: err, verbose: verbose }
    }

    pub fn out<'a>(&'a mut self) -> &'a mut Shell {
        &mut self.out
    }

    pub fn err<'a>(&'a mut self) -> &'a mut Shell {
        &mut self.err
    }

    pub fn say<T: ToString>(&mut self, message: T, color: Color) -> IoResult<()> {
        self.out().say(message, color)
    }

    pub fn status<T: Show, U: Show>(&mut self, status: T, message: U) -> IoResult<()> {
        self.out().say_status(status, message, GREEN)
    }

    pub fn verbose(&mut self, callback: Callback) -> IoResult<()> {
        if self.verbose { return callback(self) }
        Ok(())
    }

    pub fn concise(&mut self, callback: Callback) -> IoResult<()> {
        if !self.verbose { return callback(self) }
        Ok(())
    }

    pub fn error<T: ToString>(&mut self, message: T) -> IoResult<()> {
        self.err().say(message, RED)
    }

    pub fn warn<T: ToString>(&mut self, message: T) -> IoResult<()> {
        self.err().say(message, YELLOW)
    }
}

pub type ShellCallback<'a> = |&mut Shell|:'a -> IoResult<()>;

impl Shell {
    pub fn create(out: Box<Writer>, config: ShellConfig) -> Shell {
        if config.tty && config.color {
            let term: Option<term::TerminfoTerminal<Box<Writer>>> = Terminal::new(out);
            term.map(|t| Shell {
                terminal: Color(box t as Box<Terminal<Box<Writer>>>),
                config: config
            }).unwrap_or_else(|| {
                Shell { terminal: NoColor(box stderr() as Box<Writer>), config: config }
            })
        } else {
            Shell { terminal: NoColor(out), config: config }
        }
    }

    pub fn verbose(&mut self, callback: ShellCallback) -> IoResult<()> {
        if self.config.verbose { return callback(self) }
        Ok(())
    }

    pub fn concise(&mut self, callback: ShellCallback) -> IoResult<()> {
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
}

impl Terminal<Box<Writer>> for Shell {
    fn new(out: Box<Writer>) -> Option<Shell> {
        Some(Shell {
            terminal: NoColor(out),
            config: ShellConfig {
                color: true,
                verbose: false,
                tty: false,
            }
        })
    }

    fn fg(&mut self, color: color::Color) -> IoResult<bool> {
        match self.terminal {
            Color(ref mut c) => c.fg(color),
            NoColor(_) => Ok(false)
        }
    }

    fn bg(&mut self, color: color::Color) -> IoResult<bool> {
        match self.terminal {
            Color(ref mut c) => c.bg(color),
            NoColor(_) => Ok(false)
        }
    }

    fn attr(&mut self, attr: Attr) -> IoResult<bool> {
        match self.terminal {
            Color(ref mut c) => c.attr(attr),
            NoColor(_) => Ok(false)
        }
    }

    fn supports_attr(&self, attr: Attr) -> bool {
        match self.terminal {
            Color(ref c) => c.supports_attr(attr),
            NoColor(_) => false
        }
    }

    fn reset(&mut self) -> IoResult<()> {
        match self.terminal {
            Color(ref mut c) => c.reset(),
            NoColor(_) => Ok(())
        }
    }

    fn unwrap(self) -> Box<Writer> {
        fail!("Can't unwrap a Shell");
    }

    fn get_ref<'a>(&'a self) -> &'a Box<Writer> {
        match self.terminal {
            Color(ref c) => c.get_ref(),
            NoColor(ref w) => w
        }
    }

    fn get_mut<'a>(&'a mut self) -> &'a mut Box<Writer> {
        match self.terminal {
            Color(ref mut c) => c.get_mut(),
            NoColor(ref mut w) => w
        }
    }
}

impl Writer for Shell {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        match self.terminal {
            Color(ref mut c) => c.write(buf),
            NoColor(ref mut n) => n.write(buf)
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        match self.terminal {
            Color(ref mut c) => c.flush(),
            NoColor(ref mut n) => n.flush()
        }
    }
}
