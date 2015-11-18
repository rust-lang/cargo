% Package ID Specifications - Cargo Documentation

# Package ID Specifications

Subcommands of cargo frequently need to refer to a particular package within a
dependency graph for various operations like updating, cleaning, building, etc.
To solve this problem, cargo supports Package ID Specifications. A specification
is a string which is used to uniquely refer to one package within a graph of
packages.

## Specification grammar

The formal grammar for a Package Id Specification is:

```notrust
pkgid := pkgname
       | [ proto "://" ] hostname-and-path [ "#" ( pkgname | semver ) ]
pkgname := name [ ":" semver ]

proto := "http" | "git" | ...
```

Here, brackets indicate that the contents are optional.

## Example Specifications

These could all be references to a package `foo` version `1.2.3` from the
registry at `crates.io`

|         pkgid                  |  name  |  version  |          url         |
|-------------------------------:|:------:|:---------:|:--------------------:|
| `foo`                          | foo    | *         | *                    |
| `foo:1.2.3`                    | foo    | 1.2.3     | *                    |
| `crates.io/foo`                | foo    | *         | *://crates.io/foo    |
| `crates.io/foo#1.2.3`          | foo    | 1.2.3     | *://crates.io/foo    |
| `crates.io/bar#foo:1.2.3`      | foo    | 1.2.3     | *://crates.io/bar    |
| `http://crates.io/foo#1.2.3`   | foo    | 1.2.3     | http://crates.io/foo |

## Brevity of Specifications

The goal of this is to enable both succinct and exhaustive syntaxes for
referring to packages in a dependency graph. Ambiguous references may refer to
one or more packages. Most commands generate an error if more than one package
could be referred to with the same specification.
