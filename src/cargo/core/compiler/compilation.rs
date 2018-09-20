use std::collections::{BTreeSet, HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;

use semver::Version;

use core::{Edition, Package, PackageId, Target, TargetKind};
use util::{self, join_paths, process, CargoResult, CfgExpr, Config, ProcessBuilder};
use super::BuildContext;

pub struct Doctest {
    /// The package being doctested.
    pub package: Package,
    /// The target being tested (currently always the package's lib).
    pub target: Target,
    /// Extern dependencies needed by `rustdoc`. The path is the location of
    /// the compiled lib.
    pub deps: Vec<(String, PathBuf)>,
}

/// A structure returning the result of a compilation.
pub struct Compilation<'cfg> {
    /// A mapping from a package to the list of libraries that need to be
    /// linked when working with that package.
    // TODO: deprecated, remove
    pub libraries: HashMap<PackageId, HashSet<(Target, PathBuf)>>,

    /// An array of all tests created during this compilation.
    pub tests: Vec<(Package, TargetKind, String, PathBuf)>,

    /// An array of all binaries created.
    pub binaries: Vec<PathBuf>,

    /// All directories for the output of native build commands.
    ///
    /// This is currently used to drive some entries which are added to the
    /// LD_LIBRARY_PATH as appropriate.
    ///
    /// The order should be deterministic.
    // TODO: deprecated, remove
    pub native_dirs: BTreeSet<PathBuf>,

    /// Root output directory (for the local package's artifacts)
    pub root_output: PathBuf,

    /// Output directory for rust dependencies.
    /// May be for the host or for a specific target.
    pub deps_output: PathBuf,

    /// Output directory for the rust host dependencies.
    pub host_deps_output: PathBuf,

    /// The path to rustc's own libstd
    pub host_dylib_path: Option<PathBuf>,

    /// The path to libstd for the target
    pub target_dylib_path: Option<PathBuf>,

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

    target_runner: Option<(PathBuf, Vec<String>)>,
}

impl<'cfg> Compilation<'cfg> {
    pub fn new<'a>(bcx: &BuildContext<'a, 'cfg>) -> CargoResult<Compilation<'cfg>> {
        // If we're using cargo as a rustc wrapper then we're in a situation
        // like `cargo fix`. For now just disregard the `RUSTC_WRAPPER` env var
        // (which is typically set to `sccache` for now). Eventually we'll
        // probably want to implement `RUSTC_WRAPPER` for `cargo fix`, but we'll
        // leave that open as a bug for now.
        let mut rustc = if bcx.build_config.cargo_as_rustc_wrapper {
            let mut rustc = bcx.rustc.process_no_wrapper();
            let prog = rustc.get_program().to_owned();
            rustc.env("RUSTC", prog);
            rustc.program(env::current_exe()?);
            rustc
        } else {
            bcx.rustc.process()
        };
        for (k, v) in bcx.build_config.extra_rustc_env.iter() {
            rustc.env(k, v);
        }
        for arg in bcx.build_config.extra_rustc_args.iter() {
            rustc.arg(arg);
        }
        let srv = bcx.build_config.rustfix_diagnostic_server.borrow();
        if let Some(server) = &*srv {
            server.configure(&mut rustc);
        }
        Ok(Compilation {
            libraries: HashMap::new(),
            native_dirs: BTreeSet::new(), // TODO: deprecated, remove
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
            host: bcx.host_triple().to_string(),
            target: bcx.target_triple().to_string(),
            target_runner: target_runner(&bcx)?,
        })
    }

    /// See `process`.
    pub fn rustc_process(&self, pkg: &Package, target: &Target) -> CargoResult<ProcessBuilder> {
        let mut p = self.fill_env(self.rustc_process.clone(), pkg, true)?;
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

    fn target_runner(&self) -> &Option<(PathBuf, Vec<String>)> {
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
            search_path.extend(self.host_dylib_path.clone());
            search_path
        } else {
            let mut search_path =
                super::filter_dynamic_search_path(self.native_dirs.iter(), &self.root_output);
            search_path.push(self.root_output.clone());
            search_path.push(self.deps_output.clone());
            search_path.extend(self.target_dylib_path.clone());
            search_path
        };

        search_path.extend(util::dylib_path().into_iter());
        let search_path = join_paths(&search_path, util::dylib_path_envvar())?;

        cmd.env(util::dylib_path_envvar(), &search_path);
        if let Some(env) = self.extra_env.get(pkg.package_id()) {
            for &(ref k, ref v) in env {
                cmd.env(k, v);
            }
        }

        let metadata = pkg.manifest().metadata();

        let cargo_exe = self.config.cargo_exe()?;
        cmd.env(::CARGO_ENV, cargo_exe);

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

fn target_runner(bcx: &BuildContext) -> CargoResult<Option<(PathBuf, Vec<String>)>> {
    let target = bcx.target_triple();

    // try target.{}.runner
    let key = format!("target.{}.runner", target);
    if let Some(v) = bcx.config.get_path_and_args(&key)? {
        return Ok(Some(v.val));
    }

    // try target.'cfg(...)'.runner
    if let Some(target_cfg) = bcx.target_info.cfg() {
        if let Some(table) = bcx.config.get_table("target")? {
            let mut matching_runner = None;

            for key in table.val.keys() {
                if CfgExpr::matches_key(key, target_cfg) {
                    let key = format!("target.{}.runner", key);
                    if let Some(runner) = bcx.config.get_path_and_args(&key)? {
                        // more than one match, error out
                        if matching_runner.is_some() {
                            bail!("several matching instances of `target.'cfg(..)'.runner` \
                                   in `.cargo/config`")
                        }

                        matching_runner = Some(runner.val);
                    }
                }
            }

            return Ok(matching_runner);
        }
    }

    Ok(None)
}
