//! Cargo registry gnome libsecret credential process.

#[cfg(target_os = "linux")]
mod libsecret;
#[cfg(not(target_os = "linux"))]
use cargo_credential::UnsupportedCredential as GnomeSecret;
#[cfg(target_os = "linux")]
use libsecret::GnomeSecret;

fn main() {
    cargo_credential::main(GnomeSecret);
}
