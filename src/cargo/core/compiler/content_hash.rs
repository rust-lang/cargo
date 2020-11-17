use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::num::NonZeroU64;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use filetime::FileTime;
use log::debug;
use md5::{Digest, Md5};
use object::Object;
use serde;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::Sha256;

/// A file location with identifying properties: size and hash.
#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub struct Fileprint {
    pub path: PathBuf, //TODO is this field needed on here?
    pub size: Option<FileSize>,
    pub hash: Option<FileHash>,
}

impl Fileprint {
    pub(crate) fn from_md5(path: PathBuf) -> Self {
        let size = CurrentFileprint::calc_size(&path);
        let hash = CurrentFileprint::calc_hash(&path, FileHashAlgorithm::Md5);
        Self { path, size, hash }
    }
}

#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Debug, Serialize, Deserialize, Hash)]
pub enum FileHashAlgorithm {
    /// Svh is embedded as a symbol or for rmeta is in the .rmeta filename inside a .rlib.
    Svh,
    Md5,
    Sha1,
    Sha256,
}

impl FromStr for FileHashAlgorithm {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<FileHashAlgorithm, Self::Err> {
        match s {
            "md5" => Ok(FileHashAlgorithm::Md5),
            "svh" => Ok(FileHashAlgorithm::Svh),
            "sha1" => Ok(FileHashAlgorithm::Sha1),
            "sha256" => Ok(FileHashAlgorithm::Sha256),
            _ => Err(anyhow::Error::msg("Unknown hash type")),
        }
    }
}

impl std::fmt::Display for FileHashAlgorithm {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Self::Md5 => fmt.write_fmt(format_args!("md5"))?,
            Self::Svh => fmt.write_fmt(format_args!("svh"))?,
            Self::Sha1 => fmt.write_fmt(format_args!("sha1"))?,
            Self::Sha256 => fmt.write_fmt(format_args!("sha256"))?,
        };
        Ok(())
    }
}

// While source files can't currently be > 4Gb, bin files could be.
pub type FileSize = NonZeroU64;

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FileHash {
    kind: FileHashAlgorithm,
    // arrays > 32 are currently hard work so broken in twain.
    hash_front: [u8; 32],
    hash_back: [u8; 32],
}

impl FileHash {
    pub fn from_hex_rev(kind: FileHashAlgorithm, hash: &str) -> Option<FileHash> {
        let mut decoded = hex::decode(hash).ok()?;
        decoded.reverse(); // The slice is stored as little endien.
        Some(Self::from_slice(kind, &decoded[..]))
    }

    pub fn from_hex(kind: FileHashAlgorithm, hash: &str) -> Option<FileHash> {
        let decoded = hex::decode(hash).ok()?;
        Some(Self::from_slice(kind, &decoded[..]))
    }

    pub fn from_slice_rev(kind: FileHashAlgorithm, hash: &[u8]) -> FileHash {
        let mut v = hash.to_vec();
        v.reverse();
        Self::from_slice(kind, &v)
    }

    pub fn from_slice(kind: FileHashAlgorithm, hash: &[u8]) -> FileHash {
        let mut result = FileHash {
            kind,
            hash_front: [0u8; 32],
            hash_back: [0u8; 32],
        };
        let len = hash.len();
        let front_len = std::cmp::min(len, 32);
        (&mut result.hash_front[..front_len]).copy_from_slice(&hash[..front_len]);
        if len > 32 {
            let back_len = std::cmp::min(len, 64);
            (&mut result.hash_back[..back_len - 32]).copy_from_slice(&hash[32..back_len]);
        }
        result
    }

    pub fn write_to_vec(&self, vec: &mut Vec<u8>) {
        vec.push(match self.kind {
            FileHashAlgorithm::Md5 => 1,
            FileHashAlgorithm::Sha1 => 2,
            FileHashAlgorithm::Sha256 => 3,
            FileHashAlgorithm::Svh => 4,
        });
        vec.extend_from_slice(&self.hash_front[..]);
        vec.extend_from_slice(&self.hash_back[..]);
    }
}

impl fmt::Display for FileHash {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            formatter,
            "{}:{}{}",
            self.kind,
            hex::encode(self.hash_front),
            hex::encode(self.hash_back)
        )
    }
}

fn get_svh_from_ar<R: Read>(reader: R) -> Option<FileHash> {
    let mut ar = ar::Archive::new(reader);
    while let Some(file) = ar.next_entry() {
        match file {
            Ok(file) => {
                let s = String::from_utf8_lossy(&file.header().identifier());
                if s.ends_with(".rmeta") {
                    if let Some(index) = s.rfind('-') {
                        return FileHash::from_hex_rev(
                            FileHashAlgorithm::Svh,
                            &s[index + 1..(s.len() - ".rmeta".len())],
                        );
                    }
                }
            }
            Err(err) => debug!("Error reading ar: {}", err),
        }
    }
    debug!("HASH svh not found in archive file.");
    None
}

// While this looks expensive, this is only invoked for dylibs
// with an incorrect timestamp the file is the expected size.
fn get_svh_from_object_file<R: Read>(mut reader: R) -> Option<FileHash> {
    let mut data = vec![];
    reader.read_to_end(&mut data).ok()?;
    let obj = object::read::File::parse(&data).ok()?;

    for (_idx, sym) in obj.symbols() {
        if let Some(name) = sym.name() {
            if name.starts_with("_rust_svh") {
                if let Some(index) = name.rfind('_') {
                    return FileHash::from_hex_rev(FileHashAlgorithm::Svh, &name[index + 1..]);
                }
            }
        }
    }
    debug!("HASH svh not found in object file");
    None
}

fn get_svh_from_rmeta_file<R: Read>(mut reader: R) -> Option<FileHash> {
    let mut data = Vec::with_capacity(128);
    data.resize(128, 0);
    reader.read_exact(&mut data).ok()?;
    parse_svh(&data)
}

fn parse_svh(data: &[u8]) -> Option<FileHash> {
    debug!("HASHXX {:?}", data);
    const METADATA_VERSION_LOC: usize = 7;

    if data[METADATA_VERSION_LOC] < 6 {
        debug!("svh not available as compiler not recent enough.");
        return None;
    }
    let rust_svh_len_pos = 12;
    assert_eq!(data[rust_svh_len_pos], 64_u8);
    let data = &data[rust_svh_len_pos + 1..];
    Some(FileHash::from_slice(FileHashAlgorithm::Svh, &data[..64]))
}

/// Cache of file properties that we know to be true.
pub struct CurrentFileprint {
    pub(crate) mtime: FileTime,
    /// This will be None if not yet looked up.
    size: Option<FileSize>,
    /// This will be None if not yet calculated for this file.
    hash: Option<FileHash>,
}

impl CurrentFileprint {
    pub(crate) fn new(mtime: FileTime) -> Self {
        CurrentFileprint {
            mtime,
            size: None,
            hash: None,
        }
    }

    pub(crate) fn size(&mut self, file: &Path) -> Option<&FileSize> {
        if self.size.is_none() {
            self.size = Self::calc_size(file);
        }
        self.size.as_ref()
    }

    pub(crate) fn calc_size(file: &Path) -> Option<FileSize> {
        std::fs::metadata(file)
            .map(|metadata| NonZeroU64::new(metadata.len()))
            .ok()
            .flatten()
    }

    pub(crate) fn file_hash(&mut self, path: &Path, reference: &FileHash) -> Option<&FileHash> {
        if self.hash.is_none() {
            self.hash = Self::calc_hash(path, reference.kind);
        }
        self.hash.as_ref()
    }

    fn invoke_digest<D, R>(reader: &mut R, kind: FileHashAlgorithm) -> Option<FileHash>
    where
        D: Digest,
        R: Read,
    {
        let mut hasher = D::new();
        let mut buffer = [0; 1024];
        loop {
            let count = reader.read(&mut buffer).ok()?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        Some(FileHash::from_slice_rev(kind, &hasher.finalize()[..]))
    }

    pub(crate) fn calc_hash(path: &Path, algo: FileHashAlgorithm) -> Option<FileHash> {
        if let Ok(file) = fs::File::open(path) {
            let mut reader: io::BufReader<fs::File> = io::BufReader::new(file);

            match algo {
                FileHashAlgorithm::Md5 => Self::invoke_digest::<Md5, _>(&mut reader, algo),
                FileHashAlgorithm::Sha1 => Self::invoke_digest::<Sha1, _>(&mut reader, algo),
                FileHashAlgorithm::Sha256 => Self::invoke_digest::<Sha256, _>(&mut reader, algo),
                FileHashAlgorithm::Svh => {
                    if path.extension() == Some(std::ffi::OsStr::new("rlib")) {
                        get_svh_from_ar(reader)
                    } else if path.extension() == Some(std::ffi::OsStr::new("rmeta")) {
                        get_svh_from_rmeta_file(reader)
                    } else {
                        get_svh_from_object_file(reader)
                    }
                }
            }
        } else {
            debug!("HASH failed to open path {:?}", path);
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::{parse_svh, FileHash, FileHashAlgorithm};

    #[test]
    fn test_no_svh_below_metadata_version_6() {
        let vec: Vec<u8> = vec![
            114, 117, 115, 116, 0, 0, 0, 5, 0, 13, 201, 29, 16, 114, 117, 115, 116, 99, 32, 49, 46,
            52, 57, 46, 48, 45, 100, 101, 118, 16, 49, 100, 54, 102, 97, 101, 54, 56, 102, 54, 100,
            52, 99, 99, 98, 102, 3, 115, 116, 100, 241, 202, 128, 159, 207, 146, 173, 243, 204, 1,
            0, 2, 17, 45, 48, 55, 56, 97, 54, 56, 51, 101, 99, 57, 57, 55, 50, 48, 53, 50, 4, 99,
            111, 114, 101, 190, 159, 241, 243, 142, 194, 224, 233, 82, 0, 2, 17, 45, 51, 101, 97,
            54, 98, 97, 57, 97, 57, 56, 99, 50, 57, 51, 54, 100, 17, 99, 111, 109, 112, 105, 108,
            101, 114, 95, 98, 117, 105, 108,
        ];
        //                      r    u    s    t /   metadata version | base |               r    u   s     t   c   ' ' 1   .   4   9   .   0   -   d    e    v  |size|  svh-->
        assert!(parse_svh(&vec).is_none());
    }

    #[test] //TODO update the bits so svh is before rust version!
    fn test_svh_in_metadata_version_6() {
        let vec: Vec<u8> = vec![
            114, 117, 115, 116, 0, 0, 0, 6, 0, 17, 73, 215, 64, 29, 94, 138, 62, 252, 69, 252, 224,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 16,
            114, 117, 115, 116, 99, 32, 49, 46, 53, 48, 46, 48, 45, 100, 101, 118, 3, 115, 116,
            100, 220, 173, 135, 163, 173, 242, 162, 182, 228, 1, 0, 2, 17, 45, 48, 55, 56, 97, 54,
            56, 51, 101, 99, 57, 57, 55, 50, 48, 53, 50,
        ];
        //  r    u    s    t /   metadata version | base | size=64 |  svh       | sizee_of_version    |  r    u   s     t   c   ' ' 1   .   4   9   .   0   -   d    e    v  | base_pointer_points_here
        assert_eq!(
            parse_svh(&vec),
            FileHash::from_hex(FileHashAlgorithm::Svh, "1d5e8a3efc45fce0")
        );
    }

    #[test]
    fn file_hash() {
        let from_str = FileHash::from_hex(FileHashAlgorithm::Svh, "0102030405060708");
        let from_slice = Some(FileHash::from_slice(
            FileHashAlgorithm::Svh,
            &[1, 2, 3, 4, 5, 6, 7, 8],
        ));
        assert_eq!(from_str, from_slice);
    }
}
