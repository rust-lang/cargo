use command_prelude::*;

use cargo::core::{GitReference, SourceId};
use cargo::ops::{self, CompileMode};
use cargo::util::ToUrl;

pub fn cli() -> App {
    subcommand("install")
        .about("Create a new cargo package in an existing directory")
        .arg(Arg::with_name("crate").multiple(true))
        .arg(
            opt("version", "Specify a version to install from crates.io")
                .alias("vers")
                .value_name("VERSION"),
        )
        .arg(opt("git", "Git URL to install the specified crate from").value_name("URL"))
        .arg(opt("branch", "Branch to use when installing from git").value_name("BRANCH"))
        .arg(opt("tag", "Tag to use when installing from git").value_name("TAG"))
        .arg(opt("rev", "Specific commit to use when installing from git").value_name("SHA"))
        .arg(opt("path", "Filesystem path to local crate to install").value_name("PATH"))
        .arg(opt(
            "list",
            "list all installed packages and their versions",
        ))
        .arg_jobs()
        .arg(opt("force", "Force overwriting existing crates or binaries").short("f"))
        .arg_features()
        .arg(opt("debug", "Build in debug mode instead of release mode"))
        .arg_targets_bins_examples(
            "Install only the specified binary",
            "Install all binaries",
            "Install only the specified example",
            "Install all examples",
        )
        .arg(opt("root", "Directory to install packages into").value_name("DIR"))
        .after_help(
            "\
This command manages Cargo's local set of installed binary crates. Only packages
which have [[bin]] targets can be installed, and all binaries are installed into
the installation root's `bin` folder. The installation root is determined, in
order of precedence, by `--root`, `$CARGO_INSTALL_ROOT`, the `install.root`
configuration key, and finally the home directory (which is either
`$CARGO_HOME` if set or `$HOME/.cargo` by default).

There are multiple sources from which a crate can be installed. The default
location is crates.io but the `--git` and `--path` flags can change this source.
If the source contains more than one package (such as crates.io or a git
repository with multiple crates) the `<crate>` argument is required to indicate
which crate should be installed.

Crates from crates.io can optionally specify the version they wish to install
via the `--vers` flags, and similarly packages from git repositories can
optionally specify the branch, tag, or revision that should be installed. If a
crate has multiple binaries, the `--bin` argument can selectively install only
one of them, and if you'd rather install examples the `--example` argument can
be used as well.

By default cargo will refuse to overwrite existing binaries. The `--force` flag
enables overwriting existing binaries. Thus you can reinstall a crate with
`cargo install --force <crate>`.

As a special convenience, omitting the <crate> specification entirely will
install the crate in the current directory. That is, `install` is equivalent to
the more explicit `install --path .`.

If the source is crates.io or `--git` then by default the crate will be built
in a temporary target directory.  To avoid this, the target directory can be
specified by setting the `CARGO_TARGET_DIR` environment variable to a relative
path.  In particular, this can be useful for caching build artifacts on
continuous integration systems.",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let mut compile_opts = args.compile_options(config, CompileMode::Build)?;
    compile_opts.release = !args.is_present("debug");

    let krates = args.values_of("crate")
        .unwrap_or_default()
        .collect::<Vec<_>>();

    let source = if let Some(url) = args.value_of("git") {
        let url = url.to_url()?;
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
        SourceId::for_path(config.cwd())?
    } else {
        SourceId::crates_io(config)?
    };

    let version = args.value_of("version");
    let root = args.value_of("root");

    if args.is_present("list") {
        ops::install_list(root, config)?;
    } else {
        ops::install(
            root,
            krates,
            &source,
            version,
            &compile_opts,
            args.is_present("force"),
        )?;
    }
    Ok(())
}
