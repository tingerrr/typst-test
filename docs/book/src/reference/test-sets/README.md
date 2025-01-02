# Test Set Language
The test set language is an expression based language, top level expression can be built up form smaller expressions consisting of binary and unary operators and built-in functions.
They form sets which are used to specify which test should be selected for various operations.

See [Using Test Sets][guide]

## Grammar
The exact EBNF [^ebnf] can be read from the source code at [grammar.pest].

## Evaluation
Test set expressions restrict the set of all tests which are contained in the expression and are compiled to an AST which check against all tests.
A test set such as `!skip()` would be checked against each test that is found by reading its annotations and filtering all tests out which do have an ignored annotation.
While the order of some operations like union and intersection doesn't matter semantically, the left operand is checked first for those where short circuiting can be applied.
The expression `!skip() & regex:'complicated regex'` is more efficient than `regex:'complicated regex' & !skip()`, since it will avoid the regex check for skipped tests entirely.
This may change in the future if optimizations are added for test set expressions.

## Operators
Test set expressions can be composed using binary and unary operators.

|Type|Prec.|Name|Symbols|Explanation|
|---|---|---|---|---|
|infix|1|union|<code>&vert;</code> , `or`|Includes all tests which are in either the left OR right test set expression.|
|infix|1|difference|`~`, `diff`|Includes all tests which are in the left but NOT in the right test set expression.|
|infix|2|intersection|`&`, `and`|Includes all tests which are in both the left AND right test set expression.|
|infix|3|symmetric difference|`^`, `xor`|Includes all tests which are in either the left OR right test set expression, but NOT in both.|
|prefix|4|complement|`!`, `not`|Includes all tests which are NOT in the test set expression.|

Be aware of precedence when combining different operators, higher precedence means operators bind more strongly, e.g. `not a and b` is `(not a) and b`, not `not (a and b)` because `not` has a higher precedence than `and`.
Binary operators are left associative, e.g. `a ~ b ~ c` is `(a ~ b) ~ c`, not `a ~ (b ~ c)`.
When in doubt, use parenthesis to force precedence.


## Sections
- [Built-in Test Sets](built-in.md) lists built-in test sets and functions.

[^ebnf]: Extended Backus-Naur-Form
[guide]: ../../guides/test-sets.md
[grammar]: https://github.com/tinger/typst-test/crates/typst-test/
