//! Utility for capturing a global cache last-use database based on the files
//! on a real-world system.
//!
//! This will look in the `CARGO_HOME` of the current system and record last-use
//! data for all files in the cache. This is intended to provide a real-world
//! example for a benchmark that should be close to what a real set of data
//! should look like.
//!
//! See `benches/global_cache_tracker.rs` for the benchmark that uses this
//! data.
//!
//! The database is kept in git. It usually shouldn't need to be re-generated
//! unless there is a change in the schema or the benchmark.

use cargo::GlobalContext;
use cargo::core::global_cache_tracker::{self, DeferredGlobalLastUse, GlobalCacheTracker};
use cargo::util::cache_lock::CacheLockMode;
use rand::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    // Set up config.
    let shell = cargo::core::Shell::new();
    let homedir = Path::new(env!("CARGO_MANIFEST_DIR")).join("global-cache-tracker");
    let cwd = homedir.clone();
    let mut gctx = GlobalContext::new(shell, cwd, homedir.clone());
    gctx.configure(
        0,
        false,
        None,
        false,
        false,
        false,
        &None,
        &["gc".to_string()],
        &[],
    )
    .unwrap();
    let db_path = GlobalCacheTracker::db_path(&gctx).into_path_unlocked();
    if db_path.exists() {
        fs::remove_file(&db_path).unwrap();
    }

    let _lock = gctx
        .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)
        .unwrap();
    let mut deferred = DeferredGlobalLastUse::new();
    let mut tracker = GlobalCacheTracker::new(&gctx).unwrap();

    let real_home = cargo::util::homedir(&std::env::current_dir().unwrap()).unwrap();

    let cache_dir = real_home.join("registry/cache");
    for dir_ent in fs::read_dir(cache_dir).unwrap() {
        let registry = dir_ent.unwrap();
        let encoded_registry_name = registry.file_name().to_string_lossy().into();
        for krate in fs::read_dir(registry.path()).unwrap() {
            let krate = krate.unwrap();
            let meta = krate.metadata().unwrap();
            deferred.mark_registry_crate_used_stamp(
                global_cache_tracker::RegistryCrate {
                    encoded_registry_name,
                    crate_filename: krate.file_name().to_string_lossy().as_ref().into(),
                    size: meta.len(),
                },
                Some(&meta.modified().unwrap()),
            );
        }
    }

    let mut src_entries = Vec::new();

    let cache_dir = real_home.join("registry/src");
    for dir_ent in fs::read_dir(cache_dir).unwrap() {
        let registry = dir_ent.unwrap();
        let encoded_registry_name = registry.file_name().to_string_lossy().into();
        for krate in fs::read_dir(registry.path()).unwrap() {
            let krate = krate.unwrap();
            let meta = krate.metadata().unwrap();
            let src = global_cache_tracker::RegistrySrc {
                encoded_registry_name,
                package_dir: krate.file_name().to_string_lossy().as_ref().into(),
                size: Some(cargo_util::du(&krate.path(), &[]).unwrap()),
            };
            src_entries.push(src.clone());
            let timestamp = meta.modified().unwrap();
            deferred.mark_registry_src_used_stamp(src, Some(&timestamp));
        }
    }

    let git_co_dir = real_home.join("git/checkouts");
    for dir_ent in fs::read_dir(git_co_dir).unwrap() {
        let git_source = dir_ent.unwrap();
        let encoded_git_name = git_source.file_name().to_string_lossy().into();
        for co in fs::read_dir(git_source.path()).unwrap() {
            let co = co.unwrap();
            let meta = co.metadata().unwrap();
            deferred.mark_git_checkout_used_stamp(
                global_cache_tracker::GitCheckout {
                    encoded_git_name,
                    short_name: co.file_name().to_string_lossy().as_ref().into(),
                    size: Some(cargo_util::du(&co.path(), &[]).unwrap()),
                },
                Some(&meta.modified().unwrap()),
            );
        }
    }

    deferred.save(&mut tracker).unwrap();
    drop(deferred);
    drop(tracker);
    fs::rename(&db_path, homedir.join("global-cache-sample")).unwrap();
    // Clean up the lock file created above.
    fs::remove_file(homedir.join(".package-cache")).unwrap();

    // Save a random sample of crates that the benchmark should update.
    // Pick whichever registry has the most entries. This is to be somewhat
    // realistic for the common case that all dependencies come from one
    // registry (crates.io).
    let mut counts = HashMap::new();
    for src in &src_entries {
        let c: &mut u32 = counts.entry(src.encoded_registry_name).or_default();
        *c += 1;
    }
    let mut counts: Vec<_> = counts.into_iter().map(|(k, v)| (v, k)).collect();
    counts.sort();
    let biggest = counts.last().unwrap().1;

    src_entries.retain(|src| src.encoded_registry_name == biggest);
    let mut rng = &mut rand::rng();
    let sample: Vec<_> = src_entries.choose_multiple(&mut rng, 500).collect();
    let mut f = File::create(homedir.join("random-sample")).unwrap();
    for src in sample {
        writeln!(
            f,
            "{},{},{}",
            src.encoded_registry_name,
            src.package_dir,
            src.size.unwrap()
        )
        .unwrap();
    }
}
