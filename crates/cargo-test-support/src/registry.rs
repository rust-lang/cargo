use crate::git::repo;
use crate::paths;
use cargo_util::paths::append;
use cargo_util::{registry::make_dep_path, Sha256};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
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
/// Gets the path for local web API uploads. Cargo will place the contents of a web API
/// request here. For example, `api/v1/crates/new` is the result of publishing a crate.
pub fn api_path() -> PathBuf {
    generate_path("api")
}
/// Gets the path where crates can be downloaded using the web API endpoint. Crates
/// should be organized as `{name}/{version}/download` to match the web API
/// endpoint. This is rarely used and must be manually set up.
fn dl_path() -> PathBuf {
    generate_path("dl")
}
/// Gets the alternative-registry version of `registry_path`.
fn alt_registry_path() -> PathBuf {
    generate_path("alternative-registry")
}
/// Gets the alternative-registry version of `registry_url`.
fn alt_registry_url() -> Url {
    generate_url("alternative-registry")
}
/// Gets the alternative-registry version of `dl_path`.
pub fn alt_dl_path() -> PathBuf {
    generate_path("alternative-dl")
}
/// Gets the alternative-registry version of `api_path`.
pub fn alt_api_path() -> PathBuf {
    generate_path("alternative-api")
}
fn generate_path(name: &str) -> PathBuf {
    paths::root().join(name)
}
fn generate_url(name: &str) -> Url {
    Url::from_file_path(generate_path(name)).ok().unwrap()
}

/// A builder for initializing registries.
pub struct RegistryBuilder {
    /// If set, configures an alternate registry with the given name.
    alternative: Option<String>,
    /// If set, the authorization token for the registry.
    token: Option<String>,
    /// If set, serves the index over http.
    http_index: bool,
    /// If set, serves the API over http.
    http_api: bool,
    /// If set, config.json includes 'api'
    api: bool,
    /// Write the token in the configuration.
    configure_token: bool,
    /// Write the registry in configuration.
    configure_registry: bool,
    /// API responders.
    custom_responders: HashMap<&'static str, Box<dyn Send + Fn(&Request) -> Response>>,
}

pub struct TestRegistry {
    _server: Option<HttpServerHandle>,
    index_url: Url,
    path: PathBuf,
    api_url: Url,
    dl_url: Url,
    token: Option<String>,
}

impl TestRegistry {
    pub fn index_url(&self) -> &Url {
        &self.index_url
    }

    pub fn api_url(&self) -> &Url {
        &self.api_url
    }

    pub fn token(&self) -> &str {
        self.token
            .as_deref()
            .expect("registry was not configured with a token")
    }
}

impl RegistryBuilder {
    #[must_use]
    pub fn new() -> RegistryBuilder {
        RegistryBuilder {
            alternative: None,
            token: Some("api-token".to_string()),
            http_api: false,
            http_index: false,
            api: true,
            configure_registry: true,
            configure_token: true,
            custom_responders: HashMap::new(),
        }
    }

    /// Adds a custom HTTP response for a specific url
    #[must_use]
    pub fn add_responder<R: 'static + Send + Fn(&Request) -> Response>(
        mut self,
        url: &'static str,
        responder: R,
    ) -> Self {
        self.custom_responders.insert(url, Box::new(responder));
        self
    }

    /// Sets whether or not to initialize as an alternative registry.
    #[must_use]
    pub fn alternative_named(mut self, alt: &str) -> Self {
        self.alternative = Some(alt.to_string());
        self
    }

    /// Sets whether or not to initialize as an alternative registry.
    #[must_use]
    pub fn alternative(self) -> Self {
        self.alternative_named("alternative")
    }

    /// Prevents placing a token in the configuration
    #[must_use]
    pub fn no_configure_token(mut self) -> Self {
        self.configure_token = false;
        self
    }

    /// Prevents adding the registry to the configuration.
    #[must_use]
    pub fn no_configure_registry(mut self) -> Self {
        self.configure_registry = false;
        self
    }

    /// Sets the token value
    #[must_use]
    pub fn token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }

    /// Operate the index over http
    #[must_use]
    pub fn http_index(mut self) -> Self {
        self.http_index = true;
        self
    }

    /// Operate the api over http
    #[must_use]
    pub fn http_api(mut self) -> Self {
        self.http_api = true;
        self
    }

    /// The registry has no api.
    #[must_use]
    pub fn no_api(mut self) -> Self {
        self.api = false;
        self
    }

    /// Initializes the registry.
    #[must_use]
    pub fn build(self) -> TestRegistry {
        let config_path = paths::home().join(".cargo/config");
        t!(fs::create_dir_all(config_path.parent().unwrap()));
        let prefix = if let Some(alternative) = &self.alternative {
            format!("{alternative}-")
        } else {
            String::new()
        };
        let registry_path = generate_path(&format!("{prefix}registry"));
        let index_url = generate_url(&format!("{prefix}registry"));
        let api_url = generate_url(&format!("{prefix}api"));
        let dl_url = generate_url(&format!("{prefix}dl"));
        let dl_path = generate_path(&format!("{prefix}dl"));
        let api_path = generate_path(&format!("{prefix}api"));

        let (server, index_url, api_url, dl_url) = if !self.http_index && !self.http_api {
            // No need to start the HTTP server.
            (None, index_url, api_url, dl_url)
        } else {
            let server = HttpServer::new(
                registry_path.clone(),
                dl_path,
                self.token.clone(),
                self.custom_responders,
            );
            let index_url = if self.http_index {
                server.index_url()
            } else {
                index_url
            };
            let api_url = if self.http_api {
                server.api_url()
            } else {
                api_url
            };
            let dl_url = server.dl_url();
            (Some(server), index_url, api_url, dl_url)
        };

        let registry = TestRegistry {
            api_url,
            index_url,
            _server: server,
            dl_url,
            path: registry_path,
            token: self.token,
        };

        if self.configure_registry {
            if let Some(alternative) = &self.alternative {
                append(
                    &config_path,
                    format!(
                        "
                    [registries.{alternative}]
                    index = '{}'",
                        registry.index_url
                    )
                    .as_bytes(),
                )
                .unwrap();
            } else {
                append(
                    &config_path,
                    format!(
                        "
                    [source.crates-io]
                    replace-with = 'dummy-registry'

                    [source.dummy-registry]
                    registry = '{}'",
                        registry.index_url
                    )
                    .as_bytes(),
                )
                .unwrap();
            }
        }

        if self.configure_token {
            let token = registry.token.as_deref().unwrap();
            let credentials = paths::home().join(".cargo/credentials");
            if let Some(alternative) = &self.alternative {
                append(
                    &credentials,
                    format!(
                        r#"
                    [registries.{alternative}]
                    token = "{token}"
                "#
                    )
                    .as_bytes(),
                )
                .unwrap();
            } else {
                append(
                    &credentials,
                    format!(
                        r#"
                    [registry]
                    token = "{token}"
                "#
                    )
                    .as_bytes(),
                )
                .unwrap();
            }
        }

        let api = if self.api {
            format!(r#","api":"{}""#, registry.api_url)
        } else {
            String::new()
        };
        // Initialize a new registry.
        repo(&registry.path)
            .file(
                "config.json",
                &format!(r#"{{"dl":"{}"{api}}}"#, registry.dl_url),
            )
            .build();
        fs::create_dir_all(api_path.join("api/v1/crates")).unwrap();

        registry
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

/// Entry with data that corresponds to [`tar::EntryType`].
#[non_exhaustive]
enum EntryData {
    Regular(String),
    Symlink(PathBuf),
}

/// A file to be created in a package.
struct PackageFile {
    path: String,
    contents: EntryData,
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
pub fn init() -> TestRegistry {
    RegistryBuilder::new().build()
}

/// Variant of `init` that initializes the "alternative" registry and crates.io
/// replacement.
pub fn alt_init() -> TestRegistry {
    init();
    RegistryBuilder::new().alternative().build()
}

pub struct HttpServerHandle {
    addr: SocketAddr,
}

impl HttpServerHandle {
    pub fn index_url(&self) -> Url {
        Url::parse(&format!("sparse+http://{}/index/", self.addr.to_string())).unwrap()
    }

    pub fn api_url(&self) -> Url {
        Url::parse(&format!("http://{}/", self.addr.to_string())).unwrap()
    }

    pub fn dl_url(&self) -> Url {
        Url::parse(&format!("http://{}/dl", self.addr.to_string())).unwrap()
    }
}

impl Drop for HttpServerHandle {
    fn drop(&mut self) {
        if let Ok(mut stream) = TcpStream::connect(self.addr) {
            // shutdown the server
            let _ = stream.write_all(b"stop");
            let _ = stream.flush();
        }
    }
}

/// Request to the test http server
#[derive(Debug)]
pub struct Request {
    pub url: Url,
    pub method: String,
    pub authorization: Option<String>,
    pub if_modified_since: Option<String>,
    pub if_none_match: Option<String>,
}

/// Response from the test http server
pub struct Response {
    pub code: u32,
    pub headers: Vec<String>,
    pub body: Vec<u8>,
}

struct HttpServer {
    listener: TcpListener,
    registry_path: PathBuf,
    dl_path: PathBuf,
    token: Option<String>,
    custom_responders: HashMap<&'static str, Box<dyn Send + Fn(&Request) -> Response>>,
}

impl HttpServer {
    pub fn new(
        registry_path: PathBuf,
        dl_path: PathBuf,
        token: Option<String>,
        api_responders: HashMap<&'static str, Box<dyn Send + Fn(&Request) -> Response>>,
    ) -> HttpServerHandle {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = HttpServer {
            listener,
            registry_path,
            dl_path,
            token,
            custom_responders: api_responders,
        };
        thread::spawn(move || server.start());
        HttpServerHandle { addr }
    }

    fn start(&self) {
        let mut line = String::new();
        'server: loop {
            let (socket, _) = self.listener.accept().unwrap();
            let mut buf = BufReader::new(socket);
            line.clear();
            if buf.read_line(&mut line).unwrap() == 0 {
                // Connection terminated.
                continue;
            }
            // Read the "GET path HTTP/1.1" line.
            let mut parts = line.split_ascii_whitespace();
            let method = parts.next().unwrap().to_ascii_lowercase();
            if method == "stop" {
                // Shutdown the server.
                return;
            }
            let addr = self.listener.local_addr().unwrap();
            let url = format!(
                "http://{}/{}",
                addr,
                parts.next().unwrap().trim_start_matches('/')
            );
            let url = Url::parse(&url).unwrap();

            // Grab headers we care about.
            let mut if_modified_since = None;
            let mut if_none_match = None;
            let mut authorization = None;
            loop {
                line.clear();
                if buf.read_line(&mut line).unwrap() == 0 {
                    continue 'server;
                }
                if line == "\r\n" {
                    // End of headers.
                    line.clear();
                    break;
                }
                let (name, value) = line.split_once(':').unwrap();
                let name = name.trim().to_ascii_lowercase();
                let value = value.trim().to_string();
                match name.as_str() {
                    "if-modified-since" => if_modified_since = Some(value),
                    "if-none-match" => if_none_match = Some(value),
                    "authorization" => authorization = Some(value),
                    _ => {}
                }
            }
            let req = Request {
                authorization,
                if_modified_since,
                if_none_match,
                method,
                url,
            };
            println!("req: {:#?}", req);
            let response = self.route(&req);
            let buf = buf.get_mut();
            write!(buf, "HTTP/1.1 {}\r\n", response.code).unwrap();
            write!(buf, "Content-Length: {}\r\n", response.body.len()).unwrap();
            for header in response.headers {
                write!(buf, "{}\r\n", header).unwrap();
            }
            write!(buf, "\r\n").unwrap();
            buf.write_all(&response.body).unwrap();
            buf.flush().unwrap();
        }
    }

    /// Route the request
    fn route(&self, req: &Request) -> Response {
        let authorized = |mutatation: bool| {
            if mutatation {
                self.token == req.authorization
            } else {
                assert!(req.authorization.is_none(), "unexpected token");
                true
            }
        };

        // Check for custom responder
        if let Some(responder) = self.custom_responders.get(req.url.path()) {
            return responder(&req);
        }
        let path: Vec<_> = req.url.path()[1..].split('/').collect();
        match (req.method.as_str(), path.as_slice()) {
            ("get", ["index", ..]) => {
                if !authorized(false) {
                    self.unauthorized(req)
                } else {
                    self.index(&req)
                }
            }
            ("get", ["dl", ..]) => {
                if !authorized(false) {
                    self.unauthorized(req)
                } else {
                    self.dl(&req)
                }
            }
            // The remainder of the operators in the test framework do nothing other than responding 'ok'.
            //
            // Note: We don't need to support anything real here because the testing framework publishes crates
            // by writing directly to the filesystem instead. If the test framework is changed to publish
            // via the HTTP API, then this should be made more complete.

            // publish
            ("put", ["api", "v1", "crates", "new"])
            // yank
            | ("delete", ["api", "v1", "crates", .., "yank"])
            // unyank
            | ("put", ["api", "v1", "crates", .., "unyank"])
            // owners
            | ("get" | "put" | "delete", ["api", "v1", "crates", .., "owners"]) => {
                if !authorized(true) {
                    self.unauthorized(req)
                } else {
                    self.ok(&req)
                }
            }
            _ => self.not_found(&req),
        }
    }

    /// Unauthorized response
    fn unauthorized(&self, _req: &Request) -> Response {
        Response {
            code: 401,
            headers: vec![],
            body: b"Unauthorized message from server.".to_vec(),
        }
    }

    /// Not found response
    fn not_found(&self, _req: &Request) -> Response {
        Response {
            code: 404,
            headers: vec![],
            body: b"not found".to_vec(),
        }
    }

    /// Respond OK without doing anything
    fn ok(&self, _req: &Request) -> Response {
        Response {
            code: 200,
            headers: vec![],
            body: br#"{"ok": true, "msg": "completed!"}"#.to_vec(),
        }
    }

    /// Serve the download endpoint
    fn dl(&self, req: &Request) -> Response {
        let file = self
            .dl_path
            .join(req.url.path().strip_prefix("/dl/").unwrap());
        println!("{}", file.display());
        if !file.exists() {
            return self.not_found(req);
        }
        return Response {
            body: fs::read(&file).unwrap(),
            code: 200,
            headers: vec![],
        };
    }

    /// Serve the registry index
    fn index(&self, req: &Request) -> Response {
        let file = self
            .registry_path
            .join(req.url.path().strip_prefix("/index/").unwrap());
        if !file.exists() {
            return self.not_found(req);
        } else {
            // Now grab info about the file.
            let data = fs::read(&file).unwrap();
            let etag = Sha256::new().update(&data).finish_hex();
            let last_modified = format!("{:?}", file.metadata().unwrap().modified().unwrap());

            // Start to construct our response:
            let mut any_match = false;
            let mut all_match = true;
            if let Some(expected) = &req.if_none_match {
                if &etag != expected {
                    all_match = false;
                } else {
                    any_match = true;
                }
            }
            if let Some(expected) = &req.if_modified_since {
                // NOTE: Equality comparison is good enough for tests.
                if &last_modified != expected {
                    all_match = false;
                } else {
                    any_match = true;
                }
            }

            if any_match && all_match {
                return Response {
                    body: Vec::new(),
                    code: 304,
                    headers: vec![],
                };
            } else {
                return Response {
                    body: data,
                    code: 200,
                    headers: vec![
                        format!("ETag: \"{}\"", etag),
                        format!("Last-Modified: {}", last_modified),
                    ],
                };
            }
        }
    }
}

impl Package {
    /// Creates a new package builder.
    /// Call `publish()` to finalize and build the package.
    pub fn new(name: &str, vers: &str) -> Package {
        let config = paths::home().join(".cargo/config");
        if !config.exists() {
            init();
        }
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
            contents: EntryData::Regular(contents.into()),
            mode,
            extra: false,
        });
        self
    }

    /// Adds a symlink to a path to the package.
    pub fn symlink(&mut self, dst: &str, src: &str) -> &mut Package {
        self.files.push(PackageFile {
            path: dst.to_string(),
            contents: EntryData::Symlink(src.into()),
            mode: DEFAULT_MODE,
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
            contents: EntryData::Regular(contents.to_string()),
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
            self.append(&mut a, "src/lib.rs", DEFAULT_MODE, &EntryData::Regular("".into()));
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

        self.append(ar, "Cargo.toml", DEFAULT_MODE, &EntryData::Regular(manifest.into()));
    }

    fn append<W: Write>(&self, ar: &mut Builder<W>, file: &str, mode: u32, contents: &EntryData) {
        self.append_raw(
            ar,
            &format!("{}-{}/{}", self.name, self.vers, file),
            mode,
            contents,
        );
    }

    fn append_raw<W: Write>(&self, ar: &mut Builder<W>, path: &str, mode: u32, contents: &EntryData) {
        let mut header = Header::new_ustar();
        let contents = match contents {
            EntryData::Regular(contents) => contents.as_str(),
            EntryData::Symlink(src) => {
                header.set_entry_type(tar::EntryType::Symlink);
                t!(header.set_link_name(src));
                "" // Symlink has no contents.
            }
        };
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
                .join("download")
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
