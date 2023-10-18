use crate::command_prelude::*;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::format_err;
use cargo::core::{GitReference, SourceId, Workspace};
use cargo::ops;
use cargo::util::IntoUrl;
use cargo::util::ToSemver;
use cargo::util::VersionReqExt;
use cargo::CargoResult;
use semver::VersionReq;

use cargo_util::paths;

pub fn cli() -> Command {
    subcommand("install")
        .about("Install a Rust binary. Default location is $HOME/.cargo/bin")
        .arg(
            Arg::new("crate")
                .value_name("CRATE[@<VER>]")
                .help("Select the package from the given source")
                .value_parser(parse_crate)
                .num_args(0..),
        )
        .arg(
            opt("version", "Specify a version to install")
                .alias("vers")
                .value_name("VERSION")
                .value_parser(parse_semver_flag)
                .requires("crate"),
        )
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
        .arg(opt("root", "Directory to install packages into").value_name("DIR"))
        .arg(flag("force", "Force overwriting existing crates or binaries").short('f'))
        .arg(flag("no-track", "Do not save tracking information"))
        .arg(flag(
            "list",
            "list all installed packages and their versions",
        ))
        .arg_ignore_rust_version()
        .arg_message_format()
        .arg_quiet()
        .arg_targets_bins_examples(
            "Install only the specified binary",
            "Install all binaries",
            "Install only the specified example",
            "Install all examples",
        )
        .arg_features()
        .arg_parallel()
        .arg(flag(
            "debug",
            "Build in debug mode (with the 'dev' profile) instead of release mode",
        ))
        .arg_redundant_default_mode("release", "install", "debug")
        .arg_profile("Install artifacts with the specified profile")
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_timings()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help install</>` for more detailed information.\n"
        ))
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

    let version = args.get_one::<VersionReq>("version");
    let krates = args
        .get_many::<CrateVersion>("crate")
        .unwrap_or_default()
        .cloned()
        .map(|(krate, local_version)| resolve_crate(krate, local_version, version))
        .collect::<crate::CargoResult<Vec<_>>>()?;

    for (crate_name, _) in krates.iter() {
        if let Some(toolchain) = crate_name.strip_prefix("+") {
            return Err(anyhow!(
                "invalid character `+` in package name: `+{toolchain}`
    Use `cargo +{toolchain} install` if you meant to use the `{toolchain}` toolchain."
            )
            .into());
        }

        if let Ok(url) = crate_name.into_url() {
            if matches!(url.scheme(), "http" | "https") {
                return Err(anyhow!(
                    "invalid package name: `{url}`
    Use `cargo install --git {url}` if you meant to install from a git repository."
                )
                .into());
            }
        }
    }

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
    } else if let Some(reg_or_index) = args.registry_or_index(config)? {
        match reg_or_index {
            ops::RegistryOrIndex::Registry(r) => SourceId::alt_registry(config, &r)?,
            ops::RegistryOrIndex::Index(url) => SourceId::for_registry(&url)?,
        }
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

type CrateVersion = (String, Option<VersionReq>);

fn parse_crate(krate: &str) -> crate::CargoResult<CrateVersion> {
    let (krate, version) = if let Some((k, v)) = krate.split_once('@') {
        if k.is_empty() {
            // by convention, arguments starting with `@` are response files
            anyhow::bail!("missing crate name before '@'");
        }
        let krate = k.to_owned();
        let version = Some(parse_semver_flag(v)?);
        (krate, version)
    } else {
        let krate = krate.to_owned();
        let version = None;
        (krate, version)
    };

    if krate.is_empty() {
        anyhow::bail!("crate name is empty");
    }

    Ok((krate, version))
}

/// Parses x.y.z as if it were =x.y.z, and gives CLI-specific error messages in the case of invalid
/// values.
fn parse_semver_flag(v: &str) -> CargoResult<VersionReq> {
    // If the version begins with character <, >, =, ^, ~ parse it as a
    // version range, otherwise parse it as a specific version
    let first = v
        .chars()
        .next()
        .ok_or_else(|| format_err!("no version provided for the `--version` flag"))?;

    let is_req = "<>=^~".contains(first) || v.contains('*');
    if is_req {
        match v.parse::<VersionReq>() {
            Ok(v) => Ok(v),
            Err(_) => bail!(
                "the `--version` provided, `{}`, is \
                     not a valid semver version requirement\n\n\
                     Please have a look at \
                     https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html \
                     for the correct format",
                v
            ),
        }
    } else {
        match v.to_semver() {
            Ok(v) => Ok(VersionReq::exact(&v)),
            Err(e) => {
                let mut msg = e.to_string();

                // If it is not a valid version but it is a valid version
                // requirement, add a note to the warning
                if v.parse::<VersionReq>().is_ok() {
                    msg.push_str(&format!(
                        "\n\n  tip: if you want to specify SemVer range, \
                             add an explicit qualifier, like '^{}'",
                        v
                    ));
                }
                bail!(msg);
            }
        }
    }
}

fn resolve_crate(
    krate: String,
    local_version: Option<VersionReq>,
    version: Option<&VersionReq>,
) -> crate::CargoResult<CrateVersion> {
    let version = match (local_version, version) {
        (Some(_), Some(_)) => {
            anyhow::bail!("cannot specify both `@<VERSION>` and `--version <VERSION>`");
        }
        (Some(l), None) => Some(l),
        (None, Some(g)) => Some(g.to_owned()),
        (None, None) => None,
    };
    Ok((krate, version))
}
