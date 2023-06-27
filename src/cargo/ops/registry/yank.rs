//! Interacts with the registry [yank] and [unyank] API.
//!
//! [yank]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html#yank
//! [unyank]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html#unyank

use anyhow::bail;
use anyhow::Context as _;
use cargo_credential::Operation;
use cargo_credential::Secret;

use crate::core::Workspace;
use crate::util::config::Config;
use crate::util::errors::CargoResult;
use crate::util::important_paths::find_root_manifest_for_wd;

pub fn yank(
    config: &Config,
    krate: Option<String>,
    version: Option<String>,
    token: Option<Secret<String>>,
    index: Option<String>,
    undo: bool,
    reg: Option<String>,
) -> CargoResult<()> {
    let name = match krate {
        Some(name) => name,
        None => {
            let manifest_path = find_root_manifest_for_wd(config.cwd())?;
            let ws = Workspace::new(&manifest_path, config)?;
            ws.current()?.package_id().name().to_string()
        }
    };
    let version = match version {
        Some(v) => v,
        None => bail!("a version must be specified to yank"),
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

    let (mut registry, _) = super::registry(
        config,
        token.as_ref().map(Secret::as_deref),
        index.as_deref(),
        reg.as_deref(),
        true,
        Some(message),
    )?;

    let package_spec = format!("{}@{}", name, version);
    if undo {
        config.shell().status("Unyank", package_spec)?;
        registry.unyank(&name, &version).with_context(|| {
            format!(
                "failed to undo a yank from the registry at {}",
                registry.host()
            )
        })?;
    } else {
        config.shell().status("Yank", package_spec)?;
        registry
            .yank(&name, &version)
            .with_context(|| format!("failed to yank from the registry at {}", registry.host()))?;
    }

    Ok(())
}
