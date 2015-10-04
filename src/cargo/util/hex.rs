use std::hash::{Hasher, Hash, SipHasher};

use rustc_serialize::hex::ToHex;

pub fn to_hex(num: u64) -> String {
    [
        (num >>  0) as u8,
        (num >>  8) as u8,
        (num >> 16) as u8,
        (num >> 24) as u8,
        (num >> 32) as u8,
        (num >> 40) as u8,
        (num >> 48) as u8,
        (num >> 56) as u8,
    ].to_hex()
}

pub fn hash_u64<H: Hash>(hashable: &H) -> u64 {
    let mut hasher = SipHasher::new_with_keys(0, 0);
    hashable.hash(&mut hasher);
    hasher.finish()
}

pub fn short_hash<H: Hash>(hashable: &H) -> String {
    to_hex(hash_u64(hashable))
}
