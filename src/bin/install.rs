use cargo::ops;
use cargo::util::{CliResult, CliError, Config};
use std::path::Path;

#[allow(dead_code)] // for now until all options are implemented

#[derive(RustcDecodable)]
struct Options {
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_no_default_features: bool,
    flag_debug: bool,
    flag_bin: Option<String>,
    flag_example: Vec<String>,
    flag_package: Vec<String>,
    flag_verbose: bool,
    flag_root: Option<String>,
}

pub const USAGE: &'static str = "
Install a crate onto the local system

Installing new crates:
    cargo install [options]
    cargo install [options] [-p CRATE | --package CRATE] [--vers VERS]
    cargo install [options] --git URL [--branch BRANCH | --tag TAG | --rev SHA]
    cargo install [options] --path PATH

Managing installed crates:
    cargo install [options] --list

Options:
    -h, --help              Print this message
    -j N, --jobs N          The number of jobs to run in parallel
    --features FEATURES     Space-separated list of features to activate
    --no-default-features   Do not build the `default` feature
    --debug                 Build in debug mode instead of release mode
    --bin NAME              Only install the binary NAME
    --example EXAMPLE       Install the example EXAMPLE instead of binaries
    -p, --package CRATE     Install this crate from crates.io or select the
                            package in a repository/path to install.
    -v, --verbose           Use verbose output
    --root DIR              Directory to install packages into

This command manages Cargo's local set of install binary crates. Only packages
which have [[bin]] targets can be installed, and all binaries are installed into
`$HOME/.cargo/bin` by default (or `$CARGO_HOME/bin` if you change the home
directory).

There are multiple methods of installing a new crate onto the system. The
`cargo install` command with no arguments will install the current crate (as
specifed by the current directory). Otherwise the `-p`, `--package`, `--git`,
and `--path` options all specify the source from which a crate is being
installed. The `-p` and `--package` options will download crates from crates.io.

Crates from crates.io can optionally specify the version they wish to install
via the `--vers` flags, and similarly packages from git repositories can
optionally specify the branch, tag, or revision that should be installed. If a
crate has multiple binaries, the `--bin` argument can selectively install only
one of them, and if you'd rather install examples the `--example` argument can
be used as well.

The `--list` option will list all installed packages (and their versions).
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    config.shell().set_verbose(options.flag_verbose);

    let compile_opts = ops::CompileOptions {
        config: config,
        jobs: options.flag_jobs,
        target: None,
        features: &options.flag_features,
        no_default_features: options.flag_no_default_features,
        spec: None,
        exec_engine: None,
        mode: ops::CompileMode::Build,
        release: true,
        filter: ops::CompileFilter::Everything,
        target_rustc_args: None,
    };

    let root = &Path::new("$HOME/.cargo/bin");

    ops::install(&root,
                 &compile_opts).map_err(|err| {
        CliError::from_boxed(err, 101)
    }).map(|_| None)
}
