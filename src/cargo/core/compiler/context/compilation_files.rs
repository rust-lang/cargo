use std::collections::HashMap;
use std::env;
use std::fmt;
use std::hash::{Hash, Hasher, SipHasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lazycell::LazyCell;
use log::info;

use super::{BuildContext, Context, FileFlavor, Kind, Layout};
use crate::core::compiler::Unit;
use crate::core::{TargetKind, Workspace};
use crate::util::{self, CargoResult};

/// The `Metadata` is a hash used to make unique file names for each unit in a build.
/// For example:
/// - A project may depend on crate `A` and crate `B`, so the package name must be in the file name.
/// - Similarly a project may depend on two versions of `A`, so the version must be in the file name.
/// In general this must include all things that need to be distinguished in different parts of
/// the same build. This is absolutely required or we override things before
/// we get chance to use them.
///
/// We use a hash because it is an easy way to guarantee
/// that all the inputs can be converted to a valid path.
///
/// This also acts as the main layer of caching provided by Cargo.
/// For example, we want to cache `cargo build` and `cargo doc` separately, so that running one
/// does not invalidate the artifacts for the other. We do this by including `CompileMode` in the
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
#[derive(Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Metadata(u64);

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

pub struct CompilationFiles<'a, 'cfg: 'a> {
    /// The target directory layout for the host (and target if it is the same as host).
    pub(super) host: Layout,
    /// The target directory layout for the target (if different from then host).
    pub(super) target: Option<Layout>,
    /// Additional directory to include a copy of the outputs.
    export_dir: Option<PathBuf>,
    /// The root targets requested by the user on the command line (does not
    /// include dependencies).
    roots: Vec<Unit<'a>>,
    ws: &'a Workspace<'cfg>,
    metas: HashMap<Unit<'a>, Option<Metadata>>,
    /// For each Unit, a list all files produced.
    outputs: HashMap<Unit<'a>, LazyCell<Arc<Vec<OutputFile>>>>,
}

#[derive(Debug)]
pub struct OutputFile {
    /// Absolute path to the file that will be produced by the build process.
    pub path: PathBuf,
    /// If it should be linked into `target`, and what it should be called
    /// (e.g., without metadata).
    pub hardlink: Option<PathBuf>,
    /// If `--out-dir` is specified, the absolute path to the exported file.
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

impl<'a, 'cfg: 'a> CompilationFiles<'a, 'cfg> {
    pub(super) fn new(
        roots: &[Unit<'a>],
        host: Layout,
        target: Option<Layout>,
        export_dir: Option<PathBuf>,
        ws: &'a Workspace<'cfg>,
        cx: &Context<'a, 'cfg>,
    ) -> CompilationFiles<'a, 'cfg> {
        let mut metas = HashMap::new();
        for unit in roots {
            metadata_of(unit, cx, &mut metas);
        }
        let outputs = metas
            .keys()
            .cloned()
            .map(|unit| (unit, LazyCell::new()))
            .collect();
        CompilationFiles {
            ws,
            host,
            target,
            export_dir,
            roots: roots.to_vec(),
            metas,
            outputs,
        }
    }

    /// Returns the appropriate directory layout for either a plugin or not.
    pub fn layout(&self, kind: Kind) -> &Layout {
        match kind {
            Kind::Host => &self.host,
            Kind::Target => self.target.as_ref().unwrap_or(&self.host),
        }
    }

    /// Gets the metadata for a target in a specific profile.
    /// We build to the path `"{filename}-{target_metadata}"`.
    /// We use a linking step to link/copy to a predictable filename
    /// like `target/debug/libfoo.{a,so,rlib}` and such.
    pub fn metadata(&self, unit: &Unit<'a>) -> Option<Metadata> {
        self.metas[unit].clone()
    }

    /// Gets the short hash based only on the `PackageId`.
    /// Used for the metadata when `target_metadata` returns `None`.
    pub fn target_short_hash(&self, unit: &Unit<'_>) -> String {
        let hashable = unit.pkg.package_id().stable_hash(self.ws.root());
        util::short_hash(&hashable)
    }

    /// Returns the appropriate output directory for the specified package and
    /// target.
    pub fn out_dir(&self, unit: &Unit<'a>) -> PathBuf {
        if unit.mode.is_doc() {
            self.layout(unit.kind).root().parent().unwrap().join("doc")
        } else if unit.target.is_custom_build() {
            self.build_script_dir(unit)
        } else if unit.target.is_example() {
            self.layout(unit.kind).examples().to_path_buf()
        } else {
            self.deps_dir(unit).to_path_buf()
        }
    }

    pub fn export_dir(&self) -> Option<PathBuf> {
        self.export_dir.clone()
    }

    pub fn pkg_dir(&self, unit: &Unit<'a>) -> String {
        let name = unit.pkg.package_id().name();
        match self.metas[unit] {
            Some(ref meta) => format!("{}-{}", name, meta),
            None => format!("{}-{}", name, self.target_short_hash(unit)),
        }
    }

    /// Returns the root of the build output tree for the target
    pub fn target_root(&self) -> &Path {
        self.target.as_ref().unwrap_or(&self.host).dest()
    }

    /// Returns the root of the build output tree for the host
    pub fn host_root(&self) -> &Path {
        self.host.dest()
    }

    pub fn host_deps(&self) -> &Path {
        self.host.deps()
    }

    /// Returns the directories where Rust crate dependencies are found for the
    /// specified unit.
    pub fn deps_dir(&self, unit: &Unit<'_>) -> &Path {
        self.layout(unit.kind).deps()
    }

    pub fn fingerprint_dir(&self, unit: &Unit<'a>) -> PathBuf {
        let dir = self.pkg_dir(unit);
        self.layout(unit.kind).fingerprint().join(dir)
    }

    /// Returns the directory where a compiled build script is stored.
    /// `/path/to/target/{debug,release}/build/PKG-HASH`
    pub fn build_script_dir(&self, unit: &Unit<'a>) -> PathBuf {
        assert!(unit.target.is_custom_build());
        assert!(!unit.mode.is_run_custom_build());
        let dir = self.pkg_dir(unit);
        self.layout(Kind::Host).build().join(dir)
    }

    /// Returns the directory where information about running a build script
    /// is stored.
    /// `/path/to/target/{debug,release}/build/PKG-HASH`
    pub fn build_script_run_dir(&self, unit: &Unit<'a>) -> PathBuf {
        assert!(unit.target.is_custom_build());
        assert!(unit.mode.is_run_custom_build());
        let dir = self.pkg_dir(unit);
        self.layout(unit.kind).build().join(dir)
    }

    /// Returns the "OUT_DIR" directory for running a build script.
    /// `/path/to/target/{debug,release}/build/PKG-HASH/out`
    pub fn build_script_out_dir(&self, unit: &Unit<'a>) -> PathBuf {
        self.build_script_run_dir(unit).join("out")
    }

    /// Returns the file stem for a given target/profile combo (with metadata).
    pub fn file_stem(&self, unit: &Unit<'a>) -> String {
        match self.metas[unit] {
            Some(ref metadata) => format!("{}-{}", unit.target.crate_name(), metadata),
            None => self.bin_stem(unit),
        }
    }

    pub(super) fn outputs(
        &self,
        unit: &Unit<'a>,
        bcx: &BuildContext<'a, 'cfg>,
    ) -> CargoResult<Arc<Vec<OutputFile>>> {
        self.outputs[unit]
            .try_borrow_with(|| self.calc_outputs(unit, bcx))
            .map(Arc::clone)
    }

    /// Returns the bin stem for a given target (without metadata).
    fn bin_stem(&self, unit: &Unit<'_>) -> String {
        if unit.target.allows_underscores() {
            unit.target.name().to_string()
        } else {
            unit.target.crate_name()
        }
    }

    /// Returns a tuple with the directory and name of the hard link we expect
    /// our target to be copied to. Eg, file_stem may be out_dir/deps/foo-abcdef
    /// and link_stem would be out_dir/foo
    /// This function returns it in two parts so the caller can add prefix/suffix
    /// to filename separately.
    ///
    /// Returns an `Option` because in some cases we don't want to link
    /// (eg a dependent lib).
    fn link_stem(&self, unit: &Unit<'a>) -> Option<(PathBuf, String)> {
        let out_dir = self.out_dir(unit);
        let bin_stem = self.bin_stem(unit);
        let file_stem = self.file_stem(unit);

        // We currently only lift files up from the `deps` directory. If
        // it was compiled into something like `example/` or `doc/` then
        // we don't want to link it up.
        if out_dir.ends_with("deps") {
            // Don't lift up library dependencies.
            if unit.target.is_bin() || self.roots.contains(unit) {
                Some((
                    out_dir.parent().unwrap().to_owned(),
                    if unit.mode.is_any_test() {
                        file_stem
                    } else {
                        bin_stem
                    },
                ))
            } else {
                None
            }
        } else if bin_stem == file_stem {
            None
        } else if out_dir.ends_with("examples") || out_dir.parent().unwrap().ends_with("build") {
            Some((out_dir, bin_stem))
        } else {
            None
        }
    }

    fn calc_outputs(
        &self,
        unit: &Unit<'a>,
        bcx: &BuildContext<'a, 'cfg>,
    ) -> CargoResult<Arc<Vec<OutputFile>>> {
        let out_dir = self.out_dir(unit);
        let file_stem = self.file_stem(unit);
        let link_stem = self.link_stem(unit);
        let info = if unit.kind == Kind::Host {
            &bcx.host_info
        } else {
            &bcx.target_info
        };

        let mut ret = Vec::new();
        let mut unsupported = Vec::new();
        {
            if unit.mode.is_check() {
                // This may be confusing. rustc outputs a file named `lib*.rmeta`
                // for both libraries and binaries.
                let path = out_dir.join(format!("lib{}.rmeta", file_stem));
                ret.push(OutputFile {
                    path,
                    hardlink: None,
                    export_path: None,
                    flavor: FileFlavor::Linkable { rmeta: false },
                });
            } else {
                let mut add = |crate_type: &str, flavor: FileFlavor| -> CargoResult<()> {
                    let crate_type = if crate_type == "lib" {
                        "rlib"
                    } else {
                        crate_type
                    };
                    let file_types = info.file_types(
                        crate_type,
                        flavor,
                        unit.target.kind(),
                        bcx.target_triple(),
                    )?;

                    match file_types {
                        Some(types) => {
                            for file_type in types {
                                let path = out_dir.join(file_type.filename(&file_stem));
                                let hardlink = link_stem
                                    .as_ref()
                                    .map(|&(ref ld, ref ls)| ld.join(file_type.filename(ls)));
                                let export_path = if unit.target.is_custom_build() {
                                    None
                                } else {
                                    self.export_dir.as_ref().and_then(|export_dir| {
                                        hardlink.as_ref().and_then(|hardlink| {
                                            Some(export_dir.join(hardlink.file_name().unwrap()))
                                        })
                                    })
                                };
                                ret.push(OutputFile {
                                    path,
                                    hardlink,
                                    export_path,
                                    flavor: file_type.flavor,
                                });
                            }
                        }
                        // Not supported; don't worry about it.
                        None => {
                            unsupported.push(crate_type.to_string());
                        }
                    }
                    Ok(())
                };
                // info!("{:?}", unit);
                match *unit.target.kind() {
                    TargetKind::Bin
                    | TargetKind::CustomBuild
                    | TargetKind::ExampleBin
                    | TargetKind::Bench
                    | TargetKind::Test => {
                        add("bin", FileFlavor::Normal)?;
                    }
                    TargetKind::Lib(..) | TargetKind::ExampleLib(..) if unit.mode.is_any_test() => {
                        add("bin", FileFlavor::Normal)?;
                    }
                    TargetKind::ExampleLib(ref kinds) | TargetKind::Lib(ref kinds) => {
                        for kind in kinds {
                            add(
                                kind.crate_type(),
                                if kind.linkable() {
                                    FileFlavor::Linkable { rmeta: false }
                                } else {
                                    FileFlavor::Normal
                                },
                            )?;
                        }
                        let path = out_dir.join(format!("lib{}.rmeta", file_stem));
                        if !unit.target.requires_upstream_objects() {
                            ret.push(OutputFile {
                                path,
                                hardlink: None,
                                export_path: None,
                                flavor: FileFlavor::Linkable { rmeta: true },
                            });
                        }
                    }
                }
            }
        }
        if ret.is_empty() {
            if !unsupported.is_empty() {
                failure::bail!(
                    "cannot produce {} for `{}` as the target `{}` \
                     does not support these crate types",
                    unsupported.join(", "),
                    unit.pkg,
                    bcx.target_triple()
                )
            }
            failure::bail!(
                "cannot compile `{}` as the target `{}` does not \
                 support any of the output crate types",
                unit.pkg,
                bcx.target_triple()
            );
        }
        info!("Target filenames: {:?}", ret);

        Ok(Arc::new(ret))
    }
}

fn metadata_of<'a, 'cfg>(
    unit: &Unit<'a>,
    cx: &Context<'a, 'cfg>,
    metas: &mut HashMap<Unit<'a>, Option<Metadata>>,
) -> Option<Metadata> {
    if !metas.contains_key(unit) {
        let meta = compute_metadata(unit, cx, metas);
        metas.insert(*unit, meta);
        for unit in cx.dep_targets(unit) {
            metadata_of(&unit, cx, metas);
        }
    }
    metas[unit].clone()
}

fn compute_metadata<'a, 'cfg>(
    unit: &Unit<'a>,
    cx: &Context<'a, 'cfg>,
    metas: &mut HashMap<Unit<'a>, Option<Metadata>>,
) -> Option<Metadata> {
    // No metadata for dylibs because of a couple issues:
    // - macOS encodes the dylib name in the executable,
    // - Windows rustc multiple files of which we can't easily link all of them.
    //
    // No metadata for bin because of an issue:
    // - wasm32 rustc/emcc encodes the `.wasm` name in the `.js` (rust-lang/cargo#4535).
    //
    // Two exceptions:
    // 1) Upstream dependencies (we aren't exporting + need to resolve name conflict),
    // 2) `__CARGO_DEFAULT_LIB_METADATA` env var.
    //
    // Note, however, that the compiler's build system at least wants
    // path dependencies (eg libstd) to have hashes in filenames. To account for
    // that we have an extra hack here which reads the
    // `__CARGO_DEFAULT_LIB_METADATA` environment variable and creates a
    // hash in the filename if that's present.
    //
    // This environment variable should not be relied on! It's
    // just here for rustbuild. We need a more principled method
    // doing this eventually.
    let bcx = &cx.bcx;
    let __cargo_default_lib_metadata = env::var("__CARGO_DEFAULT_LIB_METADATA");
    if !(unit.mode.is_any_test() || unit.mode.is_check())
        && (unit.target.is_dylib()
            || unit.target.is_cdylib()
            || (unit.target.is_executable() && bcx.target_triple().starts_with("wasm32-")))
        && unit.pkg.package_id().source_id().is_path()
        && __cargo_default_lib_metadata.is_err()
    {
        return None;
    }

    let mut hasher = SipHasher::new_with_keys(0, 0);

    // This is a generic version number that can be changed to make
    // backwards-incompatible changes to any file structures in the output
    // directory. For example, the fingerprint files or the build-script
    // output files. Normally cargo updates ship with rustc updates which will
    // cause a new hash due to the rustc version changing, but this allows
    // cargo to be extra careful to deal with different versions of cargo that
    // use the same rustc version.
    1.hash(&mut hasher);

    // Unique metadata per (name, source, version) triple. This'll allow us
    // to pull crates from anywhere without worrying about conflicts.
    unit.pkg
        .package_id()
        .stable_hash(bcx.ws.root())
        .hash(&mut hasher);

    // Also mix in enabled features to our metadata. This'll ensure that
    // when changing feature sets each lib is separately cached.
    bcx.resolve
        .features_sorted(unit.pkg.package_id())
        .hash(&mut hasher);

    // Mix in the target-metadata of all the dependencies of this target.
    {
        let mut deps_metadata = cx
            .dep_targets(unit)
            .iter()
            .map(|dep| metadata_of(dep, cx, metas))
            .collect::<Vec<_>>();
        deps_metadata.sort();
        deps_metadata.hash(&mut hasher);
    }

    // Throw in the profile we're compiling with. This helps caching
    // `panic=abort` and `panic=unwind` artifacts, additionally with various
    // settings like debuginfo and whatnot.
    unit.profile.hash(&mut hasher);
    unit.mode.hash(&mut hasher);
    if let Some(args) = bcx.extra_args_for(unit) {
        args.hash(&mut hasher);
    }

    // Throw in the rustflags we're compiling with.
    // This helps when the target directory is a shared cache for projects with different cargo configs,
    // or if the user is experimenting with different rustflags manually.
    if unit.mode.is_doc() {
        cx.bcx.rustdocflags_args(unit).hash(&mut hasher);
    } else {
        cx.bcx.rustflags_args(unit).hash(&mut hasher);
    }

    // Artifacts compiled for the host should have a different metadata
    // piece than those compiled for the target, so make sure we throw in
    // the unit's `kind` as well
    unit.kind.hash(&mut hasher);

    // Finally throw in the target name/kind. This ensures that concurrent
    // compiles of targets in the same crate don't collide.
    unit.target.name().hash(&mut hasher);
    unit.target.kind().hash(&mut hasher);

    bcx.rustc.verbose_version.hash(&mut hasher);

    // Seed the contents of `__CARGO_DEFAULT_LIB_METADATA` to the hasher if present.
    // This should be the release channel, to get a different hash for each channel.
    if let Ok(ref channel) = __cargo_default_lib_metadata {
        channel.hash(&mut hasher);
    }
    Some(Metadata(hasher.finish()))
}
