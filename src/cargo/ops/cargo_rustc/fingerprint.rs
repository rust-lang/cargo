use std::fs::{self, File, OpenOptions};
use std::hash::{self, Hasher};
use std::io::prelude::*;
use std::io::{BufReader, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use filetime::FileTime;
use rustc_serialize::{json, Encodable, Decodable, Encoder, Decoder};

use core::{Package, TargetKind};
use util;
use util::{CargoResult, Fresh, Dirty, Freshness, internal, profile, ChainError};
use util::paths;

use super::job::Work;
use super::context::{Context, Unit};

/// A tuple result of the `prepare_foo` functions in this module.
///
/// The first element of the triple is whether the target in question is
/// currently fresh or not, and the second two elements are work to perform when
/// the target is dirty or fresh, respectively.
///
/// Both units of work are always generated because a fresh package may still be
/// rebuilt if some upstream dependency changes.
pub type Preparation = (Freshness, Work, Work);

/// Prepare the necessary work for the fingerprint for a specific target.
///
/// When dealing with fingerprints, cargo gets to choose what granularity
/// "freshness" is considered at. One option is considering freshness at the
/// package level. This means that if anything in a package changes, the entire
/// package is rebuilt, unconditionally. This simplicity comes at a cost,
/// however, in that test-only changes will cause libraries to be rebuilt, which
/// is quite unfortunate!
///
/// The cost was deemed high enough that fingerprints are now calculated at the
/// layer of a target rather than a package. Each target can then be kept track
/// of separately and only rebuilt as necessary. This requires cargo to
/// understand what the inputs are to a target, so we drive rustc with the
/// --dep-info flag to learn about all input files to a unit of compilation.
///
/// This function will calculate the fingerprint for a target and prepare the
/// work necessary to either write the fingerprint or copy over all fresh files
/// from the old directories to their new locations.
pub fn prepare_target<'a, 'cfg>(cx: &mut Context<'a, 'cfg>,
                                unit: &Unit<'a>) -> CargoResult<Preparation> {
    let _p = profile::start(format!("fingerprint: {} / {}",
                                    unit.pkg.package_id(), unit.target.name()));
    let new = dir(cx, unit);
    let loc = new.join(&filename(unit));

    debug!("fingerprint at: {}", loc.display());

    let fingerprint = try!(calculate(cx, unit));
    let compare = compare_old_fingerprint(&loc, &*fingerprint);
    log_compare(unit, &compare);

    let root = cx.out_dir(unit);
    let mut missing_outputs = false;
    if unit.profile.doc {
        missing_outputs = !root.join(unit.target.crate_name())
                               .join("index.html").exists();
    } else {
        for (filename, _) in try!(cx.target_filenames(unit)) {
            missing_outputs |= fs::metadata(root.join(filename)).is_err();
        }
    }

    let allow_failure = unit.profile.rustc_args.is_some();
    let write_fingerprint = Work::new(move |_| {
        match fingerprint.update_local() {
            Ok(()) => {}
            Err(..) if allow_failure => return Ok(()),
            Err(e) => return Err(e)
        }
        write_fingerprint(&loc, &*fingerprint)
    });

    let fresh = compare.is_ok() && !missing_outputs;
    Ok((if fresh {Fresh} else {Dirty}, write_fingerprint, Work::noop()))
}

/// A fingerprint can be considered to be a "short string" representing the
/// state of a world for a package.
///
/// If a fingerprint ever changes, then the package itself needs to be
/// recompiled. Inputs to the fingerprint include source code modifications,
/// compiler flags, compiler version, etc. This structure is not simply a
/// `String` due to the fact that some fingerprints cannot be calculated lazily.
///
/// Path sources, for example, use the mtime of the corresponding dep-info file
/// as a fingerprint (all source files must be modified *before* this mtime).
/// This dep-info file is not generated, however, until after the crate is
/// compiled. As a result, this structure can be thought of as a fingerprint
/// to-be. The actual value can be calculated via `hash()`, but the operation
/// may fail as some files may not have been generated.
///
/// Note that dependencies are taken into account for fingerprints because rustc
/// requires that whenever an upstream crate is recompiled that all downstream
/// dependants are also recompiled. This is typically tracked through
/// `DependencyQueue`, but it also needs to be retained here because Cargo can
/// be interrupted while executing, losing the state of the `DependencyQueue`
/// graph.
pub struct Fingerprint {
    rustc: u64,
    features: String,
    target: u64,
    profile: u64,
    deps: Vec<(String, Arc<Fingerprint>)>,
    local: LocalFingerprint,
    memoized_hash: Mutex<Option<u64>>,
    rustflags: Vec<String>,
}

#[derive(RustcEncodable, RustcDecodable, Hash)]
enum LocalFingerprint {
    Precalculated(String),
    MtimeBased(MtimeSlot, PathBuf),
}

struct MtimeSlot(Mutex<Option<FileTime>>);

impl Fingerprint {
    fn update_local(&self) -> CargoResult<()> {
        match self.local {
            LocalFingerprint::MtimeBased(ref slot, ref path) => {
                let meta = try!(fs::metadata(path).chain_error(|| {
                    internal(format!("failed to stat `{}`", path.display()))
                }));
                let mtime = FileTime::from_last_modification_time(&meta);
                *slot.0.lock().unwrap() = Some(mtime);
            }
            LocalFingerprint::Precalculated(..) => return Ok(())
        }

        *self.memoized_hash.lock().unwrap() = None;
        Ok(())
    }

    fn hash(&self) -> u64 {
        if let Some(s) = *self.memoized_hash.lock().unwrap() {
            return s
        }
        let ret = util::hash_u64(self);
        *self.memoized_hash.lock().unwrap() = Some(ret);
        ret
    }

    fn compare(&self, old: &Fingerprint) -> CargoResult<()> {
        if self.rustc != old.rustc {
            bail!("rust compiler has changed")
        }
        if self.features != old.features {
            bail!("features have changed: {} != {}", self.features, old.features)
        }
        if self.target != old.target {
            bail!("target configuration has changed")
        }
        if self.profile != old.profile {
            bail!("profile configuration has changed")
        }
        if self.rustflags != old.rustflags {
            return Err(internal("RUSTFLAGS has changed"))
        }
        match (&self.local, &old.local) {
            (&LocalFingerprint::Precalculated(ref a),
             &LocalFingerprint::Precalculated(ref b)) => {
                if a != b {
                    bail!("precalculated components have changed: {} != {}",
                          a, b)
                }
            }
            (&LocalFingerprint::MtimeBased(ref a, ref ap),
             &LocalFingerprint::MtimeBased(ref b, ref bp)) => {
                let a = a.0.lock().unwrap();
                let b = b.0.lock().unwrap();
                if *a != *b {
                    bail!("mtime based components have changed: {:?} != {:?}, \
                           paths are {:?} and {:?}", *a, *b, ap, bp)
                }
            }
            _ => bail!("local fingerprint type has changed"),
        }

        if self.deps.len() != old.deps.len() {
            bail!("number of dependencies has changed")
        }
        for (a, b) in self.deps.iter().zip(old.deps.iter()) {
            if a.1.hash() != b.1.hash() {
                bail!("new ({}) != old ({})", a.0, b.0)
            }
        }
        Ok(())
    }
}

impl hash::Hash for Fingerprint {
    fn hash<H: Hasher>(&self, h: &mut H) {
        let Fingerprint {
            rustc,
            ref features,
            target,
            profile,
            ref deps,
            ref local,
            memoized_hash: _,
            ref rustflags,
        } = *self;
        (rustc, features, target, profile, deps, local, rustflags).hash(h)
    }
}

impl Encodable for Fingerprint {
    fn encode<E: Encoder>(&self, e: &mut E) -> Result<(), E::Error> {
        e.emit_struct("Fingerprint", 6, |e| {
            try!(e.emit_struct_field("rustc", 0, |e| self.rustc.encode(e)));
            try!(e.emit_struct_field("target", 1, |e| self.target.encode(e)));
            try!(e.emit_struct_field("profile", 2, |e| self.profile.encode(e)));
            try!(e.emit_struct_field("local", 3, |e| self.local.encode(e)));
            try!(e.emit_struct_field("features", 4, |e| {
                self.features.encode(e)
            }));
            try!(e.emit_struct_field("deps", 5, |e| {
                self.deps.iter().map(|&(ref a, ref b)| {
                    (a, b.hash())
                }).collect::<Vec<_>>().encode(e)
            }));
            try!(e.emit_struct_field("rustflags", 6, |e| self.rustflags.encode(e)));
            Ok(())
        })
    }
}

impl Decodable for Fingerprint {
    fn decode<D: Decoder>(d: &mut D) -> Result<Fingerprint, D::Error> {
        fn decode<T: Decodable, D: Decoder>(d: &mut D) -> Result<T, D::Error> {
            Decodable::decode(d)
        }
        d.read_struct("Fingerprint", 6, |d| {
            Ok(Fingerprint {
                rustc: try!(d.read_struct_field("rustc", 0, decode)),
                target: try!(d.read_struct_field("target", 1, decode)),
                profile: try!(d.read_struct_field("profile", 2, decode)),
                local: try!(d.read_struct_field("local", 3, decode)),
                features: try!(d.read_struct_field("features", 4, decode)),
                memoized_hash: Mutex::new(None),
                deps: {
                    let decode = decode::<Vec<(String, u64)>, D>;
                    let v = try!(d.read_struct_field("deps", 5, decode));
                    v.into_iter().map(|(name, hash)| {
                        (name, Arc::new(Fingerprint {
                            rustc: 0,
                            target: 0,
                            profile: 0,
                            local: LocalFingerprint::Precalculated(String::new()),
                            features: String::new(),
                            deps: Vec::new(),
                            memoized_hash: Mutex::new(Some(hash)),
                            rustflags: Vec::new(),
                        }))
                    }).collect()
                },
                rustflags: try!(d.read_struct_field("rustflags", 6, decode)),
            })
        })
    }
}

impl hash::Hash for MtimeSlot {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.lock().unwrap().hash(h)
    }
}

impl Encodable for MtimeSlot {
    fn encode<E: Encoder>(&self, e: &mut E) -> Result<(), E::Error> {
        self.0.lock().unwrap().map(|ft| {
            (ft.seconds_relative_to_1970(), ft.nanoseconds())
        }).encode(e)
    }
}

impl Decodable for MtimeSlot {
    fn decode<D: Decoder>(e: &mut D) -> Result<MtimeSlot, D::Error> {
        let kind: Option<(u64, u32)> = try!(Decodable::decode(e));
        Ok(MtimeSlot(Mutex::new(kind.map(|(s, n)| {
            FileTime::from_seconds_since_1970(s, n)
        }))))
    }
}

/// Calculates the fingerprint for a package/target pair.
///
/// This fingerprint is used by Cargo to learn about when information such as:
///
/// * A non-path package changes (changes version, changes revision, etc).
/// * Any dependency changes
/// * The compiler changes
/// * The set of features a package is built with changes
/// * The profile a target is compiled with changes (e.g. opt-level changes)
///
/// Information like file modification time is only calculated for path
/// dependencies and is calculated in `calculate_target_fresh`.
fn calculate<'a, 'cfg>(cx: &mut Context<'a, 'cfg>, unit: &Unit<'a>)
                       -> CargoResult<Arc<Fingerprint>> {
    if let Some(s) = cx.fingerprints.get(unit) {
        return Ok(s.clone())
    }

    // First, calculate all statically known "salt data" such as the profile
    // information (compiler flags), the compiler version, activated features,
    // and target configuration.
    let features = cx.resolve.features(unit.pkg.package_id());
    let features = features.map(|s| {
        let mut v = s.iter().collect::<Vec<_>>();
        v.sort();
        v
    });

    // Next, recursively calculate the fingerprint for all of our dependencies.
    //
    // Skip the fingerprints of build scripts as they may not always be
    // available and the dirtiness propagation for modification is tracked
    // elsewhere. Also skip fingerprints of binaries because they don't actually
    // induce a recompile, they're just dependencies in the sense that they need
    // to be built.
    let deps = try!(cx.dep_targets(unit));
    let deps = try!(deps.iter().filter(|u| {
        !u.target.is_custom_build() && !u.target.is_bin()
    }).map(|unit| {
        calculate(cx, unit).map(|fingerprint| {
            (unit.pkg.package_id().to_string(), fingerprint)
        })
    }).collect::<CargoResult<Vec<_>>>());

    // And finally, calculate what our own local fingerprint is
    let local = if use_dep_info(unit) {
        let dep_info = dep_info_loc(cx, unit);
        let mtime = try!(dep_info_mtime_if_fresh(&dep_info));
        LocalFingerprint::MtimeBased(MtimeSlot(Mutex::new(mtime)), dep_info)
    } else {
        let fingerprint = try!(pkg_fingerprint(cx, unit.pkg));
        LocalFingerprint::Precalculated(fingerprint)
    };
    let mut deps = deps;
    deps.sort_by(|&(ref a, _), &(ref b, _)| a.cmp(b));
    let extra_flags = if unit.profile.doc {
        try!(cx.rustdocflags_args(unit))
    } else {
        try!(cx.rustflags_args(unit))
    };
    let fingerprint = Arc::new(Fingerprint {
        rustc: util::hash_u64(&try!(cx.config.rustc_info()).verbose_version),
        target: util::hash_u64(&unit.target),
        profile: util::hash_u64(&unit.profile),
        features: format!("{:?}", features),
        deps: deps,
        local: local,
        memoized_hash: Mutex::new(None),
        rustflags: extra_flags,
    });
    cx.fingerprints.insert(*unit, fingerprint.clone());
    Ok(fingerprint)
}


// We want to use the mtime for files if we're a path source, but if we're a
// git/registry source, then the mtime of files may fluctuate, but they won't
// change so long as the source itself remains constant (which is the
// responsibility of the source)
fn use_dep_info(unit: &Unit) -> bool {
    let path = unit.pkg.summary().source_id().is_path();
    !unit.profile.doc && path
}

/// Prepare the necessary work for the fingerprint of a build command.
///
/// Build commands are located on packages, not on targets. Additionally, we
/// don't have --dep-info to drive calculation of the fingerprint of a build
/// command. This brings up an interesting predicament which gives us a few
/// options to figure out whether a build command is dirty or not:
///
/// 1. A build command is dirty if *any* file in a package changes. In theory
///    all files are candidate for being used by the build command.
/// 2. A build command is dirty if any file in a *specific directory* changes.
///    This may lose information as it may require files outside of the specific
///    directory.
/// 3. A build command must itself provide a dep-info-like file stating how it
///    should be considered dirty or not.
///
/// The currently implemented solution is option (1), although it is planned to
/// migrate to option (2) in the near future.
pub fn prepare_build_cmd<'a, 'cfg>(cx: &mut Context<'a, 'cfg>, unit: &Unit<'a>)
                                   -> CargoResult<Preparation> {
    let _p = profile::start(format!("fingerprint build cmd: {}",
                                    unit.pkg.package_id()));
    let new = dir(cx, unit);
    let loc = new.join("build");

    debug!("fingerprint at: {}", loc.display());

    // If this build script execution has been overridden, then the fingerprint
    // is just a hash of what it was overridden with. Otherwise the fingerprint
    // is that of the entire package itself as we just consider everything as
    // input to the build script.
    let (local, output_path) = {
        let state = cx.build_state.outputs.lock().unwrap();
        match state.get(&(unit.pkg.package_id().clone(), unit.kind)) {
            Some(output) => {
                let s = format!("overridden build state with hash: {}",
                                util::hash_u64(output));
                (LocalFingerprint::Precalculated(s), None)
            }
            None => {
                let &(ref output, ref deps) = &cx.build_explicit_deps[unit];

                let local = if deps.is_empty() {
                    let s = try!(pkg_fingerprint(cx, unit.pkg));
                    LocalFingerprint::Precalculated(s)
                } else {
                    let deps = deps.iter().map(|p| unit.pkg.root().join(p));
                    let mtime = mtime_if_fresh(output, deps);
                    let mtime = MtimeSlot(Mutex::new(mtime));
                    LocalFingerprint::MtimeBased(mtime, output.clone())
                };

                (local, Some(output.clone()))
            }
        }
    };

    let mut fingerprint = Fingerprint {
        rustc: 0,
        target: 0,
        profile: 0,
        features: String::new(),
        deps: Vec::new(),
        local: local,
        memoized_hash: Mutex::new(None),
        rustflags: Vec::new(),
    };
    let compare = compare_old_fingerprint(&loc, &fingerprint);
    log_compare(unit, &compare);

    // When we write out the fingerprint, we may want to actually change the
    // kind of fingerprint being recorded. If we started out, then the previous
    // run of the build script (or if it had never run before) may indicate to
    // use the `Precalculated` variant with the `pkg_fingerprint`. If the build
    // script then prints `rerun-if-changed`, however, we need to record what's
    // necessary for that fingerprint.
    //
    // Hence, if there were some `rerun-if-changed` directives forcibly change
    // the kind of fingerprint over to the `MtimeBased` variant where the
    // relevant mtime is the output path of the build script.
    let state = cx.build_state.clone();
    let key = (unit.pkg.package_id().clone(), unit.kind);
    let write_fingerprint = Work::new(move |_| {
        if let Some(output_path) = output_path {
            let outputs = state.outputs.lock().unwrap();
            if !outputs[&key].rerun_if_changed.is_empty() {
                let slot = MtimeSlot(Mutex::new(None));
                fingerprint.local = LocalFingerprint::MtimeBased(slot,
                                                                 output_path);
                try!(fingerprint.update_local());
            }
        }
        write_fingerprint(&loc, &fingerprint)
    });

    Ok((if compare.is_ok() {Fresh} else {Dirty}, write_fingerprint, Work::noop()))
}

fn write_fingerprint(loc: &Path, fingerprint: &Fingerprint) -> CargoResult<()> {
    let hash = fingerprint.hash();
    debug!("write fingerprint: {}", loc.display());
    try!(paths::write(&loc, util::to_hex(hash).as_bytes()));
    try!(paths::write(&loc.with_extension("json"),
                      json::encode(&fingerprint).unwrap().as_bytes()));
    Ok(())
}

/// Prepare work for when a package starts to build
pub fn prepare_init(cx: &mut Context, unit: &Unit) -> CargoResult<()> {
    let new1 = dir(cx, unit);
    let new2 = new1.clone();

    if fs::metadata(&new1).is_err() {
        try!(fs::create_dir(&new1));
    }
    if fs::metadata(&new2).is_err() {
        try!(fs::create_dir(&new2));
    }
    Ok(())
}

/// Return the (old, new) location for fingerprints for a package
pub fn dir(cx: &Context, unit: &Unit) -> PathBuf {
    cx.layout(unit.pkg, unit.kind).proxy().fingerprint(unit.pkg)
}

/// Returns the (old, new) location for the dep info file of a target.
pub fn dep_info_loc(cx: &Context, unit: &Unit) -> PathBuf {
    dir(cx, unit).join(&format!("dep-{}", filename(unit)))
}

fn compare_old_fingerprint(loc: &Path, new_fingerprint: &Fingerprint)
                           -> CargoResult<()> {
    let old_fingerprint_short = try!(paths::read(loc));
    let new_hash = new_fingerprint.hash();

    if util::to_hex(new_hash) == old_fingerprint_short {
        return Ok(())
    }

    let old_fingerprint_json = try!(paths::read(&loc.with_extension("json")));
    let old_fingerprint = try!(json::decode(&old_fingerprint_json).chain_error(|| {
        internal(format!("failed to deserialize json"))
    }));
    new_fingerprint.compare(&old_fingerprint)
}

fn log_compare(unit: &Unit, compare: &CargoResult<()>) {
    let mut e = match *compare {
        Ok(..) => return,
        Err(ref e) => &**e,
    };
    info!("fingerprint error for {}: {}", unit.pkg, e);
    while let Some(cause) = e.cargo_cause() {
        info!("  cause: {}", cause);
        e = cause;
    }
    let mut e = e.cause();
    while let Some(cause) = e {
        info!("  cause: {}", cause);
        e = cause.cause();
    }
}

fn dep_info_mtime_if_fresh(dep_info: &Path) -> CargoResult<Option<FileTime>> {
    macro_rules! fs_try {
        ($e:expr) => (match $e { Ok(e) => e, Err(..) => return Ok(None) })
    }
    let mut f = BufReader::new(fs_try!(File::open(dep_info)));
    // see comments in append_current_dir for where this cwd is manifested from.
    let mut cwd = Vec::new();
    if fs_try!(f.read_until(0, &mut cwd)) == 0 {
        return Ok(None)
    }
    let cwd = try!(util::bytes2path(&cwd[..cwd.len()-1]));
    let line = match f.lines().next() {
        Some(Ok(line)) => line,
        _ => return Ok(None),
    };
    let pos = try!(line.find(": ").chain_error(|| {
        internal(format!("dep-info not in an understood format: {}",
                         dep_info.display()))
    }));
    let deps = &line[pos + 2..];

    let mut paths = Vec::new();
    let mut deps = deps.split(' ').map(|s| s.trim()).filter(|s| !s.is_empty());
    loop {
        let mut file = match deps.next() {
            Some(s) => s.to_string(),
            None => break,
        };
        while file.ends_with("\\") {
            file.pop();
            file.push(' ');
            file.push_str(try!(deps.next().chain_error(|| {
                internal(format!("malformed dep-info format, trailing \\"))
            })));
        }
        paths.push(cwd.join(&file));
    }

    Ok(mtime_if_fresh(&dep_info, paths.iter()))
}

fn pkg_fingerprint(cx: &Context, pkg: &Package) -> CargoResult<String> {
    let source_id = pkg.package_id().source_id();
    let sources = cx.packages.sources();
    let source = try!(sources.get(source_id).chain_error(|| {
        internal("missing package source")
    }));
    source.fingerprint(pkg)
}

fn mtime_if_fresh<I>(output: &Path, paths: I) -> Option<FileTime>
    where I: IntoIterator,
          I::Item: AsRef<Path>,
{
    let meta = match fs::metadata(output) {
        Ok(meta) => meta,
        Err(..) => return None,
    };
    let mtime = FileTime::from_last_modification_time(&meta);

    let any_stale = paths.into_iter().any(|path| {
        let path = path.as_ref();
        let meta = match fs::metadata(path) {
            Ok(meta) => meta,
            Err(..) => {
                info!("stale: {} -- missing", path.display());
                return true
            }
        };
        let mtime2 = FileTime::from_last_modification_time(&meta);
        if mtime2 > mtime {
            info!("stale: {} -- {} vs {}", path.display(), mtime2, mtime);
            true
        } else {
            false
        }
    });

    if any_stale {
        None
    } else {
        Some(mtime)
    }
}

fn filename(unit: &Unit) -> String {
    let kind = match *unit.target.kind() {
        TargetKind::Lib(..) => "lib",
        TargetKind::Bin => "bin",
        TargetKind::Test => "integration-test",
        TargetKind::Example => "example",
        TargetKind::Bench => "bench",
        TargetKind::CustomBuild => "build-script",
    };
    let flavor = if unit.profile.test {
        "test-"
    } else if unit.profile.doc {
        "doc-"
    } else {
        ""
    };
    format!("{}{}-{}", flavor, kind, unit.target.name())
}

// The dep-info files emitted by the compiler all have their listed paths
// relative to whatever the current directory was at the time that the compiler
// was invoked. As the current directory may change over time, we need to record
// what that directory was at the beginning of the file so we can know about it
// next time.
pub fn append_current_dir(path: &Path, cwd: &Path) -> CargoResult<()> {
    debug!("appending {} <- {}", path.display(), cwd.display());
    let mut f = try!(OpenOptions::new().read(true).write(true).open(path));
    let mut contents = Vec::new();
    try!(f.read_to_end(&mut contents));
    try!(f.seek(SeekFrom::Start(0)));
    try!(f.write_all(try!(util::path2bytes(cwd))));
    try!(f.write_all(&[0]));
    try!(f.write_all(&contents));
    Ok(())
}
