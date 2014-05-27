use term;
use term::{Terminal,color};
use term::color::Color;
use term::attr::Attr;
use std::io::IoResult;
use std::io::stdio::StdWriter;

pub struct ShellConfig {
    color: bool,
    verbose: bool
}

enum AdequateTerminal {
    NoColor(BasicTerminal<StdWriter>),
    Color(Box<Terminal<StdWriter>>)
}

pub struct Shell {
    terminal: AdequateTerminal,
    config: ShellConfig
}

impl Shell {
    fn create(out: StdWriter, config: ShellConfig) -> Option<Shell> {
        let term = if out.isatty() {
            let term: Option<term::TerminfoTerminal<StdWriter>> = Terminal::new(out);
            term.map(|t| Color(box t))
        } else {
            Some(NoColor(BasicTerminal { writer: out }))
        };

        term.map(|term| Shell { terminal: term, config: config })
    }

    pub fn verbose(&mut self, callback: |&mut Shell| -> IoResult<()>) -> IoResult<()> {
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

impl Terminal<StdWriter> for Shell {
    fn new(out: StdWriter) -> Option<Shell> {
        Shell::create(out, ShellConfig { color: true, verbose: false })
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

    fn unwrap(self) -> StdWriter {
        fail!("Can't unwrap a Shell")
    }

    fn get_ref<'a>(&'a self) -> &'a StdWriter {
        match self.terminal {
            Color(ref c) => c.get_ref(),
            NoColor(ref n) => n.get_ref()
        }
    }

    fn get_mut<'a>(&'a mut self) -> &'a mut StdWriter {
        match self.terminal {
            Color(ref mut c) => c.get_mut(),
            NoColor(ref mut n) => n.get_mut()
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

pub struct BasicTerminal<T> {
    writer: T
}

impl<T: Writer> Terminal<T> for BasicTerminal<T> {
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

impl<T: Writer> Writer for BasicTerminal<T> {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.writer.flush()
    }
}

