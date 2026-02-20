//! This module handles the locking logic during compilation.

use crate::{
    CargoResult,
    core::compiler::{BuildRunner, Unit},
    util::{FileLock, Filesystem},
};
use anyhow::bail;
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    path::PathBuf,
    sync::RwLock,
};
use tracing::instrument;

/// A struct to store the lock handles for build units during compilation.
pub struct LockManager {
    locks: RwLock<HashMap<LockKey, FileLock>>,
}

impl LockManager {
    pub fn new() -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
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

        let mut locks = self.locks.write().unwrap();
        if let Some(lock) = locks.get_mut(&key) {
            lock.file().lock_shared()?;
        } else {
            let fs = Filesystem::new(key.0.clone());
            let lock_msg = format!(
                "{} ({})",
                unit.pkg.name(),
                build_runner.files().unit_hash(unit)
            );
            let lock = fs.open_ro_shared_create(&key.0, build_runner.bcx.gctx, &lock_msg)?;
            locks.insert(key.clone(), lock);
        }

        Ok(key)
    }

    #[instrument(skip(self))]
    pub fn lock(&self, key: &LockKey) -> CargoResult<()> {
        let locks = self.locks.read().unwrap();
        if let Some(lock) = locks.get(&key) {
            lock.file().lock()?;
        } else {
            bail!("lock was not found in lock manager: {key}");
        }

        Ok(())
    }

    /// Upgrades an existing exclusive lock into a shared lock.
    #[instrument(skip(self))]
    pub fn downgrade_to_shared(&self, key: &LockKey) -> CargoResult<()> {
        let locks = self.locks.read().unwrap();
        let Some(lock) = locks.get(key) else {
            bail!("lock was not found in lock manager: {key}");
        };
        lock.file().lock_shared()?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub fn unlock(&self, key: &LockKey) -> CargoResult<()> {
        let locks = self.locks.read().unwrap();
        if let Some(lock) = locks.get(key) {
            lock.file().unlock()?;
        };

        Ok(())
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct LockKey(PathBuf);

impl LockKey {
    fn from_unit(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> Self {
        Self(build_runner.files().build_unit_lock(unit))
    }
}

impl Display for LockKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}
