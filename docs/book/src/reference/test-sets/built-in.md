# Built-in Test Sets


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
The following functions operate on identifiers using matchers.

|Name|Example|Explanation|
|---|---|---|
|`id`|`id(=mod/name)`|Includes tests who's full identifier matches the pattern.|
|`mod`|`mod(/regex/)`|Includes tests who's module matches the pattern.|
|`name`|`name(~foo)`|Includes tests who's name matches the pattern.|
|`custom`|`custom(foo)`|Includes tests with have a `custom` annotation and the given identifier.|

## Matchers
Matchers are patterns which are checked against identifiers.

|Name|Example|Explanation|
|---|---|---|
|`=exact`|`=mod/name`|Matches by comparing the identifier exactly to the given term.|
|`~contains`|`~plot`|Matches by checking if the given term is contained in the identifier.|
|`/regex/`|`/mod-[234]\/.*/`|Matches using the given regex, literal slashes `/` and backslashes `\` must be escaped using a back slash `\\`.|

