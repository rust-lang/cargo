use std::cmp::Ordering;

/// The possible kinds of code source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceKind {
    /// A git repository.
    Git(GitReference),
    /// A local path.
    Path,
    /// A remote registry.
    Registry,
    /// A sparse registry.
    SparseRegistry,
    /// A local filesystem-based registry.
    LocalRegistry,
    /// A directory-based registry.
    Directory,
}

impl SourceKind {
    pub fn protocol(&self) -> Option<&str> {
        match self {
            SourceKind::Path => Some("path"),
            SourceKind::Git(_) => Some("git"),
            SourceKind::Registry => Some("registry"),
            // Sparse registry URL already includes the `sparse+` prefix, see `SourceId::new`
            SourceKind::SparseRegistry => None,
            SourceKind::LocalRegistry => Some("local-registry"),
            SourceKind::Directory => Some("directory"),
        }
    }
}

/// Note that this is specifically not derived on `SourceKind` although the
/// implementation here is very similar to what it might look like if it were
/// otherwise derived.
///
/// The reason for this is somewhat obtuse. First of all the hash value of
/// `SourceKind` makes its way into `~/.cargo/registry/index/github.com-XXXX`
/// which means that changes to the hash means that all Rust users need to
/// redownload the crates.io index and all their crates. If possible we strive
/// to not change this to make this redownloading behavior happen as little as
/// possible. How is this connected to `Ord` you might ask? That's a good
/// question!
///
/// Since the beginning of time `SourceKind` has had `#[derive(Hash)]`. It for
/// the longest time *also* derived the `Ord` and `PartialOrd` traits. In #8522,
/// however, the implementation of `Ord` changed. This handwritten implementation
/// forgot to sync itself with the originally derived implementation, namely
/// placing git dependencies as sorted after all other dependencies instead of
/// first as before.
///
/// This regression in #8522 (Rust 1.47) went unnoticed. When we switched back
/// to a derived implementation in #9133 (Rust 1.52 beta) we only then ironically
/// saw an issue (#9334). In #9334 it was observed that stable Rust at the time
/// (1.51) was sorting git dependencies last, whereas Rust 1.52 beta would sort
/// git dependencies first. This is because the `PartialOrd` implementation in
/// 1.51 used #8522, the buggy implementation, which put git deps last. In 1.52
/// it was (unknowingly) restored to the pre-1.47 behavior with git dependencies
/// first.
///
/// Because the breakage was only witnessed after the original breakage, this
/// trait implementation is preserving the "broken" behavior. Put a different way:
///
/// * Rust pre-1.47 sorted git deps first.
/// * Rust 1.47 to Rust 1.51 sorted git deps last, a breaking change (#8522) that
///   was never noticed.
/// * Rust 1.52 restored the pre-1.47 behavior (#9133, without knowing it did
///   so), and breakage was witnessed by actual users due to difference with
///   1.51.
/// * Rust 1.52 (the source as it lives now) was fixed to match the 1.47-1.51
///   behavior (#9383), which is now considered intentionally breaking from the
///   pre-1.47 behavior.
///
/// Note that this was all discovered when Rust 1.53 was in nightly and 1.52 was
/// in beta. #9133 was in both beta and nightly at the time of discovery. For
/// 1.52 #9383 reverted #9133, meaning 1.52 is the same as 1.51. On nightly
/// (1.53) #9397 was created to fix the regression introduced by #9133 relative
/// to the current stable (1.51).
///
/// That's all a long winded way of saying "it's weird that git deps hash first
/// and are sorted last, but it's the way it is right now". The author of this
/// comment chose to handwrite the `Ord` implementation instead of the `Hash`
/// implementation, but it's only required that at most one of them is
/// hand-written because the other can be derived. Perhaps one day in
/// the future someone can figure out how to remove this behavior.
impl Ord for SourceKind {
    fn cmp(&self, other: &SourceKind) -> Ordering {
        match (self, other) {
            (SourceKind::Path, SourceKind::Path) => Ordering::Equal,
            (SourceKind::Path, _) => Ordering::Less,
            (_, SourceKind::Path) => Ordering::Greater,

            (SourceKind::Registry, SourceKind::Registry) => Ordering::Equal,
            (SourceKind::Registry, _) => Ordering::Less,
            (_, SourceKind::Registry) => Ordering::Greater,

            (SourceKind::SparseRegistry, SourceKind::SparseRegistry) => Ordering::Equal,
            (SourceKind::SparseRegistry, _) => Ordering::Less,
            (_, SourceKind::SparseRegistry) => Ordering::Greater,

            (SourceKind::LocalRegistry, SourceKind::LocalRegistry) => Ordering::Equal,
            (SourceKind::LocalRegistry, _) => Ordering::Less,
            (_, SourceKind::LocalRegistry) => Ordering::Greater,

            (SourceKind::Directory, SourceKind::Directory) => Ordering::Equal,
            (SourceKind::Directory, _) => Ordering::Less,
            (_, SourceKind::Directory) => Ordering::Greater,

            (SourceKind::Git(a), SourceKind::Git(b)) => a.cmp(b),
        }
    }
}

/// Forwards to `Ord`
impl PartialOrd for SourceKind {
    fn partial_cmp(&self, other: &SourceKind) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Information to find a specific commit in a Git repository.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GitReference {
    /// From a tag.
    Tag(String),
    /// From a branch.
    Branch(String),
    /// From a specific revision. Can be a commit hash (either short or full),
    /// or a named reference like `refs/pull/493/head`.
    Rev(String),
    /// The default branch of the repository, the reference named `HEAD`.
    DefaultBranch,
}

impl GitReference {
    pub fn from_query(
        query_pairs: impl Iterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
    ) -> Self {
        let mut reference = GitReference::DefaultBranch;
        for (k, v) in query_pairs {
            let v = v.as_ref();
            match k.as_ref() {
                // Map older 'ref' to branch.
                "branch" | "ref" => reference = GitReference::Branch(v.to_owned()),

                "rev" => reference = GitReference::Rev(v.to_owned()),
                "tag" => reference = GitReference::Tag(v.to_owned()),
                _ => {}
            }
        }
        reference
    }

    /// Returns a `Display`able view of this git reference, or None if using
    /// the head of the default branch
    pub fn pretty_ref(&self, url_encoded: bool) -> Option<PrettyRef<'_>> {
        match self {
            GitReference::DefaultBranch => None,
            _ => Some(PrettyRef {
                inner: self,
                url_encoded,
            }),
        }
    }
}

/// A git reference that can be `Display`ed
pub struct PrettyRef<'a> {
    inner: &'a GitReference,
    url_encoded: bool,
}

impl<'a> std::fmt::Display for PrettyRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value: &str;
        match self.inner {
            GitReference::Branch(s) => {
                write!(f, "branch=")?;
                value = s;
            }
            GitReference::Tag(s) => {
                write!(f, "tag=")?;
                value = s;
            }
            GitReference::Rev(s) => {
                write!(f, "rev=")?;
                value = s;
            }
            GitReference::DefaultBranch => unreachable!(),
        }
        if self.url_encoded {
            for value in url::form_urlencoded::byte_serialize(value.as_bytes()) {
                write!(f, "{value}")?;
            }
        } else {
            write!(f, "{value}")?;
        }
        Ok(())
    }
}
