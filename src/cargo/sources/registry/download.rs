use anyhow::Context;
use cargo_util::Sha256;

use crate::core::PackageId;
use crate::sources::registry::make_dep_prefix;
use crate::sources::registry::MaybeLock;
use crate::sources::registry::{
    RegistryConfig, CHECKSUM_TEMPLATE, CRATE_TEMPLATE, LOWER_PREFIX_TEMPLATE, PREFIX_TEMPLATE,
    VERSION_TEMPLATE,
};
use crate::util::auth;
use crate::util::errors::CargoResult;
use crate::util::{Config, Filesystem};
use std::fmt::Write as FmtWrite;
use std::fs::{self, File, OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::str;

pub(super) fn filename(pkg: PackageId) -> String {
    format!("{}-{}.crate", pkg.name(), pkg.version())
}

pub(super) fn download(
    cache_path: &Filesystem,
    config: &Config,
    pkg: PackageId,
    checksum: &str,
    registry_config: RegistryConfig,
) -> CargoResult<MaybeLock> {
    let filename = filename(pkg);
    let path = cache_path.join(&filename);
    let path = config.assert_package_cache_locked(&path);

    // Attempt to open a read-only copy first to avoid an exclusive write
    // lock and also work with read-only filesystems. Note that we check the
    // length of the file like below to handle interrupted downloads.
    //
    // If this fails then we fall through to the exclusive path where we may
    // have to redownload the file.
    if let Ok(dst) = File::open(path) {
        let meta = dst.metadata()?;
        if meta.len() > 0 {
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
        write!(
            url,
            "/{}/{}/download",
            pkg.name(),
            pkg.version().to_string()
        )
        .unwrap();
    } else {
        let prefix = make_dep_prefix(&*pkg.name());
        url = url
            .replace(CRATE_TEMPLATE, &*pkg.name())
            .replace(VERSION_TEMPLATE, &pkg.version().to_string())
            .replace(PREFIX_TEMPLATE, &prefix)
            .replace(LOWER_PREFIX_TEMPLATE, &prefix.to_lowercase())
            .replace(CHECKSUM_TEMPLATE, checksum);
    }

    let authorization = if registry_config.auth_required {
        Some(auth::auth_token(config, &pkg.source_id(), None, None)?)
    } else {
        None
    };

    Ok(MaybeLock::Download {
        url,
        descriptor: pkg.to_string(),
        authorization: authorization,
    })
}

pub(super) fn finish_download(
    cache_path: &Filesystem,
    config: &Config,
    pkg: PackageId,
    checksum: &str,
    data: &[u8],
) -> CargoResult<File> {
    // Verify what we just downloaded
    let actual = Sha256::new().update(data).finish_hex();
    if actual != checksum {
        anyhow::bail!("failed to verify the checksum of `{}`", pkg)
    }

    let filename = filename(pkg);
    cache_path.create_dir()?;
    let path = cache_path.join(&filename);
    let path = config.assert_package_cache_locked(&path);
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

pub(super) fn is_crate_downloaded(
    cache_path: &Filesystem,
    config: &Config,
    pkg: PackageId,
) -> bool {
    let path = cache_path.join(filename(pkg));
    let path = config.assert_package_cache_locked(&path);
    if let Ok(meta) = fs::metadata(path) {
        return meta.len() > 0;
    }
    false
}
