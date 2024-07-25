# Annotations
Tests may contain annotations at the start of the file.
These annotations are placed on the leading doc comment of the file itself.

```typst
/// [ignore]
/// [custom: foo]
///
/// Synopsis:
/// ...

#import "/src/internal.typ": foo
...
```

Annotations may only be placed at the start of the doc comment on individual lines without anything between them (no empty lines or other content).

The following annotations exist:

|Annotation|Description|
|---|---|
|`ignore`|Takes not arguments, marks the test as part of the `ignored` test set, can only be used once.|
|`custom`|Takes a single identifier as argument, marks the test as part of a custom test set of the given identifier, can be used multiple times.|

A test with an annotation like `[custom: foo]` can be selected with a test set like `custom(foo)`.
