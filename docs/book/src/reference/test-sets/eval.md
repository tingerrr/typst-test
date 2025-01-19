# Evaluation
Test set expressions restrict the set of all tests which are contained in the expression and are compiled to an AST which is checked against all tests sequentially.
A test set such as `!skip()` would be checked against each test that is found by reading its annotations and filtering all tests out which do have an ignored annotation.
While the order of some operations like union and intersection doesn't matter semantically, the left operand is checked first for those where short circuiting can be applied.
As a consequence the expression `!skip() & regex:'complicated regex'` is more efficient than `regex:'complicated regex' & !skip()`, since it will avoid the regex check for skipped tests entirely, but this should not matter in practice.
