use std::{cmp, fmt, hash};
use ops::CompileMode;
use util::toml::{StringOrBool, TomlProfile, U32OrBool};
use core::interning::InternedString;

/// Collection of all user profiles.
#[derive(Clone, Debug)]
pub struct Profiles {
    dev: ProfileMaker,
    release: ProfileMaker,
    test: ProfileMaker,
    bench: ProfileMaker,
    doc: ProfileMaker,
}

impl Profiles {
    pub fn new(
        dev: Option<TomlProfile>,
        release: Option<TomlProfile>,
        test: Option<TomlProfile>,
        bench: Option<TomlProfile>,
        doc: Option<TomlProfile>,
    ) -> Profiles {
        Profiles {
            dev: ProfileMaker {
                default: Profile::default_dev(),
                toml: dev,
            },
            release: ProfileMaker {
                default: Profile::default_release(),
                toml: release,
            },
            test: ProfileMaker {
                default: Profile::default_test(),
                toml: test,
            },
            bench: ProfileMaker {
                default: Profile::default_bench(),
                toml: bench,
            },
            doc: ProfileMaker {
                default: Profile::default_doc(),
                toml: doc,
            },
        }
    }

    /// Retrieve the profile for a target.
    /// `is_member` is whether or not this package is a member of the
    /// workspace.
    pub fn get_profile(
        &self,
        pkg_name: &str,
        is_member: bool,
        profile_for: ProfileFor,
        mode: CompileMode,
        release: bool,
    ) -> Profile {
        let maker = match mode {
            CompileMode::Test => {
                if release {
                    &self.bench
                } else {
                    &self.test
                }
            }
            CompileMode::Build
            | CompileMode::Check { .. }
            | CompileMode::Doctest
            | CompileMode::RunCustomBuild => {
                // Note: RunCustomBuild doesn't normally use this code path.
                // `build_unit_profiles` normally ensures that it selects the
                // ancestor's profile.  However `cargo clean -p` can hit this
                // path.
                if release {
                    &self.release
                } else {
                    &self.dev
                }
            }
            CompileMode::Bench => &self.bench,
            CompileMode::Doc { .. } => &self.doc,
        };
        let mut profile = maker.profile_for(pkg_name, is_member, profile_for);
        // `panic` should not be set for tests/benches, or any of their
        // dependencies.
        if profile_for == ProfileFor::TestDependency || mode.is_any_test() {
            profile.panic = None;
        }
        profile
    }

    /// This returns a generic base profile. This is currently used for the
    /// `[Finished]` line.  It is not entirely accurate, since it doesn't
    /// select for the package that was actually built.
    pub fn base_profile(&self, release: bool) -> Profile {
        if release {
            self.release.profile_for("", true, ProfileFor::Any)
        } else {
            self.dev.profile_for("", true, ProfileFor::Any)
        }
    }
}

/// An object used for handling the profile override hierarchy.
///
/// The precedence of profiles are (first one wins):
/// - [profile.dev.overrides.name] - A named package.
/// - [profile.dev.overrides."*"] - This cannot apply to workspace members.
/// - [profile.dev.build-override] - This can only apply to `build.rs` scripts
///   and their dependencies.
/// - [profile.dev]
/// - Default (hard-coded) values.
#[derive(Debug, Clone)]
struct ProfileMaker {
    default: Profile,
    toml: Option<TomlProfile>,
}

impl ProfileMaker {
    fn profile_for(&self, pkg_name: &str, is_member: bool, profile_for: ProfileFor) -> Profile {
        let mut profile = self.default;
        if let Some(ref toml) = self.toml {
            merge_profile(&mut profile, toml);
            if profile_for == ProfileFor::CustomBuild {
                if let Some(ref build_override) = toml.build_override {
                    merge_profile(&mut profile, build_override);
                }
            }
            if let Some(ref overrides) = toml.overrides {
                if !is_member {
                    if let Some(star) = overrides.get("*") {
                        merge_profile(&mut profile, star);
                    }
                }
                if let Some(byname) = overrides.get(pkg_name) {
                    merge_profile(&mut profile, byname);
                }
            }
        }
        profile
    }
}

fn merge_profile(profile: &mut Profile, toml: &TomlProfile) {
    if let Some(ref opt_level) = toml.opt_level {
        profile.opt_level = InternedString::new(&opt_level.0);
    }
    match toml.lto {
        Some(StringOrBool::Bool(b)) => profile.lto = Lto::Bool(b),
        Some(StringOrBool::String(ref n)) => profile.lto = Lto::Named(InternedString::new(n)),
        None => {}
    }
    if toml.codegen_units.is_some() {
        profile.codegen_units = toml.codegen_units;
    }
    match toml.debug {
        Some(U32OrBool::U32(debug)) => profile.debuginfo = Some(debug),
        Some(U32OrBool::Bool(true)) => profile.debuginfo = Some(2),
        Some(U32OrBool::Bool(false)) => profile.debuginfo = None,
        None => {}
    }
    if let Some(debug_assertions) = toml.debug_assertions {
        profile.debug_assertions = debug_assertions;
    }
    if let Some(rpath) = toml.rpath {
        profile.rpath = rpath;
    }
    if let Some(ref panic) = toml.panic {
        profile.panic = Some(InternedString::new(panic));
    }
    if let Some(overflow_checks) = toml.overflow_checks {
        profile.overflow_checks = overflow_checks;
    }
    if let Some(incremental) = toml.incremental {
        profile.incremental = incremental;
    }
}

/// Profile settings used to determine which compiler flags to use for a
/// target.
#[derive(Debug, Clone, Copy, Eq)]
pub struct Profile {
    pub name: &'static str,
    pub opt_level: InternedString,
    pub lto: Lto,
    // None = use rustc default
    pub codegen_units: Option<u32>,
    pub debuginfo: Option<u32>,
    pub debug_assertions: bool,
    pub overflow_checks: bool,
    pub rpath: bool,
    pub incremental: bool,
    pub panic: Option<InternedString>,
}

impl Default for Profile {
    fn default() -> Profile {
        Profile {
            name: "",
            opt_level: InternedString::new("0"),
            lto: Lto::Bool(false),
            codegen_units: None,
            debuginfo: None,
            debug_assertions: false,
            overflow_checks: false,
            rpath: false,
            incremental: false,
            panic: None,
        }
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Profile({})", self.name)
    }
}

impl hash::Hash for Profile {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        self.comparable().hash(state);
    }
}

impl cmp::PartialEq for Profile {
    fn eq(&self, other: &Self) -> bool {
        self.comparable() == other.comparable()
    }
}

impl Profile {
    fn default_dev() -> Profile {
        Profile {
            name: "dev",
            debuginfo: Some(2),
            debug_assertions: true,
            overflow_checks: true,
            incremental: true,
            ..Profile::default()
        }
    }

    fn default_release() -> Profile {
        Profile {
            name: "release",
            opt_level: InternedString::new("3"),
            ..Profile::default()
        }
    }

    fn default_test() -> Profile {
        Profile {
            name: "test",
            ..Profile::default_dev()
        }
    }

    fn default_bench() -> Profile {
        Profile {
            name: "bench",
            ..Profile::default_release()
        }
    }

    fn default_doc() -> Profile {
        Profile {
            name: "doc",
            ..Profile::default_dev()
        }
    }

    /// Compare all fields except `name`, which doesn't affect compilation.
    /// This is necessary for `Unit` deduplication for things like "test" and
    /// "dev" which are essentially the same.
    fn comparable(
        &self,
    ) -> (
        &InternedString,
        &Lto,
        &Option<u32>,
        &Option<u32>,
        &bool,
        &bool,
        &bool,
        &bool,
        &Option<InternedString>,
    ) {
        (
            &self.opt_level,
            &self.lto,
            &self.codegen_units,
            &self.debuginfo,
            &self.debug_assertions,
            &self.overflow_checks,
            &self.rpath,
            &self.incremental,
            &self.panic,
        )
    }
}

/// The link-time-optimization setting.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Lto {
    /// False = no LTO
    /// True = "Fat" LTO
    Bool(bool),
    /// Named LTO settings like "thin".
    Named(InternedString),
}

/// A flag used in `Unit` to indicate the purpose for the target.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ProfileFor {
    /// A general-purpose target.
    Any,
    /// A target for `build.rs` or any of its dependencies.  This enables
    /// `build-override` profiles for these targets.
    CustomBuild,
    /// A target that is a dependency of a test or benchmark.  Currently this
    /// enforces that the `panic` setting is not set.
    TestDependency,
}

impl ProfileFor {
    pub fn all_values() -> &'static [ProfileFor] {
        static ALL: [ProfileFor; 3] = [
            ProfileFor::Any,
            ProfileFor::CustomBuild,
            ProfileFor::TestDependency,
        ];
        &ALL
    }
}
