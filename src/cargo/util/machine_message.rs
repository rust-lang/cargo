use std::borrow::Cow;

use miniserde::json;
use miniserde::ser::{self, Fragment, Serialize};

use core::{PackageId, Target};

pub trait Message: Serialize {
    fn reason(&self) -> &str;
}

pub fn emit<T: Message>(t: &T) {
    struct Wrapper<'a> {
        reason: &'a str,
        message: &'a Message,
    }

    impl<'a> Serialize for Wrapper<'a> {
        fn begin(&self) -> Fragment {
            Fragment::Map(Box::new(StreamMessage {
                reason: Some(&self.reason),
                value: match Serialize::begin(self.message) {
                    Fragment::Map(map) => map,
                    _ => panic!("machine_message::emit expected a JSON map"),
                },
            }))
        }
    }

    struct StreamMessage<'a> {
        // double reference to enable cast to &dyn Serialize
        reason: Option<&'a &'a str>,
        value: Box<ser::Map + 'a>,
    }

    impl<'a> ser::Map for StreamMessage<'a> {
        fn next(&mut self) -> Option<(Cow<str>, &Serialize)> {
            match self.reason.take() {
                Some(reason) => Some((Cow::Borrowed("reason"), reason)),
                None => self.value.next(),
            }
        }
    }

    println!("{}", json::to_string(&Wrapper {
        reason: t.reason(),
        message: t,
    }));
}

#[derive(MiniSerialize)]
pub struct FromCompiler<'a> {
    pub package_id: &'a PackageId,
    pub target: &'a Target,
    pub message: json::Value,
}

impl<'a> Message for FromCompiler<'a> {
    fn reason(&self) -> &str {
        "compiler-message"
    }
}

#[derive(MiniSerialize)]
pub struct Artifact<'a> {
    pub package_id: &'a PackageId,
    pub target: &'a Target,
    pub profile: ArtifactProfile,
    pub features: Vec<String>,
    pub filenames: Vec<String>,
    pub fresh: bool,
}

impl<'a> Message for Artifact<'a> {
    fn reason(&self) -> &str {
        "compiler-artifact"
    }
}

/// This is different from the regular `Profile` to maintain backwards
/// compatibility (in particular, `test` is no longer in `Profile`, but we
/// still want it to be included here).
#[derive(MiniSerialize)]
pub struct ArtifactProfile {
    pub opt_level: &'static str,
    pub debuginfo: Option<u32>,
    pub debug_assertions: bool,
    pub overflow_checks: bool,
    pub test: bool,
}

#[derive(MiniSerialize)]
pub struct BuildScript<'a> {
    pub package_id: &'a PackageId,
    pub linked_libs: &'a [String],
    pub linked_paths: &'a [String],
    pub cfgs: &'a [String],
    pub env: &'a [(String, String)],
}

impl<'a> Message for BuildScript<'a> {
    fn reason(&self) -> &str {
        "build-script-executed"
    }
}
