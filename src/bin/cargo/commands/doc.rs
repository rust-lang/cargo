use crate::command_prelude::*;

use anyhow::anyhow;
use cargo::ops::{self, CompileFilter, DocOptions, FilterRule, LibRule};

pub fn cli() -> App {
    subcommand("doc")
        // subcommand aliases are handled in aliased_command()
        // .alias("d")
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
        .arg(
            opt(
                "scrape-examples",
                "Scrape examples to include as function documentation",
            )
            .value_name("FLAGS"),
        )
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

    // TODO(wcrichto): move scrape example configuration into Cargo.toml before stabilization
    // See: https://github.com/rust-lang/cargo/pull/9525#discussion_r728470927
    compile_opts.rustdoc_scrape_examples = match args.value_of("scrape-examples") {
        Some(s) => Some(match s {
            "all" => CompileFilter::new_all_targets(),
            "examples" => CompileFilter::new(
                LibRule::False,
                FilterRule::none(),
                FilterRule::none(),
                FilterRule::All,
                FilterRule::none(),
            ),
            _ => {
                return Err(CliError::from(anyhow!(
                    r#"--scrape-examples must take "all" or "examples" as an argument"#
                )));
            }
        }),
        None => None,
    };

    if compile_opts.rustdoc_scrape_examples.is_some() {
        config
            .cli_unstable()
            .fail_if_stable_opt("--scrape-examples", 9910)?;
    }

    let doc_opts = DocOptions {
        open_result: args.is_present("open"),
        compile_opts,
    };
    ops::doc(&ws, &doc_opts)?;
    Ok(())
}
