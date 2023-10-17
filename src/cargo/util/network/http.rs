//! Configures libcurl's http handles.

use std::str;
use std::time::Duration;

use anyhow::bail;
use curl::easy::Easy;
use curl::easy::InfoType;
use curl::easy::SslOpt;
use curl::easy::SslVersion;
use tracing::debug;
use tracing::trace;

use crate::util::config::SslVersionConfig;
use crate::util::config::SslVersionConfigRange;
use crate::version;
use crate::CargoResult;
use crate::Config;

/// Creates a new HTTP handle with appropriate global configuration for cargo.
pub fn http_handle(config: &Config) -> CargoResult<Easy> {
    let (mut handle, timeout) = http_handle_and_timeout(config)?;
    timeout.configure(&mut handle)?;
    Ok(handle)
}

pub fn http_handle_and_timeout(config: &Config) -> CargoResult<(Easy, HttpTimeout)> {
    if config.frozen() {
        bail!(
            "attempting to make an HTTP request, but --frozen was \
             specified"
        )
    }
    if config.offline() {
        bail!(
            "attempting to make an HTTP request, but --offline was \
             specified"
        )
    }

    // The timeout option for libcurl by default times out the entire transfer,
    // but we probably don't want this. Instead we only set timeouts for the
    // connect phase as well as a "low speed" timeout so if we don't receive
    // many bytes in a large-ish period of time then we time out.
    let mut handle = Easy::new();
    let timeout = configure_http_handle(config, &mut handle)?;
    Ok((handle, timeout))
}

// Only use a custom transport if any HTTP options are specified,
// such as proxies or custom certificate authorities.
//
// The custom transport, however, is not as well battle-tested.
pub fn needs_custom_http_transport(config: &Config) -> CargoResult<bool> {
    Ok(
        super::proxy::http_proxy_exists(config.http_config()?, config)
            || *config.http_config()? != Default::default()
            || config.get_env_os("HTTP_TIMEOUT").is_some(),
    )
}

/// Configure a libcurl http handle with the defaults options for Cargo
pub fn configure_http_handle(config: &Config, handle: &mut Easy) -> CargoResult<HttpTimeout> {
    let http = config.http_config()?;
    if let Some(proxy) = super::proxy::http_proxy(http) {
        handle.proxy(&proxy)?;
    }
    if let Some(cainfo) = &http.cainfo {
        let cainfo = cainfo.resolve_path(config);
        handle.cainfo(&cainfo)?;
    }
    if let Some(check) = http.check_revoke {
        handle.ssl_options(SslOpt::new().no_revoke(!check))?;
    }

    if let Some(user_agent) = &http.user_agent {
        handle.useragent(user_agent)?;
    } else {
        handle.useragent(&format!("cargo {}", version()))?;
    }

    fn to_ssl_version(s: &str) -> CargoResult<SslVersion> {
        let version = match s {
            "default" => SslVersion::Default,
            "tlsv1" => SslVersion::Tlsv1,
            "tlsv1.0" => SslVersion::Tlsv10,
            "tlsv1.1" => SslVersion::Tlsv11,
            "tlsv1.2" => SslVersion::Tlsv12,
            "tlsv1.3" => SslVersion::Tlsv13,
            _ => bail!(
                "Invalid ssl version `{s}`,\
                 choose from 'default', 'tlsv1', 'tlsv1.0', 'tlsv1.1', 'tlsv1.2', 'tlsv1.3'."
            ),
        };
        Ok(version)
    }

    // Empty string accept encoding expands to the encodings supported by the current libcurl.
    handle.accept_encoding("")?;
    if let Some(ssl_version) = &http.ssl_version {
        match ssl_version {
            SslVersionConfig::Single(s) => {
                let version = to_ssl_version(s.as_str())?;
                handle.ssl_version(version)?;
            }
            SslVersionConfig::Range(SslVersionConfigRange { min, max }) => {
                let min_version = min
                    .as_ref()
                    .map_or(Ok(SslVersion::Default), |s| to_ssl_version(s))?;
                let max_version = max
                    .as_ref()
                    .map_or(Ok(SslVersion::Default), |s| to_ssl_version(s))?;
                handle.ssl_min_max_version(min_version, max_version)?;
            }
        }
    } else if cfg!(windows) {
        // This is a temporary workaround for some bugs with libcurl and
        // schannel and TLS 1.3.
        //
        // Our libcurl on Windows is usually built with schannel.
        // On Windows 11 (or Windows Server 2022), libcurl recently (late
        // 2022) gained support for TLS 1.3 with schannel, and it now defaults
        // to 1.3. Unfortunately there have been some bugs with this.
        // https://github.com/curl/curl/issues/9431 is the most recent. Once
        // that has been fixed, and some time has passed where we can be more
        // confident that the 1.3 support won't cause issues, this can be
        // removed.
        //
        // Windows 10 is unaffected. libcurl does not support TLS 1.3 on
        // Windows 10. (Windows 10 sorta had support, but it required enabling
        // an advanced option in the registry which was buggy, and libcurl
        // does runtime checks to prevent it.)
        handle.ssl_min_max_version(SslVersion::Default, SslVersion::Tlsv12)?;
    }

    if let Some(true) = http.debug {
        handle.verbose(true)?;
        tracing::debug!(target: "network", "{:#?}", curl::Version::get());
        handle.debug_function(|kind, data| {
            enum LogLevel {
                Debug,
                Trace,
            }
            use LogLevel::*;
            let (prefix, level) = match kind {
                InfoType::Text => ("*", Debug),
                InfoType::HeaderIn => ("<", Debug),
                InfoType::HeaderOut => (">", Debug),
                InfoType::DataIn => ("{", Trace),
                InfoType::DataOut => ("}", Trace),
                InfoType::SslDataIn | InfoType::SslDataOut => return,
                _ => return,
            };
            let starts_with_ignore_case = |line: &str, text: &str| -> bool {
                let line = line.as_bytes();
                let text = text.as_bytes();
                line[..line.len().min(text.len())].eq_ignore_ascii_case(text)
            };
            match str::from_utf8(data) {
                Ok(s) => {
                    for mut line in s.lines() {
                        if starts_with_ignore_case(line, "authorization:") {
                            line = "Authorization: [REDACTED]";
                        } else if starts_with_ignore_case(line, "h2h3 [authorization:") {
                            line = "h2h3 [Authorization: [REDACTED]]";
                        } else if starts_with_ignore_case(line, "set-cookie") {
                            line = "set-cookie: [REDACTED]";
                        }
                        match level {
                            Debug => debug!(target: "network", "http-debug: {prefix} {line}"),
                            Trace => trace!(target: "network", "http-debug: {prefix} {line}"),
                        }
                    }
                }
                Err(_) => {
                    let len = data.len();
                    match level {
                        Debug => {
                            debug!(target: "network", "http-debug: {prefix} ({len} bytes of data)")
                        }
                        Trace => {
                            trace!(target: "network", "http-debug: {prefix} ({len} bytes of data)")
                        }
                    }
                }
            }
        })?;
    }

    HttpTimeout::new(config)
}

#[must_use]
pub struct HttpTimeout {
    pub dur: Duration,
    pub low_speed_limit: u32,
}

impl HttpTimeout {
    pub fn new(config: &Config) -> CargoResult<HttpTimeout> {
        let http_config = config.http_config()?;
        let low_speed_limit = http_config.low_speed_limit.unwrap_or(10);
        let seconds = http_config
            .timeout
            .or_else(|| {
                config
                    .get_env("HTTP_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(30);
        Ok(HttpTimeout {
            dur: Duration::new(seconds, 0),
            low_speed_limit,
        })
    }

    pub fn configure(&self, handle: &mut Easy) -> CargoResult<()> {
        // The timeout option for libcurl by default times out the entire
        // transfer, but we probably don't want this. Instead we only set
        // timeouts for the connect phase as well as a "low speed" timeout so
        // if we don't receive many bytes in a large-ish period of time then we
        // time out.
        handle.connect_timeout(self.dur)?;
        handle.low_speed_time(self.dur)?;
        handle.low_speed_limit(self.low_speed_limit)?;
        Ok(())
    }
}
