# Annotations
Tests may contain annotations which are used for configuring the test runner for each test.
These annotations are placed on a leading doc comment at the start of the test script, i.e. they must be before any content or imports.
The doc comment may contain any content after the annotations, any empty lines are ignored.

For ephemeral regression tests only the main test file will be checked for annotations, the reference file will be ignored.

<div class="warning">

The syntax for annotations may change if typst adds first class annotation or documentation comment syntax.

</div>

```typst
/// [skip]
///
/// Synopsis:
/// ...

#import "/src/internal.typ": foo
...
```

The following annotations are available:

|Annotation|Description|
|---|---|
|`skip`|Marks the test as part of the `skip()` test set.|
