# Migrating
This file documents breaking changes and how to handle them while using the main branch of
typst-test. The entries are ordered in decending relevance, i.e. last breaking change first.

This file will be removed on the first release, as from then on, a changelog shall be curated.

## CI semi stable tag
The `ci-semi-stable` tag will no longer be bumped on breaking changes. Instead a branch of the same
name that follows `main` is provided for the same purpose.

Simply change your CI step to use the branch option instead:
```diff
 jobs:
   tests:
     # ...
     steps:
       - name: Install typst-test from github
         uses: baptiste0928/cargo-install@v2.2.0
         with:
           crate: typst-test
           git: https://github.com/tingerrr/typst-test.git
-          tag: ci-semi-stable
+          branch: ci-semi-stable
     # ...
```

## Folder Structure
The folder structure changed from having all tests in a dediacted folder with referencs and the like
in different dedicated folders to having a dedicated folder per test. To use your existing project's
tests, the scripts have to be moved and renamed. Previously tests were be arranged like follows:
```
tests/
  typ/
    test1.typ
    test2/
      test.typ
      ...
    ...
```

To reuse the scripts, move them into the following structure:
```
tests/
  test1/
    test.typ
    ref/
      1.png
    out/
      ...
    diff/
      ...
  test2/
    test.typ
    ...
  ...
```

Furthermore, the patterns in the `test/.gitignore` should be adjusted from `out/**` to `**/out/`,
the same for `diff`.

Observe the following:
- free standing tests are no longer allowed, they must be in a folder and be named `test.typ`
- tests can now be nested, their path serves as their name
- references, output and diff images now live directly next to the test script in their respective
  sub folders

You can copy the references into the sub folders, or simply regenerate them using the `update` sub
command.

If you used relative paths they must be adjusted, if you used absolute paths, then the tests should
continue to work as expected.
