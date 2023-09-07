//! Support for locking the package and index caches.
//!
//! This implements locking on the package and index caches (source files,
//! `.crate` files, and index caches) to coordinate when multiple cargos are
//! running at the same time. There are three styles of locks:
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
//! `Shared` lock. The download process generally does not modify source
//! files, so other cargos should be able to safely proceed in reading source
//! files[^1].
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
#[derive(Copy, Clone, Debug)]
pub enum CacheLockMode {
    /// A `Shared` lock allows multiple cargos to read from the source files.
    ///
    /// If another cargo has a `MutateExclusive` lock, then an attempt to get
    /// a `Shared` will block.
    ///
    /// If another cargo has a `DownloadExclusive` lock, then the both can
    /// operate concurrently under the assumption that downloading does not
    /// modify existing source files.
    Shared,
    /// A `DownloadExclusive` lock ensures that only one cargo is downloading
    /// new packages.
    ///
    /// If another cargo has a `MutateExclusive` lock, then an attempt to get
    /// a `DownloadExclusive` lock will block.
    ///
    /// If another cargo has a `Shared` lock, then both can operate
    /// concurrently.
    DownloadExclusive,
    /// A `MutateExclusive` lock ensures no other cargo is reading or writing
    /// from the package caches.
    ///
    /// This is used for things like garbage collection to avoid modifying
    /// caches while other cargos are running.
    MutateExclusive,
}

/// A locker that can be used to acquire locks.
#[derive(Debug)]
pub struct CacheLocker {
    state: RefCell<CacheState>,
}

/// The state of the [`CacheLocker`].
///
/// [`CacheLocker`] uses interior mutability because it is stuffed inside the
/// global `Config`, which does not allow mutation.
///
/// An important note is that locks can be `None` even when they are held.
/// This can happen on things like old NFS mounts where locking isn't
/// supported. We otherwise pretend we have a lock via the lock counts. See
/// [`FileLock`] for more detail on that.
#[derive(Debug, Default)]
struct CacheState {
    cache_lock: Option<FileLock>,
    cache_lock_count: u32,
    mutate_lock: Option<FileLock>,
    mutate_lock_count: u32,
    /// Indicator of whether or not `mutate_lock` is currently a shared lock
    /// or an exclusive one.
    mutate_is_exclusive: bool,
}

/// A held lock guard.
///
/// When this is dropped, the lock will be released.
#[must_use]
pub struct CacheLock<'lock> {
    mode: CacheLockMode,
    locker: &'lock CacheLocker,
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

/// Macro to unlock a previously acquired lock.
macro_rules! decrement {
    ($state:expr, $count:ident, $lock:ident) => {
        let new_cnt = $state.$count.checked_sub(1).unwrap();
        $state.$count = new_cnt;
        if new_cnt == 0 {
            // This will drop, releasing the lock.
            $state.$lock = None;
        }
    };
}

/// Macro for acquiring a shared lock (can block).
macro_rules! do_shared_lock {
    ($self: ident, $config: expr, $to_set: ident, $lock_name: expr, $descr: expr) => {
        $self.$to_set = match $config
            .home()
            .open_shared_create($lock_name, $config, $descr)
        {
            Ok(lock) => Some(lock),
            Err(e) => {
                // There is no error here because locking is mostly a
                // best-effort attempt. If cargo home is read-only, we don't
                // want to fail just because we couldn't create the lock file.
                tracing::warn!("failed to acquire cache lock {}: {e:?}", $lock_name);
                None
            }
        };
    };
}

/// Macro for acquiring a shared lock without blocking.
macro_rules! do_try_shared_lock {
    ($self: ident, $config: expr, $to_set: ident, $lock_name: expr, $unused: expr) => {
        $self.$to_set = match $config.home().try_open_shared_create($lock_name) {
            Ok(Some(lock)) => Some(lock),
            Ok(None) => {
                return Ok(false);
            }
            Err(e) => {
                tracing::warn!("failed to acquire cache lock {}: {e:?}", $lock_name);
                None
            }
        };
    };
}

/// Macro for acquiring an exclusive lock (can block).
macro_rules! do_exclusive_lock {
    ($self: ident, $config: expr, $to_set: ident, $lock_name: expr, $descr: expr) => {
        match $config.home().open_rw($lock_name, $config, $descr) {
            Ok(lock) => {
                $self.$to_set = Some(lock);
                Ok(())
            }
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
                    do_shared_lock!($self, $config, $to_set, $lock_name, $descr);
                    Ok(())
                } else {
                    Err(e).with_context(|| "failed to acquire package cache lock")
                }
            }
        }
    };
}

/// Macro for acquiring an exclusive lock without blocking.
macro_rules! do_try_exclusive_lock {
    ($self: ident, $config: expr, $to_set: ident, $lock_name: expr, $unused: expr) => {
        match $config.home().try_open_rw($lock_name) {
            Ok(Some(lock)) => {
                $self.$to_set = Some(lock);
                Ok(())
            }
            Ok(None) => return Ok(false),
            Err(e) => {
                if maybe_readonly(&e) {
                    do_try_shared_lock!($self, $config, $to_set, $lock_name, $unused);
                    Ok(())
                } else {
                    Err(e).with_context(|| "failed to acquire package cache lock")
                }
            }
        }
    };
}

/// Macro to help with acquiring a lock.
///
/// `shared` and `exclusive` are inputs to the macro so that either the
/// blocking or non-blocking implementations can be called based on what the
/// caller wants.
macro_rules! do_lock {
    ($self: ident, $config: expr, $mode: expr, $shared: ident, $exclusive: ident) => {
        use CacheLockMode::*;
        match (
            $mode,
            &$self.cache_lock_count,
            &$self.mutate_lock_count,
            $self.mutate_is_exclusive,
        ) {
            (Shared, 0, 0, _) => {
                // Shared lock, no locks currently held.
                $shared!($self, $config, mutate_lock, MUTATE_NAME, SHARED_DESCR);
                $self.mutate_lock_count += 1;
                $self.mutate_is_exclusive = false;
            }
            (DownloadExclusive, 0, _, _) => {
                // DownloadExclusive lock, no DownloadExclusive lock currently held.
                $exclusive!(
                    $self,
                    $config,
                    cache_lock,
                    CACHE_LOCK_NAME,
                    DOWNLOAD_EXCLUSIVE_DESCR
                )?;
                $self.cache_lock_count += 1;
            }
            (MutateExclusive, 0, 0, _) => {
                // MutateExclusive lock, no locks currently held.
                $exclusive!(
                    $self,
                    $config,
                    mutate_lock,
                    MUTATE_NAME,
                    MUTATE_EXCLUSIVE_DESCR
                )?;
                $self.mutate_lock_count += 1;
                $self.mutate_is_exclusive = true;

                // Part of the contract of MutateExclusive is that it doesn't
                // allow any processes to have a lock on the package cache, so
                // this acquires both locks.
                if let Err(e) = $exclusive!(
                    $self,
                    $config,
                    cache_lock,
                    CACHE_LOCK_NAME,
                    DOWNLOAD_EXCLUSIVE_DESCR
                ) {
                    decrement!($self, mutate_lock_count, mutate_lock);
                    return Err(e);
                }
                $self.cache_lock_count += 1;
            }
            (Shared, 1.., 0, _) => {
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
            (MutateExclusive, _, 1.., false) => {
                // MutateExclusive lock, when a Shared lock is held.
                //
                // Lock upgrades are dicey. It might be possible to support
                // this but would take a bit of work, and so far it isn't
                // needed.
                panic!("lock upgrade from Shared to MutateExclusive not supported");
            }
            (Shared, _, 1.., _) => {
                // Shared lock, when a Shared or MutateExclusive lock is held.
                //
                // MutateExclusive is more restrictive than Shared, so no need
                // to do anything.
                $self.mutate_lock_count = $self.mutate_lock_count.checked_add(1).unwrap();
            }
            (DownloadExclusive, 1.., _, _) => {
                // DownloadExclusive lock, when another DownloadExclusive is held.
                $self.cache_lock_count = $self.cache_lock_count.checked_add(1).unwrap();
            }
            (MutateExclusive, _, 1.., true) => {
                // MutateExclusive lock, when another MutateExclusive is held.
                $self.cache_lock_count += 1;
                $self.mutate_lock_count += 1;
            }
            (MutateExclusive, 1.., 0, _) => {
                // MutateExclusive lock, when only a DownloadExclusive is held.
                $exclusive!(
                    $self,
                    $config,
                    mutate_lock,
                    MUTATE_NAME,
                    MUTATE_EXCLUSIVE_DESCR
                )?;
                // Both of these need to be incremented to match the behavior
                // in the drop impl.
                $self.cache_lock_count += 1;
                $self.mutate_lock_count += 1;
                $self.mutate_is_exclusive = true;
            }
        }
    };
}

impl CacheLocker {
    /// Creates a new `CacheLocker`.
    pub fn new() -> CacheLocker {
        CacheLocker {
            state: RefCell::new(CacheState::default()),
        }
    }

    /// Acquires a lock with the given mode, possibly blocking if another
    /// cargo is holding the lock.
    pub fn lock(&self, config: &Config, mode: CacheLockMode) -> CargoResult<CacheLock<'_>> {
        let mut state = self.state.borrow_mut();
        state.lock(config, mode)?;
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
        if state.try_lock(config, mode)? {
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
            state.cache_lock_count,
            state.mutate_lock_count,
            state.mutate_is_exclusive,
        ) {
            (CacheLockMode::Shared, _, 1.., _) => true,
            (CacheLockMode::MutateExclusive, _, 1.., true) => true,
            (CacheLockMode::DownloadExclusive, 1.., _, _) => true,
            _ => false,
        }
    }
}

impl CacheState {
    fn lock(&mut self, config: &Config, mode: CacheLockMode) -> CargoResult<()> {
        do_lock!(self, config, mode, do_shared_lock, do_exclusive_lock);
        Ok(())
    }

    fn try_lock(&mut self, config: &Config, mode: CacheLockMode) -> CargoResult<bool> {
        do_lock!(
            self,
            config,
            mode,
            do_try_shared_lock,
            do_try_exclusive_lock
        );
        Ok(true)
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

impl Drop for CacheLock<'_> {
    fn drop(&mut self) {
        use CacheLockMode::*;
        let mut state = self.locker.state.borrow_mut();
        match self.mode {
            Shared => {
                decrement!(state, mutate_lock_count, mutate_lock);
            }
            DownloadExclusive => {
                decrement!(state, cache_lock_count, cache_lock);
            }
            MutateExclusive => {
                decrement!(state, cache_lock_count, cache_lock);
                decrement!(state, mutate_lock_count, mutate_lock);
            }
        }
    }
}
