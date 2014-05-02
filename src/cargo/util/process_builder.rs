use std::os;
use std::path::Path;
use std::io;
use std::io::process::{Process,ProcessConfig,ProcessOutput,InheritFd};
use ToCargoError;
use CargoResult;

#[deriving(Clone,Eq)]
pub struct ProcessBuilder {
  program: ~str,
  args: Vec<~str>,
  path: Vec<~str>,
  cwd: Path
}

// TODO: Upstream a Windows/Posix branch to Rust proper
static PATH_SEP : &'static str = ":";

impl ProcessBuilder {
  pub fn args(mut self, arguments: &[~str]) -> ProcessBuilder {
    self.args = Vec::from_slice(arguments);
    self
  }

  pub fn extra_path(mut self, path: Path) -> ProcessBuilder {
    // For now, just convert to a string, but we should do something better
    self.path.push(format!("{}", path.display()));
    self
  }

  pub fn cwd(mut self, path: Path) -> ProcessBuilder {
    self.cwd = path;
    self
  }

  // TODO: clean all this up
  pub fn exec(&self) -> io::IoResult<()> {
      let mut config = ProcessConfig::new();

      config.program = self.program.as_slice();
      config.args = self.args.as_slice();
      config.cwd = Some(&self.cwd);
      config.stdout = InheritFd(1);
      config.stderr = InheritFd(2);

      let mut process = try!(Process::configure(config));
      let exit = process.wait();

      if exit.success() {
          Ok(())
      }
      else {
          Err(io::IoError {
              kind: io::OtherIoError,
              desc: "process did not exit successfully",
              detail: None
          })
      }
  }

  pub fn exec_with_output(&self) -> CargoResult<ProcessOutput> {
    let mut config = ProcessConfig::new();

    println!("cwd: {}", self.cwd.display());

    config.program = self.program.as_slice();
    config.args = self.args.as_slice();
    config.cwd = Some(&self.cwd);

    let os_path = try!(os::getenv("PATH").to_cargo_error(~"Could not find the PATH environment variable", 1));
    let path = os_path + PATH_SEP + self.path.connect(PATH_SEP);

    let path = [(~"PATH", path)];
    config.env = Some(path.as_slice());

    println!("{:?}", config);

    Process::configure(config).map(|mut ok| ok.wait_with_output()).to_cargo_error(~"Could not spawn process", 1)
  }
}

pub fn process(cmd: &str) -> ProcessBuilder {
  ProcessBuilder {
    program: cmd.to_owned(),
    args: vec!(),
    path: vec!(),
    cwd: os::getcwd()
  }
}
