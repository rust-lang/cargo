# Credential Provider Protocol
This document describes information for building a Cargo credential provider. For information on
setting up or using a credential provider, see [Registry Authentication](registry-authentication.md).

When using an external credential provider, Cargo communicates with the credential
provider using stdin/stdout messages passed as single lines of JSON.

Cargo will always execute the credential provider with the `--cargo-plugin` argument.
This enables a credential provider executable to have additional functionality beyond
what Cargo needs. Additional arguments are included in the JSON via the `args` field.

## JSON messages
The JSON messages in this document have newlines added for readability.
Actual messages must not contain newlines.

### Credential hello
* Sent by: credential provider
* Purpose: used to identify the supported protocols on process startup
```javascript
{
    "v":[1]
}
```

Requests sent by Cargo will include a `v` field set to one of the versions listed here.
If Cargo does not support any of the versions offered by the credential provider, it will issue an
error and shut down the credential process.

### Registry information
* Sent by: Cargo
Not a message by itself. Included in all messages sent by Cargo as the `registry` field.
```javascript
{
    // Index URL of the registry
    "index-url":"https://github.com/rust-lang/crates.io-index",
    // Name of the registry in configuration (optional)
    "name": "crates-io",
    // HTTP headers received from attempting to access an authenticated registry (optional)
    "headers": ["WWW-Authenticate: cargo"]
}
```

### Login request
* Sent by: Cargo
* Purpose: collect and store credentials
```javascript
{
    // Protocol version
    "v":1,
    // Action to perform: login
    "kind":"login",
    // Registry information (see Registry information)
    "registry":{"index-url":"sparse+https://registry-url/index/", "name": "my-registry"},
    // User-specified token from stdin or command line (optional)
    "token": "<the token value>",
    // URL that the user could visit to get a token (optional)
    "login-url": "http://registry-url/login",
    // Additional command-line args (optional)
    "args":[]
}
```

If the `token` field is set, then the credential provider should use the token provided. If
the `token` is not set, then the credential provider should prompt the user for a token.

In addition to the arguments that may be passed to the credential provider in
configuration, `cargo login` also supports passing additional command line args
via `cargo login -- <additional args>`. These additional arguments will be included
in the `args` field after any args from Cargo configuration.

### Read request
* Sent by: Cargo
* Purpose: Get the credential for reading crate information
```javascript
{
    // Protocol version
    "v":1,
    // Request kind: get credentials
    "kind":"get",
    // Action to perform: read crate information
    "operation":"read",
    // Registry information (see Registry information)
    "registry":{"index-url":"sparse+https://registry-url/index/", "name": "my-registry"},
    // Additional command-line args (optional)
    "args":[]
}
```

### Publish request
* Sent by: Cargo
* Purpose: Get the credential for publishing a crate
```javascript
{
    // Protocol version
    "v":1,
    // Request kind: get credentials
    "kind":"get",
    // Action to perform: publish crate
    "operation":"publish",
    // Crate name
    "name":"sample",
    // Crate version
    "vers":"0.1.0",
    // Crate checksum
    "cksum":"...",
    // Registry information (see Registry information)
    "registry":{"index-url":"sparse+https://registry-url/index/", "name": "my-registry"},
    // Additional command-line args (optional)
    "args":[]
}
```

### Get success response
* Sent by: credential provider
* Purpose: Gives the credential to Cargo
```javascript
{"Ok":{
    // Response kind: this was a get request
    "kind":"get",
    // Token to send to the registry
    "token":"...",
    // Cache control. Can be one of the following:
    // * "never": do not cache
    // * "session": cache for the current cargo session
    // * "expires": cache for the current cargo session until expiration
    "cache":"expires",
    // Unix timestamp (only for "cache": "expires")
    "expiration":1693942857,
    // Is the token operation independent?
    "operation_independent":true
}}
```

The `token` will be sent to the registry as the value of the `Authorization` HTTP header.

`operation_independent` indicates whether the token can be cached across different
operations (such as publishing or fetching). In general, this should be `true` unless
the provider wants to generate tokens that are scoped to specific operations.

### Login success response
* Sent by: credential provider
* Purpose: Indicates the login was successful
```javascript
{"Ok":{
    // Response kind: this was a login request
    "kind":"login"
}}
```

### Logout success response
* Sent by: credential provider
* Purpose: Indicates the logout was successful
```javascript
{"Ok":{
    // Response kind: this was a logout request
    "kind":"logout"
}}
```

### Failure response (URL not supported)
* Sent by: credential provider
* Purpose: Gives error information to Cargo
```javascript
{"Err":{
    "kind":"url-not-supported"
}}
```
Sent if the credential provider is designed
to only handle specific registry URLs and the given URL
is not supported. Cargo will attempt another provider if
available.

### Failure response (not found)
* Sent by: credential provider
* Purpose: Gives error information to Cargo
```javascript
{"Err":{
    // Error: The credential could not be found in the provider.
    "kind":"not-found"
}}
```
Sent if the credential could not be found. This is expected for
`get` requests where the credential is not available, or `logout`
requests where there is nothing found to erase.

### Failure response (operation not supported)
* Sent by: credential provider
* Purpose: Gives error information to Cargo
```javascript
{"Err":{
    // Error: The credential could not be found in the provider.
    "kind":"operation-not-supported"
}}
```
Sent if the credential provider does not support the requested operation.
If a provider only supports `get` and a `login` is requested, the
provider should respond with this error.

### Failure response (other)
* Sent by: credential provider
* Purpose: Gives error information to Cargo
```javascript
{"Err":{
    // Error: something else has failed
    "kind":"other",
    // Error message string to be displayed
    "message": "free form string error message",
    // Detailed cause chain for the error (optional)
    "caused-by": ["cause 1", "cause 2"]
}}
```

## Example communication to request a token for reading:
1. Cargo spawns the credential process, capturing stdin and stdout.
2. Credential process sends the Hello message to Cargo
    ```javascript
    { "v": [1] }
   ```
3. Cargo sends the CredentialRequest message to the credential process (newlines added for readability).
    ```javascript
    {
        "v": 1,
        "kind": "get",
        "operation": "read",
        "registry":{"index-url":"sparse+https://registry-url/index/"}
    }
    ```
4. Credential process sends the CredentialResponse to Cargo (newlines added for readability).
    ```javascript
    {
        "token": "...",
        "cache": "session",
        "operation_independent": true
    }
    ```
5. Cargo closes the stdin pipe to the credential provider and it exits.
6. Cargo uses the token for the remainder of the session (until Cargo exits) when interacting with this registry.
