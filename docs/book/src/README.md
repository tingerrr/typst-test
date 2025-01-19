# Introduction
`typst-test` is a test runner for [Typst](https://typst.app/) projects.
It helps you worry less about regressions and speeds up your development.

<a href="https://asciinema.org/a/rW9HGUBbtBnmkSddgbKb7hRlI" target="_blank"><img src="https://asciinema.org/a/rW9HGUBbtBnmkSddgbKb7hRlI.svg" /></a>

## Bird's-Eye View
Out of the box `typst-test` supports the following features:
- compile and compare tests
- manage regression tests of various types
- manage and update reference documents when tests change
- filter tests effectively for concise test runs

## A Closer Look
This book contains a few sections aimed at answering the most common questions right out the gate:
- [Installation](./quickstart/install.md) outlines various ways to install `typst-test`.
- [Usage](./quickstart/usage.md) goes over some basic commands to get started.

After the quick start, a few guides delve deeper into some advanced topics, such as
- [Writing Tests](./guides/tests.md) shows how tests work and how you can add, remove and update them.
- [Using Test Sets](./guides/test-sets.md) delves into the test set language and how it can be used to isolate tests and speed up your TDD workflow.
- [Watching for Changes](./guides/watching.md) explains a workaround for how you can run tests repeatedly on changes to your project files.
- [Setting Up CI](./guides/ci.md) shows how to set up `typst-test` in your CI.

The later sections of the book are a technical reference to `typst-test` and its various features or concepts:
- [Tests](./reference/tests/index.md) explains all features of tests in-depth.
- [Test Set Language](./reference/test-sets/index.md) explains the ins and outs of the test set language, listing its operators, built-in bindings and syntactic and semantic intricacies.
<!-- - [Configuration Schema](./reference/config.md) lists all existing config options, their expected types and default values. -->

