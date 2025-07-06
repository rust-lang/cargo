//! Shared download logic between [`HttpRegistry`] and [`RemoteRegistry`].
//!
//! [`HttpRegistry`]: super::http_remote::HttpRegistry
//! [`RemoteRegistry`]: super::remote::RemoteRegistry

use crate::util::interning::InternedString;
use anyhow::Context as _;
use cargo_credential::Operation;
use cargo_util::Sha256;
use cargo_util::registry::make_dep_path;

use crate::core::PackageId;
use crate::core::global_cache_tracker;
use crate::sources::registry::MaybeLock;
use crate::sources::registry::RegistryConfig;
use crate::util::auth;
use crate::util::cache_lock::CacheLockMode;
use crate::util::errors::CargoResult;
use crate::util::{Filesystem, GlobalContext};
use std::fmt::Write as FmtWrite;
use std::fs::{self, File, OpenOptions};
use std::io::SeekFrom;
use std::io::prelude::*;
use std::str;

const CRATE_TEMPLATE: &str = "{crate}";
const VERSION_TEMPLATE: &str = "{version}";
const PREFIX_TEMPLATE: &str = "{prefix}";
const LOWER_PREFIX_TEMPLATE: &str = "{lowerprefix}";
const CHECKSUM_TEMPLATE: &str = "{sha256-checksum}";

/// Checks if `pkg` is downloaded and ready under the directory at `cache_path`.
/// If not, returns a URL to download it from.
///
/// This is primarily called by [`RegistryData::download`](super::RegistryData::download).
pub(super) fn download(
    cache_path: &Filesystem,
    gctx: &GlobalContext,
    encoded_registry_name: InternedString,
    pkg: PackageId,
    checksum: &str,
    registry_config: RegistryConfig,
) -> CargoResult<MaybeLock> {
    let path = cache_path.join(&pkg.tarball_name());
    let path = gctx.assert_package_cache_locked(CacheLockMode::DownloadExclusive, &path);

    // Attempt to open a read-only copy first to avoid an exclusive write
    // lock and also work with read-only filesystems. Note that we check the
    // length of the file like below to handle interrupted downloads.
    //
    // If this fails then we fall through to the exclusive path where we may
    // have to redownload the file.
    if let Ok(dst) = File::open(path) {
        let meta = dst.metadata()?;
        if meta.len() > 0 {
            gctx.deferred_global_last_use()?.mark_registry_crate_used(
                global_cache_tracker::RegistryCrate {
                    encoded_registry_name,
                    crate_filename: pkg.tarball_name().into(),
                    size: meta.len(),
                },
            );
            return Ok(MaybeLock::Ready(dst));
        }
    }

    let mut url = registry_config.dl;
    if !url.contains(CRATE_TEMPLATE)
        && !url.contains(VERSION_TEMPLATE)
        && !url.contains(PREFIX_TEMPLATE)
        && !url.contains(LOWER_PREFIX_TEMPLATE)
        && !url.contains(CHECKSUM_TEMPLATE)
    {
        // Original format before customizing the download URL was supported.
        write!(url, "/{}/{}/download", pkg.name(), pkg.version()).unwrap();
    } else {
        let prefix = make_dep_path(&pkg.name(), true);
        url = url
            .replace(CRATE_TEMPLATE, &*pkg.name())
            .replace(VERSION_TEMPLATE, &pkg.version().to_string())
            .replace(PREFIX_TEMPLATE, &prefix)
            .replace(LOWER_PREFIX_TEMPLATE, &prefix.to_lowercase())
            .replace(CHECKSUM_TEMPLATE, checksum);
    }

    let authorization = if registry_config.auth_required {
        Some(auth::auth_token(
            gctx,
            &pkg.source_id(),
            None,
            Operation::Read,
            vec![],
            true,
        )?)
    } else {
        None
    };

    Ok(MaybeLock::Download {
        url,
        descriptor: pkg.to_string(),
        authorization: authorization,
    })
}

/// Verifies the integrity of `data` with `checksum` and persists it under the
/// directory at `cache_path`.
///
/// This is primarily called by [`RegistryData::finish_download`](super::RegistryData::finish_download).
pub(super) fn finish_download(
    cache_path: &Filesystem,
    gctx: &GlobalContext,
    encoded_registry_name: InternedString,
    pkg: PackageId,
    checksum: &str,
    data: &[u8],
) -> CargoResult<File> {
    // Verify what we just downloaded
    let actual = Sha256::new().update(data).finish_hex();
    if actual != checksum {
        anyhow::bail!("failed to verify the checksum of `{}`", pkg)
    }
    gctx.deferred_global_last_use()?.mark_registry_crate_used(
        global_cache_tracker::RegistryCrate {
            encoded_registry_name,
            crate_filename: pkg.tarball_name().into(),
            size: data.len() as u64,
        },
    );

    cache_path.create_dir()?;
    let path = cache_path.join(&pkg.tarball_name());
    let path = gctx.assert_package_cache_locked(CacheLockMode::DownloadExclusive, &path);
    let mut dst = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("failed to open `{}`", path.display()))?;
    let meta = dst.metadata()?;
    if meta.len() > 0 {
        return Ok(dst);
    }

    dst.write_all(data)?;
    dst.seek(SeekFrom::Start(0))?;
    Ok(dst)
}

/// Checks if a tarball of `pkg` has been already downloaded under the
/// directory at `cache_path`.
///
/// This is primarily called by [`RegistryData::is_crate_downloaded`](super::RegistryData::is_crate_downloaded).
pub(super) fn is_crate_downloaded(
    cache_path: &Filesystem,
    gctx: &GlobalContext,
    pkg: PackageId,
) -> bool {
    let path = cache_path.join(pkg.tarball_name());
    let path = gctx.assert_package_cache_locked(CacheLockMode::DownloadExclusive, &path);
    if let Ok(meta) = fs::metadata(path) {
        return meta.len() > 0;
    }
    false
}
