use std::collections::{BTreeSet, HashMap, HashSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use cargo_platform::CfgExpr;
use semver::Version;

use super::BuildContext;
use crate::core::compiler::CompileKind;
use crate::core::{Edition, Package, PackageId, Target};
use crate::util::{self, config, join_paths, process, CargoResult, Config, ProcessBuilder};

/// Structure with enough information to run `rustdoc --test`.
pub struct Doctest {
    /// The package being doc-tested.
    pub package: Package,
    /// The target being tested (currently always the package's lib).
    pub target: Target,
    /// Arguments needed to pass to rustdoc to run this test.
    pub args: Vec<OsString>,
    /// Whether or not -Zunstable-options is needed.
    pub unstable_opts: bool,
}

/// A structure returning the result of a compilation.
pub struct Compilation<'cfg> {
    /// An array of all tests created during this compilation.
    /// `(package, target, path_to_test_exe)`
    pub tests: Vec<(Package, Target, PathBuf)>,

    /// An array of all binaries created.
    pub binaries: Vec<PathBuf>,

    /// All directories for the output of native build commands.
    ///
    /// This is currently used to drive some entries which are added to the
    /// LD_LIBRARY_PATH as appropriate.
    ///
    /// The order should be deterministic.
    pub native_dirs: BTreeSet<PathBuf>,

    /// Root output directory (for the local package's artifacts)
    pub root_output: PathBuf,

    /// Output directory for rust dependencies.
    /// May be for the host or for a specific target.
    pub deps_output: PathBuf,

    /// Output directory for the rust host dependencies.
    pub host_deps_output: PathBuf,

    /// The path to rustc's own libstd
    pub host_dylib_path: PathBuf,

    /// The path to libstd for the target
    pub target_dylib_path: PathBuf,

    /// Extra environment variables that were passed to compilations and should
    /// be passed to future invocations of programs.
    pub extra_env: HashMap<PackageId, Vec<(String, String)>>,

    /// Libraries to test with rustdoc.
    pub to_doc_test: Vec<Doctest>,

    /// Features per package enabled during this compilation.
    pub cfgs: HashMap<PackageId, HashSet<String>>,

    /// Flags to pass to rustdoc when invoked from cargo test, per package.
    pub rustdocflags: HashMap<PackageId, Vec<String>>,

    pub host: String,
    pub target: String,

    config: &'cfg Config,
    rustc_process: ProcessBuilder,
    primary_unit_rustc_process: Option<ProcessBuilder>,

    target_runner: Option<(PathBuf, Vec<String>)>,
}

impl<'cfg> Compilation<'cfg> {
    pub fn new<'a>(
        bcx: &BuildContext<'a, 'cfg>,
        default_kind: CompileKind,
    ) -> CargoResult<Compilation<'cfg>> {
        let mut rustc = bcx.rustc().process();

        let mut primary_unit_rustc_process = bcx.build_config.primary_unit_rustc.clone();

        if bcx.config.extra_verbose() {
            rustc.display_env_vars();

            if let Some(rustc) = primary_unit_rustc_process.as_mut() {
                rustc.display_env_vars();
            }
        }

        Ok(Compilation {
            // TODO: deprecated; remove.
            native_dirs: BTreeSet::new(),
            root_output: PathBuf::from("/"),
            deps_output: PathBuf::from("/"),
            host_deps_output: PathBuf::from("/"),
            host_dylib_path: bcx
                .target_data
                .info(CompileKind::Host)
                .sysroot_host_libdir
                .clone(),
            target_dylib_path: bcx
                .target_data
                .info(default_kind)
                .sysroot_target_libdir
                .clone(),
            tests: Vec::new(),
            binaries: Vec::new(),
            extra_env: HashMap::new(),
            to_doc_test: Vec::new(),
            cfgs: HashMap::new(),
            rustdocflags: HashMap::new(),
            config: bcx.config,
            rustc_process: rustc,
            primary_unit_rustc_process,
            host: bcx.host_triple().to_string(),
            target: bcx.target_data.short_name(&default_kind).to_string(),
            target_runner: target_runner(bcx, default_kind)?,
        })
    }

    /// See `process`.
    pub fn rustc_process(&self, pkg: &Package, is_primary: bool) -> CargoResult<ProcessBuilder> {
        let rustc = if is_primary {
            self.primary_unit_rustc_process
                .clone()
                .unwrap_or_else(|| self.rustc_process.clone())
        } else {
            self.rustc_process.clone()
        };

        self.fill_env(rustc, pkg, true)
    }

    /// See `process`.
    pub fn rustdoc_process(&self, pkg: &Package, target: &Target) -> CargoResult<ProcessBuilder> {
        let mut p = self.fill_env(process(&*self.config.rustdoc()?), pkg, false)?;
        if target.edition() != Edition::Edition2015 {
            p.arg(format!("--edition={}", target.edition()));
        }

        for crate_type in target.rustc_crate_types() {
            p.arg("--crate-type").arg(crate_type);
        }

        Ok(p)
    }

    /// See `process`.
    pub fn host_process<T: AsRef<OsStr>>(
        &self,
        cmd: T,
        pkg: &Package,
    ) -> CargoResult<ProcessBuilder> {
        self.fill_env(process(cmd), pkg, true)
    }

    pub fn target_runner(&self) -> &Option<(PathBuf, Vec<String>)> {
        &self.target_runner
    }

    /// See `process`.
    pub fn target_process<T: AsRef<OsStr>>(
        &self,
        cmd: T,
        pkg: &Package,
    ) -> CargoResult<ProcessBuilder> {
        let builder = if let Some((ref runner, ref args)) = *self.target_runner() {
            let mut builder = process(runner);
            builder.args(args);
            builder.arg(cmd);
            builder
        } else {
            process(cmd)
        };
        self.fill_env(builder, pkg, false)
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
        is_host: bool,
    ) -> CargoResult<ProcessBuilder> {
        let mut search_path = if is_host {
            let mut search_path = vec![self.host_deps_output.clone()];
            search_path.push(self.host_dylib_path.clone());
            search_path
        } else {
            let mut search_path =
                super::filter_dynamic_search_path(self.native_dirs.iter(), &self.root_output);
            search_path.push(self.deps_output.clone());
            search_path.push(self.root_output.clone());
            // For build-std, we don't want to accidentally pull in any shared
            // libs from the sysroot that ships with rustc. This may not be
            // required (at least I cannot craft a situation where it
            // matters), but is here to be safe.
            if self.config.cli_unstable().build_std.is_none() {
                search_path.push(self.target_dylib_path.clone());
            }
            search_path
        };

        let dylib_path = util::dylib_path();
        let dylib_path_is_empty = dylib_path.is_empty();
        search_path.extend(dylib_path.into_iter());
        if cfg!(target_os = "macos") && dylib_path_is_empty {
            // These are the defaults when DYLD_FALLBACK_LIBRARY_PATH isn't
            // set or set to an empty string. Since Cargo is explicitly setting
            // the value, make sure the defaults still work.
            if let Some(home) = env::var_os("HOME") {
                search_path.push(PathBuf::from(home).join("lib"));
            }
            search_path.push(PathBuf::from("/usr/local/lib"));
            search_path.push(PathBuf::from("/usr/lib"));
        }
        let search_path = join_paths(&search_path, util::dylib_path_envvar())?;

        cmd.env(util::dylib_path_envvar(), &search_path);
        if let Some(env) = self.extra_env.get(&pkg.package_id()) {
            for &(ref k, ref v) in env {
                cmd.env(k, v);
            }
        }

        let metadata = pkg.manifest().metadata();

        let cargo_exe = self.config.cargo_exe()?;
        cmd.env(crate::CARGO_ENV, cargo_exe);

        // When adding new environment variables depending on
        // crate properties which might require rebuild upon change
        // consider adding the corresponding properties to the hash
        // in BuildContext::target_metadata()
        cmd.env("CARGO_MANIFEST_DIR", pkg.root())
            .env("CARGO_PKG_VERSION_MAJOR", &pkg.version().major.to_string())
            .env("CARGO_PKG_VERSION_MINOR", &pkg.version().minor.to_string())
            .env("CARGO_PKG_VERSION_PATCH", &pkg.version().patch.to_string())
            .env(
                "CARGO_PKG_VERSION_PRE",
                &pre_version_component(pkg.version()),
            )
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
            .env("CARGO_PKG_AUTHORS", &pkg.authors().join(":"))
            .cwd(pkg.root());
        Ok(cmd)
    }
}

fn pre_version_component(v: &Version) -> String {
    if v.pre.is_empty() {
        return String::new();
    }

    let mut ret = String::new();

    for (i, x) in v.pre.iter().enumerate() {
        if i != 0 {
            ret.push('.')
        };
        ret.push_str(&x.to_string());
    }

    ret
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
            "several matching instances of `target.'cfg(..)'.runner` in `.cargo/config`\n\
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
