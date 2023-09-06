# Registries

Cargo installs crates and fetches dependencies from a "registry". The default
registry is [crates.io]. A registry contains an "index" which contains a
searchable list of available crates. A registry may also provide a web API to
support publishing new crates directly from Cargo.

> Note: If you are interested in mirroring or vendoring an existing registry,
> take a look at [Source Replacement].

If you are implementing a registry server, see [Running a Registry] for more
details about the protocol between Cargo and a registry.

If you're using a registry that requires authentication, see [Registry Authentication].
If you are implementing a credential provider, see [Credential Provider Protocol]
for details.

## Using an Alternate Registry

To use a registry other than [crates.io], the name and index URL of the
registry must be added to a [`.cargo/config.toml` file][config]. The `registries`
table has a key for each registry, for example:

```toml
[registries]
my-registry = { index = "https://my-intranet:8080/git/index" }
```

The `index` key should be a URL to a git repository with the registry's index or a
Cargo sparse registry URL with the `sparse+` prefix.

A crate can then depend on a crate from another registry by specifying the
`registry` key and a value of the registry's name in that dependency's entry
in `Cargo.toml`:

```toml
# Sample Cargo.toml
[package]
name = "my-project"
version = "0.1.0"
edition = "2021"

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

## Publishing to an Alternate Registry

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
key. For example:

```toml
[registry]
default = "my-registry"
```

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

## Registry Protocols
Cargo supports two remote registry protocols: `git` and `sparse`. If the registry
index URL starts with `sparse+`, Cargo uses the sparse protocol. Otherwise
Cargo uses the `git` protocol.

The `git` protocol stores index metadata in a git repository and requires Cargo to clone
the entire repo.

The `sparse` protocol fetches individual metadata files using plain HTTP requests.
Since Cargo only downloads the metadata for relevant crates, the `sparse` protocol can
save significant time and bandwidth.

The [crates.io] registry supports both protocols. The protocol for crates.io is
controlled via the [`registries.crates-io.protocol`] config key.

[Source Replacement]: source-replacement.md
[Running a Registry]: running-a-registry.md
[Credential Provider Protocol]: credential-provider-protocol.md
[Registry Authentication]: registry-authentication.md
[`cargo publish`]: ../commands/cargo-publish.md
[`cargo package`]: ../commands/cargo-package.md
[`cargo login`]: ../commands/cargo-login.md
[config]: config.md
[crates.io]: https://crates.io/
[`registries.crates-io.protocol`]: config.md#registriescrates-ioprotocol


<script>
(function() {
    var fragments = {
        "#running-a-registry": "running-a-registry.html",
        "#index-format": "registry-index.html",
        "#web-api": "registry-web-api.html",
        "#publish": "registry-web-api.html#publish",
        "#yank": "registry-web-api.html#yank",
        "#unyank": "registry-web-api.html#unyank",
        "#owners": "registry-web-api.html#owners",
        "#owners-list": "registry-web-api.html#owners-list",
        "#owners-add": "registry-web-api.html#owners-add",
        "#owners-remove": "registry-web-api.html#owners-remove",
        "#search": "registry-web-api.html#search",
        "#login": "registry-web-api.html#login",
    };
    var target = fragments[window.location.hash];
    if (target) {
        var url = window.location.toString();
        var base = url.substring(0, url.lastIndexOf('/'));
        window.location.replace(base + "/" + target);
    }
})();
</script>
