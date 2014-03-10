#[crate_id="cargo-read-manifest"];

extern crate cargo;
extern crate hammer;
extern crate serialize;
extern crate toml;
extern crate semver;

use hammer::{FlagDecoder,FlagConfig,FlagConfiguration};
use serialize::{Decoder,Decodable};
use serialize::json::Encoder;
use toml::from_toml;
use semver::Version;
use cargo::{Manifest,LibTarget,ExecTarget,Project};

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
struct SerializedManifest {
  project: ~Project,
  lib: Option<~[LibTarget]>,
  bin: Option<~[ExecTarget]>
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
    let mut l = lib.clone().unwrap()[0]; // crashes if lib = [] is provided in the Toml file
    if l.path.is_none() {
      l.path = Some(format!("src/{}.rs", l.name));
    }

    let b = bin.get_ref().map(|b_ref| {
      let mut b = b_ref.clone();
      if b.path.is_none() {
        b.path = Some(format!("src/bin/{}.rs", b.name));
      }
      b
    });
    (~[l.clone()], b)
  } else if lib.is_some() {
    let mut l = lib.clone().unwrap()[0]; // crashes if lib = [] is provided in the Toml file
    if l.path.is_none() {
      l.path = Some(format!("src/{}.rs", l.name));
    }
    (~[l.clone()], ~[])
  } else if bin.is_some() {
    let b = bin.get_ref().map(|b_ref| {
      let mut b = b_ref.clone();
      if b.path.is_none() {
        b.path = Some(format!("src/{}.rs", b.name));
      }
      b
    });
    (~[], b)
  } else {
    (~[], ~[])
  }
}
