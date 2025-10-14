//! This module handles the locking logic during compilation.
//!
//! The locking scheme is based on build unit level locking.
//! Generally a build unit will follow the following flow:
//! 1. Acquire an exclusive lock for the current build unit.
//! 2. Acquire shared locks on all dependency build units.
//! 3. Begin building with rustc
//! 5. Once complete release all locks.
//!
//! [`CompilationLock`] is the primary interface for locking.

use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

use itertools::Itertools;
use tracing::instrument;

use crate::{
    CargoResult,
    core::compiler::{BuildRunner, Unit},
};

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
        let unit_lock = build_runner.files().build_unit_lock(unit).into();

        let dependency_units = build_runner
            .unit_deps(unit)
            .into_iter()
            .map(|unit| build_runner.files().build_unit_lock(&unit.unit).into())
            .collect_vec();

        Self {
            unit: unit_lock,
            dependency_units,
        }
    }

    #[instrument(skip(self))]
    pub fn lock(&mut self) -> CargoResult<()> {
        self.unit.lock_exclusive()?;

        for d in self.dependency_units.iter_mut() {
            d.lock_shared()?;
        }

        Ok(())
    }
}

/// A lock for a single build unit.
struct UnitLock {
    lock: PathBuf,
    gaurd: Option<UnitLockGuard>,
}

struct UnitLockGuard {
    _handle: File,
}

impl UnitLock {
    pub fn lock_exclusive(&mut self) -> CargoResult<()> {
        assert!(self.gaurd.is_none());

        let lock = file_lock(&self.lock)?;
        lock.lock()?;

        self.gaurd = Some(UnitLockGuard { _handle: lock });
        Ok(())
    }

    pub fn lock_shared(&mut self) -> CargoResult<()> {
        assert!(self.gaurd.is_none());

        let lock = file_lock(&self.lock)?;
        lock.lock_shared()?;

        self.gaurd = Some(UnitLockGuard { _handle: lock });
        Ok(())
    }
}

impl From<PathBuf> for UnitLock {
    fn from(value: PathBuf) -> Self {
        Self {
            lock: value,
            gaurd: None,
        }
    }
}

fn file_lock<T: AsRef<Path>>(f: T) -> CargoResult<File> {
    Ok(OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(f)?)
}
