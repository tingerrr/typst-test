# Using Test Sets

## Why Tests Sets
Many operations such as running, comparing, removing or updating tests all need to somehow select which tests to operate on.
To avoid having lots of hard-to-remember options, which may or may not interact well, `typst-test` offers an expression based set language which is used to select tests.
Instead of writing

```bash
tt run --regex 'foo-.*' --path '/bar/baz' --not --skip
```

`typst-test` can be invoked like

```bash
tt run --expression '(regex:foo-.* & !skip()) | path:bar/baz'
```

This comes with quite a few advantages:
- it's easier to compose multiple identifier filters like `regex` and `path`
- options are ambiguous whether they apply to the next option only or to all options like `--regex`
- with options it's unclear how to compose complex relations like `and` vs `or` of other options
- test set expressions are visually close to the filter expressions they describe, their operators are deiberately chosen to feel like witing a predicate which is applied over all tests

Let's first disect what this expression actually means:
`(regex:foo-.* & !skip()) | exact:bar/baz`

1. We have a top-level binary expression like so `a | b`, this is a union expression, it includes all tests found in either `a` or `b`.
1. The right expression is `exact:/bar/baz`, this is a pattern (indicated by the colon `:`).
   It matches all tests who's identifier exactly matches `bar/baz`, i.e. it includes this directory of tests and but not its sub tests.
1. The left expression is itself a binary expression again, this time an intersection.
   It consists of another pattern and a complement set.
   1. The pattern is a regex pattern and behaves like one would expect, it matches on the identifier/path with the given regular expression.
      It includes all tests who's module identifier matches the given regex.
   1. The complement `!skip()` includes all tests which are _not_ marked as skipped.

Tying it all together, we can describe what this expression matches in a sentence:

> Select all tests which are not marked as skip and match the regex `foo-.*`, additionally, include `bar/baz`, but not its sub tests.

Trying to describe this relationship using options on the command line would be cumbersome, error prone and, depending on the options present, impossible. [^ref]

## Default Test Sets
Many operations take either a set of tests as positional arguments, which are matched exactly, or a test set expression.
If neither are given the default test set is used, which is defined as `!skip()`.

<div class="warning">

This may change in the future, commands my get their own, or even configurable default test sets.
See [#40](https://github.com/tingerrr/typst-test/issues/40).

</div>

More concretely given the invocation

```bash
tt list test1 test2 ...
```

is equivalent to the following invocation

```bash
tt list --expression 'none() & (exact:test1 | exact:test2 | ...)'
```

## An Iterative Example
Suppose you had a project with the following tests:
```txt
mod/sub/foo ephemeral  ignored
mod/sub/bar ephemeral
mod/sub/baz persistent
mod/foo     persistent
bar         ephemeral
baz         persistent ignored
```

and you wanted run only ephemeral tests in `mod/sub`.
You could construct a expression with the following steps:

1. Firstly, filter out all ignored tests, `typst-test` does by default, but once we use our own expression we must include this restriction ourselves.
   - `!skip() & ...`
1. Now include only those tests which are ephemeral, to do this restriction we form the intersection of the current set and the ephemeral set.
   - `!skip() & ephemeral()`
1. Now finally, restrict it to be only tests which are in `mod/sub` or its sub modules.
   You can do so by adding any of the following patterns:
   - `!skip() & ephemeral() & contains~sub`
   - `!skip() & ephemeral() & path:mod/sub`
   - `!skip() & ephemeral() & regex:^mod/sub`

You can iteratively test your results with `typst-test list -e '...'` until you're satisfied.
Then you can run whatever operation you want with the same expression. If it is a destructive operation, i.e. one that writes changes to non-temporary files, then you must prefix the expression with `all:` if your test set contains more than one test.

## Patterns
Note that patterns come in two forms:
- raw patterns: They are provided for convenience, they have been used in the examples above and are simply the pattern kind followed by a colon and any non-whitespace characters.
- string patterns: A generalization which allows for whitespace and usage in nested expressions.

This distinction is useful for scripting and some interactive use cases, note that a raw pattern would keep parsing any non whitespace character.
When nesting patterns like `(regex:foo-.*) & ...` the parser would swallow the closing parenthesis as it is a valid character in many patterns.
String patterns explicitly wrap the pattern to avoid this: `(regex:"foo-.*") & ...` is valid and will parse correctly.

## Scripting
If you build up test set expressions programmatically, consider taking a look at the built-in test set functions.
Specifically the `all()` and `none()` sets can be used as identity sets for certain operators, possibly simplifying the code generating the test sets.

Some of the syntax used in test sets may interfere with your shell, especially the use of whitespace and special tokens within patterns like `$` in regexes.
Use non-interpreting quotes around the test set expression (commonly single quotes `'...'`) to avoid interpreting them as shell specific sequences.

[^ref]: To get a more complete look at test sets, take a look at the [reference](../reference/test-sets.md).
