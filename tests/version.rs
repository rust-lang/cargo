extern crate cargo;
extern crate cargotest;
extern crate hamcrest;
extern crate rustc_serialize;

use cargotest::support::{project, execs};
use hamcrest::assert_that;

#[test]
fn simple() {
    let p = project("foo");

    assert_that(p.cargo_process("version"),
                execs().with_status(0).with_stdout(&format!("{}\n",
                                                            cargo::version())));

    assert_that(p.cargo_process("--version"),
                execs().with_status(0).with_stdout(&format!("{}\n",
                                                            cargo::version())));

}

#[derive(RustcDecodable)]
struct FooFlags {
    flag_version: bool,
}

fn real_main(flags: FooFlags, _config: &cargo::Config) ->
        cargo::CliResult<Option<String>> {
    if flags.flag_version {
        Ok(Some("foo <version>".to_string()))
    } else {
        Ok(None)
    }
}

#[test]
fn subcommand_with_version_using_exec_main_without_stdin() {
    let usage = "
Usage: cargo foo [--version]

Options:
    -V, --version       Print version info
";
    let args: Vec<String> = vec!["cargo", "foo", "--version"]
        .into_iter().map(|s| s.to_string()).collect();
    let result = cargo::call_main_without_stdin(
                real_main, &cargo::Config::default().unwrap(),
                usage, &args, false);
    assert_eq!(result.unwrap(), Some("foo <version>".to_string()));
}
