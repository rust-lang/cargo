//! Support for tracking the last time files were used to assist with cleaning
//! up those files if they haven't been used in a while.
//!
//! Tracking of cache files is stored in a sqlite database which contains a
//! timestamp of the last time the file was used, as well as the size of the
//! file.
//!
//! While cargo is running, when it detects a use of a cache file, it adds a
//! timestamp to [`DeferredGlobalLastUse`]. This batches up a set of changes
//! that are then flushed to the database all at once (via
//! [`DeferredGlobalLastUse::save`]). Ideally saving would only be done once
//! for performance reasons, but that is not really possible due to the way
//! cargo works, since there are different ways cargo can be used (like `cargo
//! generate-lockfile`, `cargo fetch`, and `cargo build` are all very
//! different ways the code is used).
//!
//! All of the database interaction is done through the [`GlobalCacheTracker`]
//! type.
//!
//! There is a single global [`GlobalCacheTracker`] and
//! [`DeferredGlobalLastUse`] stored in [`GlobalContext`].
//!
//! The high-level interface for performing garbage collection is defined in
//! the [`crate::core::gc`] module. The functions there are responsible for
//! interacting with the [`GlobalCacheTracker`] to handle cleaning of global
//! cache data.
//!
//! ## Automatic gc
//!
//! Some commands (primarily the build commands) will trigger an automatic
//! deletion of files that haven't been used in a while. The high-level
//! interface for this is the [`crate::core::gc::auto_gc`] function.
//!
//! The [`GlobalCacheTracker`] database tracks the last time an automatic gc
//! was performed so that it is only done once per day for performance
//! reasons.
//!
//! ## Manual gc
//!
//! The user can perform a manual garbage collection with the `cargo clean`
//! command. That command has a variety of options to specify what to delete.
//! Manual gc supports deleting based on age or size or both. From a
//! high-level, this is done by the [`crate::core::gc::Gc::gc`] method, which
//! calls into [`GlobalCacheTracker`] to handle all the cleaning.
//!
//! ## Locking
//!
//! Usage of the database requires that the package cache is locked to prevent
//! concurrent access. Although sqlite has built-in locking support, we want
//! to use cargo's locking so that the "Blocking" message gets displayed, and
//! so that locks can block indefinitely for long-running build commands.
//! [`rusqlite`] has a default timeout of 5 seconds, though that is
//! configurable.
//!
//! When garbage collection is being performed, the package cache lock must be
//! in [`CacheLockMode::MutateExclusive`] to ensure no other cargo process is
//! running. See [`crate::util::cache_lock`] for more detail on locking.
//!
//! When performing automatic gc, [`crate::core::gc::auto_gc`] will skip the
//! GC if the package cache lock is already held by anything else. Automatic
//! GC is intended to be opportunistic, and should impose as little disruption
//! to the user as possible.
//!
//! ## Compatibility
//!
//! The database must retain both forwards and backwards compatibility between
//! different versions of cargo. For the most part, this shouldn't be too
//! difficult to maintain. Generally sqlite doesn't change on-disk formats
//! between versions (the introduction of WAL is one of the few examples where
//! version 3 had a format change, but we wouldn't use it anyway since it has
//! shared-memory requirements cargo can't depend on due to things like
//! network mounts).
//!
//! Schema changes must be managed through [`migrations`] by adding new
//! entries that make a change to the database. Changes must not break older
//! versions of cargo. Generally, adding columns should be fine (either with a
//! default value, or NULL). Adding tables should also be fine. Just don't do
//! destructive things like removing a column, or changing the semantics of an
//! existing column.
//!
//! Since users may run older versions of cargo that do not do cache tracking,
//! the [`GlobalCacheTracker::sync_db_with_files`] method helps dealing with
//! keeping the database in sync in the presence of older versions of cargo
//! touching the cache directories.
//!
//! ## Performance
//!
//! A lot of focus on the design of this system is to minimize the performance
//! impact. Every build command needs to save updates which we try to avoid
//! having a noticeable impact on build times. Systems like Windows,
//! particularly with a magnetic hard disk, can experience a fairly large
//! impact of cargo's overhead. Cargo's benchsuite has some benchmarks to help
//! compare different environments, or changes to the code here. Please try to
//! keep performance in mind if making any major changes.
//!
//! Performance of `cargo clean` is not quite as important since it is not
//! expected to be run often. However, it is still courteous to the user to
//! try to not impact it too much. One part that has a performance concern is
//! that the clean command will synchronize the database with whatever is on
//! disk if needed (in case files were added by older versions of cargo that
//! don't do cache tracking, or if the user manually deleted some files). This
//! can potentially be very slow, especially if the two are very out of sync.
//!
//! ## Filesystems
//!
//! Everything here is sensitive to the kind of filesystem it is running on.
//! People tend to run cargo in all sorts of strange environments that have
//! limited capabilities, or on things like read-only mounts. The code here
//! needs to gracefully handle as many situations as possible.
//!
//! See also the information in the [Performance](#performance) and
//! [Locking](#locking) sections when considering different filesystems and
//! their impact on performance and locking.
//!
//! There are checks for read-only filesystems, which is generally ignored.

use crate::core::Verbosity;
use crate::core::gc::GcOpts;
use crate::ops::CleanContext;
use crate::util::cache_lock::CacheLockMode;
use crate::util::interning::InternedString;
use crate::util::sqlite::{self, Migration, basic_migration};
use crate::util::{Filesystem, Progress, ProgressStyle};
use crate::{CargoResult, GlobalContext};
use anyhow::{Context as _, bail};
use cargo_util::paths;
use rusqlite::{Connection, ErrorCode, params};
use std::collections::{HashMap, hash_map};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::{debug, trace};

/// The filename of the database.
const GLOBAL_CACHE_FILENAME: &str = ".global-cache";

const REGISTRY_INDEX_TABLE: &str = "registry_index";
const REGISTRY_CRATE_TABLE: &str = "registry_crate";
const REGISTRY_SRC_TABLE: &str = "registry_src";
const GIT_DB_TABLE: &str = "git_db";
const GIT_CO_TABLE: &str = "git_checkout";

/// How often timestamps will be updated.
///
/// As an optimization timestamps are not updated unless they are older than
/// the given number of seconds. This helps reduce the amount of disk I/O when
/// running cargo multiple times within a short window.
const UPDATE_RESOLUTION: u64 = 60 * 5;

/// Type for timestamps as stored in the database.
///
/// These are seconds since the Unix epoch.
type Timestamp = u64;

/// The key for a registry index entry stored in the database.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegistryIndex {
    /// A unique name of the registry source.
    pub encoded_registry_name: InternedString,
}

/// The key for a registry `.crate` entry stored in the database.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegistryCrate {
    /// A unique name of the registry source.
    pub encoded_registry_name: InternedString,
    /// The filename of the compressed crate, like `foo-1.2.3.crate`.
    pub crate_filename: InternedString,
    /// The size of the `.crate` file.
    pub size: u64,
}

/// The key for a registry src directory entry stored in the database.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegistrySrc {
    /// A unique name of the registry source.
    pub encoded_registry_name: InternedString,
    /// The directory name of the extracted source, like `foo-1.2.3`.
    pub package_dir: InternedString,
    /// Total size of the src directory in bytes.
    ///
    /// This can be None when the size is unknown. For example, when the src
    /// directory already exists on disk, and we just want to update the
    /// last-use timestamp. We don't want to take the expense of computing disk
    /// usage unless necessary. [`GlobalCacheTracker::populate_untracked`]
    /// will handle any actual NULL values in the database, which can happen
    /// when the src directory is created by an older version of cargo that
    /// did not track sizes.
    pub size: Option<u64>,
}

/// The key for a git db entry stored in the database.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct GitDb {
    /// A unique name of the git database.
    pub encoded_git_name: InternedString,
}

/// The key for a git checkout entry stored in the database.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct GitCheckout {
    /// A unique name of the git database.
    pub encoded_git_name: InternedString,
    /// A unique name of the checkout without the database.
    pub short_name: InternedString,
    /// Total size of the checkout directory.
    ///
    /// This can be None when the size is unknown. See [`RegistrySrc::size`]
    /// for an explanation.
    pub size: Option<u64>,
}

/// Filesystem paths in the global cache.
///
/// Accessing these assumes a lock has already been acquired.
struct BasePaths {
    /// Root path to the index caches.
    index: PathBuf,
    /// Root path to the git DBs.
    git_db: PathBuf,
    /// Root path to the git checkouts.
    git_co: PathBuf,
    /// Root path to the `.crate` files.
    crate_dir: PathBuf,
    /// Root path to the `src` directories.
    src: PathBuf,
}

/// Migrations which initialize the database, and can be used to evolve it over time.
///
/// See [`Migration`] for more detail.
///
/// **Be sure to not change the order or entries here!**
fn migrations() -> Vec<Migration> {
    vec![
        // registry_index tracks the overall usage of an index cache, and tracks a
        // numeric ID to refer to that index that is used in other tables.
        basic_migration(
            "CREATE TABLE registry_index (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                timestamp INTEGER NOT NULL
            )",
        ),
        // .crate files
        basic_migration(
            "CREATE TABLE registry_crate (
                registry_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                size INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                PRIMARY KEY (registry_id, name),
                FOREIGN KEY (registry_id) REFERENCES registry_index (id) ON DELETE CASCADE
             )",
        ),
        // Extracted src directories
        //
        // Note that `size` can be NULL. This will happen when marking a src
        // directory as used that was created by an older version of cargo
        // that didn't do size tracking.
        basic_migration(
            "CREATE TABLE registry_src (
                registry_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                size INTEGER,
                timestamp INTEGER NOT NULL,
                PRIMARY KEY (registry_id, name),
                FOREIGN KEY (registry_id) REFERENCES registry_index (id) ON DELETE CASCADE
             )",
        ),
        // Git db directories
        basic_migration(
            "CREATE TABLE git_db (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                timestamp INTEGER NOT NULL
             )",
        ),
        // Git checkout directories
        basic_migration(
            "CREATE TABLE git_checkout (
                git_id INTEGER NOT NULL,
                name TEXT UNIQUE NOT NULL,
                size INTEGER,
                timestamp INTEGER NOT NULL,
                PRIMARY KEY (git_id, name),
                FOREIGN KEY (git_id) REFERENCES git_db (id) ON DELETE CASCADE
             )",
        ),
        // This is a general-purpose single-row table that can store arbitrary
        // data. Feel free to add columns (with ALTER TABLE) if necessary.
        basic_migration(
            "CREATE TABLE global_data (
                last_auto_gc INTEGER NOT NULL
            )",
        ),
        // last_auto_gc tracks the last time auto-gc was run (so that it only
        // runs roughly once a day for performance reasons). Prime it with the
        // current time to establish a baseline.
        Box::new(|conn| {
            conn.execute(
                "INSERT INTO global_data (last_auto_gc) VALUES (?1)",
                [now()],
            )?;
            Ok(())
        }),
    ]
}

/// Type for SQL columns that refer to the primary key of their parent table.
///
/// For example, `registry_crate.registry_id` refers to its parent `registry_index.id`.
#[derive(Copy, Clone, Debug, PartialEq)]
struct ParentId(i64);

impl rusqlite::types::FromSql for ParentId {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let i = i64::column_result(value)?;
        Ok(ParentId(i))
    }
}

impl rusqlite::types::ToSql for ParentId {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::from(self.0))
    }
}

/// Tracking for the global shared cache (registry files, etc.).
///
/// This is the interface to the global cache database, used for tracking and
/// cleaning. See the [`crate::core::global_cache_tracker`] module docs for
/// details.
#[derive(Debug)]
pub struct GlobalCacheTracker {
    /// Connection to the SQLite database.
    conn: Connection,
    /// This is an optimization used to make sure cargo only checks if gc
    /// needs to run once per session. This starts as `false`, and then the
    /// first time it checks if automatic gc needs to run, it will be set to
    /// `true`.
    auto_gc_checked_this_session: bool,
}

impl GlobalCacheTracker {
    /// Creates a new [`GlobalCacheTracker`].
    ///
    /// The caller is responsible for locking the package cache with
    /// [`CacheLockMode::DownloadExclusive`] before calling this.
    pub fn new(gctx: &GlobalContext) -> CargoResult<GlobalCacheTracker> {
        let db_path = Self::db_path(gctx);
        // A package cache lock is required to ensure only one cargo is
        // accessing at the same time. If there is concurrent access, we
        // want to rely on cargo's own "Blocking" system (which can
        // provide user feedback) rather than blocking inside sqlite
        // (which by default has a short timeout).
        let db_path = gctx.assert_package_cache_locked(CacheLockMode::DownloadExclusive, &db_path);
        let mut conn = Connection::open(db_path)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        sqlite::migrate(&mut conn, &migrations())?;
        Ok(GlobalCacheTracker {
            conn,
            auto_gc_checked_this_session: false,
        })
    }

    /// The path to the database.
    pub fn db_path(gctx: &GlobalContext) -> Filesystem {
        gctx.home().join(GLOBAL_CACHE_FILENAME)
    }

    /// Given an encoded registry name, returns its ID.
    ///
    /// Returns None if the given name isn't in the database.
    fn id_from_name(
        conn: &Connection,
        table_name: &str,
        encoded_name: &str,
    ) -> CargoResult<Option<ParentId>> {
        let mut stmt =
            conn.prepare_cached(&format!("SELECT id FROM {table_name} WHERE name = ?"))?;
        match stmt.query_row([encoded_name], |row| row.get(0)) {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Returns a map of ID to path for the given ids in the given table.
    ///
    /// For example, given `registry_index` IDs, it returns filenames of the
    /// form "index.crates.io-6f17d22bba15001f".
    fn get_id_map(
        conn: &Connection,
        table_name: &str,
        ids: &[i64],
    ) -> CargoResult<HashMap<i64, PathBuf>> {
        let mut stmt =
            conn.prepare_cached(&format!("SELECT name FROM {table_name} WHERE id = ?1"))?;
        ids.iter()
            .map(|id| {
                let name = stmt.query_row(params![id], |row| {
                    Ok(PathBuf::from(row.get::<_, String>(0)?))
                })?;
                Ok((*id, name))
            })
            .collect()
    }

    /// Returns all index cache timestamps.
    pub fn registry_index_all(&self) -> CargoResult<Vec<(RegistryIndex, Timestamp)>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT name, timestamp FROM registry_index")?;
        let rows = stmt
            .query_map([], |row| {
                let encoded_registry_name = row.get_unwrap(0);
                let timestamp = row.get_unwrap(1);
                let kind = RegistryIndex {
                    encoded_registry_name,
                };
                Ok((kind, timestamp))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Returns all registry crate cache timestamps.
    pub fn registry_crate_all(&self) -> CargoResult<Vec<(RegistryCrate, Timestamp)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT registry_index.name, registry_crate.name, registry_crate.size, registry_crate.timestamp
             FROM registry_index, registry_crate
             WHERE registry_crate.registry_id = registry_index.id",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let encoded_registry_name = row.get_unwrap(0);
                let crate_filename = row.get_unwrap(1);
                let size = row.get_unwrap(2);
                let timestamp = row.get_unwrap(3);
                let kind = RegistryCrate {
                    encoded_registry_name,
                    crate_filename,
                    size,
                };
                Ok((kind, timestamp))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Returns all registry source cache timestamps.
    pub fn registry_src_all(&self) -> CargoResult<Vec<(RegistrySrc, Timestamp)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT registry_index.name, registry_src.name, registry_src.size, registry_src.timestamp
             FROM registry_index, registry_src
             WHERE registry_src.registry_id = registry_index.id",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let encoded_registry_name = row.get_unwrap(0);
                let package_dir = row.get_unwrap(1);
                let size = row.get_unwrap(2);
                let timestamp = row.get_unwrap(3);
                let kind = RegistrySrc {
                    encoded_registry_name,
                    package_dir,
                    size,
                };
                Ok((kind, timestamp))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Returns all git db timestamps.
    pub fn git_db_all(&self) -> CargoResult<Vec<(GitDb, Timestamp)>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT name, timestamp FROM git_db")?;
        let rows = stmt
            .query_map([], |row| {
                let encoded_git_name = row.get_unwrap(0);
                let timestamp = row.get_unwrap(1);
                let kind = GitDb { encoded_git_name };
                Ok((kind, timestamp))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Returns all git checkout timestamps.
    pub fn git_checkout_all(&self) -> CargoResult<Vec<(GitCheckout, Timestamp)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT git_db.name, git_checkout.name, git_checkout.size, git_checkout.timestamp
             FROM git_db, git_checkout
             WHERE git_checkout.git_id = git_db.id",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let encoded_git_name = row.get_unwrap(0);
                let short_name = row.get_unwrap(1);
                let size = row.get_unwrap(2);
                let timestamp = row.get_unwrap(3);
                let kind = GitCheckout {
                    encoded_git_name,
                    short_name,
                    size,
                };
                Ok((kind, timestamp))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Returns whether or not an auto GC should be performed, compared to the
    /// last time it was recorded in the database.
    pub fn should_run_auto_gc(&mut self, frequency: Duration) -> CargoResult<bool> {
        trace!(target: "gc", "should_run_auto_gc");
        if self.auto_gc_checked_this_session {
            return Ok(false);
        }
        let last_auto_gc: Timestamp =
            self.conn
                .query_row("SELECT last_auto_gc FROM global_data", [], |row| row.get(0))?;
        let should_run = last_auto_gc + frequency.as_secs() < now();
        trace!(target: "gc",
            "last auto gc was {}, {}",
            last_auto_gc,
            if should_run { "running" } else { "skipping" }
        );
        self.auto_gc_checked_this_session = true;
        Ok(should_run)
    }

    /// Writes to the database to indicate that an automatic GC has just been
    /// completed.
    pub fn set_last_auto_gc(&self) -> CargoResult<()> {
        self.conn
            .execute("UPDATE global_data SET last_auto_gc = ?1", [now()])?;
        Ok(())
    }

    /// Deletes files from the global cache based on the given options.
    pub fn clean(&mut self, clean_ctx: &mut CleanContext<'_>, gc_opts: &GcOpts) -> CargoResult<()> {
        self.clean_inner(clean_ctx, gc_opts)
            .context("failed to clean entries from the global cache")
    }

    #[tracing::instrument(skip_all)]
    fn clean_inner(
        &mut self,
        clean_ctx: &mut CleanContext<'_>,
        gc_opts: &GcOpts,
    ) -> CargoResult<()> {
        let gctx = clean_ctx.gctx;
        let base = BasePaths {
            index: gctx.registry_index_path().into_path_unlocked(),
            git_db: gctx.git_db_path().into_path_unlocked(),
            git_co: gctx.git_checkouts_path().into_path_unlocked(),
            crate_dir: gctx.registry_cache_path().into_path_unlocked(),
            src: gctx.registry_source_path().into_path_unlocked(),
        };
        let now = now();
        trace!(target: "gc", "cleaning {gc_opts:?}");
        let tx = self.conn.transaction()?;
        let mut delete_paths = Vec::new();
        // This can be an expensive operation, so only perform it if necessary.
        if gc_opts.is_download_cache_opt_set() {
            // TODO: Investigate how slow this might be.
            Self::sync_db_with_files(
                &tx,
                now,
                gctx,
                &base,
                gc_opts.is_download_cache_size_set(),
                &mut delete_paths,
            )
            .context("failed to sync tracking database")?
        }
        if let Some(max_age) = gc_opts.max_index_age {
            let max_age = now - max_age.as_secs();
            Self::get_registry_index_to_clean(&tx, max_age, &base, &mut delete_paths)?;
        }
        if let Some(max_age) = gc_opts.max_src_age {
            let max_age = now - max_age.as_secs();
            Self::get_registry_items_to_clean_age(
                &tx,
                max_age,
                REGISTRY_SRC_TABLE,
                &base.src,
                &mut delete_paths,
            )?;
        }
        if let Some(max_age) = gc_opts.max_crate_age {
            let max_age = now - max_age.as_secs();
            Self::get_registry_items_to_clean_age(
                &tx,
                max_age,
                REGISTRY_CRATE_TABLE,
                &base.crate_dir,
                &mut delete_paths,
            )?;
        }
        if let Some(max_age) = gc_opts.max_git_db_age {
            let max_age = now - max_age.as_secs();
            Self::get_git_db_items_to_clean(&tx, max_age, &base, &mut delete_paths)?;
        }
        if let Some(max_age) = gc_opts.max_git_co_age {
            let max_age = now - max_age.as_secs();
            Self::get_git_co_items_to_clean(&tx, max_age, &base.git_co, &mut delete_paths)?;
        }
        // Size collection must happen after date collection so that dates
        // have precedence, since size constraints are a more blunt
        // instrument.
        //
        // These are also complicated by the `--max-download-size` option
        // overlapping with `--max-crate-size` and `--max-src-size`, which
        // requires some coordination between those options which isn't
        // necessary with the age-based options. An item's age is either older
        // or it isn't, but contrast that with size which is based on the sum
        // of all tracked items. Also, `--max-download-size` is summed against
        // both the crate and src tracking, which requires combining them to
        // compute the size, and then separating them to calculate the correct
        // paths.
        if let Some(max_size) = gc_opts.max_crate_size {
            Self::get_registry_items_to_clean_size(
                &tx,
                max_size,
                REGISTRY_CRATE_TABLE,
                &base.crate_dir,
                &mut delete_paths,
            )?;
        }
        if let Some(max_size) = gc_opts.max_src_size {
            Self::get_registry_items_to_clean_size(
                &tx,
                max_size,
                REGISTRY_SRC_TABLE,
                &base.src,
                &mut delete_paths,
            )?;
        }
        if let Some(max_size) = gc_opts.max_git_size {
            Self::get_git_items_to_clean_size(&tx, max_size, &base, &mut delete_paths)?;
        }
        if let Some(max_size) = gc_opts.max_download_size {
            Self::get_registry_items_to_clean_size_both(&tx, max_size, &base, &mut delete_paths)?;
        }

        clean_ctx.remove_paths(&delete_paths)?;

        if clean_ctx.dry_run {
            tx.rollback()?;
        } else {
            tx.commit()?;
        }
        Ok(())
    }

    /// Returns a list of directory entries in the given path that are
    /// themselves directories.
    fn list_dir_names(path: &Path) -> CargoResult<Vec<String>> {
        Self::read_dir_with_filter(path, &|entry| {
            entry.file_type().map_or(false, |ty| ty.is_dir())
        })
    }

    /// Returns a list of names in a directory, filtered by the given callback.
    fn read_dir_with_filter(
        path: &Path,
        filter: &dyn Fn(&std::fs::DirEntry) -> bool,
    ) -> CargoResult<Vec<String>> {
        let entries = match path.read_dir() {
            Ok(e) => e,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(Vec::new());
                } else {
                    return Err(
                        anyhow::Error::new(e).context(format!("failed to read path `{path:?}`"))
                    );
                }
            }
        };
        let names = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| filter(entry))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        Ok(names)
    }

    /// Synchronizes the database to match the files on disk.
    ///
    /// This performs the following cleanups:
    ///
    /// 1. Remove entries from the database that are missing on disk.
    /// 2. Adds missing entries to the database that are on disk (such as when
    ///    files are added by older versions of cargo).
    /// 3. Fills in the `size` column where it is NULL (such as when something
    ///    is added to disk by an older version of cargo, and one of the mark
    ///    functions marked it without knowing the size).
    ///
    ///    Size computations are only done if `sync_size` is set since it can
    ///    be a very expensive operation. This should only be set if the user
    ///    requested to clean based on the cache size.
    /// 4. Checks for orphaned files. For example, if there are `.crate` files
    ///    associated with an index that does not exist.
    ///
    ///    These orphaned files will be added to `delete_paths` so that the
    ///    caller can delete them.
    #[tracing::instrument(skip(conn, gctx, base, delete_paths))]
    fn sync_db_with_files(
        conn: &Connection,
        now: Timestamp,
        gctx: &GlobalContext,
        base: &BasePaths,
        sync_size: bool,
        delete_paths: &mut Vec<PathBuf>,
    ) -> CargoResult<()> {
        debug!(target: "gc", "starting db sync");
        // For registry_index and git_db, add anything that is missing in the db.
        Self::update_parent_for_missing_from_db(conn, now, REGISTRY_INDEX_TABLE, &base.index)?;
        Self::update_parent_for_missing_from_db(conn, now, GIT_DB_TABLE, &base.git_db)?;

        // For registry_crate, registry_src, and git_checkout, remove anything
        // from the db that isn't on disk.
        Self::update_db_for_removed(
            conn,
            REGISTRY_INDEX_TABLE,
            "registry_id",
            REGISTRY_CRATE_TABLE,
            &base.crate_dir,
        )?;
        Self::update_db_for_removed(
            conn,
            REGISTRY_INDEX_TABLE,
            "registry_id",
            REGISTRY_SRC_TABLE,
            &base.src,
        )?;
        Self::update_db_for_removed(conn, GIT_DB_TABLE, "git_id", GIT_CO_TABLE, &base.git_co)?;

        // For registry_index and git_db, remove anything from the db that
        // isn't on disk.
        //
        // This also collects paths for any child files that don't have their
        // respective parent on disk.
        Self::update_db_parent_for_removed_from_disk(
            conn,
            REGISTRY_INDEX_TABLE,
            &base.index,
            &[&base.crate_dir, &base.src],
            delete_paths,
        )?;
        Self::update_db_parent_for_removed_from_disk(
            conn,
            GIT_DB_TABLE,
            &base.git_db,
            &[&base.git_co],
            delete_paths,
        )?;

        // For registry_crate, registry_src, and git_checkout, add anything
        // that is missing in the db.
        Self::populate_untracked_crate(conn, now, &base.crate_dir)?;
        Self::populate_untracked(
            conn,
            now,
            gctx,
            REGISTRY_INDEX_TABLE,
            "registry_id",
            REGISTRY_SRC_TABLE,
            &base.src,
            sync_size,
        )?;
        Self::populate_untracked(
            conn,
            now,
            gctx,
            GIT_DB_TABLE,
            "git_id",
            GIT_CO_TABLE,
            &base.git_co,
            sync_size,
        )?;

        // Update any NULL sizes if needed.
        if sync_size {
            Self::update_null_sizes(
                conn,
                gctx,
                REGISTRY_INDEX_TABLE,
                "registry_id",
                REGISTRY_SRC_TABLE,
                &base.src,
            )?;
            Self::update_null_sizes(
                conn,
                gctx,
                GIT_DB_TABLE,
                "git_id",
                GIT_CO_TABLE,
                &base.git_co,
            )?;
        }
        Ok(())
    }

    /// For parent tables, add any entries that are on disk but aren't tracked in the db.
    #[tracing::instrument(skip(conn, now, base_path))]
    fn update_parent_for_missing_from_db(
        conn: &Connection,
        now: Timestamp,
        parent_table_name: &str,
        base_path: &Path,
    ) -> CargoResult<()> {
        trace!(target: "gc", "checking for untracked parent to add to {parent_table_name}");
        let names = Self::list_dir_names(base_path)?;

        let mut stmt = conn.prepare_cached(&format!(
            "INSERT INTO {parent_table_name} (name, timestamp)
                VALUES (?1, ?2)
                ON CONFLICT DO NOTHING",
        ))?;
        for name in names {
            stmt.execute(params![name, now])?;
        }
        Ok(())
    }

    /// Removes database entries for any files that are not on disk for the child tables.
    ///
    /// This could happen for example if the user manually deleted the file or
    /// any such scenario where the filesystem and db are out of sync.
    #[tracing::instrument(skip(conn, base_path))]
    fn update_db_for_removed(
        conn: &Connection,
        parent_table_name: &str,
        id_column_name: &str,
        table_name: &str,
        base_path: &Path,
    ) -> CargoResult<()> {
        trace!(target: "gc", "checking for db entries to remove from {table_name}");
        let mut select_stmt = conn.prepare_cached(&format!(
            "SELECT {table_name}.rowid, {parent_table_name}.name, {table_name}.name
             FROM {parent_table_name}, {table_name}
             WHERE {table_name}.{id_column_name} = {parent_table_name}.id",
        ))?;
        let mut delete_stmt =
            conn.prepare_cached(&format!("DELETE FROM {table_name} WHERE rowid = ?1"))?;
        let mut rows = select_stmt.query([])?;
        while let Some(row) = rows.next()? {
            let rowid: i64 = row.get_unwrap(0);
            let id_name: String = row.get_unwrap(1);
            let name: String = row.get_unwrap(2);
            if !base_path.join(id_name).join(name).exists() {
                delete_stmt.execute([rowid])?;
            }
        }
        Ok(())
    }

    /// Removes database entries for any files that are not on disk for the parent tables.
    #[tracing::instrument(skip(conn, base_path, child_base_paths, delete_paths))]
    fn update_db_parent_for_removed_from_disk(
        conn: &Connection,
        parent_table_name: &str,
        base_path: &Path,
        child_base_paths: &[&Path],
        delete_paths: &mut Vec<PathBuf>,
    ) -> CargoResult<()> {
        trace!(target: "gc", "checking for db entries to remove from {parent_table_name}");
        let mut select_stmt =
            conn.prepare_cached(&format!("SELECT rowid, name FROM {parent_table_name}"))?;
        let mut delete_stmt =
            conn.prepare_cached(&format!("DELETE FROM {parent_table_name} WHERE rowid = ?1"))?;
        let mut rows = select_stmt.query([])?;
        while let Some(row) = rows.next()? {
            let rowid: i64 = row.get_unwrap(0);
            let id_name: String = row.get_unwrap(1);
            if !base_path.join(&id_name).exists() {
                delete_stmt.execute([rowid])?;
                // Make sure any child data is also cleaned up.
                for child_base in child_base_paths {
                    let child_path = child_base.join(&id_name);
                    if child_path.exists() {
                        debug!(target: "gc", "removing orphaned path {child_path:?}");
                        delete_paths.push(child_path);
                    }
                }
            }
        }
        Ok(())
    }

    /// Updates the database to add any `.crate` files that are currently
    /// not tracked (such as when they are downloaded by an older version of
    /// cargo).
    #[tracing::instrument(skip(conn, now, base_path))]
    fn populate_untracked_crate(
        conn: &Connection,
        now: Timestamp,
        base_path: &Path,
    ) -> CargoResult<()> {
        trace!(target: "gc", "populating untracked crate files");
        let mut insert_stmt = conn.prepare_cached(
            "INSERT INTO registry_crate (registry_id, name, size, timestamp)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT DO NOTHING",
        )?;
        let index_names = Self::list_dir_names(&base_path)?;
        for index_name in index_names {
            let Some(id) = Self::id_from_name(conn, REGISTRY_INDEX_TABLE, &index_name)? else {
                // The id is missing from the database. This should be resolved
                // via update_db_parent_for_removed_from_disk.
                continue;
            };
            let index_path = base_path.join(index_name);
            let crates = Self::read_dir_with_filter(&index_path, &|entry| {
                entry.file_type().map_or(false, |ty| ty.is_file())
                    && entry
                        .file_name()
                        .to_str()
                        .map_or(false, |name| name.ends_with(".crate"))
            })?;
            for crate_name in crates {
                // Missing files should have already been taken care of by
                // update_db_for_removed.
                let size = paths::metadata(index_path.join(&crate_name))?.len();
                insert_stmt.execute(params![id, crate_name, size, now])?;
            }
        }
        Ok(())
    }

    /// Updates the database to add any files that are currently not tracked
    /// (such as when they are downloaded by an older version of cargo).
    #[tracing::instrument(skip(conn, now, gctx, base_path, populate_size))]
    fn populate_untracked(
        conn: &Connection,
        now: Timestamp,
        gctx: &GlobalContext,
        id_table_name: &str,
        id_column_name: &str,
        table_name: &str,
        base_path: &Path,
        populate_size: bool,
    ) -> CargoResult<()> {
        trace!(target: "gc", "populating untracked files for {table_name}");
        // Gather names (and make sure they are in the database).
        let id_names = Self::list_dir_names(&base_path)?;

        // This SELECT is used to determine if the directory is already
        // tracked. We don't want to do the expensive size computation unless
        // necessary.
        let mut select_stmt = conn.prepare_cached(&format!(
            "SELECT 1 FROM {table_name}
             WHERE {id_column_name} = ?1 AND name = ?2",
        ))?;
        let mut insert_stmt = conn.prepare_cached(&format!(
            "INSERT INTO {table_name} ({id_column_name}, name, size, timestamp)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT DO NOTHING",
        ))?;
        let mut progress = Progress::with_style("Scanning", ProgressStyle::Ratio, gctx);
        // Compute the size of any directory not in the database.
        for id_name in id_names {
            let Some(id) = Self::id_from_name(conn, id_table_name, &id_name)? else {
                // The id is missing from the database. This should be resolved
                // via update_db_parent_for_removed_from_disk.
                continue;
            };
            let index_path = base_path.join(id_name);
            let names = Self::list_dir_names(&index_path)?;
            let max = names.len();
            for (i, name) in names.iter().enumerate() {
                if select_stmt.exists(params![id, name])? {
                    continue;
                }
                let dir_path = index_path.join(name);
                if !dir_path.is_dir() {
                    continue;
                }
                progress.tick(i, max, "")?;
                let size = if populate_size {
                    Some(du(&dir_path, table_name)?)
                } else {
                    None
                };
                insert_stmt.execute(params![id, name, size, now])?;
            }
        }
        Ok(())
    }

    /// Fills in the `size` column where it is NULL.
    ///
    /// This can happen when something is added to disk by an older version of
    /// cargo, and one of the mark functions marked it without knowing the
    /// size.
    ///
    /// `update_db_for_removed` should be called before this is called.
    #[tracing::instrument(skip(conn, gctx, base_path))]
    fn update_null_sizes(
        conn: &Connection,
        gctx: &GlobalContext,
        parent_table_name: &str,
        id_column_name: &str,
        table_name: &str,
        base_path: &Path,
    ) -> CargoResult<()> {
        trace!(target: "gc", "updating NULL size information in {table_name}");
        let mut null_stmt = conn.prepare_cached(&format!(
            "SELECT {table_name}.rowid, {table_name}.name, {parent_table_name}.name
             FROM {table_name}, {parent_table_name}
             WHERE {table_name}.size IS NULL AND {table_name}.{id_column_name} = {parent_table_name}.id",
        ))?;
        let mut update_stmt = conn.prepare_cached(&format!(
            "UPDATE {table_name} SET size = ?1 WHERE rowid = ?2"
        ))?;
        let mut progress = Progress::with_style("Scanning", ProgressStyle::Ratio, gctx);
        let rows: Vec<_> = null_stmt
            .query_map([], |row| {
                Ok((row.get_unwrap(0), row.get_unwrap(1), row.get_unwrap(2)))
            })?
            .collect();
        let max = rows.len();
        for (i, row) in rows.into_iter().enumerate() {
            let (rowid, name, id_name): (i64, String, String) = row?;
            let path = base_path.join(id_name).join(name);
            progress.tick(i, max, "")?;
            // Missing files should have already been taken care of by
            // update_db_for_removed.
            let size = du(&path, table_name)?;
            update_stmt.execute(params![size, rowid])?;
        }
        Ok(())
    }

    /// Adds paths to delete from either `registry_crate` or `registry_src` whose
    /// last use is older than the given timestamp.
    fn get_registry_items_to_clean_age(
        conn: &Connection,
        max_age: Timestamp,
        table_name: &str,
        base_path: &Path,
        delete_paths: &mut Vec<PathBuf>,
    ) -> CargoResult<()> {
        debug!(target: "gc", "cleaning {table_name} since {max_age:?}");
        let mut stmt = conn.prepare_cached(&format!(
            "DELETE FROM {table_name} WHERE timestamp < ?1
                RETURNING registry_id, name"
        ))?;
        let rows = stmt
            .query_map(params![max_age], |row| {
                let registry_id = row.get_unwrap(0);
                let name: String = row.get_unwrap(1);
                Ok((registry_id, name))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let ids: Vec<_> = rows.iter().map(|r| r.0).collect();
        let id_map = Self::get_id_map(conn, REGISTRY_INDEX_TABLE, &ids)?;
        for (id, name) in rows {
            let encoded_registry_name = &id_map[&id];
            delete_paths.push(base_path.join(encoded_registry_name).join(name));
        }
        Ok(())
    }

    /// Adds paths to delete from either `registry_crate` or `registry_src` in
    /// order to keep the total size under the given max size.
    fn get_registry_items_to_clean_size(
        conn: &Connection,
        max_size: u64,
        table_name: &str,
        base_path: &Path,
        delete_paths: &mut Vec<PathBuf>,
    ) -> CargoResult<()> {
        debug!(target: "gc", "cleaning {table_name} till under {max_size:?}");
        let total_size: u64 = conn.query_row(
            &format!("SELECT coalesce(SUM(size), 0) FROM {table_name}"),
            [],
            |row| row.get(0),
        )?;
        if total_size <= max_size {
            return Ok(());
        }
        // This SQL statement selects all of the rows ordered by timestamp,
        // and then uses a window function to keep a running total of the
        // size. It selects all rows until the running total exceeds the
        // threshold of the total number of bytes that we want to delete.
        //
        // The window function essentially computes an aggregate over all
        // previous rows as it goes along. As long as the running size is
        // below the total amount that we need to delete, it keeps picking
        // more rows.
        //
        // The ORDER BY includes `name` mainly for test purposes so that
        // entries with the same timestamp have deterministic behavior.
        //
        // The coalesce helps convert NULL to 0.
        let mut stmt = conn.prepare(&format!(
            "DELETE FROM {table_name} WHERE rowid IN \
                (SELECT x.rowid FROM \
                    (SELECT rowid, size, SUM(size) OVER \
                        (ORDER BY timestamp, name ROWS UNBOUNDED PRECEDING) AS running_amount \
                        FROM {table_name}) x \
                    WHERE coalesce(x.running_amount, 0) - x.size < ?1) \
                RETURNING registry_id, name;"
        ))?;
        let rows = stmt
            .query_map(params![total_size - max_size], |row| {
                let id = row.get_unwrap(0);
                let name: String = row.get_unwrap(1);
                Ok((id, name))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        // Convert registry_id to the encoded registry name, and join those.
        let ids: Vec<_> = rows.iter().map(|r| r.0).collect();
        let id_map = Self::get_id_map(conn, REGISTRY_INDEX_TABLE, &ids)?;
        for (id, name) in rows {
            let encoded_name = &id_map[&id];
            delete_paths.push(base_path.join(encoded_name).join(name));
        }
        Ok(())
    }

    /// Adds paths to delete from both `registry_crate` and `registry_src` in
    /// order to keep the total size under the given max size.
    fn get_registry_items_to_clean_size_both(
        conn: &Connection,
        max_size: u64,
        base: &BasePaths,
        delete_paths: &mut Vec<PathBuf>,
    ) -> CargoResult<()> {
        debug!(target: "gc", "cleaning download till under {max_size:?}");

        // This SQL statement selects from both registry_src and
        // registry_crate so that sorting of timestamps incorporates both of
        // them at the same time. It uses a const value of 1 or 2 as the first
        // column so that the code below can determine which table the value
        // came from.
        let mut stmt = conn.prepare_cached(
            "SELECT 1, registry_src.rowid, registry_src.name AS name, registry_index.name,
                    registry_src.size, registry_src.timestamp AS timestamp
             FROM registry_src, registry_index
             WHERE registry_src.registry_id = registry_index.id AND registry_src.size NOT NULL

             UNION

             SELECT 2, registry_crate.rowid, registry_crate.name AS name, registry_index.name,
                    registry_crate.size, registry_crate.timestamp AS timestamp
             FROM registry_crate, registry_index
             WHERE registry_crate.registry_id = registry_index.id

             ORDER BY timestamp, name",
        )?;
        let mut delete_src_stmt =
            conn.prepare_cached("DELETE FROM registry_src WHERE rowid = ?1")?;
        let mut delete_crate_stmt =
            conn.prepare_cached("DELETE FROM registry_crate WHERE rowid = ?1")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get_unwrap(0),
                    row.get_unwrap(1),
                    row.get_unwrap(2),
                    row.get_unwrap(3),
                    row.get_unwrap(4),
                ))
            })?
            .collect::<Result<Vec<(i64, i64, String, String, u64)>, _>>()?;
        let mut total_size: u64 = rows.iter().map(|r| r.4).sum();
        debug!(target: "gc", "total download cache size appears to be {total_size}");
        for (table, rowid, name, index_name, size) in rows {
            if total_size <= max_size {
                break;
            }
            if table == 1 {
                delete_paths.push(base.src.join(index_name).join(name));
                delete_src_stmt.execute([rowid])?;
            } else {
                delete_paths.push(base.crate_dir.join(index_name).join(name));
                delete_crate_stmt.execute([rowid])?;
            }
            // TODO: If delete crate, ensure src is also deleted.
            total_size -= size;
        }
        Ok(())
    }

    /// Adds paths to delete from the git cache, keeping the total size under
    /// the give value.
    ///
    /// Paths are relative to the `git` directory in the cache directory.
    fn get_git_items_to_clean_size(
        conn: &Connection,
        max_size: u64,
        base: &BasePaths,
        delete_paths: &mut Vec<PathBuf>,
    ) -> CargoResult<()> {
        debug!(target: "gc", "cleaning git till under {max_size:?}");

        // Collect all the sizes from git_db and git_checkouts, and then sort them by timestamp.
        let mut stmt = conn.prepare_cached("SELECT rowid, name, timestamp FROM git_db")?;
        let mut git_info = stmt
            .query_map([], |row| {
                let rowid: i64 = row.get_unwrap(0);
                let name: String = row.get_unwrap(1);
                let timestamp: Timestamp = row.get_unwrap(2);
                // Size is added below so that the error doesn't need to be
                // converted to a rusqlite error.
                Ok((timestamp, rowid, None, name, 0))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for info in &mut git_info {
            let size = cargo_util::du(&base.git_db.join(&info.3), &[])?;
            info.4 = size;
        }

        let mut stmt = conn.prepare_cached(
            "SELECT git_checkout.rowid, git_db.name, git_checkout.name,
                git_checkout.size, git_checkout.timestamp
                FROM git_checkout, git_db
                WHERE git_checkout.git_id = git_db.id AND git_checkout.size NOT NULL",
        )?;
        let git_co_rows = stmt
            .query_map([], |row| {
                let rowid = row.get_unwrap(0);
                let db_name: String = row.get_unwrap(1);
                let name = row.get_unwrap(2);
                let size = row.get_unwrap(3);
                let timestamp = row.get_unwrap(4);
                Ok((timestamp, rowid, Some(db_name), name, size))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        git_info.extend(git_co_rows);

        // Sort by timestamp, and name. The name is included mostly for test
        // purposes so that entries with the same timestamp have deterministic
        // behavior.
        git_info.sort_by(|a, b| (b.0, &b.3).cmp(&(a.0, &a.3)));

        // Collect paths to delete.
        let mut delete_db_stmt = conn.prepare_cached("DELETE FROM git_db WHERE rowid = ?1")?;
        let mut delete_co_stmt =
            conn.prepare_cached("DELETE FROM git_checkout WHERE rowid = ?1")?;
        let mut total_size: u64 = git_info.iter().map(|r| r.4).sum();
        debug!(target: "gc", "total git cache size appears to be {total_size}");
        while let Some((_timestamp, rowid, db_name, name, size)) = git_info.pop() {
            if total_size <= max_size {
                break;
            }
            if let Some(db_name) = db_name {
                delete_paths.push(base.git_co.join(db_name).join(name));
                delete_co_stmt.execute([rowid])?;
                total_size -= size;
            } else {
                total_size -= size;
                delete_paths.push(base.git_db.join(&name));
                delete_db_stmt.execute([rowid])?;
                // If the db is deleted, then all the checkouts must be deleted.
                let mut i = 0;
                while i < git_info.len() {
                    if git_info[i].2.as_deref() == Some(name.as_ref()) {
                        let (_, rowid, db_name, name, size) = git_info.remove(i);
                        delete_paths.push(base.git_co.join(db_name.unwrap()).join(name));
                        delete_co_stmt.execute([rowid])?;
                        total_size -= size;
                    } else {
                        i += 1;
                    }
                }
            }
        }
        Ok(())
    }

    /// Adds paths to delete from `registry_index` whose last use is older
    /// than the given timestamp.
    fn get_registry_index_to_clean(
        conn: &Connection,
        max_age: Timestamp,
        base: &BasePaths,
        delete_paths: &mut Vec<PathBuf>,
    ) -> CargoResult<()> {
        debug!(target: "gc", "cleaning index since {max_age:?}");
        let mut stmt = conn.prepare_cached(
            "DELETE FROM registry_index WHERE timestamp < ?1
                RETURNING name",
        )?;
        let mut rows = stmt.query([max_age])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get_unwrap(0);
            delete_paths.push(base.index.join(&name));
            // Also delete .crate and src directories, since by definition
            // they cannot be used without their index.
            delete_paths.push(base.src.join(&name));
            delete_paths.push(base.crate_dir.join(&name));
        }
        Ok(())
    }

    /// Adds paths to delete from `git_checkout` whose last use is
    /// older than the given timestamp.
    fn get_git_co_items_to_clean(
        conn: &Connection,
        max_age: Timestamp,
        base_path: &Path,
        delete_paths: &mut Vec<PathBuf>,
    ) -> CargoResult<()> {
        debug!(target: "gc", "cleaning git co since {max_age:?}");
        let mut stmt = conn.prepare_cached(
            "DELETE FROM git_checkout WHERE timestamp < ?1
                RETURNING git_id, name",
        )?;
        let rows = stmt
            .query_map(params![max_age], |row| {
                let git_id = row.get_unwrap(0);
                let name: String = row.get_unwrap(1);
                Ok((git_id, name))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let ids: Vec<_> = rows.iter().map(|r| r.0).collect();
        let id_map = Self::get_id_map(conn, GIT_DB_TABLE, &ids)?;
        for (id, name) in rows {
            let encoded_git_name = &id_map[&id];
            delete_paths.push(base_path.join(encoded_git_name).join(name));
        }
        Ok(())
    }

    /// Adds paths to delete from `git_db` in order to keep the total size
    /// under the given max size.
    fn get_git_db_items_to_clean(
        conn: &Connection,
        max_age: Timestamp,
        base: &BasePaths,
        delete_paths: &mut Vec<PathBuf>,
    ) -> CargoResult<()> {
        debug!(target: "gc", "cleaning git db since {max_age:?}");
        let mut stmt = conn.prepare_cached(
            "DELETE FROM git_db WHERE timestamp < ?1
                RETURNING name",
        )?;
        let mut rows = stmt.query([max_age])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get_unwrap(0);
            delete_paths.push(base.git_db.join(&name));
            // Also delete checkout directories, since by definition they
            // cannot be used without their db.
            delete_paths.push(base.git_co.join(&name));
        }
        Ok(())
    }
}

/// Helper to generate the upsert for the parent tables.
///
/// This handles checking if the row already exists, and only updates the
/// timestamp it if it hasn't been updated recently. This also handles keeping
/// a cached map of the `id` value.
///
/// Unfortunately it is a bit tricky to share this code without a macro.
macro_rules! insert_or_update_parent {
    ($self:expr, $conn:expr, $table_name:expr, $timestamps_field:ident, $keys_field:ident, $encoded_name:ident) => {
        let mut select_stmt = $conn.prepare_cached(concat!(
            "SELECT id, timestamp FROM ",
            $table_name,
            " WHERE name = ?1"
        ))?;
        let mut insert_stmt = $conn.prepare_cached(concat!(
            "INSERT INTO ",
            $table_name,
            " (name, timestamp)
                VALUES (?1, ?2)
                ON CONFLICT DO UPDATE SET timestamp=excluded.timestamp
                RETURNING id",
        ))?;
        let mut update_stmt = $conn.prepare_cached(concat!(
            "UPDATE ",
            $table_name,
            " SET timestamp = ?1 WHERE id = ?2"
        ))?;
        for (parent, new_timestamp) in std::mem::take(&mut $self.$timestamps_field) {
            trace!(target: "gc",
                concat!("insert ", $table_name, " {:?} {}"),
                parent,
                new_timestamp
            );
            let mut rows = select_stmt.query([parent.$encoded_name])?;
            let id = if let Some(row) = rows.next()? {
                let id: ParentId = row.get_unwrap(0);
                let timestamp: Timestamp = row.get_unwrap(1);
                if timestamp < new_timestamp - UPDATE_RESOLUTION {
                    update_stmt.execute(params![new_timestamp, id])?;
                }
                id
            } else {
                insert_stmt.query_row(params![parent.$encoded_name, new_timestamp], |row| {
                    row.get(0)
                })?
            };
            match $self.$keys_field.entry(parent.$encoded_name) {
                hash_map::Entry::Occupied(o) => {
                    assert_eq!(*o.get(), id);
                }
                hash_map::Entry::Vacant(v) => {
                    v.insert(id);
                }
            }
        }
        return Ok(());
    };
}

/// This is a cache of modifications that will be saved to disk all at once
/// via the [`DeferredGlobalLastUse::save`] method.
///
/// This is here to improve performance.
#[derive(Debug)]
pub struct DeferredGlobalLastUse {
    /// Cache of registry keys, used for faster fetching.
    ///
    /// The key is the registry name (which is its directory name) and the
    /// value is the `id` in the `registry_index` table.
    registry_keys: HashMap<InternedString, ParentId>,
    /// Cache of git keys, used for faster fetching.
    ///
    /// The key is the git db name (which is its directory name) and the value
    /// is the `id` in the `git_db` table.
    git_keys: HashMap<InternedString, ParentId>,

    /// New registry index entries to insert.
    registry_index_timestamps: HashMap<RegistryIndex, Timestamp>,
    /// New registry `.crate` entries to insert.
    registry_crate_timestamps: HashMap<RegistryCrate, Timestamp>,
    /// New registry src directory entries to insert.
    registry_src_timestamps: HashMap<RegistrySrc, Timestamp>,
    /// New git db entries to insert.
    git_db_timestamps: HashMap<GitDb, Timestamp>,
    /// New git checkout entries to insert.
    git_checkout_timestamps: HashMap<GitCheckout, Timestamp>,
    /// This is used so that a warning about failing to update the database is
    /// only displayed once.
    save_err_has_warned: bool,
    /// The current time, used to improve performance to avoid accessing the
    /// clock hundreds of times.
    now: Timestamp,
}

impl DeferredGlobalLastUse {
    pub fn new() -> DeferredGlobalLastUse {
        DeferredGlobalLastUse {
            registry_keys: HashMap::new(),
            git_keys: HashMap::new(),
            registry_index_timestamps: HashMap::new(),
            registry_crate_timestamps: HashMap::new(),
            registry_src_timestamps: HashMap::new(),
            git_db_timestamps: HashMap::new(),
            git_checkout_timestamps: HashMap::new(),
            save_err_has_warned: false,
            now: now(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.registry_index_timestamps.is_empty()
            && self.registry_crate_timestamps.is_empty()
            && self.registry_src_timestamps.is_empty()
            && self.git_db_timestamps.is_empty()
            && self.git_checkout_timestamps.is_empty()
    }

    fn clear(&mut self) {
        self.registry_index_timestamps.clear();
        self.registry_crate_timestamps.clear();
        self.registry_src_timestamps.clear();
        self.git_db_timestamps.clear();
        self.git_checkout_timestamps.clear();
    }

    /// Indicates the given [`RegistryIndex`] has been used right now.
    pub fn mark_registry_index_used(&mut self, registry_index: RegistryIndex) {
        self.mark_registry_index_used_stamp(registry_index, None);
    }

    /// Indicates the given [`RegistryCrate`] has been used right now.
    ///
    /// Also implicitly marks the index used, too.
    pub fn mark_registry_crate_used(&mut self, registry_crate: RegistryCrate) {
        self.mark_registry_crate_used_stamp(registry_crate, None);
    }

    /// Indicates the given [`RegistrySrc`] has been used right now.
    ///
    /// Also implicitly marks the index used, too.
    pub fn mark_registry_src_used(&mut self, registry_src: RegistrySrc) {
        self.mark_registry_src_used_stamp(registry_src, None);
    }

    /// Indicates the given [`GitCheckout`] has been used right now.
    ///
    /// Also implicitly marks the git db used, too.
    pub fn mark_git_checkout_used(&mut self, git_checkout: GitCheckout) {
        self.mark_git_checkout_used_stamp(git_checkout, None);
    }

    /// Indicates the given [`RegistryIndex`] has been used with the given
    /// time (or "now" if `None`).
    pub fn mark_registry_index_used_stamp(
        &mut self,
        registry_index: RegistryIndex,
        timestamp: Option<&SystemTime>,
    ) {
        let timestamp = timestamp.map_or(self.now, to_timestamp);
        self.registry_index_timestamps
            .insert(registry_index, timestamp);
    }

    /// Indicates the given [`RegistryCrate`] has been used with the given
    /// time (or "now" if `None`).
    ///
    /// Also implicitly marks the index used, too.
    pub fn mark_registry_crate_used_stamp(
        &mut self,
        registry_crate: RegistryCrate,
        timestamp: Option<&SystemTime>,
    ) {
        let timestamp = timestamp.map_or(self.now, to_timestamp);
        let index = RegistryIndex {
            encoded_registry_name: registry_crate.encoded_registry_name,
        };
        self.registry_index_timestamps.insert(index, timestamp);
        self.registry_crate_timestamps
            .insert(registry_crate, timestamp);
    }

    /// Indicates the given [`RegistrySrc`] has been used with the given
    /// time (or "now" if `None`).
    ///
    /// Also implicitly marks the index used, too.
    pub fn mark_registry_src_used_stamp(
        &mut self,
        registry_src: RegistrySrc,
        timestamp: Option<&SystemTime>,
    ) {
        let timestamp = timestamp.map_or(self.now, to_timestamp);
        let index = RegistryIndex {
            encoded_registry_name: registry_src.encoded_registry_name,
        };
        self.registry_index_timestamps.insert(index, timestamp);
        self.registry_src_timestamps.insert(registry_src, timestamp);
    }

    /// Indicates the given [`GitCheckout`] has been used with the given
    /// time (or "now" if `None`).
    ///
    /// Also implicitly marks the git db used, too.
    pub fn mark_git_checkout_used_stamp(
        &mut self,
        git_checkout: GitCheckout,
        timestamp: Option<&SystemTime>,
    ) {
        let timestamp = timestamp.map_or(self.now, to_timestamp);
        let db = GitDb {
            encoded_git_name: git_checkout.encoded_git_name,
        };
        self.git_db_timestamps.insert(db, timestamp);
        self.git_checkout_timestamps.insert(git_checkout, timestamp);
    }

    /// Saves all of the deferred information to the database.
    ///
    /// This will also clear the state of `self`.
    #[tracing::instrument(skip_all)]
    pub fn save(&mut self, tracker: &mut GlobalCacheTracker) -> CargoResult<()> {
        trace!(target: "gc", "saving last-use data");
        if self.is_empty() {
            return Ok(());
        }
        let tx = tracker.conn.transaction()?;
        // These must run before the ones that refer to their IDs.
        self.insert_registry_index_from_cache(&tx)?;
        self.insert_git_db_from_cache(&tx)?;
        self.insert_registry_crate_from_cache(&tx)?;
        self.insert_registry_src_from_cache(&tx)?;
        self.insert_git_checkout_from_cache(&tx)?;
        tx.commit()?;
        trace!(target: "gc", "last-use save complete");
        Ok(())
    }

    /// Variant of [`DeferredGlobalLastUse::save`] that does not return an
    /// error.
    ///
    /// This will log or display a warning to the user.
    pub fn save_no_error(&mut self, gctx: &GlobalContext) {
        if let Err(e) = self.save_with_gctx(gctx) {
            // Because there is an assertion in auto-gc that checks if this is
            // empty, be sure to clear it so that assertion doesn't fail.
            self.clear();
            if !self.save_err_has_warned {
                if is_silent_error(&e) && gctx.shell().verbosity() != Verbosity::Verbose {
                    tracing::warn!("failed to save last-use data: {e:?}");
                } else {
                    crate::display_warning_with_error(
                        "failed to save last-use data\n\
                        This may prevent cargo from accurately tracking what is being \
                        used in its global cache. This information is used for \
                        automatically removing unused data in the cache.",
                        &e,
                        &mut gctx.shell(),
                    );
                    self.save_err_has_warned = true;
                }
            }
        }
    }

    fn save_with_gctx(&mut self, gctx: &GlobalContext) -> CargoResult<()> {
        let mut tracker = gctx.global_cache_tracker()?;
        self.save(&mut tracker)
    }

    /// Flushes all of the `registry_index_timestamps` to the database,
    /// clearing `registry_index_timestamps`.
    fn insert_registry_index_from_cache(&mut self, conn: &Connection) -> CargoResult<()> {
        insert_or_update_parent!(
            self,
            conn,
            "registry_index",
            registry_index_timestamps,
            registry_keys,
            encoded_registry_name
        );
    }

    /// Flushes all of the `git_db_timestamps` to the database,
    /// clearing `registry_index_timestamps`.
    fn insert_git_db_from_cache(&mut self, conn: &Connection) -> CargoResult<()> {
        insert_or_update_parent!(
            self,
            conn,
            "git_db",
            git_db_timestamps,
            git_keys,
            encoded_git_name
        );
    }

    /// Flushes all of the `registry_crate_timestamps` to the database,
    /// clearing `registry_index_timestamps`.
    fn insert_registry_crate_from_cache(&mut self, conn: &Connection) -> CargoResult<()> {
        let registry_crate_timestamps = std::mem::take(&mut self.registry_crate_timestamps);
        for (registry_crate, timestamp) in registry_crate_timestamps {
            trace!(target: "gc", "insert registry crate {registry_crate:?} {timestamp}");
            let registry_id = self.registry_id(conn, registry_crate.encoded_registry_name)?;
            let mut stmt = conn.prepare_cached(
                "INSERT INTO registry_crate (registry_id, name, size, timestamp)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT DO UPDATE SET timestamp=excluded.timestamp
                    WHERE timestamp < ?5
                 ",
            )?;
            stmt.execute(params![
                registry_id,
                registry_crate.crate_filename,
                registry_crate.size,
                timestamp,
                timestamp - UPDATE_RESOLUTION
            ])?;
        }
        Ok(())
    }

    /// Flushes all of the `registry_src_timestamps` to the database,
    /// clearing `registry_index_timestamps`.
    fn insert_registry_src_from_cache(&mut self, conn: &Connection) -> CargoResult<()> {
        let registry_src_timestamps = std::mem::take(&mut self.registry_src_timestamps);
        for (registry_src, timestamp) in registry_src_timestamps {
            trace!(target: "gc", "insert registry src {registry_src:?} {timestamp}");
            let registry_id = self.registry_id(conn, registry_src.encoded_registry_name)?;
            let mut stmt = conn.prepare_cached(
                "INSERT INTO registry_src (registry_id, name, size, timestamp)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT DO UPDATE SET timestamp=excluded.timestamp
                    WHERE timestamp < ?5
                 ",
            )?;
            stmt.execute(params![
                registry_id,
                registry_src.package_dir,
                registry_src.size,
                timestamp,
                timestamp - UPDATE_RESOLUTION
            ])?;
        }

        Ok(())
    }

    /// Flushes all of the `git_checkout_timestamps` to the database,
    /// clearing `registry_index_timestamps`.
    fn insert_git_checkout_from_cache(&mut self, conn: &Connection) -> CargoResult<()> {
        let git_checkout_timestamps = std::mem::take(&mut self.git_checkout_timestamps);
        for (git_checkout, timestamp) in git_checkout_timestamps {
            let git_id = self.git_id(conn, git_checkout.encoded_git_name)?;
            let mut stmt = conn.prepare_cached(
                "INSERT INTO git_checkout (git_id, name, size, timestamp)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT DO UPDATE SET timestamp=excluded.timestamp
                    WHERE timestamp < ?5",
            )?;
            stmt.execute(params![
                git_id,
                git_checkout.short_name,
                git_checkout.size,
                timestamp,
                timestamp - UPDATE_RESOLUTION
            ])?;
        }

        Ok(())
    }

    /// Returns the numeric ID of the registry, either fetching from the local
    /// cache, or getting it from the database.
    ///
    /// It is an error if the registry does not exist.
    fn registry_id(
        &mut self,
        conn: &Connection,
        encoded_registry_name: InternedString,
    ) -> CargoResult<ParentId> {
        match self.registry_keys.get(&encoded_registry_name) {
            Some(i) => Ok(*i),
            None => {
                let Some(id) = GlobalCacheTracker::id_from_name(
                    conn,
                    REGISTRY_INDEX_TABLE,
                    &encoded_registry_name,
                )?
                else {
                    bail!(
                        "expected registry_index {encoded_registry_name} to exist, but wasn't found"
                    );
                };
                self.registry_keys.insert(encoded_registry_name, id);
                Ok(id)
            }
        }
    }

    /// Returns the numeric ID of the git db, either fetching from the local
    /// cache, or getting it from the database.
    ///
    /// It is an error if the git db does not exist.
    fn git_id(
        &mut self,
        conn: &Connection,
        encoded_git_name: InternedString,
    ) -> CargoResult<ParentId> {
        match self.git_keys.get(&encoded_git_name) {
            Some(i) => Ok(*i),
            None => {
                let Some(id) =
                    GlobalCacheTracker::id_from_name(conn, GIT_DB_TABLE, &encoded_git_name)?
                else {
                    bail!("expected git_db {encoded_git_name} to exist, but wasn't found")
                };
                self.git_keys.insert(encoded_git_name, id);
                Ok(id)
            }
        }
    }
}

/// Converts a [`SystemTime`] to a [`Timestamp`] which can be stored in the database.
fn to_timestamp(t: &SystemTime) -> Timestamp {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .expect("invalid clock")
        .as_secs()
}

/// Returns the current time.
///
/// This supports pretending that the time is different for testing using an
/// environment variable.
///
/// If possible, try to avoid calling this too often since accessing clocks
/// can be a little slow on some systems.
#[allow(clippy::disallowed_methods)]
fn now() -> Timestamp {
    match std::env::var("__CARGO_TEST_LAST_USE_NOW") {
        Ok(now) => now.parse().unwrap(),
        Err(_) => to_timestamp(&SystemTime::now()),
    }
}

/// Returns whether or not the given error should cause a warning to be
/// displayed to the user.
///
/// In some situations, like a read-only global cache, we don't want to spam
/// the user with a warning. I think once cargo has controllable lints, I
/// think we should consider changing this to always warn, but give the user
/// an option to silence the warning.
pub fn is_silent_error(e: &anyhow::Error) -> bool {
    if let Some(e) = e.downcast_ref::<rusqlite::Error>() {
        if matches!(
            e.sqlite_error_code(),
            Some(ErrorCode::CannotOpen | ErrorCode::ReadOnly)
        ) {
            return true;
        }
    }
    false
}

/// Returns the disk usage for a git checkout directory.
#[tracing::instrument]
fn du_git_checkout(path: &Path) -> CargoResult<u64> {
    // !.git is used because clones typically use hardlinks for the git
    // contents. TODO: Verify behavior on Windows.
    // TODO: Or even better, switch to worktrees, and remove this.
    cargo_util::du(&path, &["!.git"])
}

fn du(path: &Path, table_name: &str) -> CargoResult<u64> {
    if table_name == GIT_CO_TABLE {
        du_git_checkout(path)
    } else {
        cargo_util::du(&path, &[])
    }
}
