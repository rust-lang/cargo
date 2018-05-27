## Continuous Integration

### Travis CI

To test your project on Travis CI, here is a sample `.travis.yml` file:

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

To test your project on GitLab CI, here is a sample `.gitlab-ci.yml` file:

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
