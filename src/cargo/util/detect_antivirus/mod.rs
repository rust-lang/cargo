//! # Utilities for detecting whether antivirus software is active
//!
//! Antivirus software such as Microsoft Defender and macOS' XProtect usually
//! intercept process creation for new binaries, and do a signature-based
//! check to see if the binary contains known malware:
//! <https://support.apple.com/en-gb/guide/security/sec469d47bd8/web>
//!
//! Most users do not create new binaries all the time (and some malware
//! allegedly got around antivirus checks by doing this in the past), so from
//! a security standpoint it makes sense to spend time analyzing these.
//!
//! But developers *do* often create new binaries (Cargo does it for build
//! scripts and tests), and since there is a fairly high cost to these checks,
//! it makes sense for us to guide the user towards selectively disabling
//! these security features of their OS to allow for faster iteration time
//! when developing their software.

use super::{CargoResult, GlobalContext};

#[cfg(target_os = "macos")]
mod execution_policy;
#[cfg(target_os = "macos")]
mod sip;

/// Detect and report if macOS' XProtect (Gatekeeper) is enabled in the
/// context of the current process, and thus likely to introduce overhead
/// in the first launch of binaries we create.
///
/// This is the case if the top-level program that we're running under
/// (often Terminal.app or sshd-keygen-wrapper if in an ssh session) is
/// marked as having Developer Tool permissions, or if (parts of) SIP is
/// disabled.
///
/// NOTE: This check is not necessarily exhaustive - there might be other
/// yet-unknown factors that influence how long it takes to launch a newly
/// created binary? This fact is part of the motivation for allowing the
/// user to opt-out of the check.
#[cfg(target_os = "macos")] // Host macOS
pub fn detect_and_report(gtcx: &GlobalContext) -> CargoResult<()> {
    use self::execution_policy::{EPDeveloperTool, EPDeveloperToolStatus, ExecutionPolicyHandle};

    // We use Objective-C objects in here, use an autorelease pool to make
    // sure it's cleaned up afterwards.
    objc2::rc::autoreleasepool(|_| {
        let Some(handle) = ExecutionPolicyHandle::open()? else {
            tracing::debug!("the ExecutionPolicy framework is (expectedly) not available");
            return Ok(());
        };

        let developer_tool = EPDeveloperTool::new(&handle)?;

        // Check whether we're running under an environment that has the
        // "Developer Tool" grant.
        let status = developer_tool.authorization_status();
        let status_str = match status {
            EPDeveloperToolStatus::NOT_DETERMINED => "not determined",
            EPDeveloperToolStatus::RESTRICTED => "restricted",
            EPDeveloperToolStatus::DENIED => "denied",
            EPDeveloperToolStatus::AUTHORIZED => "authorized",
            _ => "unknown",
        };
        tracing::debug!("Developer Tool authorization status: {status_str}");
        if status == EPDeveloperToolStatus::AUTHORIZED {
            // We are! No need to report anything then, newly created binaries
            // should be fast to run from the get-go.
            return Ok(());
        }

        // Otherwise, detect if SIP's Filesystem Protections are disabled.
        //
        // We do this check secondly, because the "happy path" / the fast path
        // should be that the user has Developer Tool authorization.
        let sip_fs_enabled = sip::fs_from_command()?;
        tracing::debug!("are SIP Filesystem Protections enabled? {sip_fs_enabled}");
        if !sip_fs_enabled {
            // They are! Also no need to report anything here.
            return Ok(());
        }

        // If we aren't authorized, attempt to request "Developer Tool"
        // privileges from the system.
        //
        // NOTE: This has the side-effect of adding the parent binary to
        // `System Preferences > Security & Privacy > Developer Tools`, even
        // if the request fails, which is why we do this as the last resort.
        //
        // The side-effect is desired though, because it makes it much easier
        // for the user to see which binary they actually need to allow as a
        // Developer Tool (e.g. if using a third-party terminal like iTerm).
        //
        // This is kinda similar to `spctl developer-mode enable-terminal`,
        // except that the binary that is added there is always Terminal.app.
        let res = developer_tool.request_access()?;
        if res {
            // Our request for access was granted! No need to report anything.
            return Ok(());
        }

        gtcx.shell().note(
            "detected that XProtect is enabled in this session, which may \
            slow down builds as it scans build scripts and test binaries \
            before they are run.\
            \n\
            If you trust the software that you run in your terminal, then \
            this overhead can be avoided by giving it more permissions under \
            `System Preferences > Security & Privacy > Developer Tools`. \
            (Cargo has made an entry for the current terminal appear there, \
            though you will need to go and manually enable it).\
            \n\
            Alternatively, you can disable this note by adding \
            `build.detect-antivirus = false` to your ~/.cargo/config.toml.\
            \n\
            See <https://doc.rust-lang.org/cargo/appendix/antivirus.html#xprotect> \
            for more information.",
        )?;

        Ok(())
    })
}

#[cfg(not(target_os = "macos"))]
pub fn detect_and_report(_gtcx: &GlobalContext) -> CargoResult<()> {
    Ok(())
}
