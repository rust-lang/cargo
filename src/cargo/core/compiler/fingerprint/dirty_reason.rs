use std::fmt;
use std::fmt::Debug;

use serde::Serialize;

use super::*;
use crate::core::Shell;

/// Tells a better story of why a build is considered "dirty" that leads
/// to a recompile. Usually constructed via [`Fingerprint::compare`].
///
/// [`Fingerprint::compare`]: super::Fingerprint::compare
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "dirty_reason", rename_all = "kebab-case")]
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
                    StaleItem::MissingFile { path } => {
                        let file = path.strip_prefix(root).unwrap_or(&path);
                        s.dirty_because(
                            unit,
                            format_args!("the file `{}` is missing", file.display()),
                        )
                    }
                    StaleItem::UnableToReadFile { path } => {
                        let file = path.strip_prefix(root).unwrap_or(&path);
                        s.dirty_because(
                            unit,
                            format_args!("the file `{}` could not be read", file.display()),
                        )
                    }
                    StaleItem::FailedToReadMetadata { path } => {
                        let file = path.strip_prefix(root).unwrap_or(&path);
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
                    StaleItem::MissingChecksum { path } => {
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

// These test the actual JSON structure that will be logged.
// In the future we might decouple this from the actual log message schema.
#[cfg(test)]
mod json_schema {
    use super::*;
    use snapbox::IntoData;
    use snapbox::assert_data_eq;
    use snapbox::str;

    fn to_json<T: Serialize>(value: &T) -> String {
        serde_json::to_string_pretty(value).unwrap()
    }

    #[test]
    fn rustc_changed() {
        let reason = DirtyReason::RustcChanged;
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "rustc-changed"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn fresh_build() {
        let reason = DirtyReason::FreshBuild;
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "fresh-build"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn forced() {
        let reason = DirtyReason::Forced;
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "forced"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn nothing_obvious() {
        let reason = DirtyReason::NothingObvious;
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "nothing-obvious"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn features_changed() {
        let reason = DirtyReason::FeaturesChanged {
            old: "f1".to_string(),
            new: "f1,f2".to_string(),
        };
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "features-changed",
  "new": "f1,f2",
  "old": "f1"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn rustflags_changed() {
        let reason = DirtyReason::RustflagsChanged {
            old: vec!["-C".into(), "opt-level=2".into()],
            new: vec!["--cfg".into(), "tokio_unstable".into()],
        };
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "rustflags-changed",
  "old": [
    "-C",
    "opt-level=2"
  ],
  "new": [
    "--cfg",
    "tokio_unstable"
  ]
}
"#]]
        );
    }

    #[test]
    fn env_var_changed_both_some() {
        let reason = DirtyReason::EnvVarChanged {
            name: "VAR".into(),
            old_value: Some("old".into()),
            new_value: Some("new".into()),
        };
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "env-var-changed",
  "name": "VAR",
  "new_value": "new",
  "old_value": "old"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn env_var_changed_old_none() {
        let reason = DirtyReason::EnvVarChanged {
            name: "VAR".into(),
            old_value: None,
            new_value: Some("new".into()),
        };
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "env-var-changed",
  "name": "VAR",
  "new_value": "new",
  "old_value": null
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn dep_info_output_changed() {
        let reason = DirtyReason::DepInfoOutputChanged {
            old: "target/debug/old.d".into(),
            new: "target/debug/new.d".into(),
        };
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "dep-info-output-changed",
  "old": "target/debug/old.d",
  "new": "target/debug/new.d"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn number_of_dependencies_changed() {
        let reason = DirtyReason::NumberOfDependenciesChanged { old: 5, new: 7 };
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "number-of-dependencies-changed",
  "old": 5,
  "new": 7
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn unit_dependency_name_changed() {
        let reason = DirtyReason::UnitDependencyNameChanged {
            old: "old_dep".into(),
            new: "new_dep".into(),
        };
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "unit-dependency-name-changed",
  "old": "old_dep",
  "new": "new_dep"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn unit_dependency_info_changed() {
        let reason = DirtyReason::UnitDependencyInfoChanged {
            old_name: "serde".into(),
            old_fingerprint: 0x1234567890abcdef,
            new_name: "serde".into(),
            new_fingerprint: 0xfedcba0987654321,
        };
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "unit-dependency-info-changed",
  "new_fingerprint": 18364757930599072545,
  "new_name": "serde",
  "old_fingerprint": 1311768467294899695,
  "old_name": "serde"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn fs_status_stale() {
        let reason = DirtyReason::FsStatusOutdated(FsStatus::Stale);
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "fs-status-outdated",
  "fs_status": "stale"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn fs_status_missing_file() {
        let reason = DirtyReason::FsStatusOutdated(FsStatus::StaleItem(StaleItem::MissingFile {
            path: "src/lib.rs".into(),
        }));
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "fs-status-outdated",
  "fs_status": "stale-item",
  "path": "src/lib.rs",
  "stale_item": "missing-file"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn fs_status_changed_file() {
        let reason = DirtyReason::FsStatusOutdated(FsStatus::StaleItem(StaleItem::ChangedFile {
            reference: "target/debug/deps/libfoo-abc123.rmeta".into(),
            reference_mtime: FileTime::from_unix_time(1730567890, 123000000),
            stale: "src/lib.rs".into(),
            stale_mtime: FileTime::from_unix_time(1730567891, 456000000),
        }));
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "fs-status-outdated",
  "fs_status": "stale-item",
  "reference": "target/debug/deps/libfoo-abc123.rmeta",
  "reference_mtime": 1730567890123.0,
  "stale": "src/lib.rs",
  "stale_item": "changed-file",
  "stale_mtime": 1730567891456.0
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn fs_status_changed_checksum() {
        use super::dep_info::ChecksumAlgo;
        let reason =
            DirtyReason::FsStatusOutdated(FsStatus::StaleItem(StaleItem::ChangedChecksum {
                source: "src/main.rs".into(),
                stored_checksum: Checksum::new(ChecksumAlgo::Sha256, [0xaa; 32]),
                new_checksum: Checksum::new(ChecksumAlgo::Sha256, [0xbb; 32]),
            }));
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "fs-status-outdated",
  "fs_status": "stale-item",
  "new_checksum": "sha256=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "source": "src/main.rs",
  "stale_item": "changed-checksum",
  "stored_checksum": "sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn fs_status_stale_dependency() {
        let reason = DirtyReason::FsStatusOutdated(FsStatus::StaleDependency {
            name: "serde".into(),
            dep_mtime: FileTime::from_unix_time(1730567892, 789000000),
            max_mtime: FileTime::from_unix_time(1730567890, 123000000),
        });
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dep_mtime": 1730567892789.0,
  "dirty_reason": "fs-status-outdated",
  "fs_status": "stale-dependency",
  "max_mtime": 1730567890123.0,
  "name": "serde"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn fs_status_stale_dep_fingerprint() {
        let reason = DirtyReason::FsStatusOutdated(FsStatus::StaleDepFingerprint {
            name: "tokio".into(),
        });
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "fs-status-outdated",
  "fs_status": "stale-dep-fingerprint",
  "name": "tokio"
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn fs_status_unable_to_read_file() {
        let reason =
            DirtyReason::FsStatusOutdated(FsStatus::StaleItem(StaleItem::UnableToReadFile {
                path: "src/lib.rs".into(),
            }));
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "fs-status-outdated",
  "fs_status": "stale-item",
  "stale_item": "unable-to-read-file",
  "path": "src/lib.rs"
}
"#]]
        );
    }

    #[test]
    fn fs_status_failed_to_read_metadata() {
        let reason =
            DirtyReason::FsStatusOutdated(FsStatus::StaleItem(StaleItem::FailedToReadMetadata {
                path: "src/lib.rs".into(),
            }));
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "fs-status-outdated",
  "fs_status": "stale-item",
  "stale_item": "failed-to-read-metadata",
  "path": "src/lib.rs"
}
"#]]
        );
    }

    #[test]
    fn fs_status_file_size_changed() {
        let reason =
            DirtyReason::FsStatusOutdated(FsStatus::StaleItem(StaleItem::FileSizeChanged {
                path: "src/lib.rs".into(),
                old_size: 1024,
                new_size: 2048,
            }));
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "fs-status-outdated",
  "fs_status": "stale-item",
  "stale_item": "file-size-changed",
  "path": "src/lib.rs",
  "old_size": 1024,
  "new_size": 2048
}
"#]]
        );
    }

    #[test]
    fn fs_status_missing_checksum() {
        let reason =
            DirtyReason::FsStatusOutdated(FsStatus::StaleItem(StaleItem::MissingChecksum {
                path: "src/lib.rs".into(),
            }));
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "fs-status-outdated",
  "fs_status": "stale-item",
  "stale_item": "missing-checksum",
  "path": "src/lib.rs"
}
"#]]
        );
    }

    #[test]
    fn fs_status_changed_env() {
        let reason = DirtyReason::FsStatusOutdated(FsStatus::StaleItem(StaleItem::ChangedEnv {
            var: "VAR".into(),
            previous: Some("old".into()),
            current: Some("new".into()),
        }));
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "fs-status-outdated",
  "fs_status": "stale-item",
  "stale_item": "changed-env",
  "var": "VAR",
  "previous": "old",
  "current": "new"
}
"#]]
        );
    }

    #[test]
    fn checksum_use_changed() {
        let reason = DirtyReason::ChecksumUseChanged { old: false };
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "checksum-use-changed",
  "old": false
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn rerun_if_changed_output_paths_changed() {
        let reason = DirtyReason::RerunIfChangedOutputPathsChanged {
            old: vec!["file1.txt".into(), "file2.txt".into()],
            new: vec!["file1.txt".into(), "file2.txt".into(), "file3.txt".into()],
        };
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "rerun-if-changed-output-paths-changed",
  "old": [
    "file1.txt",
    "file2.txt"
  ],
  "new": [
    "file1.txt",
    "file2.txt",
    "file3.txt"
  ]
}
"#]]
            .is_json()
        );
    }

    #[test]
    fn local_fingerprint_type_changed() {
        let reason = DirtyReason::LocalFingerprintTypeChanged {
            old: "precalculated",
            new: "rerun-if-changed",
        };
        assert_data_eq!(
            to_json(&reason),
            str![[r#"
{
  "dirty_reason": "local-fingerprint-type-changed",
  "new": "rerun-if-changed",
  "old": "precalculated"
}
"#]]
            .is_json()
        );
    }
}
