# Using Test Sets
## Why Tests Sets
Many operations such as running, comparing, removing or updating tests all need to somehow select which tests to operate on.
`typst-test` offers a functional set-based language which is used to select tests, it visually resembles writing a predicate which is applied to each test.

Test set expresisons are passed using the `--expression` or `-e` flag, they support the following features:
- binary and unary operators like `&`/`and` or `!`/`not`
- built-in primitive test sets such as `ephemeral()`, `compile-only()` or `skip()`
- identity test sets for easier scripting like `all()` and `none()`
- identifier pattern such as `regex:foo-\d{2}`

This allows you to concisely filter your test suite without having to remember a number of hard-to-compose CLI options. [^ref]

## An Iterative Example
Suppose you had a project with the following tests:
```txt
tests
├─ features
│  ├─ foo1      persistent   skipped
│  ├─ foo2      persistent
│  ├─ bar       ephemeral
│  └─ baz       compile-only
├─ regressions
│  ├─ issue-42  ephemeral    skipped
│  ├─ issue-33  persistent
│  └─ qux       compile-only
└─ frobnicate   compile-only
```

You can use `tt list` to ensure your test set expression is correct before running or updating tests.
This is not just faster, it also saves you the headache of losing a test you accidentally deleted.

If you just run `tt list` without any expression it'll use `all()` and you should see:
```txt
features/foo2
features/bar
features/baz
regressions/issue-33
regressions/qux
frobnicate
```

You may notice that we're missing two tests, those marked as `skipped` above:
- `features/foo1`
- `regressions/issue-42`

If you want to refer to these skipped tests, then you need to pass the `--no-implicit-skip` flag, otherwise the expression is wrapped in `(...) ~ skip()` by default.
If you pass tests by name explicitly like `tt list features/foo1 regressions/issue-42`, then this flag is implied.

Let's say you want to run all tests, which are either ephemeral or persistent, i.e. those which aren't compile-only, then you can use either `ephemeral() | persistent()` or `not compile-only()`.
Because there are only these three kinds at the moment those are equivalent.

If you run
```shell
tt list -e 'not compile-only()'
```
you should see
```txt
features/foo1
features/foo2
features/bar
regressions/issue-42
regressions/issue-33
```

The you can simply run `tt run` with the same expression and it will run only those tests.

If you want to incldue or exclude various directories or tests by identifier you can use patterns.
Let's you want to only run feature tests, you can a pattern like `c:features` or more correctly `r:^features`.

If you run
```shell
tt list -e 'r:^features'
```
you should see
```txt
features/foo1
features/foo2
features/bar
features/baz
```

Any combination using the various operators also works.
If you wanted to only compile those tests which are both in `features` and are not `compile-only`, then you would combine them with an intersection, i.e the `and`/`&` operator.

If you run
```shell
tt list -e 'not compile-only() and r:^features'
```
you should see
```txt
features/baz
```

If you wanted to include all tests which are either you'd use the union instead:

If you run
```shell
tt list -e 'not compile-only() or r:^features'
```
you should see
```txt
features/foo1
features/foo2
features/bar
features/baz
regressions/qux
frobnicate
```

If you update or remove tests and the test set evaluates to more than one test, then you must either specify the `all:` prefix in the test set expression, or confirm the operation in a terminal prompt.

## Patterns
Note that patterns come in two forms:
- raw patterns: They are provided for convenience, they have been used in the examples above and are simply the pattern kind followed by a colon and any non-whitespace characters.
- string patterns: A generalization which allows for whitespace and usage in nested expressions.

This distinction is useful for scripting and some interactive use cases.
For example, a raw pattern would keep parsing any non whitespace character, when nesting patterns like `(... | regex:foo-.*) & ...` the parser would therefor swallow the closing parenthesis and not close the group.
String patterns have delimiters with which this can be avoided: `(... | regex:"foo-.*") & ...` will parse correctly and close the group before the `&`.

## Scripting
If you build up test set expressions programmatically, consider taking a look at the built-in test set functions.
Specifically the `all()` and `none()` test set constructors can be used as identity sets for certain operators, possibly simplifying the code generating the test sets.

Some of the syntax used in test sets may interfere with your shell, especially the use of whitespace and special tokens within patterns like `$` in regexes.
Use non-interpreting quotes around the test set expression (commonly single quotes `'...'`) to avoid interpreting them as shell specific sequences.

This should give you a rough overview of how test sets work, you can check out the [reference] to learn which operators, patterns and test sets exist.

[reference]: ../reference/test-sets/index.html
