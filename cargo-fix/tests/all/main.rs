extern crate difference;
extern crate url;

use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;
use std::sync::atomic::*;
use std::time::Instant;

use difference::{Changeset, Difference};
use url::Url;

static CNT: AtomicUsize = ATOMIC_USIZE_INIT;
thread_local!(static IDX: usize = CNT.fetch_add(1, Ordering::SeqCst));

struct ProjectBuilder {
    files: Vec<(String, String)>,
}

struct Project {
    root: PathBuf,
}

fn project() -> ProjectBuilder {
    ProjectBuilder {
        files: Vec::new(),
    }
}

fn root() -> PathBuf {
    let idx = IDX.with(|x| *x);

    let mut me = env::current_exe().unwrap();
    me.pop(); // chop off exe name
    me.pop(); // chop off `deps`
    me.pop(); // chop off `debug` / `release`
    me.push("generated-tests");
    me.push(&format!("test{}", idx));
    return me
}

impl ProjectBuilder {
    fn file(&mut self, name: &str, contents: &str) -> &mut ProjectBuilder {
        self.files.push((name.to_string(), contents.to_string()));
        self
    }

    fn build(&mut self) -> Project {
        if !self.files.iter().any(|f| f.0.ends_with("Cargo.toml")) {
            let manifest = r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [workspace]
            "#;
            self.files.push(("Cargo.toml".to_string(), manifest.to_string()));
        }
        let root = root();
        drop(fs::remove_dir_all(&root));
        for &(ref file, ref contents) in self.files.iter() {
            let dst = root.join(file);
            fs::create_dir_all(dst.parent().unwrap()).unwrap();
            fs::File::create(&dst).unwrap().write_all(contents.as_ref()).unwrap();
        }
        Project { root }
    }
}

impl Project {
    fn expect_cmd<'a>(&'a self, cmd: &'a str) -> ExpectCmd<'a> {
        ExpectCmd {
            project: self,
            cmd: cmd,
            stdout: None,
            stdout_contains: Vec::new(),
            stderr: None,
            stderr_contains: Vec::new(),
            status: 0,
            ran: false,
            cwd: None,
        }
    }

    fn read(&self, path: &str) -> String {
        let mut ret = String::new();
        File::open(self.root.join(path))
            .unwrap()
            .read_to_string(&mut ret)
            .unwrap();
        return ret
    }
}

struct ExpectCmd<'a> {
    ran: bool,
    project: &'a Project,
    cmd: &'a str,
    stdout: Option<String>,
    stdout_contains: Vec<String>,
    stderr: Option<String>,
    stderr_contains: Vec<String>,
    status: i32,
    cwd: Option<PathBuf>,
}

impl<'a> ExpectCmd<'a> {
    fn status(&mut self, status: i32) -> &mut Self {
        self.status = status;
        self
    }

    fn cwd<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.cwd = Some(self.project.root.join(path));
        self
    }

    fn stdout(&mut self, s: &str) -> &mut Self {
        self.stdout = Some(s.to_string());
        self
    }

    fn stderr(&mut self, s: &str) -> &mut Self {
        self.stderr = Some(s.to_string());
        self
    }

    fn stderr_contains(&mut self, s: &str) -> &mut Self {
        self.stderr_contains.push(s.to_string());
        self
    }

    fn run(&mut self) {
        self.ran = true;
        let mut parts = self.cmd.split_whitespace();
        let mut cmd = Command::new(parts.next().unwrap());
        cmd.args(parts);
        match self.cwd {
            Some(ref p) => { cmd.current_dir(p); }
            None => { cmd.current_dir(&self.project.root); }
        }

        let mut me = env::current_exe().unwrap();
        me.pop(); // chop off exe name
        me.pop(); // chop off `deps`

        let mut new_path = Vec::new();
        new_path.push(me);
        new_path.extend(
            env::split_paths(&env::var_os("PATH").unwrap_or(Default::default())),
        );
        cmd.env("PATH", env::join_paths(&new_path).unwrap());

        println!("\n···················································");
        println!("running {:?}", cmd);
        let start = Instant::now();
        let output = match cmd.output() {
            Ok(output) => output,
            Err(err) => panic!("failed to spawn: {}", err),
        };
        let dur = start.elapsed();
        println!("dur: {}.{:03}ms", dur.as_secs(), dur.subsec_nanos() / 1_000_000);
        println!("exit: {}", output.status);
        if output.stdout.len() > 0 {
            println!("stdout ---\n{}", String::from_utf8_lossy(&output.stdout));
        }
        if output.stderr.len() > 0 {
            println!("stderr ---\n{}", String::from_utf8_lossy(&output.stderr));
        }
        println!("···················································");
        let code = match output.status.code() {
            Some(code) => code,
            None => panic!("super extra failure: {}", output.status),
        };
        if code != self.status {
            panic!("expected exit code `{}` got `{}`", self.status, code);
        }
        self.match_std(&output.stdout, &self.stdout, &self.stdout_contains);
        self.match_std(&output.stderr, &self.stderr, &self.stderr_contains);
    }

    fn match_std(&self, actual: &[u8], expected: &Option<String>, contains: &[String]) {
        let actual = match str::from_utf8(actual) {
            Ok(s) => s,
            Err(_) => panic!("std wasn't utf8"),
        };
        let actual = self.clean(actual);
        if let Some(ref expected) = *expected {
            diff(&self.clean(expected), &actual);
        }
        for s in contains {
            let s = self.clean(s);
            if actual.contains(&s) {
                continue
            }
            println!("\nfailed to find contents within output stream\n\
                      expected to find\n  {}\n\nwithin:\n\n{}\n\n",
                     s,
                     actual);
            panic!("test failed");
        }
    }

    fn clean(&self, s: &str) -> String {
        let url = Url::from_file_path(&self.project.root).unwrap();
        let s = s.replace("[CHECKING]", "    Checking")
            .replace("[FINISHED]", "    Finished")
            .replace("[COMPILING]", "   Compiling")
            .replace(&url.to_string(), "CWD")
            .replace(&self.project.root.display().to_string(), "CWD")
            .replace("\\", "/");
        let lines = s.lines()
            .map(|s| {
                let i = match s.find("target(s) in") {
                    Some(i) => i,
                    None => return s.to_string(),
                };
                if s.trim().starts_with("Finished") {
                    s[..i].to_string()
                } else {
                    s.to_string()
                }
            });
        let mut ret = String::new();
        for (i, line) in lines.enumerate() {
            if i != 0 {
                ret.push_str("\n");
            }
            ret.push_str(&line);
        }
        ret
    }
}

impl<'a> Drop for ExpectCmd<'a> {
    fn drop(&mut self) {
        if !self.ran {
            panic!("forgot to run this command");
        }
    }
}

fn diff(expected: &str, actual: &str) {
    let changeset = Changeset::new(expected.trim(), actual.trim(), "\n");

    let mut different = false;
    for diff in changeset.diffs {
        let (prefix, diff) = match diff {
            Difference::Same(_) => continue,
            Difference::Add(add) => ("+", add),
            Difference::Rem(rem) => ("-", rem),
        };
        if !different {
            println!("differences found (+ == actual, - == expected):\n");
            different = true;
        }
        for diff in diff.lines() {
            println!("{} {}", prefix, diff);
        }
    }
    if different {
        println!("");
        panic!("found some differences");
    }
}

mod dependencies;
mod smoke;
mod subtargets;
mod warnings;
