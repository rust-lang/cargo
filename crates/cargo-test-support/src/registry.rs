use crate::git::repo;
use crate::paths;
use crate::publish::{create_index_line, write_to_index};
use cargo_util::paths::append;
use cargo_util::Sha256;
use flate2::write::GzEncoder;
use flate2::Compression;
use pasetors::keys::{AsymmetricPublicKey, AsymmetricSecretKey};
use pasetors::paserk::FormatAsPaserk;
use pasetors::token::UntrustedToken;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use tar::{Builder, Header};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};
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

#[derive(Clone)]
pub enum Token {
    Plaintext(String),
    Keys(String, Option<String>),
}

impl Token {
    /// This is a valid PASETO secret key.
    /// This one is already publicly available as part of the text of the RFC so is safe to use for tests.
    pub fn rfc_key() -> Token {
        Token::Keys(
            "k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36"
                .to_string(),
            Some("sub".to_string()),
        )
    }
}

type RequestCallback = Box<dyn Send + Fn(&Request, &HttpServer) -> Response>;

/// A builder for initializing registries.
pub struct RegistryBuilder {
    /// If set, configures an alternate registry with the given name.
    alternative: Option<String>,
    /// The authorization token for the registry.
    token: Option<Token>,
    /// If set, the registry requires authorization for all operations.
    auth_required: bool,
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
    custom_responders: HashMap<String, RequestCallback>,
    /// Handler for 404 responses.
    not_found_handler: RequestCallback,
    /// If nonzero, the git index update to be delayed by the given number of seconds.
    delayed_index_update: usize,
    /// Credential provider in configuration
    credential_provider: Option<String>,
}

pub struct TestRegistry {
    server: Option<HttpServerHandle>,
    index_url: Url,
    path: PathBuf,
    api_url: Url,
    dl_url: Url,
    token: Token,
}

impl TestRegistry {
    pub fn index_url(&self) -> &Url {
        &self.index_url
    }

    pub fn api_url(&self) -> &Url {
        &self.api_url
    }

    pub fn token(&self) -> &str {
        match &self.token {
            Token::Plaintext(s) => s,
            Token::Keys(_, _) => panic!("registry was not configured with a plaintext token"),
        }
    }

    pub fn key(&self) -> &str {
        match &self.token {
            Token::Plaintext(_) => panic!("registry was not configured with a secret key"),
            Token::Keys(s, _) => s,
        }
    }

    /// Shutdown the server thread and wait for it to stop.
    /// `Drop` automatically stops the server, but this additionally
    /// waits for the thread to stop.
    pub fn join(self) {
        if let Some(mut server) = self.server {
            server.stop();
            let handle = server.handle.take().unwrap();
            handle.join().unwrap();
        }
    }
}

impl RegistryBuilder {
    #[must_use]
    pub fn new() -> RegistryBuilder {
        let not_found = |_req: &Request, _server: &HttpServer| -> Response {
            Response {
                code: 404,
                headers: vec![],
                body: b"not found".to_vec(),
            }
        };
        RegistryBuilder {
            alternative: None,
            token: None,
            auth_required: false,
            http_api: false,
            http_index: false,
            api: true,
            configure_registry: true,
            configure_token: true,
            custom_responders: HashMap::new(),
            not_found_handler: Box::new(not_found),
            delayed_index_update: 0,
            credential_provider: None,
        }
    }

    /// Adds a custom HTTP response for a specific url
    #[must_use]
    pub fn add_responder<R: 'static + Send + Fn(&Request, &HttpServer) -> Response>(
        mut self,
        url: impl Into<String>,
        responder: R,
    ) -> Self {
        self.custom_responders
            .insert(url.into(), Box::new(responder));
        self
    }

    #[must_use]
    pub fn not_found_handler<R: 'static + Send + Fn(&Request, &HttpServer) -> Response>(
        mut self,
        responder: R,
    ) -> Self {
        self.not_found_handler = Box::new(responder);
        self
    }

    /// Configures the git index update to be delayed by the given number of seconds.
    #[must_use]
    pub fn delayed_index_update(mut self, delay: usize) -> Self {
        self.delayed_index_update = delay;
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
    pub fn token(mut self, token: Token) -> Self {
        self.token = Some(token);
        self
    }

    /// Sets this registry to require the authentication token for
    /// all operations.
    #[must_use]
    pub fn auth_required(mut self) -> Self {
        self.auth_required = true;
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

    /// The credential provider to configure for this registry.
    #[must_use]
    pub fn credential_provider(mut self, provider: &[&str]) -> Self {
        self.credential_provider = Some(format!("['{}']", provider.join("','")));
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
        let token = self
            .token
            .unwrap_or_else(|| Token::Plaintext(format!("{prefix}sekrit")));

        let (server, index_url, api_url, dl_url) = if !self.http_index && !self.http_api {
            // No need to start the HTTP server.
            (None, index_url, api_url, dl_url)
        } else {
            let server = HttpServer::new(
                registry_path.clone(),
                dl_path,
                api_path.clone(),
                token.clone(),
                self.auth_required,
                self.custom_responders,
                self.not_found_handler,
                self.delayed_index_update,
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
            server,
            dl_url,
            path: registry_path,
            token,
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
                if let Some(p) = &self.credential_provider {
                    append(
                        &config_path,
                        &format!(
                            "
                        credential-provider = {p}
                        "
                        )
                        .as_bytes(),
                    )
                    .unwrap()
                }
            } else {
                append(
                    &config_path,
                    format!(
                        "
                        [source.crates-io]
                        replace-with = 'dummy-registry'

                        [registries.dummy-registry]
                        index = '{}'",
                        registry.index_url
                    )
                    .as_bytes(),
                )
                .unwrap();

                if let Some(p) = &self.credential_provider {
                    append(
                        &config_path,
                        &format!(
                            "
                        [registry]
                        credential-provider = {p}
                        "
                        )
                        .as_bytes(),
                    )
                    .unwrap()
                }
            }
        }

        if self.configure_token {
            let credentials = paths::home().join(".cargo/credentials.toml");
            match &registry.token {
                Token::Plaintext(token) => {
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
                Token::Keys(key, subject) => {
                    let mut out = if let Some(alternative) = &self.alternative {
                        format!("\n[registries.{alternative}]\n")
                    } else {
                        format!("\n[registry]\n")
                    };
                    out += &format!("secret-key = \"{key}\"\n");
                    if let Some(subject) = subject {
                        out += &format!("secret-key-subject = \"{subject}\"\n");
                    }

                    append(&credentials, out.as_bytes()).unwrap();
                }
            }
        }

        let auth = if self.auth_required {
            r#","auth-required":true"#
        } else {
            ""
        };
        let api = if self.api {
            format!(r#","api":"{}""#, registry.api_url)
        } else {
            String::new()
        };
        // Initialize a new registry.
        repo(&registry.path)
            .file(
                "config.json",
                &format!(r#"{{"dl":"{}"{api}{auth}}}"#, registry.dl_url),
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
/// ```no_run
/// use cargo_test_support::registry::Package;
/// use cargo_test_support::project;
///
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

pub(crate) type FeatureMap = BTreeMap<String, Vec<String>>;

#[derive(Clone)]
pub struct Dependency {
    name: String,
    vers: String,
    kind: String,
    artifact: Option<String>,
    bindep_target: Option<String>,
    lib: bool,
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
    handle: Option<JoinHandle<()>>,
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

    fn stop(&self) {
        if let Ok(mut stream) = TcpStream::connect(self.addr) {
            // shutdown the server
            let _ = stream.write_all(b"stop");
            let _ = stream.flush();
        }
    }
}

impl Drop for HttpServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Request to the test http server
#[derive(Clone)]
pub struct Request {
    pub url: Url,
    pub method: String,
    pub body: Option<Vec<u8>>,
    pub authorization: Option<String>,
    pub if_modified_since: Option<String>,
    pub if_none_match: Option<String>,
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // body is not included as it can produce long debug outputs
        f.debug_struct("Request")
            .field("url", &self.url)
            .field("method", &self.method)
            .field("authorization", &self.authorization)
            .field("if_modified_since", &self.if_modified_since)
            .field("if_none_match", &self.if_none_match)
            .finish()
    }
}

/// Response from the test http server
pub struct Response {
    pub code: u32,
    pub headers: Vec<String>,
    pub body: Vec<u8>,
}

pub struct HttpServer {
    listener: TcpListener,
    registry_path: PathBuf,
    dl_path: PathBuf,
    api_path: PathBuf,
    addr: SocketAddr,
    token: Token,
    auth_required: bool,
    custom_responders: HashMap<String, RequestCallback>,
    not_found_handler: RequestCallback,
    delayed_index_update: usize,
}

/// A helper struct that collects the arguments for [`HttpServer::check_authorized`].
/// Based on looking at the request, these are the fields that the authentication header should attest to.
struct Mutation<'a> {
    mutation: &'a str,
    name: Option<&'a str>,
    vers: Option<&'a str>,
    cksum: Option<&'a str>,
}

impl HttpServer {
    pub fn new(
        registry_path: PathBuf,
        dl_path: PathBuf,
        api_path: PathBuf,
        token: Token,
        auth_required: bool,
        custom_responders: HashMap<String, RequestCallback>,
        not_found_handler: RequestCallback,
        delayed_index_update: usize,
    ) -> HttpServerHandle {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = HttpServer {
            listener,
            registry_path,
            dl_path,
            api_path,
            addr,
            token,
            auth_required,
            custom_responders,
            not_found_handler,
            delayed_index_update,
        };
        let handle = Some(thread::spawn(move || server.start()));
        HttpServerHandle { addr, handle }
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
            let mut content_len = None;
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
                    "content-length" => content_len = Some(value),
                    _ => {}
                }
            }

            let mut body = None;
            if let Some(con_len) = content_len {
                let len = con_len.parse::<u64>().unwrap();
                let mut content = vec![0u8; len as usize];
                buf.read_exact(&mut content).unwrap();
                body = Some(content)
            }

            let req = Request {
                authorization,
                if_modified_since,
                if_none_match,
                method,
                url,
                body,
            };
            println!("req: {:#?}", req);
            let response = self.route(&req);
            let buf = buf.get_mut();
            write!(buf, "HTTP/1.1 {}\r\n", response.code).unwrap();
            write!(buf, "Content-Length: {}\r\n", response.body.len()).unwrap();
            write!(buf, "Connection: close\r\n").unwrap();
            for header in response.headers {
                write!(buf, "{}\r\n", header).unwrap();
            }
            write!(buf, "\r\n").unwrap();
            buf.write_all(&response.body).unwrap();
            buf.flush().unwrap();
        }
    }

    fn check_authorized(&self, req: &Request, mutation: Option<Mutation<'_>>) -> bool {
        let (private_key, private_key_subject) = if mutation.is_some() || self.auth_required {
            match &self.token {
                Token::Plaintext(token) => return Some(token) == req.authorization.as_ref(),
                Token::Keys(private_key, private_key_subject) => {
                    (private_key.as_str(), private_key_subject)
                }
            }
        } else {
            assert!(req.authorization.is_none(), "unexpected token");
            return true;
        };

        macro_rules! t {
            ($e:expr) => {
                match $e {
                    Some(e) => e,
                    None => return false,
                }
            };
        }

        let secret: AsymmetricSecretKey<pasetors::version3::V3> = private_key.try_into().unwrap();
        let public: AsymmetricPublicKey<pasetors::version3::V3> = (&secret).try_into().unwrap();
        let pub_key_id: pasetors::paserk::Id = (&public).into();
        let mut paserk_pub_key_id = String::new();
        FormatAsPaserk::fmt(&pub_key_id, &mut paserk_pub_key_id).unwrap();
        // https://github.com/rust-lang/rfcs/blob/master/text/3231-cargo-asymmetric-tokens.md#how-the-registry-server-will-validate-an-asymmetric-token

        // - The PASETO is in v3.public format.
        let authorization = t!(&req.authorization);
        let untrusted_token = t!(
            UntrustedToken::<pasetors::Public, pasetors::version3::V3>::try_from(authorization)
                .ok()
        );

        // - The PASETO validates using the public key it looked up based on the key ID.
        #[derive(serde::Deserialize, Debug)]
        struct Footer<'a> {
            url: &'a str,
            kip: &'a str,
        }
        let footer: Footer<'_> =
            t!(serde_json::from_slice(untrusted_token.untrusted_footer()).ok());
        if footer.kip != paserk_pub_key_id {
            return false;
        }
        let trusted_token =
            t!(
                pasetors::version3::PublicToken::verify(&public, &untrusted_token, None, None,)
                    .ok()
            );

        // - The URL matches the registry base URL
        if footer.url != "https://github.com/rust-lang/crates.io-index"
            && footer.url != &format!("sparse+http://{}/index/", self.addr.to_string())
        {
            return false;
        }

        // - The PASETO is still within its valid time period.
        #[derive(serde::Deserialize)]
        struct Message<'a> {
            iat: &'a str,
            sub: Option<&'a str>,
            mutation: Option<&'a str>,
            name: Option<&'a str>,
            vers: Option<&'a str>,
            cksum: Option<&'a str>,
            _challenge: Option<&'a str>, // todo: PASETO with challenges
            v: Option<u8>,
        }
        let message: Message<'_> = t!(serde_json::from_str(trusted_token.payload()).ok());
        let token_time = t!(OffsetDateTime::parse(message.iat, &Rfc3339).ok());
        let now = OffsetDateTime::now_utc();
        if (now - token_time) > Duration::MINUTE {
            return false;
        }
        if private_key_subject.as_deref() != message.sub {
            return false;
        }
        // - If the claim v is set, that it has the value of 1.
        if let Some(v) = message.v {
            if v != 1 {
                return false;
            }
        }
        // - If the server issues challenges, that the challenge has not yet been answered.
        // todo: PASETO with challenges
        // - If the operation is a mutation:
        if let Some(mutation) = mutation {
            //  - That the operation matches the mutation field and is one of publish, yank, or unyank.
            if message.mutation != Some(mutation.mutation) {
                return false;
            }
            //  - That the package, and version match the request.
            if message.name != mutation.name {
                return false;
            }
            if message.vers != mutation.vers {
                return false;
            }
            //  - If the mutation is publish, that the version has not already been published, and that the hash matches the request.
            if mutation.mutation == "publish" {
                if message.cksum != mutation.cksum {
                    return false;
                }
            }
        } else {
            // - If the operation is a read, that the mutation field is not set.
            if message.mutation.is_some()
                || message.name.is_some()
                || message.vers.is_some()
                || message.cksum.is_some()
            {
                return false;
            }
        }
        true
    }

    /// Route the request
    fn route(&self, req: &Request) -> Response {
        // Check for custom responder
        if let Some(responder) = self.custom_responders.get(req.url.path()) {
            return responder(&req, self);
        }
        let path: Vec<_> = req.url.path()[1..].split('/').collect();
        match (req.method.as_str(), path.as_slice()) {
            ("get", ["index", ..]) => {
                if !self.check_authorized(req, None) {
                    self.unauthorized(req)
                } else {
                    self.index(&req)
                }
            }
            ("get", ["dl", ..]) => {
                if !self.check_authorized(req, None) {
                    self.unauthorized(req)
                } else {
                    self.dl(&req)
                }
            }
            // publish
            ("put", ["api", "v1", "crates", "new"]) => self.check_authorized_publish(req),
            // The remainder of the operators in the test framework do nothing other than responding 'ok'.
            //
            // Note: We don't need to support anything real here because there are no tests that
            // currently require anything other than publishing via the http api.

            // yank / unyank
            ("delete" | "put", ["api", "v1", "crates", crate_name, version, mutation]) => {
                if !self.check_authorized(
                    req,
                    Some(Mutation {
                        mutation,
                        name: Some(crate_name),
                        vers: Some(version),
                        cksum: None,
                    }),
                ) {
                    self.unauthorized(req)
                } else {
                    self.ok(&req)
                }
            }
            // owners
            ("get" | "put" | "delete", ["api", "v1", "crates", crate_name, "owners"]) => {
                if !self.check_authorized(
                    req,
                    Some(Mutation {
                        mutation: "owners",
                        name: Some(crate_name),
                        vers: None,
                        cksum: None,
                    }),
                ) {
                    self.unauthorized(req)
                } else {
                    self.ok(&req)
                }
            }
            _ => self.not_found(&req),
        }
    }

    /// Unauthorized response
    pub fn unauthorized(&self, _req: &Request) -> Response {
        Response {
            code: 401,
            headers: vec![
                r#"WWW-Authenticate: Cargo login_url="https://test-registry-login/me""#.to_string(),
            ],
            body: b"Unauthorized message from server.".to_vec(),
        }
    }

    /// Not found response
    pub fn not_found(&self, req: &Request) -> Response {
        (self.not_found_handler)(req, self)
    }

    /// Respond OK without doing anything
    pub fn ok(&self, _req: &Request) -> Response {
        Response {
            code: 200,
            headers: vec![],
            body: br#"{"ok": true, "msg": "completed!"}"#.to_vec(),
        }
    }

    /// Return an internal server error (HTTP 500)
    pub fn internal_server_error(&self, _req: &Request) -> Response {
        Response {
            code: 500,
            headers: vec![],
            body: br#"internal server error"#.to_vec(),
        }
    }

    /// Serve the download endpoint
    pub fn dl(&self, req: &Request) -> Response {
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
    pub fn index(&self, req: &Request) -> Response {
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

    pub fn check_authorized_publish(&self, req: &Request) -> Response {
        if let Some(body) = &req.body {
            // Mimic the publish behavior for local registries by writing out the request
            // so tests can verify publishes made to either registry type.
            let path = self.api_path.join("api/v1/crates/new");
            t!(fs::create_dir_all(path.parent().unwrap()));
            t!(fs::write(&path, body));

            // Get the metadata of the package
            let (len, remaining) = body.split_at(4);
            let json_len = u32::from_le_bytes(len.try_into().unwrap());
            let (json, remaining) = remaining.split_at(json_len as usize);
            let new_crate = serde_json::from_slice::<crates_io::NewCrate>(json).unwrap();
            // Get the `.crate` file
            let (len, remaining) = remaining.split_at(4);
            let file_len = u32::from_le_bytes(len.try_into().unwrap());
            let (file, _remaining) = remaining.split_at(file_len as usize);
            let file_cksum = cksum(&file);

            if !self.check_authorized(
                req,
                Some(Mutation {
                    mutation: "publish",
                    name: Some(&new_crate.name),
                    vers: Some(&new_crate.vers),
                    cksum: Some(&file_cksum),
                }),
            ) {
                return self.unauthorized(req);
            }

            let dst = self
                .dl_path
                .join(&new_crate.name)
                .join(&new_crate.vers)
                .join("download");

            if self.delayed_index_update == 0 {
                save_new_crate(dst, new_crate, file, file_cksum, &self.registry_path);
            } else {
                let delayed_index_update = self.delayed_index_update;
                let registry_path = self.registry_path.clone();
                let file = Vec::from(file);
                thread::spawn(move || {
                    thread::sleep(std::time::Duration::new(delayed_index_update as u64, 0));
                    save_new_crate(dst, new_crate, &file, file_cksum, &registry_path);
                });
            }

            self.ok(&req)
        } else {
            Response {
                code: 400,
                headers: vec![],
                body: b"The request was missing a body".to_vec(),
            }
        }
    }
}

fn save_new_crate(
    dst: PathBuf,
    new_crate: crates_io::NewCrate,
    file: &[u8],
    file_cksum: String,
    registry_path: &Path,
) {
    // Write the `.crate`
    t!(fs::create_dir_all(dst.parent().unwrap()));
    t!(fs::write(&dst, file));

    let deps = new_crate
        .deps
        .iter()
        .map(|dep| {
            let (name, package) = match &dep.explicit_name_in_toml {
                Some(explicit) => (explicit.to_string(), Some(dep.name.to_string())),
                None => (dep.name.to_string(), None),
            };
            serde_json::json!({
                "name": name,
                "req": dep.version_req,
                "features": dep.features,
                "default_features": true,
                "target": dep.target,
                "optional": dep.optional,
                "kind": dep.kind,
                "registry": dep.registry,
                "package": package,
            })
        })
        .collect::<Vec<_>>();

    let line = create_index_line(
        serde_json::json!(new_crate.name),
        &new_crate.vers,
        deps,
        &file_cksum,
        new_crate.features,
        false,
        new_crate.links,
        None,
        None,
    );

    write_to_index(registry_path, &new_crate.name, line, false);
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
    /// ```toml
    /// [dependencies]
    /// foo = {version = "1.0"}
    /// ```
    pub fn dep(&mut self, name: &str, vers: &str) -> &mut Package {
        self.add_dep(&Dependency::new(name, vers))
    }

    /// Adds a dependency with the given feature. Example:
    /// ```toml
    /// [dependencies]
    /// foo = {version = "1.0", "features": ["feat1", "feat2"]}
    /// ```
    pub fn feature_dep(&mut self, name: &str, vers: &str, features: &[&str]) -> &mut Package {
        self.add_dep(Dependency::new(name, vers).enable_features(features))
    }

    /// Adds a platform-specific dependency. Example:
    /// ```toml
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
    /// ```toml
    /// [dev-dependencies]
    /// foo = {version = "1.0"}
    /// ```
    pub fn dev_dep(&mut self, name: &str, vers: &str) -> &mut Package {
        self.add_dep(Dependency::new(name, vers).dev())
    }

    /// Adds a build-dependency. Example:
    /// ```toml
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
    /// See `cargo::sources::registry::IndexPackage` for more information.
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
                let artifact = if let Some(artifact) = &dep.artifact {
                    serde_json::json!([artifact])
                } else {
                    serde_json::json!(null)
                };
                serde_json::json!({
                    "name": dep.name,
                    "req": dep.vers,
                    "features": dep.features,
                    "default_features": true,
                    "target": dep.target,
                    "artifact": artifact,
                    "bindep_target": dep.bindep_target,
                    "lib": dep.lib,
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
        let line = create_index_line(
            name,
            &self.vers,
            deps,
            &cksum,
            self.features.clone(),
            self.yanked,
            self.links.clone(),
            self.rust_version.as_deref(),
            self.v,
        );

        let registry_path = if self.alternative {
            alt_registry_path()
        } else {
            registry_path()
        };

        write_to_index(&registry_path, &self.name, line, self.local);

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
            self.append(
                &mut a,
                "src/lib.rs",
                DEFAULT_MODE,
                &EntryData::Regular("".into()),
            );
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
            let mut features = String::new();
            serde::Serialize::serialize(
                &self.cargo_features,
                toml::ser::ValueSerializer::new(&mut features),
            )
            .unwrap();
            manifest.push_str(&format!("cargo-features = {}\n\n", features));
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
            if let Some(artifact) = &dep.artifact {
                manifest.push_str(&format!("artifact = \"{}\"\n", artifact));
            }
            if let Some(target) = &dep.bindep_target {
                manifest.push_str(&format!("target = \"{}\"\n", target));
            }
            if dep.lib {
                manifest.push_str("lib = true\n");
            }
            if let Some(registry) = &dep.registry {
                assert_eq!(registry, "alternative");
                manifest.push_str(&format!("registry-index = \"{}\"", alt_registry_url()));
            }
        }
        if self.proc_macro {
            manifest.push_str("[lib]\nproc-macro = true\n");
        }

        self.append(
            ar,
            "Cargo.toml",
            DEFAULT_MODE,
            &EntryData::Regular(manifest.into()),
        );
    }

    fn append<W: Write>(&self, ar: &mut Builder<W>, file: &str, mode: u32, contents: &EntryData) {
        self.append_raw(
            ar,
            &format!("{}-{}/{}", self.name, self.vers, file),
            mode,
            contents,
        );
    }

    fn append_raw<W: Write>(
        &self,
        ar: &mut Builder<W>,
        path: &str,
        mode: u32,
        contents: &EntryData,
    ) {
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
            bindep_target: None,
            lib: false,
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
        self.artifact = Some(kind.to_string());
        self.bindep_target = target;
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
