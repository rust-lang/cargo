use crate::command_prelude::*;

use cargo::core::{GitReference, SourceId, Workspace};
use cargo::ops;
use cargo::util::IntoUrl;

use cargo_util::paths;

pub fn cli() -> Command {
    subcommand("install")
        .about("Install a Rust binary. Default location is $HOME/.cargo/bin")
        .arg_quiet()
        .arg(
            Arg::new("crate")
                .value_parser(clap::builder::NonEmptyStringValueParser::new())
                .num_args(0..),
        )
        .arg(
            opt("version", "Specify a version to install")
                .alias("vers")
                .value_name("VERSION")
                .requires("crate"),
        )
        .arg(
            opt("git", "Git URL to install the specified crate from")
                .value_name("URL")
                .conflicts_with_all(&["path", "index", "registry"]),
        )
        .arg(
            opt("branch", "Branch to use when installing from git")
                .value_name("BRANCH")
                .requires("git"),
        )
        .arg(
            opt("tag", "Tag to use when installing from git")
                .value_name("TAG")
                .requires("git"),
        )
        .arg(
            opt("rev", "Specific commit to use when installing from git")
                .value_name("SHA")
                .requires("git"),
        )
        .arg(
            opt("path", "Filesystem path to local crate to install")
                .value_name("PATH")
                .conflicts_with_all(&["git", "index", "registry"]),
        )
        .arg(flag(
            "list",
            "list all installed packages and their versions",
        ))
        .arg_jobs()
        .arg(flag("force", "Force overwriting existing crates or binaries").short('f'))
        .arg(flag("no-track", "Do not save tracking information"))
        .arg_features()
        .arg_profile("Install artifacts with the specified profile")
        .arg(flag(
            "debug",
            "Build in debug mode (with the 'dev' profile) instead of release mode",
        ))
        .arg_targets_bins_examples(
            "Install only the specified binary",
            "Install all binaries",
            "Install only the specified example",
            "Install all examples",
        )
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg(opt("root", "Directory to install packages into").value_name("DIR"))
        .arg(
            opt("index", "Registry index to install from")
                .value_name("INDEX")
                .requires("crate")
                .conflicts_with_all(&["git", "path", "registry"]),
        )
        .arg(
            opt("registry", "Registry to use")
                .value_name("REGISTRY")
                .requires("crate")
                .conflicts_with_all(&["git", "path", "index"]),
        )
        .arg_ignore_rust_version()
        .arg_message_format()
        .arg_timings()
        .after_help("Run `cargo help install` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let path = args.value_of_path("path", config);
    if let Some(path) = &path {
        config.reload_rooted_at(path)?;
    } else {
        // TODO: Consider calling set_search_stop_path(home).
        config.reload_rooted_at(config.home().clone().into_path_unlocked())?;
    }

    // In general, we try to avoid normalizing paths in Cargo,
    // but in these particular cases we need it to fix rust-lang/cargo#10283.
    // (Handle `SourceId::for_path` and `Workspace::new`,
    // but not `Config::reload_rooted_at` which is always cwd)
    let path = path.map(|p| paths::normalize_path(&p));

    let version = args.get_one::<String>("version").map(String::as_str);
    let krates = args
        .get_many::<String>("crate")
        .unwrap_or_default()
        .map(|k| resolve_crate(k, version))
        .collect::<crate::CargoResult<Vec<_>>>()?;

    let mut from_cwd = false;

    let source = if let Some(url) = args.get_one::<String>("git") {
        let url = url.into_url()?;
        let gitref = if let Some(branch) = args.get_one::<String>("branch") {
            GitReference::Branch(branch.clone())
        } else if let Some(tag) = args.get_one::<String>("tag") {
            GitReference::Tag(tag.clone())
        } else if let Some(rev) = args.get_one::<String>("rev") {
            GitReference::Rev(rev.clone())
        } else {
            GitReference::DefaultBranch
        };
        SourceId::for_git(&url, gitref)?
    } else if let Some(path) = &path {
        SourceId::for_path(path)?
    } else if krates.is_empty() {
        from_cwd = true;
        SourceId::for_path(config.cwd())?
    } else if let Some(index) = args.get_one::<String>("index") {
        SourceId::for_registry(&index.into_url()?)?
    } else if let Some(registry) = args.registry(config)? {
        SourceId::alt_registry(config, &registry)?
    } else {
        SourceId::crates_io(config)?
    };

    let root = args.get_one::<String>("root").map(String::as_str);

    // We only provide workspace information for local crate installation from
    // one of the following sources:
    // - From current working directory (only work for edition 2015).
    // - From a specific local file path (from `--path` arg).
    //
    // This workspace information is for emitting helpful messages from
    // `ArgMatchesExt::compile_options` and won't affect the actual compilation.
    let workspace = if from_cwd {
        args.workspace(config).ok()
    } else if let Some(path) = &path {
        Workspace::new(&path.join("Cargo.toml"), config).ok()
    } else {
        None
    };

    let mut compile_opts = args.compile_options(
        config,
        CompileMode::Build,
        workspace.as_ref(),
        ProfileChecking::Custom,
    )?;

    compile_opts.build_config.requested_profile =
        args.get_profile_name(config, "release", ProfileChecking::Custom)?;

    if args.flag("list") {
        ops::install_list(root, config)?;
    } else {
        ops::install(
            config,
            root,
            krates,
            source,
            from_cwd,
            &compile_opts,
            args.flag("force"),
            args.flag("no-track"),
        )?;
    }
    Ok(())
}

fn resolve_crate<'k>(
    mut krate: &'k str,
    mut version: Option<&'k str>,
) -> crate::CargoResult<(&'k str, Option<&'k str>)> {
    if let Some((k, v)) = krate.split_once('@') {
        if version.is_some() {
            anyhow::bail!("cannot specify both `@{v}` and `--version`");
        }
        if k.is_empty() {
            // by convention, arguments starting with `@` are response files
            anyhow::bail!("missing crate name for `@{v}`");
        }
        krate = k;
        version = Some(v);
    }
    Ok((krate, version))
}
