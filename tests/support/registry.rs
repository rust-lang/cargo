use std::io::{mod, fs, File};

use flate2::Default;
use flate2::writer::GzEncoder;
use git2;
use serialize::hex::ToHex;
use tar::Archive;
use url::Url;

use support::{ResultTest, project};
use support::paths;
use support::git::repo;
use cargo::util::Sha256;

pub fn registry_path() -> Path { paths::root().join("registry") }
pub fn registry() -> Url { Url::from_file_path(&registry_path()).unwrap() }
pub fn dl_path() -> Path { paths::root().join("dl") }
pub fn dl_url() -> Url { Url::from_file_path(&dl_path()).unwrap() }

pub fn init() {
    let config = paths::home().join(".cargo/config");
    fs::mkdir_recursive(&config.dir_path(), io::USER_DIR).assert();
    File::create(&config).write_str(format!(r#"
        [registry]
            index = "{reg}"
            token = "api-token"
    "#, reg = registry()).as_slice()).assert();

    // Init a new registry
    repo(&registry_path())
        .file("config.json", format!(r#"
            {{"dl":"{}","api":""}}
        "#, dl_url()).as_slice())
        .build();
}

pub fn mock_archive(name: &str, version: &str, deps: &[(&str, &str)]) {
    let mut manifest = format!(r#"
        [package]
        name = "{}"
        version = "{}"
        authors = []
    "#, name, version);
    for &(dep, req) in deps.iter() {
        manifest.push_str(format!(r#"
            [dependencies.{}]
            version = "{}"
        "#, dep, req).as_slice());
    }
    let p = project(name)
        .file("Cargo.toml", manifest.as_slice())
        .file("src/lib.rs", "");
    p.build();

    let dst = mock_archive_dst(name, version);
    fs::mkdir_recursive(&dst.dir_path(), io::USER_DIR).assert();
    let f = File::create(&dst).unwrap();
    let a = Archive::new(GzEncoder::new(f, Default));
    a.append(format!("{}-{}/Cargo.toml", name, version).as_slice(),
             &mut File::open(&p.root().join("Cargo.toml")).unwrap()).unwrap();
    a.append(format!("{}-{}/src/lib.rs", name, version).as_slice(),
             &mut File::open(&p.root().join("src/lib.rs")).unwrap()).unwrap();
    a.finish().unwrap();
}

pub fn mock_archive_dst(name: &str, version: &str) -> Path {
    dl_path().join(name).join(version).join("download")
}

pub fn mock_pkg(name: &str, version: &str, deps: &[(&str, &str)]) {
    mock_pkg_yank(name, version, deps, false)
}

pub fn mock_pkg_yank(name: &str, version: &str, deps: &[(&str, &str)],
                     yanked: bool) {
    mock_archive(name, version, deps);
    let c = File::open(&mock_archive_dst(name, version)).read_to_end().unwrap();
    let line = pkg(name, version, deps, cksum(c.as_slice()).as_slice(), yanked);

    let file = match name.len() {
        1 => format!("1/{}", name),
        2 => format!("2/{}", name),
        3 => format!("3/{}/{}", name.slice_to(1), name),
        _ => format!("{}/{}/{}", name.slice(0, 2), name.slice(2, 4), name),
    };
    publish(file.as_slice(), line.as_slice());
}

pub fn publish(file: &str, line: &str) {
    let repo = git2::Repository::open(&registry_path()).unwrap();
    let mut index = repo.index().unwrap();
    {
        let dst = registry_path().join(file);
        let prev = File::open(&dst).read_to_string().unwrap_or(String::new());
        fs::mkdir_recursive(&dst.dir_path(), io::USER_DIR).unwrap();
        File::create(&dst).write_str((prev + line + "\n").as_slice()).unwrap();
    }
    index.add_path(&Path::new(file)).unwrap();
    index.write().unwrap();
    let id = index.write_tree().unwrap();
    let tree = repo.find_tree(id).unwrap();
    let sig = repo.signature().unwrap();
    let parent = repo.refname_to_id("refs/heads/master").unwrap();
    let parent = repo.find_commit(parent).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig,
                "Another commit", &tree,
                [&parent]).unwrap();
}

pub fn pkg(name: &str, vers: &str, deps: &[(&str, &str)], cksum: &str,
           yanked: bool) -> String {
    let deps = deps.iter().map(|&(a, b)| dep(a, b)).collect::<Vec<String>>();
    format!("{{\"name\":\"{}\",\"vers\":\"{}\",\
               \"deps\":{},\"cksum\":\"{}\",\"features\":{{}},\
               \"yanked\":{}}}",
            name, vers, deps, cksum, yanked)
}

pub fn dep(name: &str, req: &str) -> String {
    format!("{{\"name\":\"{}\",\
               \"req\":\"{}\",\
               \"features\":[],\
               \"default_features\":false,\
               \"target\":null,\
               \"optional\":false}}", name, req)
}

pub fn cksum(s: &[u8]) -> String {
    let mut sha = Sha256::new();
    sha.update(s);
    sha.finish().to_hex()
}
