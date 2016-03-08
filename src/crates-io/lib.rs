extern crate curl;
extern crate url;
extern crate rustc_serialize;

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File};
use std::io::prelude::*;
use std::io::{self, Cursor};
use std::path::Path;
use std::result;

use curl::http;
use curl::http::handle::Method::{Put, Get, Delete};
use curl::http::handle::{Method, Request};
use rustc_serialize::json;

use url::percent_encoding::{percent_encode, QUERY_ENCODE_SET};

pub struct Registry {
    host: String,
    token: Option<String>,
    handle: http::Handle,
}

pub type Result<T> = result::Result<T, Error>;

#[derive(PartialEq, Clone, Copy)]
pub enum Auth {
    Authorized,
    Unauthorized
}

pub enum Error {
    Curl(curl::ErrCode),
    NotOkResponse(http::Response),
    NonUtf8Body,
    Api(Vec<String>),
    Unauthorized,
    TokenMissing,
    Io(io::Error),
    NotFound,
    JsonEncodeError(json::EncoderError),
    JsonDecodeError(json::DecoderError),
}

impl From<json::EncoderError> for Error {
    fn from(err: json::EncoderError) -> Error {
        Error::JsonEncodeError(err)
    }
}

impl From<json::DecoderError> for Error {
    fn from(err: json::DecoderError) -> Error {
        Error::JsonDecodeError(err)
    }
}

#[derive(RustcDecodable)]
pub struct Crate {
    pub name: String,
    pub description: Option<String>,
    pub max_version: String
}

#[derive(RustcEncodable)]
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
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub repository: Option<String>,
}

#[derive(RustcEncodable)]
pub struct NewCrateDependency {
    pub optional: bool,
    pub default_features: bool,
    pub name: String,
    pub features: Vec<String>,
    pub version_req: String,
    pub target: Option<String>,
    pub kind: String,
}

#[derive(RustcDecodable)]
pub struct User {
    pub id: u32,
    pub login: String,
    pub avatar: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
}

#[derive(RustcDecodable)] struct R { ok: bool }
#[derive(RustcDecodable)] struct ApiErrorList { errors: Vec<ApiError> }
#[derive(RustcDecodable)] struct ApiError { detail: String }
#[derive(RustcEncodable)] struct OwnersReq<'a> { users: &'a [&'a str] }
#[derive(RustcDecodable)] struct Users { users: Vec<User> }
#[derive(RustcDecodable)] struct TotalCrates { total: u32 }
#[derive(RustcDecodable)] struct Crates { crates: Vec<Crate>, meta: TotalCrates }

impl Registry {
    pub fn new(host: String, token: Option<String>) -> Registry {
        Registry::new_handle(host, token, http::Handle::new())
    }

    pub fn new_handle(host: String, token: Option<String>,
                      handle: http::Handle) -> Registry {
        Registry {
            host: host,
            token: token,
            handle: handle,
        }
    }

    pub fn add_owners(&mut self, krate: &str, owners: &[&str]) -> Result<()> {
        let body = try!(json::encode(&OwnersReq { users: owners }));
        let body = try!(self.put(format!("/crates/{}/owners", krate),
                                 body.as_bytes()));
        assert!(try!(json::decode::<R>(&body)).ok);
        Ok(())
    }

    pub fn remove_owners(&mut self, krate: &str, owners: &[&str]) -> Result<()> {
        let body = try!(json::encode(&OwnersReq { users: owners }));
        let body = try!(self.delete(format!("/crates/{}/owners", krate),
                                    Some(body.as_bytes())));
        assert!(try!(json::decode::<R>(&body)).ok);
        Ok(())
    }

    pub fn list_owners(&mut self, krate: &str) -> Result<Vec<User>> {
        let body = try!(self.get(format!("/crates/{}/owners", krate)));
        Ok(try!(json::decode::<Users>(&body)).users)
    }

    pub fn publish(&mut self, krate: &NewCrate, tarball: &Path) -> Result<()> {
        let json = try!(json::encode(krate));
        // Prepare the body. The format of the upload request is:
        //
        //      <le u32 of json>
        //      <json request> (metadata for the package)
        //      <le u32 of tarball>
        //      <source tarball>
        let stat = try!(fs::metadata(tarball).map_err(Error::Io));
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
        let tarball = try!(File::open(tarball).map_err(Error::Io));
        let size = stat.len() as usize + header.len();
        let mut body = Cursor::new(header).chain(tarball);

        let url = format!("{}/api/v1/crates/new", self.host);

        let token = match self.token.as_ref() {
            Some(s) => s,
            None => return Err(Error::TokenMissing),
        };
        let request = self.handle.put(url, &mut body)
            .content_length(size)
            .header("Accept", "application/json")
            .header("Authorization", &token);
        let response = handle(request.exec());
        let _body = try!(response);
        Ok(())
    }

    pub fn search(&mut self, query: &str, limit: u8) -> Result<(Vec<Crate>, u32)> {
        let formated_query = percent_encode(query.as_bytes(), QUERY_ENCODE_SET);
        let body = try!(self.req(
            format!("/crates?q={}&per_page={}", formated_query, limit),
            None, Get, Auth::Unauthorized
        ));

        let crates = try!(json::decode::<Crates>(&body));
        Ok((crates.crates, crates.meta.total))
    }

    pub fn yank(&mut self, krate: &str, version: &str) -> Result<()> {
        let body = try!(self.delete(format!("/crates/{}/{}/yank", krate, version),
                                    None));
        assert!(try!(json::decode::<R>(&body)).ok);
        Ok(())
    }

    pub fn unyank(&mut self, krate: &str, version: &str) -> Result<()> {
        let body = try!(self.put(format!("/crates/{}/{}/unyank", krate, version),
                                 &[]));
        assert!(try!(json::decode::<R>(&body)).ok);
        Ok(())
    }

    fn put(&mut self, path: String, b: &[u8]) -> Result<String> {
        self.req(path, Some(b), Put, Auth::Authorized)
    }

    fn get(&mut self, path: String) -> Result<String> {
        self.req(path, None, Get, Auth::Authorized)
    }

    fn delete(&mut self, path: String, b: Option<&[u8]>) -> Result<String> {
        self.req(path, b, Delete, Auth::Authorized)
    }

    fn req(&mut self, path: String, body: Option<&[u8]>,
           method: Method, authorized: Auth) -> Result<String> {
        let mut req = Request::new(&mut self.handle, method)
                              .uri(format!("{}/api/v1{}", self.host, path))
                              .header("Accept", "application/json")
                              .content_type("application/json");

        if authorized == Auth::Authorized {
            let token = match self.token.as_ref() {
                Some(s) => s,
                None => return Err(Error::TokenMissing),
            };
            req = req.header("Authorization", &token);
        }
        match body {
            Some(b) => req = req.body(b),
            None => {}
        }
        handle(req.exec())
    }
}

fn handle(response: result::Result<http::Response, curl::ErrCode>)
          -> Result<String> {
    let response = try!(response.map_err(Error::Curl));
    match response.get_code() {
        0 => {} // file upload url sometimes
        200 => {}
        403 => return Err(Error::Unauthorized),
        404 => return Err(Error::NotFound),
        _ => return Err(Error::NotOkResponse(response))
    }

    let body = match String::from_utf8(response.move_body()) {
        Ok(body) => body,
        Err(..) => return Err(Error::NonUtf8Body),
    };
    match json::decode::<ApiErrorList>(&body) {
        Ok(errors) => {
            return Err(Error::Api(errors.errors.into_iter().map(|s| s.detail)
                                        .collect()))
        }
        Err(..) => {}
    }
    Ok(body)
}

impl fmt::Display for Error {
    #[allow(deprecated)] // connect => join in 1.3
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::NonUtf8Body => write!(f, "response body was not utf-8"),
            Error::Curl(ref err) => write!(f, "http error: {}", err),
            Error::NotOkResponse(ref resp) => {
                write!(f, "failed to get a 200 OK response: {}", resp)
            }
            Error::Api(ref errs) => {
                write!(f, "api errors: {}", errs.connect(", "))
            }
            Error::Unauthorized => write!(f, "unauthorized API access"),
            Error::TokenMissing => write!(f, "no upload token found, please run `cargo login`"),
            Error::Io(ref e) => write!(f, "io error: {}", e),
            Error::NotFound => write!(f, "cannot find crate"),
            Error::JsonEncodeError(ref e) => write!(f, "json encode error: {}", e),
            Error::JsonDecodeError(ref e) => write!(f, "json decode error: {}", e),
        }
    }
}
