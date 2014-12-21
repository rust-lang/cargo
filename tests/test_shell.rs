use std::io::{MemWriter, IoResult, ChanReader, ChanWriter};
use term::{Terminal, TerminfoTerminal, color};
use hamcrest::{assert_that};

use cargo::core::shell::{Shell,ShellConfig};

use support::{ResultTest,Tap,shell_writes};

fn setup() {
}

fn pair() -> (ChanWriter, ChanReader) {
    let (tx, rx) = channel();
    (ChanWriter::new(tx), ChanReader::new(rx))
}

test!(non_tty {
    let config = ShellConfig { color: true, verbose: true, tty: false };
    let (tx, mut rx) = pair();

    Shell::create(box tx, config).tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
    });

    let buf = rx.read_to_end().unwrap();
    assert_that(buf.as_slice(), shell_writes("Hey Alex\n"));
});

test!(color_explicitly_disabled {
    let config = ShellConfig { color: false, verbose: true, tty: true };
    let (tx, mut rx) = pair();

    Shell::create(box tx, config).tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
    });
    let buf = rx.read_to_end().unwrap();
    assert_that(buf.as_slice(), shell_writes("Hey Alex\n"));
});

test!(colored_shell {
    let term = TerminfoTerminal::new(MemWriter::new());
    if term.is_none() { return }

    let config = ShellConfig { color: true, verbose: true, tty: true };
    let (tx, mut rx) = pair();

    Shell::create(box tx, config).tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
    });
    let buf = rx.read_to_end().unwrap();
    assert_that(buf.as_slice(),
                shell_writes(colored_output("Hey Alex\n",
                                            color::RED).assert()));
});

fn colored_output<S: Str>(string: S, color: color::Color) -> IoResult<String> {
    let mut term = TerminfoTerminal::new(MemWriter::new()).unwrap();
    try!(term.reset());
    try!(term.fg(color));
    try!(term.write_str(string.as_slice()));
    try!(term.reset());
    try!(term.flush());
    Ok(String::from_utf8_lossy(term.get_ref().get_ref()).to_string())
}
