use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};

use url::Url;

use cargo_util::Sha256;

pub type FeatureMap = BTreeMap<String, Vec<String>>;
pub type RequestCallback = Box<dyn Send + Fn(&Request, &HttpServer) -> Response>;

pub struct HttpServerHandle {
    addr: SocketAddr,
    pub(super) handle: Option<JoinHandle<()>>,
}

impl HttpServerHandle {
    pub fn index_url(&self) -> Url {
        Url::parse(&format!("sparse+http://{}/index/", self.addr)).unwrap()
    }

    pub fn api_url(&self) -> Url {
        Url::parse(&format!("http://{}/", self.addr)).unwrap()
    }

    pub fn dl_url(&self) -> Url {
        Url::parse(&format!("http://{}/dl", self.addr)).unwrap()
    }

    pub(super) fn stop(&self) {
        if let Ok(mut stream) = TcpStream::connect(self.addr) {
            // shutdown the server
            let _ = stream.write_all(b"stop");
            let _ = stream.flush();
        }
    }

    pub fn join(mut self) {
        self.stop();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
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
    token: String,
    auth_required: bool,
    custom_responders: HashMap<String, RequestCallback>,
    not_found_handler: RequestCallback,
    delayed_index_update: usize,
}

/// A helper struct that collects the arguments for [`HttpServer::check_authorized`].
/// Based on looking at the request, these are the fields that the authentication header should attest to.
struct Mutation {}

impl HttpServer {
    pub fn new(
        registry_path: PathBuf,
        dl_path: PathBuf,
        api_path: PathBuf,
        token: String,
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

    fn check_authorized(&self, req: &Request, mutation: Option<Mutation>) -> bool {
        if mutation.is_some() || self.auth_required {
            Some(&self.token) == req.authorization.as_ref()
        } else {
            assert!(req.authorization.is_none(), "unexpected token");
            true
        }
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
            ("delete" | "put", ["api", "v1", "crates", _, _, _]) => {
                if !self.check_authorized(req, Some(Mutation {})) {
                    self.unauthorized(req)
                } else {
                    self.ok(&req)
                }
            }
            // owners
            ("get" | "put" | "delete", ["api", "v1", "crates", _, "owners"]) => {
                if !self.check_authorized(req, Some(Mutation {})) {
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
                r#"www-authenticate: Cargo login_url="https://test-registry-login/me""#.to_string(),
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

    /// Return too many requests (HTTP 429)
    pub fn too_many_requests(&self, _req: &Request, delay: std::time::Duration) -> Response {
        Response {
            code: 429,
            headers: vec![format!("Retry-After: {}", delay.as_secs())],
            body: format!(
                "too many requests, try again in {} seconds",
                delay.as_secs()
            )
            .into_bytes(),
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
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, body).unwrap();

            // Get the metadata of the package
            let (len, remaining) = body.split_at(4);
            let json_len = u32::from_le_bytes(len.try_into().unwrap());
            let (json, remaining) = remaining.split_at(json_len as usize);
            let new_crate = serde_json::from_slice::<crates_io::NewCrate>(json).unwrap();
            // Get the `.crate` file
            let (len, remaining) = remaining.split_at(4);
            let file_len = u32::from_le_bytes(len.try_into().unwrap());
            let (file, _remaining) = remaining.split_at(file_len as usize);
            let file_cksum = Sha256::new().update(file).finish_hex();
            if !self.check_authorized(req, Some(Mutation {})) {
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
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::write(&dst, file).unwrap();

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
                "default_features": dep.default_features,
                "target": dep.target,
                "optional": dep.optional,
                "kind": dep.kind,
                "registry": dep.registry,
                "package": package,
                "artifact": dep.artifact,
                "bindep_target": dep.bindep_target,
                "lib": dep.lib,
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
        new_crate.rust_version.as_deref(),
        None,
        None,
    );

    write_to_index(registry_path, &new_crate.name, line, false);
}
pub fn create_index_line(
    name: serde_json::Value,
    vers: &str,
    deps: Vec<serde_json::Value>,
    cksum: &str,
    features: FeatureMap,
    yanked: bool,
    links: Option<String>,
    rust_version: Option<&str>,
    pubtime: Option<&str>,
    v: Option<u32>,
) -> String {
    // This emulates what crates.io does to retain backwards compatibility.
    let (features, features2) = split_index_features(features.clone());
    let mut json = serde_json::json!({
        "name": name,
        "vers": vers,
        "deps": deps,
        "cksum": cksum,
        "features": features,
        "yanked": yanked,
        "links": links,
    });
    if let Some(f2) = &features2 {
        json["features2"] = serde_json::json!(f2);
        json["v"] = serde_json::json!(2);
    }
    if let Some(v) = v {
        json["v"] = serde_json::json!(v);
    }
    if let Some(rust_version) = rust_version {
        json["rust_version"] = serde_json::json!(rust_version);
    }
    if let Some(pubtime) = pubtime {
        json["pubtime"] = serde_json::json!(pubtime);
    }

    json.to_string()
}

pub fn write_to_index(registry_path: &Path, name: &str, line: String, local: bool) {
    let file = cargo_util::registry::make_dep_path(name, false);

    // Write file/line in the index.
    let dst = if local {
        registry_path.join("index").join(&file)
    } else {
        registry_path.join(&file)
    };
    let prev = fs::read_to_string(&dst).unwrap_or_default();
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::write(&dst, prev + &line[..] + "\n").unwrap();

    // Add the new file to the index.
    if !local {
        let repo = git2::Repository::open(&registry_path).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(&file)).unwrap();
        index.write().unwrap();
        let id = index.write_tree().unwrap();

        // Commit this change.
        let tree = repo.find_tree(id).unwrap();
        let sig = repo.signature().unwrap();
        let parent = repo.refname_to_id("refs/heads/master").unwrap();
        let parent = repo.find_commit(parent).unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Another commit",
            &tree,
            &[&parent],
        )
        .unwrap();
    }
}

fn split_index_features(mut features: FeatureMap) -> (FeatureMap, Option<FeatureMap>) {
    let mut features2 = FeatureMap::new();
    for (feat, values) in features.iter_mut() {
        if values
            .iter()
            .any(|value| value.starts_with("dep:") || value.contains("?/"))
        {
            let new_values = std::mem::take(values);
            features2.insert(feat.clone(), new_values);
        }
    }
    if features2.is_empty() {
        (features, None)
    } else {
        (features, Some(features2))
    }
}
