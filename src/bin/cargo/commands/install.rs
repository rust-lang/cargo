use crate::command_prelude::*;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::format_err;
use cargo::CargoResult;
use cargo::core::{GitReference, SourceId, Workspace};
use cargo::ops;
use cargo::util::IntoUrl;
use cargo::util::VersionExt;
use cargo_util_schemas::manifest::PackageName;
use itertools::Itertools;
use semver::VersionReq;

use cargo_util::paths;

pub fn cli() -> Command {
    subcommand("install")
        .about("Install a Rust binary")
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
            opt("path", "Filesystem path to local crate to install from")
                .value_name("PATH")
                .conflicts_with_all(&["git", "index", "registry"])
                .add(clap_complete::engine::ArgValueCompleter::new(
                    clap_complete::engine::PathCompleter::any()
                        .filter(|path| path.join("Cargo.toml").exists()),
                )),
        )
        .arg(opt("root", "Directory to install packages into").value_name("DIR"))
        .arg(flag("force", "Force overwriting existing crates or binaries").short('f'))
        .arg_dry_run("Perform all checks without installing (unstable)")
        .arg(flag("no-track", "Do not save tracking information"))
        .arg(flag(
            "list",
            "List all installed packages and their versions",
        ))
        .arg_ignore_rust_version()
        .arg_message_format()
        .arg_silent_suggestion()
        .arg_targets_bins_examples(
            "Install only the specified binary",
            "Install all binaries",
            "Install only the specified example",
            "Install all examples",
        )
        .arg_features()
        .arg_parallel()
        .arg(
            flag(
                "debug",
                "Build in debug mode (with the 'dev' profile) instead of release mode",
            )
            .conflicts_with("profile"),
        )
        .arg_redundant_default_mode("release", "install", "debug")
        .arg_profile("Install artifacts with the specified profile")
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_timings()
        .arg_lockfile_path()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help install</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let path = args.value_of_path("path", gctx);
    if let Some(path) = &path {
        gctx.reload_rooted_at(path)?;
    } else {
        // TODO: Consider calling set_search_stop_path(home).
        gctx.reload_rooted_at(gctx.home().clone().into_path_unlocked())?;
    }

    // In general, we try to avoid normalizing paths in Cargo,
    // but in these particular cases we need it to fix rust-lang/cargo#10283.
    // (Handle `SourceId::for_path` and `Workspace::new`,
    // but not `GlobalContext::reload_rooted_at` which is always cwd)
    let path = path.map(|p| paths::normalize_path(&p));

    let version = args.get_one::<VersionReq>("version");
    let krates = args
        .get_many::<CrateVersion>("crate")
        .unwrap_or_default()
        .cloned()
        .dedup_by(|x, y| x == y)
        .map(|(krate, local_version)| resolve_crate(krate, local_version, version))
        .collect::<crate::CargoResult<Vec<_>>>()?;

    for (crate_name, _) in krates.iter() {
        let package_name = PackageName::new(crate_name);
        if !crate_name.contains("@") && package_name.is_err() {
            for (idx, ch) in crate_name.char_indices() {
                if !(unicode_ident::is_xid_continue(ch) || ch == '-') {
                    let mut suggested_crate_name = crate_name.to_string();
                    suggested_crate_name.insert_str(idx, "@");
                    if let Ok((_, Some(_))) = parse_crate(&suggested_crate_name.as_str()) {
                        let err = package_name.unwrap_err();
                        return Err(
                            anyhow::format_err!("{err}\n\n\
                                help: if this is meant to be a package name followed by a version, insert an `@` like `{suggested_crate_name}`").into());
                    }
                }
            }
        }

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
        SourceId::for_path(gctx.cwd())?
    } else if let Some(reg_or_index) = args.registry_or_index(gctx)? {
        match reg_or_index {
            ops::RegistryOrIndex::Registry(r) => SourceId::alt_registry(gctx, &r)?,
            ops::RegistryOrIndex::Index(url) => SourceId::for_registry(&url)?,
        }
    } else {
        SourceId::crates_io(gctx)?
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
        args.workspace(gctx).ok()
    } else if let Some(path) = &path {
        Workspace::new(&path.join("Cargo.toml"), gctx).ok()
    } else {
        None
    };

    let mut compile_opts = args.compile_options(
        gctx,
        UserIntent::Build,
        workspace.as_ref(),
        ProfileChecking::Custom,
    )?;

    compile_opts.build_config.requested_profile =
        args.get_profile_name("release", ProfileChecking::Custom)?;
    if args.dry_run() {
        gctx.cli_unstable().fail_if_stable_opt("--dry-run", 11123)?;
    }

    let requested_lockfile_path = args.lockfile_path(gctx)?;
    // 14421: lockfile path should imply --locked on running `install`
    if requested_lockfile_path.is_some() {
        gctx.set_locked(true);
    }

    if args.flag("list") {
        ops::install_list(root, gctx)?;
    } else {
        ops::install(
            gctx,
            root,
            krates,
            source,
            from_cwd,
            &compile_opts,
            args.flag("force"),
            args.flag("no-track"),
            args.dry_run(),
            requested_lockfile_path.as_deref(),
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

    if let Some(stripped) = v.strip_prefix("v") {
        bail!(
            "the version provided, `{v}` is not a valid SemVer requirement\n\n\
            help: try changing the version to `{stripped}`",
        )
    }
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
        match v.trim().parse::<semver::Version>() {
            Ok(v) => Ok(v.to_exact_req()),
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
