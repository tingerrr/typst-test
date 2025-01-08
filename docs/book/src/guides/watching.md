# Watching for Changes

`typst-test` does not currently support a "watch" command, which is common in this sort of tooling.
This is due to some of the complexity in how it uses the core Typst libraries.

However, you may workaround this by using [`watchexec`](https://watchexec.github.io/).
To begin, install it following the installation instructions in its [README](https://github.com/watchexec/watchexec).

Then, run a command like this in your package root directory, which is the same directory with your `typst.toml` and `README.md` file:

```bash
watchexec \
  --watch . \
  --clear \
  --ignore 'tests/**/diff/**' \
  --ignore 'tests/**/out/**' \
  "typst-test r"
```

This will run `typst-test r` whenever any file changes other than those in your tests' `{diff,out}` directories.

You may create an alias in your shell to make it more convenient:

```bash
alias ttw="watchexec --watch . --clear --ignore 'tests/**/diff/**' --ignore 'tests/**/out/**' 'typst-test r'"
```

Note, if your tests change any files in your package source tree, you may need to include them as additional `--ignore <glob>` patterns to the command.
