# Index Format

The following defines the format of the index. New features are occasionally
added, which are only understood starting with the version of Cargo that
introduced them. Older versions of Cargo may not be able to use packages that
make use of new features. However, the format for older packages should not
change, so older versions of Cargo should be able to use them.

## Index Configuration
The root of the index contains a file named `config.json` which contains JSON
information used by Cargo for accessing the registry. This is an example of
what the [crates.io] config file looks like:

```javascript
{
    "dl": "https://crates.io/api/v1/crates",
    "api": "https://crates.io"
}
```

The keys are:
- `dl`: This is the URL for downloading crates listed in the index. The value
  may have the following markers which will be replaced with their
  corresponding value:

  - `{crate}`: The name of crate.
  - `{version}`: The crate version.
  - `{prefix}`: A directory prefix computed from the crate name. For example,
    a crate named `cargo` has a prefix of `ca/rg`. See below for details.
  - `{lowerprefix}`: Lowercase variant of `{prefix}`.
  - `{sha256-checksum}`: The crate's sha256 checksum.

  If none of the markers are present, then the value
  `/{crate}/{version}/download` is appended to the end.
- `api`: This is the base URL for the web API. This key is optional, but if it
  is not specified, commands such as [`cargo publish`] will not work. The web
  API is described below.
- `auth-required`: indicates whether this is a private registry that requires
  all operations to be authenticated including API requests, crate downloads
  and sparse index updates.


## Download Endpoint
The download endpoint should send the `.crate` file for the requested package.
Cargo supports https, http, and file URLs, HTTP redirects, HTTP1 and HTTP2.
The exact specifics of TLS support depend on the platform that Cargo is
running on, the version of Cargo, and how it was compiled.

If `auth-required: true` is set in `config.json`, the `Authorization` header
will be included with http(s) download requests.

## Index files
The rest of the index repository contains one file for each package, where the
filename is the name of the package in lowercase. Each version of the package
has a separate line in the file. The files are organized in a tier of
directories:

- Packages with 1 character names are placed in a directory named `1`.
- Packages with 2 character names are placed in a directory named `2`.
- Packages with 3 character names are placed in the directory
  `3/{first-character}` where `{first-character}` is the first character of
  the package name.
- All other packages are stored in directories named
  `{first-two}/{second-two}` where the top directory is the first two
  characters of the package name, and the next subdirectory is the third and
  fourth characters of the package name. For example, `cargo` would be stored
  in a file named `ca/rg/cargo`.

> Note: Although the index filenames are in lowercase, the fields that contain
> package names in `Cargo.toml` and the index JSON data are case-sensitive and
> may contain upper and lower case characters.

The directory name above is calculated based on the package name converted to
lowercase; it is represented by the marker `{lowerprefix}`.  When the original
package name is used without case conversion, the resulting directory name is
represented by the marker `{prefix}`.  For example, the package `MyCrate` would
have a `{prefix}` of `My/Cr` and a `{lowerprefix}` of `my/cr`.  In general,
using `{prefix}` is recommended over `{lowerprefix}`, but there are pros and
cons to each choice.  Using `{prefix}` on case-insensitive filesystems results
in (harmless-but-inelegant) directory aliasing.  For example, `crate` and
`CrateTwo` have `{prefix}` values of `cr/at` and `Cr/at`; these are distinct on
Unix machines but alias to the same directory on Windows.  Using directories
with normalized case avoids aliasing, but on case-sensitive filesystems it's
harder to support older versions of Cargo that lack `{prefix}`/`{lowerprefix}`.
For example, nginx rewrite rules can easily construct `{prefix}` but can't
perform case-conversion to construct `{lowerprefix}`.

## Name restrictions

Registries should consider enforcing limitations on package names added to
their index. Cargo itself allows names with any [alphanumeric], `-`, or `_`
characters. [crates.io] imposes its own limitations, including the following:

- Only allows ASCII characters.
- Only alphanumeric, `-`, and `_` characters.
- First character must be alphabetic.
- Case-insensitive collision detection.
- Prevent differences of `-` vs `_`.
- Under a specific length (max 64).
- Rejects reserved names, such as Windows special filenames like "nul".

Registries should consider incorporating similar restrictions, and consider
the security implications, such as [IDN homograph
attacks](https://en.wikipedia.org/wiki/IDN_homograph_attack) and other
concerns in [UTR36](https://www.unicode.org/reports/tr36/) and
[UTS39](https://www.unicode.org/reports/tr39/).

## Version uniqueness

Indexes *must* ensure that each version only appears once for each package.
This includes ignoring SemVer build metadata.
For example, the index must *not* contain two entries with a version `1.0.7` and `1.0.7+extra`.

## JSON schema

Each line in a package file contains a JSON object that describes a published
version of the package. The following is a pretty-printed example with comments
explaining the format of the entry.

```javascript
{
    // The name of the package.
    // This must only contain alphanumeric, `-`, or `_` characters.
    "name": "foo",
    // The version of the package this row is describing.
    // This must be a valid version number according to the Semantic
    // Versioning 2.0.0 spec at https://semver.org/.
    "vers": "0.1.0",
    // Array of direct dependencies of the package.
    "deps": [
        {
            // Name of the dependency.
            // If the dependency is renamed from the original package name,
            // this is the new name. The original package name is stored in
            // the `package` field.
            "name": "rand",
            // The SemVer requirement for this dependency.
            // This must be a valid version requirement defined at
            // https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html.
            "req": "^0.6",
            // Array of features (as strings) enabled for this dependency.
            "features": ["i128_support"],
            // Boolean of whether or not this is an optional dependency.
            "optional": false,
            // Boolean of whether or not default features are enabled.
            "default_features": true,
            // The target platform for the dependency.
            // null if not a target dependency.
            // Otherwise, a string such as "cfg(windows)".
            "target": null,
            // The dependency kind.
            // "dev", "build", or "normal".
            // Note: this is a required field, but a small number of entries
            // exist in the crates.io index with either a missing or null
            // `kind` field due to implementation bugs.
            "kind": "normal",
            // The URL of the index of the registry where this dependency is
            // from as a string. If not specified or null, it is assumed the
            // dependency is in the current registry.
            "registry": null,
            // If the dependency is renamed, this is a string of the actual
            // package name. If not specified or null, this dependency is not
            // renamed.
            "package": null,
        }
    ],
    // A SHA256 checksum of the `.crate` file.
    "cksum": "d867001db0e2b6e0496f9fac96930e2d42233ecd3ca0413e0753d4c7695d289c",
    // Set of features defined for the package.
    // Each feature maps to an array of features or dependencies it enables.
    "features": {
        "extras": ["rand/simd_support"]
    },
    // Boolean of whether or not this version has been yanked.
    "yanked": false,
    // The `links` string value from the package's manifest, or null if not
    // specified. This field is optional and defaults to null.
    "links": null,
    // An unsigned 32-bit integer value indicating the schema version of this
    // entry.
    //
    // If this not specified, it should be interpreted as the default of 1.
    //
    // Cargo (starting with version 1.51) will ignore versions it does not
    // recognize. This provides a method to safely introduce changes to index
    // entries and allow older versions of cargo to ignore newer entries it
    // doesn't understand. Versions older than 1.51 ignore this field, and
    // thus may misinterpret the meaning of the index entry.
    //
    // The current values are:
    //
    // * 1: The schema as documented here, not including newer additions.
    //      This is honored in Rust version 1.51 and newer.
    // * 2: The addition of the `features2` field.
    //      This is honored in Rust version 1.60 and newer.
    "v": 2,
    // This optional field contains features with new, extended syntax.
    // Specifically, namespaced features (`dep:`) and weak dependencies
    // (`pkg?/feat`).
    //
    // This is separated from `features` because versions older than 1.19
    // will fail to load due to not being able to parse the new syntax, even
    // with a `Cargo.lock` file.
    //
    // Cargo will merge any values listed here with the "features" field.
    //
    // If this field is included, the "v" field should be set to at least 2.
    //
    // Registries are not required to use this field for extended feature
    // syntax, they are allowed to include those in the "features" field.
    // Using this is only necessary if the registry wants to support cargo
    // versions older than 1.19, which in practice is only crates.io since
    // those older versions do not support other registries.
    "features2": {
        "serde": ["dep:serde", "chrono?/serde"]
    }
    // The minimal supported Rust version (optional)
    // This must be a valid version requirement without an operator (e.g. no `=`)
    "rust_version": "1.60"
}
```

The JSON objects should not be modified after they are added except for the
`yanked` field whose value may change at any time.

> **Note**: The index JSON format has subtle differences from the JSON format of the [Publish API] and [`cargo metadata`].
> If you are using one of those as a source to generate index entries, you are encouraged to carefully inspect the documentation differences between them.
>
> For the [Publish API], the differences are:
>
> * `deps`
>     * `name` --- When the dependency is [renamed] in `Cargo.toml`, the publish API puts the original package name in the `name` field and the aliased name in the `explicit_name_in_toml` field.
>       The index places the aliased name in the `name` field, and the original package name in the `package` field.
>     * `req` --- The Publish API field is called `version_req`.
> * `cksum` --- The publish API does not specify the checksum, it must be computed by the registry before adding to the index.
> * `features` --- Some features may be placed in the `features2` field.
>   Note: This is only a legacy requirement for [crates.io]; other registries should not need to bother with modifying the features map.
>   The `v` field indicates the presence of the `features2` field.
> * The publish API includes several other fields, such as `description` and `readme`, which don't appear in the index.
>   These are intended to make it easier for a registry to obtain the metadata about the crate to display on a website without needing to extract and parse the `.crate` file.
>   This additional information is typically added to a database on the registry server.
> * Although `rust_version` is included here, [crates.io] will ignore this field
>   and instead read it from the `Cargo.toml` contained in the `.crate` file.
>
> For [`cargo metadata`], the differences are:
>
> * `vers` --- The `cargo metadata` field is called `version`.
> * `deps`
>   * `name` --- When the dependency is [renamed] in `Cargo.toml`, `cargo metadata` puts the original package name in the `name` field and the aliased name in the `rename` field.
>     The index places the aliased name in the `name` field, and the original package name in the `package` field.
>   * `default_features` --- The `cargo metadata` field is called `uses_default_features`.
>   * `registry` --- `cargo metadata` uses a value of `null` to indicate that the dependency comes from [crates.io].
>     The index uses a value of `null` to indicate that the dependency comes from the same registry as the index.
>     When creating an index entry, a registry other than [crates.io] should translate a value of `null` to be `https://github.com/rust-lang/crates.io-index` and translate a URL that matches the current index to be `null`.
>   * `cargo metadata` includes some extra fields, such as `source` and `path`.
> * The index includes additional fields such as `yanked`, `cksum`, and `v`.

[renamed]: specifying-dependencies.md#renaming-dependencies-in-cargotoml
[Publish API]: registry-web-api.md#publish
[`cargo metadata`]: ../commands/cargo-metadata.md

## Index Protocols
Cargo supports two remote registry protocols: `git` and `sparse`. The `git` protocol
stores index files in a git repository and the `sparse` protocol fetches individual
files over HTTP.

### Git Protocol
The git protocol has no protocol prefix in the index url. For example the git index URL
for [crates.io] is `https://github.com/rust-lang/crates.io-index`.

Cargo caches the git repository on disk so that it can efficiently incrementally fetch
updates.

### Sparse Protocol
The sparse protocol uses the `sparse+` protocol prefix in the registry URL. For example,
the sparse index URL for [crates.io] is `sparse+https://index.crates.io/`.

The sparse protocol downloads each index file using an individual HTTP request. Since
this results in a large number of small HTTP requests, performance is significantly
improved with a server that supports pipelining and HTTP/2.

#### Sparse authentication
Cargo will attempt to fetch the `config.json` file before
fetching any other files. If the server responds with an HTTP 401, then Cargo will assume
that the registry requires authentication and re-attempt the request for `config.json`
with the authentication token included.

On authentication failure (or a missing authentication token) the server may include a
`www-authenticate` header with a `Cargo login_url="<URL>"` challenge to indicate where the user
can go to get a token.

Registries that require authentication must set `auth-required: true` in `config.json`.

#### Caching
Cargo caches the crate metadata files, and captures the `ETag` or `Last-Modified` 
HTTP header from the server for each entry. When refreshing crate metadata, Cargo
sends the `If-None-Match` or `If-Modified-Since` header to allow the server to respond
with HTTP 304 "Not Modified" if the local cache is valid, saving time and bandwidth.
If both `ETag` and `Last-Modified` headers are present, Cargo uses the `ETag` only.

#### Cache Invalidation
If a registry is using some kind of CDN or proxy which caches access to the index files,
then it is recommended that registries implement some form of cache invalidation when
the files are updated. If these caches are not updated, then users may not be able to
access new crates until the cache is cleared.

#### Nonexistent Crates
For crates that do not exist, the registry should respond with a 404 "Not Found", 410 "Gone"
or 451 "Unavailable For Legal Reasons" code.

#### Sparse Limitations
Since the URL of the registry is stored in the lockfile, it's not recommended to offer
a registry with both protocols. Discussion about a transition plan is ongoing in issue 
[#10964]. The [crates.io] registry is an exception, since Cargo internally substitutes
the equivalent git URL when the sparse protocol is used.

If a registry does offer both protocols, it's currently recommended to choose one protocol
as the canonical protocol and use [source replacement] for the other protocol.


[`cargo publish`]: ../commands/cargo-publish.md
[alphanumeric]: ../../std/primitive.char.html#method.is_alphanumeric
[crates.io]: https://crates.io/
[source replacement]: ../reference/source-replacement.md
[#10964]: https://github.com/rust-lang/cargo/issues/10964
