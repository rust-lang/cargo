use crate::core::Workspace;
use crate::core::compiler::Unit;
use crate::util::path_args;
use anyhow::{Context as _, Result};
use base64::Engine as _;
use std::fs;
use std::path::{Path, PathBuf};

const TRANSFORM_CACHE_FILE: &str = ".cargo-wasm-proc-macro-transform.cache";
const TRANSFORM_SOURCE_HASH_FILE: &str = ".cargo-wasm-proc-macro-transform.source-hash";
const TRANSFORM_VERSION: &str = "2026-05-13-normalize-proc-macro-output";

#[derive(Debug)]
pub struct ProcMacroTransform {
    pub source_arg: PathBuf,
    pub cwd: PathBuf,
    pub custom_section_b64: String,
}

#[derive(Debug)]
pub struct SourceTransform {
    pub source_arg: PathBuf,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone)]
struct PackageVersionInfo {
    version: String,
    major: String,
    minor: String,
    patch: String,
}

impl PackageVersionInfo {
    fn from_manifest(version: &str, _edition: &str) -> Self {
        let mut parts = version.split('.');
        let major = parts.next().unwrap_or("0").to_string();
        let minor = parts.next().unwrap_or("0").to_string();
        let patch_with_pre = parts.next().unwrap_or("0");
        let patch = patch_with_pre
            .split(['-', '+'])
            .next()
            .unwrap_or("0")
            .to_string();
        Self {
            version: version.to_string(),
            major,
            minor,
            patch,
        }
    }

    fn no_mangle_attr(&self) -> &'static str {
        "#[no_mangle]"
    }
}

#[derive(Debug, Clone)]
struct ProcMacroEntry {
    kind: ProcMacroKind,
    name: String,
    fn_name: String,
    helper_attrs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProcMacroKind {
    Derive,
    FunctionLike,
    Attribute,
}

impl ProcMacroKind {
    fn metadata_prefix(self) -> &'static str {
        match self {
            ProcMacroKind::Derive => "derive",
            ProcMacroKind::FunctionLike => "bang",
            ProcMacroKind::Attribute => "attr",
        }
    }
}

pub fn prepare_proc_macro_transform(
    ws: &Workspace<'_>,
    unit: &Unit,
) -> Result<Option<ProcMacroTransform>> {
    if !unit.target.proc_macro() {
        return Ok(None);
    }

    let (source_arg, source_cwd) = path_args(ws, unit);
    let package_root = unit.pkg.root();
    let target_root = ws.target_dir().into_path_unlocked();
    let transform_root =
        target_root
            .join("cargo-proc-macro-transform")
            .join(sanitize_path_component(
                unit.pkg.package_id().name().as_str(),
            ));
    let source_abs = if source_arg.is_absolute() {
        source_arg.clone()
    } else {
        source_cwd.join(&source_arg)
    };
    let source_rel = source_abs
        .strip_prefix(package_root)
        .ok()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("src/lib.rs"));

    let entries = transform_proc_macro_crate(package_root, &transform_root)?;
    if entries.is_empty() {
        return Ok(None);
    }

    let section_bytes = generate_proc_macro_custom_section(&entries);
    let custom_section_b64 = base64::engine::general_purpose::STANDARD.encode(section_bytes);
    Ok(Some(ProcMacroTransform {
        source_arg: transform_root.join(source_rel),
        cwd: transform_root,
        custom_section_b64,
    }))
}

pub fn proc_macro2_vendor_root(unit: &Unit) -> Option<PathBuf> {
    if unit.pkg.package_id().name().as_str() != "proc-macro2" {
        return None;
    }
    if unit.target.is_custom_build() || !unit.target.is_lib() {
        return None;
    }
    let version = unit.pkg.package_id().version();
    let root = PathBuf::from(format!("/vendor/proc-macro2-{version}"));
    root.join("src/lib.rs").exists().then_some(root)
}

pub fn prepare_wstd_attr_transform(
    ws: &Workspace<'_>,
    unit: &Unit,
) -> Result<Option<SourceTransform>> {
    if unit.target.proc_macro() || unit.target.is_custom_build() {
        return Ok(None);
    }

    let (source_arg, source_cwd) = path_args(ws, unit);
    let source_abs = if source_arg.is_absolute() {
        source_arg.clone()
    } else {
        source_cwd.join(&source_arg)
    };
    if !source_abs.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&source_abs)
        .with_context(|| format!("failed to read {}", source_abs.display()))?;
    if !content.contains("#[wstd::http_server]") && !content.contains("#[wstd::main]") {
        return Ok(None);
    }

    let Some(transformed_content) = transform_wstd_attrs_content(&content)? else {
        return Ok(None);
    };

    let package_root = unit.pkg.root();
    let target_root = ws.target_dir().into_path_unlocked();
    let transform_root =
        target_root
            .join("cargo-wstd-attr-transform")
            .join(sanitize_path_component(
                unit.pkg.package_id().name().as_str(),
            ));
    if transform_root.exists() {
        fs::remove_dir_all(&transform_root).with_context(|| {
            format!(
                "failed to clear stale wstd attr transform dir {}",
                transform_root.display()
            )
        })?;
    }
    fs::create_dir_all(&transform_root)
        .with_context(|| format!("failed to create {}", transform_root.display()))?;
    copy_dir_recursive(package_root, &transform_root)?;

    let source_rel = source_abs
        .strip_prefix(package_root)
        .ok()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("src/main.rs"));
    let transformed_source = transform_root.join(&source_rel);
    fs::write(&transformed_source, transformed_content)
        .with_context(|| format!("failed to write {}", transformed_source.display()))?;

    Ok(Some(SourceTransform {
        source_arg: transformed_source,
        cwd: transform_root,
    }))
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn transform_proc_macro_crate(
    source_path: &Path,
    output_path: &Path,
) -> Result<Vec<ProcMacroEntry>> {
    let source_hash = compute_tree_fingerprint(source_path)
        .context("failed to fingerprint proc-macro source tree")?;
    if output_path.exists() {
        if let Some(entries) = load_transform_cache(output_path) {
            if let Ok(cached_hash) = fs::read_to_string(source_hash_path(output_path)) {
                if cached_hash.trim() == source_hash {
                    return Ok(entries);
                }
            }
        }
        fs::remove_dir_all(output_path).with_context(|| {
            format!(
                "failed to clear stale proc-macro transform dir {}",
                output_path.display()
            )
        })?;
    }

    fs::create_dir_all(output_path)
        .with_context(|| format!("failed to create {}", output_path.display()))?;
    copy_dir_recursive(source_path, output_path)?;

    let cargo_toml = output_path.join("Cargo.toml");
    let cargo_content = fs::read_to_string(&cargo_toml)
        .with_context(|| format!("failed to read {}", cargo_toml.display()))?;
    let version =
        parse_package_value(&cargo_content, "version").unwrap_or_else(|| "0.0.0".to_string());
    let edition =
        parse_package_value(&cargo_content, "edition").unwrap_or_else(|| "2021".to_string());
    let version_info = PackageVersionInfo::from_manifest(&version, &edition);

    let lib_rs = output_path.join("src/lib.rs");
    let entries = if lib_rs.exists() {
        let content = fs::read_to_string(&lib_rs)
            .with_context(|| format!("failed to read {}", lib_rs.display()))?;
        let (new_content, entries) = transform_lib_rs_content(&content, &version_info)?;
        fs::write(&lib_rs, new_content)
            .with_context(|| format!("failed to write {}", lib_rs.display()))?;
        entries
    } else {
        Vec::new()
    };

    write_transform_cache(output_path, &entries)?;
    fs::write(source_hash_path(output_path), source_hash.as_bytes())?;
    Ok(entries)
}

fn parse_package_value(cargo_toml: &str, key: &str) -> Option<String> {
    let mut in_package = false;
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let Some((found_key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if found_key.trim() == key {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src).with_context(|| format!("failed to read {}", src.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" || name == "target" {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(name);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::create_dir_all(&dst_path)?;
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "failed copying {} -> {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn cache_path(output_path: &Path) -> PathBuf {
    output_path.join(TRANSFORM_CACHE_FILE)
}

fn source_hash_path(output_path: &Path) -> PathBuf {
    output_path.join(TRANSFORM_SOURCE_HASH_FILE)
}

fn kind_to_str(kind: ProcMacroKind) -> &'static str {
    match kind {
        ProcMacroKind::Derive => "derive",
        ProcMacroKind::FunctionLike => "bang",
        ProcMacroKind::Attribute => "attr",
    }
}

fn str_to_kind(kind: &str) -> Option<ProcMacroKind> {
    match kind {
        "derive" => Some(ProcMacroKind::Derive),
        "bang" => Some(ProcMacroKind::FunctionLike),
        "attr" => Some(ProcMacroKind::Attribute),
        _ => None,
    }
}

fn load_transform_cache(output_path: &Path) -> Option<Vec<ProcMacroEntry>> {
    let raw = fs::read_to_string(cache_path(output_path)).ok()?;
    let mut entries = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(4, '\t');
        let kind = str_to_kind(parts.next()?)?;
        let name = parts.next()?.to_string();
        let fn_name = parts.next()?.to_string();
        let helper_attrs = parts
            .next()
            .map(|s| {
                s.split(',')
                    .map(str::trim)
                    .filter(|attr| !attr.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        entries.push(ProcMacroEntry {
            kind,
            name,
            fn_name,
            helper_attrs,
        });
    }
    (!entries.is_empty()).then_some(entries)
}

fn write_transform_cache(output_path: &Path, entries: &[ProcMacroEntry]) -> Result<()> {
    let mut out = String::from("# kind\tname\tfn_name\thelper_attrs\n");
    for entry in entries {
        out.push_str(kind_to_str(entry.kind));
        out.push('\t');
        out.push_str(&entry.name);
        out.push('\t');
        out.push_str(&entry.fn_name);
        out.push('\t');
        out.push_str(&entry.helper_attrs.join(","));
        out.push('\n');
    }
    fs::write(cache_path(output_path), out).context("failed to write proc-macro transform cache")
}

fn fnv1a64_update(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn collect_files_recursive(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("failed reading {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        let meta = fs::metadata(&path)?;
        if meta.is_dir() {
            collect_files_recursive(root, &path, files)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(root)
                .ok()
                .unwrap_or(path.as_path())
                .to_string_lossy();
            if rel.ends_with(TRANSFORM_CACHE_FILE) || rel.ends_with(TRANSFORM_SOURCE_HASH_FILE) {
                continue;
            }
            files.push(path);
        }
    }
    Ok(())
}

fn compute_tree_fingerprint(root: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect_files_recursive(root, root, &mut files)?;
    files.sort_by(|a, b| {
        let ar = a
            .strip_prefix(root)
            .ok()
            .unwrap_or(a.as_path())
            .to_string_lossy();
        let br = b
            .strip_prefix(root)
            .ok()
            .unwrap_or(b.as_path())
            .to_string_lossy();
        ar.cmp(&br)
    });

    let mut hash = 0xcbf29ce484222325_u64;
    hash = fnv1a64_update(hash, TRANSFORM_VERSION.as_bytes());
    hash = fnv1a64_update(hash, b"\0");
    for file in files {
        let rel = file
            .strip_prefix(root)
            .ok()
            .unwrap_or(file.as_path())
            .to_string_lossy();
        hash = fnv1a64_update(hash, rel.as_bytes());
        hash = fnv1a64_update(hash, b"\0");
        let bytes = fs::read(&file)?;
        hash = fnv1a64_update(hash, &bytes);
        hash = fnv1a64_update(hash, b"\0");
    }
    Ok(format!("{hash:016x}"))
}

fn transform_lib_rs_content(
    content: &str,
    version_info: &PackageVersionInfo,
) -> Result<(String, Vec<ProcMacroEntry>)> {
    let lines: Vec<&str> = content.lines().collect();
    let mut output = String::new();
    let mut entries = Vec::new();
    let mut normalize_helper_emitted = false;
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.starts_with("#[proc_macro_derive") {
            let (full_attr, end_line) = collect_multiline_attr(&lines, i);
            if let Some((macro_name, helper_attrs)) = extract_derive_info(&full_attr) {
                if let Some((_fn_name, fn_line)) = find_fn_after_attr(&lines, end_line) {
                    let watt_fn_name = format!("derive_{}", to_snake_case(&macro_name));
                    let param_name = extract_single_param_name(lines[fn_line])
                        .unwrap_or_else(|| "input".to_string());
                    emit_normalize_helper(&mut output, &mut normalize_helper_emitted);
                    emit_exported_wrapper(
                        &mut output,
                        version_info,
                        &watt_fn_name,
                        &[param_name.as_str()],
                    );
                    emit_inner_signature(&mut output, &watt_fn_name, &[param_name.as_str()]);
                    entries.push(ProcMacroEntry {
                        kind: ProcMacroKind::Derive,
                        name: macro_name,
                        fn_name: watt_fn_name,
                        helper_attrs,
                    });
                    i = fn_line + 1;
                    continue;
                }
            }
        } else if trimmed == "#[proc_macro_attribute]" {
            if let Some((fn_name, fn_line)) = find_fn_after_attr(&lines, i) {
                let (attr_param, item_param) = extract_two_param_names(lines[fn_line])
                    .unwrap_or_else(|| ("attr".to_string(), "item".to_string()));
                emit_normalize_helper(&mut output, &mut normalize_helper_emitted);
                emit_exported_wrapper(
                    &mut output,
                    version_info,
                    &fn_name,
                    &[attr_param.as_str(), item_param.as_str()],
                );
                emit_inner_signature(
                    &mut output,
                    &fn_name,
                    &[attr_param.as_str(), item_param.as_str()],
                );
                entries.push(ProcMacroEntry {
                    kind: ProcMacroKind::Attribute,
                    name: fn_name.clone(),
                    fn_name,
                    helper_attrs: Vec::new(),
                });
                i = fn_line + 1;
                continue;
            }
        } else if trimmed == "#[proc_macro]" {
            if let Some((fn_name, fn_line)) = find_fn_after_attr(&lines, i) {
                let param_name = extract_single_param_name(lines[fn_line])
                    .unwrap_or_else(|| "input".to_string());
                emit_normalize_helper(&mut output, &mut normalize_helper_emitted);
                emit_exported_wrapper(&mut output, version_info, &fn_name, &[param_name.as_str()]);
                emit_inner_signature(&mut output, &fn_name, &[param_name.as_str()]);
                entries.push(ProcMacroEntry {
                    kind: ProcMacroKind::FunctionLike,
                    name: fn_name.clone(),
                    fn_name,
                    helper_attrs: Vec::new(),
                });
                i = fn_line + 1;
                continue;
            }
        }

        if trimmed.starts_with("extern crate proc_macro") && !trimmed.contains("proc_macro2") {
            i += 1;
            continue;
        }

        let transformed = transform_line(line, version_info);
        if !transformed.is_empty() {
            output.push_str(&transformed);
        }
        output.push('\n');
        i += 1;
    }

    Ok((output, entries))
}

fn emit_normalize_helper(output: &mut String, emitted: &mut bool) {
    if *emitted {
        return;
    }
    output.push_str(
        "#[allow(dead_code)]\n\
fn __cargo_wasm_normalize_token_stream(stream: proc_macro2::TokenStream) -> proc_macro2::TokenStream {\n\
    stream.to_string().parse().unwrap()\n\
}\n\n",
    );
    *emitted = true;
}

fn emit_exported_wrapper(
    output: &mut String,
    version_info: &PackageVersionInfo,
    exported_name: &str,
    params: &[&str],
) {
    output.push_str(version_info.no_mangle_attr());
    output.push('\n');
    output.push_str(&format!(
        "pub extern \"C\" fn {}({}) -> proc_macro2::TokenStream {{\n",
        exported_name,
        proc_macro_params(params),
    ));
    output.push_str("    __cargo_wasm_normalize_token_stream(");
    output.push_str(&inner_fn_name(exported_name));
    output.push('(');
    output.push_str(&params.join(", "));
    output.push_str("))\n");
    output.push_str("}\n");
}

fn emit_inner_signature(output: &mut String, exported_name: &str, params: &[&str]) {
    output.push_str(&format!(
        "fn {}({}) -> proc_macro2::TokenStream {{\n",
        inner_fn_name(exported_name),
        proc_macro_params(params),
    ));
}

fn proc_macro_params(params: &[&str]) -> String {
    params
        .iter()
        .map(|name| format!("{name}: proc_macro2::TokenStream"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn inner_fn_name(exported_name: &str) -> String {
    format!("__cargo_wasm_inner_{exported_name}")
}

fn collect_multiline_attr(lines: &[&str], start: usize) -> (String, usize) {
    let mut attr = String::new();
    let mut bracket_count = 0;
    let mut end_line = start;

    for (j, line) in lines.iter().enumerate().skip(start) {
        let line = line.trim();
        attr.push_str(line);
        attr.push(' ');
        bracket_count += line.matches('[').count();
        bracket_count -= line.matches(']').count();
        if bracket_count == 0 && j > start {
            end_line = j;
            break;
        }
        if line.ends_with(")]") {
            end_line = j;
            break;
        }
        end_line = j;
    }

    (attr, end_line)
}

fn extract_derive_info(line: &str) -> Option<(String, Vec<String>)> {
    let start = line.find('(')? + 1;
    let rest = &line[start..];
    let name_end = rest.find([',', ')']).unwrap_or(rest.len());
    let name = rest[..name_end].trim().to_string();
    let mut helper_attrs = Vec::new();
    if let Some(attr_start) = rest.find("attributes(") {
        let attr_content_start = attr_start + "attributes(".len();
        if let Some(attr_end) = rest[attr_content_start..].find(')') {
            let attrs_str = &rest[attr_content_start..attr_content_start + attr_end];
            helper_attrs.extend(
                attrs_str
                    .split(',')
                    .map(str::trim)
                    .filter(|attr| !attr.is_empty())
                    .map(str::to_string),
            );
        }
    }
    Some((name, helper_attrs))
}

fn find_fn_after_attr(lines: &[&str], attr_line: usize) -> Option<(String, usize)> {
    for (i, line) in lines.iter().enumerate().skip(attr_line + 1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with("#[") {
            continue;
        }
        if line.starts_with("pub fn ")
            || line.starts_with("pub(crate) fn ")
            || line.contains(" fn ") && line.contains("pub")
        {
            if let Some(fn_name) = extract_fn_name(line) {
                return Some((fn_name, i));
            }
        }
        break;
    }
    None
}

fn extract_fn_name(line: &str) -> Option<String> {
    let fn_pos = line.find("fn ")?;
    let after_fn = &line[fn_pos + 3..];
    let end = after_fn
        .find(|c: char| c == '(' || c == '<' || c.is_whitespace())
        .unwrap_or(after_fn.len());
    Some(after_fn[..end].to_string())
}

fn extract_single_param_name(fn_line: &str) -> Option<String> {
    let start = fn_line.find('(')?;
    let end = fn_line[start + 1..].find(')')?;
    let params = &fn_line[start + 1..start + 1 + end];
    let first = params.split(',').next()?.trim();
    let name = first.split(':').next()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(name.split_whitespace().last()?.to_string())
}

fn extract_two_param_names(fn_line: &str) -> Option<(String, String)> {
    let start = fn_line.find('(')?;
    let end = fn_line[start + 1..].find(')')?;
    let params = &fn_line[start + 1..start + 1 + end];
    let mut iter = params.split(',').map(str::trim);
    let first = iter.next()?;
    let second = iter.next()?;
    let first_name = first
        .split(':')
        .next()?
        .trim()
        .split_whitespace()
        .last()?
        .to_string();
    let second_name = second
        .split(':')
        .next()?
        .trim()
        .split_whitespace()
        .last()?
        .to_string();
    Some((first_name, second_name))
}

fn transform_line(line: &str, version_info: &PackageVersionInfo) -> String {
    let trimmed = line.trim();
    if trimmed == "use syn::parse_macro_input;" {
        return String::new();
    }

    let mut result = line
        .replace("use proc_macro::", "use proc_macro2::")
        .replace("proc_macro::TokenStream", "proc_macro2::TokenStream")
        .replace(
            "env!(\"CARGO_PKG_VERSION_PATCH\")",
            &format!("\"{}\"", version_info.patch),
        )
        .replace(
            "env!(\"CARGO_PKG_VERSION_MINOR\")",
            &format!("\"{}\"", version_info.minor),
        )
        .replace(
            "env!(\"CARGO_PKG_VERSION_MAJOR\")",
            &format!("\"{}\"", version_info.major),
        )
        .replace(
            "env!(\"CARGO_PKG_VERSION\")",
            &format!("\"{}\"", version_info.version),
        );

    if result.contains("use syn::{") && result.contains("parse_macro_input") {
        result = result.replace("parse_macro_input, ", "");
        result = result.replace(", parse_macro_input", "");
        result = result.replace("parse_macro_input", "");
    }
    if result.contains("parse_macro_input!") {
        result = transform_parse_macro_input(&result);
    }

    result
}

fn transform_parse_macro_input(line: &str) -> String {
    let mut result = line.to_string();
    while let Some(start) = result.find("parse_macro_input!(") {
        let replacement_start = if result[..start].ends_with("syn::") {
            start - "syn::".len()
        } else {
            start
        };
        let after_start = &result[start + "parse_macro_input!(".len()..];
        let mut depth = 1;
        let mut end_offset = 0;
        for (i, ch) in after_start.chars().enumerate() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end_offset = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            break;
        }
        let macro_content = &after_start[..end_offset];
        let macro_end = start + "parse_macro_input!(".len() + end_offset + 1;
        let replacement = if let Some(as_pos) = macro_content.find(" as ") {
            let var_name = macro_content[..as_pos].trim();
            let type_name = macro_content[as_pos + 4..].trim();
            format!("syn::parse2::<{}>({}).unwrap()", type_name, var_name)
        } else if let Some(type_info) = extract_type_from_let_binding(&result[..start]) {
            let var_name = macro_content.trim();
            format!("syn::parse2::<{}>({}).unwrap()", type_info, var_name)
        } else {
            let var_name = macro_content.trim();
            format!("syn::parse2({}).unwrap()", var_name)
        };
        result = format!(
            "{}{}{}",
            &result[..replacement_start],
            replacement,
            &result[macro_end..]
        );
    }
    result
}

fn transform_wstd_attrs_content(content: &str) -> Result<Option<String>> {
    let mut transformed = content.to_string();
    let mut changed = false;

    while let Some(pos) = transformed.find("#[wstd::http_server]") {
        transformed = replace_wstd_attr(&transformed, pos, WstdAttrKind::HttpServer)?;
        changed = true;
    }

    while let Some(pos) = transformed.find("#[wstd::main]") {
        transformed = replace_wstd_attr(&transformed, pos, WstdAttrKind::Main)?;
        changed = true;
    }

    Ok(changed.then_some(transformed))
}

#[derive(Clone, Copy)]
enum WstdAttrKind {
    HttpServer,
    Main,
}

impl WstdAttrKind {
    fn attr_text(self) -> &'static str {
        match self {
            WstdAttrKind::HttpServer => "#[wstd::http_server]",
            WstdAttrKind::Main => "#[wstd::main]",
        }
    }
}

fn replace_wstd_attr(content: &str, attr_pos: usize, kind: WstdAttrKind) -> Result<String> {
    let attr_end = attr_pos + kind.attr_text().len();
    let fn_start = skip_whitespace(content, attr_end);
    let brace_pos = content[fn_start..]
        .find('{')
        .map(|idx| fn_start + idx)
        .ok_or_else(|| anyhow::anyhow!("failed to find body for {}", kind.attr_text()))?;
    let body_end = find_matching_brace(content, brace_pos)
        .ok_or_else(|| anyhow::anyhow!("failed to parse body for {}", kind.attr_text()))?;
    let signature = content[fn_start..brace_pos].trim();
    let body = &content[brace_pos + 1..body_end];
    let parsed = parse_main_signature(signature)
        .with_context(|| format!("failed to parse signature for {}", kind.attr_text()))?;
    let expansion = match kind {
        WstdAttrKind::HttpServer => expand_wstd_http_server(&parsed, body),
        WstdAttrKind::Main => expand_wstd_main(&parsed, body)?,
    };

    let mut output = String::new();
    output.push_str(&content[..attr_pos]);
    output.push_str(&expansion);
    output.push_str(&content[body_end + 1..]);
    Ok(output)
}

struct MainSignature<'a> {
    vis: String,
    is_async: bool,
    inputs: &'a str,
    output: &'a str,
}

fn parse_main_signature(signature: &str) -> Option<MainSignature<'_>> {
    let fn_pos = signature.find("fn main")?;
    let prefix = signature[..fn_pos].trim();
    let is_async = prefix.split_whitespace().any(|part| part == "async");
    let vis = prefix
        .split_whitespace()
        .filter(|part| *part != "async")
        .collect::<Vec<_>>()
        .join(" ");
    let open_paren = signature[fn_pos..].find('(')? + fn_pos;
    let close_paren = find_matching_paren(signature, open_paren)?;
    Some(MainSignature {
        vis,
        is_async,
        inputs: signature[open_paren + 1..close_paren].trim(),
        output: signature[close_paren + 1..].trim(),
    })
}

fn expand_wstd_http_server(sig: &MainSignature<'_>, body: &str) -> String {
    let run_async = sig.is_async.then_some("async ").unwrap_or("");
    let run_await = sig.is_async.then_some(".await").unwrap_or("");
    let vis = if sig.vis.is_empty() {
        String::new()
    } else {
        format!("{} ", sig.vis)
    };
    let output = if sig.output.is_empty() {
        String::new()
    } else {
        format!(" {}", sig.output)
    };
    format!(
        r#"struct TheServer;

impl ::wstd::__internal::wasip2::exports::http::incoming_handler::Guest for TheServer {{
    fn handle(
        request: ::wstd::__internal::wasip2::http::types::IncomingRequest,
        response_out: ::wstd::__internal::wasip2::http::types::ResponseOutparam,
    ) {{
        {vis}{run_async}fn __run({inputs}){output} {{
{body}
        }}

        let responder = ::wstd::http::server::Responder::new(response_out);
        ::wstd::runtime::block_on(async move {{
            match ::wstd::http::request::try_from_incoming(request) {{
                Ok(request) => match __run(request){run_await} {{
                    Ok(response) => {{ responder.respond(response).await.unwrap() }}
                    Err(err) => responder.fail(err),
                }}
                Err(err) => responder.fail(err),
            }}
        }})
    }}
}}

::wstd::__internal::wasip2::http::proxy::export!(
    TheServer with_types_in ::wstd::__internal::wasip2
);

fn main() {{
    unreachable!("HTTP server components should be run with `handle` rather than `run`")
}}"#,
        vis = vis,
        run_async = run_async,
        inputs = sig.inputs,
        output = output,
        body = body,
        run_await = run_await,
    )
}

fn expand_wstd_main(sig: &MainSignature<'_>, body: &str) -> Result<String> {
    if !sig.inputs.is_empty() {
        anyhow::bail!("arguments to #[wstd::main] are not supported");
    }
    if !sig.is_async {
        anyhow::bail!("#[wstd::main] requires async fn main");
    }
    let vis = if sig.vis.is_empty() {
        String::new()
    } else {
        format!("{} ", sig.vis)
    };
    let output = if sig.output.is_empty() {
        String::new()
    } else {
        format!(" {}", sig.output)
    };
    Ok(format!(
        r#"{vis}fn main(){output} {{
    async fn __run(){output} {{
{body}
    }}

    ::wstd::runtime::block_on(async {{
        __run().await
    }})
}}"#,
        vis = vis,
        output = output,
        body = body,
    ))
}

fn skip_whitespace(content: &str, mut pos: usize) -> usize {
    while let Some(ch) = content[pos..].chars().next() {
        if !ch.is_whitespace() {
            break;
        }
        pos += ch.len_utf8();
    }
    pos
}

fn find_matching_paren(content: &str, open_pos: usize) -> Option<usize> {
    find_matching_delimiter(content, open_pos, '(', ')')
}

fn find_matching_brace(content: &str, open_pos: usize) -> Option<usize> {
    find_matching_delimiter(content, open_pos, '{', '}')
}

fn find_matching_delimiter(
    content: &str,
    open_pos: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut pos = open_pos;
    let mut depth = 0usize;
    let mut in_line_comment = false;
    let mut in_block_comment = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escape = false;

    while pos < bytes.len() {
        let ch = content[pos..].chars().next()?;
        let next_pos = pos + ch.len_utf8();
        let next = content[next_pos..].chars().next();

        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            pos = next_pos;
            continue;
        }
        if in_block_comment > 0 {
            if ch == '/' && next == Some('*') {
                in_block_comment += 1;
                pos = next_pos + 1;
                continue;
            }
            if ch == '*' && next == Some('/') {
                in_block_comment -= 1;
                pos = next_pos + 1;
                continue;
            }
            pos = next_pos;
            continue;
        }
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            pos = next_pos;
            continue;
        }
        if in_char {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '\'' {
                in_char = false;
            }
            pos = next_pos;
            continue;
        }

        if ch == '/' && next == Some('/') {
            in_line_comment = true;
            pos = next_pos + 1;
            continue;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = 1;
            pos = next_pos + 1;
            continue;
        }
        if ch == '"' {
            in_string = true;
            pos = next_pos;
            continue;
        }
        if ch == '\'' {
            in_char = true;
            pos = next_pos;
            continue;
        }

        if ch == open {
            depth += 1;
        } else if ch == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(pos);
            }
        }
        pos = next_pos;
    }
    None
}

fn extract_type_from_let_binding(s: &str) -> Option<String> {
    let trimmed = s.trim_end();
    if !trimmed.ends_with('=') {
        return None;
    }
    let before_eq = trimmed[..trimmed.len() - 1].trim_end();
    let colon_pos = before_eq.rfind(':')?;
    let type_str = before_eq[colon_pos + 1..].trim();
    (!type_str.is_empty()).then(|| type_str.to_string())
}

fn generate_metadata(entries: &[ProcMacroEntry]) -> Vec<u8> {
    let mut metadata = String::new();
    for entry in entries {
        metadata.push_str(entry.kind.metadata_prefix());
        metadata.push(':');
        metadata.push_str(&entry.name);
        metadata.push(':');
        metadata.push_str(&entry.fn_name);
        if !entry.helper_attrs.is_empty() {
            metadata.push(':');
            metadata.push_str(&entry.helper_attrs.join(","));
        }
        metadata.push('\n');
    }
    metadata.into_bytes()
}

fn generate_proc_macro_custom_section(entries: &[ProcMacroEntry]) -> Vec<u8> {
    let metadata = generate_metadata(entries);
    let section_name = b".rustc_proc_macro_decls";
    let mut custom_section = vec![0];
    let name_len = encode_varuint32(section_name.len() as u32);
    let section_size = name_len.len() + section_name.len() + metadata.len();
    custom_section.extend_from_slice(&encode_varuint32(section_size as u32));
    custom_section.extend_from_slice(&name_len);
    custom_section.extend_from_slice(section_name);
    custom_section.extend_from_slice(&metadata);
    custom_section
}

fn encode_varuint32(mut value: u32) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
    out
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::new();
    for (idx, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if idx > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}
