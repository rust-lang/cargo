//! Code for representing cargo's release version number.

use std::fmt;

/// Information about the git repository where cargo was built from.
pub struct CommitInfo {
    pub short_commit_hash: String,
    pub commit_hash: String,
    pub commit_date: String,
}

/// Information provided by the outer build system (rustbuild aka bootstrap).
pub struct CfgInfo {
    /// Information about the Git repository we may have been built from.
    pub commit_info: Option<CommitInfo>,
    /// The release channel we were built for (stable/beta/nightly/dev).
    pub release_channel: String,
}

/// Cargo's version.
pub struct VersionInfo {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub pre_release: Option<String>,
    /// Information that's only available when we were built with
    /// rustbuild, rather than Cargo itself.
    pub cfg_info: Option<CfgInfo>,
}

impl fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(channel) = self.cfg_info.as_ref().map(|ci| &ci.release_channel) {
            if channel != "stable" {
                write!(f, "-{}", channel)?;
                let empty = String::new();
                write!(f, "{}", self.pre_release.as_ref().unwrap_or(&empty))?;
            }
        };

        if let Some(ref cfg) = self.cfg_info {
            if let Some(ref ci) = cfg.commit_info {
                write!(f, " ({} {})", ci.short_commit_hash, ci.commit_date)?;
            }
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

    // So this is pretty horrible...
    // There are two versions at play here:
    //   - version of cargo-the-binary, which you see when you type `cargo --version`
    //   - version of cargo-the-library, which you download from crates.io for use
    //     in your packages.
    //
    // We want to make the `binary` version the same as the corresponding Rust/rustc release.
    // At the same time, we want to keep the library version at `0.x`, because Cargo as
    // a library is (and probably will always be) unstable.
    //
    // Historically, Cargo used the same version number for both the binary and the library.
    // Specifically, rustc 1.x.z was paired with cargo 0.x+1.w.
    // We continue to use this scheme for the library, but transform it to 1.x.w for the purposes
    // of `cargo --version`.
    let major = 1;
    let minor = env!("CARGO_PKG_VERSION_MINOR").parse::<u8>().unwrap() - 1;
    let patch = env!("CARGO_PKG_VERSION_PATCH").parse::<u8>().unwrap();

    match option_env!("CFG_RELEASE_CHANNEL") {
        // We have environment variables set up from configure/make.
        Some(_) => {
            let commit_info = option_env!("CFG_COMMIT_HASH").map(|s| CommitInfo {
                commit_hash: s.to_string(),
                short_commit_hash: option_env_str!("CFG_SHORT_COMMIT_HASH").unwrap(),
                commit_date: option_env_str!("CFG_COMMIT_DATE").unwrap(),
            });
            VersionInfo {
                major,
                minor,
                patch,
                pre_release: option_env_str!("CARGO_PKG_VERSION_PRE"),
                cfg_info: Some(CfgInfo {
                    release_channel: option_env_str!("CFG_RELEASE_CHANNEL").unwrap(),
                    commit_info,
                }),
            }
        }
        // We are being compiled by Cargo itself.
        None => VersionInfo {
            major,
            minor,
            patch,
            pre_release: option_env_str!("CARGO_PKG_VERSION_PRE"),
            cfg_info: None,
        },
    }
}
