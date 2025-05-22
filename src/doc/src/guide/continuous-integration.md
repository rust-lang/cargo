# Continuous Integration

## Getting Started

A basic CI will build and test your projects:

### GitHub Actions

To test your package on GitHub Actions, here is a sample `.github/workflows/ci.yml` file:

```yaml
name: Cargo Build & Test

on:
  push:
  pull_request:

env: 
  CARGO_TERM_COLOR: always

jobs:
  build_and_test:
    name: Rust project - latest
    runs-on: ubuntu-latest
    strategy:
      matrix:
        toolchain:
          - stable
          - beta
          - nightly
    steps:
      - uses: actions/checkout@v4
      - run: rustup update ${{ matrix.toolchain }} && rustup default ${{ matrix.toolchain }}
      - run: cargo build --verbose
      - run: cargo test --verbose
  
```

This will test all three release channels (note a failure in any toolchain version will fail the entire job). You can also click `"Actions" > "new workflow"` in the GitHub UI and select Rust to add the [default configuration](https://github.com/actions/starter-workflows/blob/main/ci/rust.yml) to your repo. See [GitHub Actions documentation](https://docs.github.com/en/actions) for more information.

### GitLab CI

To test your package on GitLab CI, here is a sample `.gitlab-ci.yml` file:

```yaml
stages:
  - build

rust-latest:
  stage: build
  image: rust:latest
  script:
    - cargo build --verbose
    - cargo test --verbose

rust-nightly:
  stage: build
  image: rustlang/rust:nightly
  script:
    - cargo build --verbose
    - cargo test --verbose
  allow_failure: true
```

This will test on the stable channel and nightly channel, but any
breakage in nightly will not fail your overall build. Please see the
[GitLab CI documentation](https://docs.gitlab.com/ce/ci/yaml/index.html) for more
information.

### builds.sr.ht

To test your package on sr.ht, here is a sample `.build.yml` file.
Be sure to change `<your repo>` and `<your project>` to the repo to clone and
the directory where it was cloned.

```yaml
image: archlinux
packages:
  - rustup
sources:
  - <your repo>
tasks:
  - setup: |
      rustup toolchain install nightly stable
      cd <your project>/
      rustup run stable cargo fetch
  - stable: |
      rustup default stable
      cd <your project>/
      cargo build --verbose
      cargo test --verbose
  - nightly: |
      rustup default nightly
      cd <your project>/
      cargo build --verbose ||:
      cargo test --verbose  ||:
  - docs: |
      cd <your project>/
      rustup run stable cargo doc --no-deps
      rustup run nightly cargo doc --no-deps ||:
```

This will test and build documentation on the stable channel and nightly
channel, but any breakage in nightly will not fail your overall build. Please
see the [builds.sr.ht documentation](https://man.sr.ht/builds.sr.ht/) for more
information.


### CircleCI

To test your package on CircleCI, here is a sample `.circleci/config.yml` file:

```yaml
version: 2.1
jobs:
  build:
    docker:
      # check https://circleci.com/developer/images/image/cimg/rust#image-tags for latest
      - image: cimg/rust:1.77.2
    steps:
      - checkout
      - run: cargo test
```

To run more complex pipelines, including flaky test detection, caching, and artifact management, please see [CircleCI Configuration Reference](https://circleci.com/docs/configuration-reference/).

## Verifying Latest Dependencies

When [specifying dependencies](../reference/specifying-dependencies.md) in
`Cargo.toml`, they generally match a range of versions.
Exhaustively testing all version combination would be unwieldy.
Verifying the latest versions would at least test for users who run [`cargo
add`] or [`cargo install`].

When testing the latest versions some considerations are:
- Minimizing external factors affecting local development or CI
- Rate of new dependencies being published
- Level of risk a project is willing to accept
- CI costs, including indirect costs like if a CI service has a maximum for
  parallel runners, causing new jobs to be serialized when at the maximum.

Some potential solutions include:
- [Not checking in the `Cargo.lock`](../faq.md#why-have-cargolock-in-version-control)
  - Depending on PR velocity, many versions may go untested
  - This comes at the cost of determinism
- Have a CI job verify the latest dependencies but mark it to "continue on failure"
  - Depending on the CI service, failures might not be obvious
  - Depending on PR velocity, may use more resources than necessary
- Have a scheduled CI job to verify latest dependencies
  - A hosted CI service may disable scheduled jobs for repositories that
    haven't been touched in a while, affecting passively maintained packages
  - Depending on the CI service, notifications might not be routed to people
    who can act on the failure
  - If not balanced with dependency publish rate, may not test enough versions
    or may do redundant testing
- Regularly update dependencies through PRs, like with [Dependabot] or [RenovateBot]
  - Can isolate dependencies to their own PR or roll them up into a single PR
  - Only uses the resources necessary
  - Can configure the frequency to balance CI resources and coverage of dependency versions

An example CI job to verify latest dependencies, using GitHub Actions:
```yaml
jobs:
  latest_deps:
    name: Latest Dependencies
    runs-on: ubuntu-latest
    continue-on-error: true
    env:
      CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS: allow
    steps:
      - uses: actions/checkout@v4
      - run: rustup update stable && rustup default stable
      - run: cargo update --verbose
      - run: cargo build --verbose
      - run: cargo test --verbose
```
Notes:
- [`CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS`](../reference/config.md#resolverincompatible-rust-versions) is set to ensure the [resolver](../reference/resolver.md) doesn't limit selected dependencies because of your project's [Rust version](../reference/rust-version.md).

For projects with higher risks of per-platform or per-Rust version failures,
more combinations may want to be tested.

## Verifying `rust-version`

When publishing packages that specify [`rust-version`](../reference/manifest.md#the-rust-version-field),
it is important to verify the correctness of that field.

Some third-party tools that can help with this include:
- [`cargo-msrv`](https://crates.io/crates/cargo-msrv)
- [`cargo-hack`](https://crates.io/crates/cargo-hack)

An example of one way to do this, using GitHub Actions:
```yaml
jobs:
  msrv:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    - uses: taiki-e/install-action@cargo-hack
    - run: cargo hack check --rust-version --workspace --all-targets --ignore-private
```
This tries to balance thoroughness with turnaround time:
- A single platform is used as most projects are platform-agnostic, trusting platform-specific dependencies to verify their behavior.
- `cargo check` is used as most issues contributors will run into are API availability and not behavior.
- Unpublished packages are skipped as this assumes only consumers of the verified project, through a registry, will care about `rust-version`.

[`cargo add`]: ../commands/cargo-add.md
[`cargo install`]: ../commands/cargo-install.md
[Dependabot]: https://docs.github.com/en/code-security/dependabot/working-with-dependabot
[RenovateBot]: https://renovatebot.com/
