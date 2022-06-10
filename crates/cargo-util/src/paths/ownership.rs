use anyhow::Result;
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

/// Checks the ownership of the given path matches the current user.
///
/// The `safe_directories` is a set of paths to allow, usually loaded from config.
pub fn validate_ownership(path: &Path, safe_directories: &HashSet<PathBuf>) -> Result<()> {
    if safe_directories.get(Path::new("*")).is_some() {
        return Ok(());
    }
    for safe_dir in safe_directories {
        if path.starts_with(safe_dir) {
            return Ok(());
        }
    }
    _validate_ownership(path)
}

#[cfg(unix)]
fn _validate_ownership(path: &Path) -> Result<()> {
    use super::symlink_metadata;
    use std::env;
    use std::os::unix::fs::MetadataExt;
    let meta = symlink_metadata(path)?;
    let current_user = unsafe { libc::geteuid() };
    fn get_username(uid: u32) -> String {
        unsafe {
            let amt = match libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) {
                n if n < 0 => 512 as usize,
                n => n as usize,
            };
            let mut buf = Vec::with_capacity(amt);
            let mut passwd: libc::passwd = std::mem::zeroed();
            let mut result = std::ptr::null_mut();
            match libc::getpwuid_r(
                uid,
                &mut passwd,
                buf.as_mut_ptr(),
                buf.capacity(),
                &mut result,
            ) {
                0 if !result.is_null() => {
                    let ptr = passwd.pw_name as *const _;
                    let bytes = std::ffi::CStr::from_ptr(ptr).to_bytes().to_vec();
                    String::from_utf8_lossy(&bytes).into_owned()
                }
                _ => String::from("Unknown"),
            }
        }
    }
    // This is used for testing to simulate a failure.
    let simulate = match env::var_os("__CARGO_TEST_OWNERSHIP") {
        Some(p) if path == p => true,
        _ => false,
    };
    if current_user != meta.uid() || simulate {
        return Err(OwnershipError {
            owner: get_username(meta.uid()),
            current_user: get_username(current_user),
            path: path.to_owned(),
        }
        .into());
    }
    Ok(())
}

#[cfg(windows)]
fn _validate_ownership(path: &Path) -> Result<()> {
    unsafe { windows::_validate_ownership(path) }
}

/// An error representing a file that is owned by a different user.
#[allow(dead_code)] // Debug is required by std Error
#[derive(Debug)]
pub struct OwnershipError {
    pub owner: String,
    pub current_user: String,
    pub path: PathBuf,
}

impl std::error::Error for OwnershipError {}

impl fmt::Display for OwnershipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(windows)]
mod windows {
    use anyhow::{bail, Error, Result};
    use std::env;
    use std::ffi::OsString;
    use std::io;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::Path;
    use std::ptr::null_mut;
    use winapi::shared::minwindef::{DWORD, FALSE, HLOCAL, TRUE};
    use winapi::shared::sddl::ConvertSidToStringSidW;
    use winapi::shared::winerror::ERROR_INSUFFICIENT_BUFFER;
    use winapi::um::accctrl::SE_FILE_OBJECT;
    use winapi::um::aclapi::GetNamedSecurityInfoW;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
    use winapi::um::securitybaseapi::{
        CheckTokenMembership, EqualSid, GetTokenInformation, IsValidSid, IsWellKnownSid,
    };
    use winapi::um::winbase::{LocalFree, LookupAccountSidW};
    use winapi::um::winnt::{
        TokenUser, WinBuiltinAdministratorsSid, DACL_SECURITY_INFORMATION, HANDLE,
        OWNER_SECURITY_INFORMATION, PSID, TOKEN_QUERY, TOKEN_USER,
    };

    pub(super) unsafe fn _validate_ownership(path: &Path) -> Result<()> {
        let me = GetCurrentProcess();
        let mut token = null_mut();
        if OpenProcessToken(me, TOKEN_QUERY, &mut token) == 0 {
            return Err(
                Error::new(io::Error::last_os_error()).context("failed to get process token")
            );
        }
        let token = Handle { inner: token };
        let mut len: DWORD = 0;
        // Get the size of the token buffer.
        if GetTokenInformation(token.inner, TokenUser, null_mut(), 0, &mut len) != 0
            || GetLastError() != ERROR_INSUFFICIENT_BUFFER
        {
            return Err(Error::new(io::Error::last_os_error())
                .context("failed to get token information size"));
        }
        // Get the SID of the current user.
        let mut token_info = Vec::<u8>::with_capacity(len as usize);
        if GetTokenInformation(
            token.inner,
            TokenUser,
            token_info.as_mut_ptr() as *mut _,
            len,
            &mut len,
        ) == 0
        {
            return Err(
                Error::new(io::Error::last_os_error()).context("failed to get token information")
            );
        }
        let token_user = token_info.as_ptr() as *const TOKEN_USER;
        let user_sid = (*token_user).User.Sid;

        // Get the SID of the owner of the path.
        let path_w = wide_path(path);
        let mut owner_sid = null_mut();
        let mut descriptor = LocalFreeWrapper { inner: null_mut() };
        let result = GetNamedSecurityInfoW(
            path_w.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner_sid,
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor.inner,
        );
        if result != 0 {
            let io_err = io::Error::from_raw_os_error(result as i32);
            return Err(Error::new(io_err).context(format!(
                "failed to get security descriptor for path {}",
                path.display()
            )));
        }
        if IsValidSid(owner_sid) == 0 {
            bail!(
                "unexpected invalid file owner sid for path {}",
                path.display()
            );
        }
        let simulate = match env::var_os("__CARGO_TEST_OWNERSHIP") {
            Some(p) if path == p => true,
            _ => false,
        };
        if !simulate && EqualSid(user_sid, owner_sid) != 0 {
            return Ok(());
        }
        // Allow paths that are owned by the Administrators Group if the user is
        // also a member of the group. This is added for convenience. Files
        // created when run with "Run as Administrator" are owned by the group.
        if !simulate && IsWellKnownSid(owner_sid, WinBuiltinAdministratorsSid) == TRUE {
            let mut is_member = FALSE;
            if CheckTokenMembership(null_mut(), user_sid, &mut is_member) == 0 {
                log::info!(
                    "failed to check if member of administrators: {}",
                    io::Error::last_os_error()
                );
                // Fall through
            } else {
                if is_member == TRUE {
                    return Ok(());
                }
            }
        }

        let owner = sid_to_name(owner_sid).unwrap_or_else(|| sid_to_string(owner_sid));
        let current_user = sid_to_name(user_sid).unwrap_or_else(|| sid_to_string(user_sid));
        return Err(super::OwnershipError {
            owner,
            current_user,
            path: path.to_owned(),
        }
        .into());
    }

    unsafe fn sid_to_string(sid: PSID) -> String {
        let mut s_ptr = null_mut();
        if ConvertSidToStringSidW(sid, &mut s_ptr) == 0 {
            log::info!(
                "failed to convert sid to string: {}",
                io::Error::last_os_error()
            );
            return "Unknown".to_string();
        }
        let len = (0..).take_while(|&i| *s_ptr.offset(i) != 0).count();
        let slice: &[u16] = std::slice::from_raw_parts(s_ptr, len);
        let s = OsString::from_wide(slice);
        LocalFree(s_ptr as *mut _);
        s.into_string().unwrap_or_else(|_| "Unknown".to_string())
    }

    unsafe fn sid_to_name(sid: PSID) -> Option<String> {
        // Note: This operation may be very expensive and slow.
        let mut name_size = 0;
        let mut domain_size = 0;
        let mut pe_use = 0;
        // Get the length of the name.
        if LookupAccountSidW(
            null_mut(), // lpSystemName (where to search)
            sid,
            null_mut(), // Name
            &mut name_size,
            null_mut(), // ReferencedDomainName
            &mut domain_size,
            &mut pe_use,
        ) != 0
            || GetLastError() != ERROR_INSUFFICIENT_BUFFER
        {
            log::debug!(
                "failed to determine sid name length: {}",
                io::Error::last_os_error()
            );
            return None;
        }
        let mut name: Vec<u16> = vec![0; name_size as usize];
        let mut domain: Vec<u16> = vec![0; domain_size as usize];
        if LookupAccountSidW(
            null_mut(),
            sid,
            name.as_mut_ptr(),
            &mut name_size,
            domain.as_mut_ptr(),
            &mut domain_size,
            &mut pe_use,
        ) == 0
        {
            log::debug!(
                "failed to fetch name ({}): {}",
                name_size,
                io::Error::last_os_error()
            );
            return None;
        }
        let name = str_from_wide(&name);
        let domain = str_from_wide(&domain);

        return Some(format!("{domain}\\{name}"));
    }

    struct Handle {
        inner: HANDLE,
    }
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.inner);
            }
        }
    }

    struct LocalFreeWrapper {
        inner: HLOCAL,
    }
    impl Drop for LocalFreeWrapper {
        fn drop(&mut self) {
            unsafe {
                if !self.inner.is_null() {
                    LocalFree(self.inner);
                    self.inner = null_mut();
                }
            }
        }
    }

    fn str_from_wide(wide: &[u16]) -> String {
        let len = wide.iter().position(|i| *i == 0).unwrap_or(wide.len());
        let os_str = OsString::from_wide(&wide[..len]);
        os_str
            .into_string()
            .unwrap_or_else(|_| "Invalid UTF-8".to_string())
    }

    fn wide_path(path: &Path) -> Vec<u16> {
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.iter().any(|b| *b == 0) {
            panic!("nul byte in wide string");
        }
        wide.push(0);
        wide
    }
}
