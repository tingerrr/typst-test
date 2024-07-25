# Test Set Language
The test set language is an expression based language, top level expression can be built up form smaller expressions consisting of binary and unary operators and built-in functions and constants.

## Evaluation
Test set expressions restrict the set of all tests which are contained in the expression and are compiled to an AST which check against all tests.
A test set such as `!ignored` would be checked against each test that is found by reading its annotations and filtering all tests out which do have an ignored annotation.
While the order of some operations like union and intersection doesn't matter semantically, the left operand is checked first for those where short circuiting can be applied.
The expression `!ignored & id(/complicated regex/)` is more efficient than `id(/complicated regex/) & !ignored`, since it will avoid the regex check for ignored tests entirely.
This may change in the future if optimizations are added for test set expressions.

## Operators
Test set expressions can be composed using binary and unary operators.

|Type|Prec.|Name|Symbols|Explanation|
|---|---|---|---|---|
|infix|1|union|`∪`, <code>&vert;</code> , `+`, `or`|Includes all tests which are in either the left OR right test set expression.|
|infix|1|difference|`\`, `-` [^diff-lit]|Includes all tests which are in the left but NOT in the right test set expression.|
|infix|2|intersection|`∩`, `&`, `and`|Includes all tests which are in both the left AND right test set expression.|
|infix|3|symmetric difference|`Δ`, `^`, `xor`|Includes all tests which are in either the left OR right test set expression, but NOT in both.|
|prefix|4|complement|`¬`, `!`, `not`|Includes all tests which are NOT in the test set expression.|

Be aware of precedence when combining different operators, higher precedence means operators bind more strongly, e.g. `not a and b` is `(not a) and b`, not `not (a and b)` because `not` has a higher precedence than `and`.
Binary operators are left associative, e.g. `a - b - c` is `(a - b) - c`, not `a - (b - c)`.
When in doubt, use parenthesis to force precedence.


## Sections
- [Grammar](grammar.md) defines the formal grammar using EBNF [^ebnf].
- [Built-in Test Sets](built-in.md) lists built-in test sets and functions.

[^ebnf]: Extended Backus-Naur-Form
[^diff-lit]: There is currently no literal difference operator such as `and` or `not`.
