use std::collections::HashMap;
use std::default::Default;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str;

use toml;
use semver;
use rustc_serialize::{Decodable, Decoder};

use core::{SourceId, Profiles};
use core::{Summary, Manifest, Target, Dependency, PackageId, GitReference};
use core::dependency::Kind;
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
                    !s.starts_with(".")
                }).unwrap_or(true)
            }))
        }
        Err(_) => {/* just don't add anything if the directory doesn't exist, etc. */}
    }
}

/// Returns a new `Layout` for a given root path.
/// The `root_path` represents the directory that contains the `Cargo.toml` file.

pub fn project_layout(root_path: &Path) -> Layout {
    let mut lib = None;
    let mut bins = vec!();
    let mut examples = vec!();
    let mut tests = vec!();
    let mut benches = vec!();

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

pub fn to_manifest(contents: &[u8],
                   source_id: &SourceId,
                   layout: Layout,
                   config: &Config)
                   -> CargoResult<(Manifest, Vec<PathBuf>)> {
    let manifest = layout.root.join("Cargo.toml");
    let manifest = match util::without_prefix(&manifest, config.cwd()) {
        Some(path) => path.to_path_buf(),
        None => manifest.clone(),
    };
    let contents = try!(str::from_utf8(contents).map_err(|_| {
        human(format!("{} is not valid UTF-8", manifest.display()))
    }));
    let root = try!(parse(contents, &manifest));
    let mut d = toml::Decoder::new(toml::Value::Table(root));
    let manifest: TomlManifest = try!(Decodable::decode(&mut d).map_err(|e| {
        human(e.to_string())
    }));

    let pair = try!(manifest.to_manifest(source_id, &layout, config));
    let (mut manifest, paths) = pair;
    match d.toml {
        Some(ref toml) => add_unused_keys(&mut manifest, toml, "".to_string()),
        None => {}
    }
    if !manifest.targets().iter().any(|t| !t.is_custom_build()) {
        return Err(human(format!("no targets specified in the manifest\n  either \
                                  src/lib.rs, src/main.rs, a [lib] section, or [[bin]] \
                                  section must be present")))
    }
    return Ok((manifest, paths));

    fn add_unused_keys(m: &mut Manifest, toml: &toml::Value, key: String) {
        match *toml {
            toml::Value::Table(ref table) => {
                for (k, v) in table.iter() {
                    add_unused_keys(m, v, if key.len() == 0 {
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

pub fn parse(toml: &str, file: &Path) -> CargoResult<toml::Table> {
    let mut parser = toml::Parser::new(&toml);
    match parser.parse() {
        Some(toml) => return Ok(toml),
        None => {}
    }
    let mut error_str = format!("could not parse input as TOML\n");
    for error in parser.errors.iter() {
        let (loline, locol) = parser.to_linecol(error.lo);
        let (hiline, hicol) = parser.to_linecol(error.hi);
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

/*
 * TODO: Make all struct fields private
 */

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
}

#[derive(RustcDecodable, Clone, Default)]
pub struct TomlProfiles {
    test: Option<TomlProfile>,
    doc: Option<TomlProfile>,
    bench: Option<TomlProfile>,
    dev: Option<TomlProfile>,
    release: Option<TomlProfile>,
}

#[derive(RustcDecodable, Clone, Default)]
pub struct TomlProfile {
    opt_level: Option<u32>,
    lto: Option<bool>,
    codegen_units: Option<u32>,
    debug: Option<bool>,
    debug_assertions: Option<bool>,
    rpath: Option<bool>,
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
            name: name.to_string(),
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
                name: name,
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
                name: name.to_string(),
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
                name: name.to_string(),
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
                name: name.to_string(),
                path: Some(PathValue::Path(ex.clone())),
                .. TomlTarget::new()
            }
        })
    }).collect()
}

impl TomlManifest {
    pub fn to_manifest(&self, source_id: &SourceId, layout: &Layout,
                       config: &Config)
        -> CargoResult<(Manifest, Vec<PathBuf>)> {
        let mut nested_paths = vec!();

        let project = self.project.as_ref().or_else(|| self.package.as_ref());
        let project = try!(project.chain_error(|| {
            human("No `package` or `project` section found.")
        }));

        if project.name.trim().is_empty() {
            return Err(human("package name cannot be an empty string."))
        }

        let pkgid = try!(project.to_package_id(source_id));
        let metadata = pkgid.generate_metadata();

        // If we have no lib at all, use the inferred lib if available
        // If we have a lib with a path, we're done
        // If we have a lib with no path, use the inferred lib or_else package name

        let lib = match self.lib {
            Some(ref lib) => {
                try!(validate_library_name(lib));
                Some(if layout.lib.is_some() && lib.path.is_none() {
                    TomlTarget {
                        path: layout.lib.as_ref().map(|p| PathValue::Path(p.clone())),
                        .. lib.clone()
                    }
                } else {
                    lib.clone()
                })
            }
            None => inferred_lib_target(&project.name, layout),
        };

        let bins = match self.bin {
            Some(ref bins) => {
                let bin = layout.main();

                for target in bins {
                    try!(validate_binary_name(target));
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
            if blacklist.iter().find(|&x| *x == bin.name) != None {
                return Err(human(&format!("the binary target name `{}` is forbidden", 
                                          bin.name)));
            }
        }

        let examples = match self.example {
            Some(ref examples) => examples.clone(),
            None => inferred_example_targets(layout),
        };

        let tests = match self.test {
            Some(ref tests) => tests.clone(),
            None => inferred_test_targets(layout),
        };

        let benches = if self.bench.is_none() || self.bench.as_ref().unwrap().is_empty() {
            inferred_bench_targets(layout)
        } else {
            self.bench.as_ref().unwrap().iter().map(|t| {
                t.clone()
            }).collect()
        };

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

        let mut deps = Vec::new();

        {

            let mut cx = Context {
                deps: &mut deps,
                source_id: source_id,
                nested_paths: &mut nested_paths,
                config: config,
            };

            // Collect the deps
            try!(process_dependencies(&mut cx, self.dependencies.as_ref(),
                                      |dep| dep));
            try!(process_dependencies(&mut cx, self.dev_dependencies.as_ref(),
                                      |dep| dep.set_kind(Kind::Development)));
            try!(process_dependencies(&mut cx, self.build_dependencies.as_ref(),
                                      |dep| dep.set_kind(Kind::Build)));

            if let Some(targets) = self.target.as_ref() {
                for (name, platform) in targets.iter() {
                    try!(process_dependencies(&mut cx,
                                              platform.dependencies.as_ref(),
                                              |dep| {
                        dep.set_only_for_platform(Some(name.clone()))
                    }));
                    try!(process_dependencies(&mut cx,
                                              platform.build_dependencies.as_ref(),
                                              |dep| {
                        dep.set_only_for_platform(Some(name.clone()))
                           .set_kind(Kind::Build)
                    }));
                    try!(process_dependencies(&mut cx,
                                              platform.dev_dependencies.as_ref(),
                                              |dep| {
                        dep.set_only_for_platform(Some(name.clone()))
                           .set_kind(Kind::Development)
                    }));
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
        let profiles = build_profiles(&self.profile);
        let mut manifest = Manifest::new(summary,
                                         targets,
                                         layout.root.join("target"),
                                         layout.root.join("doc"),
                                         exclude,
                                         include,
                                         project.links.clone(),
                                         metadata,
                                         profiles);
        if project.license_file.is_some() && project.license.is_some() {
            manifest.add_warning(format!("warning: only one of `license` or \
                                                   `license-file` is necessary"));
        }

        Ok((manifest, nested_paths))
    }
}

fn validate_library_name(target: &TomlTarget) -> CargoResult<()> {
    if target.name.trim().is_empty() {
        Err(human(format!("library target names cannot be empty.")))
    } else if target.name.contains("-") {
        Err(human(format!("library target names cannot contain hyphens: {}",
                          target.name)))
    } else {
        Ok(())
    }
}

fn validate_binary_name(target: &TomlTarget) -> CargoResult<()> {
    if target.name.trim().is_empty() {
        Err(human(format!("binary target names cannot be empty.")))
    } else {
        Ok(())
    }
}

fn process_dependencies<F>(cx: &mut Context,
                           new_deps: Option<&HashMap<String, TomlDependency>>,
                           mut f: F) -> CargoResult<()>
    where F: FnMut(Dependency) -> Dependency
{
    let dependencies = match new_deps {
        Some(ref dependencies) => dependencies,
        None => return Ok(())
    };
    for (n, v) in dependencies.iter() {
        let details = match *v {
            TomlDependency::Simple(ref version) => {
                let mut d: DetailedTomlDependency = Default::default();
                d.version = Some(version.clone());
                d
            }
            TomlDependency::Detailed(ref details) => details.clone(),
        };
        let reference = details.branch.clone().map(GitReference::Branch)
            .or_else(|| details.tag.clone().map(GitReference::Tag))
            .or_else(|| details.rev.clone().map(GitReference::Rev))
            .unwrap_or_else(|| GitReference::Branch("master".to_string()));

        let new_source_id = match details.git {
            Some(ref git) => {
                let loc = try!(git.to_url().map_err(|e| {
                    human(e)
                }));
                Some(SourceId::for_git(&loc, reference))
            }
            None => {
                details.path.as_ref().map(|path| {
                    cx.nested_paths.push(PathBuf::from(path));
                    cx.source_id.clone()
                })
            }
        }.unwrap_or(try!(SourceId::for_central(cx.config)));

        let dep = try!(Dependency::parse(&n,
                                         details.version.as_ref()
                                                .map(|v| &v[..]),
                                         &new_source_id));
        let dep = f(dep)
                     .set_features(details.features.unwrap_or(Vec::new()))
                     .set_default_features(details.default_features.unwrap_or(true))
                     .set_optional(details.optional.unwrap_or(false));
        cx.deps.push(dep);
    }

    Ok(())
}

#[derive(RustcDecodable, Debug, Clone)]
struct TomlTarget {
    name: String,
    crate_type: Option<Vec<String>>,
    path: Option<PathValue>,
    test: Option<bool>,
    doctest: Option<bool>,
    bench: Option<bool>,
    doc: Option<bool>,
    plugin: Option<bool>,
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
            name: String::new(),
            crate_type: None,
            path: None,
            test: None,
            doctest: None,
            bench: None,
            doc: None,
            plugin: None,
            harness: None,
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
              .set_for_host(toml.plugin.unwrap_or(t2.for_host()));
    }

    fn lib_target(dst: &mut Vec<Target>, l: &TomlLibTarget,
                  metadata: &Metadata) {
        let path = l.path.clone().unwrap_or_else(|| {
            PathValue::Path(Path::new("src").join(&format!("{}.rs", l.name)))
        });
        let crate_types = l.crate_type.clone().and_then(|kinds| {
            kinds.iter().map(|s| LibKind::from_str(s))
                 .collect::<CargoResult<_>>().ok()
        }).unwrap_or_else(|| {
            vec![if l.plugin == Some(true) {LibKind::Dylib} else {LibKind::Lib}]
        });

        let mut target = Target::lib_target(&l.name, crate_types.clone(),
                                            &path.to_path(),
                                            metadata.clone());
        configure(l, &mut target);
        dst.push(target);
    }

    fn bin_targets(dst: &mut Vec<Target>, bins: &[TomlBinTarget],
                   default: &mut FnMut(&TomlBinTarget) -> PathBuf) {
        for bin in bins.iter() {
            let path = bin.path.clone().unwrap_or_else(|| {
                PathValue::Path(default(bin))
            });
            let mut target = Target::bin_target(&bin.name, &path.to_path(),
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

            let mut target = Target::example_target(&ex.name, &path.to_path());
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
            metadata.mix(&format!("test-{}", test.name));

            let mut target = Target::test_target(&test.name, &path.to_path(),
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
            metadata.mix(&format!("bench-{}", bench.name));

            let mut target = Target::bench_target(&bench.name,
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
                                   .join(&format!("{}.rs", bin.name)));
    } else if bins.len() > 0 {
        bin_targets(&mut ret, bins,
                    &mut |bin| Path::new("src")
                                    .join(&format!("{}.rs", bin.name)));
    }

    if let Some(custom_build) = custom_build {
        custom_build_target(&mut ret, &custom_build);
    }

    example_targets(&mut ret, examples,
                    &mut |ex| Path::new("examples")
                                   .join(&format!("{}.rs", ex.name)));

    test_targets(&mut ret, tests, metadata, &mut |test| {
        if test.name == "test" {
            Path::new("src").join("test.rs")
        } else {
            Path::new("tests").join(&format!("{}.rs", test.name))
        }
    });

    bench_targets(&mut ret, benches, metadata, &mut |bench| {
        if bench.name == "bench" {
            Path::new("src").join("bench.rs")
        } else {
            Path::new("benches").join(&format!("{}.rs", bench.name))
        }
    });

    ret
}

fn build_profiles(profiles: &Option<TomlProfiles>) -> Profiles {
    let profiles = profiles.as_ref();
    return Profiles {
        release: merge(Profile::default_release(),
                       profiles.and_then(|p| p.release.as_ref())),
        dev: merge(Profile::default_dev(),
                   profiles.and_then(|p| p.dev.as_ref())),
        test: merge(Profile::default_test(),
                    profiles.and_then(|p| p.test.as_ref())),
        bench: merge(Profile::default_bench(),
                     profiles.and_then(|p| p.bench.as_ref())),
        doc: merge(Profile::default_doc(),
                   profiles.and_then(|p| p.doc.as_ref())),
    };

    fn merge(profile: Profile, toml: Option<&TomlProfile>) -> Profile {
        let &TomlProfile {
            opt_level, lto, codegen_units, debug, debug_assertions, rpath
        } = match toml {
            Some(toml) => toml,
            None => return profile,
        };
        Profile {
            opt_level: opt_level.unwrap_or(profile.opt_level),
            lto: lto.unwrap_or(profile.lto),
            codegen_units: codegen_units,
            rustc_args: None,
            debuginfo: debug.unwrap_or(profile.debuginfo),
            debug_assertions: debug_assertions.unwrap_or(profile.debug_assertions),
            rpath: rpath.unwrap_or(profile.rpath),
            test: profile.test,
            doc: profile.doc,
        }
    }
}
