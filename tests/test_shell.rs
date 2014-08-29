use std::io::{MemWriter, IoResult, ChanReader, ChanWriter};

use term::{Terminal, TerminfoTerminal, color};
use hamcrest::{assert_that};

use support::{ResultTest, Tap, shell_writes};
use cargo::core::shell::{Shell,ShellConfig};

fn setup() {
}

fn io_channel() -> (Box<Writer + 'static>, Box<Reader + 'static>) {
    let (tx, rx) = channel();
    (box ChanWriter::new(tx), box ChanReader::new(rx))
}

test!(non_tty {
    let config = ShellConfig { color: true, verbose: true, tty: false };
    let (tx, mut rx) = io_channel();

    Shell::create(tx, config).tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
    });
    assert_that(rx.read_to_end().unwrap().as_slice(),
                shell_writes("Hey Alex\n"));
})

test!(color_explicitly_disabled {
    let config = ShellConfig { color: false, verbose: true, tty: true };
    let (tx, mut rx) = io_channel();

    Shell::create(tx, config).tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
    });
    assert_that(rx.read_to_end().unwrap().as_slice(),
                shell_writes("Hey Alex\n"));
})

test!(colored_shell {
    let term: Option<TerminfoTerminal<MemWriter>> =
        Terminal::new(MemWriter::new());
    if term.is_none() { return }
    let (tx, mut rx) = io_channel();

    let config = ShellConfig { color: true, verbose: true, tty: true };

    Shell::create(tx, config).tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
    });
    assert_that(rx.read_to_end().unwrap().as_slice(),
                shell_writes(colored_output("Hey Alex\n",
                                            color::RED).assert()));
})

fn colored_output<S: Str>(string: S, color: color::Color) -> IoResult<String> {
    let mut term: TerminfoTerminal<MemWriter> =
        Terminal::new(MemWriter::new()).assert();
    try!(term.reset());
    try!(term.fg(color));
    try!(term.write_str(string.as_slice()));
    try!(term.reset());
    try!(term.flush());
    Ok(String::from_utf8_lossy(term.get_ref().get_ref()).to_string())
}
