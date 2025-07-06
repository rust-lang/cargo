//! Interacts with the registry [owners API][1].
//!
//! [1]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html#owners

use anyhow::Context as _;
use cargo_credential::Operation;
use cargo_credential::Secret;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Workspace;
use crate::drop_print;
use crate::drop_println;
use crate::util::important_paths::find_root_manifest_for_wd;

use super::RegistryOrIndex;

pub struct OwnersOptions {
    pub krate: Option<String>,
    pub token: Option<Secret<String>>,
    pub reg_or_index: Option<RegistryOrIndex>,
    pub to_add: Option<Vec<String>>,
    pub to_remove: Option<Vec<String>>,
    pub list: bool,
}

pub fn modify_owners(gctx: &GlobalContext, opts: &OwnersOptions) -> CargoResult<()> {
    let name = match opts.krate {
        Some(ref name) => name.clone(),
        None => {
            let manifest_path = find_root_manifest_for_wd(gctx.cwd())?;
            let ws = Workspace::new(&manifest_path, gctx)?;
            ws.current()?.package_id().name().to_string()
        }
    };

    let operation = Operation::Owners { name: &name };
    let source_ids = super::get_source_id(gctx, opts.reg_or_index.as_ref())?;
    let (mut registry, _) = super::registry(
        gctx,
        &source_ids,
        opts.token.as_ref().map(Secret::as_deref),
        opts.reg_or_index.as_ref(),
        true,
        Some(operation),
    )?;

    if let Some(ref v) = opts.to_add {
        let v = v.iter().map(|s| &s[..]).collect::<Vec<_>>();
        let msg = registry.add_owners(&name, &v).with_context(|| {
            format!(
                "failed to invite owners to crate `{}` on registry at {}",
                name,
                registry.host()
            )
        })?;

        gctx.shell().status("Owner", msg)?;
    }

    if let Some(ref v) = opts.to_remove {
        let v = v.iter().map(|s| &s[..]).collect::<Vec<_>>();
        gctx.shell()
            .status("Owner", format!("removing {:?} from crate {}", v, name))?;
        registry.remove_owners(&name, &v).with_context(|| {
            format!(
                "failed to remove owners from crate `{}` on registry at {}",
                name,
                registry.host()
            )
        })?;
    }

    if opts.list {
        let owners = registry.list_owners(&name).with_context(|| {
            format!(
                "failed to list owners of crate `{}` on registry at {}",
                name,
                registry.host()
            )
        })?;
        for owner in owners.iter() {
            drop_print!(gctx, "{}", owner.login);
            match (owner.name.as_ref(), owner.email.as_ref()) {
                (Some(name), Some(email)) => drop_println!(gctx, " ({} <{}>)", name, email),
                (Some(s), None) | (None, Some(s)) => drop_println!(gctx, " ({})", s),
                (None, None) => drop_println!(gctx),
            }
        }
    }

    Ok(())
}
