use crate::git::repo;
use crate::paths;
use cargo_util::{registry::make_dep_path, Sha256};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::thread;
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

/// A builder for initializing registries.
pub struct RegistryBuilder {
    /// If `true`, adds source replacement for crates.io to a registry on the filesystem.
    replace_crates_io: bool,
    /// If `true`, configures a registry named "alternative".
    alternative: bool,
    /// If set, sets the API url for the "alternative" registry.
    /// This defaults to a directory on the filesystem.
    alt_api_url: Option<String>,
    /// If `true`, configures `.cargo/credentials` with some tokens.
    add_tokens: bool,
}

impl RegistryBuilder {
    pub fn new() -> RegistryBuilder {
        RegistryBuilder {
            replace_crates_io: true,
            alternative: false,
            alt_api_url: None,
            add_tokens: true,
        }
    }

    /// Sets whether or not to replace crates.io with a registry on the filesystem.
    /// Default is `true`.
    pub fn replace_crates_io(&mut self, replace: bool) -> &mut Self {
        self.replace_crates_io = replace;
        self
    }

    /// Sets whether or not to initialize an alternative registry named "alternative".
    /// Default is `false`.
    pub fn alternative(&mut self, alt: bool) -> &mut Self {
        self.alternative = alt;
        self
    }

    /// Sets the API url for the "alternative" registry.
    /// Defaults to a path on the filesystem ([`alt_api_path`]).
    pub fn alternative_api_url(&mut self, url: &str) -> &mut Self {
        self.alternative = true;
        self.alt_api_url = Some(url.to_string());
        self
    }

    /// Sets whether or not to initialize `.cargo/credentials` with some tokens.
    /// Defaults to `true`.
    pub fn add_tokens(&mut self, add: bool) -> &mut Self {
        self.add_tokens = add;
        self
    }

    /// Initializes the registries.
    pub fn build(&self) {
        let config_path = paths::home().join(".cargo/config");
        if config_path.exists() {
            panic!(
                "{} already exists, the registry may only be initialized once, \
                and must be done before the config file is created",
                config_path.display()
            );
        }
        t!(fs::create_dir_all(config_path.parent().unwrap()));
        let mut config = String::new();
        if self.replace_crates_io {
            write!(
                &mut config,
                "
                    [source.crates-io]
                    replace-with = 'dummy-registry'

                    [source.dummy-registry]
                    registry = '{}'
                ",
                registry_url()
            )
            .unwrap();
        }
        if self.alternative {
            write!(
                config,
                "
                    [registries.alternative]
                    index = '{}'
                ",
                alt_registry_url()
            )
            .unwrap();
        }
        t!(fs::write(&config_path, config));

        if self.add_tokens {
            let credentials = paths::home().join(".cargo/credentials");
            t!(fs::write(
                &credentials,
                r#"
                    [registry]
                    token = "api-token"

                    [registries.alternative]
                    token = "api-token"
                "#
            ));
        }

        if self.replace_crates_io {
            init_registry(registry_path(), dl_url().into(), api_url(), api_path());
        }

        if self.alternative {
            init_registry(
                alt_registry_path(),
                alt_dl_url(),
                self.alt_api_url
                    .as_ref()
                    .map_or_else(alt_api_url, |url| Url::parse(url).expect("valid url")),
                alt_api_path(),
            );
        }
    }

    /// Initializes the registries, and sets up an HTTP server for the
    /// "alternative" registry.
    ///
    /// The given callback takes a `Vec` of headers when a request comes in.
    /// The first entry should be the HTTP command, such as
    /// `PUT /api/v1/crates/new HTTP/1.1`.
    ///
    /// The callback should return the HTTP code for the response, and the
    /// response body.
    ///
    /// This method returns a `JoinHandle` which you should call
    /// `.join().unwrap()` on before exiting the test.
    pub fn build_api_server<'a>(
        &mut self,
        handler: &'static (dyn (Fn(Vec<String>) -> (u32, &'a dyn AsRef<[u8]>)) + Sync),
    ) -> thread::JoinHandle<()> {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let api_url = format!("http://{}", addr);

        self.replace_crates_io(false)
            .alternative_api_url(&api_url)
            .build();

        let t = thread::spawn(move || {
            let mut conn = BufReader::new(server.accept().unwrap().0);
            let headers: Vec<_> = (&mut conn)
                .lines()
                .map(|s| s.unwrap())
                .take_while(|s| s.len() > 2)
                .map(|s| s.trim().to_string())
                .collect();
            let (code, response) = handler(headers);
            let response = response.as_ref();
            let stream = conn.get_mut();
            write!(
                stream,
                "HTTP/1.1 {}\r\n\
                  Content-Length: {}\r\n\
                  \r\n",
                code,
                response.len()
            )
            .unwrap();
            stream.write_all(response).unwrap();
        });

        t
    }
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
    files: Vec<PackageFile>,
    yanked: bool,
    features: FeatureMap,
    local: bool,
    alternative: bool,
    invalid_json: bool,
    proc_macro: bool,
    links: Option<String>,
    rust_version: Option<String>,
    cargo_features: Vec<String>,
    v: Option<u32>,
}

type FeatureMap = BTreeMap<String, Vec<String>>;

#[derive(Clone)]
pub struct Dependency {
    name: String,
    vers: String,
    kind: String,
    artifact: Option<(String, Option<String>)>,
    target: Option<String>,
    features: Vec<String>,
    registry: Option<String>,
    package: Option<String>,
    optional: bool,
}

/// A file to be created in a package.
struct PackageFile {
    path: String,
    contents: String,
    /// The Unix mode for the file. Note that when extracted on Windows, this
    /// is mostly ignored since it doesn't have the same style of permissions.
    mode: u32,
    /// If `true`, the file is created in the root of the tarfile, used for
    /// testing invalid packages.
    extra: bool,
}

const DEFAULT_MODE: u32 = 0o644;

/// Initializes the on-disk registry and sets up the config so that crates.io
/// is replaced with the one on disk.
pub fn init() {
    let config = paths::home().join(".cargo/config");
    if config.exists() {
        return;
    }
    RegistryBuilder::new().build();
}

/// Variant of `init` that initializes the "alternative" registry.
pub fn alt_init() {
    RegistryBuilder::new().alternative(true).build();
}

/// Creates a new on-disk registry.
pub fn init_registry(registry_path: PathBuf, dl_url: String, api_url: Url, api_path: PathBuf) {
    // Initialize a new registry.
    repo(&registry_path)
        .file(
            "config.json",
            &format!(r#"{{"dl":"{}","api":"{}"}}"#, dl_url, api_url),
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
            yanked: false,
            features: BTreeMap::new(),
            local: false,
            alternative: false,
            invalid_json: false,
            proc_macro: false,
            links: None,
            rust_version: None,
            cargo_features: Vec::new(),
            v: None,
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
        self.file_with_mode(name, DEFAULT_MODE, contents)
    }

    /// Adds a file with a specific Unix mode.
    pub fn file_with_mode(&mut self, path: &str, mode: u32, contents: &str) -> &mut Package {
        self.files.push(PackageFile {
            path: path.to_string(),
            contents: contents.to_string(),
            mode,
            extra: false,
        });
        self
    }

    /// Adds an "extra" file that is not rooted within the package.
    ///
    /// Normal files are automatically placed within a directory named
    /// `$PACKAGE-$VERSION`. This allows you to override that behavior,
    /// typically for testing invalid behavior.
    pub fn extra_file(&mut self, path: &str, contents: &str) -> &mut Package {
        self.files.push(PackageFile {
            path: path.to_string(),
            contents: contents.to_string(),
            mode: DEFAULT_MODE,
            extra: true,
        });
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

    /// Specify a minimal Rust version.
    pub fn rust_version(&mut self, rust_version: &str) -> &mut Package {
        self.rust_version = Some(rust_version.into());
        self
    }

    /// Causes the JSON line emitted in the index to be invalid, presumably
    /// causing Cargo to skip over this version.
    pub fn invalid_json(&mut self, invalid: bool) -> &mut Package {
        self.invalid_json = invalid;
        self
    }

    pub fn links(&mut self, links: &str) -> &mut Package {
        self.links = Some(links.to_string());
        self
    }

    pub fn cargo_feature(&mut self, feature: &str) -> &mut Package {
        self.cargo_features.push(feature.to_owned());
        self
    }

    /// Sets the index schema version for this package.
    ///
    /// See `cargo::sources::registry::RegistryPackage` for more information.
    pub fn schema_version(&mut self, version: u32) -> &mut Package {
        self.v = Some(version);
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
                    (true, None) => {
                        Some("https://github.com/rust-lang/crates.io-index".to_string())
                    }
                    (true, Some("alternative")) => None,
                    _ => panic!("registry_dep currently only supports `alternative`"),
                };
                serde_json::json!({
                    "name": dep.name,
                    "req": dep.vers,
                    "features": dep.features,
                    "default_features": true,
                    "target": dep.target,
                    "artifact": dep.artifact,
                    "optional": dep.optional,
                    "kind": dep.kind,
                    "registry": registry_url,
                    "package": dep.package,
                })
            })
            .collect::<Vec<_>>();
        let cksum = {
            let c = t!(fs::read(&self.archive_dst()));
            cksum(&c)
        };
        let name = if self.invalid_json {
            serde_json::json!(1)
        } else {
            serde_json::json!(self.name)
        };
        // This emulates what crates.io may do in the future.
        let (features, features2) = split_index_features(self.features.clone());
        let mut json = serde_json::json!({
            "name": name,
            "vers": self.vers,
            "deps": deps,
            "cksum": cksum,
            "features": features,
            "yanked": self.yanked,
            "links": self.links,
        });
        if let Some(f2) = &features2 {
            json["features2"] = serde_json::json!(f2);
            json["v"] = serde_json::json!(2);
        }
        if let Some(v) = self.v {
            json["v"] = serde_json::json!(v);
        }
        let line = json.to_string();

        let file = make_dep_path(&self.name, false);

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
        let prev = fs::read_to_string(&dst).unwrap_or_default();
        t!(fs::create_dir_all(dst.parent().unwrap()));
        t!(fs::write(&dst, prev + &line[..] + "\n"));

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
        let dst = self.archive_dst();
        t!(fs::create_dir_all(dst.parent().unwrap()));
        let f = t!(File::create(&dst));
        let mut a = Builder::new(GzEncoder::new(f, Compression::default()));

        if !self
            .files
            .iter()
            .any(|PackageFile { path, .. }| path == "Cargo.toml")
        {
            self.append_manifest(&mut a);
        }
        if self.files.is_empty() {
            self.append(&mut a, "src/lib.rs", DEFAULT_MODE, "");
        } else {
            for PackageFile {
                path,
                contents,
                mode,
                extra,
            } in &self.files
            {
                if *extra {
                    self.append_raw(&mut a, path, *mode, contents);
                } else {
                    self.append(&mut a, path, *mode, contents);
                }
            }
        }
    }

    fn append_manifest<W: Write>(&self, ar: &mut Builder<W>) {
        let mut manifest = String::new();

        if !self.cargo_features.is_empty() {
            manifest.push_str(&format!(
                "cargo-features = {}\n\n",
                toml_edit::ser::to_item(&self.cargo_features).unwrap()
            ));
        }

        manifest.push_str(&format!(
            r#"
            [package]
            name = "{}"
            version = "{}"
            authors = []
        "#,
            self.name, self.vers
        ));

        if let Some(version) = &self.rust_version {
            manifest.push_str(&format!("rust-version = \"{}\"", version));
        }

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
            if let Some((artifact, target)) = &dep.artifact {
                manifest.push_str(&format!("artifact = \"{}\"\n", artifact));
                if let Some(target) = &target {
                    manifest.push_str(&format!("target = \"{}\"\n", target))
                }
            }
            if let Some(registry) = &dep.registry {
                assert_eq!(registry, "alternative");
                manifest.push_str(&format!("registry-index = \"{}\"", alt_registry_url()));
            }
        }
        if self.proc_macro {
            manifest.push_str("[lib]\nproc-macro = true\n");
        }

        self.append(ar, "Cargo.toml", DEFAULT_MODE, &manifest);
    }

    fn append<W: Write>(&self, ar: &mut Builder<W>, file: &str, mode: u32, contents: &str) {
        self.append_raw(
            ar,
            &format!("{}-{}/{}", self.name, self.vers, file),
            mode,
            contents,
        );
    }

    fn append_raw<W: Write>(&self, ar: &mut Builder<W>, path: &str, mode: u32, contents: &str) {
        let mut header = Header::new_ustar();
        header.set_size(contents.len() as u64);
        t!(header.set_path(path));
        header.set_mode(mode);
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
            artifact: None,
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

    /// Change the artifact to be of the given kind, like "bin", or "staticlib",
    /// along with a specific target triple if provided.
    pub fn artifact(&mut self, kind: &str, target: Option<String>) -> &mut Self {
        self.artifact = Some((kind.to_string(), target));
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

fn split_index_features(mut features: FeatureMap) -> (FeatureMap, Option<FeatureMap>) {
    let mut features2 = FeatureMap::new();
    for (feat, values) in features.iter_mut() {
        if values
            .iter()
            .any(|value| value.starts_with("dep:") || value.contains("?/"))
        {
            let new_values = values.drain(..).collect();
            features2.insert(feat.clone(), new_values);
        }
    }
    if features2.is_empty() {
        (features, None)
    } else {
        (features, Some(features2))
    }
}
