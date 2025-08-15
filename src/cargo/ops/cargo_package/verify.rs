//! Helpers to verify a packaged `.crate` file.

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::SeekFrom;
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use cargo_util::paths;
use flate2::read::GzDecoder;
use tar::Archive;

use crate::CargoResult;
use crate::core::Feature;
use crate::core::Package;
use crate::core::SourceId;
use crate::core::Workspace;
use crate::core::compiler::BuildConfig;
use crate::core::compiler::DefaultExecutor;
use crate::core::compiler::Executor;
use crate::core::compiler::UserIntent;
use crate::ops;
use crate::sources::PathSource;
use crate::util;
use crate::util::FileLock;

use super::PackageOpts;
use super::TmpRegistry;

/// Verifies whether a `.crate` file is able to compile.
pub fn run_verify(
    ws: &Workspace<'_>,
    pkg: &Package,
    tar: &FileLock,
    local_reg: Option<&TmpRegistry<'_>>,
    opts: &PackageOpts<'_>,
) -> CargoResult<()> {
    let gctx = ws.gctx();

    gctx.shell().status("Verifying", pkg)?;

    tar.file().seek(SeekFrom::Start(0))?;
    let f = GzDecoder::new(tar.file());
    let dst = ws.build_dir().as_path_unlocked().join(&format!(
        "package/{}-{}",
        pkg.name(),
        pkg.version()
    ));
    if dst.exists() {
        paths::remove_dir_all(&dst)?;
    }
    let mut archive = Archive::new(f);
    // We don't need to set the Modified Time, as it's not relevant to verification
    // and it errors on filesystems that don't support setting a modified timestamp
    archive.set_preserve_mtime(false);
    archive.unpack(dst.parent().unwrap())?;

    // Manufacture an ephemeral workspace to ensure that even if the top-level
    // package has a workspace we can still build our new crate.
    let id = SourceId::for_path(&dst)?;
    let mut src = PathSource::new(&dst, id, ws.gctx());
    let new_pkg = src.root_package()?;
    let pkg_fingerprint = hash_all(&dst)?;

    // When packaging we use an ephemeral workspace but reuse the build cache to reduce
    // verification time if the user has already compiled the dependencies and the fingerprint
    // is unchanged.
    let mut ws = Workspace::ephemeral(new_pkg, gctx, Some(ws.build_dir()), true)?;
    if let Some(local_reg) = local_reg {
        ws.add_local_overlay(
            local_reg.upstream,
            local_reg.root.as_path_unlocked().to_owned(),
        );
    }

    let rustc_args = if pkg
        .manifest()
        .unstable_features()
        .require(Feature::public_dependency())
        .is_ok()
        || ws.gctx().cli_unstable().public_dependency
    {
        // FIXME: Turn this on at some point in the future
        //Some(vec!["-D exported_private_dependencies".to_string()])
        Some(vec![])
    } else {
        None
    };

    let exec: Arc<dyn Executor> = Arc::new(DefaultExecutor);
    ops::compile_with_exec(
        &ws,
        &ops::CompileOptions {
            build_config: BuildConfig::new(
                gctx,
                opts.jobs.clone(),
                opts.keep_going,
                &opts.targets,
                UserIntent::Build,
            )?,
            cli_features: opts.cli_features.clone(),
            spec: ops::Packages::Packages(Vec::new()),
            filter: ops::CompileFilter::Default {
                required_features_filterable: true,
            },
            target_rustdoc_args: None,
            target_rustc_args: rustc_args,
            target_rustc_crate_types: None,
            rustdoc_document_private_items: false,
            honor_rust_version: None,
        },
        &exec,
    )?;

    // Check that `build.rs` didn't modify any files in the `src` directory.
    let ws_fingerprint = hash_all(&dst)?;
    if pkg_fingerprint != ws_fingerprint {
        let changes = report_hash_difference(&pkg_fingerprint, &ws_fingerprint);
        anyhow::bail!(
            "Source directory was modified by build.rs during cargo publish. \
             Build scripts should not modify anything outside of OUT_DIR.\n\
             {}\n\n\
             To proceed despite this, pass the `--no-verify` flag.",
            changes
        )
    }

    Ok(())
}

/// Hashes everything under a given directory.
///
/// This is for checking if any source file inside a `.crate` file has changed
/// durint the compilation. It is usually caused by bad build scripts or proc
/// macros trying to modify source files. Cargo disallows that.
fn hash_all(path: &Path) -> CargoResult<HashMap<PathBuf, u64>> {
    fn wrap(path: &Path) -> CargoResult<HashMap<PathBuf, u64>> {
        let mut result = HashMap::new();
        let walker = walkdir::WalkDir::new(path).into_iter();
        for entry in walker.filter_entry(|e| !(e.depth() == 1 && e.file_name() == "target")) {
            let entry = entry?;
            let file_type = entry.file_type();
            if file_type.is_file() {
                let file = File::open(entry.path())?;
                let hash = util::hex::hash_u64_file(&file)?;
                result.insert(entry.path().to_path_buf(), hash);
            } else if file_type.is_symlink() {
                let hash = util::hex::hash_u64(&fs::read_link(entry.path())?);
                result.insert(entry.path().to_path_buf(), hash);
            } else if file_type.is_dir() {
                let hash = util::hex::hash_u64(&());
                result.insert(entry.path().to_path_buf(), hash);
            }
        }
        Ok(result)
    }
    let result = wrap(path).with_context(|| format!("failed to verify output at {:?}", path))?;
    Ok(result)
}

/// Reports the hash difference before and after the compilation computed by  [`hash_all`].
fn report_hash_difference(orig: &HashMap<PathBuf, u64>, after: &HashMap<PathBuf, u64>) -> String {
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    for (key, value) in orig {
        match after.get(key) {
            Some(after_value) => {
                if value != after_value {
                    changed.push(key.to_string_lossy());
                }
            }
            None => removed.push(key.to_string_lossy()),
        }
    }
    let mut added: Vec<_> = after
        .keys()
        .filter(|key| !orig.contains_key(*key))
        .map(|key| key.to_string_lossy())
        .collect();
    let mut result = Vec::new();
    if !changed.is_empty() {
        changed.sort_unstable();
        result.push(format!("Changed: {}", changed.join("\n\t")));
    }
    if !added.is_empty() {
        added.sort_unstable();
        result.push(format!("Added: {}", added.join("\n\t")));
    }
    if !removed.is_empty() {
        removed.sort_unstable();
        result.push(format!("Removed: {}", removed.join("\n\t")));
    }
    assert!(!result.is_empty(), "unexpected empty change detection");
    result.join("\n")
}
