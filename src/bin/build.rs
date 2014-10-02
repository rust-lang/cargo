use std::os;

use cargo::core::MultiShell;
use cargo::ops::CompileOptions;
use cargo::ops;
use cargo::util::important_paths::{find_root_manifest_for_cwd};
use cargo::util::{CliResult, CliError};
use docopt;

docopt!(Options, "
Compile a local package and all of its dependencies

Usage:
    cargo build [options]

Options:
    -h, --help               Print this message
    -p SPEC, --package SPEC  Package to build
    -j N, --jobs N           The number of jobs to run in parallel
    --release                Build artifacts in release mode, with optimizations
    --features FEATURES      Space-separated list of features to also build
    --no-default-features    Do not build the `default` feature
    --target TRIPLE          Build for the target triple
    --manifest-path PATH     Path to the manifest to compile
    -v, --verbose            Use verbose output

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.
",  flag_jobs: Option<uint>, flag_target: Option<String>,
    flag_manifest_path: Option<String>, flag_features: Vec<String>,
    flag_package: Option<String>)

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-build; args={}", os::args());
    shell.set_verbose(options.flag_verbose);

    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    let env = if options.flag_release {
        "release"
    } else {
        "compile"
    };

    let mut opts = CompileOptions {
        env: env,
        shell: shell,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| t.as_slice()),
        dev_deps: false,
        features: options.flag_features.as_slice(),
        no_default_features: options.flag_no_default_features,
        spec: options.flag_package.as_ref().map(|s| s.as_slice()),
    };

    ops::compile(&root, &mut opts).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}
