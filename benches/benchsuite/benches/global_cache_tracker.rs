//! Benchmarks for the global cache tracker.

use cargo::core::global_cache_tracker::{self, DeferredGlobalLastUse, GlobalCacheTracker};
use cargo::util::GlobalContext;
use cargo::util::cache_lock::CacheLockMode;
use cargo::util::interning::InternedString;
use criterion::{Criterion, criterion_group, criterion_main};
use std::fs;
use std::path::{Path, PathBuf};

// Samples of real-world data.
const GLOBAL_CACHE_SAMPLE: &str = "global-cache-tracker/global-cache-sample";
const GLOBAL_CACHE_RANDOM: &str = "global-cache-tracker/random-sample";

/// A scratch directory where the benchmark can place some files.
fn root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    p.push("bench_global_cache_tracker");
    p
}

fn cargo_home() -> PathBuf {
    let mut p = root();
    p.push("chome");
    p
}

fn initialize_context() -> GlobalContext {
    // Set up config.
    let shell = cargo::core::Shell::new();
    let homedir = cargo_home();
    if !homedir.exists() {
        fs::create_dir_all(&homedir).unwrap();
    }
    let cwd = homedir.clone();
    let mut gctx = GlobalContext::new(shell, cwd, homedir);
    gctx.nightly_features_allowed = true;
    gctx.set_search_stop_path(root());
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
    // Set up database sample.
    let db_path = GlobalCacheTracker::db_path(&gctx).into_path_unlocked();
    if db_path.exists() {
        fs::remove_file(&db_path).unwrap();
    }
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join(GLOBAL_CACHE_SAMPLE);
    fs::copy(sample, &db_path).unwrap();
    gctx
}

/// Benchmarks how long it takes to initialize `GlobalCacheTracker` with an already
/// existing full database.
fn global_tracker_init(c: &mut Criterion) {
    let gctx = initialize_context();
    let _lock = gctx
        .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)
        .unwrap();
    c.bench_function("global_tracker_init", |b| {
        b.iter(|| {
            GlobalCacheTracker::new(&gctx).unwrap();
        })
    });
}

/// Benchmarks how long it takes to save a `GlobalCacheTracker` when there are zero
/// updates.
fn global_tracker_empty_save(c: &mut Criterion) {
    let gctx = initialize_context();
    let _lock = gctx
        .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)
        .unwrap();
    let mut deferred = DeferredGlobalLastUse::new();
    let mut tracker = GlobalCacheTracker::new(&gctx).unwrap();

    c.bench_function("global_tracker_empty_save", |b| {
        b.iter(|| {
            deferred.save(&mut tracker).unwrap();
        })
    });
}

fn load_random_sample() -> Vec<(InternedString, InternedString, u64)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(GLOBAL_CACHE_RANDOM);
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|s| {
            let mut s = s.split(',');
            (
                s.next().unwrap().into(),
                s.next().unwrap().into(),
                s.next().unwrap().parse().unwrap(),
            )
        })
        .collect()
}

/// Tests performance of updating the last-use timestamps in an already
/// populated database.
///
/// This runs for different sizes of number of crates to update (selecting
/// from the random sample stored on disk).
fn global_tracker_update(c: &mut Criterion) {
    let gctx = initialize_context();
    let _lock = gctx
        .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)
        .unwrap();
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join(GLOBAL_CACHE_SAMPLE);
    let db_path = GlobalCacheTracker::db_path(&gctx).into_path_unlocked();

    let random_sample = load_random_sample();

    let mut group = c.benchmark_group("global_tracker_update");
    for size in [1, 10, 100, 500] {
        if db_path.exists() {
            fs::remove_file(&db_path).unwrap();
        }

        fs::copy(&sample, &db_path).unwrap();
        let mut deferred = DeferredGlobalLastUse::new();
        let mut tracker = GlobalCacheTracker::new(&gctx).unwrap();
        group.bench_with_input(size.to_string(), &size, |b, &size| {
            b.iter(|| {
                for (encoded_registry_name, name, size) in &random_sample[..size] {
                    deferred.mark_registry_crate_used(global_cache_tracker::RegistryCrate {
                        encoded_registry_name: *encoded_registry_name,
                        crate_filename: format!("{}.crate", name).into(),
                        size: *size,
                    });
                    deferred.mark_registry_src_used(global_cache_tracker::RegistrySrc {
                        encoded_registry_name: *encoded_registry_name,
                        package_dir: *name,
                        size: Some(*size),
                    });
                }
                deferred.save(&mut tracker).unwrap();
            })
        });
    }
}

criterion_group!(
    benches,
    global_tracker_init,
    global_tracker_empty_save,
    global_tracker_update
);
criterion_main!(benches);
