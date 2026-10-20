use std::cmp::Ordering;
use std::fmt;
use std::mem;
use std::path::{Path, PathBuf};

/// The kind of source where a config value is defined.
#[derive(Clone, Copy, Debug)]
pub enum DefinitionKind {
    /// A built-in value.
    BuiltIn,
    /// Loaded due to config file discovery.
    Path,
    /// A value defined in an environment variable.
    Environment,
    /// Loaded due to a request from the global cli arg `--config`
    ///
    /// Indirect configs loaded via [`super::ConfigInclude`] are also seen as from cli args,
    /// if the initial config is being loaded from cli.
    Cli,
}

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

impl PartialOrd for Definition {
    fn partial_cmp(&self, other: &Definition) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Definition {
    fn cmp(&self, other: &Definition) -> Ordering {
        if mem::discriminant(self) == mem::discriminant(other) {
            Ordering::Equal
        } else if self.is_higher_priority(other) {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }
}

impl Definition {
    /// Root directory where this is defined.
    ///
    /// If from a file, it is the directory above `.cargo/config.toml`.
    /// CLI and env use the provided current working directory.
    pub fn root<'a>(&'a self, cwd: &'a Path) -> &'a Path {
        match self {
            Definition::Path(p) | Definition::Cli(Some(p)) => p.parent().unwrap().parent().unwrap(),
            Definition::Environment(_) | Definition::Cli(None) | Definition::BuiltIn => cwd,
        }
    }

    /// Returns true if self is a higher priority to other.
    ///
    /// CLI is preferred over environment, which is preferred over files.
    pub fn is_higher_priority(&self, other: &Definition) -> bool {
        matches!(
            (self, other),
            (Definition::Cli(_), Definition::Environment(_))
                | (Definition::Cli(_), Definition::Path(_))
                | (Definition::Cli(_), Definition::BuiltIn)
                | (Definition::Environment(_), Definition::Path(_))
                | (Definition::Environment(_), Definition::BuiltIn)
                | (Definition::Path(_), Definition::BuiltIn)
        )
    }
}

impl PartialEq for Definition {
    fn eq(&self, other: &Definition) -> bool {
        // configuration values are equivalent no matter where they're defined,
        // but they need to be defined in the same location. For example if
        // they're defined in the environment that's different than being
        // defined in a file due to path interpretations.
        mem::discriminant(self) == mem::discriminant(other)
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
