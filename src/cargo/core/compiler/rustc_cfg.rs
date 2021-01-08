use cargo_platform::Cfg;

use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::util::CargoResult;

use std::collections::HashMap;
use std::io::Write;

const VERSION: u32 = 1;

// {
//     "version": 1,
//     "host": [
//         "windows",
//         "debug_assertions",
//         "target_arch='x86_64'",
//         "target_endian='little'",
//         "target_env='msvc'",
//         "target_family='windows'",
//         "target_feature='fxsr'",
//         "target_feature='sse'",
//         "target_feature='sse2'",
//         "target_os='windows'",
//         "target_pointer_width='64'",
//         "target_vendor='pc'"
//     ],
//     "targets": [
//         {
//             "x86_64-unknown-linux-gnu": [
//                 "unix",
//                 "debug_assertions",
//                 "target_arch='x86_64'",
//                 "target_endian='little'",
//                 "target_env='gnu'",
//                 "target_family='unix'",
//                 "target_feature='fxsr'",
//                 "target_feature='sse'",
//                 "target_feature='sse2'",
//                 "target_os='linux'",
//                 "target_pointer_width='64'",
//                 "target_vendor='unknown'"
//             ]
//         },
//         {
//             "i686-pc-windows-msvc": [
//                 "windows",
//                 "debug_assertions",
//                 "target_arch='x86'",
//                 "target_endian='little'",
//                 "target_env='msvc'",
//                 "target_family='windows'",
//                 "target_feature='fxsr'",
//                 "target_feature='sse'",
//                 "target_feature='sse2'",
//                 "target_os='windows'",
//                 "target_pointer_width='32'",
//                 "target_vendor='pc'"
//             ]
//         }
//     }
// }

#[derive(serde::Serialize)]
struct SerializedRustcCfg<'a> {
    version: u32,
    host: SerializedTargetData<'a>,
    targets: Vec<HashMap<&'a str, SerializedTargetData<'a>>>,
}

#[derive(serde::Serialize)]
struct SerializedTargetData<'a> {
    cfgs: Vec<&'a str>,
}

impl<'a> SerializedTargetData<'a> {
    fn new(rtd: &'a RustcTargetData, kind: CompileKind) -> Self {
        Self {
            cfgs: rtd
                .cfg(kind)
                .iter()
                .map(|c| match c {
                    Cfg::Name(n) => n.as_str(),
                    Cfg::KeyPair(k, v) => format!("{}='{}'", k, v).as_str()
                })
                .collect()
        }
    }
}

pub fn emit_serialized_rustc_cfg(rtd: &RustcTargetData, kinds: &[CompileKind]) -> CargoResult<()> {
    let host = SerializedTargetData::new(rtd, CompileKind::Host);
    let targets = kinds
        .iter()
        .filter_map(|k| match k {
            CompileKind::Host => None,
            CompileKind::Target(ct) => {
                let mut target = HashMap::with_capacity(1);
                target.insert(ct.short_name(), SerializedTargetData::new(rtd, *k));
                Some(target)
            },
        })
        .collect();
    let s = SerializedRustcCfg {
        version: VERSION,
        host,
        targets,
    };
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, &s)?;
    drop(writeln!(lock));
    Ok(())
}
