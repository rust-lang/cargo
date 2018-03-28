use std::collections::hash_map::{Entry, HashMap};
use std::env;
use std::fmt;
use std::hash::{Hash, Hasher, SipHasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lazycell::LazyCell;

use core::{TargetKind, Workspace};
use ops::cargo_rustc::layout::Layout;
use ops::cargo_rustc::TargetFileType;
use ops::{Context, Kind, Unit};
use util::{self, CargoResult};

#[derive(Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Metadata(u64);

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

pub struct CompilationFiles<'a, 'cfg: 'a> {
    /// The target directory layout for the host (and target if it is the same as host)
    pub(super) host: Layout,
    /// The target directory layout for the target (if different from then host)
    pub(super) target: Option<Layout>,
    ws: &'a Workspace<'cfg>,
    metas: HashMap<Unit<'a>, Option<Metadata>>,
    /// For each Unit, a list all files produced as a triple of
    ///
    ///  - File name that will be produced by the build process (in `deps`)
    ///  - If it should be linked into `target`, and what it should be called (e.g. without
    ///    metadata).
    ///  - Type of the file (library / debug symbol / else)
    outputs: HashMap<Unit<'a>, LazyCell<Arc<Vec<(PathBuf, Option<PathBuf>, TargetFileType)>>>>,
}

impl<'a, 'cfg: 'a> CompilationFiles<'a, 'cfg> {
    pub(super) fn new(
        roots: &[Unit<'a>],
        host: Layout,
        target: Option<Layout>,
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

    /// Get the metadata for a target in a specific profile
    /// We build to the path: "{filename}-{target_metadata}"
    /// We use a linking step to link/copy to a predictable filename
    /// like `target/debug/libfoo.{a,so,rlib}` and such.
    pub fn metadata(&self, unit: &Unit<'a>) -> Option<Metadata> {
        self.metas[unit].clone()
    }

    /// Get the short hash based only on the PackageId
    /// Used for the metadata when target_metadata returns None
    pub fn target_short_hash(&self, unit: &Unit) -> String {
        let hashable = unit.pkg.package_id().stable_hash(self.ws.root());
        util::short_hash(&hashable)
    }

    /// Returns the appropriate output directory for the specified package and
    /// target.
    pub fn out_dir(&self, unit: &Unit<'a>) -> PathBuf {
        if unit.profile.doc {
            self.layout(unit.kind).root().parent().unwrap().join("doc")
        } else if unit.target.is_custom_build() {
            self.build_script_dir(unit)
        } else if unit.target.is_example() {
            self.layout(unit.kind).examples().to_path_buf()
        } else {
            self.deps_dir(unit).to_path_buf()
        }
    }

    pub fn pkg_dir(&self, unit: &Unit<'a>) -> String {
        let name = unit.pkg.package_id().name();
        match self.metas[unit] {
            Some(ref meta) => format!("{}-{}", name, meta),
            None => format!("{}-{}", name, self.target_short_hash(unit)),
        }
    }

    /// Return the root of the build output tree
    pub fn target_root(&self) -> &Path {
        self.host.dest()
    }

    pub fn host_deps(&self) -> &Path {
        self.host.deps()
    }

    /// Returns the directories where Rust crate dependencies are found for the
    /// specified unit.
    pub fn deps_dir(&self, unit: &Unit) -> &Path {
        self.layout(unit.kind).deps()
    }

    pub fn fingerprint_dir(&self, unit: &Unit<'a>) -> PathBuf {
        let dir = self.pkg_dir(unit);
        self.layout(unit.kind).fingerprint().join(dir)
    }

    /// Returns the appropriate directory layout for either a plugin or not.
    pub fn build_script_dir(&self, unit: &Unit<'a>) -> PathBuf {
        assert!(unit.target.is_custom_build());
        assert!(!unit.profile.run_custom_build);
        let dir = self.pkg_dir(unit);
        self.layout(Kind::Host).build().join(dir)
    }

    /// Returns the appropriate directory layout for either a plugin or not.
    pub fn build_script_out_dir(&self, unit: &Unit<'a>) -> PathBuf {
        assert!(unit.target.is_custom_build());
        assert!(unit.profile.run_custom_build);
        let dir = self.pkg_dir(unit);
        self.layout(unit.kind).build().join(dir).join("out")
    }

    /// Returns the file stem for a given target/profile combo (with metadata)
    pub fn file_stem(&self, unit: &Unit<'a>) -> String {
        match self.metas[unit] {
            Some(ref metadata) => format!("{}-{}", unit.target.crate_name(), metadata),
            None => self.bin_stem(unit),
        }
    }

    pub(super) fn target_filenames(
        &self,
        unit: &Unit<'a>,
        cx: &Context<'a, 'cfg>,
    ) -> CargoResult<Arc<Vec<(PathBuf, Option<PathBuf>, TargetFileType)>>> {
        self.outputs[unit]
            .try_borrow_with(|| self.calc_target_filenames(unit, cx))
            .map(Arc::clone)
    }

    /// Returns the bin stem for a given target (without metadata)
    fn bin_stem(&self, unit: &Unit) -> String {
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
    /// to filename separately
    ///
    /// Returns an Option because in some cases we don't want to link
    /// (eg a dependent lib)
    fn link_stem(&self, unit: &Unit<'a>) -> Option<(PathBuf, String)> {
        let src_dir = self.out_dir(unit);
        let bin_stem = self.bin_stem(unit);
        let file_stem = self.file_stem(unit);

        // We currently only lift files up from the `deps` directory. If
        // it was compiled into something like `example/` or `doc/` then
        // we don't want to link it up.
        if src_dir.ends_with("deps") {
            // Don't lift up library dependencies
            if self.ws.members().find(|&p| p == unit.pkg).is_none() && !unit.target.is_bin() {
                None
            } else {
                Some((
                    src_dir.parent().unwrap().to_owned(),
                    if unit.profile.test {
                        file_stem
                    } else {
                        bin_stem
                    },
                ))
            }
        } else if bin_stem == file_stem {
            None
        } else if src_dir.ends_with("examples") || src_dir.parent().unwrap().ends_with("build") {
            Some((src_dir, bin_stem))
        } else {
            None
        }
    }

    fn calc_target_filenames(
        &self,
        unit: &Unit<'a>,
        cx: &Context<'a, 'cfg>,
    ) -> CargoResult<Arc<Vec<(PathBuf, Option<PathBuf>, TargetFileType)>>> {
        let out_dir = self.out_dir(unit);
        let stem = self.file_stem(unit);
        let link_stem = self.link_stem(unit);
        let info = if unit.target.for_host() {
            &cx.host_info
        } else {
            &cx.target_info
        };

        let mut ret = Vec::new();
        let mut unsupported = Vec::new();
        {
            if unit.profile.check {
                let filename = out_dir.join(format!("lib{}.rmeta", stem));
                let link_dst = link_stem
                    .clone()
                    .map(|(ld, ls)| ld.join(format!("lib{}.rmeta", ls)));
                ret.push((filename, link_dst, TargetFileType::Linkable));
            } else {
                let mut add = |crate_type: &str, file_type: TargetFileType| -> CargoResult<()> {
                    let crate_type = if crate_type == "lib" {
                        "rlib"
                    } else {
                        crate_type
                    };
                    let mut crate_types = info.crate_types.borrow_mut();
                    let entry = crate_types.entry(crate_type.to_string());
                    let crate_type_info = match entry {
                        Entry::Occupied(o) => &*o.into_mut(),
                        Entry::Vacant(v) => {
                            let value = info.discover_crate_type(v.key())?;
                            &*v.insert(value)
                        }
                    };
                    match *crate_type_info {
                        Some((ref prefix, ref suffix)) => {
                            let suffixes = add_target_specific_suffixes(
                                cx.target_triple(),
                                crate_type,
                                unit.target.kind(),
                                suffix,
                                file_type,
                            );
                            for (suffix, file_type, should_replace_hyphens) in suffixes {
                                // wasm bin target will generate two files in deps such as
                                // "web-stuff.js" and "web_stuff.wasm". Note the different usages of
                                // "-" and "_". should_replace_hyphens is a flag to indicate that
                                // we need to convert the stem "web-stuff" to "web_stuff", so we
                                // won't miss "web_stuff.wasm".
                                let conv = |s: String| {
                                    if should_replace_hyphens {
                                        s.replace("-", "_")
                                    } else {
                                        s
                                    }
                                };
                                let filename = out_dir.join(format!(
                                    "{}{}{}",
                                    prefix,
                                    conv(stem.clone()),
                                    suffix
                                ));
                                let link_dst = link_stem.clone().map(|(ld, ls)| {
                                    ld.join(format!("{}{}{}", prefix, conv(ls), suffix))
                                });
                                ret.push((filename, link_dst, file_type));
                            }
                            Ok(())
                        }
                        // not supported, don't worry about it
                        None => {
                            unsupported.push(crate_type.to_string());
                            Ok(())
                        }
                    }
                };
                //info!("{:?}", unit);
                match *unit.target.kind() {
                    TargetKind::Bin
                    | TargetKind::CustomBuild
                    | TargetKind::ExampleBin
                    | TargetKind::Bench
                    | TargetKind::Test => {
                        add("bin", TargetFileType::Normal)?;
                    }
                    TargetKind::Lib(..) | TargetKind::ExampleLib(..) if unit.profile.test => {
                        add("bin", TargetFileType::Normal)?;
                    }
                    TargetKind::ExampleLib(ref kinds) | TargetKind::Lib(ref kinds) => {
                        for kind in kinds {
                            add(
                                kind.crate_type(),
                                if kind.linkable() {
                                    TargetFileType::Linkable
                                } else {
                                    TargetFileType::Normal
                                },
                            )?;
                        }
                    }
                }
            }
        }
        if ret.is_empty() {
            if !unsupported.is_empty() {
                bail!(
                    "cannot produce {} for `{}` as the target `{}` \
                     does not support these crate types",
                    unsupported.join(", "),
                    unit.pkg,
                    cx.target_triple()
                )
            }
            bail!(
                "cannot compile `{}` as the target `{}` does not \
                 support any of the output crate types",
                unit.pkg,
                cx.target_triple()
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
    // No metadata for dylibs because of a couple issues
    // - OSX encodes the dylib name in the executable
    // - Windows rustc multiple files of which we can't easily link all of them
    //
    // No metadata for bin because of an issue
    // - wasm32 rustc/emcc encodes the .wasm name in the .js (rust-lang/cargo#4535)
    //
    // Two exceptions
    // 1) Upstream dependencies (we aren't exporting + need to resolve name conflict)
    // 2) __CARGO_DEFAULT_LIB_METADATA env var
    //
    // Note, though, that the compiler's build system at least wants
    // path dependencies (eg libstd) to have hashes in filenames. To account for
    // that we have an extra hack here which reads the
    // `__CARGO_DEFAULT_LIB_METADATA` environment variable and creates a
    // hash in the filename if that's present.
    //
    // This environment variable should not be relied on! It's
    // just here for rustbuild. We need a more principled method
    // doing this eventually.
    let __cargo_default_lib_metadata = env::var("__CARGO_DEFAULT_LIB_METADATA");
    if !(unit.profile.test || unit.profile.check)
        && (unit.target.is_dylib() || unit.target.is_cdylib()
            || (unit.target.is_bin() && cx.target_triple().starts_with("wasm32-")))
        && unit.pkg.package_id().source_id().is_path()
        && !__cargo_default_lib_metadata.is_ok()
    {
        return None;
    }

    let mut hasher = SipHasher::new_with_keys(0, 0);

    // Unique metadata per (name, source, version) triple. This'll allow us
    // to pull crates from anywhere w/o worrying about conflicts
    unit.pkg
        .package_id()
        .stable_hash(cx.ws.root())
        .hash(&mut hasher);

    // Add package properties which map to environment variables
    // exposed by Cargo
    let manifest_metadata = unit.pkg.manifest().metadata();
    manifest_metadata.authors.hash(&mut hasher);
    manifest_metadata.description.hash(&mut hasher);
    manifest_metadata.homepage.hash(&mut hasher);

    // Also mix in enabled features to our metadata. This'll ensure that
    // when changing feature sets each lib is separately cached.
    cx.resolve
        .features_sorted(unit.pkg.package_id())
        .hash(&mut hasher);

    // Mix in the target-metadata of all the dependencies of this target
    {
        let mut deps_metadata = cx.dep_targets(unit)
            .iter()
            .map(|dep| metadata_of(dep, cx, metas))
            .collect::<Vec<_>>();
        deps_metadata.sort();
        deps_metadata.hash(&mut hasher);
    }

    // Throw in the profile we're compiling with. This helps caching
    // panic=abort and panic=unwind artifacts, additionally with various
    // settings like debuginfo and whatnot.
    unit.profile.hash(&mut hasher);

    // Artifacts compiled for the host should have a different metadata
    // piece than those compiled for the target, so make sure we throw in
    // the unit's `kind` as well
    unit.kind.hash(&mut hasher);

    // Finally throw in the target name/kind. This ensures that concurrent
    // compiles of targets in the same crate don't collide.
    unit.target.name().hash(&mut hasher);
    unit.target.kind().hash(&mut hasher);

    if let Ok(rustc) = cx.config.rustc() {
        rustc.verbose_version.hash(&mut hasher);
    }

    // Seed the contents of __CARGO_DEFAULT_LIB_METADATA to the hasher if present.
    // This should be the release channel, to get a different hash for each channel.
    if let Ok(ref channel) = __cargo_default_lib_metadata {
        channel.hash(&mut hasher);
    }
    Some(Metadata(hasher.finish()))
}

// (not a rustdoc)
// Return a list of 3-tuples (suffix, file_type, should_replace_hyphens).
//
// should_replace_hyphens will be used by the caller to replace "-" with "_"
// in a bin_stem. See the caller side (calc_target_filenames()) for details.
fn add_target_specific_suffixes(
    target_triple: &str,
    crate_type: &str,
    target_kind: &TargetKind,
    suffix: &str,
    file_type: TargetFileType,
) -> Vec<(String, TargetFileType, bool)> {
    let mut ret = vec![(suffix.to_string(), file_type, false)];

    // rust-lang/cargo#4500
    if target_triple.ends_with("pc-windows-msvc") && crate_type.ends_with("dylib")
        && suffix == ".dll"
    {
        ret.push((".dll.lib".to_string(), TargetFileType::Normal, false));
    }

    // rust-lang/cargo#4535
    if target_triple.starts_with("wasm32-") && crate_type == "bin" && suffix == ".js" {
        ret.push((".wasm".to_string(), TargetFileType::Normal, true));
    }

    // rust-lang/cargo#4490, rust-lang/cargo#4960
    //  - only uplift debuginfo for binaries.
    //    tests are run directly from target/debug/deps/
    //    and examples are inside target/debug/examples/ which already have symbols next to them
    //    so no need to do anything.
    if *target_kind == TargetKind::Bin {
        if target_triple.contains("-apple-") {
            ret.push((".dSYM".to_string(), TargetFileType::DebugInfo, false));
        } else if target_triple.ends_with("-msvc") {
            ret.push((".pdb".to_string(), TargetFileType::DebugInfo, false));
        }
    }

    ret
}
