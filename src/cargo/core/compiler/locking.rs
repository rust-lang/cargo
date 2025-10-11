use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
};

use itertools::Itertools;

use crate::core::compiler::{BuildRunner, Unit};

pub struct CompilationLock {
    /// The path to the lock file of the unit to compile
    unit: PathBuf,
    /// The paths to lock files of the unit's dependencies
    dependency_units: Vec<PathBuf>,
}

impl CompilationLock {
    pub fn new(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> Self {
        let unit_path = build_runner.files().build_unit_lock(unit);

        let dependency_units = build_runner
            .unit_deps(unit)
            .into_iter()
            .map(|unit| build_runner.files().build_unit_lock(&unit.unit))
            .collect_vec();

        Self {
            unit: unit_path,
            dependency_units,
        }
    }

    pub fn lock(self) -> CompilationLockGuard {
        let unit_lock = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(self.unit)
            .unwrap();
        unit_lock.lock().unwrap();

        let dependency_locks = self
            .dependency_units
            .into_iter()
            .map(|d| {
                let f = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .append(true)
                    .open(d)
                    .unwrap();
                f.lock_shared().unwrap();
                f
            })
            .collect::<Vec<_>>();

        CompilationLockGuard {
            _lock: unit_lock,
            _dependency_locks: dependency_locks,
        }
    }
}

pub struct CompilationLockGuard {
    _lock: File,
    _dependency_locks: Vec<File>,
}
