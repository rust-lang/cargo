use term;
use term::{Terminal,color};
use term::color::Color;
use term::attr::Attr;
use std::io::IoResult;
use std::io::stdio::StdWriter;

pub struct ShellConfig {
    pub color: bool,
    pub verbose: bool,
    pub tty: bool
}

enum AdequateTerminal<T> {
    NoColor(BasicTerminal<T>),
    Color(Box<Terminal<T>>)
}

pub struct Shell<T> {
    terminal: AdequateTerminal<T>,
    config: ShellConfig
}

impl<T: Writer + Send> Shell<T> {
    pub fn create(out: T, config: ShellConfig) -> Option<Shell<T>> {
        if config.tty {
            let term: Option<term::TerminfoTerminal<T>> = Terminal::new(out);
            term.map(|t| Shell { terminal: Color(box t as Box<Terminal<T>>), config: config })
        } else {
            Some(Shell { terminal: NoColor(BasicTerminal { writer: out }), config: config })
        }
    }

    pub fn verbose(&mut self, callback: |&mut Shell<T>| -> IoResult<()>) -> IoResult<()> {
        if self.config.verbose {
            return callback(self)
        }

        Ok(())
    }

    pub fn say<T: Str>(&mut self, message: T, color: Color) -> IoResult<()> {
        try!(self.reset());
        try!(self.fg(color));
        try!(self.write_line(message.as_slice()));
        try!(self.reset());
        try!(self.flush());
        Ok(())
    }
}

impl<T: Writer + Send> Terminal<T> for Shell<T> {
    fn new(out: T) -> Option<Shell<T>> {
        Shell::create(out, ShellConfig { color: true, verbose: false, tty: false })
    }

    fn fg(&mut self, color: color::Color) -> IoResult<bool> {
        match self.terminal {
            Color(ref mut c) => c.fg(color),
            NoColor(ref mut n) => n.fg(color)
        }
    }

    fn bg(&mut self, color: color::Color) -> IoResult<bool> {
        match self.terminal {
            Color(ref mut c) => c.bg(color),
            NoColor(ref mut n) => n.bg(color)
        }
    }

    fn attr(&mut self, attr: Attr) -> IoResult<bool> {
        match self.terminal {
            Color(ref mut c) => c.attr(attr),
            NoColor(ref mut n) => n.attr(attr)
        }
    }

    fn supports_attr(&self, attr: Attr) -> bool {
        match self.terminal {
            Color(ref c) => c.supports_attr(attr),
            NoColor(ref n) => n.supports_attr(attr)
        }
    }

    fn reset(&mut self) -> IoResult<()> {
        match self.terminal {
            Color(ref mut c) => c.reset(),
            NoColor(ref mut n) => n.reset()
        }
    }

    fn unwrap(self) -> T {
        fail!("Can't unwrap a Shell");
    }

    fn get_ref<'a>(&'a self) -> &'a T {
        match self.terminal {
            Color(ref c) => c.get_ref(),
            NoColor(ref n) => n.get_ref()
        }
    }

    fn get_mut<'a>(&'a mut self) -> &'a mut T {
        match self.terminal {
            Color(ref mut c) => c.get_mut(),
            NoColor(ref mut n) => n.get_mut()
        }
    }
}

impl<T: Writer + Send> Writer for Shell<T> {
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

pub struct BasicTerminal<T> {
    writer: T
}

impl<T: Writer + Send> Terminal<T> for BasicTerminal<T> {
    fn new(out: T) -> Option<BasicTerminal<T>> {
        Some(BasicTerminal { writer: out })
    }

    fn fg(&mut self, _: Color) -> IoResult<bool> {
        Ok(false)
    }

    fn bg(&mut self, _: Color) -> IoResult<bool> {
        Ok(false)
    }

    fn attr(&mut self, _: Attr) -> IoResult<bool> {
        Ok(false)
    }

    fn supports_attr(&self, _: Attr) -> bool {
        false
    }

    fn reset(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn unwrap(self) -> T {
        self.writer
    }

    fn get_ref<'a>(&'a self) -> &'a T {
        &self.writer
    }

    fn get_mut<'a>(&'a mut self) -> &'a mut T {
        &mut self.writer
    }
}

impl<T: Writer + Send> Writer for BasicTerminal<T> {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.writer.flush()
    }
}

