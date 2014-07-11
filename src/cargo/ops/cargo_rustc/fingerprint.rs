use std::hash::Hasher;
use std::hash::sip::SipHasher;
use std::io::File;

use core::{Package, Target};
use util;
use util::{CargoResult, Fresh, Dirty, Freshness};

use super::job::Job;
use super::context::Context;

/// Calculates the fingerprint of a package's targets and prepares to write a
/// new fingerprint.
///
/// This function will first calculate the freshness of the package and return
/// it as the first part of the return tuple. It will then prepare a job to
/// update the fingerprint if this package is actually rebuilt as part of
/// compilation, returning the job as the second part of the tuple.
pub fn prepare(cx: &mut Context, pkg: &Package,
               targets: &[&Target]) -> CargoResult<(Freshness, Job)> {
    let fingerprint_loc = cx.dest().join(format!(".{}.fingerprint",
                                                 pkg.get_name()));

    let (is_fresh, fingerprint) = try!(is_fresh(pkg, &fingerprint_loc,
                                                cx, targets));
    let write_fingerprint = Job::new(proc() {
        try!(File::create(&fingerprint_loc).write_str(fingerprint.as_slice()));
        Ok(Vec::new())
    });
    Ok((if is_fresh {Fresh} else {Dirty}, write_fingerprint))
}

fn is_fresh(dep: &Package, loc: &Path, cx: &mut Context, targets: &[&Target])
            -> CargoResult<(bool, String)> {
    let new_pkg_fingerprint = format!("{}{}", cx.rustc_version,
                                  try!(dep.get_fingerprint(cx.config)));

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

fn hash_targets(targets: &[&Target]) -> u64 {
    let hasher = SipHasher::new_with_keys(0,0);
    let targets = targets.iter().map(|t| (*t).clone()).collect::<Vec<Target>>();
    hasher.hash(&targets)
}

fn fingerprint(package: String, profiles: u64) -> String {
    let hasher = SipHasher::new_with_keys(0,0);
    util::to_hex(hasher.hash(&(package, profiles)))
}
