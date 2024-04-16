//! A source that takes other source and patches it with local patch files.
//! See [`PatchedSource`] for details.

use std::path::Path;
use std::path::PathBuf;
use std::task::Poll;

use anyhow::Context as _;
use cargo_util::paths;
use cargo_util::ProcessBuilder;
use cargo_util::Sha256;
use cargo_util_schemas::core::PatchInfo;
use cargo_util_schemas::core::SourceKind;
use lazycell::LazyCell;

use crate::core::Dependency;
use crate::core::Package;
use crate::core::PackageId;
use crate::core::SourceId;
use crate::core::Verbosity;
use crate::sources::source::MaybePackage;
use crate::sources::source::QueryKind;
use crate::sources::source::Source;
use crate::sources::IndexSummary;
use crate::sources::PathSource;
use crate::sources::SourceConfigMap;
use crate::util::cache_lock::CacheLockMode;
use crate::util::hex;
use crate::util::OptVersionReq;
use crate::CargoResult;
use crate::GlobalContext;

/// A file indicates that if present, the patched source is ready to use.
const READY_LOCK: &str = ".cargo-ok";

/// `PatchedSource` is a source that, when fetching, it patches a paticular
/// package with given local patch files.
///
/// This could only be created from [the `[patch]` section][patch] with any
/// entry carrying `{ .., patches = ["..."] }` field. Other kinds of dependency
/// sections (normal, dev, build) shouldn't allow to create any `PatchedSource`.
///
/// [patch]: https://doc.rust-lang.org/nightly/cargo/reference/overriding-dependencies.html#the-patch-section
///
/// ## Filesystem layout
///
/// When Cargo fetches a package from a `PatchedSource`, it'll copy everything
/// from the original source to a dedicated patched source directory. That
/// directory is located under `$CARGO_HOME`. The patched source of each package
/// would be put under:
///
/// ```text
/// $CARGO_HOME/patched-src/<hash-of-original-source>/<pkg>-<version>/<cksum-of-patches>/`.
/// ```
///
/// The file tree of the patched source directory roughly looks like:
///
/// ```text
/// $CARGO_HOME/patched-src/github.com-6d038ece37e82ae2
/// ├── gimli-0.29.0/
/// │  ├── a0d193bd15a5ed96/    # checksum of all patch files from a patch to gimli@0.29.0
/// │  ├── c58e1db3de7c154d/
/// └── serde-1.0.197/
///    └── deadbeef12345678/
/// ```
///
/// ## `SourceId` for tracking the original package
///
/// Due to the nature that a patched source is actually locked to a specific
/// version of one package, the SourceId URL of a `PatchedSource` needs to
/// carry such information. It looks like:
///
/// ```text
/// patched+registry+https://github.com/rust-lang/crates.io-index?name=foo&version=1.0.0&patch=0001-bugfix.patch
/// ```
///
/// where the `patched+` protocol is essential for Cargo to distinguish between
/// a patched source and the source it patches. The query string contains the
/// name and version of the package being patched. We want patches to be as
/// reproducible as it could, so lock to one specific version here.
/// See [`PatchInfo::from_query`] to learn what are being tracked.
///
/// To achieve it, the version specified in any of the entry in `[patch]` must
/// be an exact version via the `=` SemVer comparsion operator. For example,
/// this will fetch source of serde@1.2.3 from crates.io, and apply patches to it.
///
/// ```toml
/// [patch.crates-io]
/// serde = { version = "=1.2.3", patches = ["patches/0001-serde-bug.patch"] }
/// ```
///
/// ## Patch tools
///
/// When patching a package, Cargo will change the working directory to
/// the root directory of the copied source code, and then execute the tool
/// specified via the `patchtool.path` config value in the Cargo configuration.
/// Paths of patch files will be provided as absolute paths to the tool.
pub struct PatchedSource<'gctx> {
    source_id: SourceId,
    /// The source of the package we're going to patch.
    original_source: Box<dyn Source + 'gctx>,
    /// Checksum from all patch files.
    patches_checksum: LazyCell<String>,
    /// For respecting `[source]` replacement configuration.
    map: SourceConfigMap<'gctx>,
    path_source: Option<PathSource<'gctx>>,
    quiet: bool,
    gctx: &'gctx GlobalContext,
}

impl<'gctx> PatchedSource<'gctx> {
    pub fn new(
        source_id: SourceId,
        gctx: &'gctx GlobalContext,
    ) -> CargoResult<PatchedSource<'gctx>> {
        let original_id = {
            let mut url = source_id.url().clone();
            url.set_query(None);
            url.set_fragment(None);
            let url = url.as_str();
            let Some(url) = url.strip_prefix("patched+") else {
                anyhow::bail!("patched source url requires a `patched` scheme, got `{url}`");
            };
            SourceId::from_url(&url)?
        };
        let map = SourceConfigMap::new(gctx)?;
        let source = PatchedSource {
            source_id,
            original_source: map.load(original_id, &Default::default())?,
            patches_checksum: LazyCell::new(),
            map,
            path_source: None,
            quiet: false,
            gctx,
        };
        Ok(source)
    }

    /// Downloads the package source if needed.
    fn download_pkg(&mut self) -> CargoResult<Package> {
        let patch_info = self.patch_info();
        let exact_req = &format!("={}", patch_info.version());
        let original_id = self.original_source.source_id();
        let dep = Dependency::parse(patch_info.name(), Some(exact_req), original_id)?;
        let pkg_id = loop {
            match self.original_source.query_vec(&dep, QueryKind::Exact) {
                Poll::Ready(deps) => break deps?.remove(0).as_summary().package_id(),
                Poll::Pending => self.original_source.block_until_ready()?,
            }
        };

        let source = self.map.load(original_id, &Default::default())?;
        Box::new(source).download_now(pkg_id, self.gctx)
    }

    fn copy_pkg_src(&self, pkg: &Package, dst: &Path) -> CargoResult<()> {
        let src = pkg.root();
        for entry in walkdir::WalkDir::new(src) {
            let entry = entry?;
            let path = entry.path().strip_prefix(src).unwrap();
            let src = entry.path();
            let dst = dst.join(path);
            if entry.file_type().is_dir() {
                paths::create_dir_all(dst)?;
            } else {
                // TODO: handle symlink?
                paths::copy(src, dst)?;
            }
        }
        Ok(())
    }

    fn apply_patches(&self, pkg: &Package, dst: &Path) -> CargoResult<()> {
        let patches = self.patch_info().patches();
        let n = patches.len();
        assert!(n > 0, "must have at least one patch, got {n}");

        self.gctx.shell().status("Patching", pkg)?;

        let patchtool_config = self.gctx.patchtool_config()?;
        let Some(tool) = patchtool_config.path.as_ref() else {
            anyhow::bail!("missing `[patchtool]` for patching dependencies");
        };

        let program = tool.path.resolve_program(self.gctx);
        let mut cmd = ProcessBuilder::new(program);
        cmd.cwd(dst).args(&tool.args);

        for patch_path in patches {
            let patch_path = self.gctx.cwd().join(patch_path);
            let mut cmd = cmd.clone();
            cmd.arg(patch_path);
            if matches!(self.gctx.shell().verbosity(), Verbosity::Verbose) {
                self.gctx.shell().status("Running", &cmd)?;
                cmd.exec()?;
            } else {
                cmd.exec_with_output()?;
            }
        }

        Ok(())
    }

    /// Gets the destination directory we put the patched source at.
    fn dest_src_dir(&self, pkg: &Package) -> CargoResult<PathBuf> {
        let patched_src_root = self.gctx.patched_source_path();
        let patched_src_root = self
            .gctx
            .assert_package_cache_locked(CacheLockMode::DownloadExclusive, &patched_src_root);
        let pkg_id = pkg.package_id();
        let source_id = pkg_id.source_id();
        let ident = source_id.url().host_str().unwrap_or_default();
        let hash = hex::short_hash(&source_id);
        let name = pkg_id.name();
        let version = pkg_id.version();
        let mut dst = patched_src_root.join(format!("{ident}-{hash}"));
        dst.push(format!("{name}-{version}"));
        dst.push(self.patches_checksum()?);
        Ok(dst)
    }

    fn patches_checksum(&self) -> CargoResult<&String> {
        self.patches_checksum.try_borrow_with(|| {
            let mut cksum = Sha256::new();
            for patch in self.patch_info().patches() {
                cksum.update_path(patch)?;
            }
            let mut cksum = cksum.finish_hex();
            // TODO: is it safe to truncate sha256?
            cksum.truncate(16);
            Ok(cksum)
        })
    }

    fn patch_info(&self) -> &PatchInfo {
        let SourceKind::Patched(info) = self.source_id.kind() else {
            panic!("patched source must be SourceKind::Patched");
        };
        info
    }
}

impl<'gctx> Source for PatchedSource<'gctx> {
    fn source_id(&self) -> SourceId {
        self.source_id
    }

    fn supports_checksums(&self) -> bool {
        false
    }

    fn requires_precise(&self) -> bool {
        false
    }

    fn query(
        &mut self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(IndexSummary),
    ) -> Poll<CargoResult<()>> {
        // Version requirement here is still the `=` exact one for fetching
        // the source to patch, so switch it to a wildchard requirement.
        // It is safe because this source contains one and the only package.
        let mut dep = dep.clone();
        dep.set_version_req(OptVersionReq::Any);
        if let Some(src) = self.path_source.as_mut() {
            src.query(&dep, kind, f)
        } else {
            Poll::Pending
        }
    }

    fn invalidate_cache(&mut self) {
        // No cache for a patched source
    }

    fn set_quiet(&mut self, quiet: bool) {
        self.quiet = quiet;
    }

    fn download(&mut self, id: PackageId) -> CargoResult<MaybePackage> {
        self.path_source
            .as_mut()
            .expect("path source must exist")
            .download(id)
    }

    fn finish_download(&mut self, _pkg_id: PackageId, _contents: Vec<u8>) -> CargoResult<Package> {
        panic!("no download should have started")
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        let fingerprint = self.original_source.fingerprint(pkg)?;
        let cksum = self.patches_checksum()?;
        Ok(format!("{fingerprint}/{cksum}"))
    }

    fn describe(&self) -> String {
        use std::fmt::Write as _;
        let mut desc = self.original_source.describe();
        let n = self.patch_info().patches().len();
        let plural = if n == 1 { "" } else { "s" };
        write!(desc, " with {n} patch file{plural}").unwrap();
        desc
    }

    fn add_to_yanked_whitelist(&mut self, _pkgs: &[PackageId]) {
        // There is no yanked package for a patched source
    }

    fn is_yanked(&mut self, _pkg: PackageId) -> Poll<CargoResult<bool>> {
        // There is no yanked package for a patched source
        Poll::Ready(Ok(false))
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        if self.path_source.is_some() {
            return Ok(());
        }

        let pkg = self.download_pkg().context("failed to download source")?;
        let dst = self.dest_src_dir(&pkg)?;

        let ready_lock = dst.join(READY_LOCK);
        let cksum = self.patches_checksum()?;
        match paths::read(&ready_lock) {
            Ok(prev_cksum) if &prev_cksum == cksum => {
                // We've applied patches. Assume they never change.
            }
            _ => {
                // Either we were interrupted, or never get started.
                // We just start over here.
                if let Err(e) = paths::remove_dir_all(&dst) {
                    tracing::trace!("failed to remove `{}`: {e}", dst.display());
                }
                self.copy_pkg_src(&pkg, &dst)
                    .context("failed to copy source")?;
                self.apply_patches(&pkg, &dst)
                    .context("failed to apply patches")?;
                paths::write(&ready_lock, cksum)?;
            }
        }

        self.path_source = Some(PathSource::new(&dst, self.source_id, self.gctx));

        Ok(())
    }
}
