extern crate cc;
extern crate pkg_config;

#[cfg(target_env = "msvc")]
extern crate vcpkg;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let zlib_ng_compat = env::var("CARGO_FEATURE_ZLIB_NG_COMPAT").is_ok();

    if !zlib_ng_compat && try_vcpkg() {
        return;
    }

    // The system copy of libssh2 is not used by default because it
    // can lead to having two copies of libssl loaded at once.
    // See https://github.com/alexcrichton/ssh2-rs/pull/88
    println!("cargo:rerun-if-env-changed=LIBSSH2_SYS_USE_PKG_CONFIG");
    if env::var("LIBSSH2_SYS_USE_PKG_CONFIG").is_ok() {
        if zlib_ng_compat {
            panic!("LIBSSH2_SYS_USE_PKG_CONFIG set, but cannot use zlib-ng-compat with system libssh2");
        }
        if let Ok(lib) = pkg_config::find_library("libssh2") {
            for path in &lib.include_paths {
                println!("cargo:include={}", path.display());
            }
            return;
        }
    }

    if !Path::new("libssh2/.git").exists() {
        let _ = Command::new("git")
            .args(&["submodule", "update", "--init"])
            .status();
    }

    let target = env::var("TARGET").unwrap();
    let profile = env::var("PROFILE").unwrap();
    let dst = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut cfg = cc::Build::new();

    let include = dst.join("include");
    println!("cargo:include={}", include.display());
    println!("cargo:root={}", dst.display());
    let build = dst.join("build");
    cfg.out_dir(&build);
    fs::create_dir_all(&build).unwrap();
    fs::create_dir_all(&include).unwrap();

    fs::copy("libssh2/include/libssh2.h", include.join("libssh2.h")).unwrap();
    fs::copy(
        "libssh2/include/libssh2_publickey.h",
        include.join("libssh2_publickey.h"),
    )
    .unwrap();
    fs::copy(
        "libssh2/include/libssh2_sftp.h",
        include.join("libssh2_sftp.h"),
    )
    .unwrap();

    cfg.file("libssh2/src/agent.c")
        .file("libssh2/src/bcrypt_pbkdf.c")
        .file("libssh2/src/blowfish.c")
        .file("libssh2/src/chacha.c")
        .file("libssh2/src/channel.c")
        .file("libssh2/src/cipher-chachapoly.c")
        .file("libssh2/src/comp.c")
        .file("libssh2/src/crypt.c")
        .file("libssh2/src/crypto.c")
        .file("libssh2/src/global.c")
        .file("libssh2/src/hostkey.c")
        .file("libssh2/src/keepalive.c")
        .file("libssh2/src/kex.c")
        .file("libssh2/src/knownhost.c")
        .file("libssh2/src/mac.c")
        .file("libssh2/src/misc.c")
        .file("libssh2/src/packet.c")
        .file("libssh2/src/pem.c")
        .file("libssh2/src/poly1305.c")
        .file("libssh2/src/publickey.c")
        .file("libssh2/src/scp.c")
        .file("libssh2/src/session.c")
        .file("libssh2/src/sftp.c")
        .file("libssh2/src/transport.c")
        .file("libssh2/src/userauth.c")
        .file("libssh2/src/userauth_kbd_packet.c")
        .include(&include)
        .include("libssh2/src");

    cfg.define("HAVE_LONGLONG", None);

    if target.contains("windows") {
        cfg.include("libssh2/win32");
        cfg.define("LIBSSH2_WIN32", None);
        cfg.file("libssh2/src/agent_win.c");

        if env::var_os("CARGO_FEATURE_OPENSSL_ON_WIN32").is_some() {
            cfg.define("LIBSSH2_OPENSSL", None);
            cfg.define("HAVE_EVP_AES_128_CTR", None);
            let lib_prefix = if target.contains("windows-msvc") {
                "lib"
            } else {
                ""
            };
            println!("cargo:rustc-link-lib=static={lib_prefix}ssl");
            println!("cargo:rustc-link-lib=static={lib_prefix}crypto");
        } else {
            cfg.define("LIBSSH2_WINCNG", None);
        }
    } else {
        cfg.flag("-fvisibility=hidden");
        cfg.define("HAVE_SNPRINTF", None);
        cfg.define("HAVE_UNISTD_H", None);
        cfg.define("HAVE_INTTYPES_H", None);
        cfg.define("HAVE_STDLIB_H", None);
        cfg.define("HAVE_SYS_SELECT_H", None);
        cfg.define("HAVE_SYS_SOCKET_H", None);
        cfg.define("HAVE_SYS_IOCTL_H", None);
        cfg.define("HAVE_SYS_TIME_H", None);
        cfg.define("HAVE_SYS_UN_H", None);
        cfg.define("HAVE_O_NONBLOCK", None);
        cfg.define("LIBSSH2_OPENSSL", None);
        cfg.define("HAVE_LIBCRYPT32", None);
        cfg.define("HAVE_EVP_AES_128_CTR", None);
        cfg.define("HAVE_POLL", None);
        cfg.define("HAVE_GETTIMEOFDAY", None);

        // Create `libssh2_config.h`
        let config = fs::read_to_string("libssh2/src/libssh2_config_cmake.h.in").unwrap();
        let config = config
            .lines()
            .filter(|l| !l.contains("#cmakedefine"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(build.join("libssh2_config.h"), &config).unwrap();
        cfg.include(&build);
    }

    /* Enable newer diffie-hellman-group-exchange-sha1 syntax */
    cfg.define("LIBSSH2_DH_GEX_NEW", None);

    cfg.define("LIBSSH2_HAVE_ZLIB", None);

    if profile.contains("debug") {
        cfg.define("LIBSSH2DEBUG", None);
    }

    println!("cargo:rerun-if-env-changed=DEP_Z_INCLUDE");
    if let Some(path) = env::var_os("DEP_Z_INCLUDE") {
        cfg.include(path);
    }

    println!("cargo:rerun-if-env-changed=DEP_OPENSSL_INCLUDE");
    if let Some(path) = env::var_os("DEP_OPENSSL_INCLUDE") {
        if let Some(path) = env::split_paths(&path).next() {
            if let Some(path) = path.to_str() {
                if path.len() > 0 {
                    cfg.include(path);
                }
            }
        }
    }

    let libssh2h = fs::read_to_string("libssh2/include/libssh2.h").unwrap();
    let version_line = libssh2h
        .lines()
        .find(|l| l.contains("LIBSSH2_VERSION"))
        .unwrap();
    let version = &version_line[version_line.find('"').unwrap() + 1..version_line.len() - 1];

    let pkgconfig = dst.join("lib/pkgconfig");
    fs::create_dir_all(&pkgconfig).unwrap();
    fs::write(
        pkgconfig.join("libssh2.pc"),
        fs::read_to_string("libssh2/libssh2.pc.in")
            .unwrap()
            .replace("@prefix@", dst.to_str().unwrap())
            .replace("@exec_prefix@", "")
            .replace("@libdir@", dst.join("lib").to_str().unwrap())
            .replace("@includedir@", include.to_str().unwrap())
            .replace("@LIBS@", "")
            .replace("@LIBSREQUIRED@", "")
            .replace("@LIBSSH2VER@", version),
    )
    .unwrap();

    cfg.warnings(false);
    cfg.compile("ssh2");

    if target.contains("windows") {
        println!("cargo:rustc-link-lib=bcrypt");
        println!("cargo:rustc-link-lib=crypt32");
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=ntdll");
    }
}

#[cfg(not(target_env = "msvc"))]
fn try_vcpkg() -> bool {
    false
}

#[cfg(target_env = "msvc")]
fn try_vcpkg() -> bool {
    vcpkg::Config::new()
        .emit_includes(true)
        .probe("libssh2")
        .map(|_| {
            // found libssh2 which depends on openssl and zlib
            vcpkg::Config::new()
                .lib_name("libssl")
                .lib_name("libcrypto")
                .probe("openssl")
                .or_else(|_| {
                    // openssl 1.1 was not found, try openssl 1.0
                    vcpkg::Config::new()
                        .lib_name("libeay32")
                        .lib_name("ssleay32")
                        .probe("openssl")
                })
                .expect(
                    "configured libssh2 from vcpkg but could not \
                     find openssl libraries that it depends on",
                );

            vcpkg::Config::new()
                .lib_names("zlib", "zlib1")
                .probe("zlib")
                .expect(
                    "configured libssh2 from vcpkg but could not \
                     find the zlib library that it depends on",
                );

            println!("cargo:rustc-link-lib=crypt32");
            println!("cargo:rustc-link-lib=gdi32");
            println!("cargo:rustc-link-lib=user32");
        })
        .is_ok()
}
