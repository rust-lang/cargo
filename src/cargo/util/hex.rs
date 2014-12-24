use std::io::MemWriter;
use std::hash::{Hasher, Hash};
use std::hash::sip::SipHasher;

use rustc_serialize::hex::ToHex;

pub fn to_hex(num: u64) -> String {
    let mut writer = MemWriter::with_capacity(8);
    writer.write_le_u64(num).unwrap(); // this should never fail
    writer.get_ref().to_hex()
}

pub fn short_hash<H: Hash>(hashable: &H) -> String {
    let hasher = SipHasher::new_with_keys(0, 0);
    to_hex(hasher.hash(hashable))
}
