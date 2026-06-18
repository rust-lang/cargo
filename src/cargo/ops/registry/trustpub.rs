use anyhow::Context as _;
use cargo_credential::Operation;
use cargo_credential::Secret;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Workspace;
use crate::drop_print;
use crate::drop_println;
use crate::util::important_paths::find_root_manifest_for_wd;

pub enum TrustpubCommand {
    List,
    Add {
        repository_owner: String,
        repository_name: String,
        workflow_filename: String,
        environment: Option<String>,
    },
}

pub struct TrustpubOptions {
    pub krate: Option<String>,
    pub token: Option<Secret<String>>,
    pub command: TrustpubCommand,
}

pub fn trusted_publish(gctx: &GlobalContext, opts: &TrustpubOptions) -> CargoResult<()> {
    let name = match opts.krate {
        Some(ref name) => name.clone(),
        None => {
            let manifest_path = find_root_manifest_for_wd(gctx.cwd())?;
            let ws = Workspace::new(&manifest_path, gctx)?;
            ws.current()?.package_id().name().to_string()
        }
    };

    let operation = Operation::Owners { name: &name };
    let source_ids = super::get_source_id(gctx, None)?;
    let (mut registry, _) = super::registry(
        gctx,
        &source_ids,
        opts.token.as_ref().map(Secret::as_deref),
        None,
        true,
        Some(operation),
    )?;

    match &opts.command {
        TrustpubCommand::List => {
            let configs = registry.list_github_trustpub_configs(&name).with_context(|| {
                format!(
                    "failed to list trusted publishing configs for crate `{}` on registry at {}",
                    name,
                    registry.host()
                )
            })?;
            if configs.is_empty() {
                drop_println!(
                    gctx,
                    "no trusted publishing configs found for crate `{}`",
                    name
                );
            }
            for config in configs.iter() {
                drop_print!(
                    gctx,
                    "{}: github {}/{} workflow={}",
                    config.id,
                    config.repository_owner,
                    config.repository_name,
                    config.workflow_filename,
                );
                match config.environment.as_ref() {
                    Some(env) => drop_println!(gctx, " environment={}", env),
                    None => drop_println!(gctx),
                }
            }
        }
        TrustpubCommand::Add {
            repository_owner,
            repository_name,
            workflow_filename,
            environment,
        } => {
            let config = registry
                .add_github_trustpub_config(
                    &name,
                    repository_owner,
                    repository_name,
                    workflow_filename,
                    environment.as_deref(),
                )
                .with_context(|| {
                    format!(
                        "failed to add trusted publishing config to crate `{}` on registry at {}",
                        name,
                        registry.host()
                    )
                })?;
            let environment = match config.environment.as_ref() {
                Some(env) => format!(" environment={}", env),
                None => String::new(),
            };
            gctx.shell().status(
                "Added",
                format!(
                    "trusted publishing config {} ({}/{} workflow={}{}) for crate `{}`",
                    config.id,
                    config.repository_owner,
                    config.repository_name,
                    config.workflow_filename,
                    environment,
                    name,
                ),
            )?;
        }
    }

    Ok(())
}
