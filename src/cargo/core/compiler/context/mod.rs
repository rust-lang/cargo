#![allow(deprecated)]
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Arc;

use jobserver::Client;

use crate::core::compiler::compilation;
use crate::core::compiler::Unit;
use crate::core::{Package, PackageId, Resolve};
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::{internal, profile, Config};

use super::build_plan::BuildPlan;
use super::custom_build::{self, BuildDeps, BuildScripts, BuildState};
use super::fingerprint::Fingerprint;
use super::job_queue::JobQueue;
use super::layout::Layout;
use super::{BuildContext, Compilation, CompileMode, Executor, FileFlavor, Kind};

mod unit_dependencies;
use self::unit_dependencies::build_unit_dependencies;

mod compilation_files;
use self::compilation_files::CompilationFiles;
pub use self::compilation_files::{Metadata, OutputFile};

pub struct Context<'a, 'cfg: 'a> {
    pub bcx: &'a BuildContext<'a, 'cfg>,
    pub compilation: Compilation<'cfg>,
    pub build_state: Arc<BuildState>,
    pub build_script_overridden: HashSet<(PackageId, Kind)>,
    pub build_explicit_deps: HashMap<Unit<'a>, BuildDeps>,
    pub fingerprints: HashMap<Unit<'a>, Arc<Fingerprint>>,
    pub compiled: HashSet<Unit<'a>>,
    pub build_scripts: HashMap<Unit<'a>, Arc<BuildScripts>>,
    pub links: Links,
    pub jobserver: Client,
    primary_packages: HashSet<PackageId>,
    unit_dependencies: HashMap<Unit<'a>, Vec<Unit<'a>>>,
    files: Option<CompilationFiles<'a, 'cfg>>,
    package_cache: HashMap<PackageId, &'a Package>,

    /// A flag indicating whether pipelining is enabled for this compilation
    /// session. Pipelining largely only affects the edges of the dependency
    /// graph that we generate at the end, and otherwise it's pretty
    /// straightforward.
    pipelining: bool,

    /// A set of units which are compiling rlibs and are expected to produce
    /// metadata files in addition to the rlib itself. This is only filled in
    /// when `pipelining` above is enabled.
    rmeta_required: HashSet<Unit<'a>>,
}

impl<'a, 'cfg> Context<'a, 'cfg> {
    pub fn new(config: &'cfg Config, bcx: &'a BuildContext<'a, 'cfg>) -> CargoResult<Self> {
        // Load up the jobserver that we'll use to manage our parallelism. This
        // is the same as the GNU make implementation of a jobserver, and
        // intentionally so! It's hoped that we can interact with GNU make and
        // all share the same jobserver.
        //
        // Note that if we don't have a jobserver in our environment then we
        // create our own, and we create it with `n-1` tokens because one token
        // is ourself, a running process.
        let jobserver = match config.jobserver_from_env() {
            Some(c) => c.clone(),
            None => Client::new(bcx.build_config.jobs as usize - 1)
                .chain_err(|| "failed to create jobserver")?,
        };

        let pipelining = bcx
            .config
            .get_bool("build.pipelining")?
            .map(|t| t.val)
            .unwrap_or(false);

        Ok(Self {
            bcx,
            compilation: Compilation::new(bcx)?,
            build_state: Arc::new(BuildState::new(&bcx.host_config, &bcx.target_config)),
            fingerprints: HashMap::new(),
            compiled: HashSet::new(),
            build_scripts: HashMap::new(),
            build_explicit_deps: HashMap::new(),
            links: Links::new(),
            jobserver,
            build_script_overridden: HashSet::new(),

            primary_packages: HashSet::new(),
            unit_dependencies: HashMap::new(),
            files: None,
            package_cache: HashMap::new(),
            rmeta_required: HashSet::new(),
            pipelining,
        })
    }

    // Returns a mapping of the root package plus its immediate dependencies to
    // where the compiled libraries are all located.
    pub fn compile(
        mut self,
        units: &[Unit<'a>],
        export_dir: Option<PathBuf>,
        exec: &Arc<dyn Executor>,
    ) -> CargoResult<Compilation<'cfg>> {
        let mut queue = JobQueue::new(self.bcx);
        let mut plan = BuildPlan::new();
        let build_plan = self.bcx.build_config.build_plan;
        self.prepare_units(export_dir, units)?;
        self.prepare()?;
        custom_build::build_map(&mut self, units)?;
        self.check_collistions()?;

        for unit in units.iter() {
            // Build up a list of pending jobs, each of which represent
            // compiling a particular package. No actual work is executed as
            // part of this, that's all done next as part of the `execute`
            // function which will run everything in order with proper
            // parallelism.
            let force_rebuild = self.bcx.build_config.force_rebuild;
            super::compile(&mut self, &mut queue, &mut plan, unit, exec, force_rebuild)?;
        }

        // Now that we've figured out everything that we're going to do, do it!
        queue.execute(&mut self, &mut plan)?;

        if build_plan {
            plan.set_inputs(self.build_plan_inputs()?);
            plan.output_plan();
        }

        for unit in units.iter() {
            for output in self.outputs(unit)?.iter() {
                if output.flavor == FileFlavor::DebugInfo {
                    continue;
                }

                let bindst = output.bin_dst();

                if unit.mode == CompileMode::Test {
                    self.compilation.tests.push((
                        unit.pkg.clone(),
                        unit.target.clone(),
                        output.path.clone(),
                    ));
                } else if unit.target.is_executable() {
                    self.compilation.binaries.push(bindst.clone());
                }
            }

            for dep in self.dep_targets(unit).iter() {
                if !unit.target.is_lib() {
                    continue;
                }

                if dep.mode.is_run_custom_build() {
                    let out_dir = self.files().build_script_out_dir(dep).display().to_string();
                    self.compilation
                        .extra_env
                        .entry(dep.pkg.package_id())
                        .or_insert_with(Vec::new)
                        .push(("OUT_DIR".to_string(), out_dir));
                }
            }

            if unit.mode == CompileMode::Doctest {
                // Note that we can *only* doc-test rlib outputs here. A
                // staticlib output cannot be linked by the compiler (it just
                // doesn't do that). A dylib output, however, can be linked by
                // the compiler, but will always fail. Currently all dylibs are
                // built as "static dylibs" where the standard library is
                // statically linked into the dylib. The doc tests fail,
                // however, for now as they try to link the standard library
                // dynamically as well, causing problems. As a result we only
                // pass `--extern` for rlib deps and skip out on all other
                // artifacts.
                let mut doctest_deps = Vec::new();
                for dep in self.dep_targets(unit) {
                    if dep.target.is_lib() && dep.mode == CompileMode::Build {
                        let outputs = self.outputs(&dep)?;
                        let outputs = outputs.iter().filter(|output| {
                            output.path.extension() == Some(OsStr::new("rlib"))
                                || dep.target.for_host()
                        });
                        for output in outputs {
                            doctest_deps.push((
                                self.bcx.extern_crate_name(unit, &dep)?,
                                output.path.clone(),
                            ));
                        }
                    }
                }
                // Help with tests to get a stable order with renamed deps.
                doctest_deps.sort();
                self.compilation.to_doc_test.push(compilation::Doctest {
                    package: unit.pkg.clone(),
                    target: unit.target.clone(),
                    deps: doctest_deps,
                });
            }

            let feats = self.bcx.resolve.features(unit.pkg.package_id());
            if !feats.is_empty() {
                self.compilation
                    .cfgs
                    .entry(unit.pkg.package_id())
                    .or_insert_with(|| {
                        feats
                            .iter()
                            .map(|feat| format!("feature=\"{}\"", feat))
                            .collect()
                    });
            }
            let rustdocflags = self.bcx.rustdocflags_args(unit);
            if !rustdocflags.is_empty() {
                self.compilation
                    .rustdocflags
                    .entry(unit.pkg.package_id())
                    .or_insert(rustdocflags.to_vec());
            }

            super::output_depinfo(&mut self, unit)?;
        }

        for (&(ref pkg, _), output) in self.build_state.outputs.lock().unwrap().iter() {
            self.compilation
                .cfgs
                .entry(pkg.clone())
                .or_insert_with(HashSet::new)
                .extend(output.cfgs.iter().cloned());

            self.compilation
                .extra_env
                .entry(pkg.clone())
                .or_insert_with(Vec::new)
                .extend(output.env.iter().cloned());

            for dir in output.library_paths.iter() {
                self.compilation.native_dirs.insert(dir.clone());
            }
        }
        Ok(self.compilation)
    }

    /// Returns the executable for the specified unit (if any).
    pub fn get_executable(&mut self, unit: &Unit<'a>) -> CargoResult<Option<PathBuf>> {
        for output in self.outputs(unit)?.iter() {
            if output.flavor == FileFlavor::DebugInfo {
                continue;
            }

            let is_binary = unit.target.is_executable();
            let is_test = unit.mode.is_any_test() && !unit.mode.is_check();

            if is_binary || is_test {
                return Ok(Option::Some(output.bin_dst().clone()));
            }
        }
        Ok(None)
    }

    pub fn prepare_units(
        &mut self,
        export_dir: Option<PathBuf>,
        units: &[Unit<'a>],
    ) -> CargoResult<()> {
        let dest = if self.bcx.build_config.release {
            "release"
        } else {
            "debug"
        };
        let host_layout = Layout::new(self.bcx.ws, None, dest)?;
        let target_layout = match self.bcx.build_config.requested_target.as_ref() {
            Some(target) => Some(Layout::new(self.bcx.ws, Some(target), dest)?),
            None => None,
        };
        self.primary_packages
            .extend(units.iter().map(|u| u.pkg.package_id()));

        build_unit_dependencies(self, units)?;
        let files = CompilationFiles::new(
            units,
            host_layout,
            target_layout,
            export_dir,
            self.bcx.ws,
            self,
        );
        self.files = Some(files);
        Ok(())
    }

    /// Prepare this context, ensuring that all filesystem directories are in
    /// place.
    pub fn prepare(&mut self) -> CargoResult<()> {
        let _p = profile::start("preparing layout");

        self.files_mut()
            .host
            .prepare()
            .chain_err(|| internal("couldn't prepare build directories"))?;
        if let Some(ref mut target) = self.files.as_mut().unwrap().target {
            target
                .prepare()
                .chain_err(|| internal("couldn't prepare build directories"))?;
        }

        self.compilation.host_deps_output = self.files_mut().host.deps().to_path_buf();

        let files = self.files.as_ref().unwrap();
        let layout = files.target.as_ref().unwrap_or(&files.host);
        self.compilation.root_output = layout.dest().to_path_buf();
        self.compilation.deps_output = layout.deps().to_path_buf();
        Ok(())
    }

    pub fn files(&self) -> &CompilationFiles<'a, 'cfg> {
        self.files.as_ref().unwrap()
    }

    fn files_mut(&mut self) -> &mut CompilationFiles<'a, 'cfg> {
        self.files.as_mut().unwrap()
    }

    /// Returns the filenames that the given unit will generate.
    pub fn outputs(&self, unit: &Unit<'a>) -> CargoResult<Arc<Vec<OutputFile>>> {
        self.files.as_ref().unwrap().outputs(unit, self.bcx)
    }

    /// For a package, return all targets which are registered as dependencies
    /// for that package.
    //
    // TODO: this ideally should be `-> &[Unit<'a>]`.
    pub fn dep_targets(&self, unit: &Unit<'a>) -> Vec<Unit<'a>> {
        // If this build script's execution has been overridden then we don't
        // actually depend on anything, we've reached the end of the dependency
        // chain as we've got all the info we're gonna get.
        //
        // Note there's a subtlety about this piece of code! The
        // `build_script_overridden` map here is populated in
        // `custom_build::build_map` which you need to call before inspecting
        // dependencies. However, that code itself calls this method and
        // gets a full pre-filtered set of dependencies. This is not super
        // obvious, and clear, but it does work at the moment.
        if unit.target.is_custom_build() {
            let key = (unit.pkg.package_id(), unit.kind);
            if self.build_script_overridden.contains(&key) {
                return Vec::new();
            }
        }
        self.unit_dependencies[unit].clone()
    }

    pub fn is_primary_package(&self, unit: &Unit<'a>) -> bool {
        self.primary_packages.contains(&unit.pkg.package_id())
    }

    /// Gets a package for the given package ID.
    pub fn get_package(&self, id: PackageId) -> CargoResult<&'a Package> {
        self.package_cache
            .get(&id)
            .cloned()
            .ok_or_else(|| failure::format_err!("failed to find {}", id))
    }

    /// Returns the list of filenames read by cargo to generate the `BuildContext`
    /// (all `Cargo.toml`, etc.).
    pub fn build_plan_inputs(&self) -> CargoResult<Vec<PathBuf>> {
        let mut inputs = Vec::new();
        // Note that we're using the `package_cache`, which should have been
        // populated by `build_unit_dependencies`, and only those packages are
        // considered as all the inputs.
        //
        // (Notably, we skip dev-deps here if they aren't present.)
        for pkg in self.package_cache.values() {
            inputs.push(pkg.manifest_path().to_path_buf());
        }
        inputs.sort();
        Ok(inputs)
    }

    fn check_collistions(&self) -> CargoResult<()> {
        let mut output_collisions = HashMap::new();
        let describe_collision =
            |unit: &Unit<'_>, other_unit: &Unit<'_>, path: &PathBuf| -> String {
                format!(
                    "The {} target `{}` in package `{}` has the same output \
                     filename as the {} target `{}` in package `{}`.\n\
                     Colliding filename is: {}\n",
                    unit.target.kind().description(),
                    unit.target.name(),
                    unit.pkg.package_id(),
                    other_unit.target.kind().description(),
                    other_unit.target.name(),
                    other_unit.pkg.package_id(),
                    path.display()
                )
            };
        let suggestion =
            "Consider changing their names to be unique or compiling them separately.\n\
             This may become a hard error in the future; see \
             <https://github.com/rust-lang/cargo/issues/6313>.";
        let report_collision = |unit: &Unit<'_>,
                                other_unit: &Unit<'_>,
                                path: &PathBuf|
         -> CargoResult<()> {
            if unit.target.name() == other_unit.target.name() {
                self.bcx.config.shell().warn(format!(
                    "output filename collision.\n\
                     {}\
                     The targets should have unique names.\n\
                     {}",
                    describe_collision(unit, other_unit, path),
                    suggestion
                ))
            } else {
                self.bcx.config.shell().warn(format!(
                    "output filename collision.\n\
                    {}\
                    The output filenames should be unique.\n\
                    {}\n\
                    If this looks unexpected, it may be a bug in Cargo. Please file a bug report at\n\
                    https://github.com/rust-lang/cargo/issues/ with as much information as you\n\
                    can provide.\n\
                    {} running on `{}` target `{}`\n\
                    First unit: {:?}\n\
                    Second unit: {:?}",
                    describe_collision(unit, other_unit, path),
                    suggestion,
                    crate::version(), self.bcx.host_triple(), self.bcx.target_triple(),
                    unit, other_unit))
            }
        };
        let mut keys = self
            .unit_dependencies
            .keys()
            .filter(|unit| !unit.mode.is_run_custom_build())
            .collect::<Vec<_>>();
        // Sort for consistent error messages.
        keys.sort_unstable();
        for unit in keys {
            for output in self.outputs(unit)?.iter() {
                if let Some(other_unit) = output_collisions.insert(output.path.clone(), unit) {
                    report_collision(unit, other_unit, &output.path)?;
                }
                if let Some(hardlink) = output.hardlink.as_ref() {
                    if let Some(other_unit) = output_collisions.insert(hardlink.clone(), unit) {
                        report_collision(unit, other_unit, hardlink)?;
                    }
                }
                if let Some(ref export_path) = output.export_path {
                    if let Some(other_unit) = output_collisions.insert(export_path.clone(), unit) {
                        self.bcx.config.shell().warn(format!(
                            "`--out-dir` filename collision.\n\
                             {}\
                             The exported filenames should be unique.\n\
                             {}",
                            describe_collision(unit, other_unit, export_path),
                            suggestion
                        ))?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Returns whether when `parent` depends on `dep` if it only requires the
    /// metadata file from `dep`.
    pub fn only_requires_rmeta(&self, parent: &Unit<'a>, dep: &Unit<'a>) -> bool {
        // this is only enabled when pipelining is enabled
        self.pipelining
            // We're only a candidate for requiring an `rmeta` file if we
            // ourselves are building an rlib,
            && !parent.target.requires_upstream_objects()
            && parent.mode == CompileMode::Build
            // Our dependency must also be built as an rlib, otherwise the
            // object code must be useful in some fashion
            && !dep.target.requires_upstream_objects()
            && dep.mode == CompileMode::Build
    }

    /// Returns whether when `unit` is built whether it should emit metadata as
    /// well because some compilations rely on that.
    pub fn rmeta_required(&self, unit: &Unit<'a>) -> bool {
        self.rmeta_required.contains(unit)
    }
}

#[derive(Default)]
pub struct Links {
    validated: HashSet<PackageId>,
    links: HashMap<String, PackageId>,
}

impl Links {
    pub fn new() -> Links {
        Links {
            validated: HashSet::new(),
            links: HashMap::new(),
        }
    }

    pub fn validate(&mut self, resolve: &Resolve, unit: &Unit<'_>) -> CargoResult<()> {
        if !self.validated.insert(unit.pkg.package_id()) {
            return Ok(());
        }
        let lib = match unit.pkg.manifest().links() {
            Some(lib) => lib,
            None => return Ok(()),
        };
        if let Some(&prev) = self.links.get(lib) {
            let pkg = unit.pkg.package_id();

            let describe_path = |pkgid: PackageId| -> String {
                let dep_path = resolve.path_to_top(&pkgid);
                let mut dep_path_desc = format!("package `{}`", dep_path[0]);
                for dep in dep_path.iter().skip(1) {
                    write!(dep_path_desc, "\n    ... which is depended on by `{}`", dep).unwrap();
                }
                dep_path_desc
            };

            failure::bail!(
                "multiple packages link to native library `{}`, \
                 but a native library can be linked only once\n\
                 \n\
                 {}\nlinks to native library `{}`\n\
                 \n\
                 {}\nalso links to native library `{}`",
                lib,
                describe_path(prev),
                lib,
                describe_path(pkg),
                lib
            )
        }
        if !unit
            .pkg
            .manifest()
            .targets()
            .iter()
            .any(|t| t.is_custom_build())
        {
            failure::bail!(
                "package `{}` specifies that it links to `{}` but does not \
                 have a custom build script",
                unit.pkg.package_id(),
                lib
            )
        }
        self.links.insert(lib.to_string(), unit.pkg.package_id());
        Ok(())
    }
}
