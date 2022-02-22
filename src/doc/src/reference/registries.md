## Registries

Cargo installs crates and fetches dependencies from a "registry". The default
registry is [crates.io]. A registry contains an "index" which contains a
searchable list of available crates. A registry may also provide a web API to
support publishing new crates directly from Cargo.

> Note: If you are interested in mirroring or vendoring an existing registry,
> take a look at [Source Replacement].

### Using an Alternate Registry

To use a registry other than [crates.io], the name and index URL of the
registry must be added to a [`.cargo/config.toml` file][config]. The `registries`
table has a key for each registry, for example:

```toml
[registries]
my-registry = { index = "https://my-intranet:8080/git/index" }
```

The `index` key should be a URL to a git repository with the registry's index.
A crate can then depend on a crate from another registry by specifying the
`registry` key and a value of the registry's name in that dependency's entry
in `Cargo.toml`:

```toml
# Sample Cargo.toml
[package]
name = "my-project"
version = "0.1.0"

[dependencies]
other-crate = { version = "1.0", registry = "my-registry" }
```

As with most config values, the index may be specified with an environment
variable instead of a config file. For example, setting the following
environment variable will accomplish the same thing as defining a config file:

```ignore
CARGO_REGISTRIES_MY_REGISTRY_INDEX=https://my-intranet:8080/git/index
```

> Note: [crates.io] does not accept packages that depend on crates from other
> registries.

### Publishing to an Alternate Registry

If the registry supports web API access, then packages can be published
directly to the registry from Cargo. Several of Cargo's commands such as
[`cargo publish`] take a `--registry` command-line flag to indicate which
registry to use. For example, to publish the package in the current directory:

1. `cargo login --registry=my-registry`

    This only needs to be done once. You must enter the secret API token
    retrieved from the registry's website. Alternatively the token may be
    passed directly to the `publish` command with the `--token` command-line
    flag or an environment variable with the name of the registry such as
    `CARGO_REGISTRIES_MY_REGISTRY_TOKEN`.

2. `cargo publish --registry=my-registry`

Instead of always passing the `--registry` command-line option, the default
registry may be set in [`.cargo/config.toml`][config] with the `registry.default`
key.

Setting the `package.publish` key in the `Cargo.toml` manifest restricts which
registries the package is allowed to be published to. This is useful to
prevent accidentally publishing a closed-source package to [crates.io]. The
value may be a list of registry names, for example:

```toml
[package]
# ...
publish = ["my-registry"]
```

The `publish` value may also be `false` to restrict all publishing, which is
the same as an empty list.

The authentication information saved by [`cargo login`] is stored in the
`credentials.toml` file in the Cargo home directory (default `$HOME/.cargo`). It
has a separate table for each registry, for example:

```toml
[registries.my-registry]
token = "854DvwSlUwEHtIo3kWy6x7UCPKHfzCmy"
```

### Running a Registry

A minimal registry can be implemented by having a git repository that contains
an index, and a server that contains the compressed `.crate` files created by
[`cargo package`]. Users won't be able to use Cargo to publish to it, but this
may be sufficient for closed environments.

A full-featured registry that supports publishing will additionally need to
have a web API service that conforms to the API used by Cargo. The web API is
documented below.

Commercial and community projects are available for building and running a
registry. See <https://github.com/rust-lang/cargo/wiki/Third-party-registries>
for a list of what is available.

### Index Format

The following defines the format of the index. New features are occasionally
added, which are only understood starting with the version of Cargo that
introduced them. Older versions of Cargo may not be able to use packages that
make use of new features. However, the format for older packages should not
change, so older versions of Cargo should be able to use them.

The index is stored in a git repository so that Cargo can efficiently fetch
incremental updates to the index. In the root of the repository is a file
named `config.json` which contains JSON information used by Cargo for
accessing the registry. This is an example of what the [crates.io] config file
looks like:

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

The download endpoint should send the `.crate` file for the requested package.
Cargo supports https, http, and file URLs, HTTP redirects, HTTP1 and HTTP2.
The exact specifics of TLS support depend on the platform that Cargo is
running on, the version of Cargo, and how it was compiled.

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
}
```

The JSON objects should not be modified after they are added except for the
`yanked` field whose value may change at any time.

### Web API

A registry may host a web API at the location defined in `config.json` to
support any of the actions listed below.

Cargo includes the `Authorization` header for requests that require
authentication. The header value is the API token. The server should respond
with a 403 response code if the token is not valid. Users are expected to
visit the registry's website to obtain a token, and Cargo can store the token
using the [`cargo login`] command, or by passing the token on the
command-line.

Responses use a 200 response code for both success and errors. Cargo looks at
the JSON response to determine if there was success or failure. Failure
responses have a JSON object with the following structure:

```javascript
{
    // Array of errors to display to the user.
    "errors": [
        {
            // The error message as a string.
            "detail": "error message text"
        }
    ]
}
```

Servers may also respond with a 404 response code to indicate the requested
resource is not found (for example, an unknown crate name). However, using a
200 response with an `errors` object allows a registry to provide a more
detailed error message if desired.

For backwards compatibility, servers should ignore any unexpected query
parameters or JSON fields. If a JSON field is missing, it should be assumed to
be null. The endpoints are versioned with the `v1` component of the path, and
Cargo is responsible for handling backwards compatibility fallbacks should any
be required in the future.

Cargo sets the following headers for all requests:

- `Content-Type`: `application/json`
- `Accept`: `application/json`
- `User-Agent`: The Cargo version such as `cargo 1.32.0 (8610973aa
  2019-01-02)`. This may be modified by the user in a configuration value.
  Added in 1.29.

#### Publish

- Endpoint: `/api/v1/crates/new`
- Method: PUT
- Authorization: Included

The publish endpoint is used to publish a new version of a crate. The server
should validate the crate, make it available for download, and add it to the
index.

The body of the data sent by Cargo is:

- 32-bit unsigned little-endian integer of the length of JSON data.
- Metadata of the package as a JSON object.
- 32-bit unsigned little-endian integer of the length of the `.crate` file.
- The `.crate` file.

The following is a commented example of the JSON object. Some notes of some
restrictions imposed by [crates.io] are included only to illustrate some
suggestions on types of validation that may be done, and should not be
considered as an exhaustive list of restrictions [crates.io] imposes.

```javascript
{
    // The name of the package.
    "name": "foo",
    // The version of the package being published.
    "vers": "0.1.0",
    // Array of direct dependencies of the package.
    "deps": [
        {
            // Name of the dependency.
            // If the dependency is renamed from the original package name,
            // this is the original name. The new package name is stored in
            // the `explicit_name_in_toml` field.
            "name": "rand",
            // The semver requirement for this dependency.
            "version_req": "^0.6",
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
            "kind": "normal",
            // The URL of the index of the registry where this dependency is
            // from as a string. If not specified or null, it is assumed the
            // dependency is in the current registry.
            "registry": null,
            // If the dependency is renamed, this is a string of the new
            // package name. If not specified or null, this dependency is not
            // renamed.
            "explicit_name_in_toml": null,
        }
    ],
    // Set of features defined for the package.
    // Each feature maps to an array of features or dependencies it enables.
    // Cargo does not impose limitations on feature names, but crates.io
    // requires alphanumeric ASCII, `_` or `-` characters.
    "features": {
        "extras": ["rand/simd_support"]
    },
    // List of strings of the authors.
    // May be empty.
    "authors": ["Alice <a@example.com>"],
    // Description field from the manifest.
    // May be null. crates.io requires at least some content.
    "description": null,
    // String of the URL to the website for this package's documentation.
    // May be null.
    "documentation": null,
    // String of the URL to the website for this package's home page.
    // May be null.
    "homepage": null,
    // String of the content of the README file.
    // May be null.
    "readme": null,
    // String of a relative path to a README file in the crate.
    // May be null.
    "readme_file": null,
    // Array of strings of keywords for the package.
    "keywords": [],
    // Array of strings of categories for the package.
    "categories": [],
    // String of the license for the package.
    // May be null. crates.io requires either `license` or `license_file` to be set.
    "license": null,
    // String of a relative path to a license file in the crate.
    // May be null.
    "license_file": null,
    // String of the URL to the website for the source repository of this package.
    // May be null.
    "repository": null,
    // Optional object of "status" badges. Each value is an object of
    // arbitrary string to string mappings.
    // crates.io has special interpretation of the format of the badges.
    "badges": {
        "travis-ci": {
            "branch": "master",
            "repository": "rust-lang/cargo"
        }
    },
    // The `links` string value from the package's manifest, or null if not
    // specified. This field is optional and defaults to null.
    "links": null
}
```

A successful response includes the JSON object:

```javascript
{
    // Optional object of warnings to display to the user.
    "warnings": {
        // Array of strings of categories that are invalid and ignored.
        "invalid_categories": [],
        // Array of strings of badge names that are invalid and ignored.
        "invalid_badges": [],
        // Array of strings of arbitrary warnings to display to the user.
        "other": []
    }
}
```

#### Yank

- Endpoint: `/api/v1/crates/{crate_name}/{version}/yank`
- Method: DELETE
- Authorization: Included

The yank endpoint will set the `yank` field of the given version of a crate to
`true` in the index.

A successful response includes the JSON object:

```javascript
{
    // Indicates the delete succeeded, always true.
    "ok": true,
}
```

#### Unyank

- Endpoint: `/api/v1/crates/{crate_name}/{version}/unyank`
- Method: PUT
- Authorization: Included

The unyank endpoint will set the `yank` field of the given version of a crate
to `false` in the index.

A successful response includes the JSON object:

```javascript
{
    // Indicates the delete succeeded, always true.
    "ok": true,
}
```

#### Owners

Cargo does not have an inherent notion of users and owners, but it does
provide the `owner` command to assist managing who has authorization to
control a crate. It is up to the registry to decide exactly how users and
owners are handled. See the [publishing documentation] for a description of
how [crates.io] handles owners via GitHub users and teams.

##### Owners: List

- Endpoint: `/api/v1/crates/{crate_name}/owners`
- Method: GET
- Authorization: Included

The owners endpoint returns a list of owners of the crate.

A successful response includes the JSON object:

```javascript
{
    // Array of owners of the crate.
    "users": [
        {
            // Unique unsigned 32-bit integer of the owner.
            "id": 70,
            // The unique username of the owner.
            "login": "github:rust-lang:core",
            // Name of the owner.
            // This is optional and may be null.
            "name": "Core",
        }
    ]
}
```

##### Owners: Add

- Endpoint: `/api/v1/crates/{crate_name}/owners`
- Method: PUT
- Authorization: Included

A PUT request will send a request to the registry to add a new owner to a
crate. It is up to the registry how to handle the request. For example,
[crates.io] sends an invite to the user that they must accept before being
added.

The request should include the following JSON object:

```javascript
{
    // Array of `login` strings of owners to add.
    "users": ["login_name"]
}
```

A successful response includes the JSON object:

```javascript
{
    // Indicates the add succeeded, always true.
    "ok": true,
    // A string to be displayed to the user.
    "msg": "user ehuss has been invited to be an owner of crate cargo"
}
```

##### Owners: Remove

- Endpoint: `/api/v1/crates/{crate_name}/owners`
- Method: DELETE
- Authorization: Included

A DELETE request will remove an owner from a crate. The request should include
the following JSON object:

```javascript
{
    // Array of `login` strings of owners to remove.
    "users": ["login_name"]
}
```

A successful response includes the JSON object:

```javascript
{
    // Indicates the remove succeeded, always true.
    "ok": true
}
```

#### Search

- Endpoint: `/api/v1/crates`
- Method: GET
- Query Parameters:
    - `q`: The search query string.
    - `per_page`: Number of results, default 10, max 100.

The search request will perform a search for crates, using criteria defined on
the server.

A successful response includes the JSON object:

```javascript
{
    // Array of results.
    "crates": [
        {
            // Name of the crate.
            "name": "rand",
            // The highest version available.
            "max_version": "0.6.1",
            // Textual description of the crate.
            "description": "Random number generators and other randomness functionality.\n",
        }
    ],
    "meta": {
        // Total number of results available on the server.
        "total": 119
    }
}
```

#### Login

- Endpoint: `/me`

The "login" endpoint is not an actual API request. It exists solely for the
[`cargo login`] command to display a URL to instruct a user to visit in a web
browser to log in and retrieve an API token.

[Source Replacement]: source-replacement.md
[`cargo login`]: ../commands/cargo-login.md
[`cargo package`]: ../commands/cargo-package.md
[`cargo publish`]: ../commands/cargo-publish.md
[alphanumeric]: ../../std/primitive.char.html#method.is_alphanumeric
[config]: config.md
[crates.io]: https://crates.io/
[publishing documentation]: publishing.md#cargo-owner
