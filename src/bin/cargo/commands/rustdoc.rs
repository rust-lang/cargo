use cargo::ops::{self, DocOptions};

use crate::command_prelude::*;

pub fn cli() -> App {
    subcommand("rustdoc")
        .setting(AppSettings::TrailingVarArg)
        .about("Build a package's documentation, using specified custom flags.")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(Arg::with_name("args").multiple(true))
        .arg(opt(
            "open",
            "Opens the docs in a browser after the operation",
        ))
        .arg_package("Package to document")
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
            "Build all targets",
        )
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .after_help(
            "\
The specified target for the current package (or package specified by SPEC if
provided) will be documented with the specified `<opts>...` being passed to the
final rustdoc invocation. Dependencies will not be documented as part of this
command. Note that rustdoc will still unconditionally receive arguments such
as `-L`, `--extern`, and `--crate-type`, and the specified `<opts>...` will
simply be added to the rustdoc invocation.

If the `--package` argument is given, then SPEC is a package ID specification
which indicates which package should be documented. If it is not given, then the
current package is documented. For more information on SPEC and its format, see
the `cargo help pkgid` command.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let mut compile_opts = args.compile_options_for_single_package(
        config,
        CompileMode::Doc { deps: false },
        Some(&ws),
    )?;
    let target_args = values(args, "args");
    compile_opts.target_rustdoc_args = if target_args.is_empty() {
        None
    } else {
        Some(target_args)
    };
    let doc_opts = DocOptions {
        open_result: args.is_present("open"),
        compile_opts,
    };
    ops::doc(&ws, &doc_opts)?;
    Ok(())
}
