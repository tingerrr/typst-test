# Grammar
The exact grammar can be read from the source code at [grammar.pest].
Because it is a functional language it consists only of expressions, no statements.

It supports
- groups for precedence (`(...)`),
- binary and unary operators (`and`, `not`, `!`, etc.),
- functions (`func(a, b, c)`),
- patterns (`r:^foo`),
- and basic data types like strings (`"..."`, `'...'`) and numbers (`1`, `1_000`).

# Operators
The following operators are available:

|Type|Prec.|Name|Symbols|Explanation|
|---|---|---|---|---|
|infix|1|union|<code>&vert;</code> , `or`|Includes all tests which are in either the left OR right test set expression.|
|infix|1|difference|`~`, `diff`|Includes all tests which are in the left but NOT in the right test set expression.|
|infix|2|intersection|`&`, `and`|Includes all tests which are in both the left AND right test set expression.|
|infix|3|symmetric difference|`^`, `xor`|Includes all tests which are in either the left OR right test set expression, but NOT in both.|
|prefix|4|complement|`!`, `not`|Includes all tests which are NOT in the test set expression.|

Be aware of precedence when combining different operators, higher precedence means operators bind more strongly, e.g. `not a and b` is `(not a) and b`, not `not (a and b)` because `not` has a higher precedence than `and`.
Binary operators are left associative, e.g. `a ~ b ~ c` is `(a ~ b) ~ c`, not `a ~ (b ~ c)`.
When in doubt, use parentheses to force the precedence of expressions.

[grammar]: https://github.com/tinger/typst-test/crates/typst-test/
