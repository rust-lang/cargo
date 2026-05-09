use crate::CargoResult;
use crate::core::{Package, Workspace};
use crate::ops::PackageOpts;
use crate::sources::PathEntry;
use serde::Serialize;

#[derive(Serialize)]
pub struct VcsInfo {
    git: GitVcsInfo,
    path_in_vcs: String,
}

#[derive(Serialize)]
struct GitVcsInfo {
    sha1: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    dirty: bool,
}

pub fn check_repo_state(
    _p: &Package,
    _src_files: &[PathEntry],
    _ws: &Workspace<'_>,
    _opts: &PackageOpts<'_>,
) -> CargoResult<Option<VcsInfo>> {
    Ok(None)
}
