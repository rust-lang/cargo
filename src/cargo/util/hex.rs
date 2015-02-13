use std::hash::{Hasher, Hash, SipHasher};

use rustc_serialize::hex::ToHex;

pub fn to_hex(num: u64) -> String {
    let mut writer = Vec::with_capacity(8);
    writer.write_le_u64(num).unwrap(); // this should never fail
    writer.to_hex()
}

pub fn short_hash<H: Hash<SipHasher>>(hashable: &H) -> String {
    let mut hasher = SipHasher::new_with_keys(0, 0);
    hashable.hash(&mut hasher);
    to_hex(hasher.finish())
}
