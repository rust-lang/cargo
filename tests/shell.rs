extern crate cargo;
extern crate cargotest;
extern crate hamcrest;
extern crate term;

use std::io::prelude::*;
use std::io;
use std::sync::{Arc, Mutex};

use cargo::core::shell::ColorConfig::{Auto,Always, Never};
use cargo::core::shell::{Shell, ShellConfig};
use cargo::util::CargoResult;
use cargotest::support::{Tap, execs, shell_writes};
use hamcrest::{assert_that};
use term::{Terminal, TerminfoTerminal, color};

struct Sink(Arc<Mutex<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        Write::write(&mut *self.0.lock().unwrap(), data)
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[test]
fn non_tty() {
    let config = ShellConfig { color_config: Auto, tty: false };
    let a = Arc::new(Mutex::new(Vec::new()));

    Shell::create(|| Box::new(Sink(a.clone())), config).tap(|shell| {
        shell.say("Hey Alex", color::RED).unwrap();
    });
    let buf = a.lock().unwrap().clone();
    assert_that(&buf[..], shell_writes("Hey Alex\n"));
}

#[test]
fn color_explicitly_disabled() {
    let term = TerminfoTerminal::new(Vec::new());
    if term.is_none() { return }

    let config = ShellConfig { color_config: Never, tty: true };
    let a = Arc::new(Mutex::new(Vec::new()));

    Shell::create(|| Box::new(Sink(a.clone())), config).tap(|shell| {
        shell.say("Hey Alex", color::RED).unwrap();
    });
    let buf = a.lock().unwrap().clone();
    assert_that(&buf[..], shell_writes("Hey Alex\n"));
}

#[test]
fn colored_shell() {
    let term = TerminfoTerminal::new(Vec::new());
    if term.is_none() { return }

    let config = ShellConfig { color_config: Auto, tty: true };
    let a = Arc::new(Mutex::new(Vec::new()));

    Shell::create(|| Box::new(Sink(a.clone())), config).tap(|shell| {
        shell.say("Hey Alex", color::RED).unwrap();
    });
    let buf = a.lock().unwrap().clone();
    let expected_output = if term.unwrap().supports_color() {
        shell_writes(colored_output("Hey Alex\n", color::RED).unwrap())
    } else {
        shell_writes("Hey Alex\n")
    };
    assert_that(&buf[..], expected_output);
}

#[test]
fn color_explicitly_enabled() {
    let term = TerminfoTerminal::new(Vec::new());
    if term.is_none() { return }
    if !term.unwrap().supports_color() { return }

    let config = ShellConfig { color_config: Always, tty: false };
    let a = Arc::new(Mutex::new(Vec::new()));

    Shell::create(|| Box::new(Sink(a.clone())), config).tap(|shell| {
        shell.say("Hey Alex", color::RED).unwrap();
    });
    let buf = a.lock().unwrap().clone();
    assert_that(&buf[..],
                shell_writes(colored_output("Hey Alex\n",
                                            color::RED).unwrap()));
}

#[test]
fn no_term() {
    // Verify that shell creation is successful when $TERM does not exist.
    assert_that(cargotest::cargo_process().env_remove("TERM"),
                execs().with_stderr(""));
}

fn colored_output(string: &str, color: color::Color) -> CargoResult<String> {
    let mut term = TerminfoTerminal::new(Vec::new()).unwrap();
    term.reset()?;
    term.fg(color)?;
    write!(&mut term, "{}", string)?;
    term.reset()?;
    term.flush()?;
    Ok(String::from_utf8_lossy(term.get_ref()).to_string())
}
