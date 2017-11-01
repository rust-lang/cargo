use std::collections::{HashMap, BTreeMap, HashSet, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str;

use semver::{self, VersionReq};
use serde::ser;
use serde::de::{self, Deserialize};
use serde_ignored;
use toml;
use url::Url;

use core::{SourceId, Profiles, PackageIdSpec, GitReference, WorkspaceConfig, WorkspaceRootConfig};
use core::{Summary, Manifest, Target, Dependency, PackageId};
use core::{EitherManifest, VirtualManifest, Features, Feature};
use core::dependency::{Kind, Platform};
use core::manifest::{LibKind, Profile, ManifestMetadata};
use sources::CRATES_IO;
use util::paths;
use util::{self, ToUrl, Config};
use util::errors::{CargoError, CargoResult, CargoResultExt};

mod targets;
use self::targets::targets;

pub fn read_manifest(path: &Path, source_id: &SourceId, config: &Config)
                     -> CargoResult<(EitherManifest, Vec<PathBuf>)> {
    trace!("read_manifest; path={}; source-id={}", path.display(), source_id);
    let contents = paths::read(path)?;

    do_read_manifest(&contents, path, source_id, config).chain_err(|| {
        format!("failed to parse manifest at `{}`", path.display())
    })
}

fn do_read_manifest(contents: &str,
                    manifest_file: &Path,
                    source_id: &SourceId,
                    config: &Config)
                    -> CargoResult<(EitherManifest, Vec<PathBuf>)> {
    let package_root = manifest_file.parent().unwrap();

    let toml = {
        let pretty_filename =
            util::without_prefix(manifest_file, config.cwd()).unwrap_or(manifest_file);
        parse(contents, pretty_filename, config)?
    };

    let mut unused = BTreeSet::new();
    let manifest: TomlManifest = serde_ignored::deserialize(toml, |path| {
        let mut key = String::new();
        stringify(&mut key, &path);
        unused.insert(key);
    })?;

    let manifest = Rc::new(manifest);
    return match TomlManifest::to_real_manifest(&manifest,
                                                source_id,
                                                package_root,
                                                config) {
        Ok((mut manifest, paths)) => {
            for key in unused {
                manifest.add_warning(format!("unused manifest key: {}", key));
            }
            if !manifest.targets().iter().any(|t| !t.is_custom_build()) {
                bail!("no targets specified in the manifest\n  \
                       either src/lib.rs, src/main.rs, a [lib] section, or \
                       [[bin]] section must be present")
            }
            Ok((EitherManifest::Real(manifest), paths))
        }
        Err(e) => {
            match TomlManifest::to_virtual_manifest(&manifest,
                                                    source_id,
                                                    package_root,
                                                    config) {
                Ok((m, paths)) => Ok((EitherManifest::Virtual(m), paths)),
                Err(..) => Err(e),
            }
        }
    };

    fn stringify(dst: &mut String, path: &serde_ignored::Path) {
        use serde_ignored::Path;

        match *path {
            Path::Root => {}
            Path::Seq { parent, index } => {
                stringify(dst, parent);
                if !dst.is_empty() {
                    dst.push('.');
                }
                dst.push_str(&index.to_string());
            }
            Path::Map { parent, ref key } => {
                stringify(dst, parent);
                if !dst.is_empty() {
                    dst.push('.');
                }
                dst.push_str(key);
            }
            Path::Some { parent } |
            Path::NewtypeVariant { parent } |
            Path::NewtypeStruct { parent } => stringify(dst, parent),
        }
    }
}

pub fn parse(toml: &str,
             file: &Path,
             config: &Config) -> CargoResult<toml::Value> {
    let first_error = match toml.parse() {
        Ok(ret) => return Ok(ret),
        Err(e) => e,
    };

    let mut second_parser = toml::de::Deserializer::new(toml);
    second_parser.set_require_newline_after_table(false);
    if let Ok(ret) = toml::Value::deserialize(&mut second_parser) {
        let msg = format!("\
TOML file found which contains invalid syntax and will soon not parse
at `{}`.

The TOML spec requires newlines after table definitions (e.g. `[a] b = 1` is
invalid), but this file has a table header which does not have a newline after
it. A newline needs to be added and this warning will soon become a hard error
in the future.", file.display());
        config.shell().warn(&msg)?;
        return Ok(ret)
    }

    Err(first_error).chain_err(|| {
        "could not parse input as TOML"
    })
}

type TomlLibTarget = TomlTarget;
type TomlBinTarget = TomlTarget;
type TomlExampleTarget = TomlTarget;
type TomlTestTarget = TomlTarget;
type TomlBenchTarget = TomlTarget;

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum TomlDependency {
    Simple(String),
    Detailed(DetailedTomlDependency)
}

impl<'de> de::Deserialize<'de> for TomlDependency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: de::Deserializer<'de>
    {
        struct TomlDependencyVisitor;

        impl<'de> de::Visitor<'de> for TomlDependencyVisitor {
            type Value = TomlDependency;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a version string like \"0.9.8\" or a \
                                     detailed dependency like { version = \"0.9.8\" }")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
                where E: de::Error
            {
                Ok(TomlDependency::Simple(s.to_owned()))
            }

            fn visit_map<V>(self, map: V) -> Result<Self::Value, V::Error>
                where V: de::MapAccess<'de>
            {
                let mvd = de::value::MapAccessDeserializer::new(map);
                DetailedTomlDependency::deserialize(mvd).map(TomlDependency::Detailed)
            }
        }

        deserializer.deserialize_any(TomlDependencyVisitor)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct DetailedTomlDependency {
    version: Option<String>,
    registry: Option<String>,
    path: Option<String>,
    git: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
    features: Option<Vec<String>>,
    optional: Option<bool>,
    #[serde(rename = "default-features")]
    default_features: Option<bool>,
    #[serde(rename = "default_features")]
    default_features2: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TomlManifest {
    #[serde(rename = "cargo-features")]
    cargo_features: Option<Vec<String>>,
    package: Option<Box<TomlProject>>,
    project: Option<Box<TomlProject>>,
    profile: Option<TomlProfiles>,
    lib: Option<TomlLibTarget>,
    bin: Option<Vec<TomlBinTarget>>,
    example: Option<Vec<TomlExampleTarget>>,
    test: Option<Vec<TomlTestTarget>>,
    bench: Option<Vec<TomlTestTarget>>,
    dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "dev-dependencies")]
    dev_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "dev_dependencies")]
    dev_dependencies2: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "build-dependencies")]
    build_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "build_dependencies")]
    build_dependencies2: Option<BTreeMap<String, TomlDependency>>,
    features: Option<BTreeMap<String, Vec<String>>>,
    target: Option<BTreeMap<String, TomlPlatform>>,
    replace: Option<BTreeMap<String, TomlDependency>>,
    patch: Option<BTreeMap<String, BTreeMap<String, TomlDependency>>>,
    workspace: Option<TomlWorkspace>,
    badges: Option<BTreeMap<String, BTreeMap<String, String>>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct TomlProfiles {
    test: Option<TomlProfile>,
    doc: Option<TomlProfile>,
    bench: Option<TomlProfile>,
    dev: Option<TomlProfile>,
    release: Option<TomlProfile>,
}

#[derive(Clone, Debug)]
pub struct TomlOptLevel(String);

impl<'de> de::Deserialize<'de> for TomlOptLevel {
    fn deserialize<D>(d: D) -> Result<TomlOptLevel, D::Error>
        where D: de::Deserializer<'de>
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = TomlOptLevel;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an optimization level")
            }

            fn visit_i64<E>(self, value: i64) -> Result<TomlOptLevel, E>
                where E: de::Error
            {
                Ok(TomlOptLevel(value.to_string()))
            }

            fn visit_str<E>(self, value: &str) -> Result<TomlOptLevel, E>
                where E: de::Error
            {
                if value == "s" || value == "z" {
                    Ok(TomlOptLevel(value.to_string()))
                } else {
                    Err(E::custom(format!("must be an integer, `z`, or `s`, \
                                           but found: {}", value)))
                }
            }
        }

        d.deserialize_u32(Visitor)
    }
}

impl ser::Serialize for TomlOptLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer,
    {
        match self.0.parse::<u32>() {
            Ok(n) => n.serialize(serializer),
            Err(_) => self.0.serialize(serializer),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum U32OrBool {
    U32(u32),
    Bool(bool),
}

impl<'de> de::Deserialize<'de> for U32OrBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: de::Deserializer<'de>
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = U32OrBool;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a boolean or an integer")
            }

            fn visit_i64<E>(self, u: i64) -> Result<Self::Value, E>
                where E: de::Error,
            {
                Ok(U32OrBool::U32(u as u32))
            }

            fn visit_u64<E>(self, u: u64) -> Result<Self::Value, E>
                where E: de::Error,
            {
                Ok(U32OrBool::U32(u as u32))
            }

            fn visit_bool<E>(self, b: bool) -> Result<Self::Value, E>
                where E: de::Error,
            {
                Ok(U32OrBool::Bool(b))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct TomlProfile {
    #[serde(rename = "opt-level")]
    opt_level: Option<TomlOptLevel>,
    lto: Option<bool>,
    #[serde(rename = "codegen-units")]
    codegen_units: Option<u32>,
    debug: Option<U32OrBool>,
    #[serde(rename = "debug-assertions")]
    debug_assertions: Option<bool>,
    rpath: Option<bool>,
    panic: Option<String>,
    #[serde(rename = "overflow-checks")]
    overflow_checks: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum StringOrBool {
    String(String),
    Bool(bool),
}

impl<'de> de::Deserialize<'de> for StringOrBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: de::Deserializer<'de>
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = StringOrBool;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a boolean or a string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
                where E: de::Error,
            {
                Ok(StringOrBool::String(s.to_string()))
            }

            fn visit_bool<E>(self, b: bool) -> Result<Self::Value, E>
                where E: de::Error,
            {
                Ok(StringOrBool::Bool(b))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum VecStringOrBool {
    VecString(Vec<String>),
    Bool(bool),
}

impl<'de> de::Deserialize<'de> for VecStringOrBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: de::Deserializer<'de>
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = VecStringOrBool;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a boolean or vector of strings")
            }

            fn visit_seq<V>(self, v: V) -> Result<Self::Value, V::Error>
                where V: de::SeqAccess<'de>
            {
                let seq = de::value::SeqAccessDeserializer::new(v);
                Vec::deserialize(seq).map(VecStringOrBool::VecString)
            }

            fn visit_bool<E>(self, b: bool) -> Result<Self::Value, E>
                where E: de::Error,
            {
                Ok(VecStringOrBool::Bool(b))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TomlProject {
    name: String,
    version: semver::Version,
    authors: Option<Vec<String>>,
    build: Option<StringOrBool>,
    links: Option<String>,
    exclude: Option<Vec<String>>,
    include: Option<Vec<String>>,
    publish: Option<VecStringOrBool>,
    workspace: Option<String>,
    #[serde(rename = "im-a-teapot")]
    im_a_teapot: Option<bool>,

    // package metadata
    description: Option<String>,
    homepage: Option<String>,
    documentation: Option<String>,
    readme: Option<String>,
    keywords: Option<Vec<String>>,
    categories: Option<Vec<String>>,
    license: Option<String>,
    #[serde(rename = "license-file")]
    license_file: Option<String>,
    repository: Option<String>,
    metadata: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TomlWorkspace {
    members: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
}

impl TomlProject {
    pub fn to_package_id(&self, source_id: &SourceId) -> CargoResult<PackageId> {
        PackageId::new(&self.name, self.version.clone(), source_id)
    }
}

struct Context<'a, 'b> {
    pkgid: Option<&'a PackageId>,
    deps: &'a mut Vec<Dependency>,
    source_id: &'a SourceId,
    nested_paths: &'a mut Vec<PathBuf>,
    config: &'b Config,
    warnings: &'a mut Vec<String>,
    platform: Option<Platform>,
    root: &'a Path,
    features: &'a Features,
}

impl TomlManifest {
    pub fn prepare_for_publish(&self) -> TomlManifest {
        let mut package = self.package.as_ref()
                              .or_else(|| self.project.as_ref())
                              .unwrap()
                              .clone();
        package.workspace = None;
        return TomlManifest {
            package: Some(package),
            project: None,
            profile: self.profile.clone(),
            lib: self.lib.clone(),
            bin: self.bin.clone(),
            example: self.example.clone(),
            test: self.test.clone(),
            bench: self.bench.clone(),
            dependencies: map_deps(self.dependencies.as_ref()),
            dev_dependencies: map_deps(self.dev_dependencies.as_ref()
                                         .or_else(|| self.dev_dependencies2.as_ref())),
            dev_dependencies2: None,
            build_dependencies: map_deps(self.build_dependencies.as_ref()
                                         .or_else(|| self.build_dependencies2.as_ref())),
            build_dependencies2: None,
            features: self.features.clone(),
            target: self.target.as_ref().map(|target_map| {
                target_map.iter().map(|(k, v)| {
                    (k.clone(), TomlPlatform {
                        dependencies: map_deps(v.dependencies.as_ref()),
                        dev_dependencies: map_deps(v.dev_dependencies.as_ref()
                                                     .or_else(|| v.dev_dependencies2.as_ref())),
                        dev_dependencies2: None,
                        build_dependencies: map_deps(v.build_dependencies.as_ref()
                                                     .or_else(|| v.build_dependencies2.as_ref())),
                        build_dependencies2: None,
                    })
                }).collect()
            }),
            replace: None,
            patch: None,
            workspace: None,
            badges: self.badges.clone(),
            cargo_features: self.cargo_features.clone(),
        };

        fn map_deps(deps: Option<&BTreeMap<String, TomlDependency>>)
                        -> Option<BTreeMap<String, TomlDependency>>
        {
            let deps = match deps {
                Some(deps) => deps,
                None => return None
            };
            Some(deps.iter().map(|(k, v)| (k.clone(), map_dependency(v))).collect())
        }

        fn map_dependency(dep: &TomlDependency) -> TomlDependency {
            match *dep {
                TomlDependency::Detailed(ref d) => {
                    let mut d = d.clone();
                    d.path.take(); // path dependencies become crates.io deps
                    TomlDependency::Detailed(d)
                }
                TomlDependency::Simple(ref s) => {
                    TomlDependency::Detailed(DetailedTomlDependency {
                        version: Some(s.clone()),
                        ..Default::default()
                    })
                }
            }
        }
    }

    fn to_real_manifest(me: &Rc<TomlManifest>,
                        source_id: &SourceId,
                        package_root: &Path,
                        config: &Config)
                        -> CargoResult<(Manifest, Vec<PathBuf>)> {
        let mut nested_paths = vec![];
        let mut warnings = vec![];
        let mut errors = vec![];

        // Parse features first so they will be available when parsing other parts of the toml
        let empty = Vec::new();
        let cargo_features = me.cargo_features.as_ref().unwrap_or(&empty);
        let features = Features::new(&cargo_features, &mut warnings)?;

        let project = me.project.as_ref().or_else(|| me.package.as_ref());
        let project = project.ok_or_else(|| {
            CargoError::from("no `package` section found.")
        })?;

        let package_name = project.name.trim();
        if package_name.is_empty() {
            bail!("package name cannot be an empty string.")
        }

        let pkgid = project.to_package_id(source_id)?;

        // If we have no lib at all, use the inferred lib if available
        // If we have a lib with a path, we're done
        // If we have a lib with no path, use the inferred lib or_else package name
        let targets = targets(me, package_name, package_root, &project.build,
                              &mut warnings, &mut errors)?;

        if targets.is_empty() {
            debug!("manifest has no build targets");
        }

        if let Err(e) = unique_build_targets(&targets, package_root) {
            warnings.push(format!("file found to be present in multiple \
                                   build targets: {}", e));
        }

        let mut deps = Vec::new();
        let replace;
        let patch;

        {

            let mut cx = Context {
                pkgid: Some(&pkgid),
                deps: &mut deps,
                source_id: source_id,
                nested_paths: &mut nested_paths,
                config: config,
                warnings: &mut warnings,
                features: &features,
                platform: None,
                root: package_root,
            };

            fn process_dependencies(
                cx: &mut Context,
                new_deps: Option<&BTreeMap<String, TomlDependency>>,
                kind: Option<Kind>)
                -> CargoResult<()>
            {
                let dependencies = match new_deps {
                    Some(dependencies) => dependencies,
                    None => return Ok(())
                };
                for (n, v) in dependencies.iter() {
                    let dep = v.to_dependency(n, cx, kind)?;
                    cx.deps.push(dep);
                }

                Ok(())
            }

            // Collect the deps
            process_dependencies(&mut cx, me.dependencies.as_ref(),
                                 None)?;
            let dev_deps = me.dev_dependencies.as_ref()
                               .or_else(|| me.dev_dependencies2.as_ref());
            process_dependencies(&mut cx, dev_deps, Some(Kind::Development))?;
            let build_deps = me.build_dependencies.as_ref()
                               .or_else(|| me.build_dependencies2.as_ref());
            process_dependencies(&mut cx, build_deps, Some(Kind::Build))?;

            for (name, platform) in me.target.iter().flat_map(|t| t) {
                cx.platform = Some(name.parse()?);
                process_dependencies(&mut cx, platform.dependencies.as_ref(),
                                     None)?;
                let build_deps = platform.build_dependencies.as_ref()
                                         .or_else(|| platform.build_dependencies2.as_ref());
                process_dependencies(&mut cx, build_deps, Some(Kind::Build))?;
                let dev_deps = platform.dev_dependencies.as_ref()
                                         .or_else(|| platform.dev_dependencies2.as_ref());
                process_dependencies(&mut cx, dev_deps, Some(Kind::Development))?;
            }

            replace = me.replace(&mut cx)?;
            patch = me.patch(&mut cx)?;
        }

        {
            let mut names_sources = BTreeMap::new();
            for dep in &deps {
                let name = dep.name();
                let prev = names_sources.insert(name, dep.source_id());
                if prev.is_some() && prev != Some(dep.source_id()) {
                    bail!("Dependency '{}' has different source paths depending on the build \
                           target. Each dependency must have a single canonical source path \
                           irrespective of build target.", name);
                }
            }
        }

        let exclude = project.exclude.clone().unwrap_or_default();
        let include = project.include.clone().unwrap_or_default();

        let summary = Summary::new(pkgid, deps, me.features.clone()
            .unwrap_or_else(BTreeMap::new))?;
        let metadata = ManifestMetadata {
            description: project.description.clone(),
            homepage: project.homepage.clone(),
            documentation: project.documentation.clone(),
            readme: project.readme.clone(),
            authors: project.authors.clone().unwrap_or_default(),
            license: project.license.clone(),
            license_file: project.license_file.clone(),
            repository: project.repository.clone(),
            keywords: project.keywords.clone().unwrap_or_default(),
            categories: project.categories.clone().unwrap_or_default(),
            badges: me.badges.clone().unwrap_or_default(),
        };

        let workspace_config = match (me.workspace.as_ref(),
                                      project.workspace.as_ref()) {
            (Some(config), None) => {
                WorkspaceConfig::Root(
                    WorkspaceRootConfig::new(&package_root, &config.members, &config.exclude)
                )
            }
            (None, root) => {
                WorkspaceConfig::Member { root: root.cloned() }
            }
            (Some(..), Some(..)) => {
                bail!("cannot configure both `package.workspace` and \
                       `[workspace]`, only one can be specified")
            }
        };
        let profiles = build_profiles(&me.profile);
        let publish = match project.publish {
            Some(VecStringOrBool::VecString(ref vecstring)) => {
                features.require(Feature::alternative_registries()).chain_err(|| {
                    "the `publish` manifest key is unstable for anything other than a value of true or false"
                })?;
                Some(vecstring.clone())
            },
            Some(VecStringOrBool::Bool(false)) => Some(vec![]),
            _ => None,
        };
        let mut manifest = Manifest::new(summary,
                                         targets,
                                         exclude,
                                         include,
                                         project.links.clone(),
                                         metadata,
                                         profiles,
                                         publish,
                                         replace,
                                         patch,
                                         workspace_config,
                                         features,
                                         project.im_a_teapot,
                                         Rc::clone(me));
        if project.license_file.is_some() && project.license.is_some() {
            manifest.add_warning("only one of `license` or \
                                 `license-file` is necessary".to_string());
        }
        for warning in warnings {
            manifest.add_warning(warning);
        }
        for error in errors {
            manifest.add_critical_warning(error);
        }

        manifest.feature_gate()?;

        Ok((manifest, nested_paths))
    }

    fn to_virtual_manifest(me: &Rc<TomlManifest>,
                           source_id: &SourceId,
                           root: &Path,
                           config: &Config)
                           -> CargoResult<(VirtualManifest, Vec<PathBuf>)> {
        if me.project.is_some() {
            bail!("virtual manifests do not define [project]");
        }
        if me.package.is_some() {
            bail!("virtual manifests do not define [package]");
        }
        if me.lib.is_some() {
            bail!("virtual manifests do not specify [lib]");
        }
        if me.bin.is_some() {
            bail!("virtual manifests do not specify [[bin]]");
        }
        if me.example.is_some() {
            bail!("virtual manifests do not specify [[example]]");
        }
        if me.test.is_some() {
            bail!("virtual manifests do not specify [[test]]");
        }
        if me.bench.is_some() {
            bail!("virtual manifests do not specify [[bench]]");
        }

        let mut nested_paths = Vec::new();
        let mut warnings = Vec::new();
        let mut deps = Vec::new();
        let empty = Vec::new();
        let cargo_features = me.cargo_features.as_ref().unwrap_or(&empty);
        let features = Features::new(&cargo_features, &mut warnings)?;

        let (replace, patch) = {
            let mut cx = Context {
                pkgid: None,
                deps: &mut deps,
                source_id: source_id,
                nested_paths: &mut nested_paths,
                config: config,
                warnings: &mut warnings,
                platform: None,
                features: &features,
                root: root
            };
            (me.replace(&mut cx)?, me.patch(&mut cx)?)
        };
        let profiles = build_profiles(&me.profile);
        let workspace_config = match me.workspace {
            Some(ref config) => {
                WorkspaceConfig::Root(
                    WorkspaceRootConfig::new(&root, &config.members, &config.exclude)
                )
            }
            None => {
                bail!("virtual manifests must be configured with [workspace]");
            }
        };
        Ok((VirtualManifest::new(replace, patch, workspace_config, profiles), nested_paths))
    }

    fn replace(&self, cx: &mut Context)
               -> CargoResult<Vec<(PackageIdSpec, Dependency)>> {
        if self.patch.is_some() && self.replace.is_some() {
            bail!("cannot specify both [replace] and [patch]");
        }
        let mut replace = Vec::new();
        for (spec, replacement) in self.replace.iter().flat_map(|x| x) {
            let mut spec = PackageIdSpec::parse(spec).chain_err(|| {
                format!("replacements must specify a valid semver \
                         version to replace, but `{}` does not",
                        spec)
            })?;
            if spec.url().is_none() {
                spec.set_url(CRATES_IO.parse().unwrap());
            }

            let version_specified = match *replacement {
                TomlDependency::Detailed(ref d) => d.version.is_some(),
                TomlDependency::Simple(..) => true,
            };
            if version_specified {
                bail!("replacements cannot specify a version \
                       requirement, but found one for `{}`", spec);
            }

            let mut dep = replacement.to_dependency(spec.name(), cx, None)?;
            {
                let version = spec.version().ok_or_else(|| {
                    CargoError::from(format!("replacements must specify a version \
                             to replace, but `{}` does not",
                            spec))
                })?;
                dep.set_version_req(VersionReq::exact(version));
            }
            replace.push((spec, dep));
        }
        Ok(replace)
    }

    fn patch(&self, cx: &mut Context)
             -> CargoResult<HashMap<Url, Vec<Dependency>>> {
        let mut patch = HashMap::new();
        for (url, deps) in self.patch.iter().flat_map(|x| x) {
            let url = match &url[..] {
                "crates-io" => CRATES_IO.parse().unwrap(),
                _ => url.to_url()?,
            };
            patch.insert(url, deps.iter().map(|(name, dep)| {
                dep.to_dependency(name, cx, None)
            }).collect::<CargoResult<Vec<_>>>()?);
        }
        Ok(patch)
    }

    fn maybe_custom_build(&self,
                          build: &Option<StringOrBool>,
                          package_root: &Path)
                          -> Option<PathBuf> {
        let build_rs = package_root.join("build.rs");
        match *build {
            Some(StringOrBool::Bool(false)) => None,        // explicitly no build script
            Some(StringOrBool::Bool(true)) => Some(build_rs.into()),
            Some(StringOrBool::String(ref s)) => Some(PathBuf::from(s)),
            None => {
                match fs::metadata(&build_rs) {
                    // If there is a build.rs file next to the Cargo.toml, assume it is
                    // a build script
                    Ok(ref e) if e.is_file() => Some(build_rs.into()),
                    Ok(_) | Err(_) => None,
                }
            }
        }
    }
}

/// Will check a list of build targets, and make sure the target names are unique within a vector.
/// If not, the name of the offending build target is returned.
fn unique_build_targets(targets: &[Target], package_root: &Path) -> Result<(), String> {
    let mut seen = HashSet::new();
    for v in targets.iter().map(|e| package_root.join(e.src_path())) {
        if !seen.insert(v.clone()) {
            return Err(v.display().to_string());
        }
    }
    Ok(())
}

impl TomlDependency {
    fn to_dependency(&self,
                     name: &str,
                     cx: &mut Context,
                     kind: Option<Kind>)
                     -> CargoResult<Dependency> {
        let details = match *self {
            TomlDependency::Simple(ref version) => DetailedTomlDependency {
                version: Some(version.clone()),
                .. Default::default()
            },
            TomlDependency::Detailed(ref details) => details.clone(),
        };

        if details.version.is_none() && details.path.is_none() &&
           details.git.is_none() {
            let msg = format!("dependency ({}) specified without \
                               providing a local path, Git repository, or \
                               version to use. This will be considered an \
                               error in future versions", name);
            cx.warnings.push(msg);
        }

        if details.git.is_none() {
            let git_only_keys = [
                (&details.branch, "branch"),
                (&details.tag, "tag"),
                (&details.rev, "rev")
            ];

            for &(key, key_name) in &git_only_keys {
                if key.is_some() {
                    let msg = format!("key `{}` is ignored for dependency ({}). \
                                       This will be considered an error in future versions",
                                      key_name, name);
                    cx.warnings.push(msg)
                }
            }
        }

        let new_source_id = match (details.git.as_ref(), details.path.as_ref(), details.registry.as_ref()) {
            (Some(_), _, Some(_)) => bail!("dependency ({}) specification is ambiguous. \
                                            Only one of `git` or `registry` is allowed.", name),
            (_, Some(_), Some(_)) => bail!("dependency ({}) specification is ambiguous. \
                                            Only one of `path` or `registry` is allowed.", name),
            (Some(git), maybe_path, _) => {
                if maybe_path.is_some() {
                    let msg = format!("dependency ({}) specification is ambiguous. \
                                       Only one of `git` or `path` is allowed. \
                                       This will be considered an error in future versions", name);
                    cx.warnings.push(msg)
                }

                let n_details = [&details.branch, &details.tag, &details.rev]
                    .iter()
                    .filter(|d| d.is_some())
                    .count();

                if n_details > 1 {
                    let msg = format!("dependency ({}) specification is ambiguous. \
                                       Only one of `branch`, `tag` or `rev` is allowed. \
                                       This will be considered an error in future versions", name);
                    cx.warnings.push(msg)
                }

                let reference = details.branch.clone().map(GitReference::Branch)
                    .or_else(|| details.tag.clone().map(GitReference::Tag))
                    .or_else(|| details.rev.clone().map(GitReference::Rev))
                    .unwrap_or_else(|| GitReference::Branch("master".to_string()));
                let loc = git.to_url()?;
                SourceId::for_git(&loc, reference)?
            },
            (None, Some(path), _) => {
                cx.nested_paths.push(PathBuf::from(path));
                // If the source id for the package we're parsing is a path
                // source, then we normalize the path here to get rid of
                // components like `..`.
                //
                // The purpose of this is to get a canonical id for the package
                // that we're depending on to ensure that builds of this package
                // always end up hashing to the same value no matter where it's
                // built from.
                if cx.source_id.is_path() {
                    let path = cx.root.join(path);
                    let path = util::normalize_path(&path);
                    SourceId::for_path(&path)?
                } else {
                    cx.source_id.clone()
                }
            },
            (None, None, Some(registry)) => {
                cx.features.require(Feature::alternative_registries())?;
                SourceId::alt_registry(cx.config, registry)?
            }
            (None, None, None) => SourceId::crates_io(cx.config)?,
        };

        let version = details.version.as_ref().map(|v| &v[..]);
        let mut dep = match cx.pkgid {
            Some(id) => {
                Dependency::parse(name, version, &new_source_id,
                                  id, cx.config)?
            }
            None => Dependency::parse_no_deprecated(name, version, &new_source_id)?,
        };
        dep.set_features(details.features.unwrap_or_default())
           .set_default_features(details.default_features
                                        .or(details.default_features2)
                                        .unwrap_or(true))
           .set_optional(details.optional.unwrap_or(false))
           .set_platform(cx.platform.clone());
        if let Some(kind) = kind {
            dep.set_kind(kind);
        }
        Ok(dep)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
struct TomlTarget {
    name: Option<String>,

    // The intention was to only accept `crate-type` here but historical
    // versions of Cargo also accepted `crate_type`, so look for both.
    #[serde(rename = "crate-type")]
    crate_type: Option<Vec<String>>,
    #[serde(rename = "crate_type")]
    crate_type2: Option<Vec<String>>,

    path: Option<PathValue>,
    test: Option<bool>,
    doctest: Option<bool>,
    bench: Option<bool>,
    doc: Option<bool>,
    plugin: Option<bool>,
    #[serde(rename = "proc-macro")]
    proc_macro: Option<bool>,
    #[serde(rename = "proc_macro")]
    proc_macro2: Option<bool>,
    harness: Option<bool>,
    #[serde(rename = "required-features")]
    required_features: Option<Vec<String>>,
}

#[derive(Clone)]
struct PathValue(PathBuf);

impl<'de> de::Deserialize<'de> for PathValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: de::Deserializer<'de>
    {
        Ok(PathValue(String::deserialize(deserializer)?.into()))
    }
}

impl ser::Serialize for PathValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer,
    {
        self.0.serialize(serializer)
    }
}

/// Corresponds to a `target` entry, but `TomlTarget` is already used.
#[derive(Serialize, Deserialize, Debug)]
struct TomlPlatform {
    dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "build-dependencies")]
    build_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "build_dependencies")]
    build_dependencies2: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "dev-dependencies")]
    dev_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "dev_dependencies")]
    dev_dependencies2: Option<BTreeMap<String, TomlDependency>>,
}

impl TomlTarget {
    fn new() -> TomlTarget {
        TomlTarget::default()
    }

    fn name(&self) -> String {
        match self.name {
            Some(ref name) => name.clone(),
            None => panic!("target name is required")
        }
    }

    fn proc_macro(&self) -> Option<bool> {
        self.proc_macro.or(self.proc_macro2)
    }

    fn crate_types(&self) -> Option<&Vec<String>> {
        self.crate_type.as_ref().or_else(|| self.crate_type2.as_ref())
    }
}

impl fmt::Debug for PathValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

fn build_profiles(profiles: &Option<TomlProfiles>) -> Profiles {
    let profiles = profiles.as_ref();
    let mut profiles = Profiles {
        release: merge(Profile::default_release(),
                       profiles.and_then(|p| p.release.as_ref())),
        dev: merge(Profile::default_dev(),
                   profiles.and_then(|p| p.dev.as_ref())),
        test: merge(Profile::default_test(),
                    profiles.and_then(|p| p.test.as_ref())),
        test_deps: merge(Profile::default_dev(),
                         profiles.and_then(|p| p.dev.as_ref())),
        bench: merge(Profile::default_bench(),
                     profiles.and_then(|p| p.bench.as_ref())),
        bench_deps: merge(Profile::default_release(),
                          profiles.and_then(|p| p.release.as_ref())),
        doc: merge(Profile::default_doc(),
                   profiles.and_then(|p| p.doc.as_ref())),
        custom_build: Profile::default_custom_build(),
        check: merge(Profile::default_check(),
                     profiles.and_then(|p| p.dev.as_ref())),
        check_test: merge(Profile::default_check_test(),
                          profiles.and_then(|p| p.dev.as_ref())),
        doctest: Profile::default_doctest(),
    };
    // The test/bench targets cannot have panic=abort because they'll all get
    // compiled with --test which requires the unwind runtime currently
    profiles.test.panic = None;
    profiles.bench.panic = None;
    profiles.test_deps.panic = None;
    profiles.bench_deps.panic = None;
    return profiles;

    fn merge(profile: Profile, toml: Option<&TomlProfile>) -> Profile {
        let &TomlProfile {
            ref opt_level, lto, codegen_units, ref debug, debug_assertions, rpath,
            ref panic, ref overflow_checks,
        } = match toml {
            Some(toml) => toml,
            None => return profile,
        };
        let debug = match *debug {
            Some(U32OrBool::U32(debug)) => Some(Some(debug)),
            Some(U32OrBool::Bool(true)) => Some(Some(2)),
            Some(U32OrBool::Bool(false)) => Some(None),
            None => None,
        };
        Profile {
            opt_level: opt_level.clone().unwrap_or(TomlOptLevel(profile.opt_level)).0,
            lto: lto.unwrap_or(profile.lto),
            codegen_units: codegen_units,
            rustc_args: None,
            rustdoc_args: None,
            debuginfo: debug.unwrap_or(profile.debuginfo),
            debug_assertions: debug_assertions.unwrap_or(profile.debug_assertions),
            overflow_checks: overflow_checks.unwrap_or(profile.overflow_checks),
            rpath: rpath.unwrap_or(profile.rpath),
            test: profile.test,
            doc: profile.doc,
            run_custom_build: profile.run_custom_build,
            check: profile.check,
            panic: panic.clone().or(profile.panic),
        }
    }
}
