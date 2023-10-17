//! Type definitions for the result of a compilation.

use std::collections::{BTreeSet, HashMap};
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use cargo_platform::CfgExpr;
use cargo_util::{paths, ProcessBuilder};

use crate::core::compiler::apply_env_config;
use crate::core::compiler::BuildContext;
use crate::core::compiler::{CompileKind, Metadata, Unit};
use crate::core::Package;
use crate::util::{config, CargoResult, Config};

/// Structure with enough information to run `rustdoc --test`.
pub struct Doctest {
    /// What's being doctested
    pub unit: Unit,
    /// Arguments needed to pass to rustdoc to run this test.
    pub args: Vec<OsString>,
    /// Whether or not -Zunstable-options is needed.
    pub unstable_opts: bool,
    /// The -Clinker value to use.
    pub linker: Option<PathBuf>,
    /// The script metadata, if this unit's package has a build script.
    ///
    /// This is used for indexing [`Compilation::extra_env`].
    pub script_meta: Option<Metadata>,

    /// Environment variables to set in the rustdoc process.
    pub env: HashMap<String, OsString>,
}

/// Information about the output of a unit.
#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub struct UnitOutput {
    /// The unit that generated this output.
    pub unit: Unit,
    /// Path to the unit's primary output (an executable or cdylib).
    pub path: PathBuf,
    /// The script metadata, if this unit's package has a build script.
    ///
    /// This is used for indexing [`Compilation::extra_env`].
    pub script_meta: Option<Metadata>,
}

/// A structure returning the result of a compilation.
pub struct Compilation<'cfg> {
    /// An array of all tests created during this compilation.
    pub tests: Vec<UnitOutput>,

    /// An array of all binaries created.
    pub binaries: Vec<UnitOutput>,

    /// An array of all cdylibs created.
    pub cdylibs: Vec<UnitOutput>,

    /// The crate names of the root units specified on the command-line.
    pub root_crate_names: Vec<String>,

    /// All directories for the output of native build commands.
    ///
    /// This is currently used to drive some entries which are added to the
    /// LD_LIBRARY_PATH as appropriate.
    ///
    /// The order should be deterministic.
    pub native_dirs: BTreeSet<PathBuf>,

    /// Root output directory (for the local package's artifacts)
    pub root_output: HashMap<CompileKind, PathBuf>,

    /// Output directory for rust dependencies.
    /// May be for the host or for a specific target.
    pub deps_output: HashMap<CompileKind, PathBuf>,

    /// The path to the host libdir for the compiler used
    sysroot_host_libdir: PathBuf,

    /// The path to libstd for each target
    sysroot_target_libdir: HashMap<CompileKind, PathBuf>,

    /// Extra environment variables that were passed to compilations and should
    /// be passed to future invocations of programs.
    ///
    /// The key is the build script metadata for uniquely identifying the
    /// `RunCustomBuild` unit that generated these env vars.
    pub extra_env: HashMap<Metadata, Vec<(String, String)>>,

    /// Libraries to test with rustdoc.
    pub to_doc_test: Vec<Doctest>,

    /// The target host triple.
    pub host: String,

    config: &'cfg Config,

    /// Rustc process to be used by default
    rustc_process: ProcessBuilder,
    /// Rustc process to be used for workspace crates instead of rustc_process
    rustc_workspace_wrapper_process: ProcessBuilder,
    /// Optional rustc process to be used for primary crates instead of either rustc_process or
    /// rustc_workspace_wrapper_process
    primary_rustc_process: Option<ProcessBuilder>,

    target_runners: HashMap<CompileKind, Option<(PathBuf, Vec<String>)>>,
    /// The linker to use for each host or target.
    target_linkers: HashMap<CompileKind, Option<PathBuf>>,
}

impl<'cfg> Compilation<'cfg> {
    pub fn new<'a>(bcx: &BuildContext<'a, 'cfg>) -> CargoResult<Compilation<'cfg>> {
        let mut rustc = bcx.rustc().process();
        let mut primary_rustc_process = bcx.build_config.primary_unit_rustc.clone();
        let mut rustc_workspace_wrapper_process = bcx.rustc().workspace_process();

        if bcx.config.extra_verbose() {
            rustc.display_env_vars();
            rustc_workspace_wrapper_process.display_env_vars();

            if let Some(rustc) = primary_rustc_process.as_mut() {
                rustc.display_env_vars();
            }
        }

        Ok(Compilation {
            // TODO: deprecated; remove.
            native_dirs: BTreeSet::new(),
            root_output: HashMap::new(),
            deps_output: HashMap::new(),
            sysroot_host_libdir: bcx
                .target_data
                .info(CompileKind::Host)
                .sysroot_host_libdir
                .clone(),
            sysroot_target_libdir: get_sysroot_target_libdir(bcx)?,
            tests: Vec::new(),
            binaries: Vec::new(),
            cdylibs: Vec::new(),
            root_crate_names: Vec::new(),
            extra_env: HashMap::new(),
            to_doc_test: Vec::new(),
            config: bcx.config,
            host: bcx.host_triple().to_string(),
            rustc_process: rustc,
            rustc_workspace_wrapper_process,
            primary_rustc_process,
            target_runners: bcx
                .build_config
                .requested_kinds
                .iter()
                .chain(Some(&CompileKind::Host))
                .map(|kind| Ok((*kind, target_runner(bcx, *kind)?)))
                .collect::<CargoResult<HashMap<_, _>>>()?,
            target_linkers: bcx
                .build_config
                .requested_kinds
                .iter()
                .chain(Some(&CompileKind::Host))
                .map(|kind| Ok((*kind, target_linker(bcx, *kind)?)))
                .collect::<CargoResult<HashMap<_, _>>>()?,
        })
    }

    /// Returns a [`ProcessBuilder`] for running `rustc`.
    ///
    /// `is_primary` is true if this is a "primary package", which means it
    /// was selected by the user on the command-line (such as with a `-p`
    /// flag), see [`crate::core::compiler::Context::primary_packages`].
    ///
    /// `is_workspace` is true if this is a workspace member.
    pub fn rustc_process(
        &self,
        unit: &Unit,
        is_primary: bool,
        is_workspace: bool,
    ) -> CargoResult<ProcessBuilder> {
        let rustc = if is_primary && self.primary_rustc_process.is_some() {
            self.primary_rustc_process.clone().unwrap()
        } else if is_workspace {
            self.rustc_workspace_wrapper_process.clone()
        } else {
            self.rustc_process.clone()
        };

        let cmd = fill_rustc_tool_env(rustc, unit);
        self.fill_env(cmd, &unit.pkg, None, unit.kind, true)
    }

    /// Returns a [`ProcessBuilder`] for running `rustdoc`.
    pub fn rustdoc_process(
        &self,
        unit: &Unit,
        script_meta: Option<Metadata>,
    ) -> CargoResult<ProcessBuilder> {
        let rustdoc = ProcessBuilder::new(&*self.config.rustdoc()?);
        let cmd = fill_rustc_tool_env(rustdoc, unit);
        let mut cmd = self.fill_env(cmd, &unit.pkg, script_meta, unit.kind, true)?;
        cmd.retry_with_argfile(true);
        unit.target.edition().cmd_edition_arg(&mut cmd);

        for crate_type in unit.target.rustc_crate_types() {
            cmd.arg("--crate-type").arg(crate_type.as_str());
        }

        Ok(cmd)
    }

    /// Returns a [`ProcessBuilder`] appropriate for running a process for the
    /// host platform.
    ///
    /// This is currently only used for running build scripts. If you use this
    /// for anything else, please be extra careful on how environment
    /// variables are set!
    pub fn host_process<T: AsRef<OsStr>>(
        &self,
        cmd: T,
        pkg: &Package,
    ) -> CargoResult<ProcessBuilder> {
        self.fill_env(
            ProcessBuilder::new(cmd),
            pkg,
            None,
            CompileKind::Host,
            false,
        )
    }

    pub fn target_runner(&self, kind: CompileKind) -> Option<&(PathBuf, Vec<String>)> {
        self.target_runners.get(&kind).and_then(|x| x.as_ref())
    }

    /// Gets the user-specified linker for a particular host or target.
    pub fn target_linker(&self, kind: CompileKind) -> Option<PathBuf> {
        self.target_linkers.get(&kind).and_then(|x| x.clone())
    }

    /// Returns a [`ProcessBuilder`] appropriate for running a process for the
    /// target platform. This is typically used for `cargo run` and `cargo
    /// test`.
    ///
    /// `script_meta` is the metadata for the `RunCustomBuild` unit that this
    /// unit used for its build script. Use `None` if the package did not have
    /// a build script.
    pub fn target_process<T: AsRef<OsStr>>(
        &self,
        cmd: T,
        kind: CompileKind,
        pkg: &Package,
        script_meta: Option<Metadata>,
    ) -> CargoResult<ProcessBuilder> {
        let builder = if let Some((runner, args)) = self.target_runner(kind) {
            let mut builder = ProcessBuilder::new(runner);
            builder.args(args);
            builder.arg(cmd);
            builder
        } else {
            ProcessBuilder::new(cmd)
        };
        self.fill_env(builder, pkg, script_meta, kind, false)
    }

    /// Prepares a new process with an appropriate environment to run against
    /// the artifacts produced by the build process.
    ///
    /// The package argument is also used to configure environment variables as
    /// well as the working directory of the child process.
    fn fill_env(
        &self,
        mut cmd: ProcessBuilder,
        pkg: &Package,
        script_meta: Option<Metadata>,
        kind: CompileKind,
        is_rustc_tool: bool,
    ) -> CargoResult<ProcessBuilder> {
        let mut search_path = Vec::new();
        if is_rustc_tool {
            search_path.push(self.deps_output[&CompileKind::Host].clone());
            search_path.push(self.sysroot_host_libdir.clone());
        } else {
            search_path.extend(super::filter_dynamic_search_path(
                self.native_dirs.iter(),
                &self.root_output[&kind],
            ));
            search_path.push(self.deps_output[&kind].clone());
            search_path.push(self.root_output[&kind].clone());
            // For build-std, we don't want to accidentally pull in any shared
            // libs from the sysroot that ships with rustc. This may not be
            // required (at least I cannot craft a situation where it
            // matters), but is here to be safe.
            if self.config.cli_unstable().build_std.is_none() {
                search_path.push(self.sysroot_target_libdir[&kind].clone());
            }
        }

        let dylib_path = paths::dylib_path();
        let dylib_path_is_empty = dylib_path.is_empty();
        search_path.extend(dylib_path.into_iter());
        if cfg!(target_os = "macos") && dylib_path_is_empty {
            // These are the defaults when DYLD_FALLBACK_LIBRARY_PATH isn't
            // set or set to an empty string. Since Cargo is explicitly setting
            // the value, make sure the defaults still work.
            if let Some(home) = self.config.get_env_os("HOME") {
                search_path.push(PathBuf::from(home).join("lib"));
            }
            search_path.push(PathBuf::from("/usr/local/lib"));
            search_path.push(PathBuf::from("/usr/lib"));
        }
        let search_path = paths::join_paths(&search_path, paths::dylib_path_envvar())?;

        cmd.env(paths::dylib_path_envvar(), &search_path);
        if let Some(meta) = script_meta {
            if let Some(env) = self.extra_env.get(&meta) {
                for (k, v) in env {
                    cmd.env(k, v);
                }
            }
        }

        let metadata = pkg.manifest().metadata();

        let cargo_exe = self.config.cargo_exe()?;
        cmd.env(crate::CARGO_ENV, cargo_exe);

        // When adding new environment variables depending on
        // crate properties which might require rebuild upon change
        // consider adding the corresponding properties to the hash
        // in BuildContext::target_metadata()
        let rust_version = pkg.rust_version().as_ref().map(ToString::to_string);
        cmd.env("CARGO_MANIFEST_DIR", pkg.root())
            .env("CARGO_PKG_VERSION_MAJOR", &pkg.version().major.to_string())
            .env("CARGO_PKG_VERSION_MINOR", &pkg.version().minor.to_string())
            .env("CARGO_PKG_VERSION_PATCH", &pkg.version().patch.to_string())
            .env("CARGO_PKG_VERSION_PRE", pkg.version().pre.as_str())
            .env("CARGO_PKG_VERSION", &pkg.version().to_string())
            .env("CARGO_PKG_NAME", &*pkg.name())
            .env(
                "CARGO_PKG_DESCRIPTION",
                metadata.description.as_ref().unwrap_or(&String::new()),
            )
            .env(
                "CARGO_PKG_HOMEPAGE",
                metadata.homepage.as_ref().unwrap_or(&String::new()),
            )
            .env(
                "CARGO_PKG_REPOSITORY",
                metadata.repository.as_ref().unwrap_or(&String::new()),
            )
            .env(
                "CARGO_PKG_LICENSE",
                metadata.license.as_ref().unwrap_or(&String::new()),
            )
            .env(
                "CARGO_PKG_LICENSE_FILE",
                metadata.license_file.as_ref().unwrap_or(&String::new()),
            )
            .env("CARGO_PKG_AUTHORS", &pkg.authors().join(":"))
            .env(
                "CARGO_PKG_RUST_VERSION",
                &rust_version.as_deref().unwrap_or_default(),
            )
            .env(
                "CARGO_PKG_README",
                metadata.readme.as_ref().unwrap_or(&String::new()),
            )
            .cwd(pkg.root());

        apply_env_config(self.config, &mut cmd)?;

        Ok(cmd)
    }
}

/// Prepares a rustc_tool process with additional environment variables
/// that are only relevant in a context that has a unit
fn fill_rustc_tool_env(mut cmd: ProcessBuilder, unit: &Unit) -> ProcessBuilder {
    if unit.target.is_executable() {
        let name = unit
            .target
            .binary_filename()
            .unwrap_or(unit.target.name().to_string());

        cmd.env("CARGO_BIN_NAME", name);
    }
    cmd.env("CARGO_CRATE_NAME", unit.target.crate_name());
    cmd
}

fn get_sysroot_target_libdir(
    bcx: &BuildContext<'_, '_>,
) -> CargoResult<HashMap<CompileKind, PathBuf>> {
    bcx.all_kinds
        .iter()
        .map(|&kind| {
            let Some(info) = bcx.target_data.get_info(kind) else {
                let target = match kind {
                    CompileKind::Host => "host".to_owned(),
                    CompileKind::Target(s) => s.short_name().to_owned(),
                };

                let dependency = bcx
                    .unit_graph
                    .iter()
                    .find_map(|(u, _)| (u.kind == kind).then_some(u.pkg.summary().package_id()))
                    .unwrap();

                anyhow::bail!(
                    "could not find specification for target `{target}`.\n  \
                    Dependency `{dependency}` requires to build for target `{target}`."
                )
            };

            Ok((kind, info.sysroot_target_libdir.clone()))
        })
        .collect()
}

fn target_runner(
    bcx: &BuildContext<'_, '_>,
    kind: CompileKind,
) -> CargoResult<Option<(PathBuf, Vec<String>)>> {
    let target = bcx.target_data.short_name(&kind);

    // try target.{}.runner
    let key = format!("target.{}.runner", target);

    if let Some(v) = bcx.config.get::<Option<config::PathAndArgs>>(&key)? {
        let path = v.path.resolve_program(bcx.config);
        return Ok(Some((path, v.args)));
    }

    // try target.'cfg(...)'.runner
    let target_cfg = bcx.target_data.info(kind).cfg();
    let mut cfgs = bcx
        .config
        .target_cfgs()?
        .iter()
        .filter_map(|(key, cfg)| cfg.runner.as_ref().map(|runner| (key, runner)))
        .filter(|(key, _runner)| CfgExpr::matches_key(key, target_cfg));
    let matching_runner = cfgs.next();
    if let Some((key, runner)) = cfgs.next() {
        anyhow::bail!(
            "several matching instances of `target.'cfg(..)'.runner` in configurations\n\
             first match `{}` located in {}\n\
             second match `{}` located in {}",
            matching_runner.unwrap().0,
            matching_runner.unwrap().1.definition,
            key,
            runner.definition
        );
    }
    Ok(matching_runner.map(|(_k, runner)| {
        (
            runner.val.path.clone().resolve_program(bcx.config),
            runner.val.args.clone(),
        )
    }))
}

/// Gets the user-specified linker for a particular host or target from the configuration.
fn target_linker(bcx: &BuildContext<'_, '_>, kind: CompileKind) -> CargoResult<Option<PathBuf>> {
    // Try host.linker and target.{}.linker.
    if let Some(path) = bcx
        .target_data
        .target_config(kind)
        .linker
        .as_ref()
        .map(|l| l.val.clone().resolve_program(bcx.config))
    {
        return Ok(Some(path));
    }

    // Try target.'cfg(...)'.linker.
    let target_cfg = bcx.target_data.info(kind).cfg();
    let mut cfgs = bcx
        .config
        .target_cfgs()?
        .iter()
        .filter_map(|(key, cfg)| cfg.linker.as_ref().map(|linker| (key, linker)))
        .filter(|(key, _linker)| CfgExpr::matches_key(key, target_cfg));
    let matching_linker = cfgs.next();
    if let Some((key, linker)) = cfgs.next() {
        anyhow::bail!(
            "several matching instances of `target.'cfg(..)'.linker` in configurations\n\
             first match `{}` located in {}\n\
             second match `{}` located in {}",
            matching_linker.unwrap().0,
            matching_linker.unwrap().1.definition,
            key,
            linker.definition
        );
    }
    Ok(matching_linker.map(|(_k, linker)| linker.val.clone().resolve_program(bcx.config)))
}
