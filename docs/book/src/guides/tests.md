# Writing tests
To start writing tests, you only need to write regular `typst` scripts, no special syntax or annotations are required.

Let's start with the most common type of tests, regression tests. 
We'll assume you have a normal package directory structure:
```txt
<project>
├─ src
│  └─ lib.typ
└─ typst.toml
```

## Regression tests
Regression tests are found in the `test` directory of your project (remember that this is where your `typst.toml` manifest is found).

Let's write our first test, you can run `tt add my-test` to add a new regression test, this creates a new directory called `my-test` inside `tests` and adds a test script and reference document.
This test is located in `tests/my-test/tests.typ` and is the entrypoint script (like a `main.typ` file).
Assuming you passed no extra options to `tt add`, this test is going to be a `persistent` regression test, this means that its output will be compared to a reference document which is stored in `tests/my-test/ref/` as individual pages.

You could also pass `--ephemeral`, which means to create a script which creates this document on every test run or `--compile-only`, which means the test doesn't create any output and is only compiled.

Your project will now look like this:
```txt
<project>
├─ src
│  └─ lib.typ
├─ tests
│  └─ my-test
│     ├─ ref
│     │  └─ 1.png
│     └─ test.typ
└─ typst.toml
```

If you now run
```shell
tt run my-test
```
you should see something along the lines of
```txt
  Starting 1 tests (run ID: 4863ce3b-70ea-4aea-9151-b83e25f11c94)
      pass [ 0s  38ms 413µs] my-test
──────────
   Summary [ 0s  38ms 494µs] 1/1 tests run: all 1 passed
```

This means that the test was run successfully.

Let's edit the test to actually do something, right now it simply contains `Hello World`.
Write something else in there and see what happens:
```diff
-Hello World
+Typst is Great!
```

Once we run `typst-test` again we'll see that the test no longer passes:

```txt
  Starting 1 tests (run ID: 7cae75f3-3cc3-4770-8e3a-cb87dd6971cf)
      fail [ 0s  44ms 631µs] my-test
           Page 1 had 1292 deviations
           hint: Diff images have been saved at '<project>/test/tests/my-test/diff'
──────────
   Summary [ 0s  44ms 762µs] 1/1 tests run: all 1 failed
```

`typst-test` has compared the reference output from the original `Hello World` document to the new document and determined that they don't match.
It also told you where you can inspect the difference, the `<project>/tests/my-test` contains a `diff` directory.
You can take a look to see what changed, you can also take a look at the `out` and `ref` directories, these contain the output of the current test and the expected reference output respectively.

Well, but this wasn't a mistake, this was a deliberate change.
So, let's update the references to reflect that and try again.
For this we use the appropriately named `update` command:

```bash
tt update my-test
```

You should see output similar to

```txt
  Starting 1 tests (run ID: f11413cf-3f7f-4e02-8269-ad9023dbefab)
      pass [ 0s  51ms 550µs] my-test
──────────
   Summary [ 0s  51ms 652µs] 1/1 tests run: all 1 passed
```

and the test should once again pass.

<div class="warning">

Note that, at the moment `typst-test` does not compress the reference images, this means that, if you use a version control system like git or mericural, the reference images of persistent tests can quickly bloat your repository, if you update them frequently.
Consider using a program like [`oxipng`][oxipng] to compress them, `typst-test` can still read them without any problems.

`typst-test` will warn you when you update your images for now, in the future it will compress images automatically if they are created as persistent references, see [#72].

</div>

This test is still somewhat arcane, let's actually test something interesting, like the API of your fancy package.

Let's say you have this function inside your `src/lib.typ` file:

```typst
/// Frobnicates a value.
///
/// -> content
#let frobnicate(
  /// The argument to frobnicate, cannot be `none`.
  ///
  /// -> any
  arg
) = {
  assert.ne(type(arg), type(none), message: "Cannot frobnicate `none`!")

  [Frobnicating #arg]
}
```

Because `typst-test` comes with a custom standard library you can catch panics and extract their messages to ensure your code also works in the failure path.

Let's add another test where we check that this function behaves correctly and let's not return any output but instead just check how it behaves with various inputs:

```shell
tt add --compile-only frobnicate
```

You project should now look like this:
```txt
<project>
├─ src
│  └─ lib.typ
├─ tests
│  ├─ my-test
│  │  ├─ ref
│  │  │  └─ 1.png
│  │  └─ test.typ
│  └─ frobnicate
│     └─ test.typ
└─ typst.toml
```

Note that the `frobnicate` test does not contain any other directories for references.
Because this test is within the project root it can access the projects internal files, even if they aren't reachable from the package entrypoint.

Let's import our function and test it:
```typst
#import "/src/lib.typ": frobnicate

// Passing `auto` should work:
#frobnicate(auto)

// We could even compare it:
#assert.eq(frobnicate("Strings work!"), [Frobnicate #"Strings work!"])
#assert.eq(frobnicate[Content works!], [Frobnicate Content works!])

// If we pass `none`, then this must panic, otherwise we did something wrong.
#assert-panic(() => frobnicate(none))

// We can also unwrap the panics and inspect their eror message.
// Note that we get an array of strings back if a panic occured, or `none` if
// there was no panic.
#assert.eq(
  catch(() => frobnicate(none)),
  "panicked with: Cannot frobnicate `none`!",
)
```

<div class="warning">

The exact interface of this library may change in the future.

See [#73].

</div>

The more your project grows

## Template tests

<div class="warning">

In the future you'll be able to automatically test your templates too, but these are currently unsupported

See [#49].

</div>

## Documentation tests

<div class="warning">

In the future you'll be able to automatically test your documentation examples too, but these are currently unsupported

See [#34].

</div>

This should equip you with all the knowledge of how to reliably test your projects, but if you're still curious about all the details check out the [reference for tests][tests].

[#72]: https://github.com/tingerrr/typst-test/issues/72
[#73]: https://github.com/tingerrr/typst-test/issues/73
[#49]: https://github.com/tingerrr/typst-test/issues/49
[#34]: https://github.com/tingerrr/typst-test/issues/34
[tests]: ../reference/tests/index.html
[oxipng]: https://github.com/shssoichiro/oxipng
