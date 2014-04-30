pub use self::namever::{
    NameVer
};

pub use self::registry::{
    Registry,
};

pub use self::manifest::{
    Manifest,
    Project,
    LibTarget,
    ExecTarget
};

pub use self::package::{
    Package,
    PackageSet
};

pub use self::dependency::Dependency;

pub mod namever;
pub mod source;
pub mod package;
pub mod dependency;
pub mod manifest;
mod registry;
mod resolver;
