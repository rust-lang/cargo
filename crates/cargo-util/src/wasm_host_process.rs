use crate::process_error::ProcessError;

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};

use super::ProcessBuilder;

#[link(wasm_import_module = "edit-dev:upstream-cargo/process@0.1.0")]
unsafe extern "C" {
    #[link_name = "run"]
    fn host_process_run(ptr: *const u8, len: usize, ret: *mut usize);
}

#[used]
#[unsafe(link_section = "component-type:edit-dev:upstream-cargo@0.1.0:runtime:encoded world")]
static EDIT_DEV_UPSTREAM_CARGO_PROCESS_COMPONENT_TYPE: [u8; 216] =
    *include_bytes!("edit_dev_upstream_cargo_process_component_type.bin");

#[derive(Serialize)]
struct HostProcessRequest {
    program: String,
    arg0: Option<String>,
    args: Vec<String>,
    cwd: Option<String>,
    env: BTreeMap<String, String>,
    stdin: Option<String>,
}

#[derive(Serialize)]
struct HostProcessTrace<'a> {
    program: &'a str,
    arg0: Option<&'a str>,
    cwd: Option<&'a str>,
    args: &'a [String],
    env: BTreeMap<&'a str, &'a str>,
    env_keys: Vec<&'a str>,
}

#[derive(Deserialize)]
struct HostProcessResponse {
    ok: bool,
    exit_code: i32,
    stdout: String,
    stderr: String,
    error: Option<String>,
}

pub struct HostProcessOutput {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl HostProcessOutput {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }

    pub fn into_std_output(self) -> Output {
        Output {
            status: ExitStatus::default(),
            stdout: self.stdout,
            stderr: self.stderr,
        }
    }

    pub fn process_error(&self, message: &str, include_output: bool) -> ProcessError {
        ProcessError::new_raw(
            message,
            Some(self.exit_code),
            &format!("exit status: {}", self.exit_code),
            include_output.then_some(self.stdout.as_slice()),
            include_output.then_some(self.stderr.as_slice()),
        )
    }
}

pub fn run(process_builder: &ProcessBuilder) -> io::Result<HostProcessOutput> {
    let request = HostProcessRequest {
        program: os_to_string(process_builder.get_program()),
        arg0: process_builder.get_arg0().map(os_to_string),
        args: process_builder.get_args().map(os_to_string).collect(),
        cwd: process_builder
            .get_cwd()
            .map(|path| path.to_string_lossy().into_owned())
            .or_else(|| {
                env::current_dir()
                    .ok()
                    .map(|path| path.to_string_lossy().into_owned())
            }),
        env: effective_env(process_builder),
        stdin: process_builder
            .stdin
            .as_ref()
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned()),
    };
    trace_request(&request);
    let request_json = serde_json::to_string(&request)
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    let response = invoke_host(&request_json)?;
    let response: HostProcessResponse = serde_json::from_str(&response)
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    if response.ok && response.exit_code == 0 {
        maybe_apply_proc_macro_section(&request)?;
    }
    if !response.ok {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            response
                .error
                .unwrap_or_else(|| "host process invocation failed".to_string()),
        ));
    }
    Ok(HostProcessOutput {
        exit_code: response.exit_code,
        stdout: response.stdout.into_bytes(),
        stderr: response.stderr.into_bytes(),
    })
}

fn maybe_apply_proc_macro_section(request: &HostProcessRequest) -> io::Result<()> {
    let Some(section_b64) = request.env.get("CARGO_PROC_MACRO_CUSTOM_SECTION_B64") else {
        return Ok(());
    };
    if section_b64.is_empty() || executable_basename(&request.program) != "rustc" {
        return Ok(());
    }

    let section = base64::engine::general_purpose::STANDARD
        .decode(section_b64)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let crate_name = rustc_arg_value(&request.args, "--crate-name")
        .unwrap_or("proc_macro")
        .replace('-', "_");
    let Some(out_dir) = rustc_arg_value(&request.args, "--out-dir") else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "proc-macro section requested without --out-dir",
        ));
    };
    let out_dir = absolutize_guest_path(request.cwd.as_deref(), out_dir);
    let extra_filename = rustc_extra_filename(&request.args).unwrap_or_default();
    let wasm_path =
        find_proc_macro_wasm(&out_dir, &crate_name, &extra_filename)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "proc-macro section requested but no wasm artifact found in {} for {}",
                    out_dir.display(),
                    crate_name
                ),
            )
        })?;
    inject_custom_section(&wasm_path, &section)?;
    mirror_proc_macro_artifacts(&out_dir, &wasm_path, &crate_name, &extra_filename)?;
    if trace_enabled() {
        eprintln!(
            "cargo-upstream-process-debug proc-macro-section path={} bytes={}",
            wasm_path.display(),
            section.len()
        );
    }
    Ok(())
}

fn mirror_proc_macro_artifacts(
    out_dir: &Path,
    wasm_path: &Path,
    crate_name: &str,
    extra_filename: &str,
) -> io::Result<()> {
    let Some(target_out_dir) = proc_macro_target_out_dir(out_dir) else {
        return Ok(());
    };
    fs::create_dir_all(&target_out_dir)?;
    let Some(wasm_name) = wasm_path.file_name() else {
        return Ok(());
    };
    fs::copy(wasm_path, target_out_dir.join(wasm_name))?;

    let rmeta_name = format!("lib{crate_name}{extra_filename}.rmeta");
    let rmeta_path = out_dir.join(&rmeta_name);
    if rmeta_path.exists() {
        fs::copy(&rmeta_path, target_out_dir.join(rmeta_name))?;
    }
    Ok(())
}

fn proc_macro_target_out_dir(out_dir: &Path) -> Option<PathBuf> {
    if out_dir.file_name().and_then(|name| name.to_str()) != Some("deps") {
        return None;
    }
    let profile_dir = out_dir.parent()?;
    let profile = profile_dir.file_name()?;
    let target_dir = profile_dir.parent()?;
    Some(target_dir.join("wasm32-wasip1").join(profile).join("deps"))
}

fn executable_basename(program: &str) -> &str {
    program
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(program)
}

fn rustc_arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
}

fn rustc_extra_filename(args: &[String]) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "-C" {
            if let Some(value) = iter.next() {
                if let Some(extra) = value.strip_prefix("extra-filename=") {
                    return Some(extra.to_string());
                }
            }
        } else if let Some(extra) = arg.strip_prefix("-Cextra-filename=") {
            return Some(extra.to_string());
        }
    }
    None
}

fn absolutize_guest_path(cwd: Option<&str>, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    Path::new(cwd.unwrap_or("/")).join(path)
}

fn find_proc_macro_wasm(
    out_dir: &Path,
    crate_name: &str,
    extra_filename: &str,
) -> io::Result<Option<PathBuf>> {
    let direct = [
        out_dir.join(format!("{crate_name}{extra_filename}.wasm")),
        out_dir.join(format!("lib{crate_name}{extra_filename}.wasm")),
    ];
    for candidate in direct {
        if candidate.exists() {
            return Ok(Some(candidate));
        }
    }

    let mut matches = Vec::new();
    let Ok(entries) = fs::read_dir(out_dir) else {
        return Ok(None);
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("wasm") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.starts_with(crate_name) || file_name.starts_with(&format!("lib{crate_name}")) {
            matches.push(path);
        }
    }
    matches.sort();
    Ok(matches.into_iter().next())
}

fn inject_custom_section(wasm_path: &Path, custom_section: &[u8]) -> io::Result<()> {
    let wasm = fs::read(wasm_path)?;
    if wasm.len() < 8 || &wasm[..4] != b"\0asm" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} is not a wasm binary", wasm_path.display()),
        ));
    }
    let mut out = Vec::with_capacity(wasm.len() + custom_section.len());
    out.extend_from_slice(&wasm[..8]);
    out.extend_from_slice(custom_section);
    out.extend_from_slice(&wasm[8..]);
    fs::write(wasm_path, out)
}

fn invoke_host(request: &str) -> io::Result<String> {
    let mut ret = [0usize; 2];
    unsafe {
        host_process_run(request.as_ptr(), request.len(), ret.as_mut_ptr());
    }
    let ptr = ret[0] as *mut u8;
    let len = ret[1];
    if len == 0 {
        return Ok(String::new());
    }
    if ptr.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "host returned a null string pointer",
        ));
    }
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len) };
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn effective_env(process_builder: &ProcessBuilder) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = env::vars().collect();
    for (key, value) in process_builder.get_envs() {
        if let Some(value) = value {
            out.insert(key.clone(), os_to_string(value));
        } else {
            out.remove(key);
        }
    }
    out
}

fn trace_request(request: &HostProcessRequest) {
    if !trace_enabled() {
        return;
    }

    let include_full_env = env::var_os("CARGO_UPSTREAM_TRACE_RUSTC_FULL_ENV").is_some();
    let env = request
        .env
        .iter()
        .filter(|(key, _value)| include_full_env || is_interesting_env_key(key))
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let env_keys = request.env.keys().map(String::as_str).collect();
    let trace = HostProcessTrace {
        program: &request.program,
        arg0: request.arg0.as_deref(),
        cwd: request.cwd.as_deref(),
        args: &request.args,
        env,
        env_keys,
    };
    match serde_json::to_string(&trace) {
        Ok(json) => eprintln!("cargo-upstream-process-debug {json}"),
        Err(error) => {
            eprintln!("cargo-upstream-process-debug <trace serialization failed: {error}>")
        }
    }
}

fn trace_enabled() -> bool {
    env::var_os("CARGO_UPSTREAM_TRACE_RUSTC").is_some()
        || env::var_os("CARGO_UPSTREAM_PROCESS_DEBUG").is_some()
}

fn is_interesting_env_key(key: &str) -> bool {
    key == "PATH"
        || key == "RUSTC"
        || key == "RUSTFLAGS"
        || key == "CARGO"
        || key == "CARGO_ENCODED_RUSTFLAGS"
        || key == "CARGO_BUILD_TARGET"
        || key == "CARGO_HOME"
        || key == "CARGO_MANIFEST_DIR"
        || key == "CARGO_PRIMARY_PACKAGE"
        || key == "CARGO_TARGET_DIR"
        || key == "HOME"
        || key == "RUST_BACKTRACE"
        || key == "RUST_LIB_BACKTRACE"
        || key == "RUST_SYSROOT"
        || key == "TMPDIR"
        || key.starts_with("CARGO_BIN_")
        || key.starts_with("CARGO_CFG_")
        || key.starts_with("CARGO_CRATE_")
        || key.starts_with("CARGO_PKG_")
        || key.starts_with("CARGO_PROFILE_")
        || key.starts_with("CARGO_TARGET_")
}

fn os_to_string(value: impl AsRef<std::ffi::OsStr>) -> String {
    value.as_ref().to_string_lossy().into_owned()
}
