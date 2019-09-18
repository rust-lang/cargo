use std::collections::{BTreeSet, HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;

use semver::Version;

use super::BuildContext;
use crate::core::{Edition, InternedString, Package, PackageId, Target};
use crate::util::{
    self, join_paths, process, rustc::Rustc, CargoResult, CfgExpr, Config, ProcessBuilder,
};

pub struct Doctest {
    /// The package being doc-tested.
    pub package: Package,
    /// The target being tested (currently always the package's lib).
    pub target: Target,
    /// Extern dependencies needed by `rustdoc`. The path is the location of
    /// the compiled lib.
    pub deps: Vec<(InternedString, PathBuf)>,
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
    supports_rustdoc_crate_type: bool,
}

impl<'cfg> Compilation<'cfg> {
    pub fn new<'a>(bcx: &BuildContext<'a, 'cfg>) -> CargoResult<Compilation<'cfg>> {
        let mut rustc = bcx.rustc.process();

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
            host_dylib_path: bcx.host_info.sysroot_libdir.clone(),
            target_dylib_path: bcx.target_info.sysroot_libdir.clone(),
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
            target: bcx.target_triple().to_string(),
            target_runner: target_runner(bcx)?,
            supports_rustdoc_crate_type: supports_rustdoc_crate_type(bcx.config, &bcx.rustc)?,
        })
    }

    /// See `process`.
    pub fn rustc_process(
        &self,
        pkg: &Package,
        target: &Target,
        is_primary: bool,
    ) -> CargoResult<ProcessBuilder> {
        let rustc = if is_primary {
            self.primary_unit_rustc_process
                .clone()
                .unwrap_or_else(|| self.rustc_process.clone())
        } else {
            self.rustc_process.clone()
        };

        let mut p = self.fill_env(rustc, pkg, true)?;
        if target.edition() != Edition::Edition2015 {
            p.arg(format!("--edition={}", target.edition()));
        }
        Ok(p)
    }

    /// See `process`.
    pub fn rustdoc_process(&self, pkg: &Package, target: &Target) -> CargoResult<ProcessBuilder> {
        let mut p = self.fill_env(process(&*self.config.rustdoc()?), pkg, false)?;
        if target.edition() != Edition::Edition2015 {
            p.arg(format!("--edition={}", target.edition()));
        }

        if self.supports_rustdoc_crate_type {
            for crate_type in target.rustc_crate_types() {
                p.arg("--crate-type").arg(crate_type);
            }
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
            search_path.push(self.target_dylib_path.clone());
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

fn target_runner(bcx: &BuildContext<'_, '_>) -> CargoResult<Option<(PathBuf, Vec<String>)>> {
    let target = bcx.target_triple();

    // try target.{}.runner
    let key = format!("target.{}.runner", target);
    if let Some(v) = bcx.config.get_path_and_args(&key)? {
        return Ok(Some(v.val));
    }

    // try target.'cfg(...)'.runner
    if let Some(table) = bcx.config.get_table("target")? {
        let mut matching_runner = None;

        for key in table.val.keys() {
            if CfgExpr::matches_key(key, bcx.target_info.cfg()) {
                let key = format!("target.{}.runner", key);
                if let Some(runner) = bcx.config.get_path_and_args(&key)? {
                    // more than one match, error out
                    if matching_runner.is_some() {
                        failure::bail!(
                            "several matching instances of `target.'cfg(..)'.runner` \
                             in `.cargo/config`"
                        )
                    }

                    matching_runner = Some(runner.val);
                }
            }
        }

        return Ok(matching_runner);
    }

    Ok(None)
}

fn supports_rustdoc_crate_type(config: &Config, rustc: &Rustc) -> CargoResult<bool> {
    // NOTE: Unconditionally return 'true' once support for
    // rustdoc '--crate-type' rides to stable
    let mut crate_type_test = process(config.rustdoc()?);
    // If '--crate-type' is not supported by rustcoc, this command
    // will exit with an error. Otherwise, it will print a help message,
    // and exit successfully
    crate_type_test.args(&["--crate-type", "proc-macro", "--help"]);
    Ok(rustc.cached_output(&crate_type_test).is_ok())
}
