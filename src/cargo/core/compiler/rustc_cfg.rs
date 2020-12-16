use cargo_platform::Cfg;

use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::util::CargoResult;

use std::collections::HashMap;
use std::io::Write;

const VERSION: u32 = 1;

// {
//     "version": 1,
//     "host": {
//         "names": ["windows", "debug_assertions"],
//         "arch":"x86_64",
//         "endian":"little",
//         "env":"msvc",
//         "family":"windows",
//         "features":["fxsr","sse","sse2"],
//         "os":"windows",
//         "pointer_width":"64",
//         "vendor":"pc"
//     },
//     "targets": [
//         "x86_64-unknown-linux-gnu": {
//             "names": ["debug_assertions", "unix"],
//             "arch":"x86_64",
//             "endian":"little",
//             "env":"gnu",
//             "family":"unix",
//             "features": ["fxsr","sse","sse2"],
//             "os":"linux",
//             "pointer_width":"64",
//             "vendor":"unknown"
//         }
//     ]
// }

#[derive(serde::Serialize)]
struct SerializedRustcCfg<'a> {
    version: u32,
    host: SerializedCfg<'a>,
    targets: HashMap<&'a str, SerializedCfg<'a>>,
}

#[derive(serde::Serialize)]
struct SerializedCfg<'a> {
    arch: Option<&'a str>,
    endian: Option<&'a str>,
    env: Option<&'a str>,
    family: Option<&'a str>,
    features: Vec<&'a str>,
    names: Vec<&'a str>,
    os: Option<&'a str>,
    pointer_width: Option<&'a str>,
    vendor: Option<&'a str>,
}

impl<'a> SerializedCfg<'a> {
    fn new(rtd: &'a RustcTargetData, kind: CompileKind) -> Self {
        let features = rtd.cfg(kind).iter().filter_map(|c| {
            match c {
                Cfg::Name(..) => None,
                Cfg::KeyPair(k, v) => {
                    if k == "target_feature" {
                        Some(v.as_str())
                    } else {
                        None
                    }
                }
            }
        }).collect();
        let names = rtd.cfg(kind).iter().filter_map(|c| {
            match c {
                Cfg::Name(s) => Some(s.as_str()),
                Cfg::KeyPair(..) => None,
            }
        }).collect();
        Self {
            arch: Self::find(rtd, kind, "target_arch"),
            endian: Self::find(rtd, kind, "target_endian"),
            env: Self::find(rtd, kind, "target_env"),
            family: Self::find(rtd, kind, "target_family"),
            features,
            names,
            os: Self::find(rtd, kind, "target_os"),
            pointer_width: Self::find(rtd, kind, "target_pointer_width"),
            vendor: Self::find(rtd, kind, "target_vendor"),
        }
    }

    fn find(rtd: &'a RustcTargetData, kind: CompileKind, key: &str) -> Option<&'a str> {
         rtd.cfg(kind).iter().find_map(|c| {
            match c {
                Cfg::Name(..) => None,
                Cfg::KeyPair(k, v) =>
                    if k == key {
                        Some(v.as_str())
                    } else {
                        None
                    }
            }
        })
    }
}

pub fn emit_serialized_rustc_cfg(rtd: &RustcTargetData, kinds: &[CompileKind]) -> CargoResult<()> {
    let host = SerializedCfg::new(rtd, CompileKind::Host);
    let targets = kinds.iter().filter_map(|k| {
         match k {
            CompileKind::Host => None,
            CompileKind::Target(ct) => Some((ct.short_name(), SerializedCfg::new(rtd, *k))),
         }
    }).collect();
    let s = SerializedRustcCfg {
        version: VERSION,
        host,
        targets
    };
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, &s)?;
    drop(writeln!(lock));
    Ok(())
}
