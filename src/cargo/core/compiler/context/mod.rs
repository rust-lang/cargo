#![allow(deprecated)]
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::cmp::Ordering;

use jobserver::Client;

use core::{Package, PackageId, Resolve, Target};
use core::compiler::compilation;
use core::profiles::Profile;
use util::errors::{CargoResult, CargoResultExt};
use util::{internal, profile, Config, short_hash};

use super::custom_build::{self, BuildDeps, BuildScripts, BuildState};
use super::fingerprint::Fingerprint;
use super::job_queue::JobQueue;
use super::layout::Layout;
use super::{BuildContext, Compilation, CompileMode, Executor, FileFlavor, Kind};
use super::build_plan::BuildPlan;

mod unit_dependencies;
use self::unit_dependencies::build_unit_dependencies;

mod compilation_files;
pub use self::compilation_files::{Metadata, OutputFile};
use self::compilation_files::CompilationFiles;

/// All information needed to define a Unit.
///
/// A unit is an object that has enough information so that cargo knows how to build it.
/// For example, if your package has dependencies, then every dependency will be built as a library
/// unit. If your package is a library, then it will be built as a library unit as well, or if it
/// is a binary with `main.rs`, then a binary will be output. There are also separate unit types
/// for `test`ing and `check`ing, amongst others.
///
/// The unit also holds information about all possible metadata about the package in `pkg`.
///
/// A unit needs to know extra information in addition to the type and root source file. For
/// example, it needs to know the target architecture (OS, chip arch etc.) and it needs to know
/// whether you want a debug or release build. There is enough information in this struct to figure
/// all that out.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct Unit<'a> {
    /// Information about available targets, which files to include/exclude, etc. Basically stuff in
    /// `Cargo.toml`.
    pub pkg: &'a Package,
    /// Information about the specific target to build, out of the possible targets in `pkg`. Not
    /// to be confused with *target-triple* (or *target architecture* ...), the target arch for a
    /// build.
    pub target: &'a Target,
    /// The profile contains information about *how* the build should be run, including debug
    /// level, etc.
    pub profile: Profile,
    /// Whether this compilation unit is for the host or target architecture.
    ///
    /// For example, when
    /// cross compiling and using a custom build script, the build script needs to be compiled for
    /// the host architecture so the host rustc can use it (when compiling to the target
    /// architecture).
    pub kind: Kind,
    /// The "mode" this unit is being compiled for.  See `CompileMode` for
    /// more details.
    pub mode: CompileMode,
}

impl<'a> Unit<'a> {
    pub fn buildkey(&self) -> String {
        format!("{}-{}", self.pkg.name(), short_hash(self))
	}
}

impl<'a> Ord for Unit<'a> {
    fn cmp(&self, other: &Unit) -> Ordering {
        self.buildkey().cmp(&other.buildkey())
    }
}

impl<'a> PartialOrd for Unit<'a> {
    fn partial_cmp(&self, other: &Unit) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct Context<'a, 'cfg: 'a> {
    pub bcx: &'a BuildContext<'a, 'cfg>,
    pub compilation: Compilation<'cfg>,
    pub build_state: Arc<BuildState>,
    pub build_script_overridden: HashSet<(PackageId, Kind)>,
    pub build_explicit_deps: HashMap<Unit<'a>, BuildDeps>,
    pub fingerprints: HashMap<Unit<'a>, Arc<Fingerprint>>,
    pub compiled: HashSet<Unit<'a>>,
    pub build_scripts: HashMap<Unit<'a>, Arc<BuildScripts>>,
    pub links: Links<'a>,
    pub jobserver: Client,
    primary_packages: HashSet<&'a PackageId>,
    unit_dependencies: HashMap<Unit<'a>, Vec<Unit<'a>>>,
    files: Option<CompilationFiles<'a, 'cfg>>,
    package_cache: HashMap<&'a PackageId, &'a Package>,
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
        })
    }

    // Returns a mapping of the root package plus its immediate dependencies to
    // where the compiled libraries are all located.
    pub fn compile(
        mut self,
        units: &[Unit<'a>],
        export_dir: Option<PathBuf>,
        exec: &Arc<Executor>,
    ) -> CargoResult<Compilation<'cfg>> {
        let mut queue = JobQueue::new(self.bcx);
        let mut plan = BuildPlan::new();
        let build_plan = self.bcx.build_config.build_plan;
        self.prepare_units(export_dir, units)?;
        self.prepare()?;
        custom_build::build_map(&mut self, units)?;

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

                let bindst = match output.hardlink {
                    Some(ref link_dst) => link_dst,
                    None => &output.path,
                };

                if unit.mode == CompileMode::Test {
                    self.compilation.tests.push((
                        unit.pkg.clone(),
                        unit.target.kind().clone(),
                        unit.target.name().to_string(),
                        output.path.clone(),
                    ));
                } else if unit.target.is_bin() || unit.target.is_bin_example() {
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
                        .entry(dep.pkg.package_id().clone())
                        .or_insert_with(Vec::new)
                        .push(("OUT_DIR".to_string(), out_dir));
                }
            }

            if unit.mode == CompileMode::Doctest {
                // Note that we can *only* doctest rlib outputs here.  A
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
                    .entry(unit.pkg.package_id().clone())
                    .or_insert_with(|| {
                        feats
                            .iter()
                            .map(|feat| format!("feature=\"{}\"", feat))
                            .collect()
                    });
            }
            let rustdocflags = self.bcx.rustdocflags_args(unit)?;
            if !rustdocflags.is_empty() {
                self.compilation
                    .rustdocflags
                    .entry(unit.pkg.package_id().clone())
                    .or_insert(rustdocflags);
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
        self.primary_packages.extend(units.iter().map(|u| u.pkg.package_id()));

        build_unit_dependencies(
            units,
            self.bcx,
            &mut self.unit_dependencies,
            &mut self.package_cache,
        )?;
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

    /// Return the filenames that the given target for the given profile will
    /// generate as a list of 3-tuples (filename, link_dst, linkable)
    ///
    ///  - filename: filename rustc compiles to. (Often has metadata suffix).
    ///  - link_dst: Optional file to link/copy the result to (without metadata suffix)
    ///  - linkable: Whether possible to link against file (eg it's a library)
    pub fn outputs(&mut self, unit: &Unit<'a>) -> CargoResult<Arc<Vec<OutputFile>>> {
        self.files.as_ref().unwrap().outputs(unit, self.bcx)
    }

    /// For a package, return all targets which are registered as dependencies
    /// for that package.
    // TODO: this ideally should be `-> &[Unit<'a>]`
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
            let key = (unit.pkg.package_id().clone(), unit.kind);
            if self.build_script_overridden.contains(&key) {
                return Vec::new();
            }
        }
        let mut deps = self.unit_dependencies[unit].clone();
        deps.sort();
        deps
    }

    pub fn incremental_args(&self, unit: &Unit) -> CargoResult<Vec<String>> {
        // There's a number of ways to configure incremental compilation right
        // now. In order of descending priority (first is highest priority) we
        // have:
        //
        // * `CARGO_INCREMENTAL` - this is blanket used unconditionally to turn
        //   on/off incremental compilation for any cargo subcommand. We'll
        //   respect this if set.
        // * `build.incremental` - in `.cargo/config` this blanket key can
        //   globally for a system configure whether incremental compilation is
        //   enabled. Note that setting this to `true` will not actually affect
        //   all builds though. For example a `true` value doesn't enable
        //   release incremental builds, only dev incremental builds. This can
        //   be useful to globally disable incremental compilation like
        //   `CARGO_INCREMENTAL`.
        // * `profile.dev.incremental` - in `Cargo.toml` specific profiles can
        //   be configured to enable/disable incremental compilation. This can
        //   be primarily used to disable incremental when buggy for a package.
        // * Finally, each profile has a default for whether it will enable
        //   incremental compilation or not. Primarily development profiles
        //   have it enabled by default while release profiles have it disabled
        //   by default.
        let global_cfg = self.bcx
            .config
            .get_bool("build.incremental")?
            .map(|c| c.val);
        let incremental = match (
            self.bcx.incremental_env,
            global_cfg,
            unit.profile.incremental,
        ) {
            (Some(v), _, _) => v,
            (None, Some(false), _) => false,
            (None, _, other) => other,
        };

        if !incremental {
            return Ok(Vec::new());
        }

        // Only enable incremental compilation for sources the user can
        // modify (aka path sources). For things that change infrequently,
        // non-incremental builds yield better performance in the compiler
        // itself (aka crates.io / git dependencies)
        //
        // (see also https://github.com/rust-lang/cargo/issues/3972)
        if !unit.pkg.package_id().source_id().is_path() {
            return Ok(Vec::new());
        }

        let dir = self.files().layout(unit.kind).incremental().display();
        Ok(vec!["-C".to_string(), format!("incremental={}", dir)])
    }

    pub fn is_primary_package(&self, unit: &Unit<'a>) -> bool {
        self.primary_packages.contains(unit.pkg.package_id())
    }

    /// Gets a package for the given package id.
    pub fn get_package(&self, id: &PackageId) -> CargoResult<&'a Package> {
        self.package_cache.get(id)
            .cloned()
            .ok_or_else(|| format_err!("failed to find {}", id))
    }

    /// Return the list of filenames read by cargo to generate the BuildContext
    /// (all Cargo.toml, etc).
    pub fn build_plan_inputs(&self) -> CargoResult<Vec<PathBuf>> {
        let mut inputs = Vec::new();
        // Note that we're using the `package_cache`, which should have been
        // populated by `build_unit_dependencies`, and only those packages are
        // considered as all the inputs.
        //
        // (notably we skip dev-deps here if they aren't present)
        for pkg in self.package_cache.values() {
            inputs.push(pkg.manifest_path().to_path_buf());
        }
        inputs.sort();
        Ok(inputs)
    }
}

#[derive(Default)]
pub struct Links<'a> {
    validated: HashSet<&'a PackageId>,
    links: HashMap<String, &'a PackageId>,
}

impl<'a> Links<'a> {
    pub fn new() -> Links<'a> {
        Links {
            validated: HashSet::new(),
            links: HashMap::new(),
        }
    }

    pub fn validate(&mut self, resolve: &Resolve, unit: &Unit<'a>) -> CargoResult<()> {
        if !self.validated.insert(unit.pkg.package_id()) {
            return Ok(());
        }
        let lib = match unit.pkg.manifest().links() {
            Some(lib) => lib,
            None => return Ok(()),
        };
        if let Some(prev) = self.links.get(lib) {
            let pkg = unit.pkg.package_id();

            let describe_path = |pkgid: &PackageId| -> String {
                let dep_path = resolve.path_to_top(pkgid);
                let mut dep_path_desc = format!("package `{}`", dep_path[0]);
                for dep in dep_path.iter().skip(1) {
                    write!(dep_path_desc, "\n    ... which is depended on by `{}`", dep).unwrap();
                }
                dep_path_desc
            };

            bail!(
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
        if !unit.pkg
            .manifest()
            .targets()
            .iter()
            .any(|t| t.is_custom_build())
        {
            bail!(
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
