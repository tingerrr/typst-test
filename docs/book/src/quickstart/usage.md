# Usage
`typst-test` is a command line program, it can be run by simply invoking it in your favorite shell and passing the appropriate arguments.

If you open a shell in the folder `project` and `typst-test` is at `project/bin/typst-test`, then you can run it using `./project/bin/typst-test`.
Placing it directly in your project is most likely not what you will do, or want to do.
You should install it to a directory which is contained in your `PATH`, allowing you to simply run it using `typst-test` directly.
How to add such folders to your `PATH` depends on your operating system, but if you installed `typst-test` using one of the recommended methods in [Installation](.install.md), then such a folder should be chosen for you.

<div class="warning">

For the remainder of this document `tt` is used in favor of `typst-test` whenever a command line example is shown.
When you see an example such as
```bash
tt run -e 'ephemeral() | compile-only()'
```
it is meant to be run as
```bash
typst-test run -e 'ephemeral() | compile-only()'
```

You can also define an alias of the same name to make typing it easier.

</div>

`typst-test` requires a certain project structure to work, if you want to start testing your project's code, you can create an example test and the required directory structure using the `init` command.

```bash
tt init
```

This will create the default example to give you a grasp at where tests are located, and how they are structured.
`typs-test` will look for the project root by checking for directories containing a `typst.toml` manifest file.
This is because `typst-test` is primarily aimed at developers of packages, if you want to use a different project root, or don't have a manifest file, you can provide the root directory using the `--root` like so.

```bash
tt init --root ./path/to/root/
```

Keep in mind that you must pass this option to every command that operates on a project.
Alternatively the `TYPST_ROOT` environment variable can be set to the project root.

Further examples assume the existence of a manifest, or the `TYPST_ROOT` variable being set
If you're just following along and don't have a package to test this with, you can use an empty project with the following manifest:

```toml
[package]
name = "foo"
description = "A fancy Typst package!"
authors = ["John Doe"]
license = "MIT"

entrypoint = "src/lib.typ"
version = "0.1.0"
```

Once the project is initialized, you can run the example test to see that everything works.

```bash
tt run example
```

You should see something along the lines of

```txt
  Starting 1 tests (run ID: 4863ce3b-70ea-4aea-9151-b83e25f11c94)
      pass [ 0s  38ms 413µs] example
──────────
   Summary [ 0s  38ms 494µs] 1/1 tests run: all 1 passed
```

Let's edit the test to actually do something, the default example test can be found in `<project>/tests/example/` and simply contains `Hello World`.
Write something else in there and see what happens
```diff
-Hello World
+Typst is Great!
```

Once we run `typst-test` again we'll see that the test no longer passes:

```txt
  Starting 1 tests (run ID: 7cae75f3-3cc3-4770-8e3a-cb87dd6971cf)
      fail [ 0s  44ms 631µs] example
           Page 1 had 1292 deviations
           hint: Diff images have been saved at '/home/tinger/test/tests/example/diff'
──────────
   Summary [ 0s  44ms 762µs] 1/1 tests run: all 1 failed
```

`typst-test` has compared the reference output from the original `Hello World` document to the new document and determined that they don't match.
It also told you where you can inspect the difference, the `<project>/test/example` contains a `diff` directory.
You can take a look to see what changed, you can also take a look at the `out` and `ref` directories, these contain the output of the current test and the expected reference output respectively.

Well, but this wasn't a mistake, this was a deliberate change.
So, let's update the references to reflect that and try again.
For this we use the appropriately named `update` command:

```bash
tt update example
```

You should see output similar to

```txt
  Starting 1 tests (run ID: f11413cf-3f7f-4e02-8269-ad9023dbefab)
      pass [ 0s  51ms 550µs] example
──────────
   Summary [ 0s  51ms 652µs] 1/1 tests run: all 1 passed
```

and the test should once again pass.

<div class="warning">

Note that, at the moment typst-test does not compress the reference images, this means that, if you use a version control system like git or mericural, the reference images of persistent tests can quickly bloat your repository if you update them frequently.
Consider using a program like [`oxipng`][oxipng] to compress them, `typst-test` can still read them without any problems.

</div>

[oxipng]: https://github.com/shssoichiro/oxipng
