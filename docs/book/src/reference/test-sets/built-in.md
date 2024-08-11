# Built-in Test Sets
## Types
There are a few available types:
|Type|Explanation|
|---|---|
|`function`|Functions which evaluate to another type upon compilation.|
|`test set`|Represents a set of tests.|
|`integer`|Positive and negative whole numbers, used for.|
|`string`|Used for patterns containing special characters.|
|`pattern`|See below.|

A test set expression must always evaluate to a test set, otherwise it is ill-formed, all operators operate on test sets currently.
There is no arithmetic, and at the time of writing this literals like numbers, strings and patterns can only be used as function arguments.
The following may be valid `set(1) & set("aaa", 2)`, but `set & 1` is not.

## Constants
The following constants are available, they can be written out in place of any expression.

|Name|Explanation|
|---|---|
|`none`|Includes no tests.|
|`all`|Includes all tests.|
|`ignored`|Includes tests with an ignored annotation|
|`compile-only`|Includes tests without references.|
|`ephemeral`|Includes tests with ephemeral references.|
|`persistent`|Includes tests with persistent references.|
|`default`|A shorthand for `!ignored`, this is used as a default if no test set is passed.|

## Functions
The following functions operate on identifiers using patterns.

|Name|Example|Explanation|
|---|---|---|
|`id`|`id(=mod/name)`|Includes tests who's full identifier matches the pattern.|
|`mod`|`mod(:regex)`|Includes tests who's module matches the pattern.|
|`name`|`name(~foo)`|Includes tests who's name matches the pattern.|
|`custom`|`custom(#foo)`|Includes tests which have a `custom` annotation with a test set matching the given pattern.|

## Patterns
Patterns are special types which are checked against identifiers.
A pattern starts with a pattern prefix and is either followed by a raw pattern or a string.
Raw patterns don't have any delimiters and parse anything that's not whitespace, commas, or unmatched parentheses.
This means that `:hello(-world)?` is valid, but `#foo(` isn't.
There's never a reason to require unbalanced parenthesis, but it is possible using string patterns.
String patterns are pattern prefixes direclty followed by literal strings.
A string pattern could be used as an escape hatch for the above unbalanced parenthesis case: `#'foo\\('`, it requires two backslashes since they are used to escape special characters.

The following pattern prefixes exist:

|Prefix|Example|Explanation|
|---|---|---|
|`=`|`=mod/name`|Matches by comparing the identifier exactly to the given term.|
|`~`|`~plot`|Matches by checking if the given term is contained in the identifier.|
|`:`|`mod-[234]/.*`|Matches using the given regex, unbalanced parenthesis (even if escaped) need to be used in a string pattern.|
|`#`|`#foo/**/bar`|Matches using the given glob battern.|

