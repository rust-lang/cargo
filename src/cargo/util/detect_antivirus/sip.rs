//! Utilities to detect which parts of SIP (System Integrity Protection) are enabled.

use std::process::Command;

use anyhow::Context;

use crate::CargoResult;

/// Invoke `csrutil status`, and parse the output to figure out if Filesystem
/// Protections are enabled (disabled with `csrutil disable`, or selectively
/// with `csrutil enable --without fs`).
///
/// Might fail if the output changes in the future. If this happens, consider
/// one of the alternative implementations in <https://github.com/madsmtm/check_execution_policy>.
pub fn fs_from_command() -> CargoResult<bool> {
    // Invoke directly, to avoid issues if a weird PATH is set.
    let output = Command::new("/usr/bin/csrutil")
        .arg("status")
        .output()
        .context("failed invoking `/usr/bin/csrutil status`")?;

    if !output.status.success() {
        anyhow::bail!("`/usr/bin/csrutil status` failed: {output:?}");
    }

    let stdout = String::from_utf8(output.stdout).unwrap();

    if stdout.contains("Filesystem Protections: enabled")
        || stdout.contains("System Integrity Protection status: enabled")
    {
        Ok(true)
    } else if stdout.contains("Filesystem Protections: disabled")
        || stdout.contains("System Integrity Protection status: disabled")
    {
        Ok(false)
    } else {
        // We could consider making this a warning instead?
        Err(anyhow::format_err!(
            "could not parse output of `/usr/bin/csrutil status`: {stdout:?}",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_crash() {
        fs_from_command().unwrap();
    }
}
