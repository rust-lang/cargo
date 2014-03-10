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

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
struct SerializedManifest {
  project: ~Project,
  lib: Option<~[LibTarget]>,
  bin: Option<~[ExecTarget]>
}

#[deriving(Encodable,Eq,Clone,Ord)]
struct Manifest {
  project: ~Project,
  lib: ~[LibTarget],
  bin: ~[ExecTarget]
}

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
struct Target {
  name: ~str,
  path: Option<~str>
}

type LibTarget = Target;
type ExecTarget = Target;

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
struct Project {
  name: ~str,
  version: ~str,
  authors: ~[~str]
}

#[deriving(Decodable,Eq,Clone,Ord)]
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

  let toml_manifest = from_toml::<SerializedManifest>(root.clone());

  let (lib, bin) = normalize(&toml_manifest.lib, &toml_manifest.bin);

  let manifest = Manifest{
    project: toml_manifest.project,
    lib: lib,
    bin: bin
  };

  let encoded: ~str = Encoder::str_encode(&manifest);

  println!("{}", encoded);
}

fn normalize(lib: &Option<~[LibTarget]>, bin: &Option<~[ExecTarget]>) -> (~[LibTarget], ~[ExecTarget]) {
  if lib.is_some() && bin.is_some() {
    (~[], ~[])
  } else if lib.is_some() {
    let mut l = lib.clone().unwrap()[0]; // crashes if lib = [] is provided in the Toml file
    if l.path.is_none() {
      l.path = Some(format!("{}.rs", l.name));
    }
    (~[l.clone()], ~[])
  } else if bin.is_some() {
    let b = bin.get_ref().map(|b_ref| {
      let mut b = b_ref.clone();
      if b.path.is_none() {
        b.path = Some(format!("{}.rs", b.name));
      }
      b
    });
    (~[], b)
  } else {
    (~[], ~[])
  }
}
