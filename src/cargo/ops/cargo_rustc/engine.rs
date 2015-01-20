use std::collections::HashMap;
use std::ffi::CString;
use std::fmt::{self, Formatter};
use std::io::process::ProcessOutput;
use std::os;
use std::path::BytesContainer;

use util::{self, CargoResult, ProcessError, ProcessBuilder};

/// Trait for objects that can execute commands.
pub trait ExecEngine: Send + Sync {
    fn exec(&self, CommandPrototype) -> Result<(), ProcessError>;
    fn exec_with_output(&self, CommandPrototype) -> Result<ProcessOutput, ProcessError>;
}

/// Default implementation of `ExecEngine`.
#[derive(Copy)]
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
#[derive(Clone)]
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

    pub fn arg<T: BytesContainer>(mut self, arg: T) -> CommandPrototype {
        self.args.push(CString::from_slice(arg.container_as_bytes()));
        self
    }

    pub fn args<T: BytesContainer>(mut self, arguments: &[T]) -> CommandPrototype {
        self.args.extend(arguments.iter().map(|t| {
            CString::from_slice(t.container_as_bytes())
        }));
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

    pub fn env<T: BytesContainer>(mut self, key: &str,
                                  val: Option<T>) -> CommandPrototype {
        let val = val.map(|t| CString::from_slice(t.container_as_bytes()));
        self.env.insert(key.to_string(), val);
        self
    }

    pub fn get_env(&self, var: &str) -> Option<CString> {
        self.env.get(var).cloned().or_else(|| {
            Some(os::getenv(var).map(|s| CString::from_vec(s.into_bytes())))
        }).and_then(|val| val)
    }

    pub fn get_envs(&self) -> &HashMap<String, Option<CString>> {
        &self.env
    }

    pub fn into_process_builder(self) -> CargoResult<ProcessBuilder> {
        let mut builder = try!(match self.ty {
            CommandType::Rustc => util::process("rustc"),
            CommandType::Rustdoc => util::process("rustdoc"),
            CommandType::Target(ref cmd) | CommandType::Host(ref cmd) => {
                util::process(cmd)
            },
        });

        for arg in self.args.into_iter() {
            builder = builder.arg(arg);
        }
        for (key, val) in self.env.into_iter() {
            builder = builder.env(key.as_slice(), val.as_ref());
        }

        builder = builder.cwd(self.cwd);

        Ok(builder)
    }
}

impl fmt::String for CommandPrototype {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.ty {
            CommandType::Rustc => try!(write!(f, "`rustc")),
            CommandType::Rustdoc => try!(write!(f, "`rustdoc")),
            CommandType::Target(ref cmd) | CommandType::Host(ref cmd) => {
                try!(write!(f, "`{}", String::from_utf8_lossy(cmd.as_bytes())));
            },
        }

        for arg in self.args.iter() {
            try!(write!(f, " {}", String::from_utf8_lossy(arg.as_bytes())));
        }

        write!(f, "`")
    }
}

#[derive(Clone, Show)]
pub enum CommandType {
    Rustc,
    Rustdoc,

    /// The command is to be executed for the target architecture.
    Target(CString),

    /// The command is to be executed for the host architecture.
    Host(CString),
}
