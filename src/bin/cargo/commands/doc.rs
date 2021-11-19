use std::path::PathBuf;

use crate::command_prelude::*;

use cargo::{
    ops::{self, DocOptions},
    util::Filesystem,
    CargoResult,
};

pub fn cli() -> App {
    subcommand("doc")
        // subcommand aliases are handled in aliased_command()
        // .alias("d")
        .about("Build a package's documentation")
        .arg(opt("quiet", "Do not print cargo log messages").short("q"))
        .arg(opt(
            "open",
            "Opens the docs in a browser after the operation",
        ))
        .arg(
            opt(
                "publish-dir",
                "Directory to copy the generated documentation to",
            )
            .value_name("DIR"),
        )
        .arg_package_spec(
            "Package to document",
            "Document all packages in the workspace",
            "Exclude packages from the build",
        )
        .arg(opt("no-deps", "Don't build documentation for dependencies"))
        .arg(opt("document-private-items", "Document private items"))
        .arg_jobs()
        .arg_targets_lib_bin_example(
            "Document only this package's library",
            "Document only the specified binary",
            "Document all binaries",
            "Document only the specified example",
            "Document all examples",
        )
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .arg_ignore_rust_version()
        .arg_unit_graph()
        .after_help("Run `cargo help doc` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let mode = CompileMode::Doc {
        deps: !args.is_present("no-deps"),
    };
    let mut compile_opts =
        args.compile_options(config, mode, Some(&ws), ProfileChecking::Custom)?;
    compile_opts.rustdoc_document_private_items = args.is_present("document-private-items");
    let publish_dir_arg = args.value_of("publish-dir");
    let publish_dir = resolved_doc_publish_dir(publish_dir_arg, config)?;

    let doc_opts = DocOptions {
        open_result: args.is_present("open"),
        publish_dir,
        compile_opts,
    };
    ops::doc(&ws, &doc_opts)?;
    Ok(())
}

/// Determines the publish_dir directory where documentation is placed.
pub(crate) fn resolved_doc_publish_dir(
    flag: Option<&str>,
    config: &Config,
) -> CargoResult<Option<Filesystem>> {
    let config_publish_dir = config.get_path("doc.publish-dir")?;
    Ok(flag
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("CARGO_DOC_PUBLISH_DIR").map(PathBuf::from))
        .or_else(move || config_publish_dir.map(|v| v.val))
        .map(Filesystem::new))
}
