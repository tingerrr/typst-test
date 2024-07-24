# Using Test Sets

## Why Tests Sets
Many operations such as running, comparing, removing or updating tests all need to somehow select which tests to operate on.
To avoid having lots of hard-to-remember options, which may or may not interact well, `typst-test` offers an expression based set language which is used to select tests.
Instead of writing

```bash
tt run --regex --mod 'foo-.*' --name 'bar/baz' --no-ignored
```

`typst-test` can be invoked like

```bash
tt run --expression '(mod(/foo-.*/) & !ignored) | name(=bar/baz)'
```

This comes with quite a few advantages:
- it's easier to compose multiple identifier filters like `mod` and `name`
- options are ambiguous whether they apply to the next option only or to all options like `--regex`
- with options it's unclear how to compose complex relations like `and` vs `or` of other options
- test set expressions are visually close to the filter expressions they describe, their operators are deiberately chosen to feel like witing a predicate which is applied over all tests

Let's first disect what this expression actually means:
`(mod(/foo-.*/) & !ignored) | id(=bar/baz)`

1. We have a top-level binary expression like so `a | b`, this is a union expression, it includes all tests found in either `a` or `b`.
1. The right expression is `id(=bar/baz)`, this includes all tests who's full identifier matches the given pattern `=bar/baz`.
   That's an exact matcher (indicated by `=`) for the test identifier `bar/baz`.
   This means that whatever is on the left of your union, we also include the test `bar/baz`.
1. The left expression is itself a binary expression again, this time an intersection.
   It consists of another matcher test set and a complement.
   1. The name matcher is only applied to modules this time, indiicated by `mod` and uses a regex matcher (delimited by `/`).
      It includes all tests who's module identifier matches the given regex.
   1. The complement `!ignored` includes all tests which are not marked as ignored.

Tying it all together, we can describe what this expression matches in a sentence:

> Select all tests which are not marked ignore and are inside a module starting with `foo-`, include also the test `bar/baz`.

Trying to describe this relationship using options on the command line would be cumbersome, error prone and, depending on the options present, impossible.

## Default Test Sets
Many operations take either a set of tests as positional arguments, which are matched exactly, or a test set expression.
If neither are given the `default` test set is used, which is itself a shorthand for `!ignored`.

<div class="warning">

This may change in the future, commands my get their own, or even configurable default test sets.
See [#40](https://github.com/tingerrr/typst-test/issues/40).

</div>

More concretely given the invocation
```bash
tt list test1 test2 ...
```

is equivalent to the following invocation

```txt
tt list --expression 'default & (id(=test1) | id(=test2) | ...)'
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
   Both of the following would work.
   - `default & ...`
   - `!ignored & ...`
   Let's go with `default ` to keep it simple.
1. Now include only those tests which are ephemeral, to do this, add the ephemeral test set.
   - `default & ephemeral`
1. Now finally, restrict it to be only tests which are in `mod/sub` or it's sub modules.
   You can do so by adding any of the following identifier matchers:
   - `default & ephemeral & mod(~sub)`
   - `default & ephemeral & mod(=mod/sub)`
   - `default & ephemeral & id(/^mod\/sub/)`

You can iteratively test your results with `typst-test list -e '...'` until you're satisfied.
Then you can run whatever operation you want with the same expression. IF it is a destructive operation, i.e. one that writes chaanges to non-temporary files, then you must also pass `--all` if your test set contains more than one test.

## Scripting
If you build up test set expressions programmatically, consider taking a look at the built-in test set constants.
Specifically the `all` and `none` test sets can be used as identity sets for certain operators, possibly simplifying the code generating the test sets.

Some of the syntax used in test sets may interfere with your shell, especially the use of whitespace.
Use non-interpreting quotes around the test set expression (commonly single quotes `'...'`) to avoid interpreting them as shell specific sequences.
