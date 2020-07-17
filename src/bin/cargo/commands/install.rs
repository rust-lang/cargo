use crate::command_prelude::*;

use cargo::core::{GitReference, SourceId};
use cargo::ops;
use cargo::util::IntoUrl;

pub fn cli() -> App {
    subcommand("install")
        .about("Install a Rust binary. Default location is $HOME/.cargo/bin")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(Arg::with_name("crate").empty_values(false).multiple(true))
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
        .arg(opt(
            "list",
            "list all installed packages and their versions",
        ))
        .arg_jobs()
        .arg(opt("force", "Force overwriting existing crates or binaries").short("f"))
        .arg(opt("no-track", "Do not save tracking information"))
        .arg_features()
        .arg_profile("Install artifacts with the specified profile")
        .arg(opt("debug", "Build in debug mode instead of release mode"))
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
        .after_help(
            "\
This command manages Cargo's local set of installed binary crates. Only
packages which have executable [[bin]] or [[example]] targets can be
installed, and all executables are installed into the installation root's
`bin` folder. The installation root is determined, in order of precedence, by
`--root`, `$CARGO_INSTALL_ROOT`, the `install.root` configuration key, and
finally the home directory (which is either `$CARGO_HOME` if set or
`$HOME/.cargo` by default).

There are multiple sources from which a crate can be installed. The default
location is crates.io but the `--git`, `--path`, and `--registry` flags can
change this source. If the source contains more than one package (such as
crates.io or a git repository with multiple crates) the `<crate>` argument is
required to indicate which crate should be installed.

Crates from crates.io can optionally specify the version they wish to install
via the `--version` flags, and similarly packages from git repositories can
optionally specify the branch, tag, or revision that should be installed. If a
crate has multiple binaries, the `--bin` argument can selectively install only
one of them, and if you'd rather install examples the `--example` argument can
be used as well.

If the package is already installed, Cargo will reinstall it if the installed
version does not appear to be up-to-date. Installing with `--path` will always
build and install, unless there are conflicting binaries from another package.

If the source is crates.io or `--git` then by default the crate will be built
in a temporary target directory. To avoid this, the target directory can be
specified by setting the `CARGO_TARGET_DIR` environment variable to a relative
path. In particular, this can be useful for caching build artifacts on
continuous integration systems.",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    if let Some(path) = args.value_of_path("path", config) {
        config.reload_rooted_at(path)?;
    } else {
        config.reload_rooted_at(config.home().clone().into_path_unlocked())?;
    }

    let workspace = args.workspace(config).ok();
    let mut compile_opts = args.compile_options(
        config,
        CompileMode::Build,
        workspace.as_ref(),
        ProfileChecking::Checked,
    )?;

    compile_opts.build_config.requested_profile =
        args.get_profile_name(config, "release", ProfileChecking::Checked)?;

    let krates = args
        .values_of("crate")
        .unwrap_or_default()
        .collect::<Vec<_>>();

    let mut from_cwd = false;

    let source = if let Some(url) = args.value_of("git") {
        let url = url.into_url()?;
        let gitref = if let Some(branch) = args.value_of("branch") {
            GitReference::Branch(branch.to_string())
        } else if let Some(tag) = args.value_of("tag") {
            GitReference::Tag(tag.to_string())
        } else if let Some(rev) = args.value_of("rev") {
            GitReference::Rev(rev.to_string())
        } else {
            GitReference::Branch("master".to_string())
        };
        SourceId::for_git(&url, gitref)?
    } else if let Some(path) = args.value_of_path("path", config) {
        SourceId::for_path(&path)?
    } else if krates.is_empty() {
        from_cwd = true;
        SourceId::for_path(config.cwd())?
    } else if let Some(registry) = args.registry(config)? {
        SourceId::alt_registry(config, &registry)?
    } else if let Some(index) = args.value_of("index") {
        SourceId::for_registry(&index.into_url()?)?
    } else {
        SourceId::crates_io(config)?
    };

    let version = args.value_of("version");
    let root = args.value_of("root");

    if args.is_present("list") {
        ops::install_list(root, config)?;
    } else {
        ops::install(
            config,
            root,
            krates,
            source,
            from_cwd,
            version,
            &compile_opts,
            args.is_present("force"),
            args.is_present("no-track"),
        )?;
    }
    Ok(())
}
