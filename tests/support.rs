// use std::io::fs::{mkdir_recursive,rmdir_recursive};
use std::io::fs;
use std::os;
use std::path::{Path};
use cargo::util::{process,ProcessBuilder};

static CARGO_INTEGRATION_TEST_DIR : &'static str = "cargo-integration-tests";
static MKDIR_PERM : u32 = 0o755;

/*
 *
 * ===== Builders =====
 *
 */

#[deriving(Eq,Clone)]
struct FileBuilder {
    path: Path,
    body: ~str
}

impl FileBuilder {
    pub fn new(path: Path, body: &str) -> FileBuilder {
        FileBuilder { path: path, body: body.to_owned() }
    }

    fn mk(&self) -> Result<(), ~str> {
        try!(mkdir_recursive(&self.dirname()));

        let mut file = try!(
            fs::File::create(&self.path)
                .with_err_msg(format!("Could not create file; path={}", self.path.display())));

        file.write_str(self.body.as_slice())
            .with_err_msg(format!("Could not write to file; path={}", self.path.display()))
    }

    fn dirname(&self) -> Path {
        Path::new(self.path.dirname())
    }
}

#[deriving(Eq,Clone)]
struct ProjectBuilder {
    name: ~str,
    root: Path,
    files: ~[FileBuilder]
}

impl ProjectBuilder {
    pub fn new(name: &str, root: Path) -> ProjectBuilder {
        ProjectBuilder {
            name: name.to_owned(),
            root: root,
            files: ~[]
        }
    }

    pub fn root(&self) -> Path {
      self.root.clone()
    }

    pub fn cargo_process(&self, program: &str) -> ProcessBuilder {
      process(program)
        .cwd(self.root())
        .extra_path(cargo_dir())
    }

    pub fn file(mut self, path: &str, body: &str) -> ProjectBuilder {
        self.files.push(FileBuilder::new(self.root.join(path), body));
        self
    }

    // TODO: return something different than a ProjectBuilder
    pub fn build(self) -> ProjectBuilder {
        match self.build_with_result() {
            Err(e) => fail!(e),
            _ => return self
        }
    }

    pub fn build_with_result(&self) -> Result<(), ~str> {
        // First, clean the directory if it already exists
        try!(self.rm_root());

        // Create the empty directory
        try!(mkdir_recursive(&self.root));

        for file in self.files.iter() {
          try!(file.mk());
        }

        println!("{}", self.root.display());
        println!("{:?}", self);
        Ok(())
    }

    fn rm_root(&self) -> Result<(), ~str> {
        if self.root.exists() {
            rmdir_recursive(&self.root)
        }
        else {
            Ok(())
        }
    }
}

// Generates a project layout
pub fn project(name: &str) -> ProjectBuilder {
    ProjectBuilder::new(name, os::tmpdir().join(CARGO_INTEGRATION_TEST_DIR))
}

// === Helpers ===

pub fn mkdir_recursive(path: &Path) -> Result<(), ~str> {
    fs::mkdir_recursive(path, MKDIR_PERM)
        .with_err_msg(format!("could not create directory; path={}", path.display()))
}

pub fn rmdir_recursive(path: &Path) -> Result<(), ~str> {
    fs::rmdir_recursive(path)
        .with_err_msg(format!("could not rm directory; path={}", path.display()))
}

trait ErrMsg<T> {
    fn with_err_msg(self, val: ~str) -> Result<T, ~str>;
}

impl<T, E> ErrMsg<T> for Result<T, E> {
    fn with_err_msg(self, val: ~str) -> Result<T, ~str> {
        match self {
            Ok(val) => Ok(val),
            Err(_) => Err(val)
        }
    }
}

// Path to cargo executables
pub fn cargo_dir() -> ~str {
  os::getenv("CARGO_BIN_PATH").unwrap_or_else(|| {
    fail!("CARGO_BIN_PATH wasn't set. Cannot continue running test")
  })
}

/*
 *
 * ===== Matchers =====
 *
 */
