pub use self::registry::{
    Registry,
    MemRegistry
};

pub use self::manifest::{
    Manifest,
    Project,
    LibTarget,
    ExecTarget
};

pub use self::package::{
    Package,
    NameVer
};

pub use self::dependency::Dependency;

pub mod source;
pub mod package;
pub mod dependency;
mod manifest;
mod registry;
mod resolver;
