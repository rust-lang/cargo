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
use curl::easy::{Easy, List};
use log::{debug, trace, warn};
use std::cell::{Cell, RefCell, RefMut};
use std::fmt::Write as FmtWrite;
use std::fs::{self, File, OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::Path;
use std::str;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ChangelogState {
    Unknown,
    Unsupported,
    Synchronized {
        epoch: usize,
        changelog_offset: usize,
    },
}

impl ChangelogState {
    fn is_synchronized(&self) -> bool {
        matches!(self, ChangelogState::Synchronized { .. })
    }
    fn is_unknown(&self) -> bool {
        matches!(self, ChangelogState::Unknown)
    }
    fn is_unsupported(&self) -> bool {
        matches!(self, ChangelogState::Unsupported)
    }
}

impl Into<(ChangelogState, InternedString)> for ChangelogState {
    fn into(self) -> (ChangelogState, InternedString) {
        let is = InternedString::from(self.to_string());
        (self, is)
    }
}

impl std::str::FromStr for ChangelogState {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "unknown" {
            return Ok(ChangelogState::Unknown);
        }
        if s == "unsupported" {
            return Ok(ChangelogState::Unsupported);
        }

        let mut parts = s.split('.');
        let epoch = parts.next().expect("split always yields one item");
        let epoch = usize::from_str_radix(epoch, 10).map_err(|_| "invalid epoch")?;
        let changelog_offset = parts.next().ok_or("no changelog offset")?;
        let changelog_offset =
            usize::from_str_radix(changelog_offset, 10).map_err(|_| "invalid changelog offset")?;
        Ok(ChangelogState::Synchronized {
            epoch,
            changelog_offset,
        })
    }
}

impl ToString for ChangelogState {
    fn to_string(&self) -> String {
        match *self {
            ChangelogState::Unknown => String::from("unknown"),
            ChangelogState::Unsupported => String::from("unsupported"),
            ChangelogState::Synchronized {
                epoch,
                changelog_offset,
            } => format!("{}.{}", epoch, changelog_offset),
        }
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
    at: Cell<(ChangelogState, InternedString)>,
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
            at: Cell::new(ChangelogState::Unknown.into()),
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
        if self.at.get().0.is_unknown() && !self.checked_for_at.get() {
            self.checked_for_at.set(true);
            let path = self.config.assert_package_cache_locked(&self.index_path);
            if path.exists() {
                let cl_state = paths::read(&path.join(LAST_UPDATED_FILE))?;
                let cl_state: ChangelogState = cl_state
                    .parse()
                    .map_err(|e| anyhow::anyhow!("{}", e))
                    .chain_err(|| {
                        format!("failed to parse last changelog state: '{}'", cl_state)
                    })?;
                self.at.set(cl_state.into());
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
        let cl_state = self.at.get();
        if cl_state.0.is_unknown() {
            None
        } else {
            Some(cl_state.1)
        }
    }

    fn load(
        &self,
        root: &Path,
        path: &Path,
        data: &mut dyn FnMut(&[u8]) -> CargoResult<()>,
    ) -> CargoResult<()> {
        let pkg = root.join(path);
        let bytes;
        let was = if pkg.exists() {
            bytes = paths::read_bytes(&pkg)?;
            let mut lines = bytes.splitn(3, |&c| c == b'\n');
            let etag = lines.next().expect("splitn always returns >=1 item");
            let last_modified = if let Some(lm) = lines.next() {
                lm
            } else {
                anyhow::bail!("index file is missing HTTP header header");
            };
            let rest = if let Some(rest) = lines.next() {
                rest
            } else {
                anyhow::bail!("index file is missing HTTP header header");
            };

            if !self.at.get().0.is_unsupported() || self.config.offline() {
                return data(rest);
            } else {
                // we cannot trust the index files -- need to check with server
                let etag = std::str::from_utf8(etag)?;
                let last_modified = std::str::from_utf8(last_modified)?;
                Some((etag, last_modified))
            }
        } else {
            None
        };

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

        if let Some((etag, last_modified)) = was {
            let mut list = List::new();
            list.append(&format!("If-None-Match: {}", etag))?;
            list.append(&format!("If-Modified-Since: {}", last_modified))?;
            handle.http_headers(list)?;
        }

        let mut contents = Vec::new();
        let mut etag = None;
        let mut last_modified = None;
        let mut transfer = handle.transfer();
        transfer.write_function(|buf| {
            contents.extend_from_slice(buf);
            Ok(buf.len())
        })?;

        // capture ETag and Last-Modified
        transfer.header_function(|buf| {
            const ETAG: &'static [u8] = b"ETag:";
            const LAST_MODIFIED: &'static [u8] = b"Last-Modified:";

            let (tag, buf) =
                if buf.len() >= ETAG.len() && buf[..ETAG.len()].eq_ignore_ascii_case(ETAG) {
                    (ETAG, &buf[ETAG.len()..])
                } else if buf.len() >= LAST_MODIFIED.len()
                    && buf[..LAST_MODIFIED.len()].eq_ignore_ascii_case(LAST_MODIFIED)
                {
                    (LAST_MODIFIED, &buf[LAST_MODIFIED.len()..])
                } else {
                    return true;
                };

            // don't let server sneak more lines into index file
            if buf.contains(&b'\n') {
                return true;
            }

            if let Ok(buf) = std::str::from_utf8(buf) {
                let buf = buf.trim();
                let mut s = String::with_capacity(buf.len() + 1);
                s.push_str(buf);
                s.push('\n');
                if tag == ETAG {
                    etag = Some(s);
                } else if tag == LAST_MODIFIED {
                    last_modified = Some(s);
                }
            }

            true
        })?;

        // TODO: should we display transfer status here somehow?

        transfer
            .perform()
            .chain_err(|| format!("failed to fetch index file `{}`", path.display()))?;
        drop(transfer);

        // don't send If-Modified-Since with future requests
        let mut list = List::new();
        list.append("If-Modified-Since:")?;
        handle.http_headers(list)?;

        match handle.response_code()? {
            200 => {}
            304 => {
                // not modified
                assert!(was.is_some());
            }
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

        let mut file = paths::create(&root.join(path))?;
        file.write_all(etag.as_deref().unwrap_or("\n").as_bytes())?;
        file.write_all(last_modified.as_deref().unwrap_or("\n").as_bytes())?;
        file.write_all(&contents)?;
        file.flush()?;
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
            Follow {
                epoch: usize,
                changelog_offset: usize,
            },
        }

        let mut handle = self.http()?;
        // TODO: .join? may do the wrong thing if url does not end with /
        handle.url(&format!("{}/changelog", url))?;
        let mut plan = if let ChangelogState::Synchronized {
            epoch,
            changelog_offset,
        } = self.at.get().0
        {
            ChangelogUse::Follow {
                epoch,
                changelog_offset,
            }
        } else {
            ChangelogUse::FirstFetch { full: false }
        };

        'changelog: loop {
            // reset in case we looped
            handle.range("")?;
            handle.resume_from(0)?;

            match plan {
                ChangelogUse::Follow {
                    changelog_offset, ..
                } => {
                    handle.resume_from(changelog_offset as u64)?;
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
                    if let ChangelogUse::Follow {
                        changelog_offset, ..
                    } = plan
                    {
                        if contents.len() < changelog_offset {
                            // must have rolled over.
                            // luckily, since the server sent the whole response,
                            // we can just continue as if that was our plan all along.
                            plan = ChangelogUse::FirstFetch { full: true };
                        } else {
                            contents = &contents[changelog_offset..];
                            if contents.is_empty() {
                                // no changes in changelog
                                break;
                            }
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
                                ChangelogUse::Follow {
                                    changelog_offset, ..
                                } => changelog_offset + contents.len(),
                            }
                        }
                        Some(b) => b,
                    }
                }
                204 => {
                    // no changes in changelog
                    assert!(self.at.get().0.is_synchronized());
                    break;
                }
                404 => {
                    // server does not have a changelog
                    if self.at.get().0.is_synchronized() {
                        // we used to have a changelog, but now we don't. it's important that we
                        // record that fact so that later calls to load() will all double-check
                        // with the server.
                        self.at.set(ChangelogState::Unsupported.into());
                    }
                    break;
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
            let mut new_changelog = false;
            let mut fetched_epoch = None;
            while contents.read_line(&mut line)? != 0 {
                let mut parts = line.trim().splitn(2, ' ');
                let epoch = parts.next().expect("split always has one element");
                if epoch.is_empty() {
                    // skip empty lines
                    continue;
                }
                let epoch = if let Ok(epoch) = epoch.parse::<usize>() {
                    fetched_epoch = Some(epoch);
                    epoch
                } else {
                    warn!("index {} changelog has invalid lines", url);
                    // ensure that all future index fetches check with server
                    self.at.set(ChangelogState::Unsupported.into());
                    break 'changelog;
                };

                match plan {
                    ChangelogUse::FirstFetch { .. } => {
                        new_changelog = true;

                        // we don't actually care about the remainder of the changelog,
                        // since we've completely purged our local index.
                        break;
                    }
                    ChangelogUse::Follow {
                        epoch: last_epoch, ..
                    } if last_epoch != epoch => {
                        debug!("index {} changelog has rolled over", url);
                        // TODO: try previous changelog if available?

                        new_changelog = true;
                        break;
                    }
                    ChangelogUse::Follow { .. } => {}
                }

                let rest = if let Some(rest) = parts.next() {
                    rest
                } else {
                    warn!("index {} changelog has invalid lines", url);
                    // ensure that all future index fetches check with server
                    self.at.set(ChangelogState::Unsupported.into());
                    break 'changelog;
                };
                let mut parts = rest.rsplitn(2, ' ');
                let krate = parts.next().expect("rsplit always has one element");
                if krate.is_empty() {
                    warn!("index {} changelog has invalid lines", url);
                    // ensure that all future index fetches check with server
                    self.at.set(ChangelogState::Unsupported.into());
                    break 'changelog;
                }

                // remove the index file -- we'll have to re-fetch it
                let path = path.join(&Path::new(&make_dep_prefix(krate))).join(krate);
                if path.exists() {
                    paths::remove_file(path)?;
                }
            }

            if total_bytes == 0 {
                // the changelog has rolled over, but we didn't realize since we didn't actually
                // _observe_ another epoch number. catch that here.
                new_changelog = true;
            }

            if new_changelog {
                if let Some(epoch) = fetched_epoch {
                    debug!(
                        "index {} is at epoch {} (offset: {})",
                        url, epoch, total_bytes
                    );

                    // we don't know which index entries are now invalid and which are not.
                    // so we purge them all.
                    // XXX: will this cause issues with directory locking?
                    paths::remove_dir_all(&path)?;
                    paths::create_dir_all(&path)?;

                    // but from this point forward we're synchronized
                    self.at.set(
                        ChangelogState::Synchronized {
                            epoch,
                            changelog_offset: total_bytes,
                        }
                        .into(),
                    );
                } else {
                    // we have a new changelog, but we don't know what the epoch of that changelog
                    // is since it was empty (otherwise fetched_epoch would be Some).
                    self.at.set(ChangelogState::Unknown.into());
                }
                break;
            }

            // keep track of our new byte offset in the changelog
            let epoch = fetched_epoch.expect("changelog was non-empty (total_bytes != 0)");
            self.at.set(
                ChangelogState::Synchronized {
                    epoch,
                    changelog_offset: total_bytes,
                }
                .into(),
            );
            break;
        }

        // reset the http handle
        handle.range("")?;
        handle.resume_from(0)?;

        self.config.updated_sources().insert(self.source_id);

        // Record the latest known state of the index.
        paths::write(&path.join(LAST_UPDATED_FILE), self.at.get().1.as_bytes())?;

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
