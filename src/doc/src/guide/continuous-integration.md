## Continuous Integration

### Travis CI

To test your package on Travis CI, here is a sample `.travis.yml` file:

```yaml
language: rust
rust:
  - stable
  - beta
  - nightly
matrix:
  allow_failures:
    - rust: nightly
```

This will test all three release channels, but any breakage in nightly
will not fail your overall build. Please see the [Travis CI Rust
documentation](https://docs.travis-ci.com/user/languages/rust/) for more
information.

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
[GitLab CI](https://docs.gitlab.com/ce/ci/yaml/README.html) for more
information.

### GitHub Actions

To test your package on GitHub Actions, here is a sample `workflow.yml` file, which has to be in the directory `.github/workflows`:
```yaml
name: Rust

on: [push]

jobs:
  build:

    runs-on: ubuntu-latest

    steps:
    - uses: actions/checkout@v2

    - name: Cache cargo registry
      uses: actions/cache@v1
      with:
        path: ~/.cargo/registry
        key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}
    - name: Cache cargo build
      uses: actions/cache@v1
      with:
        path: target
        key: ${{ runner.os }}-cargo-build-target-${{ hashFiles('**/Cargo.lock') }}

    - name: Build
      run: cargo build --verbose
    - name: Run tests
      run: cargo test --verbose
```
This will run on a push to the repository. Pleas see the [GitHub Actions help](https://help.github.com/en/actions) for more information.

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
