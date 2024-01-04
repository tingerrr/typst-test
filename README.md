# typst test
`typst-test` is a program to compile, compare and update references of tests scripts for typst. It is
currently work in progress and is aimed at providing automated visual regression testing for typst
packages.

## Features
- auto discovery of current project using `typst.toml`
- overriding of typst binary to test typst PRs
- automatic compilation and optional visual comparison of test output for all tests
- diff image generation for visual aid
- project setup with git support
- updating and optimizing of reference images

## Planned features
- prettier output reporting, especially for slower operations such as updating
- cli and lib separation to allow others to reuse the primary test running implementation
- using the typst crate directly
  - detecting mutliple tests in one file with common setup, running tests fro a single file in
    isolation
  - in memory comparison with references
- custom user actions
- better diff images
- better test filtering

## Tutorial
Assuming typst-test is in your path and you're in a package project, this is how you use it on a
new project:
```bash
typst-test init
typst-test run
```

### Notable commands and options
The following commands are available:
- `init`, initialize a project with a test directory
- `uinit`, remove a project's test directory
- `clean`, clean test output artifacts
- `run [test-filter]`, compile and run tests matching `test-filter`
- `compile [test-filter]`, compile but don't run tests matching `test-filter`
- `update [test-filter]`, update the reference images for tests matching `test-filter`

`test-filter` is a substring filter. If no filter is given it will match all tests.

The following global options are available:
- `--typst`, the path to the typst binary to compile the tests with
- `--root`, the project root directory
  - the root directory for typst when compiling
  - where `tests` is placed on `init`
- `--verbose`, increase the logging verbosity
  - please run typst-test with this when reporting issues

## Motivation
After releasing a very broken version of [hydra], I started writing tests and, as such, also a small
script to run them automatically. I got a bit carried away and overengineered the test script, but
it had a fundamental flaw; It could not run tests in parallel. This and the additional burden of
maintaining the messy script was enough to prompt me to write this program. This is direct port
of my [hydra test script][hydra-test].

[hydra]: https://github.com/tingerrr/hydra
[hydra-test]: https://github.com/tingerrr/hydra/blob/10127b1a5835a40a127b437b082c395a61d082d1/tests/run.nu
