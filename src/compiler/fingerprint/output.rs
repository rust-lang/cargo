use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Context;
use filetime::FileTime;
use serde::{Deserialize, Serialize};

use super::dep_info::{CHECKSUM_ALGO, Checksum, ChecksumAlgo};
use crate::util::data_structures::HashMap;
use crate::util::errors::CargoResult;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OutputId(#[serde(with = "output_id")] pub u128);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OutputFingerprint {
    /// An ID representing some collection of output files.
    pub output_id: OutputId,
    /// The last time the output files where checked.
    /// This is only updated if the mtime of an output changes, forcing a recheck
    #[serde(with = "unix_millis")]
    pub checked_at: SystemTime,
    /// A set of output files and their checksums
    pub checksums: BTreeMap<PathBuf, Checksum>,
    /// The build unit's out dir
    #[serde(skip)]
    pub out_dir: PathBuf,
    /// If the outputs changed indicating that we need to rewrite the fingerprint
    /// with the update output fingerprint
    #[serde(skip)]
    pub refreshed: bool,
}

impl OutputFingerprint {
    pub fn new(out_dir: PathBuf) -> Self {
        Self {
            output_id: OutputId(rand::random()),
            checked_at: SystemTime::UNIX_EPOCH,
            checksums: BTreeMap::new(),
            out_dir,
            refreshed: false,
        }
    }

    /// Parses the output from the "short" fingerprint (as opposed to the JSON fingerprint)
    ///
    /// Importantly this does not populate checksums since the short fingerprint does not store
    /// the checksum of each output file.
    pub fn from_short_fingerprint(short: &str, root: PathBuf) -> Option<Self> {
        let mut fields = short.split('\n');
        let hash = fields.next()?;
        if hash.len() != 16 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let id = fields.next()?;
        if id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let millis = fields.next()?;
        if millis.is_empty() || !millis.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        Some(Self {
            checked_at: unix_millis::from_millis(millis.parse().ok()?)?,
            output_id: OutputId(u128::from_str_radix(id, 16).ok()?),
            checksums: BTreeMap::new(),
            out_dir: root,
            refreshed: false,
        })
    }

    /// Compares the build unit's outputs and returns Ok(true) if they have not been changed
    pub fn compare(
        &mut self,
        outputs: &[PathBuf],
        mtimes: &HashMap<PathBuf, FileTime>,
        load: impl FnOnce() -> CargoResult<Self>,
    ) -> CargoResult<bool> {
        if self.checked_at == SystemTime::UNIX_EPOCH {
            return Ok(false);
        }

        let now = SystemTime::now();
        let checked_at = FileTime::from_system_time(self.checked_at);
        if now >= self.checked_at && mtimes.values().all(|mtime| *mtime < checked_at) {
            return Ok(true);
        }

        // At least one of the output files mtimes has changed, so we need to load the previous
        // fingerprint and check if the checksums of the outputs changed.

        let previous = load()?;
        if previous.output_id != self.output_id
            || previous.checked_at != self.checked_at
            || previous.checksums.len() != outputs.len()
        {
            return Ok(false);
        }

        if outputs.iter().any(|path| {
            !previous
                .checksums
                .contains_key(self.relative_path(path).as_ref())
        }) {
            return Ok(false);
        }

        let Some(checksums) =
            self.compute_checksums(outputs, |path| previous.checksums[path].algo())?
        else {
            return Ok(false);
        };
        if checksums != previous.checksums {
            return Ok(false);
        }
        self.checksums = checksums;
        self.checked_at = now;
        self.refreshed = true;
        Ok(true)
    }

    pub fn begin_build(&mut self) {
        self.output_id = OutputId(rand::random());
    }

    /// Returns false if an expected output is missing.
    pub fn finish_build(&mut self, outputs: &[PathBuf]) -> CargoResult<bool> {
        let checked_at = SystemTime::now();
        let Some(checksums) = self.compute_checksums(outputs, |_| CHECKSUM_ALGO)? else {
            return Ok(false);
        };
        self.checksums = checksums;
        self.checked_at = checked_at;
        Ok(true)
    }

    fn compute_checksums(
        &self,
        outputs: &[PathBuf],
        algo: impl Fn(&Path) -> ChecksumAlgo,
    ) -> CargoResult<Option<BTreeMap<PathBuf, Checksum>>> {
        let mut checksums = BTreeMap::new();
        for path in outputs {
            let relative = self.relative_path(path).into_owned();
            let checksum = match Self::checksum_output(path, algo(&relative)) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                result => result
                    .with_context(|| format!("failed to checksum output `{}`", path.display()))?,
            };
            checksums.insert(relative, checksum);
        }
        Ok(Some(checksums))
    }

    fn checksum_output(path: &Path, algo: ChecksumAlgo) -> io::Result<Checksum> {
        let mut file = File::open(path)?;
        let checksum = Checksum::compute(algo, &mut file)?;
        Ok(checksum)
    }

    fn relative_path<'a>(&self, path: &'a Path) -> Cow<'a, Path> {
        match path.strip_prefix(&self.out_dir) {
            Ok(path) => Cow::Borrowed(path),
            Err(_) => match pathdiff::diff_paths(path, &self.out_dir) {
                Some(path) => Cow::Owned(path),
                None => Cow::Borrowed(path),
            },
        }
    }
}

mod unix_millis {
    use std::time::{Duration, SystemTime};

    use serde::{Deserialize, Deserializer, Serializer};

    use crate::util::errors::CargoResult;

    pub fn serialize<S: Serializer>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(to_millis(time).map_err(serde::ser::Error::custom)?)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<SystemTime, D::Error> {
        let millis = u64::deserialize(deserializer)?;
        from_millis(millis)
            .ok_or_else(|| serde::de::Error::custom("Unix millisecond timestamp out of range"))
    }

    fn to_millis(time: &SystemTime) -> CargoResult<u64> {
        Ok(time
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis()
            .try_into()?)
    }

    pub fn from_millis(millis: u64) -> Option<SystemTime> {
        SystemTime::UNIX_EPOCH.checked_add(Duration::from_millis(millis))
    }
}

mod output_id {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(id: &u128, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format_args!("{id:032x}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u128, D::Error> {
        let id = String::deserialize(deserializer)?;
        u128::from_str_radix(&id, 16).map_err(serde::de::Error::custom)
    }
}
