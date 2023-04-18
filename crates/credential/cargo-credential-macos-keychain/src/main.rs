//! Cargo registry macos keychain credential process.

#[cfg(target_os = "macos")]
mod macos {
    use cargo_credential::{Credential, Error};
    use security_framework::os::macos::keychain::SecKeychain;

    pub(crate) struct MacKeychain;

    /// The account name is not used.
    const ACCOUNT: &'static str = "";

    fn registry(registry_name: &str) -> String {
        format!("cargo-registry:{}", registry_name)
    }

    impl Credential for MacKeychain {
        fn name(&self) -> &'static str {
            env!("CARGO_PKG_NAME")
        }

        fn get(&self, index_url: &str) -> Result<String, Error> {
            let keychain = SecKeychain::default().unwrap();
            let service_name = registry(index_url);
            let (pass, _item) = keychain.find_generic_password(&service_name, ACCOUNT)?;
            String::from_utf8(pass.as_ref().to_vec())
                .map_err(|_| "failed to convert token to UTF8".into())
        }

        fn store(&self, index_url: &str, token: &str, name: Option<&str>) -> Result<(), Error> {
            let keychain = SecKeychain::default().unwrap();
            let service_name = registry(name.unwrap_or(index_url));
            if let Ok((_pass, mut item)) = keychain.find_generic_password(&service_name, ACCOUNT) {
                item.set_password(token.as_bytes())?;
            } else {
                keychain.add_generic_password(&service_name, ACCOUNT, token.as_bytes())?;
            }
            Ok(())
        }

        fn erase(&self, index_url: &str) -> Result<(), Error> {
            let keychain = SecKeychain::default().unwrap();
            let service_name = registry(index_url);
            let (_pass, item) = keychain.find_generic_password(&service_name, ACCOUNT)?;
            item.delete();
            Ok(())
        }
    }
}

#[cfg(not(target_os = "macos"))]
use cargo_credential::UnsupportedCredential as MacKeychain;
#[cfg(target_os = "macos")]
use macos::MacKeychain;

fn main() {
    cargo_credential::main(MacKeychain);
}
