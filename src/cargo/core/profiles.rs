use std::collections::BTreeMap;
use std::collections::HashSet;
use std::{cmp, env, fmt, hash};

use serde::Deserialize;

use crate::core::compiler::{CompileMode, ProfileKind};
use crate::core::interning::InternedString;
use crate::core::{Feature, Features, PackageId, PackageIdSpec, PackageSet, Shell};
use crate::util::errors::CargoResultExt;
use crate::util::toml::{ProfilePackageSpec, StringOrBool, TomlProfile, TomlProfiles, U32OrBool};
use crate::util::{closest_msg, CargoResult, Config};

/// Collection of all user profiles.
#[derive(Clone, Debug)]
pub struct Profiles {
    /// Incremental compilation can be overridden globally via:
    /// - `CARGO_INCREMENTAL` environment variable.
    /// - `build.incremental` config value.
    incremental: Option<bool>,
    dir_names: BTreeMap<String, String>,
    by_name: BTreeMap<String, ProfileMaker>,
}

impl Profiles {
    pub fn new(
        profiles: Option<&TomlProfiles>,
        config: &Config,
        features: &Features,
        warnings: &mut Vec<String>,
    ) -> CargoResult<Profiles> {
        if let Some(profiles) = profiles {
            profiles.validate(features, warnings)?;
        }

        let config_profiles = config.profiles()?;

        let incremental = match env::var_os("CARGO_INCREMENTAL") {
            Some(v) => Some(v == "1"),
            None => config.get::<Option<bool>>("build.incremental")?,
        };

        let mut profile_makers = Profiles {
            incremental,
            dir_names: Self::predefined_dir_names(),
            by_name: BTreeMap::new(),
        };

        Self::add_root_profiles(&mut profile_makers, profiles, config_profiles);

        let mut profiles = if let Some(profiles) = profiles {
            profiles.get_all().clone()
        } else {
            BTreeMap::new()
        };

        // Feature gating
        for (profile_name, profile) in &profiles {
            match profile_name.as_str() {
                "dev" | "release" | "bench" | "test" | "doc" | "check" => {
                    if profile.dir_name.is_some() {
                        features.require(Feature::named_profiles())?;
                        break;
                    }

                    match &profile.dir_name {
                        None => {}
                        Some(dir_name) => {
                            validate_name(&dir_name, "dir-name")?;
                        }
                    }
                }
                _ => {
                    features.require(Feature::named_profiles())?;
                    break;
                }
            }
        }

        // Merge with predefined profiles
        use std::collections::btree_map::Entry;
        for (predef_name, mut predef_prof) in Self::predefined_profiles().into_iter() {
            match profiles.entry(predef_name.to_owned()) {
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

        profile_makers.process_customs(&profiles)?;

        Ok(profile_makers)
    }

    fn predefined_dir_names() -> BTreeMap<String, String> {
        let mut dir_names = BTreeMap::new();
        dir_names.insert("dev".to_owned(), "debug".to_owned());
        dir_names.insert("check".to_owned(), "debug".to_owned());
        dir_names.insert("test".to_owned(), "debug".to_owned());
        dir_names.insert("bench".to_owned(), "release".to_owned());
        dir_names
    }

    fn add_root_profiles(
        profile_makers: &mut Profiles,
        profiles: Option<&TomlProfiles>,
        config_profiles: &ConfigProfiles,
    ) {
        let profile_name = "dev";
        profile_makers.by_name.insert(
            profile_name.to_owned(),
            ProfileMaker {
                default: Profile::default_dev(),
                toml: profiles.and_then(|p| p.get(profile_name).cloned()),
                config: config_profiles.dev.clone(),
                inherits: vec![],
            },
        );

        let profile_name = "release";
        profile_makers.by_name.insert(
            profile_name.to_owned(),
            ProfileMaker {
                default: Profile::default_release(),
                toml: profiles.and_then(|p| p.get(profile_name).cloned()),
                config: config_profiles.release.clone(),
                inherits: vec![],
            },
        );
    }

    fn predefined_profiles() -> Vec<(&'static str, TomlProfile)> {
        vec![
            (
                "bench",
                TomlProfile {
                    inherits: Some(String::from("release")),
                    ..TomlProfile::default()
                },
            ),
            (
                "test",
                TomlProfile {
                    inherits: Some(String::from("dev")),
                    ..TomlProfile::default()
                },
            ),
            (
                "check",
                TomlProfile {
                    inherits: Some(String::from("dev")),
                    ..TomlProfile::default()
                },
            ),
            (
                "doc",
                TomlProfile {
                    inherits: Some(String::from("dev")),
                    ..TomlProfile::default()
                },
            ),
        ]
    }

    fn process_customs(&mut self, profiles: &BTreeMap<String, TomlProfile>) -> CargoResult<()> {
        for (name, profile) in profiles {
            let mut set = HashSet::new();
            let mut result = Vec::new();

            set.insert(name.as_str().to_owned());
            match &profile.dir_name {
                None => {}
                Some(dir_name) => {
                    self.dir_names.insert(name.clone(), dir_name.to_owned());
                }
            }
            match name.as_str() {
                "dev" | "release" => {
                    continue;
                }
                _ => {}
            };

            let mut maker = self.process_chain(name, &profile, &mut set, &mut result, profiles)?;
            result.reverse();
            maker.inherits = result;

            self.by_name.insert(name.as_str().to_owned(), maker);
        }

        Ok(())
    }

    fn process_chain(
        &mut self,
        name: &String,
        profile: &TomlProfile,
        set: &mut HashSet<String>,
        result: &mut Vec<TomlProfile>,
        profiles: &BTreeMap<String, TomlProfile>,
    ) -> CargoResult<ProfileMaker> {
        result.push(profile.clone());
        match profile.inherits.as_ref().map(|x| x.as_str()) {
            Some(name @ "dev") | Some(name @ "release") => {
                // These are the root profiles
                return Ok(self.by_name.get(name).unwrap().clone());
            }
            Some(name) => {
                let name = name.to_owned();
                if set.get(&name).is_some() {
                    failure::bail!("Inheritance loop of profiles cycles with profile '{}'", name);
                }

                set.insert(name.clone());
                match profiles.get(&name) {
                    None => {
                        failure::bail!("Profile '{}' not found in Cargo.toml", name);
                    }
                    Some(parent) => self.process_chain(&name, parent, set, result, profiles),
                }
            }
            None => {
                failure::bail!(
                    "An 'inherits' directive is needed for all \
                     profiles that are not 'dev' or 'release'. Here \
                     it is missing from '{}'",
                    name
                );
            }
        }
    }

    /// Retrieves the profile for a target.
    /// `is_member` is whether or not this package is a member of the
    /// workspace.
    pub fn get_profile(
        &self,
        pkg_id: PackageId,
        is_member: bool,
        unit_for: UnitFor,
        mode: CompileMode,
        profile_kind: ProfileKind,
    ) -> Profile {
        let maker = match self.by_name.get(profile_kind.name()) {
            None => panic!("Profile {} undefined", profile_kind.name()),
            Some(r) => r,
        };
        let mut profile = maker.get_profile(Some(pkg_id), is_member, unit_for);
        // `panic` should not be set for tests/benches, or any of their
        // dependencies.
        if !unit_for.is_panic_abort_ok() || mode.is_any_test() {
            profile.panic = PanicStrategy::Unwind;
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
        if !pkg_id.source_id().is_path() {
            profile.incremental = false;
        }
        profile
    }

    /// The profile for *running* a `build.rs` script is only used for setting
    /// a few environment variables. To ensure proper de-duplication of the
    /// running `Unit`, this uses a stripped-down profile (so that unrelated
    /// profile flags don't cause `build.rs` to needlessly run multiple
    /// times).
    pub fn get_profile_run_custom_build(&self, for_unit_profile: &Profile) -> Profile {
        let mut result = Profile::default();
        result.root = for_unit_profile.root;
        result.debuginfo = for_unit_profile.debuginfo;
        result.opt_level = for_unit_profile.opt_level;
        result
    }

    /// This returns the base profile. This is currently used for the
    /// `[Finished]` line. It is not entirely accurate, since it doesn't
    /// select for the package that was actually built.
    pub fn base_profile(&self, profile_kind: &ProfileKind) -> CargoResult<Profile> {
        match self.by_name.get(profile_kind.name()) {
            None => failure::bail!("Profile {} undefined", profile_kind.name()),
            Some(r) => Ok(r.get_profile(None, true, UnitFor::new_normal())),
        }
    }

    pub fn get_dir_name(&self, profile_kind: &ProfileKind) -> String {
        let dest = profile_kind.name();
        match self.dir_names.get(dest) {
            None => dest.to_owned(),
            Some(s) => s.clone(),
        }
    }

    /// Used to check for overrides for non-existing packages.
    pub fn validate_packages(
        &self,
        shell: &mut Shell,
        packages: &PackageSet<'_>,
    ) -> CargoResult<()> {
        for (_, profile) in &self.by_name {
            profile.validate_packages(shell, packages)?;
        }
        Ok(())
    }
}

/// Validate dir-names and profile names according to RFC 2678.
pub fn validate_name(name: &str, what: &str) -> CargoResult<()> {
    if let Some(ch) = name
        .chars()
        .find(|ch| !ch.is_alphanumeric() && *ch != '_' && *ch != '-')
    {
        failure::bail!("Invalid character `{}` in {}: `{}`", ch, what, name);
    }

    match name {
        "package" | "build" | "debug" => {
            failure::bail!("Invalid {}: `{}`", what, name);
        }
        _ => {}
    }

    Ok(())
}

/// An object used for handling the profile override hierarchy.
///
/// The precedence of profiles are (first one wins):
/// - Profiles in `.cargo/config` files (using same order as below).
/// - [profile.dev.overrides.name] -- a named package.
/// - [profile.dev.overrides."*"] -- this cannot apply to workspace members.
/// - [profile.dev.build-override] -- this can only apply to `build.rs` scripts
///   and their dependencies.
/// - [profile.dev]
/// - Default (hard-coded) values.
#[derive(Debug, Clone)]
struct ProfileMaker {
    /// The starting, hard-coded defaults for the profile.
    default: Profile,
    /// The profile from the `Cargo.toml` manifest.
    toml: Option<TomlProfile>,

    /// Profiles from which we inherit, in the order from which
    /// we inherit.
    inherits: Vec<TomlProfile>,

    /// Profile loaded from `.cargo/config` files.
    config: Option<TomlProfile>,
}

impl ProfileMaker {
    fn get_profile(
        &self,
        pkg_id: Option<PackageId>,
        is_member: bool,
        unit_for: UnitFor,
    ) -> Profile {
        let mut profile = self.default;

        let mut tomls = vec![];
        if let Some(ref toml) = self.toml {
            tomls.push(toml);
        }
        for toml in &self.inherits {
            tomls.push(toml);
        }

        // First merge the profiles
        for toml in &tomls {
            merge_profile(&mut profile, toml);
        }

        // Then their overrides
        for toml in &tomls {
            merge_toml_overrides(pkg_id, is_member, unit_for, &mut profile, toml);
        }

        // '.cargo/config' can still overrides everything we had so far.
        if let Some(ref toml) = self.config {
            merge_profile(&mut profile, toml);
            merge_toml_overrides(pkg_id, is_member, unit_for, &mut profile, toml);
        }

        profile
    }

    fn validate_packages(&self, shell: &mut Shell, packages: &PackageSet<'_>) -> CargoResult<()> {
        self.validate_packages_toml(shell, packages, &self.toml, true)?;
        self.validate_packages_toml(shell, packages, &self.config, false)?;
        Ok(())
    }

    fn validate_packages_toml(
        &self,
        shell: &mut Shell,
        packages: &PackageSet<'_>,
        toml: &Option<TomlProfile>,
        warn_unmatched: bool,
    ) -> CargoResult<()> {
        let toml = match *toml {
            Some(ref toml) => toml,
            None => return Ok(()),
        };
        let overrides = match toml.overrides {
            Some(ref overrides) => overrides,
            None => return Ok(()),
        };
        // Verify that a package doesn't match multiple spec overrides.
        let mut found = HashSet::new();
        for pkg_id in packages.package_ids() {
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
                    failure::bail!(
                        "multiple profile overrides in profile `{}` match package `{}`\n\
                         found profile override specs: {}",
                        self.default.name,
                        pkg_id,
                        specs
                    );
                }
            }
        }

        if !warn_unmatched {
            return Ok(());
        }
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
            let name_matches: Vec<String> = packages
                .package_ids()
                .filter_map(|pkg_id| {
                    if pkg_id.name() == spec.name() {
                        Some(pkg_id.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            if name_matches.is_empty() {
                let suggestion =
                    closest_msg(&spec.name(), packages.package_ids(), |p| p.name().as_str());
                shell.warn(format!(
                    "profile override spec `{}` did not match any packages{}",
                    spec, suggestion
                ))?;
            } else {
                shell.warn(format!(
                    "version or URL in profile override spec `{}` does not \
                     match any of the packages: {}",
                    spec,
                    name_matches.join(", ")
                ))?;
            }
        }
        Ok(())
    }
}

fn merge_toml_overrides(
    pkg_id: Option<PackageId>,
    is_member: bool,
    unit_for: UnitFor,
    profile: &mut Profile,
    toml: &TomlProfile,
) {
    if unit_for.is_build() {
        if let Some(ref build_override) = toml.build_override {
            merge_profile(profile, build_override);
        }
    }
    if let Some(ref overrides) = toml.overrides {
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
                    "package `{}` matched multiple profile overrides",
                    pkg_id
                );
            }
        }
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
}

#[derive(Clone, Copy, Eq, PartialOrd, Ord, PartialEq, Debug)]
pub enum ProfileRoot {
    Release,
    Debug,
}

/// Profile settings used to determine which compiler flags to use for a
/// target.
#[derive(Clone, Copy, Eq, PartialOrd, Ord)]
pub struct Profile {
    pub name: &'static str,
    pub opt_level: InternedString,
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
}

impl Default for Profile {
    fn default() -> Profile {
        Profile {
            name: "",
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
        }
    }
}

compact_debug! {
    impl fmt::Debug for Profile {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let (default, default_name) = match self.name {
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
            name: "dev",
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
            name: "release",
            root: ProfileRoot::Release,
            opt_level: InternedString::new("3"),
            ..Profile::default()
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
        )
    }
}

/// The link-time-optimization setting.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Lto {
    /// False = no LTO
    /// True = "Fat" LTO
    Bool(bool),
    /// Named LTO settings like "thin".
    Named(InternedString),
}

/// The `panic` setting.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
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

/// Flags used in creating `Unit`s to indicate the purpose for the target, and
/// to ensure the target's dependencies have the correct settings.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct UnitFor {
    /// A target for `build.rs` or any of its dependencies, or a proc-macro or
    /// any of its dependencies. This enables `build-override` profiles for
    /// these targets.
    build: bool,
    /// This is true if it is *allowed* to set the `panic=abort` flag. Currently
    /// this is false for test/bench targets and all their dependencies, and
    /// "for_host" units such as proc macro and custom build scripts and their
    /// dependencies.
    panic_abort_ok: bool,
}

impl UnitFor {
    /// A unit for a normal target/dependency (i.e., not custom build,
    /// proc macro/plugin, or test/bench).
    pub fn new_normal() -> UnitFor {
        UnitFor {
            build: false,
            panic_abort_ok: true,
        }
    }

    /// A unit for a custom build script or its dependencies.
    pub fn new_build() -> UnitFor {
        UnitFor {
            build: true,
            panic_abort_ok: false,
        }
    }

    /// A unit for a proc macro or compiler plugin or their dependencies.
    pub fn new_compiler() -> UnitFor {
        UnitFor {
            build: false,
            panic_abort_ok: false,
        }
    }

    /// A unit for a test/bench target or their dependencies.
    pub fn new_test() -> UnitFor {
        UnitFor {
            build: false,
            panic_abort_ok: false,
        }
    }

    /// Creates a variant based on `for_host` setting.
    ///
    /// When `for_host` is true, this clears `panic_abort_ok` in a sticky fashion so
    /// that all its dependencies also have `panic_abort_ok=false`.
    pub fn with_for_host(self, for_host: bool) -> UnitFor {
        UnitFor {
            build: self.build || for_host,
            panic_abort_ok: self.panic_abort_ok && !for_host,
        }
    }

    /// Returns `true` if this unit is for a custom build script or one of its
    /// dependencies.
    pub fn is_build(self) -> bool {
        self.build
    }

    /// Returns `true` if this unit is allowed to set the `panic` compiler flag.
    pub fn is_panic_abort_ok(self) -> bool {
        self.panic_abort_ok
    }

    /// All possible values, used by `clean`.
    pub fn all_values() -> &'static [UnitFor] {
        static ALL: [UnitFor; 3] = [
            UnitFor {
                build: false,
                panic_abort_ok: true,
            },
            UnitFor {
                build: true,
                panic_abort_ok: false,
            },
            UnitFor {
                build: false,
                panic_abort_ok: false,
            },
        ];
        &ALL
    }
}

/// Profiles loaded from `.cargo/config` files.
#[derive(Clone, Debug, Deserialize, Default)]
pub struct ConfigProfiles {
    dev: Option<TomlProfile>,
    release: Option<TomlProfile>,
}

impl ConfigProfiles {
    pub fn validate(&self, features: &Features, warnings: &mut Vec<String>) -> CargoResult<()> {
        if let Some(ref profile) = self.dev {
            profile
                .validate("dev", features, warnings)
                .chain_err(|| failure::format_err!("config profile `profile.dev` is not valid"))?;
        }
        if let Some(ref profile) = self.release {
            profile
                .validate("release", features, warnings)
                .chain_err(|| {
                    failure::format_err!("config profile `profile.release` is not valid")
                })?;
        }
        Ok(())
    }
}
