use std::env;

use cargo::ops::CompileOptions;
use cargo::ops;
use cargo::util::important_paths::{find_root_manifest_for_cwd};
use cargo::util::{CliResult, CliError, Config};

#[derive(RustcDecodable)]
struct Options {
    arg_pkgid: Option<String>,
    arg_opts: Option<Vec<String>>,
    flag_profile: Option<String>,
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_no_default_features: bool,
    flag_target: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
    flag_release: bool,
    flag_lib: bool,
    flag_bin: Vec<String>,
    flag_example: Vec<String>,
    flag_test: Vec<String>,
    flag_bench: Vec<String>,
}

pub const USAGE: &'static str = "
Compile a package and all of its dependencies

Usage:
    cargo rustc [options] [<pkgid>] [--] [<opts>...]

Options:
    -h, --help              Print this message
    -p, --profile PROFILE   The profile to compile for
    -j N, --jobs N          The number of jobs to run in parallel
    --lib                   Build only this package's library
    --bin NAME              Build only the specified binary
    --example NAME          Build only the specified example
    --test NAME             Build only the specified test
    --bench NAME            Build only the specified benchmark
    --release               Build artifacts in release mode, with optimizations
    --features FEATURES     Features to compile for the package
    --no-default-features   Do not compile default features for the package
    --target TRIPLE         Target triple which compiles will be for
    --manifest-path PATH    Path to the manifest to fetch depednencies for
    -v, --verbose           Use verbose output

The <pkgid> specified (defaults to the current package) will have all of its
dependencies compiled, and then the package itself will be compiled. This
command requires that a lockfile is available and dependencies have been
fetched.

All of the trailing arguments are passed through to the *final* rustc
invocation, not any of the dependencies.

Dependencies will not be recompiled if they do not need to be, but the package
specified will always be compiled. The compiler will receive a number of
arguments unconditionally such as --extern, -L, etc. Note that dependencies are
recompiled when the flags they're compiled with change, so it is not allowed to
manually compile a package's dependencies and then compile the package against
the artifacts just generated.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-rustc; args={:?}",
           env::args().collect::<Vec<_>>());
    config.shell().set_verbose(options.flag_verbose);

    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    let spec = options.arg_pkgid.as_ref().map(|s| &s[..]);

    let opts = CompileOptions {
        config: config,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| &t[..]),
        features: &options.flag_features,
        no_default_features: options.flag_no_default_features,
        spec: spec,
        exec_engine: None,
        mode: ops::CompileMode::Build,
        release: options.flag_release,
        filter: ops::CompileFilter::new(options.flag_lib,
                                        &options.flag_bin,
                                        &options.flag_test,
                                        &options.flag_example,
                                        &options.flag_bench),
        target_rustc_args: options.arg_opts.as_ref().map(|a| &a[..]),
    };

    ops::compile(&root, &opts).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}


