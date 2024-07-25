# Introduction
`typst-test` is a test runner for [Typst](https://typst.app/) projects. It helps you worry less about regressions and speeds up your development.

<script src="https://asciinema.org/a/669405.js" id="asciicast-669405" async="true"></script>

## Bird's-Eye View
Out of the box `typst-test` supports the following features:
- locate the project it is invoked in
- collect and manage test scripts and references
- compile and run tests
- compare test output to references
- provide extra scripting functionality
- running custom scripts for test automation

## A Closer Look
This book contains a few sections aimed at answering the most common questions right out the gate.
- [Installation](./quickstart/install.md) outlines various ways to install `typst-test`.
- [Usage](./quickstart/usage.md) goes over some basic commands to get started with `typst-test`.
- [Features](./quickstart/features.md) introduces various concepts to help you use `typst-test` effectively.
- [Configuration](.quickstart/config.md) explains commonly used configuration keys.

After the quick start, a few guides delve deeper into some advanced topics.
- [Automation](./guides/automation.md) explains the ins and outs of hooks and how they can be used for testing typst preprocessors or formatters.
- [Using Test Sets](./guides/test-sets.md) delves into the test set language and how it can be used to isolate tests and speed up your TDD workflow.
- [Setting Up CI](./guides/ci.md) shows how to set up `typst-test` to continuously test all changes to your package.

The later sections of the book are a technical reference to `typst-test` and its various features or concepts.
- [Tests](./reference/tests/index.md) outlines which types of tests `typst-test` supports, how they can be customized and which features are offered within the test scripts.
- [Test Set Language](./reference/test-sets/index.md) defines the test set language and its built in test sets.
- [Configuration Schema](./reference/config.md) lists all existing config options, their expected types and default values.
- [Command Line Tool](./reference/cli/index.md) goes over `typst-test`s various sub commands, arguments and options.

