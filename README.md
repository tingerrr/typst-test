# typst test
`typst-test` is a test runner for [Typst] projects. It helps you worry less about regressions and speeds up your development.

## Features
Out of the box `typst-test` supports the following features:
- locate the project it is invoked in
- collect and manage test scripts and references
- compile and run tests
- compare test output to references
- provide extra scripting functionality
- running custom scripts for test automation

`typst-test` does not currently include a "watch" command to automatically run anytime a file changes.
However, the book [includes a suggested workaround for this](https://tingerrr.github.io/typst-test/guides/watching.html).

## Stability
`typst-test` currently makes no stability guarantees, it is considered pre-0.1.0, see the [Milestones] for its progress towards a first release.
However, all PRs and pushes to main are tested in CI.
A reasonably "stable" version of `typst-test` is available at the `ci-semi-stable` tag.
This version is already used in the CI of various Typst packages, such as cetz, codly, valkyrie, hydra or subpar.
Some prior changes impacting users of `typst-test` are documented in the [migration log][migrating].

## Documentation
To see how to get started with `typst-test`, check out the [Book].
It provides a few chapters aimed to get you started with `typst-test`.

[![An asciicast showing typst-test running the full cetz test suite.][demo-thumb]][demo]

## Contribution
See [CONTRIBUTING.md][contrib] if you want to contribute to `typst-test`.

[migrating]: migrating.md
[contrib]: CONTRIBUTING.md

[Typst]: https://typst.app
[Book]: https://tingerrr.github.io/typst-test/index.html
[Milestones]: https://github.com/tingerrr/typst-test/milestones

[demo-thumb]: https://asciinema.org/a/669405.svg
[demo]: https://asciinema.org/a/669405
