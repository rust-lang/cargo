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
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .arg(
            Arg::with_name("dep-mode")
                .long("dep-mode")
                .help("Configure the set of dependencies to document")
                .takes_value(true)
                .possible_values(&["dev", "normal", "all", "build"])
                .default_value("normal"),
        )
        .after_help(
            "\
By default the documentation for the local package and all dependencies is
built. The output is all placed in `target/doc` in rustdoc's usual format.

All packages in the workspace are documented if the `--all` flag is supplied. The
`--all` flag is automatically assumed for a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--all` flag.

If the `--package` argument is given, then SPEC is a package ID specification
which indicates which package should be documented. If it is not given, then the
current package is documented. For more information on SPEC and its format, see
the `cargo help pkgid` command.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let dep_mode = if args.is_present("no-deps") {
        DepDocMode::NoDeps
    } else {
        match args.value_of("dev") {
            Some("dev") => DepDocMode::Development,
            Some("all") => DepDocMode::All,
            Some("build") => DepDocMode::Build,
            _ => DepDocMode::Normal,
        }
    };
    let mode = CompileMode::Doc { dep_mode };
    let mut compile_opts = args.compile_options(config, mode, Some(&ws))?;

    if !config.cli_unstable().unstable_options
        && dep_mode != DepDocMode::Normal
        && dep_mode != DepDocMode::NoDeps
    {
        return Err(failure::format_err!(
            "`cargo doc --dep-mode` is unstable, pass `-Z unstable-options` to enable it"
        )
        .into());
    }

    compile_opts.local_rustdoc_args = if args.is_present("document-private-items") {
        Some(vec!["--document-private-items".to_string()])
    } else {
        None
    };
    let doc_opts = DocOptions {
        open_result: args.is_present("open"),
        compile_opts,
    };
    ops::doc(&ws, &doc_opts)?;
    Ok(())
}
