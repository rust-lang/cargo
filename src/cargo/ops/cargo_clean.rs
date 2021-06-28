use crate::core::compiler::{CompileKind, CompileMode, Layout, RustcTargetData};
use crate::core::profiles::Profiles;
use crate::core::{PackageIdSpec, TargetKind, Workspace};
use crate::ops;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::util::lev_distance;
use crate::util::Config;

use anyhow::Context as _;
use cargo_util::paths;
use std::fs;
use std::path::Path;

pub struct CleanOptions<'a> {
    pub config: &'a Config,
    /// A list of packages to clean. If empty, everything is cleaned.
    pub spec: Vec<String>,
    /// The target arch triple to clean, or None for the host arch
    pub targets: Vec<String>,
    /// Whether to clean the release directory
    pub profile_specified: bool,
    /// Whether to clean the directory of a certain build profile
    pub requested_profile: InternedString,
    /// Whether to just clean the doc directory
    pub doc: bool,
}

/// Cleans the package's build artifacts.
pub fn clean(ws: &Workspace<'_>, opts: &CleanOptions<'_>) -> CargoResult<()> {
    let mut target_dir = ws.target_dir();
    let config = ws.config();
    let rustc = config.load_global_rustc(Some(ws));

    // If the doc option is set, we just want to delete the doc directory.
    if opts.doc {
        target_dir = target_dir.join(rustc?.host).join("doc");
        return rm_rf(&target_dir.into_path_unlocked(), config);
    }

    let profiles = Profiles::new(ws, opts.requested_profile)?;

    if opts.profile_specified {
        // After parsing profiles we know the dir-name of the profile, if a profile
        // was passed from the command line. If so, delete only the directory of
        // that profile.
        let dir_name = profiles.get_dir_name();
        target_dir = target_dir.join(dir_name);
    }

    // If we have a spec, then we need to delete some packages, otherwise, just
    // remove the whole target directory and be done with it!
    //
    // Note that we don't bother grabbing a lock here as we're just going to
    // blow it all away anyway.
    if opts.spec.is_empty() {
        return rm_rf(&target_dir.into_path_unlocked(), config);
    }

    // Clean specific packages.
    let requested_kinds = CompileKind::from_requested_targets(config, rustc, &opts.targets)?;
    let target_data = RustcTargetData::new(ws, &requested_kinds)?;
    let (pkg_set, resolve) = ops::resolve_ws(ws)?;
    let prof_dir_name = profiles.get_dir_name();
    let host_layout = Layout::new(ws, None, &prof_dir_name)?;
    // Convert requested kinds to a Vec of layouts.
    let target_layouts: Vec<(CompileKind, Layout)> = requested_kinds
        .into_iter()
        .filter_map(|kind| match kind {
            CompileKind::Target(target) => match Layout::new(ws, Some(target), &prof_dir_name) {
                Ok(layout) => Some(Ok((kind, layout))),
                Err(e) => Some(Err(e)),
            },
            CompileKind::Host => None,
        })
        .collect::<CargoResult<_>>()?;
    // A Vec of layouts. This is a little convoluted because there can only be
    // one host_layout.
    let layouts: Vec<(CompileKind, &Layout)> = target_layouts
        .iter()
        .map(|(kind, layout)| (*kind, layout))
        .collect();
    // Create a Vec that also includes the host for things that need to clean both.
    let layouts_with_host: Vec<(CompileKind, &Layout)> =
        std::iter::once((CompileKind::Host, &host_layout))
            .chain(layouts.iter().map(|(k, l)| (*k, *l)))
            .collect();

    // Cleaning individual rustdoc crates is currently not supported.
    // For example, the search index would need to be rebuilt to fully
    // remove it (otherwise you're left with lots of broken links).
    // Doc tests produce no output.

    // Get Packages for the specified specs.
    let mut pkg_ids = Vec::new();
    for spec_str in opts.spec.iter() {
        // Translate the spec to a Package.
        let spec = PackageIdSpec::parse(spec_str)?;
        if spec.version().is_some() {
            config.shell().warn(&format!(
                "version qualifier in `-p {}` is ignored, \
                cleaning all versions of `{}` found",
                spec_str,
                spec.name()
            ))?;
        }
        if spec.url().is_some() {
            config.shell().warn(&format!(
                "url qualifier in `-p {}` ignored, \
                cleaning all versions of `{}` found",
                spec_str,
                spec.name()
            ))?;
        }
        let matches: Vec<_> = resolve.iter().filter(|id| spec.matches(*id)).collect();
        if matches.is_empty() {
            let mut suggestion = String::new();
            suggestion.push_str(&lev_distance::closest_msg(
                &spec.name(),
                resolve.iter(),
                |id| id.name().as_str(),
            ));
            anyhow::bail!(
                "package ID specification `{}` did not match any packages{}",
                spec,
                suggestion
            );
        }
        pkg_ids.extend(matches);
    }
    let packages = pkg_set.get_many(pkg_ids)?;

    for pkg in packages {
        let pkg_dir = format!("{}-*", pkg.name());

        // Clean fingerprints.
        for (_, layout) in &layouts_with_host {
            rm_rf_glob(&layout.fingerprint().join(&pkg_dir), config)?;
        }

        for target in pkg.targets() {
            if target.is_custom_build() {
                // Get both the build_script_build and the output directory.
                for (_, layout) in &layouts_with_host {
                    rm_rf_glob(&layout.build().join(&pkg_dir), config)?;
                }
                continue;
            }
            let crate_name = target.crate_name();
            for &mode in &[
                CompileMode::Build,
                CompileMode::Test,
                CompileMode::Check { test: false },
            ] {
                for (compile_kind, layout) in &layouts {
                    let triple = target_data.short_name(compile_kind);

                    let (file_types, _unsupported) = target_data
                        .info(*compile_kind)
                        .rustc_outputs(mode, target.kind(), triple)?;
                    let (dir, uplift_dir) = match target.kind() {
                        TargetKind::ExampleBin | TargetKind::ExampleLib(..) => {
                            (layout.examples(), Some(layout.examples()))
                        }
                        // Tests/benchmarks are never uplifted.
                        TargetKind::Test | TargetKind::Bench => (layout.deps(), None),
                        _ => (layout.deps(), Some(layout.dest())),
                    };
                    for file_type in file_types {
                        // Some files include a hash in the filename, some don't.
                        let hashed_name = file_type.output_filename(target, Some("*"));
                        let unhashed_name = file_type.output_filename(target, None);
                        rm_rf_glob(&dir.join(&hashed_name), config)?;
                        rm_rf(&dir.join(&unhashed_name), config)?;
                        // Remove dep-info file generated by rustc. It is not tracked in
                        // file_types. It does not have a prefix.
                        let hashed_dep_info = dir.join(format!("{}-*.d", crate_name));
                        rm_rf_glob(&hashed_dep_info, config)?;
                        let unhashed_dep_info = dir.join(format!("{}.d", crate_name));
                        rm_rf(&unhashed_dep_info, config)?;
                        // Remove split-debuginfo files generated by rustc.
                        let split_debuginfo_obj = dir.join(format!("{}.*.o", crate_name));
                        rm_rf_glob(&split_debuginfo_obj, config)?;
                        let split_debuginfo_dwo = dir.join(format!("{}.*.dwo", crate_name));
                        rm_rf_glob(&split_debuginfo_dwo, config)?;

                        // Remove the uplifted copy.
                        if let Some(uplift_dir) = uplift_dir {
                            let uplifted_path = uplift_dir.join(file_type.uplift_filename(target));
                            rm_rf(&uplifted_path, config)?;
                            // Dep-info generated by Cargo itself.
                            let dep_info = uplifted_path.with_extension("d");
                            rm_rf(&dep_info, config)?;
                        }
                    }
                    // TODO: what to do about build_script_build?
                    let incremental = layout.incremental().join(format!("{}-*", crate_name));
                    rm_rf_glob(&incremental, config)?;
                }
            }
        }
    }

    Ok(())
}

fn rm_rf_glob(pattern: &Path, config: &Config) -> CargoResult<()> {
    // TODO: Display utf8 warning to user?  Or switch to globset?
    let pattern = pattern
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("expected utf-8 path"))?;
    for path in glob::glob(pattern)? {
        rm_rf(&path?, config)?;
    }
    Ok(())
}

fn rm_rf(path: &Path, config: &Config) -> CargoResult<()> {
    let m = fs::symlink_metadata(path);
    if m.as_ref().map(|s| s.is_dir()).unwrap_or(false) {
        config
            .shell()
            .verbose(|shell| shell.status("Removing", path.display()))?;
        paths::remove_dir_all(path).with_context(|| "could not remove build directory")?;
    } else if m.is_ok() {
        config
            .shell()
            .verbose(|shell| shell.status("Removing", path.display()))?;
        paths::remove_file(path).with_context(|| "failed to remove build artifact")?;
    }
    Ok(())
}
