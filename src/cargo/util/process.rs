use std::os;
use std::io::process::{Process,ProcessConfig,InheritFd};

pub struct ProcessBuilder {
  program: ~str,
  args: ~[~str],
  path: ~[~str]
}

impl ProcessBuilder {
  fn args(mut self, arguments: &[~str]) -> ProcessBuilder {
    self.args = arguments.clone();
    self
  }
}

pub fn process(cmd: &str) -> ProcessBuilder {
  ProcessBuilder { program: cmd.to_owned(), args: ~[], path: get_curr_path() }
}

fn get_curr_path() -> ~[~str] {
  os::getenv("PATH").map(|path| {
    path.split(std::path::SEP).collect()
  }).or(~[])
}
