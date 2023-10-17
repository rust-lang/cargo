use crate::core::compiler::{CompileKind, CompileMode, Layout, RustcTargetData};
use crate::core::profiles::Profiles;
use crate::core::{PackageIdSpec, TargetKind, Workspace};
use crate::ops;
use crate::util::edit_distance;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::util::{human_readable_bytes, Config, Progress, ProgressStyle};
use anyhow::bail;
use cargo_util::paths;
use std::fs;
use std::path::{Path, PathBuf};

pub struct CleanOptions<'cfg> {
    pub config: &'cfg Config,
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
    /// If set, doesn't delete anything.
    pub dry_run: bool,
}

pub struct CleanContext<'cfg> {
    pub config: &'cfg Config,
    progress: Box<dyn CleaningProgressBar + 'cfg>,
    pub dry_run: bool,
    num_files_removed: u64,
    num_dirs_removed: u64,
    total_bytes_removed: u64,
}

/// Cleans various caches.
pub fn clean(ws: &Workspace<'_>, opts: &CleanOptions<'_>) -> CargoResult<()> {
    let mut target_dir = ws.target_dir();
    let config = opts.config;
    let mut ctx = CleanContext::new(config);
    ctx.dry_run = opts.dry_run;

    if opts.doc {
        if !opts.spec.is_empty() {
            // FIXME: https://github.com/rust-lang/cargo/issues/8790
            // This should support the ability to clean specific packages
            // within the doc directory. It's a little tricky since it
            // needs to find all documentable targets, but also consider
            // the fact that target names might overlap with dependency
            // names and such.
            bail!("--doc cannot be used with -p");
        }
        // If the doc option is set, we just want to delete the doc directory.
        target_dir = target_dir.join("doc");
        ctx.remove_paths(&[target_dir.into_path_unlocked()])?;
    } else {
        let profiles = Profiles::new(&ws, opts.requested_profile)?;

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
            ctx.remove_paths(&[target_dir.into_path_unlocked()])?;
        } else {
            clean_specs(&mut ctx, &ws, &profiles, &opts.targets, &opts.spec)?;
        }
    }

    ctx.display_summary()?;
    Ok(())
}

fn clean_specs(
    ctx: &mut CleanContext<'_>,
    ws: &Workspace<'_>,
    profiles: &Profiles,
    targets: &[String],
    spec: &[String],
) -> CargoResult<()> {
    // Clean specific packages.
    let requested_kinds = CompileKind::from_requested_targets(ctx.config, targets)?;
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
    let layouts = if targets.is_empty() {
        vec![(CompileKind::Host, &host_layout)]
    } else {
        target_layouts
            .iter()
            .map(|(kind, layout)| (*kind, layout))
            .collect()
    };
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
    for spec_str in spec.iter() {
        // Translate the spec to a Package.
        let spec = PackageIdSpec::parse(spec_str)?;
        if spec.partial_version().is_some() {
            ctx.config.shell().warn(&format!(
                "version qualifier in `-p {}` is ignored, \
                cleaning all versions of `{}` found",
                spec_str,
                spec.name()
            ))?;
        }
        if spec.url().is_some() {
            ctx.config.shell().warn(&format!(
                "url qualifier in `-p {}` ignored, \
                cleaning all versions of `{}` found",
                spec_str,
                spec.name()
            ))?;
        }
        let matches: Vec<_> = resolve.iter().filter(|id| spec.matches(*id)).collect();
        if matches.is_empty() {
            let mut suggestion = String::new();
            suggestion.push_str(&edit_distance::closest_msg(
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

    ctx.progress = Box::new(CleaningPackagesBar::new(ctx.config, packages.len()));

    for pkg in packages {
        let pkg_dir = format!("{}-*", pkg.name());
        ctx.progress.on_cleaning_package(&pkg.name())?;

        // Clean fingerprints.
        for (_, layout) in &layouts_with_host {
            let dir = escape_glob_path(layout.fingerprint())?;
            ctx.rm_rf_package_glob_containing_hash(&pkg.name(), &Path::new(&dir).join(&pkg_dir))?;
        }

        for target in pkg.targets() {
            if target.is_custom_build() {
                // Get both the build_script_build and the output directory.
                for (_, layout) in &layouts_with_host {
                    let dir = escape_glob_path(layout.build())?;
                    ctx.rm_rf_package_glob_containing_hash(
                        &pkg.name(),
                        &Path::new(&dir).join(&pkg_dir),
                    )?;
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
                        let dir_glob = escape_glob_path(dir)?;
                        let dir_glob = Path::new(&dir_glob);

                        ctx.rm_rf_glob(&dir_glob.join(&hashed_name))?;
                        ctx.rm_rf(&dir.join(&unhashed_name))?;
                        // Remove dep-info file generated by rustc. It is not tracked in
                        // file_types. It does not have a prefix.
                        let hashed_dep_info = dir_glob.join(format!("{}-*.d", crate_name));
                        ctx.rm_rf_glob(&hashed_dep_info)?;
                        let unhashed_dep_info = dir.join(format!("{}.d", crate_name));
                        ctx.rm_rf(&unhashed_dep_info)?;
                        // Remove split-debuginfo files generated by rustc.
                        let split_debuginfo_obj = dir_glob.join(format!("{}.*.o", crate_name));
                        ctx.rm_rf_glob(&split_debuginfo_obj)?;
                        let split_debuginfo_dwo = dir_glob.join(format!("{}.*.dwo", crate_name));
                        ctx.rm_rf_glob(&split_debuginfo_dwo)?;
                        let split_debuginfo_dwp = dir_glob.join(format!("{}.*.dwp", crate_name));
                        ctx.rm_rf_glob(&split_debuginfo_dwp)?;

                        // Remove the uplifted copy.
                        if let Some(uplift_dir) = uplift_dir {
                            let uplifted_path = uplift_dir.join(file_type.uplift_filename(target));
                            ctx.rm_rf(&uplifted_path)?;
                            // Dep-info generated by Cargo itself.
                            let dep_info = uplifted_path.with_extension("d");
                            ctx.rm_rf(&dep_info)?;
                        }
                    }
                    // TODO: what to do about build_script_build?
                    let dir = escape_glob_path(layout.incremental())?;
                    let incremental = Path::new(&dir).join(format!("{}-*", crate_name));
                    ctx.rm_rf_glob(&incremental)?;
                }
            }
        }
    }

    Ok(())
}

fn escape_glob_path(pattern: &Path) -> CargoResult<String> {
    let pattern = pattern
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("expected utf-8 path"))?;
    Ok(glob::Pattern::escape(pattern))
}

impl<'cfg> CleanContext<'cfg> {
    pub fn new(config: &'cfg Config) -> Self {
        // This progress bar will get replaced, this is just here to avoid needing
        // an Option until the actual bar is created.
        let progress = CleaningFolderBar::new(config, 0);
        CleanContext {
            config,
            progress: Box::new(progress),
            dry_run: false,
            num_files_removed: 0,
            num_dirs_removed: 0,
            total_bytes_removed: 0,
        }
    }

    /// Glob remove artifacts for the provided `package`
    ///
    /// Make sure the artifact is for `package` and not another crate that is prefixed by
    /// `package` by getting the original name stripped of the trailing hash and possible
    /// extension
    fn rm_rf_package_glob_containing_hash(
        &mut self,
        package: &str,
        pattern: &Path,
    ) -> CargoResult<()> {
        // TODO: Display utf8 warning to user?  Or switch to globset?
        let pattern = pattern
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("expected utf-8 path"))?;
        for path in glob::glob(pattern)? {
            let path = path?;

            let pkg_name = path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .and_then(|artifact| artifact.rsplit_once('-'))
                .ok_or_else(|| anyhow::anyhow!("expected utf-8 path"))?
                .0;

            if pkg_name != package {
                continue;
            }

            self.rm_rf(&path)?;
        }
        Ok(())
    }

    fn rm_rf_glob(&mut self, pattern: &Path) -> CargoResult<()> {
        // TODO: Display utf8 warning to user?  Or switch to globset?
        let pattern = pattern
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("expected utf-8 path"))?;
        for path in glob::glob(pattern)? {
            self.rm_rf(&path?)?;
        }
        Ok(())
    }

    pub fn rm_rf(&mut self, path: &Path) -> CargoResult<()> {
        let meta = match fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    self.config
                        .shell()
                        .warn(&format!("cannot access {}: {e}", path.display()))?;
                }
                return Ok(());
            }
        };

        // dry-run displays paths while walking, so don't print here.
        if !self.dry_run {
            self.config
                .shell()
                .verbose(|shell| shell.status("Removing", path.display()))?;
        }
        self.progress.display_now()?;

        let mut rm_file = |path: &Path, meta: Result<std::fs::Metadata, _>| {
            if let Ok(meta) = meta {
                // Note: This can over-count bytes removed for hard-linked
                // files. It also under-counts since it only counts the exact
                // byte sizes and not the block sizes.
                self.total_bytes_removed += meta.len();
            }
            self.num_files_removed += 1;
            if !self.dry_run {
                paths::remove_file(path)?;
            }
            Ok(())
        };

        if !meta.is_dir() {
            return rm_file(path, Ok(meta));
        }

        for entry in walkdir::WalkDir::new(path).contents_first(true) {
            let entry = entry?;
            self.progress.on_clean()?;
            if self.dry_run {
                // This prints the path without the "Removing" status since I feel
                // like it can be surprising or even frightening if cargo says it
                // is removing something without actually removing it. And I can't
                // come up with a different verb to use as the status.
                self.config
                    .shell()
                    .verbose(|shell| Ok(writeln!(shell.out(), "{}", entry.path().display())?))?;
            }
            if entry.file_type().is_dir() {
                self.num_dirs_removed += 1;
                // The contents should have been removed by now, but sometimes a race condition is hit
                // where other files have been added by the OS. `paths::remove_dir_all` also falls back
                // to `std::fs::remove_dir_all`, which may be more reliable than a simple walk in
                // platform-specific edge cases.
                if !self.dry_run {
                    paths::remove_dir_all(entry.path())?;
                }
            } else {
                rm_file(entry.path(), entry.metadata())?;
            }
        }

        Ok(())
    }

    fn display_summary(&self) -> CargoResult<()> {
        let status = if self.dry_run { "Summary" } else { "Removed" };
        let byte_count = if self.total_bytes_removed == 0 {
            String::new()
        } else {
            // Don't show a fractional number of bytes.
            if self.total_bytes_removed < 1024 {
                format!(", {}B total", self.total_bytes_removed)
            } else {
                let (bytes, unit) = human_readable_bytes(self.total_bytes_removed);
                format!(", {bytes:.1}{unit} total")
            }
        };
        // I think displaying the number of directories removed isn't
        // particularly interesting to the user. However, if there are 0
        // files, and a nonzero number of directories, cargo should indicate
        // that it did *something*, so directory counts are only shown in that
        // case.
        let file_count = match (self.num_files_removed, self.num_dirs_removed) {
            (0, 0) => format!("0 files"),
            (0, 1) => format!("1 directory"),
            (0, 2..) => format!("{} directories", self.num_dirs_removed),
            (1, _) => format!("1 file"),
            (2.., _) => format!("{} files", self.num_files_removed),
        };
        self.config
            .shell()
            .status(status, format!("{file_count}{byte_count}"))?;
        if self.dry_run {
            self.config
                .shell()
                .warn("no files deleted due to --dry-run")?;
        }
        Ok(())
    }

    /// Deletes all of the given paths, showing a progress bar as it proceeds.
    ///
    /// If any path does not exist, or is not accessible, this will not
    /// generate an error. This only generates an error for other issues, like
    /// not being able to write to the console.
    pub fn remove_paths(&mut self, paths: &[PathBuf]) -> CargoResult<()> {
        let num_paths = paths
            .iter()
            .map(|path| walkdir::WalkDir::new(path).into_iter().count())
            .sum();
        self.progress = Box::new(CleaningFolderBar::new(self.config, num_paths));
        for path in paths {
            self.rm_rf(path)?;
        }
        Ok(())
    }
}

trait CleaningProgressBar {
    fn display_now(&mut self) -> CargoResult<()>;
    fn on_clean(&mut self) -> CargoResult<()>;
    fn on_cleaning_package(&mut self, _package: &str) -> CargoResult<()> {
        Ok(())
    }
}

struct CleaningFolderBar<'cfg> {
    bar: Progress<'cfg>,
    max: usize,
    cur: usize,
}

impl<'cfg> CleaningFolderBar<'cfg> {
    fn new(cfg: &'cfg Config, max: usize) -> Self {
        Self {
            bar: Progress::with_style("Cleaning", ProgressStyle::Percentage, cfg),
            max,
            cur: 0,
        }
    }

    fn cur_progress(&self) -> usize {
        std::cmp::min(self.cur, self.max)
    }
}

impl<'cfg> CleaningProgressBar for CleaningFolderBar<'cfg> {
    fn display_now(&mut self) -> CargoResult<()> {
        self.bar.tick_now(self.cur_progress(), self.max, "")
    }

    fn on_clean(&mut self) -> CargoResult<()> {
        self.cur += 1;
        self.bar.tick(self.cur_progress(), self.max, "")
    }
}

struct CleaningPackagesBar<'cfg> {
    bar: Progress<'cfg>,
    max: usize,
    cur: usize,
    num_files_folders_cleaned: usize,
    package_being_cleaned: String,
}

impl<'cfg> CleaningPackagesBar<'cfg> {
    fn new(cfg: &'cfg Config, max: usize) -> Self {
        Self {
            bar: Progress::with_style("Cleaning", ProgressStyle::Ratio, cfg),
            max,
            cur: 0,
            num_files_folders_cleaned: 0,
            package_being_cleaned: String::new(),
        }
    }

    fn cur_progress(&self) -> usize {
        std::cmp::min(self.cur, self.max)
    }

    fn format_message(&self) -> String {
        format!(
            ": {}, {} files/folders cleaned",
            self.package_being_cleaned, self.num_files_folders_cleaned
        )
    }
}

impl<'cfg> CleaningProgressBar for CleaningPackagesBar<'cfg> {
    fn display_now(&mut self) -> CargoResult<()> {
        self.bar
            .tick_now(self.cur_progress(), self.max, &self.format_message())
    }

    fn on_clean(&mut self) -> CargoResult<()> {
        self.bar
            .tick(self.cur_progress(), self.max, &self.format_message())?;
        self.num_files_folders_cleaned += 1;
        Ok(())
    }

    fn on_cleaning_package(&mut self, package: &str) -> CargoResult<()> {
        self.cur += 1;
        self.package_being_cleaned = String::from(package);
        self.bar
            .tick(self.cur_progress(), self.max, &self.format_message())
    }
}
