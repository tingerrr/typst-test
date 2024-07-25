# Grammar
The test set expression entrypoint rule is the `main` node.
The nodes `SOI` and `EOI` stand for start of input and end of input respectively.

```ebnf
main ::=
  SOI
  WHITESPACE*
  expr
  WHITESPACE*
  EOI
  ;

expr ::=
  unary_operator*
  term
  (
    WHITESPACE*
    binary_operator
    WHITESPACE*
    unary_operator*
    term
  )*
  ;

atom ::= func | val ;
term ::= atom | "(" expr ")" ;

unary_operator ::= complement ;

complement ::= "¬" | "!" | "not" ;

binary_operator ::=
  intersect
  | union
  | difference
  | symmetric_difference
  ;

intersect ::= "∩" | "&" | "and" ;
union ::= "∪" | "|" | "or" | "+" ;
difference ::= "\\" | "-" ;
symmetric_difference ::= "Δ" | "^" | "xor" ;

val ::= id ;
func ::= id args ;

args ::= "(" arg ")" ;
arg ::= matcher ;

matcher =
  exact_matcher
  | contains_matcher
  | regex_matcher
  | plain_matcher
  ;

exact_matcher ::= "=" name
contains_matcher ::= "~" name
regex_matcher ::= "/" regex "/"
plain_matcher ::= name


id ::=
  ASCII_ALPHA
  (ASCII_ALPHANUMERIC | "-" | "_")*
  ;

name ::=
  ASCII_ALPHA
  (ASCII_ALPHANUMERIC | "-" | "_" | "/")*
  ;

regex ::=
  (
    ( "\\" "/")
    | (!"/" ANY)
  )+
  ;

WHITESPACE ::= " " | "\t" | "\r" | "\n" ;
```
