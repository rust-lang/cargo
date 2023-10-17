//! Support for testing using Docker containers.
//!
//! The [`Container`] type is a builder for configuring a container to run.
//! After you call `launch`, you can use the [`ContainerHandle`] to interact
//! with the running container.
//!
//! Tests using containers must use `#[cargo_test(container_test)]` to disable
//! them unless the CARGO_CONTAINER_TESTS environment variable is set.

use cargo_util::ProcessBuilder;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use tar::Header;

/// A builder for configuring a container to run.
pub struct Container {
    /// The host directory that forms the basis of the Docker image.
    build_context: PathBuf,
    /// Files to copy over to the image.
    files: Vec<MkFile>,
}

/// A handle to a running container.
///
/// You can use this to interact with the container.
pub struct ContainerHandle {
    /// The name of the container.
    name: String,
    /// The IP address of the container.
    ///
    /// NOTE: This is currently unused, but may be useful so I left it in.
    /// This can only be used on Linux. macOS and Windows docker doesn't allow
    /// direct connection to the container.
    pub ip_address: String,
    /// Port mappings of container_port to host_port for ports exposed via EXPOSE.
    pub port_mappings: HashMap<u16, u16>,
}

impl Container {
    pub fn new(context_dir: &str) -> Container {
        assert!(std::env::var_os("CARGO_CONTAINER_TESTS").is_some());
        let mut build_context = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        build_context.push("containers");
        build_context.push(context_dir);
        Container {
            build_context,
            files: Vec::new(),
        }
    }

    /// Adds a file to be copied into the container.
    pub fn file(mut self, file: MkFile) -> Self {
        self.files.push(file);
        self
    }

    /// Starts the container.
    pub fn launch(mut self) -> ContainerHandle {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let name = format!("cargo_test_{id}");
        remove_if_exists(&name);
        self.create_container(&name);
        self.copy_files(&name);
        self.start_container(&name);
        let info = self.container_inspect(&name);
        let ip_address = if cfg!(target_os = "linux") {
            info[0]["NetworkSettings"]["IPAddress"]
                .as_str()
                .unwrap()
                .to_string()
        } else {
            // macOS and Windows can't make direct connections to the
            // container. It only works through exposed ports or mapped ports.
            "127.0.0.1".to_string()
        };
        let port_mappings = self.port_mappings(&info);
        self.wait_till_ready(&port_mappings);

        ContainerHandle {
            name,
            ip_address,
            port_mappings,
        }
    }

    fn create_container(&self, name: &str) {
        static BUILD_LOCK: Mutex<()> = Mutex::new(());

        let image_base = self.build_context.file_name().unwrap();
        let image_name = format!("cargo-test-{}", image_base.to_str().unwrap());
        let _lock = BUILD_LOCK
            .lock()
            .map_err(|_| panic!("previous docker build failed, unable to run test"));
        ProcessBuilder::new("docker")
            .args(&["build", "--tag", image_name.as_str()])
            .arg(&self.build_context)
            .exec_with_output()
            .unwrap();

        ProcessBuilder::new("docker")
            .args(&[
                "container",
                "create",
                "--publish-all",
                "--rm",
                "--name",
                name,
            ])
            .arg(image_name)
            .exec_with_output()
            .unwrap();
    }

    fn copy_files(&mut self, name: &str) {
        if self.files.is_empty() {
            return;
        }
        let mut ar = tar::Builder::new(Vec::new());
        let files = std::mem::replace(&mut self.files, Vec::new());
        for mut file in files {
            ar.append_data(&mut file.header, &file.path, file.contents.as_slice())
                .unwrap();
        }
        let ar = ar.into_inner().unwrap();
        ProcessBuilder::new("docker")
            .args(&["cp", "-"])
            .arg(format!("{name}:/"))
            .stdin(ar)
            .exec_with_output()
            .unwrap();
    }

    fn start_container(&self, name: &str) {
        ProcessBuilder::new("docker")
            .args(&["container", "start"])
            .arg(name)
            .exec_with_output()
            .unwrap();
    }

    fn container_inspect(&self, name: &str) -> serde_json::Value {
        let output = ProcessBuilder::new("docker")
            .args(&["inspect", name])
            .exec_with_output()
            .unwrap();
        serde_json::from_slice(&output.stdout).unwrap()
    }

    /// Returns the mapping of container_port->host_port for ports that were
    /// exposed with EXPOSE.
    fn port_mappings(&self, info: &serde_json::Value) -> HashMap<u16, u16> {
        info[0]["NetworkSettings"]["Ports"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| {
                let key = key
                    .strip_suffix("/tcp")
                    .expect("expected TCP only ports")
                    .parse()
                    .unwrap();
                let values = value.as_array().unwrap();
                let value = values
                    .iter()
                    .find(|value| value["HostIp"].as_str().unwrap() == "0.0.0.0")
                    .expect("expected localhost IP");
                let host_port = value["HostPort"].as_str().unwrap().parse().unwrap();
                (key, host_port)
            })
            .collect()
    }

    fn wait_till_ready(&self, port_mappings: &HashMap<u16, u16>) {
        for port in port_mappings.values() {
            let mut ok = false;
            for _ in 0..30 {
                match std::net::TcpStream::connect(format!("127.0.0.1:{port}")) {
                    Ok(_) => {
                        ok = true;
                        break;
                    }
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::ConnectionRefused {
                            panic!("unexpected localhost connection error: {e:?}");
                        }
                        std::thread::sleep(std::time::Duration::new(1, 0));
                    }
                }
            }
            if !ok {
                panic!("no listener on localhost port {port}");
            }
        }
    }
}

impl ContainerHandle {
    /// Executes a program inside a running container.
    pub fn exec(&self, args: &[&str]) -> std::process::Output {
        ProcessBuilder::new("docker")
            .args(&["container", "exec", &self.name])
            .args(args)
            .exec_with_output()
            .unwrap()
    }

    /// Returns the contents of a file inside the container.
    pub fn read_file(&self, path: &str) -> String {
        let output = ProcessBuilder::new("docker")
            .args(&["cp", &format!("{}:{}", self.name, path), "-"])
            .exec_with_output()
            .unwrap();
        let mut ar = tar::Archive::new(output.stdout.as_slice());
        let mut entry = ar.entries().unwrap().next().unwrap().unwrap();
        let mut contents = String::new();
        entry.read_to_string(&mut contents).unwrap();
        contents
    }
}

impl Drop for ContainerHandle {
    fn drop(&mut self) {
        // To help with debugging, this will keep the container alive.
        if std::env::var_os("CARGO_CONTAINER_TEST_KEEP").is_some() {
            return;
        }
        remove_if_exists(&self.name);
    }
}

fn remove_if_exists(name: &str) {
    if let Err(e) = Command::new("docker")
        .args(&["container", "rm", "--force", name])
        .output()
    {
        panic!("failed to run docker: {e}");
    }
}

/// Builder for configuring a file to copy into a container.
pub struct MkFile {
    path: String,
    contents: Vec<u8>,
    header: Header,
}

impl MkFile {
    /// Defines a file to add to the container.
    ///
    /// This should be passed to `Container::file`.
    ///
    /// The path is the path inside the container to create the file.
    pub fn path(path: &str) -> MkFile {
        MkFile {
            path: path.to_string(),
            contents: Vec::new(),
            header: Header::new_gnu(),
        }
    }

    pub fn contents(mut self, contents: impl Into<Vec<u8>>) -> Self {
        self.contents = contents.into();
        self.header.set_size(self.contents.len() as u64);
        self
    }

    pub fn mode(mut self, mode: u32) -> Self {
        self.header.set_mode(mode);
        self
    }

    pub fn uid(mut self, uid: u64) -> Self {
        self.header.set_uid(uid);
        self
    }

    pub fn gid(mut self, gid: u64) -> Self {
        self.header.set_gid(gid);
        self
    }
}
