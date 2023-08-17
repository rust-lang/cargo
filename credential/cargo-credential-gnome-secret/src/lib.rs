#[cfg(target_os = "linux")]
mod libsecret;
#[cfg(not(target_os = "linux"))]
pub use cargo_credential::UnsupportedCredential as GnomeSecret;
#[cfg(target_os = "linux")]
pub use libsecret::GnomeSecret;
