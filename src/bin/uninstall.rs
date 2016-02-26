use cargo::ops;
use cargo::util::{CliResult, Config};

#[derive(RustcDecodable)]
pub struct Options {
    flag_bin: Vec<String>,
    flag_root: Option<String>,
    flag_verbose: Option<bool>,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,

    arg_spec: String,
}

pub const USAGE: &'static str = "
Remove a Rust binary

Usage:
    cargo uninstall [options] <spec>
    cargo uninstall (-h | --help)

Options:
    -h, --help                Print this message
    --root DIR                Directory to uninstall packages from
    --bin NAME                Only uninstall the binary NAME
    -v, --verbose             Use verbose output
    -q, --quiet               Less output printed to stdout
    --color WHEN              Coloring: auto, always, never

The argument SPEC is a package id specification (see `cargo help pkgid`) to
specify which crate should be uninstalled. By default all binaries are
uninstalled for a crate but the `--bin` and `--example` flags can be used to
only uninstall particular binaries.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    try!(config.configure_shell(options.flag_verbose,
                                options.flag_quiet,
                                &options.flag_color));

    let root = options.flag_root.as_ref().map(|s| &s[..]);
    try!(ops::uninstall(root, &options.arg_spec, &options.flag_bin, config));
    Ok(None)
}

