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

pub mod errors;
pub mod namever;
pub mod source;
pub mod package;
pub mod dependency;
pub mod manifest;
pub mod resolver;
mod registry;
