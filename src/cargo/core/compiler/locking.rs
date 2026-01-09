//! This module handles the locking logic during compilation.

use crate::{
    CargoResult,
    core::compiler::{BuildRunner, Unit},
    util::{
        FileLock, Filesystem,
        flock::{self, ReportBlocking},
        interning::InternedString,
    },
};
use anyhow::bail;
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    path::PathBuf,
    sync::Mutex,
};
use tracing::instrument;

/// A struct to store the lock handles for build units during compilation.
pub struct LockManager {
    locks: Mutex<HashMap<LockKey, FileLock>>,
}

impl LockManager {
    pub fn new() -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
        }
    }

    /// Takes a shared lock on a given [`Unit`]
    /// This prevents other Cargo instances from compiling (writing) to
    /// this build unit.
    ///
    /// This function returns a [`LockKey`] which can be used to
    /// upgrade/unlock the lock.
    #[instrument(skip_all, fields(key))]
    pub fn lock_shared(
        &self,
        build_runner: &BuildRunner<'_, '_>,
        unit: &Unit,
    ) -> CargoResult<LockKey> {
        let key = LockKey::from_unit(build_runner, unit);
        tracing::Span::current().record("key", key.0.to_str());

        let mut locks = self.locks.lock().unwrap();
        if let Some(lock) = locks.get_mut(&key) {
            lock.file().lock_shared()?;
        } else {
            let fs = Filesystem::new(key.0.clone());
            let lock_msg = key.msg();
            let lock = fs.open_ro_shared_create(&key.0, build_runner.bcx.gctx, &lock_msg)?;
            locks.insert(key.clone(), lock);
        }

        Ok(key)
    }
    #[instrument(skip(self, report_blocking))]
    pub fn lock(&self, key: &LockKey, report_blocking: impl ReportBlocking) -> CargoResult<()> {
        let mut locks = self.locks.lock().unwrap();
        if let Some(lock) = locks.get_mut(&key) {
            let file = lock.file();

            flock::acquire(
                report_blocking,
                &key.msg(),
                &key.0,
                &|| file.try_lock(),
                &|| file.lock(),
            )?;
        } else {
            bail!("lock was not found in lock manager: {key}");
        }

        Ok(())
    }

    /// Upgrades an existing exclusive lock into a shared lock.
    #[instrument(skip(self, report_blocking))]
    pub fn downgrade_to_shared(
        &self,
        key: &LockKey,
        report_blocking: impl ReportBlocking,
    ) -> CargoResult<()> {
        let mut locks = self.locks.lock().unwrap();
        let Some(lock) = locks.get_mut(key) else {
            bail!("lock was not found in lock manager: {key}");
        };
        let file = lock.file();
        flock::acquire(
            report_blocking,
            &key.msg(),
            &key.0,
            &|| file.try_lock_shared(),
            &|| file.lock_shared(),
        )?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub fn unlock(&self, key: &LockKey) -> CargoResult<()> {
        let mut locks = self.locks.lock().unwrap();
        if let Some(lock) = locks.get_mut(key) {
            lock.file().unlock()?;
        };

        Ok(())
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct LockKey(PathBuf, InternedString, String);

impl LockKey {
    fn from_unit(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> Self {
        let name = unit.pkg.name();
        let hash = build_runner.files().unit_hash(unit);
        let path = build_runner.files().build_unit_lock(unit);
        Self(path, name, hash)
    }

    fn msg(&self) -> String {
        format!("{} ({})", self.1, self.2)
    }
}

impl Display for LockKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}
