use support::{ResultTest,Tap,shell_writes};
use hamcrest::{assert_that};
use std::io::{MemWriter, BufWriter, IoResult};
use std::str::from_utf8_lossy;
use cargo::core::shell::{Shell,ShellConfig};
use term::{Terminal,TerminfoTerminal,color};

fn setup() {
}

fn writer(buf: &mut [u8]) -> Box<Writer> {
    box BufWriter::new(buf) as Box<Writer>
}

test!(non_tty {
    let config = ShellConfig { color: true, verbose: true, tty: false };
    let mut buf: Vec<u8> = Vec::from_elem(9, 0 as u8);

    Shell::create(writer(buf.as_mut_slice()), config).tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
        assert_that(buf.as_slice(), shell_writes("Hey Alex\n"));
    });
})

test!(color_explicitly_disabled {
    let config = ShellConfig { color: false, verbose: true, tty: true };
    let mut buf: Vec<u8> = Vec::from_elem(9, 0 as u8);

    Shell::create(writer(buf.as_mut_slice()), config).tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
        assert_that(buf.as_slice(), shell_writes("Hey Alex\n"));
    });
})

test!(colored_shell {
    let term: Option<TerminfoTerminal<MemWriter>> =
        Terminal::new(MemWriter::new());
    if term.is_none() { return }

    let config = ShellConfig { color: true, verbose: true, tty: true };
    let mut buf: Vec<u8> = Vec::from_elem(100, 0 as u8);

    Shell::create(writer(buf.as_mut_slice()), config).tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
        let buf = buf.as_slice().slice_to(buf.iter().position(|a| *a == 0).unwrap());
        assert_that(buf, shell_writes(colored_output("Hey Alex\n",
                                                     color::RED).assert()));
    });
})

fn colored_output<S: Str>(string: S, color: color::Color) -> IoResult<String> {
    let mut term: TerminfoTerminal<MemWriter> =
        Terminal::new(MemWriter::new()).assert();
    try!(term.reset());
    try!(term.fg(color));
    try!(term.write_str(string.as_slice()));
    try!(term.reset());
    try!(term.flush());
    Ok(from_utf8_lossy(term.get_ref().get_ref()).to_string())
}
