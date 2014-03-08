#[crate_id="cargo-read-manifest"];

extern crate hammer;
extern crate serialize;
extern crate toml;
extern crate semver;

use hammer::{FlagDecoder,FlagConfig,FlagConfiguration};
use serialize::{Decoder,Decodable};
use serialize::json::Encoder;
use toml::from_toml;
use semver::Version;

#[deriving(Decodable,Encodable)]
struct Manifest {
  project: ~Project
}

#[deriving(Decodable,Encodable)]
struct Project {
  name: ~str,
  version: ~str,
  authors: ~[~str]
}

#[deriving(Decodable)]
struct ReadManifestFlags {
  manifest_path: ~str
}

impl FlagConfig for ReadManifestFlags {
  fn config(_: Option<ReadManifestFlags>, c: FlagConfiguration) -> FlagConfiguration {
    c
  }
}

fn main() {
  let mut decoder = FlagDecoder::new::<ReadManifestFlags>(std::os::args().tail());
  let flags: ReadManifestFlags = Decodable::decode(&mut decoder);

  if decoder.error.is_some() {
    fail!("Error: {}", decoder.error.unwrap());
  }

  let root = toml::parse_from_file(flags.manifest_path).unwrap();

  let manifest = from_toml::<Manifest>(root.clone());
  let encoded: ~str = Encoder::str_encode(&manifest);

  println!("{}", encoded);
}
