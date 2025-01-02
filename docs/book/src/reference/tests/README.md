# Tests

There are currently three types of tests:
- Unit tests, tests which are run to test regressions on code changes mostly through comparison to reference documents.
- Template tests, special tests for template packages which take a scaffold document and attempt to compile it and optionally compare it.
- Doc tests, example code in documentation comments which are compiled but not compared.

<div class="warning">

`typst-test` can currently only operate on unit tests found as individual files in the test root.

In the future, template tests and doc tests will be added, see [#34] and [#49].

</div>

Tests get access to a special [test library](./lib.md) and can use [annotations](./annotations.md) configuration.

## Unit Tests
Unit tests are found in the test root as individual scripts and are the most versatile type of test.
There are three kinds of unit tests:
- compile only, tests which are compiled, but not compared
- compared
  - persistent, tests which are compared to reference persistent documents
  - ephemeral, tests which are compared to the output of another script which is compiled on the fly

> Each of those can be selected using one of the [built-in test sets](../test-sets/built-in.md#constants).

Unit tests are the only tests which have access to an extended Typst standard library.
This extended standard library provides currently provides panic-helpers for catching and comparing panics.

A test is a directory somewhere within the test root (commonly `<project>/tests`), which contains the following entries:
- `test.typ`: as the entry point
- `ref.typ` (optional): for ephemeral tests as the reference entry point
- `ref/` (optional, temporary): for persistent or ephemeral tests for the reference documents
- `out/` (temporary) for the test documents
- `diff/` (temporary) for the diff documents

The path from the test root to the test script marks the test's identifier. Its test kind is determined by the existence of the ref script and ref directory:
- If it contains a `ref` directory but no `ref.typ` script, it is considered a persistent test.
- If it a `ref.typ` script, it is considered an ephemeral test.
- If it contains neither, it is considered compile only.

Tests may contain other tests at the moment, e.g the following is valid
```txt
tests/
  foo
  foo/test.typ
  foo/bar
  foo/bar/test.typ
```

and contains the tests `foo` and `foo/bar`.

Unit tests are compiled with the project root as typst root, such that they can easily access package internals.
They can also access test library items such as `catch` for catching and binding panics for testing error reporting:

```typst
/// [annotation]
///
/// Description

// access to internals
#import "/src/internal.typ": foo

#let panics = () => {
  foo("bar")
}

// ensures there's a panic
#assert-panic(panics)

// unwraps the panic if there is one
#assert.eq(
  catch(panics).first(),
  "panicked with: Invalid arg, expected `int`, got `str`",
)
```

## Documentation Tests
TODO: See [#34].

## Template Tests
TODO: See [#49].

[#34]: https://github.com/tingerrr/typst-test/issues/34
[#49]: https://github.com/tingerrr/typst-test/issues/49
