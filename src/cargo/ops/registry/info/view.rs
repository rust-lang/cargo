use std::collections::HashMap;
use std::io::Write;

use crate::core::Shell;
use crate::util::style::{CONTEXT, ERROR, HEADER, LITERAL, NOP, WARN};
use crate::{
    CargoResult, GlobalContext,
    core::{
        Dependency, FeatureMap, Package, PackageId, SourceId, dependency::DepKind, shell::Verbosity,
    },
    sources::IndexSummary,
    util::interning::InternedString,
};

// Pretty print the package information.
pub(super) fn pretty_view(
    package: &Package,
    summaries: &[IndexSummary],
    suggest_cargo_tree_command: bool,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let summary = package.manifest().summary();
    let package_id = summary.package_id();
    let metadata = package.manifest().metadata();
    let is_package_from_crates_io = summary.source_id().is_crates_io();
    let header = HEADER;
    let error = ERROR;
    let warn = WARN;
    let context = CONTEXT;

    let mut shell = gctx.shell();
    let verbosity = shell.verbosity();
    write!(shell.out(), "{header}{}{header:#}", package_id.name())?;
    if !metadata.keywords.is_empty() {
        let message = if is_package_from_crates_io {
            metadata
                .keywords
                .iter()
                .map(|keyword| {
                    let link = shell.out_hyperlink(format!("https://crates.io/keywords/{keyword}"));
                    format!("{link}#{keyword}{link:#}")
                })
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            format!("#{}", metadata.keywords.join(" #"))
        };
        write!(shell.out(), " {context}{message}{context:#}")?;
    }

    let stdout = shell.out();
    writeln!(stdout)?;
    if let Some(ref description) = metadata.description {
        writeln!(stdout, "{}", description.trim_end())?;
    }
    write!(
        stdout,
        "{header}version:{header:#} {}",
        package_id.version()
    )?;
    // Add a warning message to stdout if the following conditions are met:
    // 1. The package version is not the latest available version.
    // 2. The package source is not crates.io.
    match (
        summaries.iter().max_by_key(|s| s.as_summary().version()),
        is_package_from_crates_io,
    ) {
        (Some(latest), false) if latest.as_summary().version() != package_id.version() => {
            write!(
                stdout,
                " {warn}(latest {} {warn:#}{context}from {}{context:#}{warn}){warn:#}",
                latest.as_summary().version(),
                pretty_source(summary.source_id(), gctx)
            )?;
        }
        (Some(latest), true) if latest.as_summary().version() != package_id.version() => {
            write!(
                stdout,
                " {warn}(latest {}){warn:#}",
                latest.as_summary().version(),
            )?;
        }
        (_, false) => {
            write!(
                stdout,
                " {context}(from {}){context:#}",
                pretty_source(summary.source_id(), gctx)
            )?;
        }
        (_, true) => {}
    }
    writeln!(stdout)?;
    writeln!(
        stdout,
        "{header}license:{header:#} {}",
        metadata
            .license
            .clone()
            .unwrap_or_else(|| format!("{error}unknown{error:#}"))
    )?;
    // TODO: color MSRV as a warning if newer than either the "workspace" MSRV or `rustc --version`
    writeln!(
        stdout,
        "{header}rust-version:{header:#} {}",
        metadata
            .rust_version
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| format!("{warn}unknown{warn:#}"))
    )?;
    if let Some(ref link) = metadata.documentation.clone().or_else(|| {
        is_package_from_crates_io.then(|| {
            format!(
                "https://docs.rs/{name}/{version}",
                name = package_id.name(),
                version = package_id.version()
            )
        })
    }) {
        writeln!(stdout, "{header}documentation:{header:#} {link}")?;
    }
    if let Some(ref link) = metadata.homepage {
        writeln!(stdout, "{header}homepage:{header:#} {link}")?;
    }
    if let Some(ref link) = metadata.repository {
        writeln!(stdout, "{header}repository:{header:#} {link}")?;
    }
    // Only print the crates.io link if the package is from crates.io.
    if is_package_from_crates_io {
        writeln!(
            stdout,
            "{header}crates.io:{header:#} https://crates.io/crates/{}/{}",
            package_id.name(),
            package_id.version()
        )?;
    }

    let activated = &["default".into()];
    let resolved_features = resolve_features(activated, summary.features());
    pretty_features(
        resolved_features.clone(),
        summary.features(),
        verbosity,
        stdout,
    )?;

    pretty_deps(
        package,
        &resolved_features,
        summary.features(),
        verbosity,
        stdout,
        gctx,
    )?;

    if suggest_cargo_tree_command {
        suggest_cargo_tree(package_id, &mut shell)?;
    }

    Ok(())
}

fn pretty_source(source: SourceId, ctx: &GlobalContext) -> String {
    if let Some(relpath) = source
        .local_path()
        .and_then(|path| pathdiff::diff_paths(path, ctx.cwd()))
    {
        let path = std::path::Path::new(".").join(relpath);
        path.display().to_string()
    } else {
        source.to_string()
    }
}

fn pretty_deps(
    package: &Package,
    resolved_features: &[(InternedString, FeatureStatus)],
    features: &FeatureMap,
    verbosity: Verbosity,
    stdout: &mut dyn Write,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    match verbosity {
        Verbosity::Quiet | Verbosity::Normal => {
            return Ok(());
        }
        Verbosity::Verbose => {}
    }

    let header = HEADER;

    let dependencies = package
        .dependencies()
        .iter()
        .filter(|d| d.kind() == DepKind::Normal)
        .collect::<Vec<_>>();
    if !dependencies.is_empty() {
        writeln!(stdout, "{header}dependencies:{header:#}")?;
        print_deps(dependencies, resolved_features, features, stdout, gctx)?;
    }

    let build_dependencies = package
        .dependencies()
        .iter()
        .filter(|d| d.kind() == DepKind::Build)
        .collect::<Vec<_>>();
    if !build_dependencies.is_empty() {
        writeln!(stdout, "{header}build-dependencies:{header:#}")?;
        print_deps(
            build_dependencies,
            resolved_features,
            features,
            stdout,
            gctx,
        )?;
    }

    Ok(())
}

fn print_deps(
    dependencies: Vec<&Dependency>,
    resolved_features: &[(InternedString, FeatureStatus)],
    features: &FeatureMap,
    stdout: &mut dyn Write,
    gctx: &GlobalContext,
) -> Result<(), anyhow::Error> {
    let enabled_by_user = HEADER;
    let enabled = NOP;
    let disabled = anstyle::Style::new() | anstyle::Effects::DIMMED;

    let mut dependencies = dependencies
        .into_iter()
        .map(|dependency| {
            let status = if !dependency.is_optional() {
                FeatureStatus::EnabledByUser
            } else if resolved_features
                .iter()
                .filter(|(_, s)| !s.is_disabled())
                .filter_map(|(n, _)| features.get(n))
                .flatten()
                .filter_map(|f| match f {
                    crate::core::FeatureValue::Feature(_) => None,
                    crate::core::FeatureValue::Dep { dep_name } => Some(dep_name),
                    crate::core::FeatureValue::DepFeature { dep_name, weak, .. } if *weak => {
                        Some(dep_name)
                    }
                    crate::core::FeatureValue::DepFeature { .. } => None,
                })
                .any(|dep_name| *dep_name == dependency.name_in_toml())
            {
                FeatureStatus::Enabled
            } else {
                FeatureStatus::Disabled
            };
            (dependency, status)
        })
        .collect::<Vec<_>>();
    dependencies.sort_by_key(|(d, s)| (*s, d.package_name()));
    for (dependency, status) in dependencies {
        // 1. Only print the version requirement if it is a registry dependency.
        // 2. Only print the source if it is not a registry dependency.
        // For example: `bar (./crates/bar)` or `bar@=1.2.3`.
        let (req, source) = if dependency.source_id().is_registry() {
            (
                format!("@{}", pretty_req(dependency.version_req())),
                String::new(),
            )
        } else {
            (
                String::new(),
                format!(" ({})", pretty_source(dependency.source_id(), gctx)),
            )
        };

        if status == FeatureStatus::EnabledByUser {
            write!(stdout, " {enabled_by_user}+{enabled_by_user:#}")?;
        } else {
            write!(stdout, "  ")?;
        }
        let style = match status {
            FeatureStatus::EnabledByUser | FeatureStatus::Enabled => enabled,
            FeatureStatus::Disabled => disabled,
        };
        writeln!(
            stdout,
            "{style}{}{}{}{style:#}",
            dependency.package_name(),
            req,
            source
        )?;
    }
    Ok(())
}

fn pretty_req(req: &crate::util::OptVersionReq) -> String {
    let mut rendered = req.to_string();
    let strip_prefix = match req {
        crate::util::OptVersionReq::Any => false,
        crate::util::OptVersionReq::Req(req)
        | crate::util::OptVersionReq::Locked(_, req)
        | crate::util::OptVersionReq::Precise(_, req) => {
            req.comparators.len() == 1 && rendered.starts_with('^')
        }
    };
    if strip_prefix {
        rendered.remove(0);
        rendered
    } else {
        rendered
    }
}

fn pretty_features(
    resolved_features: Vec<(InternedString, FeatureStatus)>,
    features: &FeatureMap,
    verbosity: Verbosity,
    stdout: &mut dyn Write,
) -> CargoResult<()> {
    let header = HEADER;
    let enabled_by_user = HEADER;
    let enabled = NOP;
    let disabled = anstyle::Style::new() | anstyle::Effects::DIMMED;
    let summary = anstyle::Style::new() | anstyle::Effects::ITALIC;

    // If there are no features, return early.
    let margin = features
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or_default();
    if margin == 0 {
        return Ok(());
    }

    writeln!(stdout, "{header}features:{header:#}")?;

    const MAX_FEATURE_PRINTS: usize = 30;
    let total_activated = resolved_features
        .iter()
        .filter(|(_, s)| !s.is_disabled())
        .count();
    let total_deactivated = resolved_features
        .iter()
        .filter(|(_, s)| s.is_disabled())
        .count();
    let show_all = match verbosity {
        Verbosity::Quiet | Verbosity::Normal => false,
        Verbosity::Verbose => true,
    };
    let show_activated = total_activated <= MAX_FEATURE_PRINTS || show_all;
    let show_deactivated = (total_activated + total_deactivated) <= MAX_FEATURE_PRINTS || show_all;
    for (current, status, current_activated) in resolved_features
        .iter()
        .map(|(n, s)| (n, s, features.get(n).unwrap()))
    {
        if !status.is_disabled() && !show_activated {
            continue;
        }
        if status.is_disabled() && !show_deactivated {
            continue;
        }
        if *status == FeatureStatus::EnabledByUser {
            write!(stdout, " {enabled_by_user}+{enabled_by_user:#}")?;
        } else {
            write!(stdout, "  ")?;
        }
        let style = match status {
            FeatureStatus::EnabledByUser | FeatureStatus::Enabled => enabled,
            FeatureStatus::Disabled => disabled,
        };
        writeln!(
            stdout,
            "{style}{current: <margin$}{style:#} = [{features}]",
            features = current_activated
                .iter()
                .map(|s| format!("{style}{s}{style:#}"))
                .collect::<Vec<String>>()
                .join(", ")
        )?;
    }
    if !show_activated {
        writeln!(
            stdout,
            "  {summary}{total_activated} activated features{summary:#}",
        )?;
    }
    if !show_deactivated {
        writeln!(
            stdout,
            "  {summary}{total_deactivated} deactivated features{summary:#}",
        )?;
    }

    Ok(())
}

// Suggest the cargo tree command to view the dependency tree.
fn suggest_cargo_tree(package_id: PackageId, shell: &mut Shell) -> CargoResult<()> {
    let literal = LITERAL;

    shell.note(format_args!(
        "to see how you depend on {name}, run `{literal}cargo tree --invert {name}@{version}{literal:#}`",
        name = package_id.name(),
        version = package_id.version(),
    ))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FeatureStatus {
    EnabledByUser,
    Enabled,
    Disabled,
}

impl FeatureStatus {
    fn is_disabled(&self) -> bool {
        *self == FeatureStatus::Disabled
    }
}

fn resolve_features(
    explicit: &[InternedString],
    features: &FeatureMap,
) -> Vec<(InternedString, FeatureStatus)> {
    let mut resolved = features
        .keys()
        .cloned()
        .map(|n| {
            if explicit.contains(&n) {
                (n, FeatureStatus::EnabledByUser)
            } else {
                (n, FeatureStatus::Disabled)
            }
        })
        .collect::<HashMap<_, _>>();

    let mut activated_queue = explicit.to_vec();

    while let Some(current) = activated_queue.pop() {
        let Some(current_activated) = features.get(&current) else {
            // `default` isn't always present
            continue;
        };
        for activated in current_activated.iter().rev().filter_map(|f| match f {
            crate::core::FeatureValue::Feature(name) => Some(name),
            crate::core::FeatureValue::Dep { .. }
            | crate::core::FeatureValue::DepFeature { .. } => None,
        }) {
            let Some(status) = resolved.get_mut(activated) else {
                continue;
            };
            if status.is_disabled() {
                *status = FeatureStatus::Enabled;
                activated_queue.push(*activated);
            }
        }
    }

    let mut resolved: Vec<_> = resolved.into_iter().collect();
    resolved.sort_by_key(|(name, status)| (*status, *name));
    resolved
}
