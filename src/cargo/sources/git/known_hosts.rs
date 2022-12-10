//! SSH host key validation support.
//!
//! A primary goal with this implementation is to provide user-friendly error
//! messages, guiding them to understand the issue and how to resolve it.
//!
//! Note that there are a lot of limitations here. This reads OpenSSH
//! known_hosts files from well-known locations, but it does not read OpenSSH
//! config files. The config file can change the behavior of how OpenSSH
//! handles known_hosts files. For example, some things we don't handle:
//!
//! - `GlobalKnownHostsFile` — Changes the location of the global host file.
//! - `UserKnownHostsFile` — Changes the location of the user's host file.
//! - `KnownHostsCommand` — A command to fetch known hosts.
//! - `CheckHostIP` — DNS spoofing checks.
//! - `VisualHostKey` — Shows a visual ascii-art key.
//! - `VerifyHostKeyDNS` — Uses SSHFP DNS records to fetch a host key.
//!
//! There's also a number of things that aren't supported but could be easily
//! added (it just adds a little complexity). For example, hashed hostnames,
//! hostname patterns, and revoked markers. See "FIXME" comments littered in
//! this file.

use crate::util::config::{Definition, Value};
use git2::cert::Cert;
use git2::CertificateCheckStatus;
use std::collections::HashSet;
use std::fmt::Write;
use std::path::{Path, PathBuf};

/// These are host keys that are hard-coded in cargo to provide convenience.
///
/// If GitHub ever publishes new keys, the user can add them to their own
/// configuration file to use those instead.
///
/// The GitHub keys are sourced from <https://api.github.com/meta> or
/// <https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/githubs-ssh-key-fingerprints>.
///
/// These will be ignored if the user adds their own entries for `github.com`,
/// which can be useful if GitHub ever revokes their old keys.
static BUNDLED_KEYS: &[(&str, &str, &str)] = &[
    ("github.com", "ssh-ed25519", "AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl"),
    ("github.com", "ecdsa-sha2-nistp256", "AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBEmKSENjQEezOmxkZMy7opKgwFB9nkt5YRrYMjNuG5N87uRgg6CLrbo5wAdT/y6v0mKV0U2w0WZ2YB/++Tpockg="),
    ("github.com", "ssh-rsa", "AAAAB3NzaC1yc2EAAAABIwAAAQEAq2A7hRGmdnm9tUDbO9IDSwBK6TbQa+PXYPCPy6rbTrTtw7PHkccKrpp0yVhp5HdEIcKr6pLlVDBfOLX9QUsyCOV0wzfjIJNlGEYsdlLJizHhbn2mUjvSAHQqZETYP81eFzLQNnPHt4EVVUh7VfDESU84KezmD5QlWpXLmvU31/yMf+Se8xhHTvKSCZIFImWwoG6mbUoWf9nzpIoaSjB+weqqUUmpaaasXVal72J+UX2B+2RPW3RcT0eOzQgqlJL3RKrTJvdsjE3JEAvGq3lGHSZXy28G3skua2SmVi/w4yCE6gbODqnTWlg7+wC604ydGXA8VJiS5ap43JXiUFFAaQ=="),
];

enum KnownHostError {
    /// Some general error happened while validating the known hosts.
    CheckError(anyhow::Error),
    /// The host key was not found.
    HostKeyNotFound {
        hostname: String,
        key_type: git2::cert::SshHostKeyType,
        remote_host_key: String,
        remote_fingerprint: String,
        other_hosts: Vec<KnownHost>,
    },
    /// The host key was found, but does not match the remote's key.
    HostKeyHasChanged {
        hostname: String,
        key_type: git2::cert::SshHostKeyType,
        old_known_host: KnownHost,
        remote_host_key: String,
        remote_fingerprint: String,
    },
}

impl From<anyhow::Error> for KnownHostError {
    fn from(err: anyhow::Error) -> KnownHostError {
        KnownHostError::CheckError(err.into())
    }
}

/// The location where a host key was located.
#[derive(Clone)]
enum KnownHostLocation {
    /// Loaded from a file from disk.
    File { path: PathBuf, lineno: u32 },
    /// Loaded from cargo's config system.
    Config { definition: Definition },
    /// Part of the hard-coded bundled keys in Cargo.
    Bundled,
}

/// The git2 callback used to validate a certificate (only ssh known hosts are validated).
pub fn certificate_check(
    cert: &Cert<'_>,
    host: &str,
    port: Option<u16>,
    config_known_hosts: Option<&Vec<Value<String>>>,
    diagnostic_home_config: &str,
) -> Result<CertificateCheckStatus, git2::Error> {
    let Some(host_key) = cert.as_hostkey() else {
        // Return passthrough for TLS X509 certificates to use whatever validation
        // was done in git2.
        return Ok(CertificateCheckStatus::CertificatePassthrough)
    };
    // If a nonstandard port is in use, check for that first.
    // The fallback to check without a port is handled in the HostKeyNotFound handler.
    let host_maybe_port = match port {
        Some(port) if port != 22 => format!("[{host}]:{port}"),
        _ => host.to_string(),
    };
    // The error message must be constructed as a string to pass through the libgit2 C API.
    let err_msg = match check_ssh_known_hosts(host_key, &host_maybe_port, config_known_hosts) {
        Ok(()) => {
            return Ok(CertificateCheckStatus::CertificateOk);
        }
        Err(KnownHostError::CheckError(e)) => {
            format!("error: failed to validate host key:\n{:#}", e)
        }
        Err(KnownHostError::HostKeyNotFound {
            hostname,
            key_type,
            remote_host_key,
            remote_fingerprint,
            other_hosts,
        }) => {
            // Try checking without the port.
            if port.is_some()
                && !matches!(port, Some(22))
                && check_ssh_known_hosts(host_key, host, config_known_hosts).is_ok()
            {
                return Ok(CertificateCheckStatus::CertificateOk);
            }
            let key_type_short_name = key_type.short_name();
            let key_type_name = key_type.name();
            let known_hosts_location = user_known_host_location_to_add(diagnostic_home_config);
            let other_hosts_message = if other_hosts.is_empty() {
                String::new()
            } else {
                let mut msg = String::from(
                    "Note: This host key was found, \
                    but is associated with a different host:\n",
                );
                for known_host in other_hosts {
                    let loc = match known_host.location {
                        KnownHostLocation::File { path, lineno } => {
                            format!("{} line {lineno}", path.display())
                        }
                        KnownHostLocation::Config { definition } => {
                            format!("config value from {definition}")
                        }
                        KnownHostLocation::Bundled => format!("bundled with cargo"),
                    };
                    write!(msg, "    {loc}: {}\n", known_host.patterns).unwrap();
                }
                msg
            };
            format!("error: unknown SSH host key\n\
                The SSH host key for `{hostname}` is not known and cannot be validated.\n\
                \n\
                To resolve this issue, add the host key to {known_hosts_location}\n\
                \n\
                The key to add is:\n\
                \n\
                {hostname} {key_type_name} {remote_host_key}\n\
                \n\
                The {key_type_short_name} key fingerprint is: SHA256:{remote_fingerprint}\n\
                This fingerprint should be validated with the server administrator that it is correct.\n\
                {other_hosts_message}\n\
                See https://doc.rust-lang.org/nightly/cargo/appendix/git-authentication.html#ssh-known-hosts \
                for more information.\n\
                ")
        }
        Err(KnownHostError::HostKeyHasChanged {
            hostname,
            key_type,
            old_known_host,
            remote_host_key,
            remote_fingerprint,
        }) => {
            let key_type_short_name = key_type.short_name();
            let key_type_name = key_type.name();
            let known_hosts_location = user_known_host_location_to_add(diagnostic_home_config);
            let old_key_resolution = match old_known_host.location {
                KnownHostLocation::File { path, lineno } => {
                    let old_key_location = path.display();
                    format!(
                        "removing the old {key_type_name} key for `{hostname}` \
                        located at {old_key_location} line {lineno}, \
                        and adding the new key to {known_hosts_location}",
                    )
                }
                KnownHostLocation::Config { definition } => {
                    format!(
                        "removing the old {key_type_name} key for `{hostname}` \
                        loaded from Cargo's config at {definition}, \
                        and adding the new key to {known_hosts_location}"
                    )
                }
                KnownHostLocation::Bundled => {
                    format!(
                        "adding the new key to {known_hosts_location}\n\
                        The current host key is bundled as part of Cargo."
                    )
                }
            };
            format!("error: SSH host key has changed for `{hostname}`\n\
                *********************************\n\
                * WARNING: HOST KEY HAS CHANGED *\n\
                *********************************\n\
                This may be caused by a man-in-the-middle attack, or the \
                server may have changed its host key.\n\
                \n\
                The {key_type_short_name} fingerprint for the key from the remote host is:\n\
                    SHA256:{remote_fingerprint}\n\
                \n\
                You are strongly encouraged to contact the server \
                administrator for `{hostname}` to verify that this new key is \
                correct.\n\
                \n\
                If you can verify that the server has a new key, you can \
                resolve this error by {old_key_resolution}\n\
                \n\
                The key provided by the remote host is:\n\
                \n\
                {hostname} {key_type_name} {remote_host_key}\n\
                \n\
                See https://doc.rust-lang.org/nightly/cargo/appendix/git-authentication.html#ssh-known-hosts \
                for more information.\n\
                ")
        }
    };
    Err(git2::Error::new(
        git2::ErrorCode::GenericError,
        git2::ErrorClass::Callback,
        err_msg,
    ))
}

/// Checks if the given host/host key pair is known.
fn check_ssh_known_hosts(
    cert_host_key: &git2::cert::CertHostkey<'_>,
    host: &str,
    config_known_hosts: Option<&Vec<Value<String>>>,
) -> Result<(), KnownHostError> {
    let Some(remote_host_key) = cert_host_key.hostkey() else {
        return Err(anyhow::format_err!("remote host key is not available").into());
    };
    let remote_key_type = cert_host_key.hostkey_type().unwrap();
    // `changed_key` keeps track of any entries where the key has changed.
    let mut changed_key = None;
    // `other_hosts` keeps track of any entries that have an identical key,
    // but a different hostname.
    let mut other_hosts = Vec::new();

    // Collect all the known host entries from disk.
    let mut known_hosts = Vec::new();
    for path in known_host_files() {
        if !path.exists() {
            continue;
        }
        let hosts = load_hostfile(&path)?;
        known_hosts.extend(hosts);
    }
    if let Some(config_known_hosts) = config_known_hosts {
        // Format errors aren't an error in case the format needs to change in
        // the future, to retain forwards compatibility.
        for line_value in config_known_hosts {
            let location = KnownHostLocation::Config {
                definition: line_value.definition.clone(),
            };
            match parse_known_hosts_line(&line_value.val, location) {
                Some(known_host) => known_hosts.push(known_host),
                None => log::warn!(
                    "failed to parse known host {} from {}",
                    line_value.val,
                    line_value.definition
                ),
            }
        }
    }
    // Load the bundled keys. Don't add keys for hosts that the user has
    // configured, which gives them the option to override them. This could be
    // useful if the keys are ever revoked.
    let configured_hosts: HashSet<_> = known_hosts
        .iter()
        .flat_map(|known_host| {
            known_host
                .patterns
                .split(',')
                .map(|pattern| pattern.to_lowercase())
        })
        .collect();
    for (patterns, key_type, key) in BUNDLED_KEYS {
        if !configured_hosts.contains(*patterns) {
            let key = base64::decode(key).unwrap();
            known_hosts.push(KnownHost {
                location: KnownHostLocation::Bundled,
                patterns: patterns.to_string(),
                key_type: key_type.to_string(),
                key,
            });
        }
    }

    for known_host in known_hosts {
        // The key type from libgit2 needs to match the key type from the host file.
        if known_host.key_type != remote_key_type.name() {
            continue;
        }
        let key_matches = known_host.key == remote_host_key;
        if !known_host.host_matches(host) {
            // `name` can be None for hashed hostnames (which libgit2 does not expose).
            if key_matches {
                other_hosts.push(known_host.clone());
            }
            continue;
        }
        if key_matches {
            return Ok(());
        }
        // The host and key type matched, but the key itself did not.
        // This indicates the key has changed.
        // This is only reported as an error if no subsequent lines have a
        // correct key.
        changed_key = Some(known_host.clone());
    }
    // Older versions of OpenSSH (before 6.8, March 2015) showed MD5
    // fingerprints (see FingerprintHash ssh config option). Here we only
    // support SHA256.
    let mut remote_fingerprint = cargo_util::Sha256::new();
    remote_fingerprint.update(remote_host_key);
    let remote_fingerprint =
        base64::encode_config(remote_fingerprint.finish(), base64::STANDARD_NO_PAD);
    let remote_host_key = base64::encode(remote_host_key);
    // FIXME: Ideally the error message should include the IP address of the
    // remote host (to help the user validate that they are connecting to the
    // host they were expecting to). However, I don't see a way to obtain that
    // information from libgit2.
    match changed_key {
        Some(old_known_host) => Err(KnownHostError::HostKeyHasChanged {
            hostname: host.to_string(),
            key_type: remote_key_type,
            old_known_host,
            remote_host_key,
            remote_fingerprint,
        }),
        None => Err(KnownHostError::HostKeyNotFound {
            hostname: host.to_string(),
            key_type: remote_key_type,
            remote_host_key,
            remote_fingerprint,
            other_hosts,
        }),
    }
}

/// Returns a list of files to try loading OpenSSH-formatted known hosts.
fn known_host_files() -> Vec<PathBuf> {
    let mut result = Vec::new();
    if cfg!(unix) {
        result.push(PathBuf::from("/etc/ssh/ssh_known_hosts"));
    } else if cfg!(windows) {
        // The msys/cygwin version of OpenSSH uses `/etc` from the posix root
        // filesystem there (such as `C:\msys64\etc\ssh\ssh_known_hosts`).
        // However, I do not know of a way to obtain that location from
        // Windows-land. The ProgramData version here is what the PowerShell
        // port of OpenSSH does.
        if let Some(progdata) = std::env::var_os("ProgramData") {
            let mut progdata = PathBuf::from(progdata);
            progdata.push("ssh");
            progdata.push("ssh_known_hosts");
            result.push(progdata)
        }
    }
    result.extend(user_known_host_location());
    result
}

/// The location of the user's known_hosts file.
fn user_known_host_location() -> Option<PathBuf> {
    // NOTE: This is a potentially inaccurate prediction of what the user
    // actually wants. The actual location depends on several factors:
    //
    // - Windows OpenSSH Powershell version: I believe this looks up the home
    //   directory via ProfileImagePath in the registry, falling back to
    //   `GetWindowsDirectoryW` if that fails.
    // - OpenSSH Portable (under msys): This is very complicated. I got lost
    //   after following it through some ldap/active directory stuff.
    // - OpenSSH (most unix platforms): Uses `pw->pw_dir` from `getpwuid()`.
    //
    // This doesn't do anything close to that. home_dir's behavior is:
    // - Windows: $USERPROFILE, or SHGetFolderPathW()
    // - Unix: $HOME, or getpwuid_r()
    //
    // Since there is a mismatch here, the location returned here might be
    // different than what the user's `ssh` CLI command uses. We may want to
    // consider trying to align it better.
    home::home_dir().map(|mut home| {
        home.push(".ssh");
        home.push("known_hosts");
        home
    })
}

/// The location to display in an error message instructing the user where to
/// add the new key.
fn user_known_host_location_to_add(diagnostic_home_config: &str) -> String {
    // Note that we don't bother with the legacy known_hosts2 files.
    let user = user_known_host_location();
    let openssh_loc = match &user {
        Some(path) => path.to_str().expect("utf-8 home"),
        None => "~/.ssh/known_hosts",
    };
    format!(
        "the `net.ssh.known-hosts` array in your Cargo configuration \
        (such as {diagnostic_home_config}) \
        or in your OpenSSH known_hosts file at {openssh_loc}"
    )
}

/// A single known host entry.
#[derive(Clone)]
struct KnownHost {
    location: KnownHostLocation,
    /// The hostname. May be comma separated to match multiple hosts.
    patterns: String,
    key_type: String,
    key: Vec<u8>,
}

impl KnownHost {
    /// Returns whether or not the given host matches this known host entry.
    fn host_matches(&self, host: &str) -> bool {
        let mut match_found = false;
        let host = host.to_lowercase();
        // FIXME: support hashed hostnames
        for pattern in self.patterns.split(',') {
            let pattern = pattern.to_lowercase();
            // FIXME: support * and ? wildcards
            if let Some(pattern) = pattern.strip_prefix('!') {
                if pattern == host {
                    return false;
                }
            } else {
                match_found = pattern == host;
            }
        }
        match_found
    }
}

/// Loads an OpenSSH known_hosts file.
fn load_hostfile(path: &Path) -> Result<Vec<KnownHost>, anyhow::Error> {
    let contents = cargo_util::paths::read(path)?;
    let entries = contents
        .lines()
        .enumerate()
        .filter_map(|(lineno, line)| {
            let location = KnownHostLocation::File {
                path: path.to_path_buf(),
                lineno: lineno as u32 + 1,
            };
            parse_known_hosts_line(line, location)
        })
        .collect();
    Ok(entries)
}

fn parse_known_hosts_line(line: &str, location: KnownHostLocation) -> Option<KnownHost> {
    let line = line.trim();
    // FIXME: @revoked and @cert-authority is currently not supported.
    if line.is_empty() || line.starts_with('#') || line.starts_with('@') {
        return None;
    }
    let mut parts = line.split([' ', '\t']).filter(|s| !s.is_empty());
    let Some(patterns) = parts.next() else { return None };
    let Some(key_type) = parts.next() else { return None };
    let Some(key) = parts.next() else { return None };
    let Ok(key) = base64::decode(key) else { return None };
    Some(KnownHost {
        location,
        patterns: patterns.to_string(),
        key_type: key_type.to_string(),
        key,
    })
}
