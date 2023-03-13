pub use self::source::GitSource;
pub use self::utils::{fetch, GitCheckout, GitDatabase, GitRemote};
mod known_hosts;
mod oxide;
mod source;
mod utils;

pub mod fetch {
    use crate::core::features::GitoxideFeatures;
    use crate::Config;

    /// The kind remote repository to fetch.
    #[derive(Debug, Copy, Clone)]
    pub enum RemoteKind {
        /// A repository belongs to a git dependency.
        GitDependency,
        /// A repository belongs to a git dependency, and due to usage of checking out specific revisions we can't
        /// use shallow clones.
        GitDependencyForbidShallow,
        /// A repository belongs to a Cargo registry.
        Registry,
    }

    #[derive(Debug, Clone)]
    pub enum History {
        Shallow(gix::remote::fetch::Shallow),
        Unshallow,
    }

    impl From<History> for gix::remote::fetch::Shallow {
        fn from(value: History) -> Self {
            match value {
                History::Unshallow => gix::remote::fetch::Shallow::undo(),
                History::Shallow(how) => how,
            }
        }
    }

    impl RemoteKind {
        /// Obtain the kind of history we would want for a fetch from our remote knowing if the target repo is already shallow
        /// via `repo_is_shallow` along with gitoxide-specific feature configuration via `config`.
        pub(crate) fn to_history(&self, repo_is_shallow: bool, config: &Config) -> History {
            let has_feature = |cb: &dyn Fn(GitoxideFeatures) -> bool| {
                config
                    .cli_unstable()
                    .gitoxide
                    .map_or(false, |features| cb(features))
            };
            let how = if repo_is_shallow {
                if matches!(self, RemoteKind::GitDependencyForbidShallow) {
                    return History::Unshallow;
                } else {
                    gix::remote::fetch::Shallow::NoChange
                }
            } else {
                match self {
                    RemoteKind::GitDependency if has_feature(&|git| git.shallow_deps) => {
                        gix::remote::fetch::Shallow::DepthAtRemote(1.try_into().expect("non-zero"))
                    }
                    RemoteKind::Registry if has_feature(&|git| git.shallow_index) => {
                        gix::remote::fetch::Shallow::DepthAtRemote(1.try_into().expect("non-zero"))
                    }
                    _ => gix::remote::fetch::Shallow::NoChange,
                }
            };
            History::Shallow(how)
        }
    }

    pub type Error = gix::env::collate::fetch::Error<gix::refspec::parse::Error>;
}
