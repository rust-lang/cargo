use url::Url;
use util::{CargoResult,ProcessBuilder,io_error,human_error,process};
use std::str;
use std::io::UserDir;
use std::io::fs::mkdir_recursive;
use serialize::{Encodable,Encoder};

macro_rules! git(
    ($config:expr, $str:expr, $($rest:expr),*) => (
        try!(git_inherit($config, format_strbuf!($str, $($rest),*)))
    );
)

macro_rules! git_output(
    ($config:expr, $str:expr, $($rest:expr),*) => (
        try!(git_output($config, format_strbuf!($str, $($rest),*)))
    );
)

#[deriving(Eq,Clone)]
struct GitConfig {
    path: Path,
    uri: Url,
    reference: StrBuf
}

#[deriving(Eq,Clone,Encodable)]
struct EncodableGitConfig {
    path: StrBuf,
    uri: StrBuf,
    reference: StrBuf
}

impl<E, S: Encoder<E>> Encodable<S, E> for GitConfig {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        EncodableGitConfig {
            path: format_strbuf!("{}", self.path.display()),
            uri: format_strbuf!("{}", self.uri),
            reference: self.reference.clone()
        }.encode(s)
    }
}

#[deriving(Eq,Clone)]
pub struct GitCommand {
    config: GitConfig
}

#[deriving(Eq,Clone,Encodable)]
pub struct GitRepo {
    config: GitConfig,
    revision: StrBuf
}

impl GitCommand {
    pub fn new(path: Path, uri: Url, reference: StrBuf) -> GitCommand {
        GitCommand { config: GitConfig { path: path, uri: uri, reference: reference } }
    }

    pub fn checkout(&self) -> CargoResult<GitRepo> {
        let config = &self.config;

        if config.path.exists() {
            git!(config, "fetch --force --quiet --tags {} refs/heads/*:refs/heads/*", config.uri);
        } else {
            let dirname = Path::new(config.path.dirname());
            let mut checkout_config = self.config.clone();
            checkout_config.path = dirname;

            try!(mkdir_recursive(&checkout_config.path, UserDir).map_err(|err|
                human_error(format_strbuf!("Couldn't recursively create `{}`", checkout_config.path.display()), format_strbuf!("path={}", checkout_config.path.display()), io_error(err))));

            git!(&checkout_config, "clone {} {} --bare --no-hardlinks --quiet", config.uri, config.path.display());
        }

        Ok(GitRepo { config: config.clone(), revision: try!(rev_for(config)) })
    }
}

fn rev_for(config: &GitConfig) -> CargoResult<StrBuf> {
    Ok(git_output!(config, "rev-parse {}", config.reference))
}

fn git(config: &GitConfig, str: &str) -> ProcessBuilder {
    println!("Executing git {} @ {}", str, config.path.display());
    process("git").args(str.split(' ').collect::<Vec<&str>>().as_slice()).cwd(config.path.clone())
}

fn git_inherit(config: &GitConfig, str: StrBuf) -> CargoResult<()> {
    git(config, str.as_slice()).exec().map_err(|err|
        human_error(format_strbuf!("Couldn't execute `git {}`: {}", str, err), None::<&str>, err))
}

fn git_output(config: &GitConfig, str: StrBuf) -> CargoResult<StrBuf> {
    let output = try!(git(config, str.as_slice()).exec_with_output().map_err(|err|
        human_error(format_strbuf!("Couldn't execute `git {}`", str), None::<&str>, err)));

    Ok(to_str(output.output.as_slice()))
}

fn to_str(vec: &[u8]) -> StrBuf {
    format_strbuf!("{}", str::from_utf8_lossy(vec))
}
