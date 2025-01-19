# Usage
`typst-test` is a command line program, it can be run by simply invoking it in your favorite shell and passing the appropriate arguments, the binary is called `tt`.

If you open a shell in the folder `project` and `typst-test` is at `project/bin/tt`, then you can run it using `./project/bin/tt`.
Placing it directly in your project is most likely not what you want to do.
You should install it to a directory which is contained in your `$PATH`, allowing you to simply run it using `tt` directly.
How to add such folders to your `PATH` depends on your operating system, but if you installed `typst-test` using one of the recommended methods in [Installation](./install.md), then such a folder should be chosen for you automatically.

`typst-test` will look for the project root by checking for directories containing a `typst.toml` manifest file.
This is because `typst-test` is primarily aimed at developers of packages.
If you want to use a different project root, or don't have a manifest file, you can provide the root directory using the `--root` like so.

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
version = "0.1.0"
authors = ["John Doe"]
license = "MIT"

entrypoint = "src/lib.typ"
```

Once you have a project root to work with you can run various commands like `tt add` or `tt run`.
Check out the [tests guide][guide] to find out how you can test your code.

[guide]: ./guides/tests.md
