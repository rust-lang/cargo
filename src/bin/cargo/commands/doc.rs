use crate::command_prelude::*;

use cargo::ops::{self, DocOptions};

pub fn cli() -> App {
    subcommand("doc")
        .about("Build a package's documentation")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(opt(
            "open",
            "Opens the docs in a browser after the operation",
        ))
        .arg_package_spec(
            "Package to document",
            "Document all packages in the workspace",
            "Exclude packages from the build",
        )
        .arg(opt("no-deps", "Don't build documentation for dependencies"))
        .arg(opt("document-private-items", "Document private items"))
        .arg_jobs()
        .arg_targets_lib_bin(
            "Document only this package's library",
            "Document only the specified binary",
            "Document all binaries",
        )
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .arg_unit_graph()
        .after_help(
            "\
By default the documentation for the local package and all dependencies is
built. The output is all placed in `target/doc` in rustdoc's usual format.

All packages in the workspace are documented if the `--workspace` flag is
supplied. The `--workspace` flag is automatically assumed for a virtual
manifest. Note that `--exclude` has to be specified in conjunction with the
`--workspace` flag.

If the `--package` argument is given, then SPEC is a package ID specification
which indicates which package should be documented. If it is not given, then the
current package is documented. For more information on SPEC and its format, see
the `cargo help pkgid` command.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let mode = CompileMode::Doc {
        deps: !args.is_present("no-deps"),
    };
    let mut compile_opts =
        args.compile_options(config, mode, Some(&ws), ProfileChecking::Checked)?;
    compile_opts.rustdoc_document_private_items = args.is_present("document-private-items");

    let doc_opts = DocOptions {
        open_result: args.is_present("open"),
        compile_opts,
    };
    ops::doc(&ws, &doc_opts)?;
    Ok(())
}
