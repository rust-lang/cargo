use cargo_platform::Cfg;

use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::util::CargoResult;

use std::collections::HashMap;
use std::io::Write;

const VERSION: u32 = 1;

// {
// "version": 1,
// "host": {
//      "names": ["windows", "debug_assertions"],
//      "key_pairs": [
//          {"target_os": "windows"},
//          {"target_vendor": "pc"}
//      ]
// },
// "targets": [
//      "x86_64-pc-windows-msvc": {
//          "names": ["windows", "debug_assertions"],
//          "key_pairs": [
//              {"target_os": "windows"},
//              {"target_vendor": "pc"}
//          ]
//      }
// ]
// }

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
//     }
// }

#[derive(serde::Serialize)]
struct SerializedRustcCfg<'a> {
    version: u32,
    host: SerializedCfg<'a>,
    targets: HashMap<&'a str, SerializedCfg<'a>>,
}

#[derive(serde::Serialize)]
struct SerializedCfg<'a> {
    names: Vec<&'a str>,
    key_pairs: Vec<HashMap<&'a str, &'a str>>,
}

impl<'a> SerializedCfg<'a> {
    fn new(rtd: &'a RustcTargetData, kind: CompileKind) -> Self {
        Self {
            names: rtd.cfg(kind).iter().filter_map(|c| {
                match c {
                    Cfg::Name(s) => Some(s.as_str()),
                    Cfg::KeyPair(..) => None,
                }
            }).collect(),
            key_pairs: rtd.cfg(kind).iter().filter_map(|c| {
                match c {
                    Cfg::Name(..) => None,
                    Cfg::KeyPair(k, v) => {
                        let mut pair = HashMap::with_capacity(1);
                        pair.insert(k.as_str(), v.as_str());
                        Some(pair)
                    }
                }
            }).collect()
        }
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
