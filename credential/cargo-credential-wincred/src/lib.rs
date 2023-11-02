//! Cargo registry windows credential process.

#[cfg(windows)]
mod win {
    use cargo_credential::{read_token, Action, CacheControl, CredentialResponse, RegistryInfo};
    use cargo_credential::{Credential, Error};
    use std::ffi::OsStr;

    use std::os::windows::ffi::OsStrExt;

    use windows_sys::core::PWSTR;
    use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::Foundation::TRUE;
    use windows_sys::Win32::Security::Credentials::CredReadW;
    use windows_sys::Win32::Security::Credentials::CredWriteW;
    use windows_sys::Win32::Security::Credentials::CREDENTIALW;
    use windows_sys::Win32::Security::Credentials::CRED_PERSIST_LOCAL_MACHINE;
    use windows_sys::Win32::Security::Credentials::CRED_TYPE_GENERIC;
    use windows_sys::Win32::Security::Credentials::{CredDeleteW, CredFree};

    pub struct WindowsCredential;

    /// Converts a string to a nul-terminated wide UTF-16 byte sequence.
    fn wstr(s: &str) -> Vec<u16> {
        let mut wide: Vec<u16> = OsStr::new(s).encode_wide().collect();
        if wide.iter().any(|b| *b == 0) {
            panic!("nul byte in wide string");
        }
        wide.push(0);
        wide
    }

    fn target_name(index_url: &str) -> Vec<u16> {
        wstr(&format!("cargo-registry:{}", index_url))
    }

    impl Credential for WindowsCredential {
        fn perform(
            &self,
            registry: &RegistryInfo<'_>,
            action: &Action<'_>,
            _args: &[&str],
        ) -> Result<CredentialResponse, Error> {
            match action {
                Action::Get(_) => {
                    let target_name = target_name(registry.index_url);
                    let mut p_credential: *mut CREDENTIALW = std::ptr::null_mut() as *mut _;
                    let bytes = unsafe {
                        if CredReadW(
                            target_name.as_ptr(),
                            CRED_TYPE_GENERIC,
                            0,
                            &mut p_credential as *mut _,
                        ) != TRUE
                        {
                            let err = std::io::Error::last_os_error();
                            if err.raw_os_error() == Some(ERROR_NOT_FOUND as i32) {
                                return Err(Error::NotFound);
                            }
                            return Err(Box::new(err).into());
                        }
                        std::slice::from_raw_parts(
                            (*p_credential).CredentialBlob,
                            (*p_credential).CredentialBlobSize as usize,
                        )
                    };
                    let token = String::from_utf8(bytes.to_vec()).map_err(Box::new);
                    unsafe { CredFree(p_credential as *mut _) };
                    Ok(CredentialResponse::Get {
                        token: token?.into(),
                        cache: CacheControl::Session,
                        operation_independent: true,
                    })
                }
                Action::Login(options) => {
                    let token = read_token(options, registry)?.expose();
                    let target_name = target_name(registry.index_url);
                    let comment = wstr("Cargo registry token");
                    let credential = CREDENTIALW {
                        Flags: 0,
                        Type: CRED_TYPE_GENERIC,
                        TargetName: target_name.as_ptr() as PWSTR,
                        Comment: comment.as_ptr() as PWSTR,
                        LastWritten: FILETIME {
                            dwLowDateTime: 0,
                            dwHighDateTime: 0,
                        },
                        CredentialBlobSize: token.len() as u32,
                        CredentialBlob: token.as_bytes().as_ptr() as *mut u8,
                        Persist: CRED_PERSIST_LOCAL_MACHINE,
                        AttributeCount: 0,
                        Attributes: std::ptr::null_mut(),
                        TargetAlias: std::ptr::null_mut(),
                        UserName: std::ptr::null_mut(),
                    };
                    let result = unsafe { CredWriteW(&credential, 0) };
                    if result != TRUE {
                        let err = std::io::Error::last_os_error();
                        return Err(Box::new(err).into());
                    }
                    Ok(CredentialResponse::Login)
                }
                Action::Logout => {
                    let target_name = target_name(registry.index_url);
                    let result = unsafe { CredDeleteW(target_name.as_ptr(), CRED_TYPE_GENERIC, 0) };
                    if result != TRUE {
                        let err = std::io::Error::last_os_error();
                        if err.raw_os_error() == Some(ERROR_NOT_FOUND as i32) {
                            return Err(Error::NotFound);
                        }
                        return Err(Box::new(err).into());
                    }
                    Ok(CredentialResponse::Logout)
                }
                _ => Err(Error::OperationNotSupported),
            }
        }
    }
}

#[cfg(not(windows))]
pub use cargo_credential::UnsupportedCredential as WindowsCredential;
#[cfg(windows)]
pub use win::WindowsCredential;
