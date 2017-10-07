use cargo::ops;
use cargo::core::{SourceId, GitReference};
use cargo::util::{CliResult, Config, ToUrl};

#[derive(Deserialize)]
pub struct Options {
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_all_features: bool,
    flag_no_default_features: bool,
    flag_debug: bool,
    flag_bin: Vec<String>,
    flag_bins: bool,
    flag_example: Vec<String>,
    flag_examples: bool,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_root: Option<String>,
    flag_list: bool,
    flag_force: bool,
    flag_frozen: bool,
    flag_locked: bool,

    arg_crate: Vec<String>,
    flag_vers: Option<String>,

    flag_git: Option<String>,
    flag_branch: Option<String>,
    flag_tag: Option<String>,
    flag_rev: Option<String>,

    flag_path: Option<String>,
    #[serde(rename = "flag_Z")]
    flag_z: Vec<String>,
}

pub const USAGE: &'static str = "
Install a Rust binary

Usage:
    cargo install [options] [<crate>...]
    cargo install [options] --list

Specifying what crate to install:
    --vers VERS               Specify a version to install from crates.io
    --git URL                 Git URL to install the specified crate from
    --branch BRANCH           Branch to use when installing from git
    --tag TAG                 Tag to use when installing from git
    --rev SHA                 Specific commit to use when installing from git
    --path PATH               Filesystem path to local crate to install

Build and install options:
    -h, --help                Print this message
    -j N, --jobs N            Number of parallel jobs, defaults to # of CPUs
    -f, --force               Force overwriting existing crates or binaries
    --features FEATURES       Space-separated list of features to activate
    --all-features            Build all available features
    --no-default-features     Do not build the `default` feature
    --debug                   Build in debug mode instead of release mode
    --bin NAME                Install only the specified binary
    --bins                    Install all binaries
    --example NAME            Install only the specified example
    --examples                Install all examples
    --root DIR                Directory to install packages into
    -v, --verbose ...         Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet               Less output printed to stdout
    --color WHEN              Coloring: auto, always, never
    --frozen                  Require Cargo.lock and cache are up to date
    --locked                  Require Cargo.lock is up to date
    -Z FLAG ...               Unstable (nightly-only) flags to Cargo

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

The `--list` option will list all installed packages (and their versions).
";

pub fn execute(options: Options, config: &mut Config) -> CliResult {
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked,
                     &options.flag_z)?;

    let compile_opts = ops::CompileOptions {
        config: config,
        jobs: options.flag_jobs,
        target: None,
        features: &options.flag_features,
        all_features: options.flag_all_features,
        no_default_features: options.flag_no_default_features,
        spec: ops::Packages::Packages(&[]),
        mode: ops::CompileMode::Build,
        release: !options.flag_debug,
        filter: ops::CompileFilter::new(false,
                                        &options.flag_bin, options.flag_bins,
                                        &[], false,
                                        &options.flag_example, options.flag_examples,
                                        &[], false,
                                        false),
        message_format: ops::MessageFormat::Human,
        target_rustc_args: None,
        target_rustdoc_args: None,
    };

    let source = if let Some(url) = options.flag_git {
        let url = url.to_url()?;
        let gitref = if let Some(branch) = options.flag_branch {
            GitReference::Branch(branch)
        } else if let Some(tag) = options.flag_tag {
            GitReference::Tag(tag)
        } else if let Some(rev) = options.flag_rev {
            GitReference::Rev(rev)
        } else {
            GitReference::Branch("master".to_string())
        };
        SourceId::for_git(&url, gitref)?
    } else if let Some(path) = options.flag_path {
        SourceId::for_path(&config.cwd().join(path))?
    } else if options.arg_crate.is_empty() {
        SourceId::for_path(config.cwd())?
    } else {
        SourceId::crates_io(config)?
    };

    let krates = options.arg_crate.iter().map(|s| &s[..]).collect::<Vec<_>>();
    let vers = options.flag_vers.as_ref().map(|s| &s[..]);
    let root = options.flag_root.as_ref().map(|s| &s[..]);

    if options.flag_list {
        ops::install_list(root, config)?;
    } else {
        ops::install(root, krates, &source, vers, &compile_opts, options.flag_force)?;
    }
    Ok(())
}
