# typst test
Typst test is a program to compile, compare and update tests scripts for typst. It is currently
work in progress and is aimed at providing automated visual regression testing for typst packages.

You can test it now by running `cargo run -- --root <project>`, or by running it within `<project>`,
where `<project>` is a directory with the following properties:
- contains a `typst.toml` manifest
- contains a `tests/typ` folder

For a test directory is like so:
```
<project>/tests
  typ/
    test1.typ
    test2/test.typ
  ref/
    test1/
      1.png
      2.png
    test2/
      1.png
```
the tests `test1` and `test2/test.typ` are run respectively and in parallel, once a test is started
a `out/<test>`, `ref/<test>` and `diff/<test>` are created if they don't already exist. Then the
test is compiled using typst and the outputs in `out/<test>` and `ref/<test>` are compared.

The name is currently a placeholder and the delegation of compilation to `typst` as a binary keeps
the project simple for now. This may change in the future.
