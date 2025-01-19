# Writing Tests
Typst-test currently supports a single type of test: regression tests.
These are located under the `tests` directory and are identified by their path.
A regression test must have at least one directory component, i.e. `tests/test.typ` is not valid and will be ignored.
For example, `tests/features/fancy-box/test.typ` is identified as `features/fancy-box`.

For example, to add a new test, run
```bash
tt add features/fancy-box
```
then run
```bash
tt run
```
to run the test suite, the new test should show up in the test run and pass.

To remove the test, run
```bash
tt remove features/fancy-box
```

## Test templates
Regression test can often have plenty of boilerplate, if Typst-test finds a `template.typ` file direclty in the `tests` directory, it will use this for new tests instead of the default `Hello World` test.
This can be turned of with `--no-template` when running the `add` sub command.

## Reference Kinds
Regression tests come in three different kinds:
- `compile-only`, these pass once they compile successfully and don't have any references to compare their output to.
- `ephemeral`, these are tests which are compared to a reference document created by a special `ref.typ` script.
- `persistent`, these are tests which are compared to a reference document which is persisted on disk as a series of PNG images, one for each page. This is the default for new tests.

To configure which kind of regression test should be added, you can use the `--compile-only` and `--ephemeral` flags on the `add` sub command.

Since the references for persistent regression tests may change as a project evolves, Typst-test provides a command to update those references to matcht he output of the current test script.

If you update your project and a persistent regression test fails, but the change in output was deliberate, you can run `tt update features/fancy-box` to update this test.

This will compile the document and save it as the new refernce document.

<div class="warning">

Note that, at the moment typst-test does not compress the reference images, this means that, if you use a version control system like git or mericural, the reference images of persistent tests can quickly bloat your repository if you update them frequently.
Consider using a program like [`oxipng`][oxipng] to compress them, Typst-test can still read them without any problems.

</div>

<div class="warning">

While there are currently no other test types supported, adding special support for testing template packages and documentation examples is planned.
See [#34] and [#49].

</div>

[#34]: https://github.com/tingerrr/typst-test/issues/34
[#49]: https://github.com/tingerrr/typst-test/issues/49
[oxipng]: https://github.com/shssoichiro/oxipng
