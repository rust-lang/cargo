use crate::command_prelude::*;

use cargo::ops::{self, DocOptions};

pub fn cli() -> Command {
    subcommand("doc")
        // subcommand aliases are handled in aliased_command()
        // .alias("d")
        .about("Build a package's documentation")
        .arg(flag(
            "open",
            "Opens the docs in a browser after the operation",
        ))
        .arg(flag(
            "no-deps",
            "Don't build documentation for dependencies",
        ))
        .arg(flag("document-private-items", "Document private items"))
        .arg_ignore_rust_version()
        .arg_message_format()
        .arg_quiet()
        .arg_package_spec(
            "Package to document",
            "Document all packages in the workspace",
            "Exclude packages from the build",
        )
        .arg_features()
        .arg_targets_lib_bin_example(
            "Document only this package's library",
            "Document only the specified binary",
            "Document all binaries",
            "Document only the specified example",
            "Document all examples",
        )
        .arg_parallel()
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_unit_graph()
        .arg_timings()
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help doc</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    let mode = CompileMode::Doc {
        deps: !args.flag("no-deps"),
    };
    let mut compile_opts =
        args.compile_options(config, mode, Some(&ws), ProfileChecking::Custom)?;
    compile_opts.rustdoc_document_private_items = args.flag("document-private-items");

    let doc_opts = DocOptions {
        open_result: args.flag("open"),
        compile_opts,
    };
    ops::doc(&ws, &doc_opts)?;
    Ok(())
}
