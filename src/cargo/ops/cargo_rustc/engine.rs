use std::collections::HashMap;
use std::c_str::CString;
use std::io::process::ProcessOutput;
use std::fmt::{mod, Show, Formatter};

use util::{mod, CargoResult, ProcessError, ProcessBuilder};

/// Trait for objects that can execute commands.
pub trait ExecEngine: Send + Sync {
    fn exec(&self, CommandPrototype) -> Result<(), ProcessError>;
    fn exec_with_output(&self, CommandPrototype) -> Result<ProcessOutput, ProcessError>;
}

/// Default implementation of `ExecEngine`.
#[deriving(Copy)]
pub struct ProcessEngine;

impl ExecEngine for ProcessEngine {
    fn exec(&self, command: CommandPrototype) -> Result<(), ProcessError> {
        command.into_process_builder().unwrap().exec()
    }

    fn exec_with_output(&self, command: CommandPrototype)
                        -> Result<ProcessOutput, ProcessError> {
        command.into_process_builder().unwrap().exec_with_output()
    }
}

/// Prototype for a command that must be executed.
#[deriving(Clone)]
pub struct CommandPrototype {
    ty: CommandType,
    args: Vec<CString>,
    env: HashMap<String, Option<CString>>,
    cwd: Path,
}

impl CommandPrototype {
    pub fn new(ty: CommandType) -> CargoResult<CommandPrototype> {
        use std::os;

        Ok(CommandPrototype {
            ty: ty,
            args: Vec::new(),
            env: HashMap::new(),
            cwd: try!(os::getcwd()),
        })
    }

    pub fn get_type(&self) -> &CommandType {
        &self.ty
    }

    pub fn arg<T: ToCStr>(mut self, arg: T) -> CommandPrototype {
        self.args.push(arg.to_c_str());
        self
    }

    pub fn args<T: ToCStr>(mut self, arguments: &[T]) -> CommandPrototype {
        self.args.extend(arguments.iter().map(|t| t.to_c_str()));
        self
    }

    pub fn get_args(&self) -> &[CString] {
        self.args.as_slice()
    }

    pub fn cwd(mut self, path: Path) -> CommandPrototype {
        self.cwd = path;
        self
    }

    pub fn get_cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn env<T: ToCStr>(mut self, key: &str, val: Option<T>) -> CommandPrototype {
        self.env.insert(key.to_string(), val.map(|t| t.to_c_str()));
        self
    }

    pub fn get_envs(&self) -> &HashMap<String, Option<CString>> {
        &self.env
    }

    pub fn into_process_builder(self) -> CargoResult<ProcessBuilder> {
        let mut builder = try!(match self.ty {
            CommandType::Rustc => util::process("rustc"),
            CommandType::Rustdoc => util::process("rustdoc"),
            CommandType::Target(ref cmd) | CommandType::Host(ref cmd) => {
                util::process(cmd.as_bytes_no_nul())
            },
        });

        for arg in self.args.into_iter() {
            builder = builder.arg(arg.as_bytes_no_nul());
        }

        for (key, val) in self.env.into_iter() {
            builder = builder.env(key.as_slice(), val.as_ref().map(|v| v.as_bytes_no_nul()));
        }

        builder = builder.cwd(self.cwd);

        Ok(builder)
    }
}

impl Show for CommandPrototype {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self.ty {
            CommandType::Rustc => try!(write!(f, "`rustc")),
            CommandType::Rustdoc => try!(write!(f, "`rustdoc")),
            CommandType::Target(ref cmd) | CommandType::Host(ref cmd) => {
                let cmd = String::from_utf8_lossy(cmd.as_bytes_no_nul());
                try!(write!(f, "`{}", cmd));
            },
        }

        for arg in self.args.iter() {
            try!(write!(f, " {}", String::from_utf8_lossy(arg.as_bytes_no_nul())));
        }

        write!(f, "`")
    }
}

#[deriving(Clone, Show)]
pub enum CommandType {
    Rustc,
    Rustdoc,

    /// The command is to be executed for the target architecture.
    Target(CString),

    /// The command is to be executed for the host architecture.
    Host(CString),
}
