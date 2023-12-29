//! Provider used for testing redirection of stdout.

#![allow(clippy::print_stderr)]
#![allow(clippy::print_stdout)]

use cargo_credential::{Action, Credential, CredentialResponse, Error, RegistryInfo};

struct MyCredential;

impl Credential for MyCredential {
    fn perform(
        &self,
        _registry: &RegistryInfo<'_>,
        _action: &Action<'_>,
        _args: &[&str],
    ) -> Result<CredentialResponse, Error> {
        // Informational messages should be sent on stderr.
        eprintln!("message on stderr should be sent to the parent process");

        // Reading from stdin and writing to stdout will go to the attached console (tty).
        println!("message from test credential provider");
        Err(Error::OperationNotSupported)
    }
}

fn main() {
    cargo_credential::main(MyCredential);
}
