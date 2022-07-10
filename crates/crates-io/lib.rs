#![allow(clippy::all)]

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::io::{Cursor, SeekFrom};
use std::time::Instant;

use anyhow::{bail, format_err, Context, Result};
use curl::easy::{Easy, List};
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use url::Url;

pub struct Registry {
    /// The base URL for issuing API requests.
    host: String,
    /// Optional authorization token.
    /// If None, commands requiring authorization will fail.
    token: Option<String>,
    /// Curl handle for issuing requests.
    handle: Easy,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Auth {
    Authorized,
    Unauthorized,
}

#[derive(Deserialize)]
pub struct Crate {
    pub name: String,
    pub description: Option<String>,
    pub max_version: String,
}

#[derive(Serialize)]
pub struct NewCrate {
    pub name: String,
    pub vers: String,
    pub deps: Vec<NewCrateDependency>,
    pub features: BTreeMap<String, Vec<String>>,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub documentation: Option<String>,
    pub homepage: Option<String>,
    pub readme: Option<String>,
    pub readme_file: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub repository: Option<String>,
    pub badges: BTreeMap<String, BTreeMap<String, String>>,
    pub links: Option<String>,
}

#[derive(Serialize)]
pub struct NewCrateDependency {
    pub optional: bool,
    pub default_features: bool,
    pub name: String,
    pub features: Vec<String>,
    pub version_req: String,
    pub target: Option<String>,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explicit_name_in_toml: Option<String>,
}

#[derive(Deserialize)]
pub struct User {
    pub id: u32,
    pub login: String,
    pub avatar: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
}

pub struct Warnings {
    pub invalid_categories: Vec<String>,
    pub invalid_badges: Vec<String>,
    pub other: Vec<String>,
}

#[derive(Deserialize)]
struct R {
    ok: bool,
}
#[derive(Deserialize)]
struct OwnerResponse {
    ok: bool,
    msg: String,
}
#[derive(Deserialize)]
struct ApiErrorList {
    errors: Vec<ApiError>,
}
#[derive(Deserialize)]
struct ApiError {
    detail: String,
}
#[derive(Serialize)]
struct OwnersReq<'a> {
    users: &'a [&'a str],
}
#[derive(Deserialize)]
struct Users {
    users: Vec<User>,
}
#[derive(Deserialize)]
struct TotalCrates {
    total: u32,
}
#[derive(Deserialize)]
struct Crates {
    crates: Vec<Crate>,
    meta: TotalCrates,
}

#[derive(Debug)]
pub enum ResponseError {
    Curl(curl::Error),
    Api {
        code: u32,
        errors: Vec<String>,
    },
    Code {
        code: u32,
        headers: Vec<String>,
        body: String,
    },
    Other(anyhow::Error),
}

impl std::error::Error for ResponseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ResponseError::Curl(..) => None,
            ResponseError::Api { .. } => None,
            ResponseError::Code { .. } => None,
            ResponseError::Other(e) => Some(e.as_ref()),
        }
    }
}

impl fmt::Display for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResponseError::Curl(e) => write!(f, "{}", e),
            ResponseError::Api { code, errors } => {
                f.write_str("the remote server responded with an error")?;
                if *code != 200 {
                    write!(f, " (status {} {})", code, reason(*code))?;
                };
                write!(f, ": {}", errors.join(", "))
            }
            ResponseError::Code {
                code,
                headers,
                body,
            } => write!(
                f,
                "failed to get a 200 OK response, got {}\n\
                 headers:\n\
                 \t{}\n\
                 body:\n\
                 {}",
                code,
                headers.join("\n\t"),
                body
            ),
            ResponseError::Other(..) => write!(f, "invalid response from server"),
        }
    }
}

impl From<curl::Error> for ResponseError {
    fn from(error: curl::Error) -> Self {
        ResponseError::Curl(error)
    }
}

impl Registry {
    /// Creates a new `Registry`.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use curl::easy::Easy;
    /// use crates_io::Registry;
    ///
    /// let mut handle = Easy::new();
    /// // If connecting to crates.io, a user-agent is required.
    /// handle.useragent("my_crawler (example.com/info)");
    /// let mut reg = Registry::new_handle(String::from("https://crates.io"), None, handle);
    /// ```
    pub fn new_handle(host: String, token: Option<String>, handle: Easy) -> Registry {
        Registry {
            host,
            token,
            handle,
        }
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn host_is_crates_io(&self) -> bool {
        is_url_crates_io(&self.host)
    }

    pub fn add_owners(&mut self, krate: &str, owners: &[&str]) -> Result<String> {
        let body = serde_json::to_string(&OwnersReq { users: owners })?;
        let body = self.put(&format!("/crates/{}/owners", krate), body.as_bytes())?;
        assert!(serde_json::from_str::<OwnerResponse>(&body)?.ok);
        Ok(serde_json::from_str::<OwnerResponse>(&body)?.msg)
    }

    pub fn remove_owners(&mut self, krate: &str, owners: &[&str]) -> Result<()> {
        let body = serde_json::to_string(&OwnersReq { users: owners })?;
        let body = self.delete(&format!("/crates/{}/owners", krate), Some(body.as_bytes()))?;
        assert!(serde_json::from_str::<OwnerResponse>(&body)?.ok);
        Ok(())
    }

    pub fn list_owners(&mut self, krate: &str) -> Result<Vec<User>> {
        let body = self.get(&format!("/crates/{}/owners", krate))?;
        Ok(serde_json::from_str::<Users>(&body)?.users)
    }

    pub fn publish(&mut self, krate: &NewCrate, mut tarball: &File) -> Result<Warnings> {
        let json = serde_json::to_string(krate)?;
        // Prepare the body. The format of the upload request is:
        //
        //      <le u32 of json>
        //      <json request> (metadata for the package)
        //      <le u32 of tarball>
        //      <source tarball>

        // NOTE: This can be replaced with `stream_len` if it is ever stabilized.
        //
        // This checks the length using seeking instead of metadata, because
        // on some filesystems, getting the metadata will fail because
        // the file was renamed in ops::package.
        let tarball_len = tarball
            .seek(SeekFrom::End(0))
            .with_context(|| "failed to seek tarball")?;
        tarball
            .seek(SeekFrom::Start(0))
            .with_context(|| "failed to seek tarball")?;
        let header = {
            let mut w = Vec::new();
            w.extend(&(json.len() as u32).to_le_bytes());
            w.extend(json.as_bytes().iter().cloned());
            w.extend(&(tarball_len as u32).to_le_bytes());
            w
        };
        let size = tarball_len as usize + header.len();
        let mut body = Cursor::new(header).chain(tarball);

        let url = format!("{}/api/v1/crates/new", self.host);

        let token = match self.token.as_ref() {
            Some(s) => s,
            None => bail!("no upload token found, please run `cargo login`"),
        };
        self.handle.put(true)?;
        self.handle.url(&url)?;
        self.handle.in_filesize(size as u64)?;
        let mut headers = List::new();
        headers.append("Accept: application/json")?;
        headers.append(&format!("Authorization: {}", token))?;
        self.handle.http_headers(headers)?;

        let started = Instant::now();
        let body = self
            .handle(&mut |buf| body.read(buf).unwrap_or(0))
            .map_err(|e| match e {
                ResponseError::Code { code, .. }
                    if code == 503
                        && started.elapsed().as_secs() >= 29
                        && self.host_is_crates_io() =>
                {
                    format_err!(
                        "Request timed out after 30 seconds. If you're trying to \
                         upload a crate it may be too large. If the crate is under \
                         10MB in size, you can email help@crates.io for assistance.\n\
                         Total size was {}.",
                        tarball_len
                    )
                }
                _ => e.into(),
            })?;

        let response = if body.is_empty() {
            "{}".parse()?
        } else {
            body.parse::<serde_json::Value>()?
        };

        let invalid_categories: Vec<String> = response
            .get("warnings")
            .and_then(|j| j.get("invalid_categories"))
            .and_then(|j| j.as_array())
            .map(|x| x.iter().flat_map(|j| j.as_str()).map(Into::into).collect())
            .unwrap_or_else(Vec::new);

        let invalid_badges: Vec<String> = response
            .get("warnings")
            .and_then(|j| j.get("invalid_badges"))
            .and_then(|j| j.as_array())
            .map(|x| x.iter().flat_map(|j| j.as_str()).map(Into::into).collect())
            .unwrap_or_else(Vec::new);

        let other: Vec<String> = response
            .get("warnings")
            .and_then(|j| j.get("other"))
            .and_then(|j| j.as_array())
            .map(|x| x.iter().flat_map(|j| j.as_str()).map(Into::into).collect())
            .unwrap_or_else(Vec::new);

        Ok(Warnings {
            invalid_categories,
            invalid_badges,
            other,
        })
    }

    pub fn search(&mut self, query: &str, limit: u32) -> Result<(Vec<Crate>, u32)> {
        let formatted_query = percent_encode(query.as_bytes(), NON_ALPHANUMERIC);
        let body = self.req(
            &format!("/crates?q={}&per_page={}", formatted_query, limit),
            None,
            Auth::Unauthorized,
        )?;

        let crates = serde_json::from_str::<Crates>(&body)?;
        Ok((crates.crates, crates.meta.total))
    }

    pub fn yank(&mut self, krate: &str, version: &str) -> Result<()> {
        let body = self.delete(&format!("/crates/{}/{}/yank", krate, version), None)?;
        assert!(serde_json::from_str::<R>(&body)?.ok);
        Ok(())
    }

    pub fn unyank(&mut self, krate: &str, version: &str) -> Result<()> {
        let body = self.put(&format!("/crates/{}/{}/unyank", krate, version), &[])?;
        assert!(serde_json::from_str::<R>(&body)?.ok);
        Ok(())
    }

    fn put(&mut self, path: &str, b: &[u8]) -> Result<String> {
        self.handle.put(true)?;
        self.req(path, Some(b), Auth::Authorized)
    }

    fn get(&mut self, path: &str) -> Result<String> {
        self.handle.get(true)?;
        self.req(path, None, Auth::Authorized)
    }

    fn delete(&mut self, path: &str, b: Option<&[u8]>) -> Result<String> {
        self.handle.custom_request("DELETE")?;
        self.req(path, b, Auth::Authorized)
    }

    fn req(&mut self, path: &str, body: Option<&[u8]>, authorized: Auth) -> Result<String> {
        self.handle.url(&format!("{}/api/v1{}", self.host, path))?;
        let mut headers = List::new();
        headers.append("Accept: application/json")?;
        headers.append("Content-Type: application/json")?;

        if authorized == Auth::Authorized {
            let token = match self.token.as_ref() {
                Some(s) => s,
                None => bail!("no upload token found, please run `cargo login`"),
            };
            headers.append(&format!("Authorization: {}", token))?;
        }
        self.handle.http_headers(headers)?;
        match body {
            Some(mut body) => {
                self.handle.upload(true)?;
                self.handle.in_filesize(body.len() as u64)?;
                self.handle(&mut |buf| body.read(buf).unwrap_or(0))
                    .map_err(|e| e.into())
            }
            None => self.handle(&mut |_| 0).map_err(|e| e.into()),
        }
    }

    fn handle(
        &mut self,
        read: &mut dyn FnMut(&mut [u8]) -> usize,
    ) -> std::result::Result<String, ResponseError> {
        let mut headers = Vec::new();
        let mut body = Vec::new();
        {
            let mut handle = self.handle.transfer();
            handle.read_function(|buf| Ok(read(buf)))?;
            handle.write_function(|data| {
                body.extend_from_slice(data);
                Ok(data.len())
            })?;
            handle.header_function(|data| {
                // Headers contain trailing \r\n, trim them to make it easier
                // to work with.
                let s = String::from_utf8_lossy(data).trim().to_string();
                headers.push(s);
                true
            })?;
            handle.perform()?;
        }

        let body = match String::from_utf8(body) {
            Ok(body) => body,
            Err(..) => {
                return Err(ResponseError::Other(format_err!(
                    "response body was not valid utf-8"
                )))
            }
        };
        let errors = serde_json::from_str::<ApiErrorList>(&body)
            .ok()
            .map(|s| s.errors.into_iter().map(|s| s.detail).collect::<Vec<_>>());

        match (self.handle.response_code()?, errors) {
            (0, None) | (200, None) => Ok(body),
            (code, Some(errors)) => Err(ResponseError::Api { code, errors }),
            (code, None) => Err(ResponseError::Code {
                code,
                headers,
                body,
            }),
        }
    }
}

fn reason(code: u32) -> &'static str {
    // Taken from https://developer.mozilla.org/en-US/docs/Web/HTTP/Status
    match code {
        100 => "Continue",
        101 => "Switching Protocol",
        103 => "Early Hints",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        300 => "Multiple Choice",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Payload Too Large",
        414 => "URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Request Range Not Satisfiable",
        417 => "Expectation Failed",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "<unknown>",
    }
}

/// Returns `true` if the host of the given URL is "crates.io".
pub fn is_url_crates_io(url: &str) -> bool {
    Url::parse(url)
        .map(|u| u.host_str() == Some("crates.io"))
        .unwrap_or(false)
}
