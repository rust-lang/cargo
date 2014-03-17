#[crate_id="cargo-read-manifest"];

extern crate cargo;
extern crate hammer;
extern crate serialize;
extern crate toml;

use hammer::{FlagDecoder,FlagConfig,FlagConfiguration};
use serialize::{Decoder,Decodable};
use serialize::json::Encoder;
use toml::from_toml;
use cargo::{Manifest,LibTarget,ExecTarget,Project};
use std::path::Path;

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
struct SerializedManifest {
    project: ~Project,
    lib: Option<~[SerializedLibTarget]>,
    bin: Option<~[SerializedExecTarget]>
}

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct SerializedTarget {
    name: ~str,
    path: Option<~str>
}

pub type SerializedLibTarget = SerializedTarget;
pub type SerializedExecTarget = SerializedTarget;


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
        root: Path::new(flags.manifest_path).dirname_str().unwrap().to_owned(),
        project: toml_manifest.project,
        lib: lib,
        bin: bin
    };

    let encoded: ~str = Encoder::str_encode(&manifest);

    println!("{}", encoded);
}

fn normalize(lib: &Option<~[SerializedLibTarget]>, bin: &Option<~[SerializedExecTarget]>) -> (~[LibTarget], ~[ExecTarget]) {
    if lib.is_some() && bin.is_some() {
        let l = lib.clone().unwrap()[0];
        let mut path = l.path.clone();

        if path.is_none() {
            path = Some(format!("src/{}.rs", l.name));
        }

        let b = bin.get_ref().map(|b_ref| {
            let b = b_ref.clone();
            let mut path = b.path.clone();
            if path.is_none() {
                path = Some(format!("src/bin/{}.rs", b.name.clone()));
            }
            ExecTarget{ path: path.unwrap(), name: b.name }
        });
        (~[LibTarget{ path: path.unwrap(), name: l.name }], b)
    } else if lib.is_some() {
        let l = lib.clone().unwrap()[0];
        let mut path = l.path.clone();

        if path.is_none() {
            path = Some(format!("src/{}.rs", l.name));
        }

        (~[LibTarget{ path: path.unwrap(), name: l.name }], ~[])
    } else if bin.is_some() {
        let b = bin.get_ref().map(|b_ref| {
            let b = b_ref.clone();
            let mut path = b.path.clone();
            if path.is_none() {
                path = Some(format!("src/bin/{}.rs", b.name.clone()));
            }
            ExecTarget{ path: path.unwrap(), name: b.name }
        });
        (~[], b)
    } else {
        (~[], ~[])
    }
}
