//! Code for representing cargo's release version number.

use std::fmt;

/// Information about the git repository where cargo was built from.
pub struct CommitInfo {
    pub short_commit_hash: String,
    pub commit_hash: String,
    pub commit_date: String,
}

/// Cargo's version.
pub struct VersionInfo {
    /// Cargo's version, such as "1.57.0", "1.58.0-beta.1", "1.59.0-nightly", etc.
    pub version: String,
    /// The release channel we were built for (stable/beta/nightly/dev).
    ///
    /// `None` if not built via rustuild.
    pub release_channel: Option<String>,
    /// Information about the Git repository we may have been built from.
    ///
    /// `None` if not built from a git repo.
    pub commit_info: Option<CommitInfo>,
}

impl fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.version)?;

        if let Some(ref ci) = self.commit_info {
            write!(f, " ({} {})", ci.short_commit_hash, ci.commit_date)?;
        };
        Ok(())
    }
}

/// Returns information about cargo's version.
pub fn version() -> VersionInfo {
    macro_rules! option_env_str {
        ($name:expr) => {
            option_env!($name).map(|s| s.to_string())
        };
    }

    // This is the version set in rustbuild, which we use to match rustc.
    let version = option_env_str!("CFG_RELEASE").unwrap_or_else(|| {
        // If cargo is not being built by rustbuild, then we just use the
        // version from cargo's own `Cargo.toml`.
        //
        // There are two versions at play here:
        //   - version of cargo-the-binary, which you see when you type `cargo --version`
        //   - version of cargo-the-library, which you download from crates.io for use
        //     in your packages.
        //
        // The library is permanently unstable, so it always has a 0 major
        // version. However, the CLI now reports a stable 1.x version
        // (starting in 1.26) which stays in sync with rustc's version.
        //
        // Coincidentally, the minor version for cargo-the-library is always
        // +1 of rustc's minor version (that is, `rustc 1.11.0` corresponds to
        // `cargo `0.12.0`). The versions always get bumped in lockstep, so
        // this should continue to hold.
        let minor = env!("CARGO_PKG_VERSION_MINOR").parse::<u8>().unwrap() - 1;
        let patch = env!("CARGO_PKG_VERSION_PATCH").parse::<u8>().unwrap();
        format!("1.{}.{}", minor, patch)
    });

    let release_channel = option_env_str!("CFG_RELEASE_CHANNEL");
    let commit_info = option_env_str!("CARGO_COMMIT_HASH").map(|commit_hash| CommitInfo {
        short_commit_hash: option_env_str!("CARGO_COMMIT_SHORT_HASH").unwrap(),
        commit_hash,
        commit_date: option_env_str!("CARGO_COMMIT_DATE").unwrap(),
    });

    VersionInfo {
        version,
        release_channel,
        commit_info,
    }
}
