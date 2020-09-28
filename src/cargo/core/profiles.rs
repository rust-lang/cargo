use crate::core::compiler::CompileMode;
use crate::core::resolver::features::FeaturesFor;
use crate::core::{Feature, Features, PackageId, PackageIdSpec, Resolve, Shell};
use crate::util::errors::CargoResultExt;
use crate::util::interning::InternedString;
use crate::util::toml::{ProfilePackageSpec, StringOrBool, TomlProfile, TomlProfiles, U32OrBool};
use crate::util::{closest_msg, config, CargoResult, Config};
use anyhow::bail;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::{cmp, env, fmt, hash};

/// Collection of all profiles.
#[derive(Clone, Debug)]
pub struct Profiles {
    /// Incremental compilation can be overridden globally via:
    /// - `CARGO_INCREMENTAL` environment variable.
    /// - `build.incremental` config value.
    incremental: Option<bool>,
    /// Map of profile name to directory name for that profile.
    dir_names: HashMap<InternedString, InternedString>,
    /// The profile makers. Key is the profile name.
    by_name: HashMap<InternedString, ProfileMaker>,
    /// Whether or not unstable "named" profiles are enabled.
    named_profiles_enabled: bool,
    /// The profile the user requested to use.
    requested_profile: InternedString,
}

impl Profiles {
    pub fn new(
        profiles: Option<&TomlProfiles>,
        config: &Config,
        requested_profile: InternedString,
        features: &Features,
    ) -> CargoResult<Profiles> {
        let incremental = match env::var_os("CARGO_INCREMENTAL") {
            Some(v) => Some(v == "1"),
            None => config.build_config()?.incremental,
        };
        let mut profiles = merge_config_profiles(profiles, config, requested_profile, features)?;

        if !features.is_enabled(Feature::named_profiles()) {
            let mut profile_makers = Profiles {
                incremental,
                named_profiles_enabled: false,
                dir_names: Self::predefined_dir_names(),
                by_name: HashMap::new(),
                requested_profile,
            };

            profile_makers.by_name.insert(
                InternedString::new("dev"),
                ProfileMaker::new(Profile::default_dev(), profiles.remove("dev")),
            );
            profile_makers
                .dir_names
                .insert(InternedString::new("dev"), InternedString::new("debug"));

            profile_makers.by_name.insert(
                InternedString::new("release"),
                ProfileMaker::new(Profile::default_release(), profiles.remove("release")),
            );
            profile_makers.dir_names.insert(
                InternedString::new("release"),
                InternedString::new("release"),
            );

            profile_makers.by_name.insert(
                InternedString::new("test"),
                ProfileMaker::new(Profile::default_test(), profiles.remove("test")),
            );
            profile_makers
                .dir_names
                .insert(InternedString::new("test"), InternedString::new("debug"));

            profile_makers.by_name.insert(
                InternedString::new("bench"),
                ProfileMaker::new(Profile::default_bench(), profiles.remove("bench")),
            );
            profile_makers
                .dir_names
                .insert(InternedString::new("bench"), InternedString::new("release"));

            profile_makers.by_name.insert(
                InternedString::new("doc"),
                ProfileMaker::new(Profile::default_doc(), profiles.remove("doc")),
            );
            profile_makers
                .dir_names
                .insert(InternedString::new("doc"), InternedString::new("debug"));

            return Ok(profile_makers);
        }

        let mut profile_makers = Profiles {
            incremental,
            named_profiles_enabled: true,
            dir_names: Self::predefined_dir_names(),
            by_name: HashMap::new(),
            requested_profile,
        };

        Self::add_root_profiles(&mut profile_makers, &profiles)?;

        // Merge with predefined profiles.
        use std::collections::btree_map::Entry;
        for (predef_name, mut predef_prof) in Self::predefined_profiles().into_iter() {
            match profiles.entry(InternedString::new(predef_name)) {
                Entry::Vacant(vac) => {
                    vac.insert(predef_prof);
                }
                Entry::Occupied(mut oc) => {
                    // Override predefined with the user-provided Toml.
                    let r = oc.get_mut();
                    predef_prof.merge(r);
                    *r = predef_prof;
                }
            }
        }

        for (name, profile) in &profiles {
            profile_makers.add_maker(*name, profile, &profiles)?;
        }
        // Verify that the requested profile is defined *somewhere*.
        // This simplifies the API (no need for CargoResult), and enforces
        // assumptions about how config profiles are loaded.
        profile_makers.get_profile_maker(requested_profile)?;
        Ok(profile_makers)
    }

    /// Returns the hard-coded directory names for built-in profiles.
    fn predefined_dir_names() -> HashMap<InternedString, InternedString> {
        let mut dir_names = HashMap::new();
        dir_names.insert(InternedString::new("dev"), InternedString::new("debug"));
        dir_names.insert(InternedString::new("check"), InternedString::new("debug"));
        dir_names.insert(InternedString::new("test"), InternedString::new("debug"));
        dir_names.insert(InternedString::new("bench"), InternedString::new("release"));
        dir_names
    }

    /// Initialize `by_name` with the two "root" profiles, `dev`, and
    /// `release` given the user's definition.
    fn add_root_profiles(
        profile_makers: &mut Profiles,
        profiles: &BTreeMap<InternedString, TomlProfile>,
    ) -> CargoResult<()> {
        profile_makers.by_name.insert(
            InternedString::new("dev"),
            ProfileMaker::new(Profile::default_dev(), profiles.get("dev").cloned()),
        );

        profile_makers.by_name.insert(
            InternedString::new("release"),
            ProfileMaker::new(Profile::default_release(), profiles.get("release").cloned()),
        );
        Ok(())
    }

    /// Returns the built-in profiles (not including dev/release, which are
    /// "root" profiles).
    fn predefined_profiles() -> Vec<(&'static str, TomlProfile)> {
        vec![
            (
                "bench",
                TomlProfile {
                    inherits: Some(InternedString::new("release")),
                    ..TomlProfile::default()
                },
            ),
            (
                "test",
                TomlProfile {
                    inherits: Some(InternedString::new("dev")),
                    ..TomlProfile::default()
                },
            ),
            (
                "check",
                TomlProfile {
                    inherits: Some(InternedString::new("dev")),
                    ..TomlProfile::default()
                },
            ),
            (
                "doc",
                TomlProfile {
                    inherits: Some(InternedString::new("dev")),
                    ..TomlProfile::default()
                },
            ),
        ]
    }

    /// Creates a `ProfileMaker`, and inserts it into `self.by_name`.
    fn add_maker(
        &mut self,
        name: InternedString,
        profile: &TomlProfile,
        profiles: &BTreeMap<InternedString, TomlProfile>,
    ) -> CargoResult<()> {
        match &profile.dir_name {
            None => {}
            Some(dir_name) => {
                self.dir_names.insert(name, dir_name.to_owned());
            }
        }

        // dev/release are "roots" and don't inherit.
        if name == "dev" || name == "release" {
            if profile.inherits.is_some() {
                bail!(
                    "`inherits` must not be specified in root profile `{}`",
                    name
                );
            }
            // Already inserted from `add_root_profiles`, no need to do anything.
            return Ok(());
        }

        // Keep track for inherits cycles.
        let mut set = HashSet::new();
        set.insert(name);
        let maker = self.process_chain(name, profile, &mut set, profiles)?;
        self.by_name.insert(name, maker);
        Ok(())
    }

    /// Build a `ProfileMaker` by recursively following the `inherits` setting.
    ///
    /// * `name`: The name of the profile being processed.
    /// * `profile`: The TOML profile being processed.
    /// * `set`: Set of profiles that have been visited, used to detect cycles.
    /// * `profiles`: Map of all TOML profiles.
    ///
    /// Returns a `ProfileMaker` to be used for the given named profile.
    fn process_chain(
        &mut self,
        name: InternedString,
        profile: &TomlProfile,
        set: &mut HashSet<InternedString>,
        profiles: &BTreeMap<InternedString, TomlProfile>,
    ) -> CargoResult<ProfileMaker> {
        let mut maker = match profile.inherits {
            Some(inherits_name) if inherits_name == "dev" || inherits_name == "release" => {
                // These are the root profiles added in `add_root_profiles`.
                self.get_profile_maker(inherits_name).unwrap().clone()
            }
            Some(inherits_name) => {
                if !set.insert(inherits_name) {
                    bail!(
                        "profile inheritance loop detected with profile `{}` inheriting `{}`",
                        name,
                        inherits_name
                    );
                }

                match profiles.get(&inherits_name) {
                    None => {
                        bail!(
                            "profile `{}` inherits from `{}`, but that profile is not defined",
                            name,
                            inherits_name
                        );
                    }
                    Some(parent) => self.process_chain(inherits_name, parent, set, profiles)?,
                }
            }
            None => {
                bail!(
                    "profile `{}` is missing an `inherits` directive \
                     (`inherits` is required for all profiles except `dev` or `release`)",
                    name
                );
            }
        };
        match &mut maker.toml {
            Some(toml) => toml.merge(profile),
            None => maker.toml = Some(profile.clone()),
        };
        Ok(maker)
    }

    /// Retrieves the profile for a target.
    /// `is_member` is whether or not this package is a member of the
    /// workspace.
    pub fn get_profile(
        &self,
        pkg_id: PackageId,
        is_member: bool,
        is_local: bool,
        unit_for: UnitFor,
        mode: CompileMode,
    ) -> Profile {
        let (profile_name, inherits) = if !self.named_profiles_enabled {
            // With the feature disabled, we degrade `--profile` back to the
            // `--release` and `--debug` predicates, and convert back from
            // ProfileKind::Custom instantiation.

            let release = matches!(self.requested_profile.as_str(), "release" | "bench");

            match mode {
                CompileMode::Test | CompileMode::Bench | CompileMode::Doctest => {
                    if release {
                        (
                            InternedString::new("bench"),
                            Some(InternedString::new("release")),
                        )
                    } else {
                        (
                            InternedString::new("test"),
                            Some(InternedString::new("dev")),
                        )
                    }
                }
                CompileMode::Build | CompileMode::Check { .. } | CompileMode::RunCustomBuild => {
                    // Note: `RunCustomBuild` doesn't normally use this code path.
                    // `build_unit_profiles` normally ensures that it selects the
                    // ancestor's profile. However, `cargo clean -p` can hit this
                    // path.
                    if release {
                        (InternedString::new("release"), None)
                    } else {
                        (InternedString::new("dev"), None)
                    }
                }
                CompileMode::Doc { .. } => (InternedString::new("doc"), None),
            }
        } else {
            (self.requested_profile, None)
        };
        let maker = self.get_profile_maker(profile_name).unwrap();
        let mut profile = maker.get_profile(Some(pkg_id), is_member, unit_for);

        // Dealing with `panic=abort` and `panic=unwind` requires some special
        // treatment. Be sure to process all the various options here.
        match unit_for.panic_setting() {
            PanicSetting::AlwaysUnwind => profile.panic = PanicStrategy::Unwind,
            PanicSetting::ReadProfile => {}
            PanicSetting::Inherit => {
                if let Some(inherits) = inherits {
                    // TODO: Fixme, broken with named profiles.
                    let maker = self.get_profile_maker(inherits).unwrap();
                    profile.panic = maker.get_profile(Some(pkg_id), is_member, unit_for).panic;
                }
            }
        }

        // Incremental can be globally overridden.
        if let Some(v) = self.incremental {
            profile.incremental = v;
        }
        // Only enable incremental compilation for sources the user can
        // modify (aka path sources). For things that change infrequently,
        // non-incremental builds yield better performance in the compiler
        // itself (aka crates.io / git dependencies)
        //
        // (see also https://github.com/rust-lang/cargo/issues/3972)
        if !is_local {
            profile.incremental = false;
        }
        profile.name = profile_name;
        profile
    }

    /// The profile for *running* a `build.rs` script is only used for setting
    /// a few environment variables. To ensure proper de-duplication of the
    /// running `Unit`, this uses a stripped-down profile (so that unrelated
    /// profile flags don't cause `build.rs` to needlessly run multiple
    /// times).
    pub fn get_profile_run_custom_build(&self, for_unit_profile: &Profile) -> Profile {
        let mut result = Profile::default();
        result.name = for_unit_profile.name;
        result.root = for_unit_profile.root;
        result.debuginfo = for_unit_profile.debuginfo;
        result.opt_level = for_unit_profile.opt_level;
        result
    }

    /// This returns the base profile. This is currently used for the
    /// `[Finished]` line. It is not entirely accurate, since it doesn't
    /// select for the package that was actually built.
    pub fn base_profile(&self) -> Profile {
        let profile_name = if !self.named_profiles_enabled {
            match self.requested_profile.as_str() {
                "release" | "bench" => self.requested_profile,
                _ => InternedString::new("dev"),
            }
        } else {
            self.requested_profile
        };

        let maker = self.get_profile_maker(profile_name).unwrap();
        maker.get_profile(None, true, UnitFor::new_normal())
    }

    /// Gets the directory name for a profile, like `debug` or `release`.
    pub fn get_dir_name(&self) -> InternedString {
        *self
            .dir_names
            .get(&self.requested_profile)
            .unwrap_or(&self.requested_profile)
    }

    /// Used to check for overrides for non-existing packages.
    pub fn validate_packages(
        &self,
        profiles: Option<&TomlProfiles>,
        shell: &mut Shell,
        resolve: &Resolve,
    ) -> CargoResult<()> {
        for (name, profile) in &self.by_name {
            let found = validate_packages_unique(resolve, name, &profile.toml)?;
            // We intentionally do not validate unmatched packages for config
            // profiles, in case they are defined in a central location. This
            // iterates over the manifest profiles only.
            if let Some(profiles) = profiles {
                if let Some(toml_profile) = profiles.get(name) {
                    validate_packages_unmatched(shell, resolve, name, toml_profile, &found)?;
                }
            }
        }
        Ok(())
    }

    /// Returns the profile maker for the given profile name.
    fn get_profile_maker(&self, name: InternedString) -> CargoResult<&ProfileMaker> {
        self.by_name
            .get(&name)
            .ok_or_else(|| anyhow::format_err!("profile `{}` is not defined", name))
    }
}

/// An object used for handling the profile hierarchy.
///
/// The precedence of profiles are (first one wins):
/// - Profiles in `.cargo/config` files (using same order as below).
/// - [profile.dev.package.name] -- a named package.
/// - [profile.dev.package."*"] -- this cannot apply to workspace members.
/// - [profile.dev.build-override] -- this can only apply to `build.rs` scripts
///   and their dependencies.
/// - [profile.dev]
/// - Default (hard-coded) values.
#[derive(Debug, Clone)]
struct ProfileMaker {
    /// The starting, hard-coded defaults for the profile.
    default: Profile,
    /// The TOML profile defined in `Cargo.toml` or config.
    toml: Option<TomlProfile>,
}

impl ProfileMaker {
    /// Creates a new `ProfileMaker`.
    ///
    /// Note that this does not process `inherits`, the caller is responsible for that.
    fn new(default: Profile, toml: Option<TomlProfile>) -> ProfileMaker {
        ProfileMaker { default, toml }
    }

    /// Generates a new `Profile`.
    fn get_profile(
        &self,
        pkg_id: Option<PackageId>,
        is_member: bool,
        unit_for: UnitFor,
    ) -> Profile {
        let mut profile = self.default;

        // First apply profile-specific settings, things like
        // `[profile.release]`
        if let Some(toml) = &self.toml {
            merge_profile(&mut profile, toml);
        }

        // Next start overriding those settings. First comes build dependencies
        // which default to opt-level 0...
        if unit_for.is_for_host() {
            // For-host units are things like procedural macros, build scripts, and
            // their dependencies. For these units most projects simply want them
            // to compile quickly and the runtime doesn't matter too much since
            // they tend to process very little data. For this reason we default
            // them to a "compile as quickly as possible" mode which for now means
            // basically turning down the optimization level and avoid limiting
            // codegen units. This ensures that we spend little time optimizing as
            // well as enabling parallelism by not constraining codegen units.
            profile.opt_level = InternedString::new("0");
            profile.codegen_units = None;
        }
        // ... and next comes any other sorts of overrides specified in
        // profiles, such as `[profile.release.build-override]` or
        // `[profile.release.package.foo]`
        if let Some(toml) = &self.toml {
            merge_toml_overrides(pkg_id, is_member, unit_for, &mut profile, toml);
        }
        profile
    }
}

/// Merge package and build overrides from the given TOML profile into the given `Profile`.
fn merge_toml_overrides(
    pkg_id: Option<PackageId>,
    is_member: bool,
    unit_for: UnitFor,
    profile: &mut Profile,
    toml: &TomlProfile,
) {
    if unit_for.is_for_host() {
        if let Some(build_override) = &toml.build_override {
            merge_profile(profile, build_override);
        }
    }
    if let Some(overrides) = toml.package.as_ref() {
        if !is_member {
            if let Some(all) = overrides.get(&ProfilePackageSpec::All) {
                merge_profile(profile, all);
            }
        }
        if let Some(pkg_id) = pkg_id {
            let mut matches = overrides
                .iter()
                .filter_map(|(key, spec_profile)| match *key {
                    ProfilePackageSpec::All => None,
                    ProfilePackageSpec::Spec(ref s) => {
                        if s.matches(pkg_id) {
                            Some(spec_profile)
                        } else {
                            None
                        }
                    }
                });
            if let Some(spec_profile) = matches.next() {
                merge_profile(profile, spec_profile);
                // `validate_packages` should ensure that there are
                // no additional matches.
                assert!(
                    matches.next().is_none(),
                    "package `{}` matched multiple package profile overrides",
                    pkg_id
                );
            }
        }
    }
}

/// Merge the given TOML profile into the given `Profile`.
///
/// Does not merge overrides (see `merge_toml_overrides`).
fn merge_profile(profile: &mut Profile, toml: &TomlProfile) {
    if let Some(ref opt_level) = toml.opt_level {
        profile.opt_level = InternedString::new(&opt_level.0);
    }
    match toml.lto {
        Some(StringOrBool::Bool(b)) => profile.lto = Lto::Bool(b),
        Some(StringOrBool::String(ref n)) if matches!(n.as_str(), "off" | "n" | "no") => {
            profile.lto = Lto::Off
        }
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
    if let Some(panic) = &toml.panic {
        profile.panic = match panic.as_str() {
            "unwind" => PanicStrategy::Unwind,
            "abort" => PanicStrategy::Abort,
            // This should be validated in TomlProfile::validate
            _ => panic!("Unexpected panic setting `{}`", panic),
        };
    }
    if let Some(overflow_checks) = toml.overflow_checks {
        profile.overflow_checks = overflow_checks;
    }
    if let Some(incremental) = toml.incremental {
        profile.incremental = incremental;
    }
    if let Some(strip) = toml.strip {
        profile.strip = strip;
    }
}

/// The root profile (dev/release).
///
/// This is currently only used for the `PROFILE` env var for build scripts
/// for backwards compatibility. We should probably deprecate `PROFILE` and
/// encourage using things like `DEBUG` and `OPT_LEVEL` instead.
#[derive(Clone, Copy, Eq, PartialOrd, Ord, PartialEq, Debug)]
pub enum ProfileRoot {
    Release,
    Debug,
}

/// Profile settings used to determine which compiler flags to use for a
/// target.
#[derive(Clone, Copy, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct Profile {
    pub name: InternedString,
    pub opt_level: InternedString,
    #[serde(skip)] // named profiles are unstable
    pub root: ProfileRoot,
    pub lto: Lto,
    // `None` means use rustc default.
    pub codegen_units: Option<u32>,
    pub debuginfo: Option<u32>,
    pub debug_assertions: bool,
    pub overflow_checks: bool,
    pub rpath: bool,
    pub incremental: bool,
    pub panic: PanicStrategy,
    pub strip: Strip,
}

impl Default for Profile {
    fn default() -> Profile {
        Profile {
            name: InternedString::new(""),
            opt_level: InternedString::new("0"),
            root: ProfileRoot::Debug,
            lto: Lto::Bool(false),
            codegen_units: None,
            debuginfo: None,
            debug_assertions: false,
            overflow_checks: false,
            rpath: false,
            incremental: false,
            panic: PanicStrategy::Unwind,
            strip: Strip::None,
        }
    }
}

compact_debug! {
    impl fmt::Debug for Profile {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let (default, default_name) = match self.name.as_str() {
                "dev" => (Profile::default_dev(), "default_dev()"),
                "release" => (Profile::default_release(), "default_release()"),
                _ => (Profile::default(), "default()"),
            };
            [debug_the_fields(
                name
                opt_level
                lto
                root
                codegen_units
                debuginfo
                debug_assertions
                overflow_checks
                rpath
                incremental
                panic
                strip
            )]
        }
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
            name: InternedString::new("dev"),
            root: ProfileRoot::Debug,
            debuginfo: Some(2),
            debug_assertions: true,
            overflow_checks: true,
            incremental: true,
            ..Profile::default()
        }
    }

    fn default_release() -> Profile {
        Profile {
            name: InternedString::new("release"),
            root: ProfileRoot::Release,
            opt_level: InternedString::new("3"),
            ..Profile::default()
        }
    }

    // NOTE: Remove the following three once `named_profiles` is default:

    fn default_test() -> Profile {
        Profile {
            name: InternedString::new("test"),
            ..Profile::default_dev()
        }
    }

    fn default_bench() -> Profile {
        Profile {
            name: InternedString::new("bench"),
            ..Profile::default_release()
        }
    }

    fn default_doc() -> Profile {
        Profile {
            name: InternedString::new("doc"),
            ..Profile::default_dev()
        }
    }

    /// Compares all fields except `name`, which doesn't affect compilation.
    /// This is necessary for `Unit` deduplication for things like "test" and
    /// "dev" which are essentially the same.
    fn comparable(
        &self,
    ) -> (
        InternedString,
        Lto,
        Option<u32>,
        Option<u32>,
        bool,
        bool,
        bool,
        bool,
        PanicStrategy,
        Strip,
    ) {
        (
            self.opt_level,
            self.lto,
            self.codegen_units,
            self.debuginfo,
            self.debug_assertions,
            self.overflow_checks,
            self.rpath,
            self.incremental,
            self.panic,
            self.strip,
        )
    }
}

/// The link-time-optimization setting.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Lto {
    /// Explicitly no LTO, disables thin-LTO.
    Off,
    /// True = "Fat" LTO
    /// False = rustc default (no args), currently "thin LTO"
    Bool(bool),
    /// Named LTO settings like "thin".
    Named(InternedString),
}

impl serde::ser::Serialize for Lto {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        match self {
            Lto::Off => "off".serialize(s),
            Lto::Bool(b) => b.to_string().serialize(s),
            Lto::Named(n) => n.serialize(s),
        }
    }
}

/// The `panic` setting.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PanicStrategy {
    Unwind,
    Abort,
}

impl fmt::Display for PanicStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            PanicStrategy::Unwind => "unwind",
            PanicStrategy::Abort => "abort",
        }
        .fmt(f)
    }
}

/// The setting for choosing which symbols to strip
#[derive(
    Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Strip {
    /// Only strip debugging symbols
    DebugInfo,
    /// Don't remove any symbols
    None,
    /// Strip all non-exported symbols from the final binary
    Symbols,
}

impl fmt::Display for Strip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Strip::DebugInfo => "debuginfo",
            Strip::None => "none",
            Strip::Symbols => "symbols",
        }
        .fmt(f)
    }
}
/// Flags used in creating `Unit`s to indicate the purpose for the target, and
/// to ensure the target's dependencies have the correct settings.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct UnitFor {
    /// A target for `build.rs` or any of its dependencies, or a proc-macro or
    /// any of its dependencies. This enables `build-override` profiles for
    /// these targets.
    ///
    /// An invariant is that if `host_features` is true, `host` must be true.
    ///
    /// Note that this is `true` for `RunCustomBuild` units, even though that
    /// unit should *not* use build-override profiles. This is a bit of a
    /// special case. When computing the `RunCustomBuild` unit, it manually
    /// uses the `get_profile_run_custom_build` method to get the correct
    /// profile information for the unit. `host` needs to be true so that all
    /// of the dependencies of that `RunCustomBuild` unit have this flag be
    /// sticky (and forced to `true` for all further dependencies) — which is
    /// the whole point of `UnitFor`.
    host: bool,
    /// A target for a build dependency or proc-macro (or any of its
    /// dependencies). This is used for computing features of build
    /// dependencies and proc-macros independently of other dependency kinds.
    ///
    /// The subtle difference between this and `host` is that the build script
    /// for a non-host package sets this to `false` because it wants the
    /// features of the non-host package (whereas `host` is true because the
    /// build script is being built for the host). `host_features` becomes
    /// `true` for build-dependencies or proc-macros, or any of their
    /// dependencies. For example, with this dependency tree:
    ///
    /// ```text
    /// foo
    /// ├── foo build.rs
    /// │   └── shared_dep (BUILD dependency)
    /// │       └── shared_dep build.rs
    /// └── shared_dep (Normal dependency)
    ///     └── shared_dep build.rs
    /// ```
    ///
    /// In this example, `foo build.rs` is HOST=true, HOST_FEATURES=false.
    /// This is so that `foo build.rs` gets the profile settings for build
    /// scripts (HOST=true) and features of foo (HOST_FEATURES=false) because
    /// build scripts need to know which features their package is being built
    /// with.
    ///
    /// But in the case of `shared_dep`, when built as a build dependency,
    /// both flags are true (it only wants the build-dependency features).
    /// When `shared_dep` is built as a normal dependency, then `shared_dep
    /// build.rs` is HOST=true, HOST_FEATURES=false for the same reasons that
    /// foo's build script is set that way.
    host_features: bool,
    /// How Cargo processes the `panic` setting or profiles. This is done to
    /// handle test/benches inheriting from dev/release, as well as forcing
    /// `for_host` units to always unwind.
    panic_setting: PanicSetting,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
enum PanicSetting {
    /// Used to force a unit to always be compiled with the `panic=unwind`
    /// strategy, notably for build scripts, proc macros, etc.
    AlwaysUnwind,

    /// Indicates that this unit will read its `profile` setting and use
    /// whatever is configured there.
    ReadProfile,

    /// This unit will ignore its `panic` setting in its profile and will
    /// instead inherit it from the `dev` or `release` profile, as appropriate.
    Inherit,
}

impl UnitFor {
    /// A unit for a normal target/dependency (i.e., not custom build,
    /// proc macro/plugin, or test/bench).
    pub fn new_normal() -> UnitFor {
        UnitFor {
            host: false,
            host_features: false,
            panic_setting: PanicSetting::ReadProfile,
        }
    }

    /// A unit for a custom build script or proc-macro or its dependencies.
    ///
    /// The `host_features` parameter is whether or not this is for a build
    /// dependency or proc-macro (something that requires being built "on the
    /// host"). Build scripts for non-host units should use `false` because
    /// they want to use the features of the package they are running for.
    pub fn new_host(host_features: bool) -> UnitFor {
        UnitFor {
            host: true,
            host_features,
            // Force build scripts to always use `panic=unwind` for now to
            // maximally share dependencies with procedural macros.
            panic_setting: PanicSetting::AlwaysUnwind,
        }
    }

    /// A unit for a compiler plugin or their dependencies.
    pub fn new_compiler() -> UnitFor {
        UnitFor {
            host: false,
            // The feature resolver doesn't know which dependencies are
            // plugins, so for now plugins don't split features. Since plugins
            // are mostly deprecated, just leave this as false.
            host_features: false,
            // Force plugins to use `panic=abort` so panics in the compiler do
            // not abort the process but instead end with a reasonable error
            // message that involves catching the panic in the compiler.
            panic_setting: PanicSetting::AlwaysUnwind,
        }
    }

    /// A unit for a test/bench target or their dependencies.
    ///
    /// Note that `config` is taken here for unstable CLI features to detect
    /// whether `panic=abort` is supported for tests. Historical versions of
    /// rustc did not support this, but newer versions do with an unstable
    /// compiler flag.
    pub fn new_test(config: &Config) -> UnitFor {
        UnitFor {
            host: false,
            host_features: false,
            // We're testing out an unstable feature (`-Zpanic-abort-tests`)
            // which inherits the panic setting from the dev/release profile
            // (basically avoid recompiles) but historical defaults required
            // that we always unwound.
            panic_setting: if config.cli_unstable().panic_abort_tests {
                PanicSetting::Inherit
            } else {
                PanicSetting::AlwaysUnwind
            },
        }
    }

    /// This is a special case for unit tests of a proc-macro.
    ///
    /// Proc-macro unit tests are forced to be run on the host.
    pub fn new_host_test(config: &Config) -> UnitFor {
        let mut unit_for = UnitFor::new_test(config);
        unit_for.host = true;
        unit_for.host_features = true;
        unit_for
    }

    /// Returns a new copy based on `for_host` setting.
    ///
    /// When `for_host` is true, this clears `panic_abort_ok` in a sticky
    /// fashion so that all its dependencies also have `panic_abort_ok=false`.
    /// This'll help ensure that once we start compiling for the host platform
    /// (build scripts, plugins, proc macros, etc) we'll share the same build
    /// graph where everything is `panic=unwind`.
    pub fn with_for_host(self, for_host: bool) -> UnitFor {
        UnitFor {
            host: self.host || for_host,
            host_features: self.host_features,
            panic_setting: if for_host {
                PanicSetting::AlwaysUnwind
            } else {
                self.panic_setting
            },
        }
    }

    /// Returns a new copy updating it whether or not it should use features
    /// for build dependencies and proc-macros.
    ///
    /// This is part of the machinery responsible for handling feature
    /// decoupling for build dependencies in the new feature resolver.
    pub fn with_host_features(mut self, host_features: bool) -> UnitFor {
        if host_features {
            assert!(self.host);
        }
        self.host_features = self.host_features || host_features;
        self
    }

    /// Returns `true` if this unit is for a build script or any of its
    /// dependencies, or a proc macro or any of its dependencies.
    pub fn is_for_host(&self) -> bool {
        self.host
    }

    pub fn is_for_host_features(&self) -> bool {
        self.host_features
    }

    /// Returns how `panic` settings should be handled for this profile
    fn panic_setting(&self) -> PanicSetting {
        self.panic_setting
    }

    /// All possible values, used by `clean`.
    pub fn all_values() -> &'static [UnitFor] {
        static ALL: &[UnitFor] = &[
            UnitFor {
                host: false,
                host_features: false,
                panic_setting: PanicSetting::ReadProfile,
            },
            UnitFor {
                host: true,
                host_features: false,
                panic_setting: PanicSetting::AlwaysUnwind,
            },
            UnitFor {
                host: false,
                host_features: false,
                panic_setting: PanicSetting::AlwaysUnwind,
            },
            UnitFor {
                host: false,
                host_features: false,
                panic_setting: PanicSetting::Inherit,
            },
            // host_features=true must always have host=true
            // `Inherit` is not used in build dependencies.
            UnitFor {
                host: true,
                host_features: true,
                panic_setting: PanicSetting::ReadProfile,
            },
            UnitFor {
                host: true,
                host_features: true,
                panic_setting: PanicSetting::AlwaysUnwind,
            },
        ];
        ALL
    }

    pub(crate) fn map_to_features_for(&self) -> FeaturesFor {
        FeaturesFor::from_for_host(self.is_for_host_features())
    }
}

/// Takes the manifest profiles, and overlays the config profiles on-top.
///
/// Returns a new copy of the profile map with all the mergers complete.
fn merge_config_profiles(
    profiles: Option<&TomlProfiles>,
    config: &Config,
    requested_profile: InternedString,
    features: &Features,
) -> CargoResult<BTreeMap<InternedString, TomlProfile>> {
    let mut profiles = match profiles {
        Some(profiles) => profiles.get_all().clone(),
        None => BTreeMap::new(),
    };
    // Set of profile names to check if defined in config only.
    let mut check_to_add = HashSet::new();
    check_to_add.insert(requested_profile);
    // Merge config onto manifest profiles.
    for (name, profile) in &mut profiles {
        if let Some(config_profile) = get_config_profile(name, config, features)? {
            profile.merge(&config_profile);
        }
        if let Some(inherits) = &profile.inherits {
            check_to_add.insert(*inherits);
        }
    }
    // Add the built-in profiles. This is important for things like `cargo
    // test` which implicitly use the "dev" profile for dependencies.
    for name in &["dev", "release", "test", "bench"] {
        check_to_add.insert(InternedString::new(name));
    }
    // Add config-only profiles.
    // Need to iterate repeatedly to get all the inherits values.
    let mut current = HashSet::new();
    while !check_to_add.is_empty() {
        std::mem::swap(&mut current, &mut check_to_add);
        for name in current.drain() {
            if !profiles.contains_key(&name) {
                if let Some(config_profile) = get_config_profile(&name, config, features)? {
                    if let Some(inherits) = &config_profile.inherits {
                        check_to_add.insert(*inherits);
                    }
                    profiles.insert(name, config_profile);
                }
            }
        }
    }
    Ok(profiles)
}

/// Helper for fetching a profile from config.
fn get_config_profile(
    name: &str,
    config: &Config,
    features: &Features,
) -> CargoResult<Option<TomlProfile>> {
    let profile: Option<config::Value<TomlProfile>> = config.get(&format!("profile.{}", name))?;
    let profile = match profile {
        Some(profile) => profile,
        None => return Ok(None),
    };
    let mut warnings = Vec::new();
    profile
        .val
        .validate(name, features, &mut warnings)
        .chain_err(|| {
            anyhow::format_err!(
                "config profile `{}` is not valid (defined in `{}`)",
                name,
                profile.definition
            )
        })?;
    for warning in warnings {
        config.shell().warn(warning)?;
    }
    Ok(Some(profile.val))
}

/// Validate that a package does not match multiple package override specs.
///
/// For example `[profile.dev.package.bar]` and `[profile.dev.package."bar:0.5.0"]`
/// would both match `bar:0.5.0` which would be ambiguous.
fn validate_packages_unique(
    resolve: &Resolve,
    name: &str,
    toml: &Option<TomlProfile>,
) -> CargoResult<HashSet<PackageIdSpec>> {
    let toml = match toml {
        Some(ref toml) => toml,
        None => return Ok(HashSet::new()),
    };
    let overrides = match toml.package.as_ref() {
        Some(overrides) => overrides,
        None => return Ok(HashSet::new()),
    };
    // Verify that a package doesn't match multiple spec overrides.
    let mut found = HashSet::new();
    for pkg_id in resolve.iter() {
        let matches: Vec<&PackageIdSpec> = overrides
            .keys()
            .filter_map(|key| match *key {
                ProfilePackageSpec::All => None,
                ProfilePackageSpec::Spec(ref spec) => {
                    if spec.matches(pkg_id) {
                        Some(spec)
                    } else {
                        None
                    }
                }
            })
            .collect();
        match matches.len() {
            0 => {}
            1 => {
                found.insert(matches[0].clone());
            }
            _ => {
                let specs = matches
                    .iter()
                    .map(|spec| spec.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                bail!(
                    "multiple package overrides in profile `{}` match package `{}`\n\
                     found package specs: {}",
                    name,
                    pkg_id,
                    specs
                );
            }
        }
    }
    Ok(found)
}

/// Check for any profile override specs that do not match any known packages.
///
/// This helps check for typos and mistakes.
fn validate_packages_unmatched(
    shell: &mut Shell,
    resolve: &Resolve,
    name: &str,
    toml: &TomlProfile,
    found: &HashSet<PackageIdSpec>,
) -> CargoResult<()> {
    let overrides = match toml.package.as_ref() {
        Some(overrides) => overrides,
        None => return Ok(()),
    };

    // Verify every override matches at least one package.
    let missing_specs = overrides.keys().filter_map(|key| {
        if let ProfilePackageSpec::Spec(ref spec) = *key {
            if !found.contains(spec) {
                return Some(spec);
            }
        }
        None
    });
    for spec in missing_specs {
        // See if there is an exact name match.
        let name_matches: Vec<String> = resolve
            .iter()
            .filter_map(|pkg_id| {
                if pkg_id.name() == spec.name() {
                    Some(pkg_id.to_string())
                } else {
                    None
                }
            })
            .collect();
        if name_matches.is_empty() {
            let suggestion = closest_msg(&spec.name(), resolve.iter(), |p| p.name().as_str());
            shell.warn(format!(
                "profile package spec `{}` in profile `{}` did not match any packages{}",
                spec, name, suggestion
            ))?;
        } else {
            shell.warn(format!(
                "profile package spec `{}` in profile `{}` \
                 has a version or URL that does not match any of the packages: {}",
                spec,
                name,
                name_matches.join(", ")
            ))?;
        }
    }
    Ok(())
}
