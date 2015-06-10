use std::fs::{self, File, OpenOptions};
use std::io::prelude::*;
use std::io::{BufReader, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use filetime::FileTime;

use core::{Package, Target, Profile};
use util;
use util::{CargoResult, Fresh, Dirty, Freshness, internal, profile, ChainError};

use super::Kind;
use super::job::Work;
use super::context::Context;

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
                                pkg: &'a Package,
                                target: &'a Target,
                                profile: &'a Profile,
                                kind: Kind) -> CargoResult<Preparation> {
    let _p = profile::start(format!("fingerprint: {} / {}",
                                    pkg.package_id(), target.name()));
    let new = dir(cx, pkg, kind);
    let loc = new.join(&filename(target, profile));

    info!("fingerprint at: {}", loc.display());

    let mut fingerprint = try!(calculate(cx, pkg, target, profile, kind));
    let is_fresh = try!(is_fresh(&loc, &mut fingerprint));

    let root = cx.out_dir(pkg, kind, target);
    let mut missing_outputs = false;
    if !profile.doc {
        for filename in try!(cx.target_filenames(pkg, target, profile,
                                                 kind)).iter() {
            missing_outputs |= fs::metadata(root.join(filename)).is_err();
        }
    }

    let allow_failure = profile.rustc_args.is_some();
    Ok(prepare(is_fresh && !missing_outputs, allow_failure, loc, fingerprint))
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
/// to-be. The actual value can be calculated via `resolve()`, but the operation
/// may fail as some files may not have been generated.
///
/// Note that dependencies are taken into account for fingerprints because rustc
/// requires that whenever an upstream crate is recompiled that all downstream
/// dependants are also recompiled. This is typically tracked through
/// `DependencyQueue`, but it also needs to be retained here because Cargo can
/// be interrupted while executing, losing the state of the `DependencyQueue`
/// graph.
pub type Fingerprint = Arc<FingerprintInner>;
struct FingerprintInner {
    extra: String,
    deps: Vec<Fingerprint>,
    local: LocalFingerprint,
    resolved: Mutex<Option<String>>,
}

#[derive(Clone)]
enum LocalFingerprint {
    Precalculated(String),
    MtimeBased(Option<FileTime>, PathBuf),
}

impl FingerprintInner {
    fn resolve(&self, force: bool) -> CargoResult<String> {
        if !force {
            if let Some(ref s) = *self.resolved.lock().unwrap() {
                return Ok(s.clone())
            }
        }
        let mut deps: Vec<_> = try!(self.deps.iter().map(|s| {
            s.resolve(force)
        }).collect());
        deps.sort();
        let known = match self.local {
            LocalFingerprint::Precalculated(ref s) => s.clone(),
            LocalFingerprint::MtimeBased(Some(n), _) if !force => n.to_string(),
            LocalFingerprint::MtimeBased(_, ref p) => {
                debug!("resolving: {}", p.display());
                let meta = try!(fs::metadata(p));
                FileTime::from_last_modification_time(&meta).to_string()
            }
        };
        let resolved = util::short_hash(&(&known, &self.extra, &deps));
        debug!("inputs: {} {} {:?} => {}", known, self.extra, deps, resolved);
        *self.resolved.lock().unwrap() = Some(resolved.clone());
        Ok(resolved)
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
fn calculate<'a, 'cfg>(cx: &mut Context<'a, 'cfg>,
                       pkg: &'a Package,
                       target: &'a Target,
                       profile: &'a Profile,
                       kind: Kind)
                       -> CargoResult<Fingerprint> {
    let key = (pkg.package_id(), target, profile, kind);
    match cx.fingerprints.get(&key) {
        Some(s) => return Ok(s.clone()),
        None => {}
    }

    // First, calculate all statically known "salt data" such as the profile
    // information (compiler flags), the compiler version, activated features,
    // and target configuration.
    let features = cx.resolve.features(pkg.package_id());
    let features = features.map(|s| {
        let mut v = s.iter().collect::<Vec<&String>>();
        v.sort();
        v
    });
    let extra = util::short_hash(&(cx.config.rustc_version(), target, &features,
                                   profile));
    debug!("extra {:?} {:?} {:?} = {}", target, profile, features, extra);

    // Next, recursively calculate the fingerprint for all of our dependencies.
    //
    // Skip the fingerprints of build scripts as they may not always be
    // available and the dirtiness propagation for modification is tracked
    // elsewhere. Also skip fingerprints of binaries because they don't actually
    // induce a recompile, they're just dependencies in the sense that they need
    // to be built.
    let deps = try!(cx.dep_targets(pkg, target, profile).into_iter()
                      .filter(|&(_, t, _)| !t.is_custom_build() && !t.is_bin())
                      .map(|(pkg, target, profile)| {
        let kind = match kind {
            Kind::Host => Kind::Host,
            Kind::Target if target.for_host() => Kind::Host,
            Kind::Target => Kind::Target,
        };
        calculate(cx, pkg, target, profile, kind)
    }).collect::<CargoResult<Vec<_>>>());

    // And finally, calculate what our own local fingerprint is
    let local = if use_dep_info(pkg, profile) {
        let dep_info = dep_info_loc(cx, pkg, target, profile, kind);
        let mtime = try!(calculate_target_mtime(&dep_info));

        // if the mtime listed is not fresh, then remove the `dep_info` file to
        // ensure that future calls to `resolve()` won't work.
        if mtime.is_none() {
            let _ = fs::remove_file(&dep_info);
        }
        LocalFingerprint::MtimeBased(mtime, dep_info)
    } else {
        LocalFingerprint::Precalculated(try!(calculate_pkg_fingerprint(cx, pkg)))
    };
    let fingerprint = Arc::new(FingerprintInner {
        extra: extra,
        deps: deps,
        local: local,
        resolved: Mutex::new(None),
    });
    cx.fingerprints.insert(key, fingerprint.clone());
    Ok(fingerprint)
}


// We want to use the mtime for files if we're a path source, but if we're a
// git/registry source, then the mtime of files may fluctuate, but they won't
// change so long as the source itself remains constant (which is the
// responsibility of the source)
fn use_dep_info(pkg: &Package, profile: &Profile) -> bool {
    let path = pkg.summary().source_id().is_path();
    !profile.doc && path
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
pub fn prepare_build_cmd(cx: &mut Context, pkg: &Package, kind: Kind)
                         -> CargoResult<Preparation> {
    let _p = profile::start(format!("fingerprint build cmd: {}",
                                    pkg.package_id()));
    let new = dir(cx, pkg, kind);
    let loc = new.join("build");

    info!("fingerprint at: {}", loc.display());

    let new_fingerprint = try!(calculate_build_cmd_fingerprint(cx, pkg));
    let new_fingerprint = Arc::new(FingerprintInner {
        extra: String::new(),
        deps: Vec::new(),
        local: LocalFingerprint::Precalculated(new_fingerprint),
        resolved: Mutex::new(None),
    });

    let is_fresh = try!(is_fresh(&loc, &new_fingerprint));

    Ok(prepare(is_fresh, false, loc, new_fingerprint))
}

/// Prepare work for when a package starts to build
pub fn prepare_init(cx: &mut Context, pkg: &Package, kind: Kind)
                    -> (Work, Work) {
    let new1 = dir(cx, pkg, kind);
    let new2 = new1.clone();

    let work1 = Work::new(move |_| {
        if fs::metadata(&new1).is_err() {
            try!(fs::create_dir(&new1));
        }
        Ok(())
    });
    let work2 = Work::new(move |_| {
        if fs::metadata(&new2).is_err() {
            try!(fs::create_dir(&new2));
        }
        Ok(())
    });

    (work1, work2)
}

/// Given the data to build and write a fingerprint, generate some Work
/// instances to actually perform the necessary work.
fn prepare(is_fresh: bool,
           allow_failure: bool,
           loc: PathBuf,
           fingerprint: Fingerprint) -> Preparation {
    let write_fingerprint = Work::new(move |_| {
        debug!("write fingerprint: {}", loc.display());
        let fingerprint = fingerprint.resolve(true).chain_error(|| {
            internal("failed to resolve a pending fingerprint")
        });
        let fingerprint = match fingerprint {
            Ok(f) => f,
            Err(..) if allow_failure => return Ok(()),
            Err(e) => return Err(e),
        };
        let mut f = try!(File::create(&loc));
        try!(f.write_all(fingerprint.as_bytes()));
        Ok(())
    });

    (if is_fresh {Fresh} else {Dirty}, write_fingerprint, Work::noop())
}

/// Return the (old, new) location for fingerprints for a package
pub fn dir(cx: &Context, pkg: &Package, kind: Kind) -> PathBuf {
    cx.layout(pkg, kind).proxy().fingerprint(pkg)
}

/// Returns the (old, new) location for the dep info file of a target.
pub fn dep_info_loc(cx: &Context, pkg: &Package, target: &Target,
                    profile: &Profile, kind: Kind) -> PathBuf {
    dir(cx, pkg, kind).join(&format!("dep-{}", filename(target, profile)))
}

fn is_fresh(loc: &Path, new_fingerprint: &Fingerprint) -> CargoResult<bool> {
    let mut file = match File::open(loc) {
        Ok(file) => file,
        Err(..) => return Ok(false),
    };

    let mut old_fingerprint = String::new();
    try!(file.read_to_string(&mut old_fingerprint));
    let new_fingerprint = match new_fingerprint.resolve(false) {
        Ok(s) => s,
        Err(..) => return Ok(false),
    };

    trace!("old fingerprint: {}", old_fingerprint);
    trace!("new fingerprint: {}", new_fingerprint);

    Ok(old_fingerprint == new_fingerprint)
}

fn calculate_target_mtime(dep_info: &Path) -> CargoResult<Option<FileTime>> {
    macro_rules! fs_try {
        ($e:expr) => (match $e { Ok(e) => e, Err(..) => return Ok(None) })
    }
    let mut f = BufReader::new(fs_try!(File::open(dep_info)));
    // see comments in append_current_dir for where this cwd is manifested from.
    let mut cwd = Vec::new();
    fs_try!(f.read_until(0, &mut cwd));
    let cwd = try!(util::bytes2path(&cwd[..cwd.len()-1]));
    let line = match f.lines().next() {
        Some(Ok(line)) => line,
        _ => return Ok(None),
    };
    let meta = try!(fs::metadata(&dep_info));
    let mtime = FileTime::from_last_modification_time(&meta);
    let pos = try!(line.find(": ").chain_error(|| {
        internal(format!("dep-info not in an understood format: {}",
                         dep_info.display()))
    }));
    let deps = &line[pos + 2..];

    let mut deps = deps.split(' ').map(|s| s.trim()).filter(|s| !s.is_empty());
    loop {
        let mut file = match deps.next() {
            Some(s) => s.to_string(),
            None => break,
        };
        while file.ends_with("\\") {
            file.pop();
            file.push(' ');
            file.push_str(deps.next().unwrap())
        }
        let meta = match fs::metadata(cwd.join(&file)) {
            Ok(meta) => meta,
            Err(..) => { info!("stale: {} -- missing", file); return Ok(None) }
        };
        let file_mtime = FileTime::from_last_modification_time(&meta);
        if file_mtime > mtime {
            info!("stale: {} -- {} vs {}", file, file_mtime, mtime);
            return Ok(None)
        }
    }

    Ok(Some(mtime))
}

fn calculate_build_cmd_fingerprint(cx: &Context, pkg: &Package)
                                   -> CargoResult<String> {
    // TODO: this should be scoped to just the `build` directory, not the entire
    // package.
    calculate_pkg_fingerprint(cx, pkg)
}

fn calculate_pkg_fingerprint(cx: &Context, pkg: &Package) -> CargoResult<String> {
    let source = cx.sources
        .get(pkg.package_id().source_id())
        .expect("BUG: Missing package source");

    source.fingerprint(pkg)
}

fn filename(target: &Target, profile: &Profile) -> String {
    let kind = if target.is_lib() {"lib"} else {"bin"};
    let flavor = if target.is_test() || profile.test {
        "test-"
    } else if profile.doc {
        "doc-"
    } else {
        ""
    };
    format!("{}{}-{}", flavor, kind, target.name())
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
