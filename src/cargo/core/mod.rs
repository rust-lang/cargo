pub use self::activation_key::ActivationKey;
pub use self::dependency::{Dependency, SerializedDependency};
pub use self::features::{CliUnstable, Edition, Feature, Features};
pub use self::manifest::{EitherManifest, VirtualManifest};
pub use self::manifest::{Manifest, Target, TargetKind};
pub use self::package::{Package, PackageSet};
pub use self::package_id::PackageId;
pub use self::package_id_spec::PackageIdSpecQuery;
pub use self::registry::Registry;
pub use self::resolver::{Resolve, ResolveVersion};
pub use self::shell::{Shell, Verbosity};
pub use self::source_id::SourceId;
pub use self::summary::{FeatureMap, FeatureValue, Summary};
pub use self::workspace::{
    find_workspace_root, resolve_relative_path, MaybePackage, Workspace, WorkspaceConfig,
    WorkspaceRootConfig,
};
pub use cargo_util_schemas::core::{GitReference, PackageIdSpec, SourceKind};

pub mod activation_key;
pub mod compiler;
pub mod dependency;
pub mod features;
pub mod gc;
pub mod global_cache_tracker;
pub mod manifest;
pub mod package;
pub mod package_id;
mod package_id_spec;
pub mod profiles;
pub mod registry;
pub mod resolver;
pub mod shell;
mod source_id;
pub mod summary;
mod workspace;
