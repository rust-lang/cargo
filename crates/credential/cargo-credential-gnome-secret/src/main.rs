//! Cargo registry gnome libsecret credential process.

use cargo_credential::{Credential, Error};
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

extern "C" {
    fn secret_password_store_sync(
        schema: *const SecretSchema,
        collection: *const gchar,
        label: *const gchar,
        password: *const gchar,
        cancellable: *mut GCancellable,
        error: *mut *mut GError,
        ...
    ) -> gboolean;
    fn secret_password_clear_sync(
        schema: *const SecretSchema,
        cancellable: *mut GCancellable,
        error: *mut *mut GError,
        ...
    ) -> gboolean;
    fn secret_password_lookup_sync(
        schema: *const SecretSchema,
        cancellable: *mut GCancellable,
        error: *mut *mut GError,
        ...
    ) -> *mut gchar;
}

struct GnomeSecret;

fn label(registry_name: &str) -> CString {
    CString::new(format!("cargo-registry:{}", registry_name)).unwrap()
}

fn schema() -> SecretSchema {
    let mut attributes = [SecretSchemaAttribute {
        name: null(),
        attr_type: SecretSchemaAttributeType::String,
    }; 32];
    attributes[0] = SecretSchemaAttribute {
        name: b"registry\0".as_ptr() as *const gchar,
        attr_type: SecretSchemaAttributeType::String,
    };
    attributes[1] = SecretSchemaAttribute {
        name: b"url\0".as_ptr() as *const gchar,
        attr_type: SecretSchemaAttributeType::String,
    };
    SecretSchema {
        name: b"org.rust-lang.cargo.registry\0".as_ptr() as *const gchar,
        flags: SecretSchemaFlags::None,
        attributes,
    }
}

impl Credential for GnomeSecret {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn get(&self, registry_name: &str, api_url: &str) -> Result<String, Error> {
        let mut error: *mut GError = null_mut();
        let attr_registry = CString::new("registry").unwrap();
        let attr_url = CString::new("url").unwrap();
        let registry_name_c = CString::new(registry_name).unwrap();
        let api_url_c = CString::new(api_url).unwrap();
        let schema = schema();
        unsafe {
            let token_c = secret_password_lookup_sync(
                &schema,
                null_mut(),
                &mut error,
                attr_registry.as_ptr(),
                registry_name_c.as_ptr(),
                attr_url.as_ptr(),
                api_url_c.as_ptr(),
                null() as *const gchar,
            );
            if !error.is_null() {
                return Err(format!(
                    "failed to get token: {}",
                    CStr::from_ptr((*error).message).to_str()?
                )
                .into());
            }
            if token_c.is_null() {
                return Err(format!("cannot find token for {}", registry_name).into());
            }
            let token = CStr::from_ptr(token_c)
                .to_str()
                .map_err(|e| format!("expected utf8 token: {}", e))?
                .to_string();
            Ok(token)
        }
    }

    fn store(&self, registry_name: &str, api_url: &str, token: &str) -> Result<(), Error> {
        let label = label(registry_name);
        let token = CString::new(token).unwrap();
        let mut error: *mut GError = null_mut();
        let attr_registry = CString::new("registry").unwrap();
        let attr_url = CString::new("url").unwrap();
        let registry_name_c = CString::new(registry_name).unwrap();
        let api_url_c = CString::new(api_url).unwrap();
        let schema = schema();
        unsafe {
            secret_password_store_sync(
                &schema,
                b"default\0".as_ptr() as *const gchar,
                label.as_ptr(),
                token.as_ptr(),
                null_mut(),
                &mut error,
                attr_registry.as_ptr(),
                registry_name_c.as_ptr(),
                attr_url.as_ptr(),
                api_url_c.as_ptr(),
                null() as *const gchar,
            );
            if !error.is_null() {
                return Err(format!(
                    "failed to store token: {}",
                    CStr::from_ptr((*error).message).to_str()?
                )
                .into());
            }
        }
        Ok(())
    }

    fn erase(&self, registry_name: &str, api_url: &str) -> Result<(), Error> {
        let schema = schema();
        let mut error: *mut GError = null_mut();
        let attr_registry = CString::new("registry").unwrap();
        let attr_url = CString::new("url").unwrap();
        let registry_name_c = CString::new(registry_name).unwrap();
        let api_url_c = CString::new(api_url).unwrap();
        unsafe {
            secret_password_clear_sync(
                &schema,
                null_mut(),
                &mut error,
                attr_registry.as_ptr(),
                registry_name_c.as_ptr(),
                attr_url.as_ptr(),
                api_url_c.as_ptr(),
                null() as *const gchar,
            );
            if !error.is_null() {
                return Err(format!(
                    "failed to erase token: {}",
                    CStr::from_ptr((*error).message).to_str()?
                )
                .into());
            }
        }
        Ok(())
    }
}

fn main() {
    cargo_credential::main(GnomeSecret);
}
