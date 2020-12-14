//! Cargo registry windows credential process.

use cargo_credential::{Credential, Error};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use winapi::shared::minwindef::{DWORD, FILETIME, LPBYTE, TRUE};
use winapi::shared::winerror;
use winapi::um::wincred;
use winapi::um::winnt::LPWSTR;

struct WindowsCredential;

/// Converts a string to a nul-terminated wide UTF-16 byte sequence.
fn wstr(s: &str) -> Vec<u16> {
    let mut wide: Vec<u16> = OsStr::new(s).encode_wide().collect();
    if wide.iter().any(|b| *b == 0) {
        panic!("nul byte in wide string");
    }
    wide.push(0);
    wide
}

fn target_name(registry_name: &str) -> Vec<u16> {
    wstr(&format!("cargo-registry:{}", registry_name))
}

impl Credential for WindowsCredential {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn get(&self, registry_name: &str, _api_url: &str) -> Result<String, Error> {
        let target_name = target_name(registry_name);
        let mut p_credential: wincred::PCREDENTIALW = std::ptr::null_mut();
        unsafe {
            if wincred::CredReadW(
                target_name.as_ptr(),
                wincred::CRED_TYPE_GENERIC,
                0,
                &mut p_credential,
            ) != TRUE
            {
                return Err(
                    format!("failed to fetch token: {}", std::io::Error::last_os_error()).into(),
                );
            }
            let bytes = std::slice::from_raw_parts(
                (*p_credential).CredentialBlob,
                (*p_credential).CredentialBlobSize as usize,
            );
            String::from_utf8(bytes.to_vec()).map_err(|_| "failed to convert token to UTF8".into())
        }
    }

    fn store(&self, registry_name: &str, _api_url: &str, token: &str) -> Result<(), Error> {
        let token = token.as_bytes();
        let target_name = target_name(registry_name);
        let comment = wstr("Cargo registry token");
        let mut credential = wincred::CREDENTIALW {
            Flags: 0,
            Type: wincred::CRED_TYPE_GENERIC,
            TargetName: target_name.as_ptr() as LPWSTR,
            Comment: comment.as_ptr() as LPWSTR,
            LastWritten: FILETIME::default(),
            CredentialBlobSize: token.len() as DWORD,
            CredentialBlob: token.as_ptr() as LPBYTE,
            Persist: wincred::CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: std::ptr::null_mut(),
            UserName: std::ptr::null_mut(),
        };
        let result = unsafe { wincred::CredWriteW(&mut credential, 0) };
        if result != TRUE {
            let err = std::io::Error::last_os_error();
            return Err(format!("failed to store token: {}", err).into());
        }
        Ok(())
    }

    fn erase(&self, registry_name: &str, _api_url: &str) -> Result<(), Error> {
        let target_name = target_name(registry_name);
        let result =
            unsafe { wincred::CredDeleteW(target_name.as_ptr(), wincred::CRED_TYPE_GENERIC, 0) };
        if result != TRUE {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(winerror::ERROR_NOT_FOUND as i32) {
                eprintln!("not currently logged in to `{}`", registry_name);
                return Ok(());
            }
            return Err(format!("failed to remove token: {}", err).into());
        }
        Ok(())
    }
}

fn main() {
    cargo_credential::main(WindowsCredential);
}
