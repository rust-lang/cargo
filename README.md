## Compiling cargo

You'll want to clone cargo using --recursive on git, to clone in it's submodule dependencies.
```
$ git clone --recursive https://github.com/carlhuda/cargo
```
or
```
$ git submodule init
$ git submodule update
```
Then it's as simple as ```make``` and you're ready to go.

## Porcelain

### cargo-compile

```
$ cargo compile
```

This command assumes the following directory structure:

```
|Cargo.toml
|~src
| | {main,lib}.rs
|~target
| |~x86_64-apple-darwin
| | |~lib
| | | |~[symlinked dependencies]
| | | | [build artifacts]
| |~...
```

When running `cargo compile`, Cargo runs the following steps:

* `cargo verify --manifest=[location of Cargo.toml]`
* ... TODO: dependency resolution and downloading ...
* `cargo prepare`
* `cargo rustc --out-dir=[from Cargo.toml]/[platform] -L [from Cargo.toml]/[platform]/lib ...`

## Plumbing

### cargo-verify

```
$ cargo verify --manifest=MANIFEST
```

Verifies that the manifest is in the location specified, in a valid
format, and contains all of the required sections.

#### Success

```
{ "success": true }
```

#### Errors

```
{
  "invalid": < "not-found" | "invalid-format" >,
  "missing-field": [ required-field... ],
  "missing-source": bool,
  "unwritable-target": bool
}
```

### cargo-rustc

```
$ cargo rustc --out-dir=LOCATION -L LIBDIR -- ...ARGS
```

### cargo-prepare

Prepare the directories (including symlinking dependency libraries) to
be ready for the flags Cargo plans to pass into `rustc`.

## NOTES and OPEN QUESTIONS

* We need to support per-platform calls to `make` (et al) to build
  native (mostly C) code. Should this be part of `prepare` or a
  different step between `prepare` and `cargo-rustc`.
