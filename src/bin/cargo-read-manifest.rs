#[crate_id="cargo-read-manifest"];
#[allow(deprecated_owned_vector)];

extern crate cargo;
extern crate hammer;
extern crate serialize;
extern crate toml;

use hammer::{FlagDecoder,FlagConfig,FlagConfiguration};
use serialize::{Decoder,Decodable};
use serialize::json::Encoder;
use toml::from_toml;
use cargo::{Manifest,LibTarget,ExecTarget,Project,CargoResult,CargoError,ToCargoError};
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
    match execute() {
        Err(e) => {
            println!("{}", e.message);
            // TODO: Exit with error code
        },
        _ => return
    }
}

fn execute() -> CargoResult<()> {
    let mut decoder = FlagDecoder::new::<ReadManifestFlags>(std::os::args().tail());
    let flags: ReadManifestFlags = Decodable::decode(&mut decoder);

    if decoder.error.is_some() {
        return Err(CargoError::new(decoder.error.unwrap(), 1));
    }

    let manifest_path = flags.manifest_path;
    let root = try!(toml::parse_from_file(manifest_path.clone()).to_cargo_error(format!("Couldn't parse Toml file: {}", manifest_path), 1));

    let toml_manifest = from_toml::<SerializedManifest>(root.clone());

    let (lib, bin) = normalize(&toml_manifest.lib, &toml_manifest.bin);

    let manifest = Manifest{
        root: try!(Path::new(manifest_path.clone()).dirname_str().to_cargo_error(format!("Could not get dirname from {}", manifest_path), 1)).to_owned(),
        project: toml_manifest.project,
        lib: lib,
        bin: bin
    };

    let encoded: ~str = Encoder::str_encode(&manifest);

    println!("{}", encoded);

    Ok(())
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
                path = Some(format!("src/{}.rs", b.name.clone()));
            }
            ExecTarget{ path: path.unwrap(), name: b.name }
        });
        (~[], b)
    } else {
        (~[], ~[])
    }
}
