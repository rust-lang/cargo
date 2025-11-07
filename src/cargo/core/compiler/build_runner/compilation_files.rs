//! See [`CompilationFiles`].

use std::cell::OnceCell;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::debug;

use super::{BuildContext, BuildRunner, CompileKind, FileFlavor, Layout};
use crate::core::compiler::{CompileMode, CompileTarget, CrateType, FileType, Unit};
use crate::core::{Target, TargetKind, Workspace};
use crate::util::{self, CargoResult, OnceExt, StableHasher};

/// This is a generic version number that can be changed to make
/// backwards-incompatible changes to any file structures in the output
/// directory. For example, the fingerprint files or the build-script
/// output files.
///
/// Normally cargo updates ship with rustc updates which will
/// cause a new hash due to the rustc version changing, but this allows
/// cargo to be extra careful to deal with different versions of cargo that
/// use the same rustc version.
const METADATA_VERSION: u8 = 2;

/// Uniquely identify a [`Unit`] under specific circumstances, see [`Metadata`] for more.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct UnitHash(u64);

impl fmt::Display for UnitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl fmt::Debug for UnitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UnitHash({:016x})", self.0)
    }
}

/// [`Metadata`] tracks several [`UnitHash`]s, including
/// [`Metadata::unit_id`], [`Metadata::c_metadata`], and [`Metadata::c_extra_filename`].
///
/// We use a hash because it is an easy way to guarantee
/// that all the inputs can be converted to a valid path.
///
/// [`Metadata::unit_id`] is used to uniquely identify a unit in the build graph.
/// This serves as a similar role as [`Metadata::c_extra_filename`] in that it uniquely identifies output
/// on the filesystem except that its always present.
///
/// [`Metadata::c_extra_filename`] is needed for cases like:
/// - A project may depend on crate `A` and crate `B`, so the package name must be in the file name.
/// - Similarly a project may depend on two versions of `A`, so the version must be in the file name.
///
/// This also acts as the main layer of caching provided by Cargo
/// so this must include all things that need to be distinguished in different parts of
/// the same build. This is absolutely required or we override things before
/// we get chance to use them.
///
/// For example, we want to cache `cargo build` and `cargo doc` separately, so that running one
/// does not invalidate the artifacts for the other. We do this by including [`CompileMode`] in the
/// hash, thus the artifacts go in different folders and do not override each other.
/// If we don't add something that we should have, for this reason, we get the
/// correct output but rebuild more than is needed.
///
/// Some things that need to be tracked to ensure the correct output should definitely *not*
/// go in the `Metadata`. For example, the modification time of a file, should be tracked to make a
/// rebuild when the file changes. However, it would be wasteful to include in the `Metadata`. The
/// old artifacts are never going to be needed again. We can save space by just overwriting them.
/// If we add something that we should not have, for this reason, we get the correct output but take
/// more space than needed. This makes not including something in `Metadata`
/// a form of cache invalidation.
///
/// Note that the `Fingerprint` is in charge of tracking everything needed to determine if a
/// rebuild is needed.
///
/// [`Metadata::c_metadata`] is used for symbol mangling, because if you have two versions of
/// the same crate linked together, their symbols need to be differentiated.
///
/// You should avoid anything that would interfere with reproducible
/// builds. For example, *any* absolute path should be avoided. This is one
/// reason that `RUSTFLAGS` is not in [`Metadata::c_metadata`], because it often has
/// absolute paths (like `--remap-path-prefix` which is fundamentally used for
/// reproducible builds and has absolute paths in it). Also, in some cases the
/// mangled symbols need to be stable between different builds with different
/// settings. For example, profile-guided optimizations need to swap
/// `RUSTFLAGS` between runs, but needs to keep the same symbol names.
#[derive(Copy, Clone, Debug)]
pub struct Metadata {
    unit_id: UnitHash,
    c_metadata: UnitHash,
    c_extra_filename: Option<UnitHash>,
}

impl Metadata {
    /// A hash to identify a given [`Unit`] in the build graph
    pub fn unit_id(&self) -> UnitHash {
        self.unit_id
    }

    /// A hash to add to symbol naming through `-C metadata`
    pub fn c_metadata(&self) -> UnitHash {
        self.c_metadata
    }

    /// A hash to add to file names through `-C extra-filename`
    pub fn c_extra_filename(&self) -> Option<UnitHash> {
        self.c_extra_filename
    }
}

/// Collection of information about the files emitted by the compiler, and the
/// output directory structure.
pub struct CompilationFiles<'a, 'gctx> {
    /// The target directory layout for the host (and target if it is the same as host).
    pub(super) host: Layout,
    /// The target directory layout for the target (if different from then host).
    pub(super) target: HashMap<CompileTarget, Layout>,
    /// Additional directory to include a copy of the outputs.
    export_dir: Option<PathBuf>,
    /// The root targets requested by the user on the command line (does not
    /// include dependencies).
    roots: Vec<Unit>,
    ws: &'a Workspace<'gctx>,
    /// Metadata hash to use for each unit.
    metas: HashMap<Unit, Metadata>,
    /// For each Unit, a list all files produced.
    outputs: HashMap<Unit, OnceCell<Arc<Vec<OutputFile>>>>,
}

/// Info about a single file emitted by the compiler.
#[derive(Debug)]
pub struct OutputFile {
    /// Absolute path to the file that will be produced by the build process.
    pub path: PathBuf,
    /// If it should be linked into `target`, and what it should be called
    /// (e.g., without metadata).
    pub hardlink: Option<PathBuf>,
    /// If `--artifact-dir` is specified, the absolute path to the exported file.
    pub export_path: Option<PathBuf>,
    /// Type of the file (library / debug symbol / else).
    pub flavor: FileFlavor,
}

impl OutputFile {
    /// Gets the hard link if present; otherwise, returns the path.
    pub fn bin_dst(&self) -> &PathBuf {
        match self.hardlink {
            Some(ref link_dst) => link_dst,
            None => &self.path,
        }
    }
}

impl<'a, 'gctx: 'a> CompilationFiles<'a, 'gctx> {
    pub(super) fn new(
        build_runner: &BuildRunner<'a, 'gctx>,
        host: Layout,
        target: HashMap<CompileTarget, Layout>,
    ) -> CompilationFiles<'a, 'gctx> {
        let mut metas = HashMap::new();
        for unit in &build_runner.bcx.roots {
            metadata_of(unit, build_runner, &mut metas);
        }
        let outputs = metas
            .keys()
            .cloned()
            .map(|unit| (unit, OnceCell::new()))
            .collect();
        CompilationFiles {
            ws: build_runner.bcx.ws,
            host,
            target,
            export_dir: build_runner.bcx.build_config.export_dir.clone(),
            roots: build_runner.bcx.roots.clone(),
            metas,
            outputs,
        }
    }

    /// Returns the appropriate directory layout for either a plugin or not.
    pub fn layout(&self, kind: CompileKind) -> &Layout {
        match kind {
            CompileKind::Host => &self.host,
            CompileKind::Target(target) => &self.target[&target],
        }
    }

    /// Gets the metadata for the given unit.
    ///
    /// See [`Metadata`] and [`fingerprint`] module for more.
    ///
    /// [`fingerprint`]: super::super::fingerprint#fingerprints-and-metadata
    pub fn metadata(&self, unit: &Unit) -> Metadata {
        self.metas[unit]
    }

    /// Gets the short hash based only on the `PackageId`.
    /// Used for the metadata when `c_extra_filename` returns `None`.
    fn target_short_hash(&self, unit: &Unit) -> String {
        let hashable = unit.pkg.package_id().stable_hash(self.ws.root());
        util::short_hash(&(METADATA_VERSION, hashable))
    }

    /// Returns the directory where the artifacts for the given unit are
    /// initially created.
    pub fn out_dir(&self, unit: &Unit) -> PathBuf {
        // Docscrape units need to have doc/ set as the out_dir so sources for reverse-dependencies
        // will be put into doc/ and not into deps/ where the *.examples files are stored.
        if unit.mode.is_doc() || unit.mode.is_doc_scrape() {
            self.layout(unit.kind).artifact_dir().doc().to_path_buf()
        } else if unit.mode.is_doc_test() {
            panic!("doc tests do not have an out dir");
        } else if unit.target.is_custom_build() {
            self.build_script_dir(unit)
        } else if unit.target.is_example() {
            self.layout(unit.kind).build_dir().examples().to_path_buf()
        } else if unit.artifact.is_true() {
            self.artifact_dir(unit)
        } else {
            self.deps_dir(unit).to_path_buf()
        }
    }

    /// Additional export directory from `--artifact-dir`.
    pub fn export_dir(&self) -> Option<PathBuf> {
        self.export_dir.clone()
    }

    /// Directory name to use for a package in the form `{NAME}/{HASH}`.
    ///
    /// Note that some units may share the same directory, so care should be
    /// taken in those cases!
    fn pkg_dir(&self, unit: &Unit) -> String {
        let seperator = match self.ws.gctx().cli_unstable().build_dir_new_layout {
            true => "/",
            false => "-",
        };
        let name = unit.pkg.package_id().name();
        let meta = self.metas[unit];
        if let Some(c_extra_filename) = meta.c_extra_filename() {
            format!("{}{}{}", name, seperator, c_extra_filename)
        } else {
            format!("{}{}{}", name, seperator, self.target_short_hash(unit))
        }
    }

    /// Returns the final artifact path for the host (`/…/target/debug`)
    pub fn host_dest(&self) -> &Path {
        self.host.artifact_dir().dest()
    }

    /// Returns the root of the build output tree for the host (`/…/build-dir`)
    pub fn host_build_root(&self) -> &Path {
        self.host.build_dir().root()
    }

    /// Returns the host `deps` directory path.
    pub fn host_deps(&self, unit: &Unit) -> PathBuf {
        let dir = self.pkg_dir(unit);
        self.host.build_dir().deps(&dir)
    }

    /// Returns the directories where Rust crate dependencies are found for the
    /// specified unit.
    pub fn deps_dir(&self, unit: &Unit) -> PathBuf {
        let dir = self.pkg_dir(unit);
        self.layout(unit.kind).build_dir().deps(&dir)
    }

    /// Directory where the fingerprint for the given unit should go.
    pub fn fingerprint_dir(&self, unit: &Unit) -> PathBuf {
        let dir = self.pkg_dir(unit);
        self.layout(unit.kind).build_dir().fingerprint(&dir)
    }

    /// Directory where incremental output for the given unit should go.
    pub fn incremental_dir(&self, unit: &Unit) -> &Path {
        self.layout(unit.kind).build_dir().incremental()
    }

    /// Directory where timing output should go.
    pub fn timings_dir(&self) -> &Path {
        self.host.artifact_dir().timings()
    }

    /// Returns the path for a file in the fingerprint directory.
    ///
    /// The "prefix" should be something to distinguish the file from other
    /// files in the fingerprint directory.
    pub fn fingerprint_file_path(&self, unit: &Unit, prefix: &str) -> PathBuf {
        // Different targets need to be distinguished in the
        let kind = unit.target.kind().description();
        let flavor = if unit.mode.is_any_test() {
            "test-"
        } else if unit.mode.is_doc() {
            "doc-"
        } else if unit.mode.is_run_custom_build() {
            "run-"
        } else {
            ""
        };
        let name = format!("{}{}{}-{}", prefix, flavor, kind, unit.target.name());
        self.fingerprint_dir(unit).join(name)
    }

    /// Path where compiler output is cached.
    pub fn message_cache_path(&self, unit: &Unit) -> PathBuf {
        self.fingerprint_file_path(unit, "output-")
    }

    /// Returns the directory where a compiled build script is stored.
    /// `/path/to/target/{debug,release}/build/PKG-HASH`
    pub fn build_script_dir(&self, unit: &Unit) -> PathBuf {
        assert!(unit.target.is_custom_build());
        assert!(!unit.mode.is_run_custom_build());
        assert!(self.metas.contains_key(unit));
        let dir = self.pkg_dir(unit);
        self.layout(CompileKind::Host)
            .build_dir()
            .build_script(&dir)
    }

    /// Returns the directory for compiled artifacts files.
    /// `/path/to/target/{debug,release}/deps/artifact/KIND/PKG-HASH`
    fn artifact_dir(&self, unit: &Unit) -> PathBuf {
        assert!(self.metas.contains_key(unit));
        assert!(unit.artifact.is_true());
        let dir = self.pkg_dir(unit);
        let kind = match unit.target.kind() {
            TargetKind::Bin => "bin",
            TargetKind::Lib(lib_kinds) => match lib_kinds.as_slice() {
                &[CrateType::Cdylib] => "cdylib",
                &[CrateType::Staticlib] => "staticlib",
                invalid => unreachable!(
                    "BUG: unexpected artifact library type(s): {:?} - these should have been split",
                    invalid
                ),
            },
            invalid => unreachable!(
                "BUG: {:?} are not supposed to be used as artifacts",
                invalid
            ),
        };
        self.layout(unit.kind)
            .build_dir()
            .artifact()
            .join(dir)
            .join(kind)
    }

    /// Returns the directory where information about running a build script
    /// is stored.
    /// `/path/to/target/{debug,release}/build/PKG-HASH`
    pub fn build_script_run_dir(&self, unit: &Unit) -> PathBuf {
        assert!(unit.target.is_custom_build());
        assert!(unit.mode.is_run_custom_build());
        let dir = self.pkg_dir(unit);
        self.layout(unit.kind)
            .build_dir()
            .build_script_execution(&dir)
    }

    /// Returns the "`OUT_DIR`" directory for running a build script.
    /// `/path/to/target/{debug,release}/build/PKG-HASH/out`
    pub fn build_script_out_dir(&self, unit: &Unit) -> PathBuf {
        self.build_script_run_dir(unit).join("out")
    }

    /// Returns the path to the executable binary for the given bin target.
    ///
    /// This should only to be used when a `Unit` is not available.
    pub fn bin_link_for_target(
        &self,
        target: &Target,
        kind: CompileKind,
        bcx: &BuildContext<'_, '_>,
    ) -> CargoResult<PathBuf> {
        assert!(target.is_bin());
        let dest = self.layout(kind).artifact_dir().dest();
        let info = bcx.target_data.info(kind);
        let (file_types, _) = info
            .rustc_outputs(
                CompileMode::Build,
                &TargetKind::Bin,
                bcx.target_data.short_name(&kind),
                bcx.gctx,
            )
            .expect("target must support `bin`");

        let file_type = file_types
            .iter()
            .find(|file_type| file_type.flavor == FileFlavor::Normal)
            .expect("target must support `bin`");

        Ok(dest.join(file_type.uplift_filename(target)))
    }

    /// Returns the filenames that the given unit will generate.
    ///
    /// Note: It is not guaranteed that all of the files will be generated.
    pub(super) fn outputs(
        &self,
        unit: &Unit,
        bcx: &BuildContext<'a, 'gctx>,
    ) -> CargoResult<Arc<Vec<OutputFile>>> {
        self.outputs[unit]
            .try_borrow_with(|| self.calc_outputs(unit, bcx))
            .map(Arc::clone)
    }

    /// Returns the path where the output for the given unit and `FileType`
    /// should be uplifted to.
    ///
    /// Returns `None` if the unit shouldn't be uplifted (for example, a
    /// dependent rlib).
    fn uplift_to(&self, unit: &Unit, file_type: &FileType, from_path: &Path) -> Option<PathBuf> {
        // Tests, check, doc, etc. should not be uplifted.
        if unit.mode != CompileMode::Build || file_type.flavor == FileFlavor::Rmeta {
            return None;
        }

        // Artifact dependencies are never uplifted.
        if unit.artifact.is_true() {
            return None;
        }

        // - Binaries: The user always wants to see these, even if they are
        //   implicitly built (for example for integration tests).
        // - dylibs: This ensures that the dynamic linker pulls in all the
        //   latest copies (even if the dylib was built from a previous cargo
        //   build). There are complex reasons for this, see #8139, #6167, #6162.
        // - Things directly requested from the command-line (the "roots").
        //   This one is a little questionable for rlibs (see #6131), but is
        //   historically how Cargo has operated. This is primarily useful to
        //   give the user access to staticlibs and cdylibs.
        if !unit.target.is_bin()
            && !unit.target.is_custom_build()
            && file_type.crate_type != Some(CrateType::Dylib)
            && !self.roots.contains(unit)
        {
            return None;
        }

        let filename = file_type.uplift_filename(&unit.target);
        let uplift_path = if unit.target.is_example() {
            // Examples live in their own little world.
            self.layout(unit.kind)
                .artifact_dir()
                .examples()
                .join(filename)
        } else if unit.target.is_custom_build() {
            self.build_script_dir(unit).join(filename)
        } else {
            self.layout(unit.kind).artifact_dir().dest().join(filename)
        };
        if from_path == uplift_path {
            // This can happen with things like examples that reside in the
            // same directory, do not have a metadata hash (like on Windows),
            // and do not have hyphens.
            return None;
        }
        Some(uplift_path)
    }

    /// Calculates the filenames that the given unit will generate.
    /// Should use [`CompilationFiles::outputs`] instead
    /// as it caches the result of this function.
    fn calc_outputs(
        &self,
        unit: &Unit,
        bcx: &BuildContext<'a, 'gctx>,
    ) -> CargoResult<Arc<Vec<OutputFile>>> {
        let ret = match unit.mode {
            _ if unit.skip_non_compile_time_dep => {
                // This skips compilations so no outputs
                vec![]
            }
            CompileMode::Doc => {
                let path = if bcx.build_config.intent.wants_doc_json_output() {
                    self.out_dir(unit)
                        .join(format!("{}.json", unit.target.crate_name()))
                } else {
                    self.out_dir(unit)
                        .join(unit.target.crate_name())
                        .join("index.html")
                };

                vec![OutputFile {
                    path,
                    hardlink: None,
                    export_path: None,
                    flavor: FileFlavor::Normal,
                }]
            }
            CompileMode::RunCustomBuild => {
                // At this time, this code path does not handle build script
                // outputs.
                vec![]
            }
            CompileMode::Doctest => {
                // Doctests are built in a temporary directory and then
                // deleted. There is the `--persist-doctests` unstable flag,
                // but Cargo does not know about that.
                vec![]
            }
            CompileMode::Docscrape => {
                // The file name needs to be stable across Cargo sessions.
                // This originally used unit.buildkey(), but that isn't stable,
                // so we use metadata instead (prefixed with name for debugging).
                let file_name = format!(
                    "{}-{}.examples",
                    unit.pkg.name(),
                    self.metadata(unit).unit_id()
                );
                let path = self.deps_dir(unit).join(file_name);
                vec![OutputFile {
                    path,
                    hardlink: None,
                    export_path: None,
                    flavor: FileFlavor::Normal,
                }]
            }
            CompileMode::Test | CompileMode::Build | CompileMode::Check { .. } => {
                let mut outputs = self.calc_outputs_rustc(unit, bcx)?;
                if bcx.build_config.sbom && bcx.gctx.cli_unstable().sbom {
                    let sbom_files: Vec<_> = outputs
                        .iter()
                        .filter(|o| matches!(o.flavor, FileFlavor::Normal | FileFlavor::Linkable))
                        .map(|output| OutputFile {
                            path: Self::append_sbom_suffix(&output.path),
                            hardlink: output.hardlink.as_ref().map(Self::append_sbom_suffix),
                            export_path: output.export_path.as_ref().map(Self::append_sbom_suffix),
                            flavor: FileFlavor::Sbom,
                        })
                        .collect();
                    outputs.extend(sbom_files.into_iter());
                }
                outputs
            }
        };
        debug!("Target filenames: {:?}", ret);

        Ok(Arc::new(ret))
    }

    /// Append the SBOM suffix to the file name.
    fn append_sbom_suffix(link: &PathBuf) -> PathBuf {
        const SBOM_FILE_EXTENSION: &str = ".cargo-sbom.json";
        let mut link_buf = link.clone().into_os_string();
        link_buf.push(SBOM_FILE_EXTENSION);
        PathBuf::from(link_buf)
    }

    /// Computes the actual, full pathnames for all the files generated by rustc.
    ///
    /// The `OutputFile` also contains the paths where those files should be
    /// "uplifted" to.
    fn calc_outputs_rustc(
        &self,
        unit: &Unit,
        bcx: &BuildContext<'a, 'gctx>,
    ) -> CargoResult<Vec<OutputFile>> {
        let out_dir = self.out_dir(unit);

        let info = bcx.target_data.info(unit.kind);
        let triple = bcx.target_data.short_name(&unit.kind);
        let (file_types, unsupported) =
            info.rustc_outputs(unit.mode, unit.target.kind(), triple, bcx.gctx)?;
        if file_types.is_empty() {
            if !unsupported.is_empty() {
                let unsupported_strs: Vec<_> = unsupported.iter().map(|ct| ct.as_str()).collect();
                anyhow::bail!(
                    "cannot produce {} for `{}` as the target `{}` \
                     does not support these crate types",
                    unsupported_strs.join(", "),
                    unit.pkg,
                    triple,
                )
            }
            anyhow::bail!(
                "cannot compile `{}` as the target `{}` does not \
                 support any of the output crate types",
                unit.pkg,
                triple,
            );
        }

        // Convert FileType to OutputFile.
        let mut outputs = Vec::new();
        for file_type in file_types {
            let meta = self.metas[unit];
            let meta_opt = meta.c_extra_filename().map(|h| h.to_string());
            let path = out_dir.join(file_type.output_filename(&unit.target, meta_opt.as_deref()));

            // If, the `different_binary_name` feature is enabled, the name of the hardlink will
            // be the name of the binary provided by the user in `Cargo.toml`.
            let hardlink = self.uplift_to(unit, &file_type, &path);
            let export_path = if unit.target.is_custom_build() {
                None
            } else {
                self.export_dir.as_ref().and_then(|export_dir| {
                    hardlink
                        .as_ref()
                        .map(|hardlink| export_dir.join(hardlink.file_name().unwrap()))
                })
            };
            outputs.push(OutputFile {
                path,
                hardlink,
                export_path,
                flavor: file_type.flavor,
            });
        }
        Ok(outputs)
    }
}

/// Gets the metadata hash for the given [`Unit`].
///
/// When a metadata hash doesn't exist for the given unit,
/// this calls itself recursively to compute metadata hashes of all its dependencies.
/// See [`compute_metadata`] for how a single metadata hash is computed.
fn metadata_of<'a>(
    unit: &Unit,
    build_runner: &BuildRunner<'_, '_>,
    metas: &'a mut HashMap<Unit, Metadata>,
) -> &'a Metadata {
    if !metas.contains_key(unit) {
        let meta = compute_metadata(unit, build_runner, metas);
        metas.insert(unit.clone(), meta);
        for dep in build_runner.unit_deps(unit) {
            metadata_of(&dep.unit, build_runner, metas);
        }
    }
    &metas[unit]
}

/// Computes the metadata hash for the given [`Unit`].
fn compute_metadata(
    unit: &Unit,
    build_runner: &BuildRunner<'_, '_>,
    metas: &mut HashMap<Unit, Metadata>,
) -> Metadata {
    let bcx = &build_runner.bcx;
    let deps_metadata = build_runner
        .unit_deps(unit)
        .iter()
        .map(|dep| *metadata_of(&dep.unit, build_runner, metas))
        .collect::<Vec<_>>();
    let use_extra_filename = use_extra_filename(bcx, unit);

    let mut shared_hasher = StableHasher::new();

    METADATA_VERSION.hash(&mut shared_hasher);

    let ws_root = if unit.is_std {
        // SourceId for stdlib crates is an absolute path inside the sysroot.
        // Pass the sysroot as workspace root so that we hash a relative path.
        // This avoids the metadata hash changing depending on where the user installed rustc.
        &bcx.target_data.get_info(unit.kind).unwrap().sysroot
    } else {
        bcx.ws.root()
    };

    // Unique metadata per (name, source, version) triple. This'll allow us
    // to pull crates from anywhere without worrying about conflicts.
    unit.pkg
        .package_id()
        .stable_hash(ws_root)
        .hash(&mut shared_hasher);

    // Also mix in enabled features to our metadata. This'll ensure that
    // when changing feature sets each lib is separately cached.
    unit.features.hash(&mut shared_hasher);

    // Throw in the profile we're compiling with. This helps caching
    // `panic=abort` and `panic=unwind` artifacts, additionally with various
    // settings like debuginfo and whatnot.
    unit.profile.hash(&mut shared_hasher);
    unit.mode.hash(&mut shared_hasher);
    build_runner.lto[unit].hash(&mut shared_hasher);

    // Artifacts compiled for the host should have a different
    // metadata piece than those compiled for the target, so make sure
    // we throw in the unit's `kind` as well.  Use `fingerprint_hash`
    // so that the StableHash doesn't change based on the pathnames
    // of the custom target JSON spec files.
    unit.kind.fingerprint_hash().hash(&mut shared_hasher);

    // Finally throw in the target name/kind. This ensures that concurrent
    // compiles of targets in the same crate don't collide.
    unit.target.name().hash(&mut shared_hasher);
    unit.target.kind().hash(&mut shared_hasher);

    hash_rustc_version(bcx, &mut shared_hasher, unit);

    if build_runner.bcx.ws.is_member(&unit.pkg) {
        // This is primarily here for clippy. This ensures that the clippy
        // artifacts are separate from the `check` ones.
        if let Some(path) = &build_runner.bcx.rustc().workspace_wrapper {
            path.hash(&mut shared_hasher);
        }
    }

    // Seed the contents of `__CARGO_DEFAULT_LIB_METADATA` to the hasher if present.
    // This should be the release channel, to get a different hash for each channel.
    if let Ok(ref channel) = build_runner
        .bcx
        .gctx
        .get_env("__CARGO_DEFAULT_LIB_METADATA")
    {
        channel.hash(&mut shared_hasher);
    }

    // std units need to be kept separate from user dependencies. std crates
    // are differentiated in the Unit with `is_std` (for things like
    // `-Zforce-unstable-if-unmarked`), so they are always built separately.
    // This isn't strictly necessary for build dependencies which probably
    // don't need unstable support. A future experiment might be to set
    // `is_std` to false for build dependencies so that they can be shared
    // with user dependencies.
    unit.is_std.hash(&mut shared_hasher);

    // While we don't hash RUSTFLAGS because it may contain absolute paths that
    // hurts reproducibility, we track whether a unit's RUSTFLAGS is from host
    // config, so that we can generate a different metadata hash for runtime
    // and compile-time units.
    //
    // HACK: This is a temporary hack for fixing rust-lang/cargo#14253
    // Need to find a long-term solution to replace this fragile workaround.
    // See https://github.com/rust-lang/cargo/pull/14432#discussion_r1725065350
    if unit.kind.is_host() && !bcx.gctx.target_applies_to_host().unwrap_or_default() {
        let host_info = bcx.target_data.info(CompileKind::Host);
        let target_configs_are_different = unit.rustflags != host_info.rustflags
            || unit.rustdocflags != host_info.rustdocflags
            || bcx
                .target_data
                .target_config(CompileKind::Host)
                .links_overrides
                != unit.links_overrides;
        target_configs_are_different.hash(&mut shared_hasher);
    }

    let mut c_metadata_hasher = shared_hasher.clone();
    // Mix in the target-metadata of all the dependencies of this target.
    let mut dep_c_metadata_hashes = deps_metadata
        .iter()
        .map(|m| m.c_metadata)
        .collect::<Vec<_>>();
    dep_c_metadata_hashes.sort();
    dep_c_metadata_hashes.hash(&mut c_metadata_hasher);

    let mut c_extra_filename_hasher = shared_hasher.clone();
    // Mix in the target-metadata of all the dependencies of this target.
    let mut dep_c_extra_filename_hashes = deps_metadata
        .iter()
        .map(|m| m.c_extra_filename)
        .collect::<Vec<_>>();
    dep_c_extra_filename_hashes.sort();
    dep_c_extra_filename_hashes.hash(&mut c_extra_filename_hasher);
    // Avoid trashing the caches on RUSTFLAGS changing via `c_extra_filename`
    //
    // Limited to `c_extra_filename` to help with reproducible build / PGO issues.
    let default = Vec::new();
    let extra_args = build_runner.bcx.extra_args_for(unit).unwrap_or(&default);
    if !has_remap_path_prefix(&extra_args) {
        extra_args.hash(&mut c_extra_filename_hasher);
    }
    if unit.mode.is_doc() || unit.mode.is_doc_scrape() {
        if !has_remap_path_prefix(&unit.rustdocflags) {
            unit.rustdocflags.hash(&mut c_extra_filename_hasher);
        }
    } else {
        if !has_remap_path_prefix(&unit.rustflags) {
            unit.rustflags.hash(&mut c_extra_filename_hasher);
        }
    }

    let c_metadata = UnitHash(Hasher::finish(&c_metadata_hasher));
    let c_extra_filename = UnitHash(Hasher::finish(&c_extra_filename_hasher));
    let unit_id = c_extra_filename;

    let c_extra_filename = use_extra_filename.then_some(c_extra_filename);

    Metadata {
        unit_id,
        c_metadata,
        c_extra_filename,
    }
}

/// HACK: Detect the *potential* presence of `--remap-path-prefix`
///
/// As CLI parsing is contextual and dependent on the CLI definition to understand the context, we
/// can't say for sure whether `--remap-path-prefix` is present, so we guess if anything looks like
/// it.
/// If we could, we'd strip it out for hashing.
/// Instead, we use this to avoid hashing rustflags if it might be present to avoid the risk of taking
/// a flag that is trying to make things reproducible and making things less reproducible by the
/// `-Cextra-filename` showing up in the rlib, even with `split-debuginfo`.
fn has_remap_path_prefix(args: &[String]) -> bool {
    args.iter()
        .any(|s| s.starts_with("--remap-path-prefix=") || s == "--remap-path-prefix")
}

/// Hash the version of rustc being used during the build process.
fn hash_rustc_version(bcx: &BuildContext<'_, '_>, hasher: &mut StableHasher, unit: &Unit) {
    let vers = &bcx.rustc().version;
    if vers.pre.is_empty() || bcx.gctx.cli_unstable().separate_nightlies {
        // For stable, keep the artifacts separate. This helps if someone is
        // testing multiple versions, to avoid recompiles. Note though that for
        // cross-compiled builds the `host:` line of `verbose_version` is
        // omitted since rustc should produce the same output for each target
        // regardless of the host.
        for line in bcx.rustc().verbose_version.lines() {
            if unit.kind.is_host() || !line.starts_with("host: ") {
                line.hash(hasher);
            }
        }
        return;
    }
    // On "nightly"/"beta"/"dev"/etc, keep each "channel" separate. Don't hash
    // the date/git information, so that whenever someone updates "nightly",
    // they won't have a bunch of stale artifacts in the target directory.
    //
    // This assumes that the first segment is the important bit ("nightly",
    // "beta", "dev", etc.). Skip other parts like the `.3` in `-beta.3`.
    vers.pre.split('.').next().hash(hasher);
    // Keep "host" since some people switch hosts to implicitly change
    // targets, (like gnu vs musl or gnu vs msvc). In the future, we may want
    // to consider hashing `unit.kind.short_name()` instead.
    if unit.kind.is_host() {
        bcx.rustc().host.hash(hasher);
    }
    // None of the other lines are important. Currently they are:
    // binary: rustc  <-- or "rustdoc"
    // commit-hash: 38114ff16e7856f98b2b4be7ab4cd29b38bed59a
    // commit-date: 2020-03-21
    // host: x86_64-apple-darwin
    // release: 1.44.0-nightly
    // LLVM version: 9.0
    //
    // The backend version ("LLVM version") might become more relevant in
    // the future when cranelift sees more use, and people want to switch
    // between different backends without recompiling.
}

/// Returns whether or not this unit should use a hash in the filename to make it unique.
fn use_extra_filename(bcx: &BuildContext<'_, '_>, unit: &Unit) -> bool {
    if unit.mode.is_doc_test() || unit.mode.is_doc() {
        // Doc tests do not have metadata.
        return false;
    }
    if unit.mode.is_any_test() || unit.mode.is_check() {
        // These always use metadata.
        return true;
    }
    // No metadata in these cases:
    //
    // - dylibs:
    //   - if any dylib names are encoded in executables, so they can't be renamed.
    //   - TODO: Maybe use `-install-name` on macOS or `-soname` on other UNIX systems
    //     to specify the dylib name to be used by the linker instead of the filename.
    // - Windows MSVC executables: The path to the PDB is embedded in the
    //   executable, and we don't want the PDB path to include the hash in it.
    // - wasm32-unknown-emscripten executables: When using emscripten, the path to the
    //   .wasm file is embedded in the .js file, so we don't want the hash in there.
    //
    // This is only done for local packages, as we don't expect to export
    // dependencies.
    //
    // The __CARGO_DEFAULT_LIB_METADATA env var is used to override this to
    // force metadata in the hash. This is only used for building libstd. For
    // example, if libstd is placed in a common location, we don't want a file
    // named /usr/lib/libstd.so which could conflict with other rustc
    // installs. In addition it prevents accidentally loading a libstd of a
    // different compiler at runtime.
    // See https://github.com/rust-lang/cargo/issues/3005
    let short_name = bcx.target_data.short_name(&unit.kind);
    if (unit.target.is_dylib()
        || unit.target.is_cdylib()
        || (unit.target.is_executable() && short_name == "wasm32-unknown-emscripten")
        || (unit.target.is_executable() && short_name.contains("msvc")))
        && unit.pkg.package_id().source_id().is_path()
        && bcx.gctx.get_env("__CARGO_DEFAULT_LIB_METADATA").is_err()
    {
        return false;
    }
    true
}
