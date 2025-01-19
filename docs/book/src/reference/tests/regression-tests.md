# Regression tests
Regression tests are those tests found in their on directory identified by a `test.typ` script and are located in `tests`.

Regression tests are the only tests which have access to an extended Typst standard library.
This [test library](./lib.md) contains modules and functions to thoroughly test both the success and failure paths of your project.

## Test kinds
There are three kinds of regression tests:
- `compile-only`: Tests which are compiled, but not compared to any reference, these don't produce any output.
- `persistent`: Tests which are compared to persistent reference documents.
  The references for these tests are stored in a `ref` directory along side the test script as individual pages using PNGs.
  These tests can be updated with the `tt update` command.
- `ephemeral`: Tests which are compared to the output of another script.
  The references for these tests are compiled on the fly using a `ref.typ` script.

Each of these kinds is available as a test set function.

## Identifiers
The directory path within the test root `tests` in your project is the identifier of a test and uses forward slahes as path separators on all platforms, the individual components of a test path must satisfy the following rules:
- must start with an ASCII alphabetic character (`a`-`z` or `A`-`Z`)
- may contain any additional sequence of ASCII alphabetic characters, numeric characters (`0`-`9`), underscores `_` or hyphens `-`

## Test structure
Given a directory within `tests`, it is considered a valid test, if it contains at least a `test.typ` file.
The strucutre of this directory looks as follows:
- `test.typ`: The main test script, this is always compiled as the entrypoint.
- `ref.typ` (optional): This makes a test ephemeral and is used to compile the reference document for eahc invocation.
- `ref` (optional, temporary): This makes a test either persistent or ephemeral and is used to store the reference documents.
  If the test is ephemeral this directory is temporary.
- `out` (temporary): Contains the test output document.
- `diff` (temporary): Contains the difference of the output and reference documents.

The kind of a test is determined as follows:
- If it contains a `ref` directory but no `ref.typ` script, it is considered a persistent test.
- If it contians a `ref.typ` script, it is considered an ephemeral test.
- If it contains neither, it is considered compile only.

Temporary directories are ignored within the VCS if one is detected, this is currently done by simply adding an ignore file within the directory which ignores all entries inside it.

A test cannot contain other her tests, if a test script is found `typst-test` will not search for any sub tests.

Regression test are compiled with the project root as their typst root, such that they can easily access package internals with absolute paths.

## Comparison
Ephemeral and persistent tests are curently compared using a simple deviation threshold which determines if two images should be considered the same or different.
If the images have differnet dimensions consider them different.
Given two images of equal dimensions, pair up each pixel and compare them, if any of the 3 channels (red, green, blue) differ by at least `min-delta` count it as a deviation.
If there are more than `max-deviation` of such deviating pixels, consider the images different.

These values can be tweaked on the command line using the `--max-deviation` and `--min-delta` options respectively:
- `--max-deviation` takes a non-negative integer, i.e. any value from `0` onwards.
- `--min-delta` takes a byte, i.e. any value from `0` to `255`.

Both values default to `0` such that any difference will trigger a failure by default.
