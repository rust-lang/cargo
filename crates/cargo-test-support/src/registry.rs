use crate::git::repo;
use crate::paths;
use cargo::sources::CRATES_IO_INDEX;
use cargo::util::Sha256;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use tar::{Builder, Header};
use url::Url;

/// Gets the path to the local index pretending to be crates.io. This is a Git repo
/// initialized with a `config.json` file pointing to `dl_path` for downloads
/// and `api_path` for uploads.
pub fn registry_path() -> PathBuf {
    generate_path("registry")
}
pub fn registry_url() -> Url {
    generate_url("registry")
}
/// Gets the path for local web API uploads. Cargo will place the contents of a web API
/// request here. For example, `api/v1/crates/new` is the result of publishing a crate.
pub fn api_path() -> PathBuf {
    generate_path("api")
}
pub fn api_url() -> Url {
    generate_url("api")
}
/// Gets the path where crates can be downloaded using the web API endpoint. Crates
/// should be organized as `{name}/{version}/download` to match the web API
/// endpoint. This is rarely used and must be manually set up.
pub fn dl_path() -> PathBuf {
    generate_path("dl")
}
pub fn dl_url() -> Url {
    generate_url("dl")
}
/// Gets the alternative-registry version of `registry_path`.
pub fn alt_registry_path() -> PathBuf {
    generate_path("alternative-registry")
}
pub fn alt_registry_url() -> Url {
    generate_url("alternative-registry")
}
/// Gets the alternative-registry version of `dl_path`.
pub fn alt_dl_path() -> PathBuf {
    generate_path("alt_dl")
}
pub fn alt_dl_url() -> String {
    generate_alt_dl_url("alt_dl")
}
/// Gets the alternative-registry version of `api_path`.
pub fn alt_api_path() -> PathBuf {
    generate_path("alt_api")
}
pub fn alt_api_url() -> Url {
    generate_url("alt_api")
}

pub fn generate_path(name: &str) -> PathBuf {
    paths::root().join(name)
}
pub fn generate_url(name: &str) -> Url {
    Url::from_file_path(generate_path(name)).ok().unwrap()
}
pub fn generate_alt_dl_url(name: &str) -> String {
    let base = Url::from_file_path(generate_path(name)).ok().unwrap();
    format!("{}/{{crate}}/{{version}}/{{crate}}-{{version}}.crate", base)
}

/// A builder for creating a new package in a registry.
///
/// This uses "source replacement" using an automatically generated
/// `.cargo/config` file to ensure that dependencies will use these packages
/// instead of contacting crates.io. See `source-replacement.md` for more
/// details on how source replacement works.
///
/// Call `publish` to finalize and create the package.
///
/// If no files are specified, an empty `lib.rs` file is automatically created.
///
/// The `Cargo.toml` file is automatically generated based on the methods
/// called on `Package` (for example, calling `dep()` will add to the
/// `[dependencies]` automatically). You may also specify a `Cargo.toml` file
/// to override the generated one.
///
/// This supports different registry types:
/// - Regular source replacement that replaces `crates.io` (the default).
/// - A "local registry" which is a subset for vendoring (see
///   `Package::local`).
/// - An "alternative registry" which requires specifying the registry name
///   (see `Package::alternative`).
///
/// This does not support "directory sources". See `directory.rs` for
/// `VendorPackage` which implements directory sources.
///
/// # Example
/// ```
/// // Publish package "a" depending on "b".
/// Package::new("a", "1.0.0")
///     .dep("b", "1.0.0")
///     .file("src/lib.rs", r#"
///         extern crate b;
///         pub fn f() -> i32 { b::f() * 2 }
///     "#)
///     .publish();
///
/// // Publish package "b".
/// Package::new("b", "1.0.0")
///     .file("src/lib.rs", r#"
///         pub fn f() -> i32 { 12 }
///     "#)
///     .publish();
///
/// // Create a project that uses package "a".
/// let p = project()
///     .file("Cargo.toml", r#"
///         [package]
///         name = "foo"
///         version = "0.0.1"
///
///         [dependencies]
///         a = "1.0"
///     "#)
///     .file("src/main.rs", r#"
///         extern crate a;
///         fn main() { println!("{}", a::f()); }
///     "#)
///     .build();
///
/// p.cargo("run").with_stdout("24").run();
/// ```
#[must_use]
pub struct Package {
    name: String,
    vers: String,
    deps: Vec<Dependency>,
    files: Vec<(String, String)>,
    extra_files: Vec<(String, String)>,
    yanked: bool,
    features: HashMap<String, Vec<String>>,
    local: bool,
    alternative: bool,
    invalid_json: bool,
    proc_macro: bool,
}

#[derive(Clone)]
pub struct Dependency {
    name: String,
    vers: String,
    kind: String,
    target: Option<String>,
    features: Vec<String>,
    registry: Option<String>,
    package: Option<String>,
    optional: bool,
}

pub fn init() {
    let config = paths::home().join(".cargo/config");
    t!(fs::create_dir_all(config.parent().unwrap()));
    if fs::metadata(&config).is_ok() {
        return;
    }
    t!(t!(File::create(&config)).write_all(
        format!(
            r#"
        [source.crates-io]
        registry = 'https://wut'
        replace-with = 'dummy-registry'

        [source.dummy-registry]
        registry = '{reg}'

        [registries.alternative]
        index = '{alt}'
    "#,
            reg = registry_url(),
            alt = alt_registry_url()
        )
        .as_bytes()
    ));
    let credentials = paths::home().join(".cargo/credentials");
    t!(t!(File::create(&credentials)).write_all(
        br#"
        [registry]
        token = "api-token"

        [registries.alternative]
        token = "api-token"
    "#
    ));

    // Initialize a new registry.
    init_registry(
        registry_path(),
        dl_url().into_string(),
        api_url(),
        api_path(),
    );

    // Initialize an alternative registry.
    init_registry(
        alt_registry_path(),
        alt_dl_url(),
        alt_api_url(),
        alt_api_path(),
    );
}

pub fn init_registry(registry_path: PathBuf, dl_url: String, api_url: Url, api_path: PathBuf) {
    // Initialize a new registry.
    repo(&registry_path)
        .file(
            "config.json",
            &format!(
                r#"
            {{"dl":"{}","api":"{}"}}
        "#,
                dl_url, api_url
            ),
        )
        .build();
    fs::create_dir_all(api_path.join("api/v1/crates")).unwrap();
}

impl Package {
    /// Creates a new package builder.
    /// Call `publish()` to finalize and build the package.
    pub fn new(name: &str, vers: &str) -> Package {
        init();
        Package {
            name: name.to_string(),
            vers: vers.to_string(),
            deps: Vec::new(),
            files: Vec::new(),
            extra_files: Vec::new(),
            yanked: false,
            features: HashMap::new(),
            local: false,
            alternative: false,
            invalid_json: false,
            proc_macro: false,
        }
    }

    /// Call with `true` to publish in a "local registry".
    ///
    /// See `source-replacement.html#local-registry-sources` for more details
    /// on local registries. See `local_registry.rs` for the tests that use
    /// this.
    pub fn local(&mut self, local: bool) -> &mut Package {
        self.local = local;
        self
    }

    /// Call with `true` to publish in an "alternative registry".
    ///
    /// The name of the alternative registry is called "alternative".
    ///
    /// See `src/doc/src/reference/registries.md` for more details on
    /// alternative registries. See `alt_registry.rs` for the tests that use
    /// this.
    pub fn alternative(&mut self, alternative: bool) -> &mut Package {
        self.alternative = alternative;
        self
    }

    /// Adds a file to the package.
    pub fn file(&mut self, name: &str, contents: &str) -> &mut Package {
        self.files.push((name.to_string(), contents.to_string()));
        self
    }

    /// Adds an "extra" file that is not rooted within the package.
    ///
    /// Normal files are automatically placed within a directory named
    /// `$PACKAGE-$VERSION`. This allows you to override that behavior,
    /// typically for testing invalid behavior.
    pub fn extra_file(&mut self, name: &str, contents: &str) -> &mut Package {
        self.extra_files
            .push((name.to_string(), contents.to_string()));
        self
    }

    /// Adds a normal dependency. Example:
    /// ```
    /// [dependencies]
    /// foo = {version = "1.0"}
    /// ```
    pub fn dep(&mut self, name: &str, vers: &str) -> &mut Package {
        self.add_dep(&Dependency::new(name, vers))
    }

    /// Adds a dependency with the given feature. Example:
    /// ```
    /// [dependencies]
    /// foo = {version = "1.0", "features": ["feat1", "feat2"]}
    /// ```
    pub fn feature_dep(&mut self, name: &str, vers: &str, features: &[&str]) -> &mut Package {
        self.add_dep(Dependency::new(name, vers).enable_features(features))
    }

    /// Adds a platform-specific dependency. Example:
    /// ```
    /// [target.'cfg(windows)'.dependencies]
    /// foo = {version = "1.0"}
    /// ```
    pub fn target_dep(&mut self, name: &str, vers: &str, target: &str) -> &mut Package {
        self.add_dep(Dependency::new(name, vers).target(target))
    }

    /// Adds a dependency to the alternative registry.
    pub fn registry_dep(&mut self, name: &str, vers: &str) -> &mut Package {
        self.add_dep(Dependency::new(name, vers).registry("alternative"))
    }

    /// Adds a dev-dependency. Example:
    /// ```
    /// [dev-dependencies]
    /// foo = {version = "1.0"}
    /// ```
    pub fn dev_dep(&mut self, name: &str, vers: &str) -> &mut Package {
        self.add_dep(Dependency::new(name, vers).dev())
    }

    /// Adds a build-dependency. Example:
    /// ```
    /// [build-dependencies]
    /// foo = {version = "1.0"}
    /// ```
    pub fn build_dep(&mut self, name: &str, vers: &str) -> &mut Package {
        self.add_dep(Dependency::new(name, vers).build())
    }

    pub fn add_dep(&mut self, dep: &Dependency) -> &mut Package {
        self.deps.push(dep.clone());
        self
    }

    /// Specifies whether or not the package is "yanked".
    pub fn yanked(&mut self, yanked: bool) -> &mut Package {
        self.yanked = yanked;
        self
    }

    /// Specifies whether or not this is a proc macro.
    pub fn proc_macro(&mut self, proc_macro: bool) -> &mut Package {
        self.proc_macro = proc_macro;
        self
    }

    /// Adds an entry in the `[features]` section.
    pub fn feature(&mut self, name: &str, deps: &[&str]) -> &mut Package {
        let deps = deps.iter().map(|s| s.to_string()).collect();
        self.features.insert(name.to_string(), deps);
        self
    }

    /// Causes the JSON line emitted in the index to be invalid, presumably
    /// causing Cargo to skip over this version.
    pub fn invalid_json(&mut self, invalid: bool) -> &mut Package {
        self.invalid_json = invalid;
        self
    }

    /// Creates the package and place it in the registry.
    ///
    /// This does not actually use Cargo's publishing system, but instead
    /// manually creates the entry in the registry on the filesystem.
    ///
    /// Returns the checksum for the package.
    pub fn publish(&self) -> String {
        self.make_archive();

        // Figure out what we're going to write into the index.
        let deps = self
            .deps
            .iter()
            .map(|dep| {
                // In the index, the `registry` is null if it is from the same registry.
                // In Cargo.toml, it is None if it is from crates.io.
                let registry_url = match (self.alternative, dep.registry.as_deref()) {
                    (false, None) => None,
                    (false, Some("alternative")) => Some(alt_registry_url().to_string()),
                    (true, None) => Some(CRATES_IO_INDEX.to_string()),
                    (true, Some("alternative")) => None,
                    _ => panic!("registry_dep currently only supports `alternative`"),
                };
                serde_json::json!({
                    "name": dep.name,
                    "req": dep.vers,
                    "features": dep.features,
                    "default_features": true,
                    "target": dep.target,
                    "optional": dep.optional,
                    "kind": dep.kind,
                    "registry": registry_url,
                    "package": dep.package,
                })
            })
            .collect::<Vec<_>>();
        let cksum = {
            let mut c = Vec::new();
            t!(t!(File::open(&self.archive_dst())).read_to_end(&mut c));
            cksum(&c)
        };
        let name = if self.invalid_json {
            serde_json::json!(1)
        } else {
            serde_json::json!(self.name)
        };
        let line = serde_json::json!({
            "name": name,
            "vers": self.vers,
            "deps": deps,
            "cksum": cksum,
            "features": self.features,
            "yanked": self.yanked,
        })
        .to_string();

        let file = match self.name.len() {
            1 => format!("1/{}", self.name),
            2 => format!("2/{}", self.name),
            3 => format!("3/{}/{}", &self.name[..1], self.name),
            _ => format!("{}/{}/{}", &self.name[0..2], &self.name[2..4], self.name),
        };

        let registry_path = if self.alternative {
            alt_registry_path()
        } else {
            registry_path()
        };

        // Write file/line in the index.
        let dst = if self.local {
            registry_path.join("index").join(&file)
        } else {
            registry_path.join(&file)
        };
        let mut prev = String::new();
        let _ = File::open(&dst).and_then(|mut f| f.read_to_string(&mut prev));
        t!(fs::create_dir_all(dst.parent().unwrap()));
        t!(t!(File::create(&dst)).write_all((prev + &line[..] + "\n").as_bytes()));

        // Add the new file to the index.
        if !self.local {
            let repo = t!(git2::Repository::open(&registry_path));
            let mut index = t!(repo.index());
            t!(index.add_path(Path::new(&file)));
            t!(index.write());
            let id = t!(index.write_tree());

            // Commit this change.
            let tree = t!(repo.find_tree(id));
            let sig = t!(repo.signature());
            let parent = t!(repo.refname_to_id("refs/heads/master"));
            let parent = t!(repo.find_commit(parent));
            t!(repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Another commit",
                &tree,
                &[&parent]
            ));
        }

        cksum
    }

    fn make_archive(&self) {
        let mut manifest = format!(
            r#"
            [package]
            name = "{}"
            version = "{}"
            authors = []
        "#,
            self.name, self.vers
        );
        for dep in self.deps.iter() {
            let target = match dep.target {
                None => String::new(),
                Some(ref s) => format!("target.'{}'.", s),
            };
            let kind = match &dep.kind[..] {
                "build" => "build-",
                "dev" => "dev-",
                _ => "",
            };
            manifest.push_str(&format!(
                r#"
                [{}{}dependencies.{}]
                version = "{}"
            "#,
                target, kind, dep.name, dep.vers
            ));
            if let Some(registry) = &dep.registry {
                assert_eq!(registry, "alternative");
                manifest.push_str(&format!("registry-index = \"{}\"", alt_registry_url()));
            }
        }
        if self.proc_macro {
            manifest.push_str("[lib]\nproc-macro = true\n");
        }

        let dst = self.archive_dst();
        t!(fs::create_dir_all(dst.parent().unwrap()));
        let f = t!(File::create(&dst));
        let mut a = Builder::new(GzEncoder::new(f, Compression::default()));
        self.append(&mut a, "Cargo.toml", &manifest);
        if self.files.is_empty() {
            self.append(&mut a, "src/lib.rs", "");
        } else {
            for &(ref name, ref contents) in self.files.iter() {
                self.append(&mut a, name, contents);
            }
        }
        for &(ref name, ref contents) in self.extra_files.iter() {
            self.append_extra(&mut a, name, contents);
        }
    }

    fn append<W: Write>(&self, ar: &mut Builder<W>, file: &str, contents: &str) {
        self.append_extra(
            ar,
            &format!("{}-{}/{}", self.name, self.vers, file),
            contents,
        );
    }

    fn append_extra<W: Write>(&self, ar: &mut Builder<W>, path: &str, contents: &str) {
        let mut header = Header::new_ustar();
        header.set_size(contents.len() as u64);
        t!(header.set_path(path));
        header.set_cksum();
        t!(ar.append(&header, contents.as_bytes()));
    }

    /// Returns the path to the compressed package file.
    pub fn archive_dst(&self) -> PathBuf {
        if self.local {
            registry_path().join(format!("{}-{}.crate", self.name, self.vers))
        } else if self.alternative {
            alt_dl_path()
                .join(&self.name)
                .join(&self.vers)
                .join(&format!("{}-{}.crate", self.name, self.vers))
        } else {
            dl_path().join(&self.name).join(&self.vers).join("download")
        }
    }
}

pub fn cksum(s: &[u8]) -> String {
    Sha256::new().update(s).finish_hex()
}

impl Dependency {
    pub fn new(name: &str, vers: &str) -> Dependency {
        Dependency {
            name: name.to_string(),
            vers: vers.to_string(),
            kind: "normal".to_string(),
            target: None,
            features: Vec::new(),
            package: None,
            optional: false,
            registry: None,
        }
    }

    /// Changes this to `[build-dependencies]`.
    pub fn build(&mut self) -> &mut Self {
        self.kind = "build".to_string();
        self
    }

    /// Changes this to `[dev-dependencies]`.
    pub fn dev(&mut self) -> &mut Self {
        self.kind = "dev".to_string();
        self
    }

    /// Changes this to `[target.$target.dependencies]`.
    pub fn target(&mut self, target: &str) -> &mut Self {
        self.target = Some(target.to_string());
        self
    }

    /// Adds `registry = $registry` to this dependency.
    pub fn registry(&mut self, registry: &str) -> &mut Self {
        self.registry = Some(registry.to_string());
        self
    }

    /// Adds `features = [ ... ]` to this dependency.
    pub fn enable_features(&mut self, features: &[&str]) -> &mut Self {
        self.features.extend(features.iter().map(|s| s.to_string()));
        self
    }

    /// Adds `package = ...` to this dependency.
    pub fn package(&mut self, pkg: &str) -> &mut Self {
        self.package = Some(pkg.to_string());
        self
    }

    /// Changes this to an optional dependency.
    pub fn optional(&mut self, optional: bool) -> &mut Self {
        self.optional = optional;
        self
    }
}
