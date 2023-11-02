use cargo::Config;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use url::Url;

#[macro_export]
macro_rules! fixtures {
    () => {
        $crate::Fixtures::new(env!("CARGO_TARGET_TMPDIR"))
    };
}

// This is an arbitrary commit that existed when I started. This helps
// ensure consistent results. It can be updated if needed, but that can
// make it harder to compare results with older versions of cargo.
const CRATES_IO_COMMIT: &str = "85f7bfd61ea4fee08ec68c468762e886b2aebec6";

pub struct Fixtures {
    cargo_target_tmpdir: PathBuf,
}

impl Fixtures {
    pub fn new(cargo_target_tmpdir: &str) -> Self {
        let bench = Self {
            cargo_target_tmpdir: PathBuf::from(cargo_target_tmpdir),
        };
        bench.create_home();
        bench.create_target_dir();
        bench.clone_index();
        bench.unpack_workspaces();
        bench
    }

    fn root(&self) -> PathBuf {
        self.cargo_target_tmpdir.join("bench")
    }

    fn target_dir(&self) -> PathBuf {
        let mut p = self.root();
        p.push("target");
        p
    }

    fn cargo_home(&self) -> PathBuf {
        let mut p = self.root();
        p.push("chome");
        p
    }

    fn index(&self) -> PathBuf {
        let mut p = self.root();
        p.push("index");
        p
    }

    fn workspaces_path(&self) -> PathBuf {
        let mut p = self.root();
        p.push("workspaces");
        p
    }

    fn registry_url(&self) -> Url {
        Url::from_file_path(self.index()).unwrap()
    }

    fn create_home(&self) {
        let home = self.cargo_home();
        if !home.exists() {
            fs::create_dir_all(&home).unwrap();
        }
        fs::write(
            home.join("config.toml"),
            format!(
                r#"
                [source.crates-io]
                replace-with = 'local-snapshot'

                [source.local-snapshot]
                registry = '{}'
            "#,
                self.registry_url()
            ),
        )
        .unwrap();
    }

    fn create_target_dir(&self) {
        // This is necessary to ensure the .rustc_info.json file is written.
        // Otherwise it won't be written, and it is very expensive to create.
        if !self.target_dir().exists() {
            fs::create_dir_all(self.target_dir()).unwrap();
        }
    }

    /// This clones crates.io at a specific point in time into tmp/index.
    fn clone_index(&self) {
        let index = self.index();
        let maybe_git = |command: &str| {
            let status = Command::new("git")
                .current_dir(&index)
                .args(command.split_whitespace().collect::<Vec<_>>())
                .status()
                .expect("git should be installed");
            status.success()
        };
        let git = |command: &str| {
            if !maybe_git(command) {
                panic!("failed to run git command: {}", command);
            }
        };
        if index.exists() {
            if maybe_git(&format!(
                "rev-parse -q --verify {}^{{commit}}",
                CRATES_IO_COMMIT
            )) {
                // Already fetched.
                return;
            }
        } else {
            fs::create_dir_all(&index).unwrap();
            git("init --bare");
            git("remote add origin https://github.com/rust-lang/crates.io-index-archive");
        }
        git(&format!("fetch origin {}", CRATES_IO_COMMIT));
        git("branch -f master FETCH_HEAD");
    }

    /// This unpacks the compressed workspace skeletons into tmp/workspaces.
    fn unpack_workspaces(&self) {
        let ws_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("workspaces");
        let archives = fs::read_dir(ws_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension() == Some(std::ffi::OsStr::new("tgz")));
        for archive in archives {
            let name = archive.file_stem().unwrap();
            let f = fs::File::open(&archive).unwrap();
            let f = flate2::read::GzDecoder::new(f);
            let dest = self.workspaces_path().join(&name);
            if dest.exists() {
                fs::remove_dir_all(&dest).unwrap();
            }
            let mut archive = tar::Archive::new(f);
            archive.unpack(self.workspaces_path()).unwrap();
        }
    }

    /// Vec of `(ws_name, ws_root)`.
    pub fn workspaces(&self) -> Vec<(String, PathBuf)> {
        // CARGO_BENCH_WORKSPACES can be used to override, otherwise it just uses
        // the workspaces in the workspaces directory.
        let mut ps: Vec<_> = match std::env::var_os("CARGO_BENCH_WORKSPACES") {
            Some(s) => std::env::split_paths(&s).collect(),
            None => fs::read_dir(self.workspaces_path())
                .unwrap()
                .map(|e| e.unwrap().path())
                // These currently fail in most cases on Windows due to long
                // filenames in the git checkouts.
                .filter(|p| {
                    !(cfg!(windows)
                        && matches!(p.file_name().unwrap().to_str().unwrap(), "servo" | "tikv"))
                })
                .collect(),
        };
        // Sort so it is consistent.
        ps.sort();
        ps.into_iter()
            .map(|p| (p.file_name().unwrap().to_str().unwrap().to_owned(), p))
            .collect()
    }

    /// Creates a new Config.
    pub fn make_config(&self, ws_root: &Path) -> Config {
        let shell = cargo::core::Shell::new();
        let mut config = Config::new(shell, ws_root.to_path_buf(), self.cargo_home());
        // Configure is needed to set the target_dir which is needed to write
        // the .rustc_info.json file which is very expensive.
        config
            .configure(
                0,
                false,
                None,
                false,
                false,
                false,
                &Some(self.target_dir()),
                &[],
                &[],
            )
            .unwrap();
        config
    }
}
