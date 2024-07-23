# Installation
To install `typst-test` on your PC, you must, for the time being, compile it from source.
Once `typst-test` reaches 0.1.0 this restruction will be lifted and each release will provide precompiled binaries for major operating systems.

## Installation From Source
To install `typst-test` from source you must have a Rust toolchain (Rust **v1.80.0+**) and cargo installed.

Run the following command to install the latest nightly version
```bash
cargo install --locked --git https://github.com/tingerrr/typst-test
```

To install the latest semi stable version run
```bash
cargo install --locked --git https://github.com/tingerrr/typst-test --tag ci-semi-stable
```

## Required Libraries
### Openssl
Openssl (**v???**) is required to allow `typst-test` to download packages from the [Typst Universe](https://typst.app/universe) package registry.

When installing from source the `vendor-openssl` feature can be used on operating systems other than Windows and macOS to statically vendor and statically link to openssl, avoiding the need for it on the operating system.

<div class="warning">

This is not yet possible, but will be once [#32](https://github.com/tingerrr/typst-test/issues/32) is resolved, in the meantime openssl may be linked ot dynamically as a transitive dependency.

<div>
