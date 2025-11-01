//! This module handles the locking logic during compilation.
//!
//! The locking scheme is based on build unit level locking.
//! Each build unit consists of a partial and full lock used to represent multiple lock states.
//!
//! | State                  | `partial.lock` | `full.lock`  |
//! |------------------------|----------------|--------------|
//! | Unlocked               | `unlocked`     | `unlocked`   |
//! | Building Exclusive     | `exclusive`    | `exclusive`  |
//! | Building Non-Exclusive | `shared`       | `exclusive`  |
//! | Shared Partial         | `shared`       | `unlocked`   |
//! | Shared Full            | `shared`       | `shared`     |
//!
//! Generally a build unit will full the following flow:
//! 1. Acquire a "building exclusive" lock for the current build unit.
//! 2. Acquire "shared" locks on all dependency build units.
//! 3. Begin building with rustc
//! 4. If we are building a library, downgrade to a "building non-exclusive" lock when the `.rmeta` has been generated.
//! 5. Once complete release all locks.
//!
//! Most build units only require metadata (.rmeta) from dependencies, so they can begin building
//! once the dependency units have produced the .rmeta. These units take a "shared partial" lock
//! which can be taken while the dependency still holds the "build non-exclusive" lock.
//!
//! Note that some build unit types like bin and proc-macros require the full dependency build
//! (.rlib). For these unit types they must take a "shared full" lock on dependency units which will
//! block until the dependency unit is fully built.
//!
//! The primary reason for the complexity here it to enable fine grain locking while also allowing pipelined builds.
//!
//! [`CompilationLock`] is the primary interface for locking.

use std::{
    collections::{HashMap, HashSet},
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, LazyLock, Mutex},
};

use anyhow::{Context, anyhow};
use itertools::Itertools;
use tracing::{instrument, trace};

use crate::{
    CargoResult,
    core::compiler::{BuildRunner, Unit, layout::BuildUnitLockLocation},
};

/// The locking mode that will be used for output directories.
#[derive(Debug)]
pub enum LockingMode {
    /// Completely disables locking (used for filesystems that do not support locking)
    None,
    /// Fine grain locking (Build unit level)
    Fine,
    /// Coarse grain locking (Profile level)
    Coarse,
}

/// The type of lock to take when taking a shared lock.
/// See the module documentation for more information about shared lock types.
#[derive(Debug)]
pub enum SharedLockType {
    /// A shared lock that might still be compiling a .rlib
    Partial,
    /// A shared lock that is guaranteed to not be compiling
    Full,
}

/// A lock for compiling a build unit.
///
/// Internally this lock is made up of many [`UnitLock`]s for the unit and it's dependencies.
pub struct CompilationLock {
    /// The path to the lock file of the unit to compile
    unit: UnitLock,
    /// The paths to lock files of the unit's dependencies
    dependency_units: Vec<UnitLock>,
}

impl CompilationLock {
    pub fn new(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> Self {
        let unit_lock = UnitLock::new(build_runner.files().build_unit_lock(unit));

        let dependency_units = all_dependency_units(build_runner, unit)
            .into_iter()
            .map(|unit| UnitLock::new(build_runner.files().build_unit_lock(&unit)))
            .collect_vec();

        Self {
            unit: unit_lock,
            dependency_units,
        }
    }

    #[instrument(skip(self))]
    pub fn lock(&mut self, ty: &SharedLockType) -> CargoResult<()> {
        self.unit.lock_exclusive()?;

        for d in self.dependency_units.iter_mut() {
            d.lock_shared(ty)?;
        }

        trace!("acquired lock: {:?}", self.unit.partial.parent());

        Ok(())
    }

    pub fn rmeta_produced(&mut self) -> CargoResult<()> {
        trace!("downgrading lock: {:?}", self.unit.partial.parent());

        // Downgrade the lock on the unit we are building so that we can unblock other units to
        // compile. We do not need to downgrade our dependency locks since they should always be a
        // shared lock.
        self.unit.downgrade()?;

        Ok(())
    }
}

/// A lock for a single build unit.
struct UnitLock {
    partial: PathBuf,
    full: PathBuf,
    guard: Option<UnitLockGuard>,
}

struct UnitLockGuard {
    partial: Arc<RcFileLock>,
    full: Option<Arc<RcFileLock>>,
}

impl Drop for UnitLockGuard {
    fn drop(&mut self) {
        self.partial.unlock().unwrap();
        if let Some(full) = &self.full {
            full.unlock().unwrap();
        }
    }
}

impl UnitLock {
    pub fn new(location: BuildUnitLockLocation) -> Self {
        Self {
            partial: location.partial,
            full: location.full,
            guard: None,
        }
    }

    pub fn lock_exclusive(&mut self) -> CargoResult<()> {
        assert!(self.guard.is_none());

        let partial = FileLockInterner::get_or_create_lock(&self.partial)?;
        partial.lock()?;

        let full = FileLockInterner::get_or_create_lock(&self.full)?;
        full.lock()?;

        self.guard = Some(UnitLockGuard {
            partial,
            full: Some(full),
        });
        Ok(())
    }

    pub fn lock_shared(&mut self, ty: &SharedLockType) -> CargoResult<()> {
        assert!(self.guard.is_none());

        let partial = FileLockInterner::get_or_create_lock(&self.partial)?;
        partial.lock_shared()?;

        let full = if matches!(ty, SharedLockType::Full) {
            let full_lock = FileLockInterner::get_or_create_lock(&self.full)?;
            full_lock.lock_shared()?;
            Some(full_lock)
        } else {
            None
        };

        self.guard = Some(UnitLockGuard { partial, full });
        Ok(())
    }

    pub fn downgrade(&mut self) -> CargoResult<()> {
        let guard = self
            .guard
            .as_ref()
            .context("guard was None while calling downgrade")?;
        guard.partial.downgrade()?;

        Ok(())
    }
}

fn open_file<T: AsRef<Path>>(f: T) -> CargoResult<File> {
    Ok(OpenOptions::new()
        .read(true)
        .create(true)
        .write(true)
        .append(true)
        .open(f)?)
}

fn all_dependency_units<'a>(
    build_runner: &'a BuildRunner<'a, '_>,
    unit: &Unit,
) -> HashSet<&'a Unit> {
    fn inner<'a>(
        build_runner: &'a BuildRunner<'a, '_>,
        unit: &Unit,
        results: &mut HashSet<&'a Unit>,
    ) {
        for dep in build_runner.unit_deps(unit) {
            if results.insert(&dep.unit) {
                inner(&build_runner, &dep.unit, results);
            }
        }
    }

    let mut results = HashSet::new();
    inner(build_runner, unit, &mut results);
    return results;
}

/// An interner to manage [`RcFileLock`]s to make sharing across compilation jobs easier.
pub struct FileLockInterner {
    locks: Mutex<HashMap<PathBuf, Arc<RcFileLock>>>,
}

impl FileLockInterner {
    pub fn new() -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_or_create_lock(path: &Path) -> CargoResult<Arc<RcFileLock>> {
        static GLOBAL: LazyLock<FileLockInterner> = LazyLock::new(FileLockInterner::new);

        let mut locks = GLOBAL
            .locks
            .lock()
            .map_err(|_| anyhow!("lock was poisoned"))?;

        if let Some(lock) = locks.get(path) {
            return Ok(Arc::clone(lock));
        }

        let file = open_file(&path)?;

        let lock = Arc::new(RcFileLock {
            inner: Mutex::new(RcFileLockInner {
                file,
                share_count: 0,
                exclusive: false,
            }),
            condvar: Condvar::new(),
        });

        locks.insert(path.to_path_buf(), Arc::clone(&lock));

        return Ok(lock);
    }
}

/// A reference counted file lock.
///
/// This lock is designed to reduce file descriptors by sharing a single file descriptor for a
/// given lock when the lock is shared. The motivation for this is to avoid hitting file descriptor
/// limits when fine grain locking is enabled.
pub struct RcFileLock {
    inner: Mutex<RcFileLockInner>,
    condvar: Condvar,
}

struct RcFileLockInner {
    file: File,
    exclusive: bool,
    share_count: u32,
}

impl RcFileLock {
    pub fn lock(&self) -> CargoResult<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow!("lock was poisoned"))?;

        while inner.exclusive || inner.share_count > 0 {
            inner = self
                .condvar
                .wait(inner)
                .map_err(|_| anyhow!("lock was poisoned"))?;
        }

        inner.file.lock()?;
        inner.exclusive = true;

        Ok(())
    }

    pub fn lock_shared(&self) -> CargoResult<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow!("lock was poisoned"))?;

        while inner.exclusive {
            inner = self
                .condvar
                .wait(inner)
                .map_err(|_| anyhow!("lock was poisoned"))?;
        }

        if inner.share_count == 0 {
            inner.file.lock_shared()?;
            inner.share_count = 1;
        } else {
            inner.share_count += 1;
        }

        Ok(())
    }

    pub fn unlock(&self) -> CargoResult<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow!("lock was poisoned"))?;

        if inner.exclusive {
            assert!(inner.share_count == 0);
            inner.file.unlock()?;
            self.condvar.notify_all();
            inner.exclusive = false;
        } else {
            if inner.share_count > 1 {
                inner.share_count -= 1;
            } else {
                inner.file.unlock()?;
                inner.share_count = 0;
                self.condvar.notify_all();
            }
        }

        Ok(())
    }

    pub fn downgrade(&self) -> CargoResult<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow!("lock was poisoned"))?;

        assert!(inner.exclusive);
        assert!(inner.share_count == 0);

        // NOTE:
        // > Subsequent flock() calls on an already locked file will convert an existing lock to the new lock mode.
        // https://man7.org/linux/man-pages/man2/flock.2.html
        //
        // However, the `std::file::File::lock/lock_shared` is allowed to change this in the
        // future. So its probably up to us if we are okay with using this or if we want to use a
        // different interface to flock.
        inner.file.lock_shared()?;

        inner.exclusive = false;
        inner.share_count = 1;

        Ok(())
    }
}
