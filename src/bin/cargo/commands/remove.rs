use cargo::core::dependency::DepKind;
use cargo::core::PackageIdSpec;
use cargo::core::Resolve;
use cargo::core::Workspace;
use cargo::ops::cargo_remove::remove;
use cargo::ops::cargo_remove::RemoveOptions;
use cargo::ops::resolve_ws;
use cargo::util::command_prelude::*;
use cargo::util::print_available_packages;
use cargo::util::toml_mut::dependency::Dependency;
use cargo::util::toml_mut::dependency::MaybeWorkspace;
use cargo::util::toml_mut::dependency::Source;
use cargo::util::toml_mut::manifest::DepTable;
use cargo::util::toml_mut::manifest::LocalManifest;
use cargo::CargoResult;

pub fn cli() -> clap::Command {
    clap::Command::new("remove")
        // Subcommand aliases are handled in `aliased_command()`.
        // .alias("rm")
        .about("Remove dependencies from a Cargo.toml manifest file")
        .args([clap::Arg::new("dependencies")
            .action(clap::ArgAction::Append)
            .required(true)
            .num_args(1..)
            .value_name("DEP_ID")
            .help("Dependencies to be removed")])
        .arg_dry_run("Don't actually write the manifest")
        .arg_quiet()
        .next_help_heading("Section")
        .args([
            clap::Arg::new("dev")
                .long("dev")
                .conflicts_with("build")
                .action(clap::ArgAction::SetTrue)
                .group("section")
                .help("Remove from dev-dependencies"),
            clap::Arg::new("build")
                .long("build")
                .conflicts_with("dev")
                .action(clap::ArgAction::SetTrue)
                .group("section")
                .help("Remove from build-dependencies"),
            clap::Arg::new("target")
                .long("target")
                .num_args(1)
                .value_name("TARGET")
                .value_parser(clap::builder::NonEmptyStringValueParser::new())
                .help("Remove from target-dependencies"),
        ])
        .arg_package("Package to remove from")
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help remove</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let dry_run = args.dry_run();

    let workspace = args.workspace(config)?;

    if args.is_present_with_zero_values("package") {
        print_available_packages(&workspace)?;
    }

    let packages = args.packages_from_flags()?;
    let packages = packages.get_packages(&workspace)?;
    let spec = match packages.len() {
        0 => {
            return Err(CliError::new(
                anyhow::format_err!(
                    "no packages selected to modify.  Please specify one with `-p <PKGID>`"
                ),
                101,
            ));
        }
        1 => packages[0],
        _ => {
            let names = packages.iter().map(|p| p.name()).collect::<Vec<_>>();
            return Err(CliError::new(
                anyhow::format_err!(
                    "`cargo remove` could not determine which package to modify. \
                    Use the `--package` option to specify a package. \n\
                    available packages: {}",
                    names.join(", ")
                ),
                101,
            ));
        }
    };

    let dependencies = args
        .get_many::<String>("dependencies")
        .expect("required(true)")
        .cloned()
        .collect::<Vec<_>>();

    let section = parse_section(args);

    let options = RemoveOptions {
        config,
        spec,
        dependencies,
        section,
        dry_run,
    };
    remove(&options)?;

    if !dry_run {
        // Clean up the workspace
        gc_workspace(&workspace)?;

        // Reload the workspace since we've changed dependencies
        let ws = args.workspace(config)?;
        let resolve = {
            // HACK: Avoid unused patch warnings by temporarily changing the verbosity.
            // In rare cases, this might cause index update messages to not show up
            let verbosity = ws.config().shell().verbosity();
            ws.config()
                .shell()
                .set_verbosity(cargo::core::Verbosity::Quiet);
            let resolve = resolve_ws(&ws);
            ws.config().shell().set_verbosity(verbosity);
            resolve?.1
        };

        // Attempt to gc unused patches and re-resolve if anything is removed
        if gc_unused_patches(&workspace, &resolve)? {
            let ws = args.workspace(config)?;
            resolve_ws(&ws)?;
        }
    }
    Ok(())
}

fn parse_section(args: &ArgMatches) -> DepTable {
    let dev = args.flag("dev");
    let build = args.flag("build");

    let kind = if dev {
        DepKind::Development
    } else if build {
        DepKind::Build
    } else {
        DepKind::Normal
    };

    let mut table = DepTable::new().set_kind(kind);

    if let Some(target) = args.get_one::<String>("target") {
        assert!(!target.is_empty(), "Target specification may not be empty");
        table = table.set_target(target);
    }

    table
}

/// Clean up the workspace.dependencies, profile, patch, and replace sections of the root manifest
/// by removing dependencies which no longer have a reference to them.
fn gc_workspace(workspace: &Workspace<'_>) -> CargoResult<()> {
    let mut manifest: toml_edit::Document =
        cargo_util::paths::read(workspace.root_manifest())?.parse()?;
    let mut is_modified = true;

    let members = workspace
        .members()
        .map(|p| LocalManifest::try_new(p.manifest_path()))
        .collect::<CargoResult<Vec<_>>>()?;

    let mut dependencies = members
        .iter()
        .flat_map(|manifest| {
            manifest.get_sections().into_iter().flat_map(|(_, table)| {
                table
                    .as_table_like()
                    .unwrap()
                    .iter()
                    .map(|(key, item)| Dependency::from_toml(&manifest.path, key, item))
                    .collect::<Vec<_>>()
            })
        })
        .collect::<CargoResult<Vec<_>>>()?;

    // Clean up the workspace.dependencies section and replace instances of
    // workspace dependencies with their definitions
    if let Some(toml_edit::Item::Table(deps_table)) = manifest
        .get_mut("workspace")
        .and_then(|t| t.get_mut("dependencies"))
    {
        deps_table.set_implicit(true);
        for (key, item) in deps_table.iter_mut() {
            let ws_dep = Dependency::from_toml(&workspace.root(), key.get(), item)?;

            // search for uses of this workspace dependency
            let mut is_used = false;
            for dep in dependencies.iter_mut().filter(|d| {
                d.toml_key() == key.get() && matches!(d.source(), Some(Source::Workspace(_)))
            }) {
                // HACK: Replace workspace references in `dependencies` to simplify later GC steps:
                // 1. Avoid having to look it up again to determine the dependency source / spec
                // 2. The entry might get deleted, preventing us from looking it up again
                //
                // This does lose extra information, like features enabled, but that shouldn't be a
                // problem for GC
                *dep = ws_dep.clone();

                is_used = true;
            }

            if !is_used {
                *item = toml_edit::Item::None;
                is_modified = true;
            }
        }
    }

    // Clean up the profile section
    //
    // Example tables:
    // - profile.dev.package.foo
    // - profile.release.package."foo:2.1.0"
    if let Some(toml_edit::Item::Table(profile_section_table)) = manifest.get_mut("profile") {
        profile_section_table.set_implicit(true);

        for (_, item) in profile_section_table.iter_mut() {
            if let toml_edit::Item::Table(profile_table) = item {
                profile_table.set_implicit(true);

                if let Some(toml_edit::Item::Table(package_table)) =
                    profile_table.get_mut("package")
                {
                    package_table.set_implicit(true);

                    for (key, item) in package_table.iter_mut() {
                        let key = key.get();
                        // Skip globs. Can't do anything with them.
                        // For example, profile.release.package."*".
                        if crate::util::restricted_names::is_glob_pattern(key) {
                            continue;
                        }
                        if !spec_has_match(
                            &PackageIdSpec::parse(key)?,
                            &dependencies,
                            workspace.config(),
                        )? {
                            *item = toml_edit::Item::None;
                            is_modified = true;
                        }
                    }
                }
            }
        }
    }

    // Clean up the replace section
    if let Some(toml_edit::Item::Table(table)) = manifest.get_mut("replace") {
        table.set_implicit(true);

        for (key, item) in table.iter_mut() {
            if !spec_has_match(
                &PackageIdSpec::parse(key.get())?,
                &dependencies,
                workspace.config(),
            )? {
                *item = toml_edit::Item::None;
                is_modified = true;
            }
        }
    }

    if is_modified {
        cargo_util::paths::write_atomic(
            workspace.root_manifest(),
            manifest.to_string().as_bytes(),
        )?;
    }

    Ok(())
}

/// Check whether or not a package ID spec matches any non-workspace dependencies.
fn spec_has_match(
    spec: &PackageIdSpec,
    dependencies: &[Dependency],
    config: &Config,
) -> CargoResult<bool> {
    for dep in dependencies {
        if spec.name().as_str() != &dep.name {
            continue;
        }

        let version_matches = match (spec.version(), dep.version()) {
            (Some(v), Some(vq)) => semver::VersionReq::parse(vq)?.matches(&v),
            (Some(_), None) => false,
            (None, None | Some(_)) => true,
        };
        if !version_matches {
            continue;
        }

        match dep.source_id(config)? {
            MaybeWorkspace::Other(source_id) => {
                if spec.url().map(|u| u == source_id.url()).unwrap_or(true) {
                    return Ok(true);
                }
            }
            MaybeWorkspace::Workspace(_) => {}
        }
    }

    Ok(false)
}

/// Removes unused patches from the manifest
fn gc_unused_patches(workspace: &Workspace<'_>, resolve: &Resolve) -> CargoResult<bool> {
    let mut manifest: toml_edit::Document =
        cargo_util::paths::read(workspace.root_manifest())?.parse()?;
    let mut modified = false;

    // Clean up the patch section
    if let Some(toml_edit::Item::Table(patch_section_table)) = manifest.get_mut("patch") {
        patch_section_table.set_implicit(true);

        for (_, item) in patch_section_table.iter_mut() {
            if let toml_edit::Item::Table(patch_table) = item {
                patch_table.set_implicit(true);

                for (key, item) in patch_table.iter_mut() {
                    let dep = Dependency::from_toml(&workspace.root_manifest(), key.get(), item)?;

                    // Generate a PackageIdSpec url for querying
                    let url = if let MaybeWorkspace::Other(source_id) =
                        dep.source_id(workspace.config())?
                    {
                        format!("{}#{}", source_id.url(), dep.name)
                    } else {
                        continue;
                    };

                    if PackageIdSpec::query_str(&url, resolve.unused_patches().iter().cloned())
                        .is_ok()
                    {
                        *item = toml_edit::Item::None;
                        modified = true;
                    }
                }
            }
        }
    }

    if modified {
        cargo_util::paths::write(workspace.root_manifest(), manifest.to_string().as_bytes())?;
    }

    Ok(modified)
}
