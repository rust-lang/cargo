//! Cargo registry macos keychain credential process.

#[cfg(target_os = "macos")]
mod macos {
    use cargo_credential::{
        read_token, Action, CacheControl, Credential, CredentialResponse, Error, RegistryInfo,
    };
    use security_framework::os::macos::keychain::SecKeychain;

    pub struct MacKeychain;

    /// The account name is not used.
    const ACCOUNT: &'static str = "";
    const NOT_FOUND: i32 = -25300; // errSecItemNotFound

    fn registry(index_url: &str) -> String {
        format!("cargo-registry:{}", index_url)
    }

    impl Credential for MacKeychain {
        fn perform(
            &self,
            reg: &RegistryInfo<'_>,
            action: &Action<'_>,
            _args: &[&str],
        ) -> Result<CredentialResponse, Error> {
            let keychain = SecKeychain::default().unwrap();
            let service_name = registry(reg.index_url);
            let not_found = security_framework::base::Error::from(NOT_FOUND).code();
            match action {
                Action::Get(_) => match keychain.find_generic_password(&service_name, ACCOUNT) {
                    Err(e) if e.code() == not_found => Err(Error::NotFound),
                    Err(e) => Err(Box::new(e).into()),
                    Ok((pass, _)) => {
                        let token = String::from_utf8(pass.as_ref().to_vec()).map_err(Box::new)?;
                        Ok(CredentialResponse::Get {
                            token: token.into(),
                            cache: CacheControl::Session,
                            operation_independent: true,
                        })
                    }
                },
                Action::Login(options) => {
                    let token = read_token(options, reg)?;
                    match keychain.find_generic_password(&service_name, ACCOUNT) {
                        Err(e) => {
                            if e.code() == not_found {
                                keychain
                                    .add_generic_password(
                                        &service_name,
                                        ACCOUNT,
                                        token.expose().as_bytes(),
                                    )
                                    .map_err(Box::new)?;
                            }
                        }
                        Ok((_, mut item)) => {
                            item.set_password(token.expose().as_bytes())
                                .map_err(Box::new)?;
                        }
                    }
                    Ok(CredentialResponse::Login)
                }
                Action::Logout => match keychain.find_generic_password(&service_name, ACCOUNT) {
                    Err(e) if e.code() == not_found => Err(Error::NotFound),
                    Err(e) => Err(Box::new(e).into()),
                    Ok((_, item)) => {
                        item.delete();
                        Ok(CredentialResponse::Logout)
                    }
                },
                _ => Err(Error::OperationNotSupported),
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub use cargo_credential::UnsupportedCredential as MacKeychain;
#[cfg(target_os = "macos")]
pub use macos::MacKeychain;
