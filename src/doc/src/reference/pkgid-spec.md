# Package ID Specifications

## Package ID specifications

Subcommands of Cargo frequently need to refer to a particular package within a
dependency graph for various operations like updating, cleaning, building, etc.
To solve this problem, Cargo supports *Package ID Specifications*. A specification
is a string which is used to uniquely refer to one package within a graph of
packages.

The specification may be fully qualified, such as
`https://github.com/rust-lang/crates.io-index#regex@1.4.3` or it may be
abbreviated, such as `regex`. The abbreviated form may be used as long as it
uniquely identifies a single package in the dependency graph. If there is
ambiguity, additional qualifiers can be added to make it unique. For example,
if there are two versions of the `regex` package in the graph, then it can be
qualified with a version to make it unique, such as `regex@1.4.3`.

### Specification grammar

The formal grammar for a Package Id Specification is:

```notrust
spec := pkgname
       | proto "://" hostname-and-path [ "#" ( pkgname | semver ) ]
pkgname := name [ ("@" | ":" ) semver ]
semver := digits [ "." digits [ "." digits [ "-" prerelease ] [ "+" build ]]]

proto := "http" | "git" | ...
```

Here, brackets indicate that the contents are optional.

The URL form can be used for git dependencies, or to differentiate packages
that come from different sources such as different registries.

### Example specifications

The following are references to the `regex` package on `crates.io`:

| Spec                                                        | Name    | Version |
|:------------------------------------------------------------|:-------:|:-------:|
| `regex`                                                     | `regex` | `*`     |
| `regex@1.4`                                                 | `regex` | `1.4.*` |
| `regex@1.4.3`                                               | `regex` | `1.4.3` |
| `https://github.com/rust-lang/crates.io-index#regex`        | `regex` | `*`     |
| `https://github.com/rust-lang/crates.io-index#regex@1.4.3`  | `regex` | `1.4.3` |

The following are some examples of specs for several different git dependencies:

| Spec                                                      | Name             | Version  |
|:----------------------------------------------------------|:----------------:|:--------:|
| `https://github.com/rust-lang/cargo#0.52.0`               | `cargo`          | `0.52.0` |
| `https://github.com/rust-lang/cargo#cargo-platform@0.1.2` | <nobr>`cargo-platform`</nobr> | `0.1.2`  |
| `ssh://git@github.com/rust-lang/regex.git#regex@1.4.3`    | `regex`          | `1.4.3`  |

Local packages on the filesystem can use `file://` URLs to reference them:

| Spec                                   | Name  | Version |
|:---------------------------------------|:-----:|:-------:|
| `file:///path/to/my/project/foo`       | `foo` | `*`     |
| `file:///path/to/my/project/foo#1.1.8` | `foo` | `1.1.8` |

### Brevity of specifications

The goal of this is to enable both succinct and exhaustive syntaxes for
referring to packages in a dependency graph. Ambiguous references may refer to
one or more packages. Most commands generate an error if more than one package
could be referred to with the same specification.
