#![allow(unknown_lints)]

extern crate curl;
extern crate url;
#[macro_use]
extern crate error_chain;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, Cursor};

use curl::easy::{Easy, List};

use url::percent_encoding::{percent_encode, QUERY_ENCODE_SET};

error_chain! {
        foreign_links {
            Curl(curl::Error);
            Io(io::Error);
            Json(serde_json::Error);
        }

        errors {
            NotOkResponse(code: u32, headers: Vec<String>, body: Vec<u8>){
                description("failed to get a 200 OK response")
                display("failed to get a 200 OK response, got {}
headers:
    {}
body:
{}", code, headers.join("\n    ", ), String::from_utf8_lossy(body))
            }
            NonUtf8Body {
                description("response body was not utf-8")
                display("response body was not utf-8")
            }
            Api(errs: Vec<String>) {
                display("api errors: {}", errs.join(", "))
            }
            Unauthorized {
                display("unauthorized API access")
            }
            TokenMissing{
                display("no upload token found, please run `cargo login`")
            }
            NotFound {
                display("cannot find crate")
            }
        }
    }

pub struct Registry {
    host: String,
    token: Option<String>,
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
    pub features: HashMap<String, Vec<String>>,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub documentation: Option<String>,
    pub homepage: Option<String>,
    pub readme: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub repository: Option<String>,
    pub badges: HashMap<String, HashMap<String, String>>,
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
}

#[derive(Deserialize)] struct R { ok: bool }
#[derive(Deserialize)] struct OwnerResponse { ok: String }
#[derive(Deserialize)] struct ApiErrorList { errors: Vec<ApiError> }
#[derive(Deserialize)] struct ApiError { detail: String }
#[derive(Serialize)] struct OwnersReq<'a> { users: &'a [&'a str] }
#[derive(Deserialize)] struct Users { users: Vec<User> }
#[derive(Deserialize)] struct TotalCrates { total: u32 }
#[derive(Deserialize)] struct Crates { crates: Vec<Crate>, meta: TotalCrates }
impl Registry {
    pub fn new(host: String, token: Option<String>) -> Registry {
        Registry::new_handle(host, token, Easy::new())
    }

    pub fn new_handle(host: String,
                      token: Option<String>,
                      handle: Easy) -> Registry {
        Registry {
            host: host,
            token: token,
            handle: handle,
        }
    }

    pub fn add_owners(&mut self, krate: &str, owners: &[&str]) -> Result<String> {
        let body = serde_json::to_string(&OwnersReq { users: owners })?;
        let body = self.put(format!("/crates/{}/owners", krate),
                                 body.as_bytes())?;
        Ok(serde_json::from_str::<OwnerResponse>(&body)?.ok)
    }

    pub fn remove_owners(&mut self, krate: &str, owners: &[&str]) -> Result<()> {
        let body = serde_json::to_string(&OwnersReq { users: owners })?;
        let body = self.delete(format!("/crates/{}/owners", krate),
                                    Some(body.as_bytes()))?;
        serde_json::from_str::<OwnerResponse>(&body)?;
        Ok(())
    }

    pub fn list_owners(&mut self, krate: &str) -> Result<Vec<User>> {
        let body = self.get(format!("/crates/{}/owners", krate))?;
        Ok(serde_json::from_str::<Users>(&body)?.users)
    }

    pub fn publish(&mut self, krate: &NewCrate, tarball: &File)
                   -> Result<Warnings> {
        let json = serde_json::to_string(krate)?;
        // Prepare the body. The format of the upload request is:
        //
        //      <le u32 of json>
        //      <json request> (metadata for the package)
        //      <le u32 of tarball>
        //      <source tarball>
        let stat = tarball.metadata()?;
        let header = {
            let mut w = Vec::new();
            w.extend([
                (json.len() >>  0) as u8,
                (json.len() >>  8) as u8,
                (json.len() >> 16) as u8,
                (json.len() >> 24) as u8,
            ].iter().map(|x| *x));
            w.extend(json.as_bytes().iter().map(|x| *x));
            w.extend([
                (stat.len() >>  0) as u8,
                (stat.len() >>  8) as u8,
                (stat.len() >> 16) as u8,
                (stat.len() >> 24) as u8,
            ].iter().map(|x| *x));
            w
        };
        let size = stat.len() as usize + header.len();
        let mut body = Cursor::new(header).chain(tarball);

        let url = format!("{}/api/v1/crates/new", self.host);

        let token = match self.token.as_ref() {
            Some(s) => s,
            None => return Err(Error::from_kind(ErrorKind::TokenMissing)),
        };
        self.handle.put(true)?;
        self.handle.url(&url)?;
        self.handle.in_filesize(size as u64)?;
        let mut headers = List::new();
        headers.append("Accept: application/json")?;
        headers.append(&format!("Authorization: {}", token))?;
        self.handle.http_headers(headers)?;

        let body = handle(&mut self.handle, &mut |buf| body.read(buf).unwrap_or(0))?;

        let response = if body.len() > 0 {
            body.parse::<serde_json::Value>()?
        } else {
            "{}".parse()?
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

        Ok(Warnings {
            invalid_categories: invalid_categories,
            invalid_badges: invalid_badges,
        })
    }

    pub fn search(&mut self, query: &str, limit: u8) -> Result<(Vec<Crate>, u32)> {
        let formated_query = percent_encode(query.as_bytes(), QUERY_ENCODE_SET);
        let body = self.req(
            format!("/crates?q={}&per_page={}", formated_query, limit),
            None, Auth::Unauthorized
        )?;

        let crates = serde_json::from_str::<Crates>(&body)?;
        Ok((crates.crates, crates.meta.total))
    }

    pub fn yank(&mut self, krate: &str, version: &str) -> Result<()> {
        let body = self.delete(format!("/crates/{}/{}/yank", krate, version),
                                    None)?;
        assert!(serde_json::from_str::<R>(&body)?.ok);
        Ok(())
    }

    pub fn unyank(&mut self, krate: &str, version: &str) -> Result<()> {
        let body = self.put(format!("/crates/{}/{}/unyank", krate, version),
                                 &[])?;
        assert!(serde_json::from_str::<R>(&body)?.ok);
        Ok(())
    }

    fn put(&mut self, path: String, b: &[u8]) -> Result<String> {
        self.handle.put(true)?;
        self.req(path, Some(b), Auth::Authorized)
    }

    fn get(&mut self, path: String) -> Result<String> {
        self.handle.get(true)?;
        self.req(path, None, Auth::Authorized)
    }

    fn delete(&mut self, path: String, b: Option<&[u8]>) -> Result<String> {
        self.handle.custom_request("DELETE")?;
        self.req(path, b, Auth::Authorized)
    }

    fn req(&mut self,
           path: String,
           body: Option<&[u8]>,
           authorized: Auth) -> Result<String> {
        self.handle.url(&format!("{}/api/v1{}", self.host, path))?;
        let mut headers = List::new();
        headers.append("Accept: application/json")?;
        headers.append("Content-Type: application/json")?;

        if authorized == Auth::Authorized {
            let token = match self.token.as_ref() {
                Some(s) => s,
                None => return Err(Error::from_kind(ErrorKind::TokenMissing)),
            };
            headers.append(&format!("Authorization: {}", token))?;
        }
        self.handle.http_headers(headers)?;
        match body {
            Some(mut body) => {
                self.handle.upload(true)?;
                self.handle.in_filesize(body.len() as u64)?;
                handle(&mut self.handle, &mut |buf| body.read(buf).unwrap_or(0))
            }
            None => handle(&mut self.handle, &mut |_| 0),
        }
    }
}

fn handle(handle: &mut Easy,
          read: &mut FnMut(&mut [u8]) -> usize) -> Result<String> {
    let mut headers = Vec::new();
    let mut body = Vec::new();
    {
        let mut handle = handle.transfer();
        handle.read_function(|buf| Ok(read(buf)))?;
        handle.write_function(|data| {
            body.extend_from_slice(data);
            Ok(data.len())
        })?;
        handle.header_function(|data| {
            headers.push(String::from_utf8_lossy(data).into_owned());
            true
        })?;
        handle.perform()?;
    }

    match handle.response_code()? {
        0 => {} // file upload url sometimes
        200 => {}
        403 => return Err(Error::from_kind(ErrorKind::Unauthorized)),
        404 => return Err(Error::from_kind(ErrorKind::NotFound)),
        code => return Err(Error::from_kind(ErrorKind::NotOkResponse(code, headers, body))),
    }

    let body = match String::from_utf8(body) {
        Ok(body) => body,
        Err(..) => return Err(Error::from_kind(ErrorKind::NonUtf8Body)),
    };
    match serde_json::from_str::<ApiErrorList>(&body) {
        Ok(errors) => {
            return Err(Error::from_kind(ErrorKind::Api(errors
                                                           .errors
                                                           .into_iter()
                                                           .map(|s| s.detail)
                                                           .collect())))
        }
        Err(..) => {}
    }
    Ok(body)
}
