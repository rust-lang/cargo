//! Resolve workspace dependencies using the selected Cargo tree options.

use super::*;

#[derive(Debug, Clone, Copy)]
struct EdgeKinds {
    normal: bool,
    build: bool,
    dev: bool,
    public: bool,
    no_proc_macro: bool,
    features: bool,
}

impl EdgeKinds {
    fn from_options(options: &ResolutionOptions) -> Result<Self> {
        let mut kinds: Vec<&str> = options.edges.iter().map(String::as_str).collect();
        if options.no_dev_dependencies {
            kinds.push("no-dev");
        }

        let public = kinds.contains(&"public");
        let no_proc_macro = kinds.contains(&"no-proc-macro");
        kinds.retain(|kind| !matches!(*kind, "public" | "no-proc-macro"));

        let has_all = kinds.contains(&"all");
        let features = kinds.contains(&"features") || has_all;
        kinds.retain(|kind| !matches!(*kind, "features" | "all"));
        if has_all {
            kinds.extend(["normal", "build", "dev"]);
        }

        if kinds.is_empty() {
            return Ok(Self {
                public,
                no_proc_macro,
                features,
                ..Self::all()
            });
        }

        let has_negative = kinds.iter().any(|kind| kind.starts_with("no-"));
        if has_negative {
            if kinds
                .iter()
                .any(|kind| matches!(*kind, "all" | "normal" | "build" | "dev"))
            {
                bail!("dependency kinds cannot be mixed with `no-*` dependency kinds");
            }

            let mut selected = Self::all();
            for kind in kinds {
                match kind {
                    "no-normal" => selected.normal = false,
                    "no-build" => selected.build = false,
                    "no-dev" => selected.dev = false,
                    _ => bail!("invalid dependency kind `{kind}`"),
                }
            }
            return Ok(Self {
                public,
                no_proc_macro,
                features,
                ..selected
            });
        }

        let mut selected = Self {
            normal: false,
            build: false,
            dev: false,
            public,
            no_proc_macro,
            features,
        };
        for kind in kinds {
            match kind {
                "normal" => selected.normal = true,
                "build" => selected.build = true,
                "dev" => selected.dev = true,
                _ => bail!("invalid dependency kind `{kind}`"),
            }
        }
        Ok(selected)
    }

    fn all() -> Self {
        Self {
            normal: true,
            build: true,
            dev: true,
            public: false,
            no_proc_macro: false,
            features: false,
        }
    }

    fn includes(self, kind: DepKind) -> bool {
        match kind {
            DepKind::Normal => self.normal,
            DepKind::Build => self.build,
            DepKind::Development => self.dev,
        }
    }
}

/// Resolved Cargo workspace with the data required to build the dependency tree.
pub(super) struct ResolvedWorkspace {
    pub(super) workspace_name: String,
    pub(super) packages: FxHashMap<PackageId, PackageSnapshot>,
    pub(super) workspace_members: FxHashSet<PackageId>,
    /// Deduplicated, classified outgoing edges keyed by source package.
    pub(super) edges: FxHashMap<PackageId, Vec<(PackageId, DepKind)>>,
    /// Incoming manifest declarations keyed by the package they resolve to.
    pub(super) usages: FxHashMap<PackageId, Vec<DependencyUsage>>,
    /// Dependency declarations in manifest direction, used to expand feature definitions.
    pub(super) feature_dependencies: FxHashMap<PackageId, Vec<FeatureDependency>>,
    /// Feature-bearing dependencies in display direction (reversed for `--invert`).
    pub(super) graph_feature_dependencies: FxHashMap<PackageId, Vec<FeatureDependency>>,
    pub(super) feature_map: FxHashMap<PackageId, FxHashMap<String, Vec<FeatureValue>>>,
    pub(super) cli_features: FxHashMap<PackageId, CliFeatures>,
    pub(super) show_features: bool,
    pub(super) inverted: bool,
    pub(super) root_ids: Vec<PackageId>,
}

#[derive(Debug, Clone)]
pub(super) struct FeatureDependency {
    pub(super) package_id: PackageId,
    pub(super) kind: DepKind,
    pub(super) name: String,
    pub(super) optional: bool,
    pub(super) uses_default_features: bool,
    pub(super) features: Vec<String>,
}

impl ResolvedWorkspace {
    /// Resolve a Cargo workspace into the minimal data needed to build the
    /// deduplicated dependency tree.
    ///
    /// This loads the workspace through Cargo's own resolver, snapshots each
    /// reachable package into a compact [`PackageSnapshot`], classifies outgoing
    /// edges by dependency kind, and records the workspace member ids that act
    /// as graph roots.
    pub(super) fn load(
        gctx: &GlobalContext,
        manifest_path: Option<PathBuf>,
        options: &ResolutionOptions,
        format_requirements: PackageFormatRequirements,
    ) -> Result<Self> {
        let manifest_path = resolve_manifest_path(gctx, manifest_path)?;
        let ws = Workspace::new(&manifest_path, gctx).context("failed to load Cargo workspace")?;
        let workspace_members = ws.members().map(|package| package.package_id()).collect();

        let all_targets =
            options.all_targets || (options.targets.len() == 1 && options.targets[0] == "all");
        let requested_targets = if all_targets {
            &[]
        } else {
            options.targets.as_slice()
        };
        let requested_kinds = CompileKind::from_requested_targets(ws.gctx(), requested_targets)
            .context("failed to determine Cargo target kinds")?;

        let mut target_data =
            RustcTargetData::new(&ws, &requested_kinds).context("failed to load target data")?;
        let packages = ops::Packages::from_flags(
            options.workspace,
            options.exclude.clone(),
            options.packages.clone(),
        )
        .context("failed to select Cargo packages")?;
        let specs = packages
            .to_package_id_specs(&ws)
            .context("failed to resolve selected package specs")?;
        let edge_kinds = EdgeKinds::from_options(options)?;
        if edge_kinds.features && options.duplicates {
            bail!("the `-e features` flag does not support `--duplicates`");
        }
        let prune_specs = options
            .prune
            .iter()
            .map(|spec| {
                PackageIdSpec::parse(spec)
                    .with_context(|| format!("invalid package spec to prune: `{spec}`"))
            })
            .collect::<Result<Vec<_>>>()?;
        let cli_features = CliFeatures::from_command_line(
            &options.features,
            options.all_features,
            !options.no_default_features,
        )
        .context("failed to parse Cargo features")?;
        let cli_features_by_package = if edge_kinds.features {
            ws.members_with_features(&specs, &cli_features)?
                .into_iter()
                .map(|(package, features)| (package.package_id(), features))
                .collect()
        } else {
            FxHashMap::default()
        };
        let ws_resolve = ops::resolve_ws_with_opts(
            &ws,
            &mut target_data,
            &requested_kinds,
            &cli_features,
            &specs,
            if edge_kinds.dev {
                HasDevUnits::Yes
            } else {
                HasDevUnits::No
            },
            if all_targets {
                ForceAllTargets::Yes
            } else {
                ForceAllTargets::No
            },
            false,
        )
        .context("failed to resolve Cargo dependencies")?;

        let active_optional_dependencies: FxHashSet<_> = ws_resolve
            .specs_and_features
            .iter()
            .flat_map(|spec| spec.resolved_features.activated_dependencies.iter())
            .flat_map(|((package_id, _), dependencies)| {
                dependencies
                    .iter()
                    .map(move |dependency| (*package_id, *dependency))
            })
            .collect();

        let pkg_set = ws_resolve.pkg_set;
        let resolve = ws_resolve.targeted_resolve;

        let workspace_name = ws
            .current_opt()
            .map(|pkg| pkg.name().as_str().to_owned())
            .unwrap_or_else(|| "workspace".to_owned());

        // Snapshot every reachable package: workspace members first (so a
        // member that also appears in pkg_set keeps its workspace identity),
        // then everything else from the resolved package set.
        let mut packages: FxHashMap<PackageId, PackageSnapshot> = FxHashMap::default();
        let mut feature_map = FxHashMap::default();
        for pkg in ws.members() {
            let package_id = pkg.package_id();
            if edge_kinds.features {
                feature_map.insert(
                    package_id,
                    pkg.summary()
                        .features()
                        .iter()
                        .map(|(name, values)| (name.to_string(), values.clone()))
                        .collect(),
                );
            }
            let features = format_requirements
                .features
                .then(|| {
                    resolve
                        .features(package_id)
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .filter(|features| !features.is_empty());
            packages.insert(
                package_id,
                PackageSnapshot::from_package(pkg, features, format_requirements),
            );
        }
        for pkg in pkg_set.packages() {
            let package_id = pkg.package_id();
            if edge_kinds.features {
                feature_map.entry(package_id).or_insert_with(|| {
                    pkg.summary()
                        .features()
                        .iter()
                        .map(|(name, values)| (name.to_string(), values.clone()))
                        .collect()
                });
            }
            packages.entry(package_id).or_insert_with(|| {
                let features = format_requirements
                    .features
                    .then(|| {
                        resolve
                            .features(package_id)
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .filter(|features| !features.is_empty());
                PackageSnapshot::from_package(pkg, features, format_requirements)
            });
        }

        // Build classified, kind-deduplicated edges keyed by source package.
        let mut edges: FxHashMap<PackageId, Vec<(PackageId, DepKind)>> = FxHashMap::default();
        let mut usages: FxHashMap<PackageId, Vec<DependencyUsage>> = FxHashMap::default();
        let mut feature_dependencies: FxHashMap<PackageId, Vec<FeatureDependency>> =
            FxHashMap::default();
        for &pkg_id in packages.keys() {
            let mut classified: Vec<(PackageId, DepKind)> = Vec::new();
            let mut package_feature_dependencies = Vec::new();
            for (dep_id, deps) in resolve.deps(pkg_id) {
                let mut seen_normal = false;
                let mut seen_dev = false;
                let mut seen_build = false;
                for dep in deps.iter() {
                    let kind = dep.kind();
                    if dep.is_optional()
                        && !active_optional_dependencies.contains(&(pkg_id, dep.name_in_toml()))
                    {
                        continue;
                    }
                    if !edge_kinds.includes(kind)
                        || (edge_kinds.public && !dep.is_public())
                        || (edge_kinds.no_proc_macro
                            && packages
                                .get(&dep_id)
                                .is_some_and(|package| package.is_proc_macro))
                        || prune_specs.iter().any(|spec| spec.matches(dep_id))
                    {
                        continue;
                    }
                    if format_requirements.metadata && packages.contains_key(&pkg_id) {
                        usages.entry(dep_id).or_default().push(DependencyUsage {
                            parent: pkg_id,
                            kind,
                            dependency: dep.clone(),
                        });
                    }
                    if edge_kinds.features {
                        let feature_dependency = FeatureDependency {
                            package_id: dep_id,
                            kind,
                            name: dep.name_in_toml().to_string(),
                            optional: dep.is_optional(),
                            uses_default_features: dep.uses_default_features(),
                            features: dep.features().iter().map(ToString::to_string).collect(),
                        };
                        if !package_feature_dependencies.iter().any(
                            |existing: &FeatureDependency| {
                                existing.package_id == feature_dependency.package_id
                                    && existing.kind == feature_dependency.kind
                                    && existing.name == feature_dependency.name
                                    && existing.uses_default_features
                                        == feature_dependency.uses_default_features
                                    && existing.features == feature_dependency.features
                            },
                        ) {
                            package_feature_dependencies.push(feature_dependency);
                        }
                    }
                    let already = match kind {
                        DepKind::Normal => std::mem::replace(&mut seen_normal, true),
                        DepKind::Development => std::mem::replace(&mut seen_dev, true),
                        DepKind::Build => std::mem::replace(&mut seen_build, true),
                    };
                    if !already {
                        classified.push((dep_id, kind));
                    }
                }
            }
            edges.insert(pkg_id, classified);
            if edge_kinds.features {
                feature_dependencies.insert(pkg_id, package_feature_dependencies);
            }
        }

        let selected_roots: Vec<PackageId> = ws
            .members()
            .filter_map(|pkg| {
                let package_id = pkg.package_id();
                specs
                    .iter()
                    .any(|spec| spec.matches(package_id))
                    .then_some(package_id)
            })
            .collect();

        let selected_reachable = reachable_packages(&selected_roots, &edges);
        let invert_specs = invert_package_specs(options)?;
        let invert_roots = if invert_specs.is_empty() {
            Vec::new()
        } else {
            packages_matching_specs(&invert_specs, &selected_reachable)?
        };

        let (tree_roots, graph_roots, inverted) = if options.duplicates {
            let graph_roots = if invert_roots.is_empty() {
                selected_roots.clone()
            } else {
                invert_roots
            };
            let reachable = reachable_packages(&graph_roots, &edges);
            let mut duplicates = duplicate_packages(&reachable);
            if duplicates.is_empty() {
                bail!("no duplicate packages found in the selected dependency tree");
            }
            duplicates.sort_unstable();
            (duplicates, graph_roots, true)
        } else if options.invert.is_some() {
            if invert_roots.is_empty() {
                bail!("`--invert` requires a package spec or `--package`");
            }
            (invert_roots, selected_roots.clone(), true)
        } else {
            (selected_roots.clone(), selected_roots, false)
        };

        let graph_scope = reachable_packages(&graph_roots, &edges);
        edges.retain(|package_id, dependencies| {
            if !graph_scope.contains(package_id) {
                return false;
            }
            dependencies.retain(|(dependency_id, _)| graph_scope.contains(dependency_id));
            true
        });
        usages.retain(|package_id, dependencies| {
            if !graph_scope.contains(package_id) {
                return false;
            }
            dependencies.retain(|usage| graph_scope.contains(&usage.parent));
            true
        });

        if inverted {
            edges = invert_edges(&edges);
        }

        feature_dependencies.retain(|package_id, dependencies| {
            if !graph_scope.contains(package_id) {
                return false;
            }
            dependencies.retain(|dependency| graph_scope.contains(&dependency.package_id));
            true
        });
        let graph_feature_dependencies = if inverted {
            invert_feature_dependencies(&feature_dependencies)
        } else {
            feature_dependencies.clone()
        };

        Ok(ResolvedWorkspace {
            workspace_name,
            packages,
            workspace_members,
            edges,
            usages,
            feature_dependencies,
            graph_feature_dependencies,
            feature_map,
            cli_features: cli_features_by_package,
            show_features: edge_kinds.features,
            inverted,
            root_ids: tree_roots,
        })
    }
}

fn invert_package_specs(options: &ResolutionOptions) -> Result<Vec<String>> {
    let Some(specs) = &options.invert else {
        return Ok(Vec::new());
    };
    if !specs.is_empty() {
        return Ok(specs.clone());
    }
    if options.packages.is_empty() {
        bail!("`--invert` requires a package spec or `--package`");
    }
    Ok(options.packages.clone())
}

fn packages_matching_specs(
    spec_values: &[String],
    candidates: &FxHashSet<PackageId>,
) -> Result<Vec<PackageId>> {
    let mut matched = FxHashSet::default();
    for value in spec_values {
        let spec = PackageIdSpec::parse(value)
            .with_context(|| format!("invalid package spec to invert: `{value}`"))?;
        let mut found = false;
        for &package_id in candidates {
            if spec.matches(package_id) {
                matched.insert(package_id);
                found = true;
            }
        }
        if !found {
            bail!("package spec `{value}` is not in the selected dependency tree");
        }
    }
    let mut matched: Vec<_> = matched.into_iter().collect();
    matched.sort_unstable();
    Ok(matched)
}

fn reachable_packages(
    roots: &[PackageId],
    edges: &FxHashMap<PackageId, Vec<(PackageId, DepKind)>>,
) -> FxHashSet<PackageId> {
    let mut reachable = FxHashSet::default();
    let mut pending = roots.to_vec();
    while let Some(package_id) = pending.pop() {
        if !reachable.insert(package_id) {
            continue;
        }
        if let Some(dependencies) = edges.get(&package_id) {
            pending.extend(dependencies.iter().map(|(dependency_id, _)| *dependency_id));
        }
    }
    reachable
}

fn duplicate_packages(reachable: &FxHashSet<PackageId>) -> Vec<PackageId> {
    let mut packages_by_name: FxHashMap<String, Vec<PackageId>> = FxHashMap::default();
    for &package_id in reachable {
        packages_by_name
            .entry(package_id.name().as_str().to_owned())
            .or_default()
            .push(package_id);
    }
    packages_by_name
        .into_values()
        .filter(|packages| {
            let first_version = packages[0].version();
            packages
                .iter()
                .any(|package_id| package_id.version() != first_version)
        })
        .flatten()
        .collect()
}

fn invert_edges(
    edges: &FxHashMap<PackageId, Vec<(PackageId, DepKind)>>,
) -> FxHashMap<PackageId, Vec<(PackageId, DepKind)>> {
    let mut inverted = FxHashMap::<PackageId, Vec<(PackageId, DepKind)>>::default();
    for (&package_id, dependencies) in edges {
        for &(dependency_id, kind) in dependencies {
            inverted
                .entry(dependency_id)
                .or_default()
                .push((package_id, kind));
        }
    }
    for dependencies in inverted.values_mut() {
        dependencies.sort_unstable();
    }
    inverted
}

fn invert_feature_dependencies(
    dependencies: &FxHashMap<PackageId, Vec<FeatureDependency>>,
) -> FxHashMap<PackageId, Vec<FeatureDependency>> {
    let mut inverted = FxHashMap::<PackageId, Vec<FeatureDependency>>::default();
    for (&source, edges) in dependencies {
        for edge in edges {
            inverted
                .entry(edge.package_id)
                .or_default()
                .push(FeatureDependency {
                    package_id: source,
                    kind: edge.kind,
                    name: edge.name.clone(),
                    optional: edge.optional,
                    uses_default_features: edge.uses_default_features,
                    features: edge.features.clone(),
                });
        }
    }
    inverted
}

/// Helper function to resolve the manifest path, handling absolute vs relative paths and
/// defaulting to finding the workspace root when no path is provided.
pub(super) fn resolve_manifest_path(
    gctx: &GlobalContext,
    manifest_path: Option<PathBuf>,
) -> Result<PathBuf> {
    let raw = match manifest_path {
        Some(path) if path.is_absolute() => path,
        Some(path) => gctx.cwd().join(path),
        None => find_root_manifest_for_wd(gctx.cwd()).context("failed to find Cargo.toml")?,
    };
    // Cargo's `Workspace::new` compares manifest paths against the normalized
    // paths it discovers via filesystem walks. Without lexical normalization,
    // an input like `../zed/Cargo.toml` produces `.../cwd/../zed/Cargo.toml`
    // and trips `validate_members` with a "wrong workspace" error.
    Ok(normalize_path(&raw))
}
