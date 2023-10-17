//! Example credential provider that stores credentials in a JSON file.
//! This is not secure

use cargo_credential::{
    Action, CacheControl, Credential, CredentialResponse, RegistryInfo, Secret,
};
use std::{collections::HashMap, fs::File, io::ErrorKind};
type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

struct FileCredential;

impl Credential for FileCredential {
    fn perform(
        &self,
        registry: &RegistryInfo<'_>,
        action: &Action<'_>,
        _args: &[&str],
    ) -> Result<CredentialResponse, cargo_credential::Error> {
        if registry.index_url != "https://github.com/rust-lang/crates.io-index" {
            // Restrict this provider to only work for crates.io. Cargo will skip it and attempt
            // another provider for any other registry.
            //
            // If a provider supports any registry, then this check should be omitted.
            return Err(cargo_credential::Error::UrlNotSupported);
        }

        // `Error::Other` takes a boxed `std::error::Error` type that causes Cargo to show the error.
        let mut creds = FileCredential::read().map_err(cargo_credential::Error::Other)?;

        match action {
            Action::Get(_) => {
                // Cargo requested a token, look it up.
                if let Some(token) = creds.get(registry.index_url) {
                    Ok(CredentialResponse::Get {
                        token: token.clone(),
                        cache: CacheControl::Session,
                        operation_independent: true,
                    })
                } else {
                    // Credential providers should respond with `NotFound` when a credential can not be
                    // found, allowing Cargo to attempt another provider.
                    Err(cargo_credential::Error::NotFound)
                }
            }
            Action::Login(login_options) => {
                // The token for `cargo login` can come from the `login_options` parameter or i
                // interactively reading from stdin.
                //
                // `cargo_credential::read_token` automatically handles this.
                let token = cargo_credential::read_token(login_options, registry)?;
                creds.insert(registry.index_url.to_string(), token);

                FileCredential::write(&creds).map_err(cargo_credential::Error::Other)?;

                // Credentials were successfully stored.
                Ok(CredentialResponse::Login)
            }
            Action::Logout => {
                if creds.remove(registry.index_url).is_none() {
                    // If the user attempts to log out from a registry that has no credentials
                    // stored, then NotFound is the appropriate error.
                    Err(cargo_credential::Error::NotFound)
                } else {
                    // Credentials were successfully erased.
                    Ok(CredentialResponse::Logout)
                }
            }
            // If a credential provider doesn't support a given operation, it should respond with `OperationNotSupported`.
            _ => Err(cargo_credential::Error::OperationNotSupported),
        }
    }
}

impl FileCredential {
    fn read() -> Result<HashMap<String, Secret<String>>, Error> {
        match File::open("cargo-credentials.json") {
            Ok(f) => Ok(serde_json::from_reader(f)?),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(HashMap::new()),
            Err(e) => Err(e)?,
        }
    }
    fn write(value: &HashMap<String, Secret<String>>) -> Result<(), Error> {
        let file = File::create("cargo-credentials.json")?;
        Ok(serde_json::to_writer_pretty(file, value)?)
    }
}

fn main() {
    cargo_credential::main(FileCredential);
}
