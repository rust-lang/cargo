use cargo::ops;
use cargo::core::{MultiShell};
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

#[deriving(RustcDecodable)]
struct Options {
    flag_features: Vec<String>,
    flag_jobs: Option<uint>,
    flag_manifest_path: Option<String>,
    flag_no_default_features: bool,
    flag_no_deps: bool,
    flag_open: bool,
    flag_verbose: bool,
    flag_package: Option<String>,
}

pub const USAGE: &'static str = "
Build a package's documentation

Usage:
    cargo doc [options]

Options:
    -h, --help               Print this message
    --open                   Opens the docs in a browser after the operation
    -p SPEC, --package SPEC  Package to document
    --no-deps                Don't build documentation for dependencies
    -j N, --jobs N           The number of jobs to run in parallel
    --features FEATURES      Space-separated list of features to also build
    --no-default-features    Do not build the `default` feature
    --manifest-path PATH     Path to the manifest to document
    -v, --verbose            Use verbose output

By default the documentation for the local package and all dependencies is
built. The output is all placed in `target/doc` in rustdoc's usual format.

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be documented. If it is not given, then the
current package is documented. For more information on SPEC and its format, see
the `cargo help pkgid` command.
";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);

    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    let mut doc_opts = ops::DocOptions {
        all: !options.flag_no_deps,
        open_result: options.flag_open,
        compile_opts: ops::CompileOptions {
            env: if options.flag_no_deps {"doc"} else {"doc-all"},
            shell: shell,
            jobs: options.flag_jobs,
            target: None,
            dev_deps: false,
            features: options.flag_features.as_slice(),
            no_default_features: options.flag_no_default_features,
            spec: options.flag_package.as_ref().map(|s| s.as_slice()),
            lib_only: false,
            exec_engine: None,
        },
    };

    try!(ops::doc(&root, &mut doc_opts).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));

    Ok(None)
}

