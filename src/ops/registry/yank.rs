//! Interacts with the registry [yank] and [unyank] API.
//!
//! [yank]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html#yank
//! [unyank]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html#unyank

use anyhow::Context as _;
use anyhow::bail;
use cargo_credential::Operation;
use cargo_credential::Secret;

use crate::core::Workspace;
use crate::util::context::GlobalContext;
use crate::util::errors::CargoResult;
use crate::util::important_paths::find_root_manifest_for_wd;

use super::RegistryOrIndex;

pub fn yank(
    gctx: &GlobalContext,
    krate: Option<String>,
    version: Option<String>,
    token: Option<Secret<String>>,
    reg_or_index: Option<RegistryOrIndex>,
    undo: bool,
) -> CargoResult<()> {
    let name = match krate {
        Some(name) => name,
        None => {
            let manifest_path = find_root_manifest_for_wd(gctx.cwd())?;
            let ws = Workspace::new(&manifest_path, gctx)?;
            ws.current()?.package_id().name().to_string()
        }
    };
    let Some(version) = version else {
        bail!("a version must be specified to yank")
    };

    let message = if undo {
        Operation::Unyank {
            name: &name,
            vers: &version,
        }
    } else {
        Operation::Yank {
            name: &name,
            vers: &version,
        }
    };
    let source_ids = super::get_source_id(gctx, reg_or_index.as_ref())?;
    let (mut registry, _) = super::registry(
        gctx,
        &source_ids,
        token.as_ref().map(Secret::as_deref),
        reg_or_index.as_ref(),
        true,
        Some(message),
    )?;

    let package_spec = format!("{}@{}", name, version);
    if undo {
        gctx.shell().status("Unyank", package_spec)?;
        registry.unyank(&name, &version).with_context(|| {
            format!(
                "failed to undo a yank from the registry at {}",
                registry.host()
            )
        })?;
    } else {
        gctx.shell().status("Yank", package_spec)?;
        registry
            .yank(&name, &version)
            .with_context(|| format!("failed to yank from the registry at {}", registry.host()))?;
    }

    Ok(())
}
