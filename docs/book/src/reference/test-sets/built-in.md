# Built-in Test Sets
## Types
There are a few available types:
|Type|Explanation|
|---|---|
|`function`|Functions which evaluate to another type upon compilation.|
|`test set`|Represents a set of tests.|
|`number`|Positive whole numbers.|
|`string`|Used for patterns containing special characters.|
|`pattern`|Special syntax for test sets which operator on test identifiers.|

A test set expression must always evaluate to a test set, otherwise it is ill-formed, all operators operate on test sets only.
The following may be valid `set(1) & set("aaa", 2)`, but `set() & 1` is not.
There is no arithmetic, and at the time of writing this literals like numbers and strings are included for future test set functionality.

## Functions
The following functions are available, they can be written out in place of any expression.

|Name|Explanation|
|---|---|
|`none()`|Includes no tests.|
|`all()`|Includes all tests.|
|`skip()`|Includes tests with a skip annotation|
|`compile-only()`|Includes tests without references.|
|`ephemeral()`|Includes tests with ephemeral references.|
|`persistent()`|Includes tests with persistent references.|

## Patterns
Patterns are special types which are checked against identifiers and automatically turned into test sets.
A pattern starts with a pattern type before a colon `:` and is either followed by a raw pattern or a string literal.
Raw patterns don't have any delimiters and parse anything that's not whitespace.
String patterns are pattern prefixes directly followed by literal strings, they can be used to avoid parsing other tokens as part of a pattern, like when nesting pattern literals in expression groups or in function arguments.

The following pattern types exist:

|Type|Example|Explanation|
|---|---|---|
|`e`/`exact`|`exact:mod/name`|Matches by comparing the identifier exactly to the given term.|
|`c`/`contains`|`c:plot`|Matches by checking if the given term is contained in the identifier.|
|`r`/`regex`|`regex:mod-[234]/.*`|Matches using the given regex.|
|`g`/`glob`|`g:foo/**/bar`|Matches using the given glob battern.|
|`p`/`path`|`p:foo`|Matches using the given glob battern.|

