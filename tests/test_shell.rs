use support::{ResultTest,Tap,shell_writes};
use hamcrest::{assert_that};
use std::io::{MemWriter,IoResult};
use std::str::from_utf8_lossy;
use cargo::core::shell::{Shell,ShellConfig};
use term::{Terminal,TerminfoTerminal,color};

fn setup() {
}

test!(non_tty {
    let config = ShellConfig { color: true, verbose: true, tty: false };
    Shell::create(MemWriter::new(), config).assert().tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
        assert_that(shell, shell_writes("Hey Alex\n"));
    });
})

test!(color_explicitly_disabled {
    let config = ShellConfig { color: false, verbose: true, tty: true };
    Shell::create(MemWriter::new(), config).assert().tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
        assert_that(shell, shell_writes("Hey Alex\n"));
    });
})

test!(colored_shell {
    let config = ShellConfig { color: true, verbose: true, tty: true };
    Shell::create(MemWriter::new(), config).assert().tap(|shell| {
        shell.say("Hey Alex", color::RED).assert();
        assert_that(shell, shell_writes(colored_output("Hey Alex\n",
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
    Ok(from_utf8_lossy(term.get_ref().get_ref()).to_str())
}
