use std::os;
use std::path::Path;
use std::io::process::{Process,ProcessConfig,ProcessOutput};
use ToCargoError;
use CargoResult;

pub struct ProcessBuilder {
  program: ~str,
  args: ~[~str],
  path: ~[~str],
  cwd: Path
}

// TODO: Upstream a Windows/Posix branch to Rust proper
static PATH_SEP : &'static str = ":";

impl ProcessBuilder {
  pub fn args(mut self, arguments: &[~str]) -> ProcessBuilder {
    self.args = arguments.to_owned();
    self
  }

  pub fn extra_path(mut self, path: &str) -> ProcessBuilder {
    self.path.push(path.to_owned());
    self
  }

  pub fn cwd(mut self, path: Path) -> ProcessBuilder {
    self.cwd = path;
    self
  }

  pub fn exec_with_output(self) -> CargoResult<ProcessOutput> {
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
    args: ~[],
    path: ~[],
    cwd: os::getcwd()
  }
}
