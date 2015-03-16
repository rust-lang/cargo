use std::io::prelude::*;
use std::io;
use std::sync::{Arc, Mutex};
use term::{Terminal, TerminfoTerminal, color};
use hamcrest::{assert_that};

use cargo::core::shell::{Shell,ShellConfig};

use support::{Tap, shell_writes};

fn setup() {
}

struct Sink(Arc<Mutex<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        Write::write(&mut *self.0.lock().unwrap(), data)
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

test!(non_tty {
    let config = ShellConfig { color: true, verbose: true, tty: false };
    let a = Arc::new(Mutex::new(Vec::new()));

    Shell::create(Box::new(Sink(a.clone())), config).tap(|shell| {
        shell.say("Hey Alex", color::RED).unwrap();
    });

    let buf = a.lock().unwrap().clone();
    assert_that(&buf[..], shell_writes("Hey Alex\n"));
});

test!(color_explicitly_disabled {
    let config = ShellConfig { color: false, verbose: true, tty: true };
    let a = Arc::new(Mutex::new(Vec::new()));

    Shell::create(Box::new(Sink(a.clone())), config).tap(|shell| {
        shell.say("Hey Alex", color::RED).unwrap();
    });
    let buf = a.lock().unwrap().clone();
    assert_that(&buf[..], shell_writes("Hey Alex\n"));
});

test!(colored_shell {
    let term = TerminfoTerminal::new(Vec::new());
    if term.is_none() { return }

    let config = ShellConfig { color: true, verbose: true, tty: true };
    let a = Arc::new(Mutex::new(Vec::new()));

    Shell::create(Box::new(Sink(a.clone())), config).tap(|shell| {
        shell.say("Hey Alex", color::RED).unwrap();
    });
    let buf = a.lock().unwrap().clone();
    assert_that(&buf[..],
                shell_writes(colored_output("Hey Alex\n",
                                            color::RED).unwrap()));
});

fn colored_output(string: &str, color: color::Color) -> io::Result<String> {
    let mut term = TerminfoTerminal::new(Vec::new()).unwrap();
    try!(term.reset());
    try!(term.fg(color));
    try!(write!(&mut term, "{}", string));
    try!(term.reset());
    try!(term.flush());
    Ok(String::from_utf8_lossy(term.get_ref()).to_string())
}
