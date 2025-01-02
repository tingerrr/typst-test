# Annotations
Tests may contain annotations at the start of the file.
These annotations are placed on the leading doc comment of the file itself.

```typst
/// [skip]
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
|`skip`|Takes not arguments, marks the test as part of the `skip` test set, can only be used once.|
