use std::hash::Hasher;
use std::hash::sip::SipHasher;
use std::io::{fs, File};

use core::{Package, Target};
use util;
use util::hex::short_hash;
use util::{CargoResult, Fresh, Dirty, Freshness, internal, Require, profile};

use super::job::Job;
use super::context::Context;

/// Calculates the fingerprint of a package's targets and prepares to write a
/// new fingerprint.
///
/// This function will first calculate the freshness of the package and return
/// it as the first part of the return tuple. It will then prepare a job to
/// update the fingerprint if this package is actually rebuilt as part of
/// compilation, returning the job as the second part of the tuple.
///
/// The third part of the tuple is a job to run when a package is discovered to
/// be fresh to ensure that all of its artifacts are moved to the correct
/// location.
pub fn prepare<'a, 'b>(cx: &mut Context<'a, 'b>, pkg: &'a Package,
                       targets: &[&'a Target])
                       -> CargoResult<(Freshness, Job, Job)> {
    let _p = profile::start(format!("fingerprint: {}", pkg));
    let filename = format!(".{}.{}.fingerprint", pkg.get_name(),
                           short_hash(pkg.get_package_id()));
    let filename = filename.as_slice();
    let (old_fingerprint_loc, new_fingerprint_loc) = {
        let layout = cx.layout(false);
        (layout.old_root().join(filename), layout.root().join(filename))
    };

    // First, figure out if the old location exists, and if it does whether it's
    // still fresh or not.
    let (is_fresh, fingerprint) = try!(is_fresh(pkg, &old_fingerprint_loc,
                                                cx, targets));

    // Prepare a job to update the location of the new fingerprint.
    let new_fingerprint_loc2 = new_fingerprint_loc.clone();
    let write_fingerprint = Job::new(proc() {
        let mut f = try!(File::create(&new_fingerprint_loc2));
        try!(f.write_str(fingerprint.as_slice()));
        Ok(Vec::new())
    });

    // Prepare a job to copy over all old artifacts into their new destination.
    let mut pairs = Vec::new();
    pairs.push((old_fingerprint_loc, new_fingerprint_loc));

    // TODO: this shouldn't explicitly pass false, for more info see
    //       cargo_rustc::compile_custom
    if pkg.get_manifest().get_build().len() > 0 {
        let layout = cx.layout(false);
        pairs.push((layout.old_native(pkg), layout.native(pkg)));
    }

    for &target in targets.iter() {
        if target.get_profile().is_doc() { continue }
        let target_layout = cx.layout(false);
        let plugin_layout = cx.layout(true);
        let req = cx.get_requirement(pkg, target);

        for filename in cx.target_filenames(target).iter() {
            let filename = filename.as_slice();
            if req.is_target() {
                pairs.push((target_layout.old_root().join(filename),
                            target_layout.root().join(filename)));
            }
            if req.is_plugin() && plugin_layout.root() != target_layout.root() {
                pairs.push((plugin_layout.old_root().join(filename),
                            plugin_layout.root().join(filename)));
            }
        }
    }
    let move_old = Job::new(proc() {
        for &(ref src, ref dst) in pairs.iter() {
            try!(fs::rename(src, dst));
        }
        Ok(Vec::new())
    });

    Ok((if is_fresh {Fresh} else {Dirty}, write_fingerprint, move_old))
}

fn is_fresh(dep: &Package, loc: &Path, cx: &mut Context, targets: &[&Target])
            -> CargoResult<(bool, String)> {
    let dep_fingerprint = try!(get_fingerprint(dep, cx));
    let new_pkg_fingerprint = format!("{}{}", cx.rustc_version, dep_fingerprint);

    let new_fingerprint = fingerprint(new_pkg_fingerprint, hash_targets(targets));

    let mut file = match File::open(loc) {
        Ok(file) => file,
        Err(..) => return Ok((false, new_fingerprint)),
    };

    let old_fingerprint = try!(file.read_to_string());

    log!(5, "old fingerprint: {}", old_fingerprint);
    log!(5, "new fingerprint: {}", new_fingerprint);

    Ok((old_fingerprint == new_fingerprint, new_fingerprint))
}

fn get_fingerprint(pkg: &Package, cx: &Context) -> CargoResult<String> {
    let id = pkg.get_package_id().get_source_id();
    let source = try!(cx.sources.get(id).require(|| {
        internal(format!("Missing package source for: {}", id))
    }));
    source.fingerprint(pkg)
}

fn hash_targets(targets: &[&Target]) -> u64 {
    let hasher = SipHasher::new_with_keys(0,0);
    let targets = targets.iter().map(|t| (*t).clone()).collect::<Vec<Target>>();
    hasher.hash(&targets)
}

fn fingerprint(package: String, profiles: u64) -> String {
    let hasher = SipHasher::new_with_keys(0,0);
    util::to_hex(hasher.hash(&(package, profiles)))
}
