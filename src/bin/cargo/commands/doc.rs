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
        .arg(
            opt(
                "deps",
                "Build documentation for given kinds of dependencies",
            )
            .takes_value(true)
            .multiple(true)
            .require_delimiter(true)
            .possible_values(&["all", "build", "dev", "normal"])
            .conflicts_with("no-deps"),
        )
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
        .after_help("Run `cargo help doc` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;

    let mut deps_mode = DocDepsMode::default();
    if let Some(deps) = args.values_of("deps") {
        config.cli_unstable().fail_if_stable_opt("--deps", 8608)?;

        // Disable documenting of transitive dependencies.
        deps_mode.deps = false;
        for dep in deps {
            match dep {
                "all" => {
                    deps_mode.build = true;
                    deps_mode.normal = true;
                    deps_mode.dev = true;
                }
                "build" => deps_mode.build = true,
                "normal" => deps_mode.normal = true,
                "dev" => deps_mode.dev = true,
                _ => unreachable!(),
            }
        }
    } else {
        let no_deps = args.is_present("no-deps");
        deps_mode.deps = !no_deps;
        deps_mode.normal = !no_deps;
    }

    let mut compile_opts = args.compile_options(
        config,
        CompileMode::Doc(deps_mode),
        Some(&ws),
        ProfileChecking::Checked,
    )?;
    compile_opts.rustdoc_document_private_items = args.is_present("document-private-items");

    let doc_opts = DocOptions {
        open_result: args.is_present("open"),
        compile_opts,
    };
    ops::doc(&ws, &doc_opts)?;
    Ok(())
}
