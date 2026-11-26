use std::cmp::Ordering;
use std::fmt;
use std::path::{Path, PathBuf};

/// Location where a config value is defined.
#[derive(Clone, Debug, Eq, Default)]
pub enum Definition {
    #[default]
    BuiltIn,
    /// Defined in a `.cargo/config`, includes the path to the file.
    Path(PathBuf),
    /// Defined in an environment variable, includes the environment key.
    Environment(String),
    /// Passed in on the command line.
    /// A path is attached when the config value is a path to a config file.
    Cli(Option<PathBuf>),
}

impl Definition {
    /// Returns the scope in which the config value is defined.
    pub fn scope(&self) -> Scope {
        match self {
            Definition::BuiltIn => Scope::BuiltIn,
            Definition::Path(_) => Scope::Path,
            Definition::Environment(_) => Scope::Environment,
            Definition::Cli(_) => Scope::Cli,
        }
    }

    /// Root directory where this is defined.
    ///
    /// If from a file, it is the directory above `.cargo/config.toml`.
    /// CLI and env use the provided current working directory.
    pub fn root<'a>(&'a self, current_dir: &'a Path) -> &'a Path {
        match self {
            Definition::Path(p) | Definition::Cli(Some(p)) => p.parent().unwrap().parent().unwrap(),
            Definition::Environment(_) | Definition::Cli(None) | Definition::BuiltIn => current_dir,
        }
    }

    /// Parent directory where this is defined.
    ///
    /// If from a file, it is the directory `.cargo/`.
    /// CLI and env use the provided current working directory.
    pub fn parent<'a>(&'a self, current_dir: &'a Path) -> &'a Path {
        match self {
            Definition::Path(p) | Definition::Cli(Some(p)) => p.parent().unwrap(),
            Definition::Environment(_) | Definition::Cli(None) | Definition::BuiltIn => current_dir,
        }
    }

    /// Returns true if self is a higher priority to other.
    ///
    /// CLI is preferred over environment, which is preferred over files.
    pub fn is_higher_priority(&self, other: &Definition) -> bool {
        self.scope().is_higher_priority(other.scope())
    }
}

impl PartialOrd for Definition {
    fn partial_cmp(&self, other: &Definition) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Definition {
    fn cmp(&self, other: &Definition) -> Ordering {
        self.scope().cmp(&other.scope())
    }
}

impl PartialEq for Definition {
    fn eq(&self, other: &Definition) -> bool {
        // configuration values are equivalent no matter where they're defined,
        // but they need to be defined in the same location. For example if
        // they're defined in the environment that's different than being
        // defined in a file due to path interpretations.
        self.scope() == other.scope()
    }
}

impl fmt::Display for Definition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Definition::Path(p) | Definition::Cli(Some(p)) => p.display().fmt(f),
            Definition::Environment(key) => write!(f, "environment variable `{}`", key),
            Definition::Cli(None) => write!(f, "--config cli option"),
            Definition::BuiltIn => write!(f, "default"),
        }
    }
}

/// The kind of source where a config value is defined.
///
/// In priority order.
#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum Scope {
    /// A built-in value.
    BuiltIn,
    /// Loaded due to config file discovery.
    Path,
    /// A value defined in an environment variable.
    Environment,
    /// Loaded due to a request from the global cli arg `--config`
    ///
    /// Configs loaded via `include` are also seen as from cli args,
    /// if the initial config is being loaded from cli.
    Cli,
}

impl Scope {
    /// Returns true if self is a higher priority to other.
    ///
    /// CLI is preferred over environment, which is preferred over files.
    pub fn is_higher_priority(&self, other: Self) -> bool {
        *self > other
    }
}
