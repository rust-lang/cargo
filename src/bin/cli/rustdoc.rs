use clap::AppSettings;

use super::utils::*;

pub fn cli() -> App {
    subcommand("rustdoc")
        .setting(AppSettings::TrailingVarArg)
        .about("Build a package's documentation, using specified custom flags.")
        .arg(Arg::with_name("args").multiple(true))
        .arg(opt("open", "Opens the docs in a browser after the operation"))
        .arg_single_package("Package to document")
        .arg_jobs()
        .arg_targets_all(
            "Build only this package's library",
            "Build only the specified binary",
            "Build all binaries",
            "Build only the specified example",
            "Build all examples",
            "Build only the specified test target",
            "Build all tests",
            "Build only the specified bench target",
            "Build all benches",
            "Build all targets (default)",
        )
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_manifest_path()
        .arg_message_format()
        .after_help("\
The specified target for the current package (or package specified by SPEC if
provided) will be documented with the specified <opts>... being passed to the
final rustdoc invocation. Dependencies will not be documented as part of this
command.  Note that rustdoc will still unconditionally receive arguments such
as -L, --extern, and --crate-type, and the specified <opts>...  will simply be
added to the rustdoc invocation.

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be documented. If it is not given, then the
current package is documented. For more information on SPEC and its format, see
the `cargo help pkgid` command.
")
}
