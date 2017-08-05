## Continuous integration

### Travis CI

To test your project on Travis CI, here is a sample `.travis.yml` file:

```
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
