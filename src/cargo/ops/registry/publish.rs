//! Interacts with the registry [publish API][1].
//!
//! [1]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html#publish

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fs::File;
use std::time::Duration;

use anyhow::bail;
use anyhow::Context as _;
use cargo_credential::Operation;
use cargo_credential::Secret;
use cargo_util::paths;
use crates_io::NewCrate;
use crates_io::NewCrateDependency;
use crates_io::Registry;

use crate::core::dependency::DepKind;
use crate::core::manifest::ManifestMetadata;
use crate::core::resolver::CliFeatures;
use crate::core::Dependency;
use crate::core::Package;
use crate::core::SourceId;
use crate::core::Workspace;
use crate::ops;
use crate::ops::PackageOpts;
use crate::ops::Packages;
use crate::sources::source::QueryKind;
use crate::sources::SourceConfigMap;
use crate::sources::CRATES_IO_REGISTRY;
use crate::util::auth;
use crate::util::cache_lock::CacheLockMode;
use crate::util::config::JobsConfig;
use crate::util::Progress;
use crate::util::ProgressStyle;
use crate::CargoResult;
use crate::Config;

use super::super::check_dep_has_version;
use super::RegistryOrIndex;

pub struct PublishOpts<'cfg> {
    pub config: &'cfg Config,
    pub token: Option<Secret<String>>,
    pub reg_or_index: Option<RegistryOrIndex>,
    pub verify: bool,
    pub allow_dirty: bool,
    pub jobs: Option<JobsConfig>,
    pub keep_going: bool,
    pub to_publish: ops::Packages,
    pub targets: Vec<String>,
    pub dry_run: bool,
    pub cli_features: CliFeatures,
}

pub fn publish(ws: &Workspace<'_>, opts: &PublishOpts<'_>) -> CargoResult<()> {
    let specs = opts.to_publish.to_package_id_specs(ws)?;
    if specs.len() > 1 {
        bail!("the `-p` argument must be specified to select a single package to publish")
    }
    if Packages::Default == opts.to_publish && ws.is_virtual() {
        bail!("the `-p` argument must be specified in the root of a virtual workspace")
    }
    let member_ids = ws.members().map(|p| p.package_id());
    // Check that the spec matches exactly one member.
    specs[0].query(member_ids)?;
    let mut pkgs = ws.members_with_features(&specs, &opts.cli_features)?;
    // In `members_with_features_old`, it will add "current" package (determined by the cwd)
    // So we need filter
    pkgs = pkgs
        .into_iter()
        .filter(|(m, _)| specs.iter().any(|spec| spec.matches(m.package_id())))
        .collect();
    // Double check. It is safe theoretically, unless logic has updated.
    assert_eq!(pkgs.len(), 1);

    let (pkg, cli_features) = pkgs.pop().unwrap();

    let mut publish_registry = match opts.reg_or_index.as_ref() {
        Some(RegistryOrIndex::Registry(registry)) => Some(registry.clone()),
        _ => None,
    };
    if let Some(ref allowed_registries) = *pkg.publish() {
        if publish_registry.is_none() && allowed_registries.len() == 1 {
            // If there is only one allowed registry, push to that one directly,
            // even though there is no registry specified in the command.
            let default_registry = &allowed_registries[0];
            if default_registry != CRATES_IO_REGISTRY {
                // Don't change the registry for crates.io and don't warn the user.
                // crates.io will be defaulted even without this.
                opts.config.shell().note(&format!(
                    "Found `{}` as only allowed registry. Publishing to it automatically.",
                    default_registry
                ))?;
                publish_registry = Some(default_registry.clone());
            }
        }

        let reg_name = publish_registry
            .clone()
            .unwrap_or_else(|| CRATES_IO_REGISTRY.to_string());
        if allowed_registries.is_empty() {
            bail!(
                "`{}` cannot be published.\n\
                 `package.publish` is set to `false` or an empty list in Cargo.toml and prevents publishing.",
                pkg.name(),
            );
        } else if !allowed_registries.contains(&reg_name) {
            bail!(
                "`{}` cannot be published.\n\
                 The registry `{}` is not listed in the `package.publish` value in Cargo.toml.",
                pkg.name(),
                reg_name
            );
        }
    }
    // This is only used to confirm that we can create a token before we build the package.
    // This causes the credential provider to be called an extra time, but keeps the same order of errors.
    let ver = pkg.version().to_string();
    let operation = Operation::Read;

    let reg_or_index = match opts.reg_or_index.clone() {
        Some(RegistryOrIndex::Registry(_)) | None => {
            publish_registry.map(RegistryOrIndex::Registry)
        }
        val => val,
    };
    let (mut registry, reg_ids) = super::registry(
        opts.config,
        opts.token.as_ref().map(Secret::as_deref),
        reg_or_index.as_ref(),
        true,
        Some(operation).filter(|_| !opts.dry_run),
    )?;
    verify_dependencies(pkg, &registry, reg_ids.original)?;

    // Prepare a tarball, with a non-suppressible warning if metadata
    // is missing since this is being put online.
    let tarball = ops::package_one(
        ws,
        pkg,
        &PackageOpts {
            config: opts.config,
            verify: opts.verify,
            list: false,
            check_metadata: true,
            allow_dirty: opts.allow_dirty,
            to_package: Packages::Default,
            targets: opts.targets.clone(),
            jobs: opts.jobs.clone(),
            keep_going: opts.keep_going,
            cli_features,
        },
    )?
    .unwrap();

    if !opts.dry_run {
        let hash = cargo_util::Sha256::new()
            .update_file(tarball.file())?
            .finish_hex();
        let operation = Operation::Publish {
            name: pkg.name().as_str(),
            vers: &ver,
            cksum: &hash,
        };
        registry.set_token(Some(auth::auth_token(
            &opts.config,
            &reg_ids.original,
            None,
            operation,
            vec![],
            false,
        )?));
    }

    opts.config
        .shell()
        .status("Uploading", pkg.package_id().to_string())?;
    transmit(
        opts.config,
        pkg,
        tarball.file(),
        &mut registry,
        reg_ids.original,
        opts.dry_run,
    )?;
    if !opts.dry_run {
        const DEFAULT_TIMEOUT: u64 = 60;
        let timeout = if opts.config.cli_unstable().publish_timeout {
            let timeout: Option<u64> = opts.config.get("publish.timeout")?;
            timeout.unwrap_or(DEFAULT_TIMEOUT)
        } else {
            DEFAULT_TIMEOUT
        };
        if 0 < timeout {
            let timeout = Duration::from_secs(timeout);
            wait_for_publish(opts.config, reg_ids.original, pkg, timeout)?;
        }
    }

    Ok(())
}

fn wait_for_publish(
    config: &Config,
    registry_src: SourceId,
    pkg: &Package,
    timeout: Duration,
) -> CargoResult<()> {
    let version_req = format!("={}", pkg.version());
    let mut source = SourceConfigMap::empty(config)?.load(registry_src, &HashSet::new())?;
    // Disable the source's built-in progress bars. Repeatedly showing a bunch
    // of independent progress bars can be a little confusing. There is an
    // overall progress bar managed here.
    source.set_quiet(true);
    let source_description = source.source_id().to_string();
    let query = Dependency::parse(pkg.name(), Some(&version_req), registry_src)?;

    let now = std::time::Instant::now();
    let sleep_time = Duration::from_secs(1);
    let max = timeout.as_secs() as usize;
    // Short does not include the registry name.
    let short_pkg_description = format!("{} v{}", pkg.name(), pkg.version());
    config.shell().status(
        "Uploaded",
        format!("{short_pkg_description} to {source_description}"),
    )?;
    config.shell().note(format!(
        "Waiting for `{short_pkg_description}` to be available at {source_description}.\n\
        You may press ctrl-c to skip waiting; the crate should be available shortly."
    ))?;
    let mut progress = Progress::with_style("Waiting", ProgressStyle::Ratio, config);
    progress.tick_now(0, max, "")?;
    let is_available = loop {
        {
            let _lock = config.acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;
            // Force re-fetching the source
            //
            // As pulling from a git source is expensive, we track when we've done it within the
            // process to only do it once, but we are one of the rare cases that needs to do it
            // multiple times
            config
                .updated_sources()
                .remove(&source.replaced_source_id());
            source.invalidate_cache();
            let summaries = loop {
                // Exact to avoid returning all for path/git
                match source.query_vec(&query, QueryKind::Exact) {
                    std::task::Poll::Ready(res) => {
                        break res?;
                    }
                    std::task::Poll::Pending => source.block_until_ready()?,
                }
            };
            if !summaries.is_empty() {
                break true;
            }
        }

        let elapsed = now.elapsed();
        if timeout < elapsed {
            config.shell().warn(format!(
                "timed out waiting for `{short_pkg_description}` to be available in {source_description}",
            ))?;
            config.shell().note(
                "The registry may have a backlog that is delaying making the \
                crate available. The crate should be available soon.",
            )?;
            break false;
        }

        progress.tick_now(elapsed.as_secs() as usize, max, "")?;
        std::thread::sleep(sleep_time);
    };
    if is_available {
        config.shell().status(
            "Published",
            format!("{short_pkg_description} at {source_description}"),
        )?;
    }

    Ok(())
}

fn verify_dependencies(
    pkg: &Package,
    registry: &Registry,
    registry_src: SourceId,
) -> CargoResult<()> {
    for dep in pkg.dependencies().iter() {
        if check_dep_has_version(dep, true)? {
            continue;
        }
        // TomlManifest::prepare_for_publish will rewrite the dependency
        // to be just the `version` field.
        if dep.source_id() != registry_src {
            if !dep.source_id().is_registry() {
                // Consider making SourceId::kind a public type that we can
                // exhaustively match on. Using match can help ensure that
                // every kind is properly handled.
                panic!("unexpected source kind for dependency {:?}", dep);
            }
            // Block requests to send to crates.io with alt-registry deps.
            // This extra hostname check is mostly to assist with testing,
            // but also prevents someone using `--index` to specify
            // something that points to crates.io.
            if registry_src.is_crates_io() || registry.host_is_crates_io() {
                bail!("crates cannot be published to crates.io with dependencies sourced from other\n\
                       registries. `{}` needs to be published to crates.io before publishing this crate.\n\
                       (crate `{}` is pulled from {})",
                      dep.package_name(),
                      dep.package_name(),
                      dep.source_id());
            }
        }
    }
    Ok(())
}

fn transmit(
    config: &Config,
    pkg: &Package,
    tarball: &File,
    registry: &mut Registry,
    registry_id: SourceId,
    dry_run: bool,
) -> CargoResult<()> {
    let deps = pkg
        .dependencies()
        .iter()
        .filter(|dep| {
            // Skip dev-dependency without version.
            dep.is_transitive() || dep.specified_req()
        })
        .map(|dep| {
            // If the dependency is from a different registry, then include the
            // registry in the dependency.
            let dep_registry_id = match dep.registry_id() {
                Some(id) => id,
                None => SourceId::crates_io(config)?,
            };
            // In the index and Web API, None means "from the same registry"
            // whereas in Cargo.toml, it means "from crates.io".
            let dep_registry = if dep_registry_id != registry_id {
                Some(dep_registry_id.url().to_string())
            } else {
                None
            };

            Ok(NewCrateDependency {
                optional: dep.is_optional(),
                default_features: dep.uses_default_features(),
                name: dep.package_name().to_string(),
                features: dep.features().iter().map(|s| s.to_string()).collect(),
                version_req: dep.version_req().to_string(),
                target: dep.platform().map(|s| s.to_string()),
                kind: match dep.kind() {
                    DepKind::Normal => "normal",
                    DepKind::Build => "build",
                    DepKind::Development => "dev",
                }
                .to_string(),
                registry: dep_registry,
                explicit_name_in_toml: dep.explicit_name_in_toml().map(|s| s.to_string()),
                artifact: dep.artifact().map(|artifact| {
                    artifact
                        .kinds()
                        .iter()
                        .map(|x| x.as_str().into_owned())
                        .collect()
                }),
                bindep_target: dep.artifact().and_then(|artifact| {
                    artifact.target().map(|target| target.as_str().to_owned())
                }),
                lib: dep.artifact().map_or(false, |artifact| artifact.is_lib()),
            })
        })
        .collect::<CargoResult<Vec<NewCrateDependency>>>()?;
    let manifest = pkg.manifest();
    let ManifestMetadata {
        ref authors,
        ref description,
        ref homepage,
        ref documentation,
        ref keywords,
        ref readme,
        ref repository,
        ref license,
        ref license_file,
        ref categories,
        ref badges,
        ref links,
        ref rust_version,
    } = *manifest.metadata();
    let rust_version = rust_version.as_ref().map(ToString::to_string);
    let readme_content = readme
        .as_ref()
        .map(|readme| {
            paths::read(&pkg.root().join(readme))
                .with_context(|| format!("failed to read `readme` file for package `{}`", pkg))
        })
        .transpose()?;
    if let Some(ref file) = *license_file {
        if !pkg.root().join(file).exists() {
            bail!("the license file `{}` does not exist", file)
        }
    }

    // Do not upload if performing a dry run
    if dry_run {
        config.shell().warn("aborting upload due to dry run")?;
        return Ok(());
    }

    let string_features = match manifest.original().features() {
        Some(features) => features
            .iter()
            .map(|(feat, values)| {
                (
                    feat.to_string(),
                    values.iter().map(|fv| fv.to_string()).collect(),
                )
            })
            .collect::<BTreeMap<String, Vec<String>>>(),
        None => BTreeMap::new(),
    };

    let warnings = registry
        .publish(
            &NewCrate {
                name: pkg.name().to_string(),
                vers: pkg.version().to_string(),
                deps,
                features: string_features,
                authors: authors.clone(),
                description: description.clone(),
                homepage: homepage.clone(),
                documentation: documentation.clone(),
                keywords: keywords.clone(),
                categories: categories.clone(),
                readme: readme_content,
                readme_file: readme.clone(),
                repository: repository.clone(),
                license: license.clone(),
                license_file: license_file.clone(),
                badges: badges.clone(),
                links: links.clone(),
                rust_version,
            },
            tarball,
        )
        .with_context(|| format!("failed to publish to registry at {}", registry.host()))?;

    if !warnings.invalid_categories.is_empty() {
        let msg = format!(
            "the following are not valid category slugs and were \
             ignored: {}. Please see https://crates.io/category_slugs \
             for the list of all category slugs. \
             ",
            warnings.invalid_categories.join(", ")
        );
        config.shell().warn(&msg)?;
    }

    if !warnings.invalid_badges.is_empty() {
        let msg = format!(
            "the following are not valid badges and were ignored: {}. \
             Either the badge type specified is unknown or a required \
             attribute is missing. Please see \
             https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata \
             for valid badge types and their required attributes.",
            warnings.invalid_badges.join(", ")
        );
        config.shell().warn(&msg)?;
    }

    if !warnings.other.is_empty() {
        for msg in warnings.other {
            config.shell().warn(&msg)?;
        }
    }

    Ok(())
}
