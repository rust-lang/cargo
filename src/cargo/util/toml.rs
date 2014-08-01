use serialize::Decodable;
use std::collections::HashMap;
use std::fmt;
use std::io::fs;
use std::str;
use toml;

use core::{SourceId, GitKind};
use core::manifest::{LibKind, Lib, Dylib, Profile};
use core::{Summary, Manifest, Target, Dependency, PackageId};
use core::package_id::Metadata;
use core::source::Location;
use util::{CargoResult, Require, human};

/// Representation of the projects file layout.
///
/// This structure is used to hold references to all project files that are relevant to cargo.

#[deriving(Clone)]
pub struct Layout {
    root: Path,
    lib: Option<Path>,
    bins: Vec<Path>,
    examples: Vec<Path>,
    tests: Vec<Path>,
}

impl Layout {
    fn main(&self) -> Option<&Path> {
        self.bins.iter().find(|p| {
            match p.filename_str() {
                Some(s) => s == "main.rs",
                None => false
            }
        })
    }
}

fn try_add_file(files: &mut Vec<Path>, root: &Path, dir: &str) {
    let p = root.join(dir);
    if p.exists() {
        files.push(p);
    }
}
fn try_add_files(files: &mut Vec<Path>, root: &Path, dir: &str) {
    match fs::readdir(&root.join(dir)) {
        Ok(new) => {
            files.extend(new.move_iter().filter(|f| f.extension_str() == Some("rs")))
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

    if root_path.join("src/lib.rs").exists() {
        lib = Some(root_path.join("src/lib.rs"));
    }

    try_add_file(&mut bins, root_path, "src/main.rs");
    try_add_files(&mut bins, root_path, "src/bin");

    try_add_files(&mut examples, root_path, "examples");

    try_add_files(&mut tests, root_path, "tests");

    Layout {
        root: root_path.clone(),
        lib: lib,
        bins: bins,
        examples: examples,
        tests: tests,
    }
}

pub fn to_manifest(contents: &[u8],
                   source_id: &SourceId,
                   layout: Layout)
                   -> CargoResult<(Manifest, Vec<Path>)> {
    let contents = try!(str::from_utf8(contents).require(|| {
        human("Cargo.toml is not valid UTF-8")
    }));
    let root = try!(parse(contents, &Path::new("Cargo.toml")));
    let mut d = toml::Decoder::new(toml::Table(root));
    let toml_manifest: TomlManifest = match Decodable::decode(&mut d) {
        Ok(t) => t,
        Err(e) => return Err(human(format!("Cargo.toml is not a valid \
                                            manifest\n\n{}", e)))
    };

    let pair = try!(toml_manifest.to_manifest(source_id, &layout).map_err(|err| {
        human(format!("Cargo.toml is not a valid manifest\n\n{}", err))
    }));
    let (mut manifest, paths) = pair;
    match d.toml {
        Some(ref toml) => add_unused_keys(&mut manifest, toml, "".to_string()),
        None => {}
    }
    if manifest.get_targets().len() == 0 {
        return Err(human(format!("either a [[lib]] or [[bin]] section must \
                                  be present")))
    }
    return Ok((manifest, paths));

    fn add_unused_keys(m: &mut Manifest, toml: &toml::Value, key: String) {
        match *toml {
            toml::Table(ref table) => {
                for (k, v) in table.iter() {
                    add_unused_keys(m, v, if key.len() == 0 {
                        k.clone()
                    } else {
                        key + "." + k.as_slice()
                    })
                }
            }
            toml::Array(ref arr) => {
                for v in arr.iter() {
                    add_unused_keys(m, v, key.clone());
                }
            }
            _ => m.add_unused_key(key),
        }
    }
}

pub fn parse(toml: &str, file: &Path) -> CargoResult<toml::Table> {
    let mut parser = toml::Parser::new(toml.as_slice());
    match parser.parse() {
        Some(toml) => return Ok(toml),
        None => {}
    }
    let mut error_str = format!("could not parse input TOML\n");
    for error in parser.errors.iter() {
        let (loline, locol) = parser.to_linecol(error.lo);
        let (hiline, hicol) = parser.to_linecol(error.hi);
        error_str.push_str(format!("{}:{}:{}{} {}\n",
                                   file.filename_display(),
                                   loline + 1, locol + 1,
                                   if loline != hiline || locol != hicol {
                                       format!("-{}:{}", hiline + 1,
                                               hicol + 1)
                                   } else {
                                       "".to_string()
                                   },
                                   error.desc).as_slice());
    }
    Err(human(error_str))
}

type TomlLibTarget = TomlTarget;
type TomlBinTarget = TomlTarget;
type TomlExampleTarget = TomlTarget;
type TomlTestTarget = TomlTarget;

/*
 * TODO: Make all struct fields private
 */

#[deriving(Encodable,Decodable,PartialEq,Clone,Show)]
pub enum TomlDependency {
    SimpleDep(String),
    DetailedDep(DetailedTomlDependency)
}


#[deriving(Encodable,Decodable,PartialEq,Clone,Show)]
pub struct DetailedTomlDependency {
    version: Option<String>,
    path: Option<String>,
    git: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>
}

#[deriving(Encodable,Decodable,PartialEq,Clone)]
pub struct TomlManifest {
    package: Option<Box<TomlProject>>,
    project: Option<Box<TomlProject>>,
    lib: Option<Vec<TomlLibTarget>>,
    bin: Option<Vec<TomlBinTarget>>,
    example: Option<Vec<TomlExampleTarget>>,
    test: Option<Vec<TomlTestTarget>>,
    dependencies: Option<HashMap<String, TomlDependency>>,
    dev_dependencies: Option<HashMap<String, TomlDependency>>
}

#[deriving(Decodable,Encodable,PartialEq,Clone,Show)]
pub struct TomlProject {
    pub name: String,
    // FIXME #54: should be a Version to be able to be Decodable'd directly.
    pub version: String,
    pub authors: Vec<String>,
    build: Option<TomlBuildCommandsList>,
}

#[deriving(Encodable,Decodable,PartialEq,Clone,Show)]
pub enum TomlBuildCommandsList {
    SingleBuildCommand(String),
    MultipleBuildCommands(Vec<String>)
}

impl TomlProject {
    pub fn to_package_id(&self, source_id: &SourceId) -> CargoResult<PackageId> {
        PackageId::new(self.name.as_slice(), self.version.as_slice(), source_id)
    }
}

struct Context<'a> {
    deps: &'a mut Vec<Dependency>,
    source_id: &'a SourceId,
    source_ids: &'a mut Vec<SourceId>,
    nested_paths: &'a mut Vec<Path>
}

// These functions produce the equivalent of specific manifest entries. One
// wrinkle is that certain paths cannot be represented in the manifest due
// to Toml's UTF-8 requirement. This could, in theory, mean that certain
// otherwise acceptable executable names are not used when inside of
// `src/bin/*`, but it seems ok to not build executables with non-UTF8
// paths.
fn inferred_lib_target(name: &str, layout: &Layout) -> Vec<TomlTarget> {
    layout.lib.as_ref().map(|lib| {
        vec![TomlTarget {
            name: name.to_string(),
            crate_type: None,
            path: Some(TomlPath(lib.clone())),
            test: None,
            plugin: None,
            doc: None,
        }]
    }).unwrap_or(Vec::new())
}

fn inferred_bin_targets(name: &str, layout: &Layout) -> Vec<TomlTarget> {
    layout.bins.iter().filter_map(|bin| {
        let name = if bin.as_vec() == b"src/main.rs" ||
                      *bin == layout.root.join("src/main.rs") {
            Some(name.to_string())
        } else {
            bin.filestem_str().map(|f| f.to_string())
        };

        name.map(|name| {
            TomlTarget {
                name: name,
                crate_type: None,
                path: Some(TomlPath(bin.clone())),
                test: None,
                plugin: None,
                doc: None,
            }
        })
    }).collect()
}

fn inferred_example_targets(layout: &Layout) -> Vec<TomlTarget> {
    layout.examples.iter().filter_map(|ex| {
        ex.filestem_str().map(|name| {
            TomlTarget {
                name: name.to_string(),
                crate_type: None,
                path: Some(TomlPath(ex.clone())),
                test: None,
                plugin: None,
                doc: None,
            }
        })
    }).collect()
}

fn inferred_test_targets(layout: &Layout) -> Vec<TomlTarget> {
    layout.tests.iter().filter_map(|ex| {
        ex.filestem_str().map(|name| {
            TomlTarget {
                name: name.to_string(),
                crate_type: None,
                path: Some(TomlPath(ex.clone())),
                test: None,
                plugin: None,
                doc: None,
            }
        })
    }).collect()
}

impl TomlManifest {
    pub fn to_manifest(&self, source_id: &SourceId, layout: &Layout)
        -> CargoResult<(Manifest, Vec<Path>)> {
        let mut sources = vec!();
        let mut nested_paths = vec!();

        let project = self.project.as_ref().or_else(|| self.package.as_ref());
        let project = try!(project.require(|| {
            human("No `package` or `project` section found.")
        }));

        let pkgid = try!(project.to_package_id(source_id));
        let metadata = pkgid.generate_metadata();

        // If we have no lib at all, use the inferred lib if available
        // If we have a lib with a path, we're done
        // If we have a lib with no path, use the inferred lib or_else package name

        let lib = if self.lib.is_none() || self.lib.get_ref().is_empty() {
            inferred_lib_target(project.name.as_slice(), layout)
        } else {
            self.lib.get_ref().iter().map(|t| {
                if layout.lib.is_some() && t.path.is_none() {
                    TomlTarget {
                        name: t.name.clone(),
                        crate_type: t.crate_type.clone(),
                        path: layout.lib.as_ref().map(|p| TomlPath(p.clone())),
                        test: t.test,
                        plugin: t.plugin,
                        doc: t.doc,
                    }
                } else {
                    t.clone()
                }
            }).collect()
        };

        let bins = if self.bin.is_none() || self.bin.get_ref().is_empty() {
            inferred_bin_targets(project.name.as_slice(), layout)
        } else {
            let bin = layout.main();

            self.bin.get_ref().iter().map(|t| {
                if bin.is_some() && t.path.is_none() {
                    TomlTarget {
                        name: t.name.clone(),
                        crate_type: t.crate_type.clone(),
                        path: bin.as_ref().map(|&p| TomlPath(p.clone())),
                        test: t.test,
                        plugin: None,
                        doc: t.doc,
                    }
                } else {
                    t.clone()
                }
            }).collect()
        };

        let examples = if self.example.is_none() || self.example.get_ref().is_empty() {
            inferred_example_targets(layout)
        } else {
            self.example.get_ref().iter().map(|t| t.clone()).collect()
        };

        let tests = if self.test.is_none() || self.test.get_ref().is_empty() {
            inferred_test_targets(layout)
        } else {
            self.test.get_ref().iter().map(|t| t.clone()).collect()
        };

        // Get targets
        let targets = normalize(lib.as_slice(),
                                bins.as_slice(),
                                examples.as_slice(),
                                tests.as_slice(),
                                &metadata);

        if targets.is_empty() {
            debug!("manifest has no build targets; project={}", self.project);
        }

        let mut deps = Vec::new();

        {

            let mut cx = Context {
                deps: &mut deps,
                source_id: source_id,
                source_ids: &mut sources,
                nested_paths: &mut nested_paths
            };

            // Collect the deps
            try!(process_dependencies(&mut cx, false, self.dependencies.as_ref()));
            try!(process_dependencies(&mut cx, true, self.dev_dependencies.as_ref()));
        }

        let summary = Summary::new(&pkgid, deps.as_slice());
        Ok((Manifest::new(
                &summary,
                targets.as_slice(),
                &Path::new("target"),
                &Path::new("doc"),
                sources,
                match project.build {
                    Some(SingleBuildCommand(ref cmd)) => vec!(cmd.clone()),
                    Some(MultipleBuildCommands(ref cmd)) => cmd.clone(),
                    None => Vec::new()
                }),
           nested_paths))
    }
}

fn process_dependencies<'a>(cx: &mut Context<'a>, dev: bool,
                            new_deps: Option<&HashMap<String, TomlDependency>>)
                            -> CargoResult<()> {
    let dependencies = match new_deps {
        Some(ref dependencies) => dependencies,
        None => return Ok(())
    };
    for (n, v) in dependencies.iter() {
        let (version, source_id) = match *v {
            SimpleDep(ref string) => {
                (Some(string.clone()), SourceId::for_central())
            },
            DetailedDep(ref details) => {
                let reference = details.branch.clone()
                    .or_else(|| details.tag.clone())
                    .or_else(|| details.rev.clone())
                    .unwrap_or_else(|| "master".to_string());

                let new_source_id = match details.git {
                    Some(ref git) => {
                        let kind = GitKind(reference.clone());
                        let loc = try!(Location::parse(git.as_slice()));
                        let source_id = SourceId::new(kind, loc);
                        // TODO: Don't do this for path
                        cx.source_ids.push(source_id.clone());
                        Some(source_id)
                    }
                    None => {
                        details.path.as_ref().map(|path| {
                            cx.nested_paths.push(Path::new(path.as_slice()));
                            cx.source_id.clone()
                        })
                    }
                }.unwrap_or(SourceId::for_central());

                (details.version.clone(), new_source_id)
            }
        };

        let mut dep = try!(Dependency::parse(n.as_slice(),
                       version.as_ref().map(|v| v.as_slice()),
                       &source_id));

        if dev { dep = dep.as_dev() }

        cx.deps.push(dep)
    }

    Ok(())
}

#[deriving(Decodable,Encodable,PartialEq,Clone,Show)]
struct TomlTarget {
    name: String,
    crate_type: Option<Vec<String>>,
    path: Option<TomlPath>,
    test: Option<bool>,
    doc: Option<bool>,
    plugin: Option<bool>,
}

#[deriving(Decodable,Encodable,PartialEq,Clone)]
enum TomlPath {
    TomlString(String),
    TomlPath(Path),
}

impl TomlPath {
    fn to_path(&self) -> Path {
        match *self {
            TomlString(ref s) => Path::new(s.as_slice()),
            TomlPath(ref p) => p.clone(),
        }
    }
}

impl fmt::Show for TomlPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TomlString(ref s) => s.fmt(f),
            TomlPath(ref p) => p.display().fmt(f),
        }
    }
}

fn normalize(libs: &[TomlLibTarget],
             bins: &[TomlBinTarget],
             examples: &[TomlExampleTarget],
             tests: &[TomlTestTarget],
             metadata: &Metadata) -> Vec<Target> {
    log!(4, "normalizing toml targets; lib={}; bin={}; example={}; test={}",
         libs, bins, examples, tests);

    enum TestDep { Needed, NotNeeded }

    fn target_profiles(target: &TomlTarget, dep: TestDep) -> Vec<Profile> {
        let mut ret = vec![Profile::default_dev(), Profile::default_release()];

        match target.test {
            Some(true) | None => ret.push(Profile::default_test()),
            Some(false) => {}
        }

        match target.doc {
            Some(true) | None => ret.push(Profile::default_doc()),
            Some(false) => {}
        }

        match dep {
            Needed => ret.push(Profile::default_test().test(false)),
            _ => {}
        }

        if target.plugin == Some(true) {
            ret = ret.move_iter().map(|p| p.plugin(true)).collect();
        }

        ret
    }

    fn lib_targets(dst: &mut Vec<Target>, libs: &[TomlLibTarget],
                   dep: TestDep, metadata: &Metadata) {
        let l = &libs[0];
        let path = l.path.clone().unwrap_or_else(|| {
            TomlString(format!("src/{}.rs", l.name))
        });
        let crate_types = l.crate_type.clone().and_then(|kinds| {
            LibKind::from_strs(kinds).ok()
        }).unwrap_or_else(|| {
            vec![if l.plugin == Some(true) {Dylib} else {Lib}]
        });

        for profile in target_profiles(l, dep).iter() {
            let mut metadata = metadata.clone();
            // Libs and their tests are built in parallel, so we need to make
            // sure that their metadata is different.
            if profile.is_test() {
                metadata.mix(&"test");
            }
            dst.push(Target::lib_target(l.name.as_slice(), crate_types.clone(),
                                        &path.to_path(), profile,
                                        metadata));
        }
    }

    fn bin_targets(dst: &mut Vec<Target>, bins: &[TomlBinTarget],
                   dep: TestDep, metadata: &Metadata,
                   default: |&TomlBinTarget| -> String) {
        for bin in bins.iter() {
            let path = bin.path.clone().unwrap_or_else(|| {
                TomlString(default(bin))
            });

            for profile in target_profiles(bin, dep).iter() {
                let metadata = if profile.is_test() {
                    // Make sure that the name of this test executable doesn't
                    // conflicts with a library that has the same name and is
                    // being tested
                    let mut metadata = metadata.clone();
                    metadata.mix(&format!("bin-{}", bin.name));
                    Some(metadata)
                } else {
                    None
                };
                dst.push(Target::bin_target(bin.name.as_slice(),
                                            &path.to_path(),
                                            profile,
                                            metadata));
            }
        }
    }

    fn example_targets(dst: &mut Vec<Target>, examples: &[TomlExampleTarget],
                       default: |&TomlExampleTarget| -> String) {
        for ex in examples.iter() {
            let path = ex.path.clone().unwrap_or_else(|| TomlString(default(ex)));

            let profile = &Profile::default_test().test(false);
            dst.push(Target::example_target(ex.name.as_slice(),
                                            &path.to_path(),
                                            profile));
        }
    }

    fn test_targets(dst: &mut Vec<Target>, tests: &[TomlTestTarget],
                    metadata: &Metadata,
                    default: |&TomlTestTarget| -> String) {
        for test in tests.iter() {
            let path = test.path.clone().unwrap_or_else(|| {
                TomlString(default(test))
            });

            // make sure this metadata is different from any same-named libs.
            let mut metadata = metadata.clone();
            metadata.mix(&format!("test-{}", test.name));

            let profile = &Profile::default_test();
            dst.push(Target::test_target(test.name.as_slice(),
                                         &path.to_path(),
                                         profile,
                                         metadata));
        }
    }

    let mut ret = Vec::new();

    let test_dep = if examples.len() > 0 || tests.len() > 0 {
        Needed
    } else {
        NotNeeded
    };

    match (libs, bins) {
        ([_, ..], [_, ..]) => {
            lib_targets(&mut ret, libs, Needed, metadata);
            bin_targets(&mut ret, bins, test_dep, metadata,
                        |bin| format!("src/bin/{}.rs", bin.name));
        },
        ([_, ..], []) => {
            lib_targets(&mut ret, libs, Needed, metadata);
        },
        ([], [_, ..]) => {
            bin_targets(&mut ret, bins, test_dep, metadata,
                        |bin| format!("src/{}.rs", bin.name));
        },
        ([], []) => ()
    }


    example_targets(&mut ret, examples,
                    |ex| format!("examples/{}.rs", ex.name));

    test_targets(&mut ret, tests, metadata,
                |test| {
                    if test.name.as_slice() == "test" {
                        "src/test.rs".to_string()
                    } else {
                        format!("tests/{}.rs", test.name)
                    }});

    ret
}
