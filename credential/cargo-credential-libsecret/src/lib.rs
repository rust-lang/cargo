#[cfg(target_os = "linux")]
mod linux {
    //! Implementation of the libsecret credential helper.

    use anyhow::Context;
    use cargo_credential::{
        read_token, Action, CacheControl, Credential, CredentialResponse, Error, RegistryInfo,
        Secret,
    };
    use libloading::{Library, Symbol};
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int};
    use std::ptr::{null, null_mut};

    #[allow(non_camel_case_types)]
    type gchar = c_char;

    #[allow(non_camel_case_types)]
    type gboolean = c_int;

    type GQuark = u32;

    #[repr(C)]
    struct GError {
        domain: GQuark,
        code: c_int,
        message: *mut gchar,
    }

    #[repr(C)]
    struct GCancellable {
        _private: [u8; 0],
    }

    #[repr(C)]
    struct SecretSchema {
        name: *const gchar,
        flags: SecretSchemaFlags,
        attributes: [SecretSchemaAttribute; 32],
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct SecretSchemaAttribute {
        name: *const gchar,
        attr_type: SecretSchemaAttributeType,
    }

    #[repr(C)]
    enum SecretSchemaFlags {
        None = 0,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    enum SecretSchemaAttributeType {
        String = 0,
    }

    type SecretPasswordStoreSync = extern "C" fn(
        schema: *const SecretSchema,
        collection: *const gchar,
        label: *const gchar,
        password: *const gchar,
        cancellable: *mut GCancellable,
        error: *mut *mut GError,
        ...
    ) -> gboolean;
    type SecretPasswordClearSync = extern "C" fn(
        schema: *const SecretSchema,
        cancellable: *mut GCancellable,
        error: *mut *mut GError,
        ...
    ) -> gboolean;
    type SecretPasswordLookupSync = extern "C" fn(
        schema: *const SecretSchema,
        cancellable: *mut GCancellable,
        error: *mut *mut GError,
        ...
    ) -> *mut gchar;

    pub struct LibSecretCredential;

    fn label(index_url: &str) -> CString {
        CString::new(format!("cargo-registry:{}", index_url)).unwrap()
    }

    fn schema() -> SecretSchema {
        let mut attributes = [SecretSchemaAttribute {
            name: null(),
            attr_type: SecretSchemaAttributeType::String,
        }; 32];
        attributes[0] = SecretSchemaAttribute {
            name: b"url\0".as_ptr() as *const gchar,
            attr_type: SecretSchemaAttributeType::String,
        };
        SecretSchema {
            name: b"org.rust-lang.cargo.registry\0".as_ptr() as *const gchar,
            flags: SecretSchemaFlags::None,
            attributes,
        }
    }

    impl Credential for LibSecretCredential {
        fn perform(
            &self,
            registry: &RegistryInfo<'_>,
            action: &Action<'_>,
            _args: &[&str],
        ) -> Result<CredentialResponse, Error> {
            // Dynamically load libsecret to avoid users needing to install
            // additional -dev packages when building this provider.
            let lib;
            let secret_password_lookup_sync: Symbol<'_, SecretPasswordLookupSync>;
            let secret_password_store_sync: Symbol<'_, SecretPasswordStoreSync>;
            let secret_password_clear_sync: Symbol<'_, SecretPasswordClearSync>;
            unsafe {
                lib = Library::new("libsecret-1.so").context(
                    "failed to load libsecret: try installing the `libsecret` \
                    or `libsecret-1-0` package with the system package manager",
                )?;
                secret_password_lookup_sync = lib
                    .get(b"secret_password_lookup_sync\0")
                    .map_err(Box::new)?;
                secret_password_store_sync =
                    lib.get(b"secret_password_store_sync\0").map_err(Box::new)?;
                secret_password_clear_sync =
                    lib.get(b"secret_password_clear_sync\0").map_err(Box::new)?;
            }

            let index_url_c = CString::new(registry.index_url).unwrap();
            match action {
                cargo_credential::Action::Get(_) => {
                    let mut error: *mut GError = null_mut();
                    let attr_url = CString::new("url").unwrap();
                    let schema = schema();
                    unsafe {
                        let token_c = secret_password_lookup_sync(
                            &schema,
                            null_mut(),
                            &mut error,
                            attr_url.as_ptr(),
                            index_url_c.as_ptr(),
                            null() as *const gchar,
                        );
                        if !error.is_null() {
                            return Err(format!(
                                "failed to get token: {}",
                                CStr::from_ptr((*error).message)
                                    .to_str()
                                    .unwrap_or_default()
                            )
                            .into());
                        }
                        if token_c.is_null() {
                            return Err(Error::NotFound);
                        }
                        let token = Secret::from(
                            CStr::from_ptr(token_c)
                                .to_str()
                                .map_err(|e| format!("expected utf8 token: {}", e))?
                                .to_string(),
                        );
                        Ok(CredentialResponse::Get {
                            token,
                            cache: CacheControl::Session,
                            operation_independent: true,
                        })
                    }
                }
                cargo_credential::Action::Login(options) => {
                    let label = label(registry.name.unwrap_or(registry.index_url));
                    let token = CString::new(read_token(options, registry)?.expose()).unwrap();
                    let mut error: *mut GError = null_mut();
                    let attr_url = CString::new("url").unwrap();
                    let schema = schema();
                    unsafe {
                        secret_password_store_sync(
                            &schema,
                            b"default\0".as_ptr() as *const gchar,
                            label.as_ptr(),
                            token.as_ptr(),
                            null_mut(),
                            &mut error,
                            attr_url.as_ptr(),
                            index_url_c.as_ptr(),
                            null() as *const gchar,
                        );
                        if !error.is_null() {
                            return Err(format!(
                                "failed to store token: {}",
                                CStr::from_ptr((*error).message)
                                    .to_str()
                                    .unwrap_or_default()
                            )
                            .into());
                        }
                    }
                    Ok(CredentialResponse::Login)
                }
                cargo_credential::Action::Logout => {
                    let schema = schema();
                    let mut error: *mut GError = null_mut();
                    let attr_url = CString::new("url").unwrap();
                    unsafe {
                        secret_password_clear_sync(
                            &schema,
                            null_mut(),
                            &mut error,
                            attr_url.as_ptr(),
                            index_url_c.as_ptr(),
                            null() as *const gchar,
                        );
                        if !error.is_null() {
                            return Err(format!(
                                "failed to erase token: {}",
                                CStr::from_ptr((*error).message)
                                    .to_str()
                                    .unwrap_or_default()
                            )
                            .into());
                        }
                    }
                    Ok(CredentialResponse::Logout)
                }
                _ => Err(Error::OperationNotSupported),
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub use cargo_credential::UnsupportedCredential as LibSecretCredential;
#[cfg(target_os = "linux")]
pub use linux::LibSecretCredential;
