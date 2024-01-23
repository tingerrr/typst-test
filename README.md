# typst test
`typst-test` is a program to compile, compare and update references of tests scripts for typst.
It is currently work in progress and is aimed at providing automated visual regression testing for
typst packages.

## Features
- auto discovery of current project using `typst.toml`
- overriding of typst binary to test typst PRs
- automatic compilation and optional visual comparison of test output for all tests
- diff image generation for visual aid
- project setup with git support
- updating and optimizing of reference images

## Planned features
- cli and lib separation to allow others to reuse the primary test running implementation
- using the typst crate directly
  - detecting mutliple tests in one file with common setup, running tests fro a single file in
    isolation
  - in memory comparison with references
- custom user actions
- better diff images

## Stability
This is work in progress, as such no stability guarantees are made, any commit may change the
behavior of various commands. Such changes will be documented in the [migration log][migrating].

The tag `ci-semi-stable` is available to use typst-test in CI, see [`tests.yml`][ci-workflow] for
an example workflow which will run typst-test for PRs and pushes to your repo. This tag will only
be bumped when critical bugs related to exisiting functionality are fixed. It will be retired once
typst-test reaches `0.1.0`.

## Tutorial
You can install typst-test by running:
```bash
cargo install typst-test --git https://github.com/tingerrr/typst-test
```

Assuming typst-test is in your path and you're in a package project, this is how you use it on a
new project:
```bash
typst-test init
typst-test run
```

[![An asciicast showing typs-test running with one test failing.][demo-thumb]][demo]

[ci-workflow]: assets/workflows/tests.yml

[migrating]: migrating.md
[demo-thumb]: https://asciinema.org/a/tbjXoYpZ0UPSiFxtO2vOaAW8v.svg
[demo]: https://asciinema.org/a/tbjXoYpZ0UPSiFxtO2vOaAW8v
