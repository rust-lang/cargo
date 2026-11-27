use std::cmp::Ordering;
use std::fmt;
use std::path::{Path, PathBuf};

/// Location where a config value is defined.
#[derive(Clone, Debug, Eq, Default)]
pub struct Definition {
    origin: Origin,
    scope: Scope,
}

impl Definition {
    pub const fn builtin() -> Self {
        Self {
            scope: Scope::BuiltIn,
            origin: Origin::BuiltIn,
        }
    }

    pub fn path(path: PathBuf) -> Self {
        Self {
            scope: Scope::Path,
            origin: Origin::Path(path),
        }
    }

    pub fn environment(key: String) -> Self {
        Self {
            scope: Scope::Environment,
            origin: Origin::Environment(key),
        }
    }

    pub fn cli(path: Option<PathBuf>) -> Self {
        Self {
            scope: Scope::Cli,
            origin: path.map(Origin::Path).unwrap_or(Origin::Cli),
        }
    }

    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    /// Returns the scope in which the config value is defined.
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// Root directory where this is defined.
    ///
    /// If from a file, it is the directory above `.cargo/config.toml`.
    /// CLI and env use the provided current working directory.
    pub fn root<'a>(&'a self, current_dir: &'a Path) -> &'a Path {
        self.origin
            .as_path()
            .map(|p| p.parent().unwrap().parent().unwrap())
            .unwrap_or(current_dir)
    }

    /// Parent directory where this is defined.
    ///
    /// If from a file, it is the directory `.cargo/`.
    /// CLI and env use the provided current working directory.
    pub fn parent<'a>(&'a self, current_dir: &'a Path) -> &'a Path {
        self.origin
            .as_path()
            .map(|p| p.parent().unwrap())
            .unwrap_or(current_dir)
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
        self.origin.fmt(f)
    }
}

/// Immediate location where a config value came from
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    #[default]
    BuiltIn,
    /// Defined in the specified TOML file
    Path(PathBuf),
    /// Env variable where this came from
    Environment(String),
    /// Passed in on the command line.
    Cli,
}

impl Origin {
    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Self::Path(p) => Some(p.as_path()),
            Self::Environment(_) | Self::Cli | Self::BuiltIn => None,
        }
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Origin::Path(p) => p.display().fmt(f),
            Origin::Environment(key) => write!(f, "environment variable `{}`", key),
            Origin::Cli => write!(f, "--config cli option"),
            Origin::BuiltIn => write!(f, "default"),
        }
    }
}

/// The kind of source where a config value is defined.
///
/// In priority order.
#[derive(Clone, Copy, Default, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum Scope {
    /// A built-in value.
    #[default]
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
