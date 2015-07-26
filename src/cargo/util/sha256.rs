pub use self::imp::Sha256;

// Someone upstream will link to OpenSSL, so we don't need to explicitly
// link to it ourselves. Hence we pick up Sha256 digests from OpenSSL
#[cfg(not(windows))]
#[allow(bad_style)]
mod imp {
    use libc;

    enum EVP_MD_CTX {}
    enum EVP_MD {}
    enum ENGINE {}

    extern {
        fn EVP_DigestInit_ex(ctx: *mut EVP_MD_CTX,
                             kind: *const EVP_MD,
                             imp: *mut ENGINE) -> libc::c_int;
        fn EVP_DigestUpdate(ctx: *mut EVP_MD_CTX,
                            d: *const libc::c_void,
                            cnt: libc::size_t) -> libc::c_int;
        fn EVP_DigestFinal_ex(ctx: *mut EVP_MD_CTX, md: *mut libc::c_uchar,
                              s: *mut libc::c_uint) -> libc::c_int;
        fn EVP_MD_CTX_create() -> *mut EVP_MD_CTX;
        fn EVP_MD_CTX_destroy(ctx: *mut EVP_MD_CTX);
        fn EVP_sha256() -> *const EVP_MD;
    }

    pub struct Sha256 { ctx: *mut EVP_MD_CTX }

    impl Sha256 {
        pub fn new() -> Sha256 {
            unsafe {
                let ctx = EVP_MD_CTX_create();
                assert!(!ctx.is_null());
                let ret = Sha256 { ctx: ctx };
                let n = EVP_DigestInit_ex(ret.ctx, EVP_sha256(), 0 as *mut _);
                assert_eq!(n, 1);
                return ret;
            }
        }

        pub fn update(&mut self, bytes: &[u8]) {
            unsafe {
                let n = EVP_DigestUpdate(self.ctx, bytes.as_ptr() as *const _,
                                         bytes.len() as libc::size_t);
                assert_eq!(n, 1);
            }
        }

        pub fn finish(&mut self) -> [u8; 32] {
            unsafe {
                let mut ret = [0u8; 32];
                let mut out = 0;
                let n = EVP_DigestFinal_ex(self.ctx, ret.as_mut_ptr(), &mut out);
                assert_eq!(n, 1);
                assert_eq!(out, 32);
                return ret;
            }
        }
    }

    impl Drop for Sha256 {
        fn drop(&mut self) {
            unsafe { EVP_MD_CTX_destroy(self.ctx) }
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
            return ret;
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
            return ret;
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
