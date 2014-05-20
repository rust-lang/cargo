use std::fmt;
use std::fmt::{Show,Formatter};
use std::os;
use std::path::Path;
use std::io::process::{Command,ProcessOutput,InheritFd};
use util::{CargoResult,io_error,process_error};
use collections::HashMap;

#[deriving(Clone,Eq)]
pub struct ProcessBuilder {
    program: StrBuf,
    args: Vec<StrBuf>,
    path: Vec<StrBuf>,
    env: HashMap<StrBuf, StrBuf>,
    cwd: Path
}

impl Show for ProcessBuilder {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "`{}", self.program));

        if self.args.len() > 0 {
            try!(write!(f, " {}", self.args.connect(" ")));
        }

        write!(f, "`")
    }
}

// TODO: Upstream a Windows/Posix branch to Rust proper
static PATH_SEP : &'static str = ":";

impl ProcessBuilder {
    pub fn args(mut self, arguments: &[StrBuf]) -> ProcessBuilder {
        self.args = Vec::from_slice(arguments);
        self
    }

    pub fn get_args<'a>(&'a self) -> &'a [StrBuf] {
        self.args.as_slice()
    }

    pub fn extra_path(mut self, path: Path) -> ProcessBuilder {
        // For now, just convert to a string, but we should do something better
        self.path.push(format_strbuf!("{}", path.display()));
        self
    }

    pub fn cwd(mut self, path: Path) -> ProcessBuilder {
        self.cwd = path;
        self
    }

    // TODO: should InheritFd be hardcoded?
    pub fn exec(&self) -> CargoResult<()> {
        let mut command = self.build_command();
        command
            .env(self.build_env())
            .stdout(InheritFd(1))
            .stderr(InheritFd(2));

        let exit = try!(command.status().map_err(io_error));

        if exit.success() {
            Ok(())
        } else {
            let msg = format_strbuf!("Could not execute process `{}`", self.debug_string());
            Err(process_error(msg, exit, None))
        }
    }

    pub fn exec_with_output(&self) -> CargoResult<ProcessOutput> {
        let mut command = self.build_command();
        command.env(self.build_env());

        let output = try!(command.output().map_err(io_error));

        if output.status.success() {
            Ok(output)
        } else {
            let msg = format_strbuf!("Could not execute process `{}`", self.debug_string());
            Err(process_error(msg, output.status.clone(), Some(output)))
        }
    }

    fn build_command(&self) -> Command {
        let mut command = Command::new(self.program.as_slice());
        command.args(self.args.as_slice()).cwd(&self.cwd);
        command
    }

    fn debug_string(&self) -> StrBuf {
        format_strbuf!("{} {}", self.program, self.args.connect(" "))
    }

    fn build_env(&self) -> ~[(StrBuf, StrBuf)] {
        let mut ret = Vec::new();

        for (key, val) in self.env.iter() {
            // Skip path
            if key.as_slice() != "PATH" {
                ret.push((key.clone(), val.clone()));
            }
        }

        match self.build_path() {
            Some(path) => ret.push(("PATH".to_strbuf(), path)),
            _ => ()
        }

        ret.as_slice().to_owned()
    }

    fn build_path(&self) -> Option<StrBuf> {
        let path = self.path.connect(PATH_SEP);

        match self.env.find_equiv(&("PATH")) {
            Some(existing) => {
                if self.path.is_empty() {
                    Some(existing.clone())
                } else {
                    Some(format_strbuf!("{}{}{}", existing, PATH_SEP, path))
                }
            },
            None => {
                if self.path.is_empty() {
                    None
                } else {
                    Some(path.to_strbuf())
                }
            }
        }
    }
}

pub fn process(cmd: &str) -> ProcessBuilder {
    ProcessBuilder {
        program: cmd.to_strbuf(),
        args: vec!(),
        path: vec!(),
        cwd: os::getcwd(),
        env: system_env()
    }
}

fn system_env() -> HashMap<StrBuf, StrBuf> {
    let mut ret = HashMap::new();

    for &(ref key, ref val) in os::env().iter() {
        ret.insert(key.to_strbuf(), val.to_strbuf());
    }

    ret
}
