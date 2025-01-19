# Tests
There are three types of tests:
- Regression tests, which are similar to unit or integration tests in other languages and are mostly used to test the API of a package and visual regressions through comparison with reference documents.
  Regression tests are standalone files in a `tests` directory inside the project root and have additional features available inside typst using a custom standard library.
- Template tests, which are similar to regression tests, but don't get any special features and are only available as persistent tests.
- Doc tests, example code in documentation comments which are compiled but not compared.

<div class="warning">

`typst-test` can currently only collect and operate on regression tests.

In the future, template tests and doc tests will be added, see [#34] and [#49] respectively.

</div>

Any test may use [annotations](./annotations.md) for configuration.

Read the [guide], if you want to see some examples on how to write and run various tests.

## Sections
- [Regression tests](./regression-tests.md) explains the structure of regression tests.
- [Regression test library](./lib.md) lists the declarations of the custom standard library.
- [Annotations](./annotations.md) lists the syntax for annotations and which are available.

[guide]: ../../guides/tests.md
[#34]: https://github.com/tingerrr/typst-test/issues/34
[#49]: https://github.com/tingerrr/typst-test/issues/49
