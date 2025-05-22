use std::fmt;
use std::fmt::Debug;

use super::*;
use crate::core::Shell;

/// Tells a better story of why a build is considered "dirty" that leads
/// to a recompile. Usually constructed via [`Fingerprint::compare`].
///
/// [`Fingerprint::compare`]: super::Fingerprint::compare
#[derive(Clone, Debug)]
pub enum DirtyReason {
    RustcChanged,
    FeaturesChanged {
        old: String,
        new: String,
    },
    DeclaredFeaturesChanged {
        old: String,
        new: String,
    },
    TargetConfigurationChanged,
    PathToSourceChanged,
    ProfileConfigurationChanged,
    RustflagsChanged {
        old: Vec<String>,
        new: Vec<String>,
    },
    ConfigSettingsChanged,
    CompileKindChanged,
    LocalLengthsChanged,
    PrecalculatedComponentsChanged {
        old: String,
        new: String,
    },
    ChecksumUseChanged {
        old: bool,
    },
    DepInfoOutputChanged {
        old: PathBuf,
        new: PathBuf,
    },
    RerunIfChangedOutputFileChanged {
        old: PathBuf,
        new: PathBuf,
    },
    RerunIfChangedOutputPathsChanged {
        old: Vec<PathBuf>,
        new: Vec<PathBuf>,
    },
    EnvVarsChanged {
        old: String,
        new: String,
    },
    EnvVarChanged {
        name: String,
        old_value: Option<String>,
        new_value: Option<String>,
    },
    LocalFingerprintTypeChanged {
        old: &'static str,
        new: &'static str,
    },
    NumberOfDependenciesChanged {
        old: usize,
        new: usize,
    },
    UnitDependencyNameChanged {
        old: InternedString,
        new: InternedString,
    },
    UnitDependencyInfoChanged {
        old_name: InternedString,
        old_fingerprint: u64,

        new_name: InternedString,
        new_fingerprint: u64,
    },
    FsStatusOutdated(FsStatus),
    NothingObvious,
    Forced,
    /// First time to build something.
    FreshBuild,
}

trait ShellExt {
    fn dirty_because(&mut self, unit: &Unit, s: impl fmt::Display) -> CargoResult<()>;
}

impl ShellExt for Shell {
    fn dirty_because(&mut self, unit: &Unit, s: impl fmt::Display) -> CargoResult<()> {
        self.status("Dirty", format_args!("{}: {s}", &unit.pkg))
    }
}

struct FileTimeDiff {
    old_time: FileTime,
    new_time: FileTime,
}

impl fmt::Display for FileTimeDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s_diff = self.new_time.seconds() - self.old_time.seconds();
        if s_diff >= 1 {
            write!(f, "{:#}", jiff::SignedDuration::from_secs(s_diff))
        } else {
            // format nanoseconds as it is, jiff would display ms, us and ns
            let ns_diff = self.new_time.nanoseconds() - self.old_time.nanoseconds();
            write!(f, "{ns_diff}ns")
        }
    }
}

#[derive(Copy, Clone)]
struct After {
    old_time: FileTime,
    new_time: FileTime,
    what: &'static str,
}

impl fmt::Display for After {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            old_time,
            new_time,
            what,
        } = *self;
        let diff = FileTimeDiff { old_time, new_time };

        write!(f, "{new_time}, {diff} after {what} at {old_time}")
    }
}

impl DirtyReason {
    /// Whether a build is dirty because it is a fresh build being kicked off.
    pub fn is_fresh_build(&self) -> bool {
        matches!(self, DirtyReason::FreshBuild)
    }

    fn after(old_time: FileTime, new_time: FileTime, what: &'static str) -> After {
        After {
            old_time,
            new_time,
            what,
        }
    }

    pub fn present_to(&self, s: &mut Shell, unit: &Unit, root: &Path) -> CargoResult<()> {
        match self {
            DirtyReason::RustcChanged => s.dirty_because(unit, "the toolchain changed"),
            DirtyReason::FeaturesChanged { .. } => {
                s.dirty_because(unit, "the list of features changed")
            }
            DirtyReason::DeclaredFeaturesChanged { .. } => {
                s.dirty_because(unit, "the list of declared features changed")
            }
            DirtyReason::TargetConfigurationChanged => {
                s.dirty_because(unit, "the target configuration changed")
            }
            DirtyReason::PathToSourceChanged => {
                s.dirty_because(unit, "the path to the source changed")
            }
            DirtyReason::ProfileConfigurationChanged => {
                s.dirty_because(unit, "the profile configuration changed")
            }
            DirtyReason::RustflagsChanged { .. } => s.dirty_because(unit, "the rustflags changed"),
            DirtyReason::ConfigSettingsChanged => {
                s.dirty_because(unit, "the config settings changed")
            }
            DirtyReason::CompileKindChanged => {
                s.dirty_because(unit, "the rustc compile kind changed")
            }
            DirtyReason::LocalLengthsChanged => {
                s.dirty_because(unit, "the local lengths changed")?;
                s.note(
                    "this could happen because of added/removed `cargo::rerun-if` instructions in the build script",
                )?;

                Ok(())
            }
            DirtyReason::PrecalculatedComponentsChanged { .. } => {
                s.dirty_because(unit, "the precalculated components changed")
            }
            DirtyReason::ChecksumUseChanged { old } => {
                if *old {
                    s.dirty_because(
                        unit,
                        "the prior compilation used checksum freshness and this one does not",
                    )
                } else {
                    s.dirty_because(unit, "checksum freshness requested, prior compilation did not use checksum freshness")
                }
            }
            DirtyReason::DepInfoOutputChanged { .. } => {
                s.dirty_because(unit, "the dependency info output changed")
            }
            DirtyReason::RerunIfChangedOutputFileChanged { .. } => {
                s.dirty_because(unit, "rerun-if-changed output file path changed")
            }
            DirtyReason::RerunIfChangedOutputPathsChanged { .. } => {
                s.dirty_because(unit, "the rerun-if-changed instructions changed")
            }
            DirtyReason::EnvVarsChanged { .. } => {
                s.dirty_because(unit, "the environment variables changed")
            }
            DirtyReason::EnvVarChanged { name, .. } => {
                s.dirty_because(unit, format_args!("the env variable {name} changed"))
            }
            DirtyReason::LocalFingerprintTypeChanged { .. } => {
                s.dirty_because(unit, "the local fingerprint type changed")
            }
            DirtyReason::NumberOfDependenciesChanged { old, new } => s.dirty_because(
                unit,
                format_args!("number of dependencies changed ({old} => {new})",),
            ),
            DirtyReason::UnitDependencyNameChanged { old, new } => s.dirty_because(
                unit,
                format_args!("name of dependency changed ({old} => {new})"),
            ),
            DirtyReason::UnitDependencyInfoChanged { .. } => {
                s.dirty_because(unit, "dependency info changed")
            }
            DirtyReason::FsStatusOutdated(status) => match status {
                FsStatus::Stale => s.dirty_because(unit, "stale, unknown reason"),
                FsStatus::StaleItem(item) => match item {
                    StaleItem::MissingFile(missing_file) => {
                        let file = missing_file.strip_prefix(root).unwrap_or(&missing_file);
                        s.dirty_because(
                            unit,
                            format_args!("the file `{}` is missing", file.display()),
                        )
                    }
                    StaleItem::UnableToReadFile(file) => {
                        let file = file.strip_prefix(root).unwrap_or(&file);
                        s.dirty_because(
                            unit,
                            format_args!("the file `{}` could not be read", file.display()),
                        )
                    }
                    StaleItem::FailedToReadMetadata(file) => {
                        let file = file.strip_prefix(root).unwrap_or(&file);
                        s.dirty_because(
                            unit,
                            format_args!("couldn't read metadata for file `{}`", file.display()),
                        )
                    }
                    StaleItem::ChangedFile {
                        stale,
                        stale_mtime,
                        reference_mtime,
                        ..
                    } => {
                        let file = stale.strip_prefix(root).unwrap_or(&stale);
                        let after = Self::after(*reference_mtime, *stale_mtime, "last build");
                        s.dirty_because(
                            unit,
                            format_args!("the file `{}` has changed ({after})", file.display()),
                        )
                    }
                    StaleItem::ChangedChecksum {
                        source,
                        stored_checksum,
                        new_checksum,
                    } => {
                        let file = source.strip_prefix(root).unwrap_or(&source);
                        s.dirty_because(
                            unit,
                            format_args!(
                                "the file `{}` has changed (checksum didn't match, {stored_checksum} != {new_checksum})",
                                file.display(),
                            ),
                        )
                    }
                    StaleItem::FileSizeChanged {
                        path,
                        old_size,
                        new_size,
                    } => {
                        let file = path.strip_prefix(root).unwrap_or(&path);
                        s.dirty_because(
                            unit,
                            format_args!(
                                "file size changed ({old_size} != {new_size}) for `{}`",
                                file.display()
                            ),
                        )
                    }
                    StaleItem::MissingChecksum(path) => {
                        let file = path.strip_prefix(root).unwrap_or(&path);
                        s.dirty_because(
                            unit,
                            format_args!("the checksum for file `{}` is missing", file.display()),
                        )
                    }
                    StaleItem::ChangedEnv { var, .. } => s.dirty_because(
                        unit,
                        format_args!("the environment variable {var} changed"),
                    ),
                },
                FsStatus::StaleDependency {
                    name,
                    dep_mtime,
                    max_mtime,
                    ..
                } => {
                    let after = Self::after(*max_mtime, *dep_mtime, "last build");
                    s.dirty_because(
                        unit,
                        format_args!("the dependency {name} was rebuilt ({after})"),
                    )
                }
                FsStatus::StaleDepFingerprint { name } => {
                    s.dirty_because(unit, format_args!("the dependency {name} was rebuilt"))
                }
                FsStatus::UpToDate { .. } => {
                    unreachable!()
                }
            },
            DirtyReason::NothingObvious => {
                // See comment in fingerprint compare method.
                s.dirty_because(unit, "the fingerprint comparison turned up nothing obvious")
            }
            DirtyReason::Forced => s.dirty_because(unit, "forced"),
            DirtyReason::FreshBuild => s.dirty_because(unit, "fresh build"),
        }
    }
}
