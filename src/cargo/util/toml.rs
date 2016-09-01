use std::collections::{HashMap, HashSet};
use std::default::Default;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str;

use toml;
use semver::{self, VersionReq};
use rustc_serialize::{Decodable, Decoder};

use core::{SourceId, Profiles, PackageIdSpec, GitReference, WorkspaceConfig};
use core::{Summary, Manifest, Target, Dependency, DependencyInner, PackageId};
use core::{EitherManifest, VirtualManifest};
use core::dependency::{Kind, Platform};
use core::manifest::{LibKind, Profile, ManifestMetadata};
use core::package_id::Metadata;
use util::{self, CargoResult, human, ToUrl, ToSemver, ChainError, Config};

/// Representation of the projects file layout.
///
/// This structure is used to hold references to all project files that are relevant to cargo.

#[derive(Clone)]
pub struct Layout {
    pub root: PathBuf,
    lib: Option<PathBuf>,
    bins: Vec<PathBuf>,
    examples: Vec<PathBuf>,
    tests: Vec<PathBuf>,
    benches: Vec<PathBuf>,
}

impl Layout {
    /// Returns a new `Layout` for a given root path.
    /// The `root_path` represents the directory that contains the `Cargo.toml` file.
    pub fn from_project_path(root_path: &Path) -> Layout {
        let mut lib = None;
        let mut bins = vec![];
        let mut examples = vec![];
        let mut tests = vec![];
        let mut benches = vec![];

        let lib_canidate = root_path.join("src").join("lib.rs");
        if fs::metadata(&lib_canidate).is_ok() {
            lib = Some(lib_canidate);
        }

        try_add_file(&mut bins, root_path.join("src").join("main.rs"));
        try_add_files(&mut bins, root_path.join("src").join("bin"));

        try_add_files(&mut examples, root_path.join("examples"));

        try_add_files(&mut tests, root_path.join("tests"));
        try_add_files(&mut benches, root_path.join("benches"));

        Layout {
            root: root_path.to_path_buf(),
            lib: lib,
            bins: bins,
            examples: examples,
            tests: tests,
            benches: benches,
        }
    }

    fn main(&self) -> Option<&PathBuf> {
        self.bins.iter().find(|p| {
            match p.file_name().and_then(|s| s.to_str()) {
                Some(s) => s == "main.rs",
                None => false
            }
        })
    }
}

fn try_add_file(files: &mut Vec<PathBuf>, file: PathBuf) {
    if fs::metadata(&file).is_ok() {
        files.push(file);
    }
}
fn try_add_files(files: &mut Vec<PathBuf>, root: PathBuf) {
    match fs::read_dir(&root) {
        Ok(new) => {
            files.extend(new.filter_map(|dir| {
                dir.map(|d| d.path()).ok()
            }).filter(|f| {
                f.extension().and_then(|s| s.to_str()) == Some("rs")
            }).filter(|f| {
                // Some unix editors may create "dotfiles" next to original
                // source files while they're being edited, but these files are
                // rarely actually valid Rust source files and sometimes aren't
                // even valid UTF-8. Here we just ignore all of them and require
                // that they are explicitly specified in Cargo.toml if desired.
                f.file_name().and_then(|s| s.to_str()).map(|s| {
                    !s.starts_with('.')
                }).unwrap_or(true)
            }))
        }
        Err(_) => {/* just don't add anything if the directory doesn't exist, etc. */}
    }
}

pub fn to_manifest(contents: &str,
                   source_id: &SourceId,
                   layout: Layout,
                   config: &Config)
                   -> CargoResult<(EitherManifest, Vec<PathBuf>)> {
    let manifest = layout.root.join("Cargo.toml");
    let manifest = match util::without_prefix(&manifest, config.cwd()) {
        Some(path) => path.to_path_buf(),
        None => manifest.clone(),
    };
    let root = try!(parse(contents, &manifest, config));
    let mut d = toml::Decoder::new(toml::Value::Table(root));
    let manifest: TomlManifest = try!(Decodable::decode(&mut d).map_err(|e| {
        human(e.to_string())
    }));

    return match manifest.to_real_manifest(source_id, &layout, config) {
        Ok((mut manifest, paths)) => {
            if let Some(ref toml) = d.toml {
                add_unused_keys(&mut manifest, toml, String::new());
            }
            if !manifest.targets().iter().any(|t| !t.is_custom_build()) {
                bail!("no targets specified in the manifest\n  \
                       either src/lib.rs, src/main.rs, a [lib] section, or \
                       [[bin]] section must be present")
            }
            Ok((EitherManifest::Real(manifest), paths))
        }
        Err(e) => {
            match manifest.to_virtual_manifest(source_id, &layout, config) {
                Ok((m, paths)) => Ok((EitherManifest::Virtual(m), paths)),
                Err(..) => Err(e),
            }
        }
    };

    fn add_unused_keys(m: &mut Manifest, toml: &toml::Value, key: String) {
        if key == "package.metadata" {
            return
        }
        match *toml {
            toml::Value::Table(ref table) => {
                for (k, v) in table.iter() {
                    add_unused_keys(m, v, if key.is_empty() {
                        k.clone()
                    } else {
                        key.clone() + "." + k
                    })
                }
            }
            toml::Value::Array(ref arr) => {
                for v in arr.iter() {
                    add_unused_keys(m, v, key.clone());
                }
            }
            _ => m.add_warning(format!("unused manifest key: {}", key)),
        }
    }
}

pub fn parse(toml: &str,
             file: &Path,
             config: &Config) -> CargoResult<toml::Table> {
    let mut first_parser = toml::Parser::new(&toml);
    if let Some(toml) = first_parser.parse() {
        return Ok(toml);
    }

    let mut second_parser = toml::Parser::new(toml);
    second_parser.set_require_newline_after_table(false);
    if let Some(toml) = second_parser.parse() {
        let msg = format!("\
TOML file found which contains invalid syntax and will soon not parse
at `{}`.

The TOML spec requires newlines after table definitions (e.g. `[a] b = 1` is
invalid), but this file has a table header which does not have a newline after
it. A newline needs to be added and this warning will soon become a hard error
in the future.", file.display());
        try!(config.shell().warn(&msg));
        return Ok(toml)
    }

    let mut error_str = format!("could not parse input as TOML\n");
    for error in first_parser.errors.iter() {
        let (loline, locol) = first_parser.to_linecol(error.lo);
        let (hiline, hicol) = first_parser.to_linecol(error.hi);
        error_str.push_str(&format!("{}:{}:{}{} {}\n",
                                    file.display(),
                                    loline + 1, locol + 1,
                                    if loline != hiline || locol != hicol {
                                        format!("-{}:{}", hiline + 1,
                                                hicol + 1)
                                    } else {
                                        "".to_string()
                                    },
                                    error.desc));
    }
    Err(human(error_str))
}

type TomlLibTarget = TomlTarget;
type TomlBinTarget = TomlTarget;
type TomlExampleTarget = TomlTarget;
type TomlTestTarget = TomlTarget;
type TomlBenchTarget = TomlTarget;

#[derive(RustcDecodable)]
pub enum TomlDependency {
    Simple(String),
    Detailed(DetailedTomlDependency)
}


#[derive(RustcDecodable, Clone, Default)]
pub struct DetailedTomlDependency {
    version: Option<String>,
    path: Option<String>,
    git: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
    features: Option<Vec<String>>,
    optional: Option<bool>,
    default_features: Option<bool>,
}

#[derive(RustcDecodable)]
pub struct TomlManifest {
    package: Option<Box<TomlProject>>,
    project: Option<Box<TomlProject>>,
    profile: Option<TomlProfiles>,
    lib: Option<TomlLibTarget>,
    bin: Option<Vec<TomlBinTarget>>,
    example: Option<Vec<TomlExampleTarget>>,
    test: Option<Vec<TomlTestTarget>>,
    bench: Option<Vec<TomlTestTarget>>,
    dependencies: Option<HashMap<String, TomlDependency>>,
    dev_dependencies: Option<HashMap<String, TomlDependency>>,
    build_dependencies: Option<HashMap<String, TomlDependency>>,
    features: Option<HashMap<String, Vec<String>>>,
    target: Option<HashMap<String, TomlPlatform>>,
    replace: Option<HashMap<String, TomlDependency>>,
    workspace: Option<TomlWorkspace>,
}

#[derive(RustcDecodable, Clone, Default)]
pub struct TomlProfiles {
    test: Option<TomlProfile>,
    doc: Option<TomlProfile>,
    bench: Option<TomlProfile>,
    dev: Option<TomlProfile>,
    release: Option<TomlProfile>,
}

#[derive(Clone)]
pub struct TomlOptLevel(String);

impl Decodable for TomlOptLevel {
    fn decode<D: Decoder>(d: &mut D) -> Result<TomlOptLevel, D::Error> {
        match d.read_u32() {
            Ok(i) => Ok(TomlOptLevel(i.to_string())),
            Err(_) => {
                match d.read_str() {
                    Ok(ref s) if s == "s" || s == "z" =>
                        Ok(TomlOptLevel(s.to_string())),
                    Ok(_) | Err(_) =>
                        Err(d.error("expected an integer, a string \"z\" or a string \"s\""))
                }
            }
        }
    }
}

#[derive(RustcDecodable, Clone, Default)]
pub struct TomlProfile {
    opt_level: Option<TomlOptLevel>,
    lto: Option<bool>,
    codegen_units: Option<u32>,
    debug: Option<bool>,
    debug_assertions: Option<bool>,
    rpath: Option<bool>,
    panic: Option<String>,
}

#[derive(RustcDecodable)]
pub struct TomlProject {
    name: String,
    version: TomlVersion,
    authors: Vec<String>,
    build: Option<String>,
    links: Option<String>,
    exclude: Option<Vec<String>>,
    include: Option<Vec<String>>,
    publish: Option<bool>,
    workspace: Option<String>,

    // package metadata
    description: Option<String>,
    homepage: Option<String>,
    documentation: Option<String>,
    readme: Option<String>,
    keywords: Option<Vec<String>>,
    license: Option<String>,
    license_file: Option<String>,
    repository: Option<String>,
}

#[derive(RustcDecodable)]
pub struct TomlWorkspace {
    members: Option<Vec<String>>,
}

pub struct TomlVersion {
    version: semver::Version,
}

impl Decodable for TomlVersion {
    fn decode<D: Decoder>(d: &mut D) -> Result<TomlVersion, D::Error> {
        let s = try!(d.read_str());
        match s.to_semver() {
            Ok(s) => Ok(TomlVersion { version: s }),
            Err(e) => Err(d.error(&e)),
        }
    }
}

impl TomlProject {
    pub fn to_package_id(&self, source_id: &SourceId) -> CargoResult<PackageId> {
        PackageId::new(&self.name, self.version.version.clone(),
                       source_id)
    }
}

struct Context<'a, 'b> {
    deps: &'a mut Vec<Dependency>,
    source_id: &'a SourceId,
    nested_paths: &'a mut Vec<PathBuf>,
    config: &'b Config,
    warnings: &'a mut Vec<String>,
    platform: Option<Platform>,
    layout: &'a Layout,
}

// These functions produce the equivalent of specific manifest entries. One
// wrinkle is that certain paths cannot be represented in the manifest due
// to Toml's UTF-8 requirement. This could, in theory, mean that certain
// otherwise acceptable executable names are not used when inside of
// `src/bin/*`, but it seems ok to not build executables with non-UTF8
// paths.
fn inferred_lib_target(name: &str, layout: &Layout) -> Option<TomlTarget> {
    layout.lib.as_ref().map(|lib| {
        TomlTarget {
            name: Some(name.to_string()),
            path: Some(PathValue::Path(lib.clone())),
            .. TomlTarget::new()
        }
    })
}

fn inferred_bin_targets(name: &str, layout: &Layout) -> Vec<TomlTarget> {
    layout.bins.iter().filter_map(|bin| {
        let name = if &**bin == Path::new("src/main.rs") ||
                      *bin == layout.root.join("src").join("main.rs") {
            Some(name.to_string())
        } else {
            bin.file_stem().and_then(|s| s.to_str()).map(|f| f.to_string())
        };

        name.map(|name| {
            TomlTarget {
                name: Some(name),
                path: Some(PathValue::Path(bin.clone())),
                .. TomlTarget::new()
            }
        })
    }).collect()
}

fn inferred_example_targets(layout: &Layout) -> Vec<TomlTarget> {
    layout.examples.iter().filter_map(|ex| {
        ex.file_stem().and_then(|s| s.to_str()).map(|name| {
            TomlTarget {
                name: Some(name.to_string()),
                path: Some(PathValue::Path(ex.clone())),
                .. TomlTarget::new()
            }
        })
    }).collect()
}

fn inferred_test_targets(layout: &Layout) -> Vec<TomlTarget> {
    layout.tests.iter().filter_map(|ex| {
        ex.file_stem().and_then(|s| s.to_str()).map(|name| {
            TomlTarget {
                name: Some(name.to_string()),
                path: Some(PathValue::Path(ex.clone())),
                .. TomlTarget::new()
            }
        })
    }).collect()
}

fn inferred_bench_targets(layout: &Layout) -> Vec<TomlTarget> {
    layout.benches.iter().filter_map(|ex| {
        ex.file_stem().and_then(|s| s.to_str()).map(|name| {
            TomlTarget {
                name: Some(name.to_string()),
                path: Some(PathValue::Path(ex.clone())),
                .. TomlTarget::new()
            }
        })
    }).collect()
}

impl TomlManifest {
    fn to_real_manifest(&self,
                        source_id: &SourceId,
                        layout: &Layout,
                        config: &Config)
                        -> CargoResult<(Manifest, Vec<PathBuf>)> {
        let mut nested_paths = vec![];
        let mut warnings = vec![];

        let project = self.project.as_ref().or_else(|| self.package.as_ref());
        let project = try!(project.chain_error(|| {
            human("no `package` or `project` section found.")
        }));

        if project.name.trim().is_empty() {
            bail!("package name cannot be an empty string.")
        }

        let pkgid = try!(project.to_package_id(source_id));
        let metadata = pkgid.generate_metadata();

        // If we have no lib at all, use the inferred lib if available
        // If we have a lib with a path, we're done
        // If we have a lib with no path, use the inferred lib or_else package name

        let lib = match self.lib {
            Some(ref lib) => {
                try!(lib.validate_library_name());
                try!(lib.validate_crate_type());
                Some(
                    TomlTarget {
                        name: lib.name.clone().or(Some(project.name.clone())),
                        path: lib.path.clone().or(
                            layout.lib.as_ref().map(|p| PathValue::Path(p.clone()))
                        ),
                        ..lib.clone()
                    }
                )
            }
            None => inferred_lib_target(&project.name, layout),
        };

        let bins = match self.bin {
            Some(ref bins) => {
                let bin = layout.main();

                for target in bins {
                    try!(target.validate_binary_name());
                }

                bins.iter().map(|t| {
                    if bin.is_some() && t.path.is_none() {
                        TomlTarget {
                            path: bin.as_ref().map(|&p| PathValue::Path(p.clone())),
                            .. t.clone()
                        }
                    } else {
                        t.clone()
                    }
                }).collect()
            }
            None => inferred_bin_targets(&project.name, layout)
        };

        let blacklist = vec!["build", "deps", "examples", "native"];

        for bin in bins.iter() {
            if blacklist.iter().find(|&x| *x == bin.name()) != None {
                bail!("the binary target name `{}` is forbidden",
                      bin.name())
            }
        }

        let examples = match self.example {
            Some(ref examples) => {
                for target in examples {
                    try!(target.validate_example_name());
                }
                examples.clone()
            }
            None => inferred_example_targets(layout)
        };

        let tests = match self.test {
            Some(ref tests) => {
                for target in tests {
                    try!(target.validate_test_name());
                }
                tests.clone()
            }
            None => inferred_test_targets(layout)
        };

        let benches = match self.bench {
            Some(ref benches) => {
                for target in benches {
                    try!(target.validate_bench_name());
                }
                benches.clone()
            }
            None => inferred_bench_targets(layout)
        };

        if let Err(e) = unique_names_in_targets(&bins) {
            bail!("found duplicate binary name {}, but all binary targets \
                   must have a unique name", e);
        }

        if let Err(e) = unique_names_in_targets(&examples) {
            bail!("found duplicate example name {}, but all binary targets \
                   must have a unique name", e);
        }

        if let Err(e) = unique_names_in_targets(&benches) {
            bail!("found duplicate bench name {}, but all binary targets must \
                   have a unique name", e);
        }

        if let Err(e) = unique_names_in_targets(&tests) {
            bail!("found duplicate test name {}, but all binary targets must \
                   have a unique name", e)
        }

        // processing the custom build script
        let new_build = project.build.as_ref().map(PathBuf::from);

        // Get targets
        let targets = normalize(&lib,
                                &bins,
                                new_build,
                                &examples,
                                &tests,
                                &benches,
                                &metadata);

        if targets.is_empty() {
            debug!("manifest has no build targets");
        }

        if let Err(e) = unique_build_targets(&targets, layout) {
            warnings.push(format!("file found to be present in multiple \
                                   build targets: {}", e));
        }

        let mut deps = Vec::new();
        let replace;

        {

            let mut cx = Context {
                deps: &mut deps,
                source_id: source_id,
                nested_paths: &mut nested_paths,
                config: config,
                warnings: &mut warnings,
                platform: None,
                layout: &layout,
            };

            fn process_dependencies(
                cx: &mut Context,
                new_deps: Option<&HashMap<String, TomlDependency>>,
                kind: Option<Kind>)
                -> CargoResult<()>
            {
                let dependencies = match new_deps {
                    Some(ref dependencies) => dependencies,
                    None => return Ok(())
                };
                for (n, v) in dependencies.iter() {
                    let dep = try!(v.to_dependency(n, cx, kind));
                    cx.deps.push(dep);
                }

                Ok(())
            }

            // Collect the deps
            try!(process_dependencies(&mut cx, self.dependencies.as_ref(),
                                      None));
            try!(process_dependencies(&mut cx, self.dev_dependencies.as_ref(),
                                      Some(Kind::Development)));
            try!(process_dependencies(&mut cx, self.build_dependencies.as_ref(),
                                      Some(Kind::Build)));

            for (name, platform) in self.target.iter().flat_map(|t| t) {
                cx.platform = Some(try!(name.parse()));
                try!(process_dependencies(&mut cx,
                                          platform.dependencies.as_ref(),
                                          None));
                try!(process_dependencies(&mut cx,
                                          platform.build_dependencies.as_ref(),
                                          Some(Kind::Build)));
                try!(process_dependencies(&mut cx,
                                          platform.dev_dependencies.as_ref(),
                                          Some(Kind::Development)));
            }

            replace = try!(self.replace(&mut cx));
        }

        {
            let mut names_sources = HashMap::new();
            for dep in deps.iter() {
                let name = dep.name();
                let prev = names_sources.insert(name, dep.source_id());
                if prev.is_some() && prev != Some(dep.source_id()) {
                    bail!("found duplicate dependency name {}, but all \
                           dependencies must have a unique name", name);
                }
            }
        }

        let exclude = project.exclude.clone().unwrap_or(Vec::new());
        let include = project.include.clone().unwrap_or(Vec::new());

        let summary = try!(Summary::new(pkgid, deps,
                                        self.features.clone()
                                            .unwrap_or(HashMap::new())));
        let metadata = ManifestMetadata {
            description: project.description.clone(),
            homepage: project.homepage.clone(),
            documentation: project.documentation.clone(),
            readme: project.readme.clone(),
            authors: project.authors.clone(),
            license: project.license.clone(),
            license_file: project.license_file.clone(),
            repository: project.repository.clone(),
            keywords: project.keywords.clone().unwrap_or(Vec::new()),
        };

        let workspace_config = match (self.workspace.as_ref(),
                                      project.workspace.as_ref()) {
            (Some(config), None) => {
                WorkspaceConfig::Root { members: config.members.clone() }
            }
            (None, root) => {
                WorkspaceConfig::Member { root: root.cloned() }
            }
            (Some(..), Some(..)) => {
                bail!("cannot configure both `package.workspace` and \
                       `[workspace]`, only one can be specified")
            }
        };
        let profiles = build_profiles(&self.profile);
        let publish = project.publish.unwrap_or(true);
        let mut manifest = Manifest::new(summary,
                                         targets,
                                         exclude,
                                         include,
                                         project.links.clone(),
                                         metadata,
                                         profiles,
                                         publish,
                                         replace,
                                         workspace_config);
        if project.license_file.is_some() && project.license.is_some() {
            manifest.add_warning("only one of `license` or \
                                 `license-file` is necessary".to_string());
        }
        for warning in warnings {
            manifest.add_warning(warning.clone());
        }

        Ok((manifest, nested_paths))
    }

    fn to_virtual_manifest(&self,
                           source_id: &SourceId,
                           layout: &Layout,
                           config: &Config)
                           -> CargoResult<(VirtualManifest, Vec<PathBuf>)> {
        if self.project.is_some() {
            bail!("virtual manifests do not define [project]");
        }
        if self.package.is_some() {
            bail!("virtual manifests do not define [package]");
        }
        if self.lib.is_some() {
            bail!("virtual manifests do not specifiy [lib]");
        }
        if self.bin.is_some() {
            bail!("virtual manifests do not specifiy [[bin]]");
        }
        if self.example.is_some() {
            bail!("virtual manifests do not specifiy [[example]]");
        }
        if self.test.is_some() {
            bail!("virtual manifests do not specifiy [[test]]");
        }
        if self.bench.is_some() {
            bail!("virtual manifests do not specifiy [[bench]]");
        }

        let mut nested_paths = Vec::new();
        let mut warnings = Vec::new();
        let mut deps = Vec::new();
        let replace = try!(self.replace(&mut Context {
            deps: &mut deps,
            source_id: source_id,
            nested_paths: &mut nested_paths,
            config: config,
            warnings: &mut warnings,
            platform: None,
            layout: layout,
        }));
        let workspace_config = match self.workspace {
            Some(ref config) => {
                WorkspaceConfig::Root { members: config.members.clone() }
            }
            None => {
                bail!("virtual manifests must be configured with [workspace]");
            }
        };
        Ok((VirtualManifest::new(replace, workspace_config), nested_paths))
    }

    fn replace(&self, cx: &mut Context)
               -> CargoResult<Vec<(PackageIdSpec, Dependency)>> {
        let mut replace = Vec::new();
        for (spec, replacement) in self.replace.iter().flat_map(|x| x) {
            let spec = try!(PackageIdSpec::parse(spec));

            let version_specified = match *replacement {
                TomlDependency::Detailed(ref d) => d.version.is_some(),
                TomlDependency::Simple(..) => true,
            };
            if version_specified {
                bail!("replacements cannot specify a version \
                       requirement, but found one for `{}`", spec);
            }

            let dep = try!(replacement.to_dependency(spec.name(), cx, None));
            let dep = {
                let version = try!(spec.version().chain_error(|| {
                    human(format!("replacements must specify a version \
                                   to replace, but `{}` does not",
                                  spec))
                }));
                let req = VersionReq::exact(version);
                dep.clone_inner().set_version_req(req)
                   .into_dependency()
            };
            replace.push((spec, dep));
        }
        Ok(replace)
    }
}

/// Will check a list of toml targets, and make sure the target names are unique within a vector.
/// If not, the name of the offending binary target is returned.
fn unique_names_in_targets(targets: &[TomlTarget]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for v in targets.iter().map(|e| e.name()) {
        if !seen.insert(v.clone()) {
            return Err(v);
        }
    }
    Ok(())
}

/// Will check a list of build targets, and make sure the target names are unique within a vector.
/// If not, the name of the offending build target is returned.
fn unique_build_targets(targets: &[Target], layout: &Layout) -> Result<(), String> {
    let mut seen = HashSet::new();
    for v in targets.iter().map(|e| layout.root.join(e.src_path())) {
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

            for &(key, key_name) in git_only_keys.iter() {
                if key.is_some() {
                    let msg = format!("key `{}` is ignored for dependency ({}). \
                                       This will be considered an error in future versions",
                                      key_name, name);
                    cx.warnings.push(msg)
                }
            }
        }

        let new_source_id = match (details.git.as_ref(), details.path.as_ref()) {
            (Some(git), maybe_path) => {
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
                let loc = try!(git.to_url());
                SourceId::for_git(&loc, reference)
            },
            (None, Some(path)) => {
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
                    let path = cx.layout.root.join(path);
                    let path = util::normalize_path(&path);
                    try!(SourceId::for_path(&path))
                } else {
                    cx.source_id.clone()
                }
            },
            (None, None) => try!(SourceId::crates_io(cx.config)),
        };

        let version = details.version.as_ref().map(|v| &v[..]);
        let mut dep = try!(DependencyInner::parse(name, version, &new_source_id));
        dep = dep.set_features(details.features.unwrap_or(Vec::new()))
                 .set_default_features(details.default_features.unwrap_or(true))
                 .set_optional(details.optional.unwrap_or(false))
                 .set_platform(cx.platform.clone());
        if let Some(kind) = kind {
            dep = dep.set_kind(kind);
        }
        Ok(dep.into_dependency())
    }
}

#[derive(RustcDecodable, Debug, Clone)]
struct TomlTarget {
    name: Option<String>,
    crate_type: Option<Vec<String>>,
    path: Option<PathValue>,
    test: Option<bool>,
    doctest: Option<bool>,
    bench: Option<bool>,
    doc: Option<bool>,
    plugin: Option<bool>,
    rustc_macro: Option<bool>,
    harness: Option<bool>,
}

#[derive(RustcDecodable, Clone)]
enum PathValue {
    String(String),
    Path(PathBuf),
}

/// Corresponds to a `target` entry, but `TomlTarget` is already used.
#[derive(RustcDecodable)]
struct TomlPlatform {
    dependencies: Option<HashMap<String, TomlDependency>>,
    build_dependencies: Option<HashMap<String, TomlDependency>>,
    dev_dependencies: Option<HashMap<String, TomlDependency>>,
}

impl TomlTarget {
    fn new() -> TomlTarget {
        TomlTarget {
            name: None,
            crate_type: None,
            path: None,
            test: None,
            doctest: None,
            bench: None,
            doc: None,
            plugin: None,
            rustc_macro: None,
            harness: None,
        }
    }

    fn name(&self) -> String {
        match self.name {
            Some(ref name) => name.clone(),
            None => panic!("target name is required")
        }
    }

    fn validate_library_name(&self) -> CargoResult<()> {
        match self.name {
            Some(ref name) => {
                if name.trim().is_empty() {
                    Err(human("library target names cannot be empty.".to_string()))
                } else if name.contains('-') {
                    Err(human(format!("library target names cannot contain hyphens: {}",
                                      name)))
                } else {
                    Ok(())
                }
            },
            None => Ok(())
        }
    }

    fn validate_binary_name(&self) -> CargoResult<()> {
        match self.name {
            Some(ref name) => {
                if name.trim().is_empty() {
                    Err(human("binary target names cannot be empty.".to_string()))
                } else {
                    Ok(())
                }
            },
            None => Err(human("binary target bin.name is required".to_string()))
        }
    }

    fn validate_example_name(&self) -> CargoResult<()> {
        match self.name {
            Some(ref name) => {
                if name.trim().is_empty() {
                    Err(human("example target names cannot be empty".to_string()))
                } else {
                    Ok(())
                }
            },
            None => Err(human("example target example.name is required".to_string()))
        }
    }

    fn validate_test_name(&self) -> CargoResult<()> {
        match self.name {
            Some(ref name) => {
                if name.trim().is_empty() {
                    Err(human("test target names cannot be empty".to_string()))
                } else {
                    Ok(())
                }
            },
            None => Err(human("test target test.name is required".to_string()))
        }
    }

    fn validate_bench_name(&self) -> CargoResult<()> {
        match self.name {
            Some(ref name) => {
                if name.trim().is_empty() {
                    Err(human("bench target names cannot be empty".to_string()))
                } else {
                    Ok(())
                }
            },
            None => Err(human("bench target bench.name is required".to_string()))
        }
    }

    fn validate_crate_type(&self) -> CargoResult<()> {
        // Per the Macros 1.1 RFC:
        //
        // > Initially if a crate is compiled with the rustc-macro crate type
        // > (and possibly others) it will forbid exporting any items in the
        // > crate other than those functions tagged #[rustc_macro_derive] and
        // > those functions must also be placed at the crate root.
        //
        // A plugin requires exporting plugin_registrar so a crate cannot be
        // both at once.
        if self.plugin == Some(true) && self.rustc_macro == Some(true) {
            Err(human("lib.plugin and lib.rustc-macro cannot both be true".to_string()))
        } else {
            Ok(())
        }
    }
}

impl PathValue {
    fn to_path(&self) -> PathBuf {
        match *self {
            PathValue::String(ref s) => PathBuf::from(s),
            PathValue::Path(ref p) => p.clone(),
        }
    }
}

impl fmt::Debug for PathValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PathValue::String(ref s) => s.fmt(f),
            PathValue::Path(ref p) => p.display().fmt(f),
        }
    }
}

fn normalize(lib: &Option<TomlLibTarget>,
             bins: &[TomlBinTarget],
             custom_build: Option<PathBuf>,
             examples: &[TomlExampleTarget],
             tests: &[TomlTestTarget],
             benches: &[TomlBenchTarget],
             metadata: &Metadata) -> Vec<Target> {
    fn configure(toml: &TomlTarget, target: &mut Target) {
        let t2 = target.clone();
        target.set_tested(toml.test.unwrap_or(t2.tested()))
              .set_doc(toml.doc.unwrap_or(t2.documented()))
              .set_doctest(toml.doctest.unwrap_or(t2.doctested()))
              .set_benched(toml.bench.unwrap_or(t2.benched()))
              .set_harness(toml.harness.unwrap_or(t2.harness()))
              .set_for_host(match (toml.plugin, toml.rustc_macro) {
                  (None, None) => t2.for_host(),
                  (Some(true), _) | (_, Some(true)) => true,
                  (Some(false), _) | (_, Some(false)) => false,
              });
    }

    fn lib_target(dst: &mut Vec<Target>,
                  l: &TomlLibTarget,
                  metadata: &Metadata) {
        let path = l.path.clone().unwrap_or(
            PathValue::Path(Path::new("src").join(&format!("{}.rs", l.name())))
        );
        let crate_types = match l.crate_type.clone() {
            Some(kinds) => kinds.iter().map(|s| LibKind::from_str(s)).collect(),
            None => {
                vec![ if l.plugin == Some(true) {LibKind::Dylib}
                      else if l.rustc_macro == Some(true) {LibKind::RustcMacro}
                      else {LibKind::Lib} ]
            }
        };

        // Binaries, examples, etc, may link to this library. Their crate names
        // have a high likelihood to being the same as ours, however, so we need
        // some extra metadata in our name to ensure symbols won't collide.
        let mut metadata = metadata.clone();
        metadata.mix(&"lib");
        let mut target = Target::lib_target(&l.name(), crate_types,
                                            &path.to_path(),
                                            metadata);
        configure(l, &mut target);
        dst.push(target);
    }

    fn bin_targets(dst: &mut Vec<Target>, bins: &[TomlBinTarget],
                   default: &mut FnMut(&TomlBinTarget) -> PathBuf) {
        for bin in bins.iter() {
            let path = bin.path.clone().unwrap_or_else(|| {
                PathValue::Path(default(bin))
            });
            let mut target = Target::bin_target(&bin.name(), &path.to_path(),
                                                None);
            configure(bin, &mut target);
            dst.push(target);
        }
    }

    fn custom_build_target(dst: &mut Vec<Target>, cmd: &Path) {
        let name = format!("build-script-{}",
                           cmd.file_stem().and_then(|s| s.to_str()).unwrap_or(""));

        dst.push(Target::custom_build_target(&name, cmd, None));
    }

    fn example_targets(dst: &mut Vec<Target>,
                       examples: &[TomlExampleTarget],
                       default: &mut FnMut(&TomlExampleTarget) -> PathBuf) {
        for ex in examples.iter() {
            let path = ex.path.clone().unwrap_or_else(|| {
                PathValue::Path(default(ex))
            });

            let mut target = Target::example_target(&ex.name(), &path.to_path());
            configure(ex, &mut target);
            dst.push(target);
        }
    }

    fn test_targets(dst: &mut Vec<Target>, tests: &[TomlTestTarget],
                    metadata: &Metadata,
                    default: &mut FnMut(&TomlTestTarget) -> PathBuf) {
        for test in tests.iter() {
            let path = test.path.clone().unwrap_or_else(|| {
                PathValue::Path(default(test))
            });

            // make sure this metadata is different from any same-named libs.
            let mut metadata = metadata.clone();
            metadata.mix(&format!("test-{}", test.name()));

            let mut target = Target::test_target(&test.name(), &path.to_path(),
                                                 metadata);
            configure(test, &mut target);
            dst.push(target);
        }
    }

    fn bench_targets(dst: &mut Vec<Target>, benches: &[TomlBenchTarget],
                     metadata: &Metadata,
                     default: &mut FnMut(&TomlBenchTarget) -> PathBuf) {
        for bench in benches.iter() {
            let path = bench.path.clone().unwrap_or_else(|| {
                PathValue::Path(default(bench))
            });

            // make sure this metadata is different from any same-named libs.
            let mut metadata = metadata.clone();
            metadata.mix(&format!("bench-{}", bench.name()));

            let mut target = Target::bench_target(&bench.name(),
                                                  &path.to_path(),
                                                  metadata);
            configure(bench, &mut target);
            dst.push(target);
        }
    }

    let mut ret = Vec::new();

    if let Some(ref lib) = *lib {
        lib_target(&mut ret, lib, metadata);
        bin_targets(&mut ret, bins,
                    &mut |bin| Path::new("src").join("bin")
                                   .join(&format!("{}.rs", bin.name())));
    } else if bins.len() > 0 {
        bin_targets(&mut ret, bins,
                    &mut |bin| Path::new("src")
                                    .join(&format!("{}.rs", bin.name())));
    }

    if let Some(custom_build) = custom_build {
        custom_build_target(&mut ret, &custom_build);
    }

    example_targets(&mut ret, examples,
                    &mut |ex| Path::new("examples")
                                   .join(&format!("{}.rs", ex.name())));

    test_targets(&mut ret, tests, metadata, &mut |test| {
        if test.name() == "test" {
            Path::new("src").join("test.rs")
        } else {
            Path::new("tests").join(&format!("{}.rs", test.name()))
        }
    });

    bench_targets(&mut ret, benches, metadata, &mut |bench| {
        if bench.name() == "bench" {
            Path::new("src").join("bench.rs")
        } else {
            Path::new("benches").join(&format!("{}.rs", bench.name()))
        }
    });

    ret
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
    };
    profiles.test_deps.panic = None;
    profiles.bench_deps.panic = None;
    return profiles;

    fn merge(profile: Profile, toml: Option<&TomlProfile>) -> Profile {
        let &TomlProfile {
            ref opt_level, lto, codegen_units, debug, debug_assertions, rpath,
            ref panic
        } = match toml {
            Some(toml) => toml,
            None => return profile,
        };
        Profile {
            opt_level: opt_level.clone().unwrap_or(TomlOptLevel(profile.opt_level)).0,
            lto: lto.unwrap_or(profile.lto),
            codegen_units: codegen_units,
            rustc_args: None,
            rustdoc_args: None,
            debuginfo: debug.unwrap_or(profile.debuginfo),
            debug_assertions: debug_assertions.unwrap_or(profile.debug_assertions),
            rpath: rpath.unwrap_or(profile.rpath),
            test: profile.test,
            doc: profile.doc,
            run_custom_build: profile.run_custom_build,
            panic: panic.clone().or(profile.panic),
        }
    }
}
