//! Support for locking the package and index caches.
//!
//! This implements locking on the package and index caches (source files,
//! `.crate` files, and index caches) to coordinate when multiple cargos are
//! running at the same time.
//!
//! ## Usage
//!
//! There is a global [`CacheLocker`] held inside cargo's venerable
//! [`Config`]. The `CacheLocker` manages creating and tracking the locks
//! being held. There are methods on `Config` for managing the locks:
//!
//! - [`Config::acquire_package_cache_lock`] --- Acquires a lock. May block if
//!   another process holds a lock.
//! - [`Config::try_acquire_package_cache_lock`] --- Acquires a lock, returning
//!   immediately if it would block.
//! - [`Config::assert_package_cache_locked`] --- This is used to ensure the
//!   proper lock is being held.
//!
//! Lower-level code that accesses the package cache typically just use
//! `assert_package_cache_locked` to ensure that the correct lock is being
//! held. Higher-level code is responsible for acquiring the appropriate lock,
//! and holding it during the duration that it is performing its operation.
//!
//! ## Types of locking
//!
//! There are three styles of locks:
//!
//! * [`CacheLockMode::DownloadExclusive`] -- This is an exclusive lock
//!   acquired while downloading packages and doing resolution.
//! * [`CacheLockMode::Shared`] -- This is a shared lock acquired while a
//!   build is running. In other words, whenever cargo just needs to read from
//!   the cache, it should hold this lock. This is here to ensure that no
//!   cargos are trying to read the source caches when cache garbage
//!   collection runs.
//! * [`CacheLockMode::MutateExclusive`] -- This is an exclusive lock acquired
//!   whenever needing to modify existing source files (for example, with
//!   cache garbage collection). This is acquired to make sure that no other
//!   cargo is reading from the cache.
//!
//! Importantly, a `DownloadExclusive` lock does *not* interfere with a
//! `Shared` lock. The download process generally does not modify source files
//! (it only adds new ones), so other cargos should be able to safely proceed
//! in reading source files[^1].
//!
//! See the [`CacheLockMode`] enum docs for more details on when the different
//! modes should be used.
//!
//! ## Locking implementation details
//!
//! This is implemented by two separate lock files, the "download" one and the
//! "mutate" one. The `MutateExclusive` lock acquired both the "mutate" and
//! "download" locks. The `Shared` lock acquires the "mutate" lock in share
//! mode.
//!
//! An important rule is that `MutateExclusive` acquires the locks in the
//! order "mutate" first and then the "download". That helps prevent
//! deadlocks. It is not allowed for a cargo to first acquire a
//! `DownloadExclusive` lock and then a `Shared` lock because that would open
//! it up for deadlock.
//!
//! Another rule is that there should be only one [`CacheLocker`] per process
//! to uphold the ordering rules. You could in theory have multiple if you
//! could ensure that other threads would make progress and drop a lock, but
//! cargo is not architected that way.
//!
//! It is safe to recursively acquire a lock as many times as you want.
//!
//! ## Interaction with older cargos
//!
//! Before version 1.74, cargo only acquired the `DownloadExclusive` lock when
//! downloading and doing resolution. Newer cargos that acquire
//! `MutateExclusive` should still correctly block when an old cargo is
//! downloading (because it also acquires `DownloadExclusive`), but they do
//! not properly coordinate when an old cargo is in the build phase (because
//! it holds no locks). This isn't expected to be much of a problem because
//! the intended use of mutating the cache is only to delete old contents
//! which aren't currently being used. It is possible for there to be a
//! conflict, particularly if the user manually deletes the entire cache, but
//! it is not expected for this scenario to happen too often, and the only
//! consequence is that one side or the other encounters an error and needs to
//! retry.
//!
//! [^1]: A minor caveat is that downloads will delete an existing `src`
//!   directory if it was extracted via an old cargo. See
//!   [`crate::sources::registry::RegistrySource::unpack_package`]. This
//!   should probably be fixed, but is unlikely to be a problem if the user is
//!   only using versions of cargo with the same deletion logic.

use super::FileLock;
use crate::CargoResult;
use crate::Config;
use anyhow::Context;
use std::cell::RefCell;
use std::io;

/// The style of lock to acquire.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CacheLockMode {
    /// A `DownloadExclusive` lock ensures that only one cargo is doing
    /// resolution and downloading new packages.
    ///
    /// You should use this when downloading new packages or doing resolution.
    ///
    /// If another cargo has a `MutateExclusive` lock, then an attempt to get
    /// a `DownloadExclusive` lock will block.
    ///
    /// If another cargo has a `Shared` lock, then both can operate
    /// concurrently.
    DownloadExclusive,
    /// A `Shared` lock allows multiple cargos to read from the source files.
    ///
    /// You should use this when cargo is reading source files from the
    /// package cache. This is typically done during the build phase, since
    /// cargo only needs to read files during that time. This allows multiple
    /// cargo processes to build concurrently without interfering with one
    /// another, while guarding against other cargos using `MutateExclusive`.
    ///
    /// If another cargo has a `MutateExclusive` lock, then an attempt to get
    /// a `Shared` will block.
    ///
    /// If another cargo has a `DownloadExclusive` lock, then they both can
    /// operate concurrently under the assumption that downloading does not
    /// modify existing source files.
    Shared,
    /// A `MutateExclusive` lock ensures no other cargo is reading or writing
    /// from the package caches.
    ///
    /// You should use this when modifying existing files in the package
    /// cache. For example, things like garbage collection want to avoid
    /// deleting files while other cargos are trying to read (`Shared`) or
    /// resolve or download (`DownloadExclusive`).
    ///
    /// If another cargo has a `DownloadExclusive` or `Shared` lock, then this
    /// will block until they all release their locks.
    MutateExclusive,
}

/// Whether or not a lock attempt should block.
#[derive(Copy, Clone)]
enum BlockingMode {
    Blocking,
    NonBlocking,
}

use BlockingMode::*;

/// Whether or not a lock attempt blocked or succeeded.
#[derive(PartialEq, Copy, Clone)]
#[must_use]
enum LockingResult {
    LockAcquired,
    WouldBlock,
}

use LockingResult::*;

/// A file lock, with a counter to assist with recursive locking.
#[derive(Debug)]
struct RecursiveLock {
    /// The file lock.
    ///
    /// An important note is that locks can be `None` even when they are held.
    /// This can happen on things like old NFS mounts where locking isn't
    /// supported. We otherwise pretend we have a lock via the lock count. See
    /// [`FileLock`] for more detail on that.
    lock: Option<FileLock>,
    /// Number locks held, to support recursive locking.
    count: u32,
    /// If this is `true`, it is an exclusive lock, otherwise it is shared.
    is_exclusive: bool,
    /// The filename of the lock.
    filename: &'static str,
}

impl RecursiveLock {
    fn new(filename: &'static str) -> RecursiveLock {
        RecursiveLock {
            lock: None,
            count: 0,
            is_exclusive: false,
            filename,
        }
    }

    /// Low-level lock count increment routine.
    fn increment(&mut self) {
        self.count = self.count.checked_add(1).unwrap();
    }

    /// Unlocks a previously acquired lock.
    fn decrement(&mut self) {
        let new_cnt = self.count.checked_sub(1).unwrap();
        self.count = new_cnt;
        if new_cnt == 0 {
            // This will drop, releasing the lock.
            self.lock = None;
        }
    }

    /// Acquires a shared lock.
    fn lock_shared(
        &mut self,
        config: &Config,
        description: &'static str,
        blocking: BlockingMode,
    ) -> LockingResult {
        match blocking {
            Blocking => {
                self.lock_shared_blocking(config, description);
                LockAcquired
            }
            NonBlocking => self.lock_shared_nonblocking(config),
        }
    }

    /// Acquires a shared lock, blocking if held by another locker.
    fn lock_shared_blocking(&mut self, config: &Config, description: &'static str) {
        if self.count == 0 {
            self.is_exclusive = false;
            self.lock =
                match config
                    .home()
                    .open_ro_shared_create(self.filename, config, description)
                {
                    Ok(lock) => Some(lock),
                    Err(e) => {
                        // There is no error here because locking is mostly a
                        // best-effort attempt. If cargo home is read-only, we don't
                        // want to fail just because we couldn't create the lock file.
                        tracing::warn!("failed to acquire cache lock {}: {e:?}", self.filename);
                        None
                    }
                };
        }
        self.increment();
    }

    /// Acquires a shared lock, returns [`WouldBlock`] if held by another locker.
    fn lock_shared_nonblocking(&mut self, config: &Config) -> LockingResult {
        if self.count == 0 {
            self.is_exclusive = false;
            self.lock = match config.home().try_open_ro_shared_create(self.filename) {
                Ok(Some(lock)) => Some(lock),
                Ok(None) => {
                    return WouldBlock;
                }
                Err(e) => {
                    // Pretend that the lock was acquired (see lock_shared_blocking).
                    tracing::warn!("failed to acquire cache lock {}: {e:?}", self.filename);
                    None
                }
            };
        }
        self.increment();
        LockAcquired
    }

    /// Acquires an exclusive lock.
    fn lock_exclusive(
        &mut self,
        config: &Config,
        description: &'static str,
        blocking: BlockingMode,
    ) -> CargoResult<LockingResult> {
        if self.count > 0 && !self.is_exclusive {
            // Lock upgrades are dicey. It might be possible to support
            // this but would take a bit of work, and so far it isn't
            // needed.
            panic!("lock upgrade from shared to exclusive not supported");
        }
        match blocking {
            Blocking => {
                self.lock_exclusive_blocking(config, description)?;
                Ok(LockAcquired)
            }
            NonBlocking => self.lock_exclusive_nonblocking(config),
        }
    }

    /// Acquires an exclusive lock, blocking if held by another locker.
    fn lock_exclusive_blocking(
        &mut self,
        config: &Config,
        description: &'static str,
    ) -> CargoResult<()> {
        if self.count == 0 {
            self.is_exclusive = true;
            match config
                .home()
                .open_rw_exclusive_create(self.filename, config, description)
            {
                Ok(lock) => self.lock = Some(lock),
                Err(e) => {
                    if maybe_readonly(&e) {
                        // This is a best-effort attempt to at least try to
                        // acquire some sort of lock. This can help in the
                        // situation where this cargo only has read-only access,
                        // but maybe some other cargo has read-write. This will at
                        // least attempt to coordinate with it.
                        //
                        // We don't want to fail on a read-only mount because
                        // cargo grabs an exclusive lock in situations where it
                        // may only be reading from the package cache. In that
                        // case, cargo isn't writing anything, and we don't want
                        // to fail on that.
                        self.lock_shared_blocking(config, description);
                        // This has to pretend it is exclusive for recursive locks to work.
                        self.is_exclusive = true;
                        return Ok(());
                    } else {
                        return Err(e).with_context(|| "failed to acquire package cache lock");
                    }
                }
            }
        }
        self.increment();
        Ok(())
    }

    /// Acquires an exclusive lock, returns [`WouldBlock`] if held by another locker.
    fn lock_exclusive_nonblocking(&mut self, config: &Config) -> CargoResult<LockingResult> {
        if self.count == 0 {
            self.is_exclusive = true;
            match config.home().try_open_rw_exclusive_create(self.filename) {
                Ok(Some(lock)) => self.lock = Some(lock),
                Ok(None) => return Ok(WouldBlock),
                Err(e) => {
                    if maybe_readonly(&e) {
                        let result = self.lock_shared_nonblocking(config);
                        // This has to pretend it is exclusive for recursive locks to work.
                        self.is_exclusive = true;
                        return Ok(result);
                    } else {
                        return Err(e).with_context(|| "failed to acquire package cache lock");
                    }
                }
            }
        }
        self.increment();
        Ok(LockAcquired)
    }
}

/// The state of the [`CacheLocker`].
#[derive(Debug)]
struct CacheState {
    /// The cache lock guards the package cache used for download and
    /// resolution (append operations that should not interfere with reading
    /// from existing src files).
    cache_lock: RecursiveLock,
    /// The mutate lock is used to either guard the entire package cache for
    /// destructive modifications (in exclusive mode), or for reading the
    /// package cache src files (in shared mode).
    ///
    /// Note that [`CacheLockMode::MutateExclusive`] holds both
    /// [`CacheState::mutate_lock`] and [`CacheState::cache_lock`].
    mutate_lock: RecursiveLock,
}

impl CacheState {
    fn lock(
        &mut self,
        config: &Config,
        mode: CacheLockMode,
        blocking: BlockingMode,
    ) -> CargoResult<LockingResult> {
        use CacheLockMode::*;
        if mode == Shared && self.cache_lock.count > 0 && self.mutate_lock.count == 0 {
            // Shared lock, when a DownloadExclusive is held.
            //
            // This isn't supported because it could cause a deadlock. If
            // one cargo is attempting to acquire a MutateExclusive lock,
            // and acquires the mutate lock, but is blocked on the
            // download lock, and the cargo that holds the download lock
            // attempts to get a shared lock, they would end up blocking
            // each other.
            panic!("shared lock while holding download lock is not allowed");
        }
        match mode {
            Shared => {
                if self.mutate_lock.lock_shared(config, SHARED_DESCR, blocking) == WouldBlock {
                    return Ok(WouldBlock);
                }
            }
            DownloadExclusive => {
                if self
                    .cache_lock
                    .lock_exclusive(config, DOWNLOAD_EXCLUSIVE_DESCR, blocking)?
                    == WouldBlock
                {
                    return Ok(WouldBlock);
                }
            }
            MutateExclusive => {
                if self
                    .mutate_lock
                    .lock_exclusive(config, MUTATE_EXCLUSIVE_DESCR, blocking)?
                    == WouldBlock
                {
                    return Ok(WouldBlock);
                }

                // Part of the contract of MutateExclusive is that it doesn't
                // allow any processes to have a lock on the package cache, so
                // this acquires both locks.
                match self
                    .cache_lock
                    .lock_exclusive(config, DOWNLOAD_EXCLUSIVE_DESCR, blocking)
                {
                    Ok(LockAcquired) => {}
                    Ok(WouldBlock) => return Ok(WouldBlock),
                    Err(e) => {
                        self.mutate_lock.decrement();
                        return Err(e);
                    }
                }
            }
        }
        Ok(LockAcquired)
    }
}

/// A held lock guard.
///
/// When this is dropped, the lock will be released.
#[must_use]
pub struct CacheLock<'lock> {
    mode: CacheLockMode,
    locker: &'lock CacheLocker,
}

impl Drop for CacheLock<'_> {
    fn drop(&mut self) {
        use CacheLockMode::*;
        let mut state = self.locker.state.borrow_mut();
        match self.mode {
            Shared => {
                state.mutate_lock.decrement();
            }
            DownloadExclusive => {
                state.cache_lock.decrement();
            }
            MutateExclusive => {
                state.cache_lock.decrement();
                state.mutate_lock.decrement();
            }
        }
    }
}

/// The filename for the [`CacheLockMode::DownloadExclusive`] lock.
const CACHE_LOCK_NAME: &str = ".package-cache";
/// The filename for the [`CacheLockMode::MutateExclusive`] and
/// [`CacheLockMode::Shared`] lock.
const MUTATE_NAME: &str = ".package-cache-mutate";

// Descriptions that are displayed in the "Blocking" message shown to the user.
const SHARED_DESCR: &str = "shared package cache";
const DOWNLOAD_EXCLUSIVE_DESCR: &str = "package cache";
const MUTATE_EXCLUSIVE_DESCR: &str = "package cache mutation";

/// A locker that can be used to acquire locks.
///
/// See the [`crate::util::cache_lock`] module documentation for an overview
/// of how cache locking works.
#[derive(Debug)]
pub struct CacheLocker {
    /// The state of the locker.
    ///
    /// [`CacheLocker`] uses interior mutability because it is stuffed inside
    /// the global `Config`, which does not allow mutation.
    state: RefCell<CacheState>,
}

impl CacheLocker {
    /// Creates a new `CacheLocker`.
    pub fn new() -> CacheLocker {
        CacheLocker {
            state: RefCell::new(CacheState {
                cache_lock: RecursiveLock::new(CACHE_LOCK_NAME),
                mutate_lock: RecursiveLock::new(MUTATE_NAME),
            }),
        }
    }

    /// Acquires a lock with the given mode, possibly blocking if another
    /// cargo is holding the lock.
    pub fn lock(&self, config: &Config, mode: CacheLockMode) -> CargoResult<CacheLock<'_>> {
        let mut state = self.state.borrow_mut();
        let _ = state.lock(config, mode, Blocking)?;
        Ok(CacheLock { mode, locker: self })
    }

    /// Acquires a lock with the given mode, returning `None` if another cargo
    /// is holding the lock.
    pub fn try_lock(
        &self,
        config: &Config,
        mode: CacheLockMode,
    ) -> CargoResult<Option<CacheLock<'_>>> {
        let mut state = self.state.borrow_mut();
        if state.lock(config, mode, NonBlocking)? == LockAcquired {
            Ok(Some(CacheLock { mode, locker: self }))
        } else {
            Ok(None)
        }
    }

    /// Returns whether or not a lock is held for the given mode in this locker.
    ///
    /// This does not tell you whether or not it is locked in some other
    /// locker (such as in another process).
    ///
    /// Note that `Shared` will return true if a `MutateExclusive` lock is
    /// held, since `MutateExclusive` is just an upgraded `Shared`. Likewise,
    /// `DownlaodExclusive` will return true if a `MutateExclusive` lock is
    /// held since they overlap.
    pub fn is_locked(&self, mode: CacheLockMode) -> bool {
        let state = self.state.borrow();
        match (
            mode,
            state.cache_lock.count,
            state.mutate_lock.count,
            state.mutate_lock.is_exclusive,
        ) {
            (CacheLockMode::Shared, _, 1.., _) => true,
            (CacheLockMode::MutateExclusive, _, 1.., true) => true,
            (CacheLockMode::DownloadExclusive, 1.., _, _) => true,
            _ => false,
        }
    }
}

/// Returns whether or not the error appears to be from a read-only filesystem.
fn maybe_readonly(err: &anyhow::Error) -> bool {
    err.chain().any(|err| {
        if let Some(io) = err.downcast_ref::<io::Error>() {
            if io.kind() == io::ErrorKind::PermissionDenied {
                return true;
            }

            #[cfg(unix)]
            return io.raw_os_error() == Some(libc::EROFS);
        }

        false
    })
}
