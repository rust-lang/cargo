use crate::core::gc::GcOpts;
use crate::ops::CleanContext;
use crate::util::interning::InternedString;
use crate::{CargoResult, GlobalContext};
use std::time::Duration;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegistryIndex {
    pub encoded_registry_name: InternedString,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegistryCrate {
    pub encoded_registry_name: InternedString,
    pub crate_filename: InternedString,
    pub size: u64,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RegistrySrc {
    pub encoded_registry_name: InternedString,
    pub package_dir: InternedString,
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct GitCheckout {
    pub encoded_git_name: InternedString,
    pub short_name: InternedString,
    pub size: Option<u64>,
}

#[derive(Debug)]
pub struct GlobalCacheTracker;

impl GlobalCacheTracker {
    pub fn new(_gctx: &GlobalContext) -> CargoResult<GlobalCacheTracker> {
        Ok(GlobalCacheTracker)
    }

    pub fn should_run_auto_gc(&mut self, _freq: Duration) -> CargoResult<bool> {
        Ok(false)
    }

    pub fn set_last_auto_gc(&mut self) -> CargoResult<()> {
        Ok(())
    }

    pub fn clean(
        &mut self,
        _clean_ctx: &mut CleanContext<'_>,
        _gc_opts: &GcOpts,
    ) -> CargoResult<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct DeferredGlobalLastUse;

impl DeferredGlobalLastUse {
    pub fn new() -> DeferredGlobalLastUse {
        DeferredGlobalLastUse
    }

    pub fn is_empty(&self) -> bool {
        true
    }

    pub fn mark_registry_index_used(&mut self, _registry_index: RegistryIndex) {}

    pub fn mark_registry_crate_used(&mut self, _registry_crate: RegistryCrate) {}

    pub fn mark_registry_src_used(&mut self, _registry_src: RegistrySrc) {}

    pub fn mark_git_checkout_used(&mut self, _git_checkout: GitCheckout) {}

    pub fn save(&mut self, _tracker: &mut GlobalCacheTracker) -> CargoResult<()> {
        Ok(())
    }

    pub fn save_no_error(&mut self, _gctx: &GlobalContext) {}
}

pub fn is_silent_error(_e: &anyhow::Error) -> bool {
    false
}
