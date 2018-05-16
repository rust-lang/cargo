use failure::{err_msg, Error, ResultExt};
use std::env;
use std::process::Command;

pub enum VersionControl {
    Git,
    Hg,
    Fossil,
    Pijul,
    None,
}

impl VersionControl {
    pub fn new() -> Self {
        if is_in_vcs(".git").is_ok() {
            VersionControl::Git
        } else if is_in_vcs(".hg").is_ok() {
            VersionControl::Hg
        } else if is_in_vcs(".pijul").is_ok() {
            VersionControl::Pijul
        } else if is_in_vcs(".fossil").is_ok() {
            VersionControl::Fossil
        } else {
            VersionControl::None
        }
    }

    pub fn is_present(&self) -> bool {
        match *self {
            VersionControl::None => false,
            _ => true,
        }
    }

    pub fn is_dirty(&self) -> Result<bool, Error> {
        let (program, args) = match *self {
            VersionControl::Git => ("git", "status --short"),
            VersionControl::Hg => ("hg", "status"),
            VersionControl::Pijul => ("pijul", "status"),
            VersionControl::Fossil => ("fossil", "changes"),
            VersionControl::None => return Ok(false),
        };

        let output = Command::new(program)
            .args(args.split_whitespace())
            .output()?
            .stdout;

        let changes = String::from_utf8_lossy(&output);
        let trimmed = changes.trim();
        if !trimmed.is_empty() {
            eprintln!("{}", trimmed);
        }

        Ok(!trimmed.is_empty())
    }
}

fn is_in_vcs(vcs_dir: &str) -> Result<(), Error> {
    let mut current_dir = env::current_dir().context("could not find the current directory")?;

    loop {
        if current_dir.join(vcs_dir).metadata().is_ok() {
            return Ok(());
        }

        current_dir = current_dir
            .parent()
            .ok_or_else(|| err_msg("could not find the parent directory"))?
            .to_owned();
    }
}
