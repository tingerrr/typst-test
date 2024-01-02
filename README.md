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
test is compiled using typst and the outputs in `out/<test>` and `ref/<test>` are compared. A diff
image is placed in `diff/<test>` for each mismatched page.

The name is currently a placeholder and the delegation of compilation to `typst` as a binary keeps
the project simple for now. This may change in the future.

## Motivation
After releasing a very broken version of [hydra], I started writing tests and as such also a small
script to run them automatically. I got a bit carried away and overengineered the test script, but
it had a fundamental flaw: It could not run tests in parallel. This and the additional burden of
maintaining the messy script was enough to prompt me to write this program. As this is direct port
of my [hydra test script][hydra-test], it already improves on various aspects, mainly speed.

## Goals
- primarily running visual regression tests
- easy test framework setup
- automatic test discovery
- simple user interface
- fast and clear feedback

Once the basic commands are implemented `typst-test` will switch to compiling the document itself if
this can further speed up the process and improve the error reporting.

I also plan to extract the inner logic into a library crate to allow running the same tests from
other programs like `typst-lsp`, as using the typst crate directly allows us to skip writing the
files to disk.

## Performance
Below is an illustration of the performance increase, most of this is from the fact that we skip
the nushell script and by extension being able to run tests in parallel. 
![
  An image showing shell session after running hyperfine with the script and binary, showing a
  ~8times speedup over the nushell + python script.
][benchmark]

[hydra]: https://github.com/tingerrr/hydra
[hydra-test]: https://github.com/tingerrr/hydra/blob/10127b1a5835a40a127b437b082c395a61d082d1/tests/run.nu
[benchmark]: assets/benchmark.png
