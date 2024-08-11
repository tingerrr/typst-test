# Grammar
The test set expression entrypoint rule is the `main` node.
The nodes `SOI` and `EOI` stand for start of input and end of input respectively.

```ebnf
main ::=
  SOI
  , { WHITESPACE }
  , expr
  , { WHITESPACE }
  , EOI
  ;

expr ::=
  { prefix_operator }
  , term
  , (
    { WHITESPACE }
    , infix_operator
    , { WHITESPACE }
    , { prefix_operator }
    , term
  )*
  ;

atom ::= lit | func | val ;
term ::= atom | "(" , expr , ")" ;

prefix_operator ::= complement ;

complement ::= "¬" | "!" | "not" ;

infix_operator ::=
  intersection
  | union
  | difference
  | symmetric_difference
  ;

intersection ::= "∩" | "&" | "and" ;
union ::= "∪" | "|" | "or" | "+" ;
difference ::= "\\" | "-" ;
symmetric_difference ::= "Δ" | "^" | "xor" ;

lit ::= string | num | pattern ;
val ::= id ;
func ::= id , args ;

args ::= "(" , [ expr ] , { "," , expr } , "," , ")" ;

string ::=
  ('"' , { ? ANY ? - '"' | ( "\\" , '"' ) | string_escape } , '"')
  | ("'" , { ? ANY ? - "'" | ( "\\" , "'" ) | string_escape } , "''")
  ;
string_escape ::= "\\" , "u" "{", 4 * ? ASCII_HEX_DIGIT ? , "}" ;

num ::= ? '1'..'9' ? { ? '0'..'9' ? } ;

pattern = pattern_prefix , ( string | raw_pattern ) ;
pattern_prefix ::= "=" | "~" | ":" | "#" ;
raw_pattern ::=
  raw_pattern_segment
  | "(" , raw_pattern , ")"
  , { raw_pattern_segment | "(" , raw_pattern , ")" }
  ;
raw_pattern_segment ::= ANY - ( WHITESPACE | "," | "(" | ")" ) ;

id ::= id_segment , { "/" , id_segment } ;
id_segment ::=
  ASCII_ALPHA
  , { ASCII_ALPHANUMERIC | "-" | "_" }
  ;

WHITESPACE ::= " " | "\t" | "\r" | "\n" ;
```

This grummar is aintained by hand and thus, may not be entirely accurate.
It is also intentionally simplified to be easier to follow.
