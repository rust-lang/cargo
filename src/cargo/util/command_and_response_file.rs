use std::ops::{Deref, DerefMut};
use std::process::Command;

use tempfile::TempPath;

/// A wrapper around `Command` which extends the lifetime of associated
/// temporary response files until the command is executed.
pub struct CommandAndResponseFile {
    pub command: Command,
    pub response_file: Option<TempPath>,
}

impl Deref for CommandAndResponseFile {
    type Target = Command;
    fn deref(&self) -> &Self::Target {
        &self.command
    }
}

impl DerefMut for CommandAndResponseFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.command
    }
}
