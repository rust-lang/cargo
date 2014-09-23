use docopt;

use cargo::ops;
use cargo::core::{MultiShell};
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

docopt!(Options, "
Build a package's documentation

Usage:
    cargo doc [options]

Options:
    -h, --help              Print this message
    --no-deps               Don't build documentation for dependencies
    -j N, --jobs N          The number of jobs to run in parallel
    --features FEATURES     Space-separated list of features to also build
    --no-default-features   Do not build the `default` feature
    --manifest-path PATH    Path to the manifest to document
    -v, --verbose           Use verbose output

By default the documentation for the local package and all dependencies is
built. The output is all placed in `target/doc` in rustdoc's usual format.
",  flag_jobs: Option<uint>,
    flag_manifest_path: Option<String>,
    flag_features: Vec<String>)

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);

    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    let mut doc_opts = ops::DocOptions {
        all: !options.flag_no_deps,
        compile_opts: ops::CompileOptions {
            env: if options.flag_no_deps {"doc"} else {"doc-all"},
            shell: shell,
            jobs: options.flag_jobs,
            target: None,
            dev_deps: false,
            features: options.flag_features.as_slice(),
            no_default_features: options.flag_no_default_features,
            spec: None,
        },
    };

    try!(ops::doc(&root, &mut doc_opts).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));

    Ok(None)
}

