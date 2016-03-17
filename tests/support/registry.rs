use std::collections::HashMap;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{PathBuf, Path};

use flate2::Compression::Default;
use flate2::write::GzEncoder;
use git2;
use rustc_serialize::hex::ToHex;
use rustc_serialize::json::ToJson;
use tar::{Builder, Header};
use url::Url;

use support::paths;
use support::git::repo;
use cargo::util::Sha256;

pub fn registry_path() -> PathBuf { paths::root().join("registry") }
pub fn registry() -> Url { Url::from_file_path(&*registry_path()).ok().unwrap() }
pub fn dl_path() -> PathBuf { paths::root().join("dl") }
pub fn dl_url() -> Url { Url::from_file_path(&*dl_path()).ok().unwrap() }

pub struct Package {
    name: String,
    vers: String,
    deps: Vec<Dependency>,
    files: Vec<(String, String)>,
    yanked: bool,
    features: HashMap<String, Vec<String>>,
}

struct Dependency {
    name: String,
    vers: String,
    kind: String,
    target: Option<String>,
    features: Vec<String>,
}

fn init() {
    let config = paths::home().join(".cargo/config");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    if fs::metadata(&config).is_ok() {
        return
    }
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

impl Package {
    pub fn new(name: &str, vers: &str) -> Package {
        init();
        Package {
            name: name.to_string(),
            vers: vers.to_string(),
            deps: Vec::new(),
            files: Vec::new(),
            yanked: false,
            features: HashMap::new(),
        }
    }

    pub fn file(&mut self, name: &str, contents: &str) -> &mut Package {
        self.files.push((name.to_string(), contents.to_string()));
        self
    }

    pub fn dep(&mut self, name: &str, vers: &str) -> &mut Package {
        self.full_dep(name, vers, None, "normal", &[])
    }

    pub fn feature_dep(&mut self,
                       name: &str,
                       vers: &str,
                       features: &[&str]) -> &mut Package {
        self.full_dep(name, vers, None, "normal", features)
    }

    pub fn target_dep(&mut self,
                      name: &str,
                      vers: &str,
                      target: &str) -> &mut Package {
        self.full_dep(name, vers, Some(target), "normal", &[])
    }

    pub fn dev_dep(&mut self, name: &str, vers: &str) -> &mut Package {
        self.full_dep(name, vers, None, "dev", &[])
    }

    fn full_dep(&mut self,
                name: &str,
                vers: &str,
                target: Option<&str>,
                kind: &str,
                features: &[&str]) -> &mut Package {
        self.deps.push(Dependency {
            name: name.to_string(),
            vers: vers.to_string(),
            kind: kind.to_string(),
            target: target.map(|s| s.to_string()),
            features: features.iter().map(|s| s.to_string()).collect(),
        });
        self
    }

    pub fn yanked(&mut self, yanked: bool) -> &mut Package {
        self.yanked = yanked;
        self
    }

    pub fn publish(&self) {
        self.make_archive();

        // Figure out what we're going to write into the index
        let deps = self.deps.iter().map(|dep| {
            let mut map = HashMap::new();
            map.insert("name".to_string(), dep.name.to_json());
            map.insert("req".to_string(), dep.vers.to_json());
            map.insert("features".to_string(), dep.features.to_json());
            map.insert("default_features".to_string(), false.to_json());
            map.insert("target".to_string(), dep.target.to_json());
            map.insert("optional".to_string(), false.to_json());
            map.insert("kind".to_string(), dep.kind.to_json());
            map
        }).collect::<Vec<_>>();
        let cksum = {
            let mut c = Vec::new();
            File::open(&self.archive_dst()).unwrap()
                 .read_to_end(&mut c).unwrap();
            cksum(&c)
        };
        let mut dep = HashMap::new();
        dep.insert("name".to_string(), self.name.to_json());
        dep.insert("vers".to_string(), self.vers.to_json());
        dep.insert("deps".to_string(), deps.to_json());
        dep.insert("cksum".to_string(), cksum.to_json());
        dep.insert("features".to_string(), self.features.to_json());
        dep.insert("yanked".to_string(), self.yanked.to_json());
        let line = dep.to_json().to_string();

        let file = match self.name.len() {
            1 => format!("1/{}", self.name),
            2 => format!("2/{}", self.name),
            3 => format!("3/{}/{}", &self.name[..1], self.name),
            _ => format!("{}/{}/{}", &self.name[0..2], &self.name[2..4], self.name),
        };

        // Write file/line in the index
        let dst = registry_path().join(&file);
        let mut prev = String::new();
        let _ = File::open(&dst).and_then(|mut f| f.read_to_string(&mut prev));
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        File::create(&dst).unwrap()
             .write_all((prev + &line[..] + "\n").as_bytes()).unwrap();

        // Add the new file to the index
        let repo = git2::Repository::open(&registry_path()).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(&file)).unwrap();
        index.write().unwrap();
        let id = index.write_tree().unwrap();

        // Commit this change
        let tree = repo.find_tree(id).unwrap();
        let sig = repo.signature().unwrap();
        let parent = repo.refname_to_id("refs/heads/master").unwrap();
        let parent = repo.find_commit(parent).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig,
                    "Another commit", &tree,
                    &[&parent]).unwrap();
    }

    fn make_archive(&self) {
        let mut manifest = format!(r#"
            [package]
            name = "{}"
            version = "{}"
            authors = []
        "#, self.name, self.vers);
        for dep in self.deps.iter() {
            let target = match dep.target {
                None => String::new(),
                Some(ref s) => format!("target.{}.", s),
            };
            let kind = match &dep.kind[..] {
                "build" => "build-",
                "dev" => "dev-",
                _ => ""
            };
            manifest.push_str(&format!(r#"
                [{}{}dependencies.{}]
                version = "{}"
            "#, target, kind, dep.name, dep.vers));
        }

        let dst = self.archive_dst();
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        let f = File::create(&dst).unwrap();
        let mut a = Builder::new(GzEncoder::new(f, Default));
        self.append(&mut a, "Cargo.toml", &manifest);
        if self.files.is_empty() {
            self.append(&mut a, "src/lib.rs", "");
        } else {
            for &(ref name, ref contents) in self.files.iter() {
                self.append(&mut a, name, contents);
            }
        }
    }

    fn append<W: Write>(&self, ar: &mut Builder<W>, file: &str, contents: &str) {
        let mut header = Header::new_ustar();
        header.set_size(contents.len() as u64);
        header.set_path(format!("{}-{}/{}", self.name, self.vers, file)).unwrap();
        header.set_cksum();

        ar.append(&header, contents.as_bytes()).unwrap();
    }

    pub fn archive_dst(&self) -> PathBuf {
        dl_path().join(&self.name).join(&self.vers).join("download")
    }
}

fn cksum(s: &[u8]) -> String {
    let mut sha = Sha256::new();
    sha.update(s);
    sha.finish().to_hex()
}
