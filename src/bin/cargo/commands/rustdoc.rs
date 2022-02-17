use cargo::ops::{self, DocOptions};

use crate::command_prelude::*;

pub fn cli() -> App {
    subcommand("rustdoc")
        .trailing_var_arg(true)
        .about("Build a package's documentation, using specified custom flags.")
        .arg_quiet()
        .arg(Arg::new("args").multiple_values(true))
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
        .arg_profile("Build artifacts with the specified profile")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .arg_unit_graph()
        .arg_ignore_rust_version()
        .arg_timings()
        .after_help("Run `cargo help rustdoc` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    let mut compile_opts = args.compile_options_for_single_package(
        config,
        CompileMode::Doc { deps: false },
        Some(&ws),
        ProfileChecking::Custom,
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
