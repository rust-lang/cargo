use crate::core::{PackageId, SourceId};
use crate::ops;
use crate::sources::registry::make_dep_prefix;
use crate::sources::registry::MaybeLock;
use crate::sources::registry::{
    RegistryConfig, RegistryData, CRATE_TEMPLATE, LOWER_PREFIX_TEMPLATE, PREFIX_TEMPLATE,
    VERSION_TEMPLATE,
};
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::interning::InternedString;
use crate::util::paths;
use crate::util::{Config, Filesystem, Sha256};
use anyhow::Context;
use curl::easy::Easy;
use log::{debug, trace, warn};
use std::cell::{Cell, RefCell, RefMut};
use std::fmt::Write as FmtWrite;
use std::fs::{self, File, OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::Path;
use std::str;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct Version {
    epoch: usize,
    changelog_offset: usize,
}

impl std::str::FromStr for Version {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('.');
        let epoch = parts.next().expect("split always yields one item");
        let epoch = usize::from_str_radix(epoch, 10).map_err(|_| "invalid epoch")?;
        let changelog_offset = parts.next().ok_or("no changelog offset")?;
        let changelog_offset =
            usize::from_str_radix(changelog_offset, 10).map_err(|_| "invalid changelog offset")?;
        Ok(Version {
            epoch,
            changelog_offset,
        })
    }
}

impl ToString for Version {
    fn to_string(&self) -> String {
        format!("{}.{}", self.epoch, self.changelog_offset)
    }
}

// When dynamically linked against libcurl, we want to ignore some failures
// when using old versions that don't support certain features.
//
// NOTE: lifted from src/cargo/core/package.rs
macro_rules! try_old_curl {
    ($e:expr, $msg:expr) => {
        let result = $e;
        if cfg!(target_os = "macos") {
            if let Err(e) = result {
                warn!("ignoring libcurl {} error: {}", $msg, e);
            }
        } else {
            result.with_context(|| {
                anyhow::format_err!("failed to enable {}, is curl not built right?", $msg)
            })?;
        }
    };
}

pub struct HttpRegistry<'cfg> {
    index_path: Filesystem,
    cache_path: Filesystem,
    source_id: SourceId,
    config: &'cfg Config,
    at: Cell<Option<(Version, InternedString)>>,
    checked_for_at: Cell<bool>,
    http: RefCell<Option<Easy>>,
    // dirty: RefCell<HashSet<String>>
}

impl<'cfg> HttpRegistry<'cfg> {
    pub fn new(source_id: SourceId, config: &'cfg Config, name: &str) -> HttpRegistry<'cfg> {
        HttpRegistry {
            index_path: config.registry_index_path().join(name),
            cache_path: config.registry_cache_path().join(name),
            source_id,
            config,
            at: Cell::new(None),
            checked_for_at: Cell::new(false),
            http: RefCell::new(None),
        }
    }

    fn filename(&self, pkg: PackageId) -> String {
        format!("{}-{}.crate", pkg.name(), pkg.version())
    }

    fn http(&self) -> CargoResult<RefMut<'_, Easy>> {
        let handle = if let Ok(h) = self.http.try_borrow_mut() {
            h
        } else {
            anyhow::bail!("concurrent index downloads are not yet supported");
        };

        if handle.is_none() {
            assert!(self.config.offline());
            anyhow::bail!("can't access remote index: you are in offline mode (--offline)");
        } else {
            Ok(RefMut::map(handle, |opt| {
                opt.as_mut().expect("!handle.is_none() implies Some")
            }))
        }
    }
}

const LAST_UPDATED_FILE: &str = ".last-updated";

impl<'cfg> RegistryData for HttpRegistry<'cfg> {
    fn prepare(&self) -> CargoResult<()> {
        if self.at.get().is_none() && !self.checked_for_at.get() {
            self.checked_for_at.set(true);
            let path = self.config.assert_package_cache_locked(&self.index_path);
            if path.exists() {
                let version = paths::read(&path.join(LAST_UPDATED_FILE))?;
                let version: Version = version
                    .parse()
                    .map_err(|e| anyhow::anyhow!("{}", e))
                    .chain_err(|| format!("failed to parse last version: '{}'", version))?;
                let as_str = InternedString::from(version.to_string());
                self.at.set(Some((version, as_str)));
            }
        }

        if !self.config.offline() {
            let mut http = if let Ok(h) = self.http.try_borrow_mut() {
                h
            } else {
                anyhow::bail!("concurrent index downloads are not yet supported");
            };

            if http.is_none() {
                // Ensure that we'll actually be able to acquire an HTTP handle later on
                // once we start trying to download crates. This will weed out any
                // problems with `.cargo/config` configuration related to HTTP.
                //
                // This way if there's a problem the error gets printed before we even
                // hit the index, which may not actually read this configuration.
                let mut handle = ops::http_handle(&self.config)?;
                handle.get(true)?;
                handle.follow_location(true)?;

                // TODO: explicitly enable HTTP2?
                // https://github.com/rust-lang/cargo/blob/905134577c1955ad7865bcf4b31440d4bc882cde/src/cargo/core/package.rs#L651-L703

                // This is an option to `libcurl` which indicates that if there's a
                // bunch of parallel requests to the same host they all wait until the
                // pipelining status of the host is known. This means that we won't
                // initiate dozens of connections to crates.io, but rather only one.
                // Once the main one is opened we realized that pipelining is possible
                // and multiplexing is possible with static.crates.io. All in all this
                // reduces the number of connections done to a more manageable state.
                //
                // NOTE: lifted from src/cargo/core/package.rs
                try_old_curl!(handle.pipewait(true), "pipewait");
                *http = Some(handle);
            }
        }
        Ok(())
    }

    fn index_path(&self) -> &Filesystem {
        // NOTE: pretty sure this method is unnecessary.
        // the only place it is used is to set .path in RegistryIndex,
        // which only uses it to call assert_index_locked below...
        &self.index_path
    }

    fn assert_index_locked<'a>(&self, path: &'a Filesystem) -> &'a Path {
        self.config.assert_package_cache_locked(path)
    }

    fn current_version(&self) -> Option<InternedString> {
        self.at.get().map(|(_, as_str)| as_str)
    }

    fn load(
        &self,
        root: &Path,
        path: &Path,
        data: &mut dyn FnMut(&[u8]) -> CargoResult<()>,
    ) -> CargoResult<()> {
        let pkg = root.join(path);
        if pkg.exists() {
            return data(&paths::read_bytes(&pkg)?);
        }

        let url = self.source_id.url();
        if self.config.offline() {
            anyhow::bail!(
                "can't download index file from '{}': you are in offline mode (--offline)",
                url
            );
        }

        self.prepare()?;
        let mut handle = self.http()?;
        handle.url(&format!("{}{}", url, path.display()))?;

        let mut contents = Vec::new();
        let mut transfer = handle.transfer();
        transfer.write_function(|buf| {
            contents.extend_from_slice(buf);
            Ok(buf.len())
        })?;

        // TODO: should we display transfer status here somehow?

        transfer
            .perform()
            .chain_err(|| format!("failed to fetch index file `{}`", path.display()))?;
        drop(transfer);

        match handle.response_code()? {
            200 => {}
            404 | 410 | 451 => {
                // crate was deleted from the registry.
                // nothing to do here since we already deleted the file from the index.
                // we just won't populate it again.
                anyhow::bail!("crate has been deleted from the registry");
            }
            code => {
                anyhow::bail!("server returned unexpected HTTP status code {}", code);
            }
        }

        paths::write(&root.join(path), &contents)?;
        data(&contents)
    }

    fn config(&mut self) -> CargoResult<Option<RegistryConfig>> {
        debug!("loading config");
        self.prepare()?;
        self.config.assert_package_cache_locked(&self.index_path);
        let mut config = None;
        self.load(Path::new(""), Path::new("config.json"), &mut |json| {
            config = Some(serde_json::from_slice(json)?);
            Ok(())
        })?;
        trace!("config loaded");
        Ok(config)
    }

    fn update_index(&mut self) -> CargoResult<()> {
        if self.config.offline() {
            return Ok(());
        }
        if self.config.cli_unstable().no_index_update {
            return Ok(());
        }
        // Make sure the index is only updated once per session since it is an
        // expensive operation. This generally only happens when the resolver
        // is run multiple times, such as during `cargo publish`.
        if self.config.updated_sources().contains(&self.source_id) {
            return Ok(());
        }

        debug!("updating the index");

        self.prepare()?;
        let path = self.config.assert_package_cache_locked(&self.index_path);
        self.config
            .shell()
            .status("Updating", self.source_id.display_index())?;

        // Fetch the tail of the changelog.
        let url = self.source_id.url();
        // let mut progress = Progress::new("Fetch", config);
        // TODO: retry logic? network::with_retry

        enum ChangelogUse {
            /// We are fetching the changelog with no historical context.
            FirstFetch { full: bool },
            /// We are trying to follow the changelog to update our view of the index.
            Follow(Version),
        }

        let mut handle = self.http()?;
        // TODO: .join? may do the wrong thing if url does not end with /
        handle.url(&format!("{}/changelog", url))?;
        let mut plan = if let Some((version, _)) = self.at.get() {
            ChangelogUse::Follow(version)
        } else {
            ChangelogUse::FirstFetch { full: false }
        };

        let all_dirty = 'changelog: loop {
            // reset in case we looped
            handle.range("")?;
            handle.resume_from(0)?;

            match plan {
                ChangelogUse::Follow(version) => {
                    handle.resume_from(version.changelog_offset as u64)?;
                }
                ChangelogUse::FirstFetch { full: false } => {
                    // we really just need the epoch number and file size,
                    // which we can get at by fetching just the first line.
                    // "1 2019-10-18 23:51:23 ".len() == 22
                    handle.range("0-22")?;
                }
                ChangelogUse::FirstFetch { full: _ } => {}
            }

            let mut contents = Vec::new();
            let mut total_bytes = None;
            let mut transfer = handle.transfer();
            transfer.write_function(|buf| {
                contents.extend_from_slice(buf);
                Ok(buf.len())
            })?;

            transfer.header_function(|buf| {
                const CONTENT_RANGE: &'static [u8] = b"Content-Range:";
                if buf.len() > CONTENT_RANGE.len()
                    && buf[..CONTENT_RANGE.len()].eq_ignore_ascii_case(CONTENT_RANGE)
                {
                    let mut buf = &buf[CONTENT_RANGE.len()..];

                    // trim whitespace
                    while !buf.is_empty() && buf[0] == b' ' {
                        buf = &buf[1..];
                    }

                    // check that the Content-Range unit is indeed bytes
                    const BYTES_UNIT: &'static [u8] = b"bytes ";
                    if !buf.starts_with(BYTES_UNIT) {
                        return true;
                    }
                    buf = &buf[BYTES_UNIT.len()..];

                    // extract out the total length (if known)
                    let rest = buf.splitn(2, |&c| c == b'/');
                    if let Some(complete_length) = rest.skip(1 /* byte-range */).next() {
                        if complete_length.starts_with(b"*") {
                            // total length isn't known
                            // this seems weird, but shrug
                            return true;
                        }
                        let complete_length = complete_length
                            .splitn(2, |&c| c == b' ')
                            .next()
                            .expect("split always yields >= 1 element");
                        if complete_length.into_iter().all(|c| c.is_ascii_digit()) {
                            let complete_length =
                                std::str::from_utf8(complete_length).expect("only ascii digits");
                            total_bytes = Some(
                                usize::from_str_radix(complete_length, 10)
                                    .expect("ascii digits make for valid numbers"),
                            );
                        }
                    }
                }
                true
            })?;

            // TODO: should we show status/progress here?

            transfer
                .perform()
                .chain_err(|| format!("failed to fetch index changelog from `{}`", url))?;
            drop(transfer);

            let mut contents = &contents[..];
            let total_bytes = match handle.response_code()? {
                200 => {
                    // server does not support Range:
                    // so we need to manually slice contents
                    let total_bytes = contents.len();
                    if let ChangelogUse::Follow(version) = plan {
                        if contents.len() < version.changelog_offset {
                            // must have rolled over.
                            // luckily, since the server sent the whole response,
                            // we can just continue as if that was our plan all along.
                            plan = ChangelogUse::FirstFetch { full: true };
                        } else {
                            contents = &contents[version.changelog_offset..];
                        }
                    }
                    total_bytes
                }
                206 => {
                    match total_bytes {
                        None => {
                            match plan {
                                ChangelogUse::FirstFetch { full } => {
                                    assert!(!full, "got partial response without Range:");

                                    // we need to know the total size of the changelog to know our
                                    // next offset. but, the server didn't give that to us when we
                                    // requested just the first few bytes, so we need to do a full
                                    // request.
                                    plan = ChangelogUse::FirstFetch { full: true };
                                    continue;
                                }
                                ChangelogUse::Follow(version) => {
                                    version.changelog_offset + contents.len()
                                }
                            }
                        }
                        Some(b) => b,
                    }
                }
                204 => {
                    // no changes in changelog
                    break false;
                }
                404 => {
                    // server does not have a changelog
                    break true;
                }
                416 => {
                    // Range Not Satisfiable
                    // changelog must have been rolled over
                    if let ChangelogUse::FirstFetch { full: false } = plan {
                        // the changelog is _probably_ empty
                        plan = ChangelogUse::FirstFetch { full: true };
                    } else {
                        plan = ChangelogUse::FirstFetch { full: false };
                    }
                    continue;
                }
                code => {
                    anyhow::bail!("server returned unexpected HTTP status code {}", code);
                }
            };

            let mut line = String::new();
            while contents.read_line(&mut line)? != 0 {
                let mut parts = line.trim().splitn(2, ' ');
                let epoch = parts.next().expect("split always has one element");
                if epoch.is_empty() {
                    // skip empty lines
                    continue;
                }
                let epoch = if let Ok(epoch) = epoch.parse::<usize>() {
                    epoch
                } else {
                    warn!("index {} changelog has invalid lines", url);
                    break 'changelog true;
                };

                let mismatch = match plan {
                    ChangelogUse::FirstFetch { .. } => true,
                    ChangelogUse::Follow(ref version) if version.epoch != epoch => {
                        debug!("index {} changelog has rolled over", url);
                        // TODO: try previous changelog if available?
                        true
                    }
                    ChangelogUse::Follow(_) => false,
                };

                if mismatch {
                    debug!(
                        "index {} is at epoch {} (offset: {})",
                        url, epoch, total_bytes
                    );

                    let version = Version {
                        epoch,
                        changelog_offset: total_bytes,
                    };
                    let as_str = InternedString::from(version.to_string());
                    self.at.set(Some((version, as_str)));

                    break 'changelog true;
                }

                let rest = if let Some(rest) = parts.next() {
                    rest
                } else {
                    warn!("index {} changelog has invalid lines", url);
                    break 'changelog true;
                };
                let mut parts = rest.rsplitn(2, ' ');
                let krate = parts.next().expect("rsplit always has one element");
                if krate.is_empty() {
                    warn!("index {} changelog has invalid lines", url);
                    break 'changelog true;
                }

                // remove the index file -- we'll have to re-fetch it
                let path = path.join(&Path::new(&make_dep_prefix(krate))).join(krate);
                if path.exists() {
                    paths::remove_file(path)?;
                }
            }

            match plan {
                ChangelogUse::Follow(version) => {
                    // update version so that index cache won't be used and load will be called
                    let version = Version {
                        epoch: version.epoch,
                        changelog_offset: total_bytes,
                    };
                    let as_str = InternedString::from(version.to_string());
                    self.at.set(Some((version, as_str)));

                    break false;
                }
                ChangelogUse::FirstFetch { .. } => {
                    // we can only get here if the changelog was empty.
                    // what do we do? we don't know what the current epoch is!
                    // mark everything as dirty and don't write out a version.
                    self.at.set(None);
                    break true;
                }
            }
        };

        // reset the http handle
        handle.range("")?;
        handle.resume_from(0)?;

        if all_dirty {
            // mark all files in index as dirty
            // TODO: this is obviously sub-optimal
            paths::remove_dir_all(&path)?;
        }

        self.config.updated_sources().insert(self.source_id);

        // Record the latest known state of the index.
        if let Some((_, version)) = self.at.get() {
            paths::write(&path.join(LAST_UPDATED_FILE), version.as_bytes())?;
        }

        Ok(())
    }

    fn download(&mut self, pkg: PackageId, _checksum: &str) -> CargoResult<MaybeLock> {
        let filename = self.filename(pkg);

        // Attempt to open an read-only copy first to avoid an exclusive write
        // lock and also work with read-only filesystems. Note that we check the
        // length of the file like below to handle interrupted downloads.
        //
        // If this fails then we fall through to the exclusive path where we may
        // have to redownload the file.
        let path = self.cache_path.join(&filename);
        let path = self.config.assert_package_cache_locked(&path);
        if let Ok(dst) = File::open(&path) {
            let meta = dst.metadata()?;
            if meta.len() > 0 {
                return Ok(MaybeLock::Ready(dst));
            }
        }

        let config = self.config()?.unwrap();
        let mut url = config.dl;
        if !url.contains(CRATE_TEMPLATE)
            && !url.contains(VERSION_TEMPLATE)
            && !url.contains(PREFIX_TEMPLATE)
            && !url.contains(LOWER_PREFIX_TEMPLATE)
        {
            write!(url, "/{}/{}/download", CRATE_TEMPLATE, VERSION_TEMPLATE).unwrap();
        }
        let prefix = make_dep_prefix(&*pkg.name());
        let url = url
            .replace(CRATE_TEMPLATE, &*pkg.name())
            .replace(VERSION_TEMPLATE, &pkg.version().to_string())
            .replace(PREFIX_TEMPLATE, &prefix)
            .replace(LOWER_PREFIX_TEMPLATE, &prefix.to_lowercase());

        Ok(MaybeLock::Download {
            url,
            descriptor: pkg.to_string(),
        })
    }

    fn finish_download(
        &mut self,
        pkg: PackageId,
        checksum: &str,
        data: &[u8],
    ) -> CargoResult<File> {
        // Verify what we just downloaded
        let actual = Sha256::new().update(data).finish_hex();
        if actual != checksum {
            anyhow::bail!("failed to verify the checksum of `{}`", pkg)
        }

        let filename = self.filename(pkg);
        self.cache_path.create_dir()?;
        let path = self.cache_path.join(&filename);
        let path = self.config.assert_package_cache_locked(&path);
        let mut dst = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .chain_err(|| format!("failed to open `{}`", path.display()))?;
        let meta = dst.metadata()?;
        if meta.len() > 0 {
            return Ok(dst);
        }

        dst.write_all(data)?;
        dst.seek(SeekFrom::Start(0))?;
        Ok(dst)
    }

    fn is_crate_downloaded(&self, pkg: PackageId) -> bool {
        let filename = format!("{}-{}.crate", pkg.name(), pkg.version());
        let path = Path::new(&filename);

        let path = self.cache_path.join(path);
        let path = self.config.assert_package_cache_locked(&path);
        if let Ok(meta) = fs::metadata(path) {
            return meta.len() > 0;
        }
        false
    }
}
