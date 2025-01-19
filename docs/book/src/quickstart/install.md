# Installation
To install `typst-test` on your PC, you must, for the time being, compile it from source.
Once `typst-test` reaches 0.1.0, this restriction will be lifted and each release will provide precompiled binaries for major operating systems (Windows, Linux and macOS).

## Installation From Source
To install `typst-test` from source, you must have a Rust toolchain (Rust **v1.80.0+**) and cargo installed.

Run the following command to install the latest nightly version:
```bash
cargo install --locked --git https://github.com/tingerrr/typst-test
```
This version has the newest features but may have unfixed bugs or rough edges.

To install the latest backport version run:
```bash
cargo install --locked --git https://github.com/tingerrr/typst-test --branch backport
```
This version is more stable but doesn't contain most of the features listed in this book, it is mostly provided for backporting critical fixes until `0.1.0` is released.

## Required Libraries
### OpenSSL
OpenSSL (**v1.0.1** to **v3.x.x**) or LibreSSL (**v2.5** to **v3.7.x**) are required to allow `typst-test` to download packages from the [Typst Universe](https://typst.app/universe) package registry.

When installing from source the `vendor-openssl` feature can be used on operating systems other than Windows and macOS to  vendor and statically link to OpenSSL, avoiding the need for it on the operating system.
