use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{PathBuf, Path};

use flate2::Compression::Default;
use flate2::write::GzEncoder;
use git2;
use rustc_serialize::hex::ToHex;
use tar::Archive;
use url::Url;

use support::project;
use support::paths;
use support::git::repo;
use cargo::util::Sha256;

pub fn registry_path() -> PathBuf { paths::root().join("registry") }
pub fn registry() -> Url { Url::from_file_path(&*registry_path()).ok().unwrap() }
pub fn dl_path() -> PathBuf { paths::root().join("dl") }
pub fn dl_url() -> Url { Url::from_file_path(&*dl_path()).ok().unwrap() }

pub fn init() {
    let config = paths::home().join(".cargo/config");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    File::create(&config).unwrap().write_all(format!(r#"
        [registry]
            index = "{reg}"
            token = "api-token"
    "#, reg = registry()).as_bytes()).unwrap();

    // Init a new registry
    repo(&registry_path())
        .file("config.json", &format!(r#"
            {{"dl":"{}","api":""}}
        "#, dl_url()))
        .build();
}

pub fn mock_archive(name: &str, version: &str, deps: &[(&str, &str, &str)]) {
    let mut manifest = format!(r#"
        [package]
        name = "{}"
        version = "{}"
        authors = []
    "#, name, version);
    for &(dep, req, kind) in deps.iter() {
        manifest.push_str(&format!(r#"
            [{}dependencies.{}]
            version = "{}"
        "#, match kind {
            "build" => "build-",
            "dev" => "dev-",
            _ => ""
        }, dep, req));
    }
    let p = project(name)
        .file("Cargo.toml", &manifest)
        .file("src/lib.rs", "");
    p.build();

    let dst = mock_archive_dst(name, version);
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let f = File::create(&dst).unwrap();
    let a = Archive::new(GzEncoder::new(f, Default));
    a.append(&format!("{}-{}/Cargo.toml", name, version),
             &mut File::open(&p.root().join("Cargo.toml")).unwrap()).unwrap();
    a.append(&format!("{}-{}/src/lib.rs", name, version),
             &mut File::open(&p.root().join("src/lib.rs")).unwrap()).unwrap();
    a.finish().unwrap();
}

pub fn mock_archive_dst(name: &str, version: &str) -> PathBuf {
    dl_path().join(name).join(version).join("download")
}

pub fn mock_pkg(name: &str, version: &str, deps: &[(&str, &str, &str)]) {
    mock_pkg_yank(name, version, deps, false)
}

pub fn mock_pkg_yank(name: &str, version: &str, deps: &[(&str, &str, &str)],
                     yanked: bool) {
    mock_archive(name, version, deps);
    let mut c = Vec::new();
    File::open(&mock_archive_dst(name, version)).unwrap()
         .read_to_end(&mut c).unwrap();
    let line = pkg(name, version, deps, &cksum(&c), yanked);

    let file = match name.len() {
        1 => format!("1/{}", name),
        2 => format!("2/{}", name),
        3 => format!("3/{}/{}", &name[..1], name),
        _ => format!("{}/{}/{}", &name[0..2], &name[2..4], name),
    };
    publish(&file, &line);
}

pub fn publish(file: &str, line: &str) {
    let repo = git2::Repository::open(&registry_path()).unwrap();
    let mut index = repo.index().unwrap();
    {
        let dst = registry_path().join(file);
        let mut prev = String::new();
        let _ = File::open(&dst).and_then(|mut f| f.read_to_string(&mut prev));
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        File::create(&dst).unwrap()
            .write_all((prev + line + "\n").as_bytes()).unwrap();
    }
    index.add_path(Path::new(file)).unwrap();
    index.write().unwrap();
    let id = index.write_tree().unwrap();
    let tree = repo.find_tree(id).unwrap();
    let sig = repo.signature().unwrap();
    let parent = repo.refname_to_id("refs/heads/master").unwrap();
    let parent = repo.find_commit(parent).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig,
                "Another commit", &tree,
                &[&parent]).unwrap();
}

pub fn pkg(name: &str, vers: &str, deps: &[(&str, &str, &str)], cksum: &str,
           yanked: bool) -> String {
    let deps = deps.iter().map(|&(a, b, c)| dep(a, b, c)).collect::<Vec<String>>();
    format!("{{\"name\":\"{}\",\"vers\":\"{}\",\
               \"deps\":[{}],\"cksum\":\"{}\",\"features\":{{}},\
               \"yanked\":{}}}",
            name, vers, deps.connect(","), cksum, yanked)
}

pub fn dep(name: &str, req: &str, kind: &str) -> String {
    format!("{{\"name\":\"{}\",\
               \"req\":\"{}\",\
               \"features\":[],\
               \"default_features\":false,\
               \"target\":null,\
               \"optional\":false,\
               \"kind\":\"{}\"}}", name, req, kind)
}

pub fn cksum(s: &[u8]) -> String {
    let mut sha = Sha256::new();
    sha.update(s);
    sha.finish().to_hex()
}
