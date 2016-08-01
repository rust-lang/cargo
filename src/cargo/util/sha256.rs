pub use self::imp::Sha256;

// Someone upstream will link to OpenSSL, so we don't need to explicitly
// link to it ourselves. Hence we pick up Sha256 digests from OpenSSL
#[cfg(not(windows))]
// allow improper ctypes because size_t falls under that in old compilers
#[allow(bad_style, improper_ctypes)]
mod imp {
    extern crate openssl;

    use std::io::Write;
    use self::openssl::crypto::hash::{Hasher, Type};

    pub struct Sha256(Hasher);

    impl Sha256 {
        pub fn new() -> Sha256 {
            Sha256(Hasher::new(Type::SHA256))
        }

        pub fn update(&mut self, bytes: &[u8]) {
            let _ = self.0.write_all(bytes);
        }

        pub fn finish(&mut self) -> [u8; 32] {
            let mut ret = [0u8; 32];
            ret.copy_from_slice(&self.0.finish()[..]);
            ret
        }
    }
}

// Leverage the crypto APIs that windows has built in.
#[cfg(windows)]
mod imp {
    extern crate winapi;
    extern crate advapi32;
    use std::io;
    use std::ptr;

    use self::winapi::{DWORD, HCRYPTPROV, HCRYPTHASH};
    use self::winapi::{PROV_RSA_AES, CRYPT_SILENT, CRYPT_VERIFYCONTEXT, CALG_SHA_256, HP_HASHVAL};
    use self::advapi32::{CryptAcquireContextW, CryptCreateHash, CryptDestroyHash};
    use self::advapi32::{CryptGetHashParam, CryptHashData, CryptReleaseContext};

    macro_rules! call{ ($e:expr) => ({
        if $e == 0 {
            panic!("failed {}: {}", stringify!($e), io::Error::last_os_error())
        }
    }) }

    pub struct Sha256 {
        hcryptprov: HCRYPTPROV,
        hcrypthash: HCRYPTHASH,
    }

    impl Sha256 {
        pub fn new() -> Sha256 {
            let mut hcp = 0;
            call!(unsafe {
                CryptAcquireContextW(&mut hcp, ptr::null(), ptr::null(),
                                     PROV_RSA_AES,
                                     CRYPT_VERIFYCONTEXT | CRYPT_SILENT)
            });
            let mut ret = Sha256 { hcryptprov: hcp, hcrypthash: 0 };
            call!(unsafe {
                CryptCreateHash(ret.hcryptprov, CALG_SHA_256,
                                0, 0, &mut ret.hcrypthash)
            });
            ret
        }

        pub fn update(&mut self, bytes: &[u8]) {
            call!(unsafe {
                CryptHashData(self.hcrypthash, bytes.as_ptr() as *mut _,
                              bytes.len() as DWORD, 0)
            })
        }

        pub fn finish(&mut self) -> [u8; 32] {
            let mut ret = [0u8; 32];
            let mut len = ret.len() as DWORD;
            call!(unsafe {
                CryptGetHashParam(self.hcrypthash, HP_HASHVAL, ret.as_mut_ptr(),
                                  &mut len, 0)
            });
            assert_eq!(len as usize, ret.len());
            ret
        }
    }

    impl Drop for Sha256 {
        fn drop(&mut self) {
            if self.hcrypthash != 0 {
                call!(unsafe { CryptDestroyHash(self.hcrypthash) });
            }
            call!(unsafe { CryptReleaseContext(self.hcryptprov, 0) });
        }
    }
}
