use crate::command_prelude::*;

use cargo::core::features;
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
        .arg(opt("check", "Runs `rustdoc --check` (nightly only)"))
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .arg_unit_graph()
        .after_help("Run `cargo help doc` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    if args.is_present("check") {
        if !features::nightly_features_allowed() {
            Err(CliError::new(
                anyhow::format_err!("This option is only available in nightly"),
                1,
            ))?;
        }
        exec_doc(
            config,
            args,
            CompileMode::DocCheck,
            ProfileChecking::Unchecked,
        )
    } else {
        exec_doc(
            config,
            args,
            CompileMode::Doc {
                deps: !args.is_present("no-deps"),
            },
            ProfileChecking::Checked,
        )
    }
}

pub fn exec_doc(
    config: &mut Config,
    args: &ArgMatches<'_>,
    mode: CompileMode,
    profile: ProfileChecking,
) -> CliResult {
    let ws = args.workspace(config)?;

    let mut compile_opts = args.compile_options(config, mode, Some(&ws), profile)?;

    if !mode.is_check() {
        compile_opts.rustdoc_document_private_items = args.is_present("document-private-items");
    }

    let mut doc_opts = DocOptions {
        open_result: false,
        compile_opts,
    };

    if !mode.is_check() {
        doc_opts.open_result = args.is_present("open");
    }

    ops::doc(&ws, &doc_opts)?;
    Ok(())
}
