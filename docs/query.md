# Queries

A Query selects Records of DataForge and values inside them. It is run by
`p4kx dcb query <archive> "<query>"` and by the `Query` API of `p4kx-dataforge`. The syntax is
SQL-like ([ADR-0009](adr/0009-sql-like-query-syntax.md)).

## Examples

```text
from EntityClassDefinition
at "libs/foundry/records/entities/spaceships/*"
from EntityClassDefinition at "*/spaceships/*" select Category, Invisible
at "*/spaceships/*" select Icon
from AmmoParams at "*/vehicle/*" select speed, lifetime where speed >= 1500 and lifetime <= 3
from EntityClassDefinition at "*/spaceships/*" select Category, Invisible where Category = "Default" and not Invisible = true
at "*/spaceships/*" select Icon, Category where Icon like "default.*" and (Category contains "def" or Invisible = true)
from EntityClassDefinition at "libs/foundry/records/entities/spaceships/*" select Components[VehicleComponentParams].vehicleName, Components[VehicleComponentParams].crewSize where Components[VehicleComponentParams].crewSize >= 4
from EntityClassDefinition at "*/ships/weapons/*" select Components[SAttachableComponentParams].AttachDef.Size, Components[SAttachableComponentParams].AttachDef.Type where Components[SAttachableComponentParams].AttachDef.Size >= 7
from EntityClassDefinition at "*/ships/weapons/*" select Components[SAttachableComponentParams].AttachDef.Manufacturer->Code, Components[SAttachableComponentParams].AttachDef.Manufacturer->Localization.Name where Components[SAttachableComponentParams].AttachDef.Manufacturer->Code = "BEH"
from EntityClassDefinition at "*/ships/weapons/*" referencing "libs/foundry/records/scitemmanufacturer/personalweapons/scitemmanufacturer.behr" select Components[SAttachableComponentParams].AttachDef.Size
referencing "65a5d887-3b21-4046-a718-6912c0c7c3be" select Components[SAttachableComponentParams].AttachDef.Type where Components[SAttachableComponentParams].AttachDef.Size >= 7
```

- `from EntityClassDefinition` lists every Record of type `EntityClassDefinition` or of a struct
  deriving from it.
- `at "…"` lists the Records whose record path matches.
- `select Category, Invisible` adds one column per dot path behind the record path.
- `where speed >= 1500 and lifetime <= 3` keeps only the Records whose values meet the condition.
- `Components[VehicleComponentParams].crewSize` reads the field `crewSize` of the components of
  type `VehicleComponentParams`, wherever they stand in the array `Components`
  ([Paths into nested data](#paths-into-nested-data)).
- `Manufacturer->Code` follows the Reference `Manufacturer` to the Record it links to and reads
  its field `Code` ([Following References](#following-references)).
- `referencing "<record path or GUID>"` selects the Records that hold a Reference to the Record at
  that record path, or with that GUID, e.g. the weapons of one manufacturer
  ([Records that refer to a Record](#records-that-refer-to-a-record)).

The opt-in tests against a real Archive run these examples.

## Result

The result is a table of rows and columns, written as a table, CSV or JSON (see
[Output formats](#output-formats)). Its first column is always the record path, headed
`record path`; then follows one column per path of `select`, in the order written, headed by the
path as the canonical form prints it.

There is one row per selected Record, in Record order ([CONTEXT.md](../CONTEXT.md)): by record
path ignoring ASCII case with `\` and `/` alike, then by Record name ignoring ASCII case, then by
the Record's position in the DataForge. Records share record paths, so a
record path can appear in several rows.

A cell holds the one value its path reaches, or for a path through `[*]` or `[TypeName]` a list
of every value it reaches ([Several values](#several-values)). A null pointer, an index out of
range or a null or dangling Reference on the way gives `null`. Without `from`, a Record whose type does not resolve the path,
e.g. has no field of the first name, gives an empty cell; with `from`, every selected type
declares the fields of the named type. A value that cannot be read shows its reason as
`error: …`. A Record whose layout cannot be read gives no row and is
reported as failed; with `where`, so is a Record whose values cannot be read to decide the
filter.

## Output formats

`p4kx dcb query <archive> "<query>" [--format table|csv|json] [--csv-raw] [--out <file>]` writes
the result in one of three formats; the default is `table`, or `json` with `--json`. All three have the
same columns and rows in the same order. No heading of a `select` path is `record path`,
because a name with a space is written in backticks.

The result is written page by page: the rows of one page are built in parallel and written
before the next page is built, so the result is never held in memory as a whole. With `--out`
it goes to a temporary file next to the target, which is renamed to the target once complete
and replaces an existing file; stdout then gets one summary line such as
`3 Records written to result.csv`, followed by `, 2 unresolved References` when `->` met
References that link to no Record ([Following References](#following-references)), or a JSON summary with `--json` (see
[cli-json.md](cli-json.md)). A Record whose layout cannot be read gives no row in any format;
it is reported on stderr and the exit code is 1. An `--out` that is the Archive itself is
rejected with exit code 2, and so is `--json` with another format and without `--out`. A run
that is killed, e.g. by Ctrl+C, can leave the temporary file `<file>.p4kx-<process id>.part`
behind; the target itself is never half written.

### Table

Columns separated by two spaces and padded to the widest text of the column over all rows, a
line of `-` under the headings, and on stdout a last line with the number of Records and, when
there are any, of unresolved References, e.g. `3 Records, 2 unresolved References`. Line
breaks and tabs in a value are shown as `\n`, `\r` and `\t`, other control characters as
U+FFFD. The widths are measured in a first pass over the pages, so the Records are read twice.
A list ([Several values](#several-values)) is written as its values in brackets separated by
`, `, e.g. `[250.5, 1200]`; a list of more than 8 values shows the first 8 and how many more
there are, e.g. `[t1, t2, t3, t4, t5, t6, t7, t8, … 4 more]`. The table is for reading; scripts
use CSV or JSON.

### CSV

CSV after [RFC 4180](https://www.rfc-editor.org/rfc/rfc4180):

- UTF-8 with a byte order mark, so that a spreadsheet reads the encoding right; every row, the
  header row included, ends with CRLF.
- The header row holds `record path` and then each `select` path in canonical form.
- One row per Record with exactly one field per column.
- A field holding `,`, `"`, CR or LF is enclosed in `"`, and each `"` in it is doubled; every
  other field is written as it is. A line break inside a value stays inside the quoted field.
- A value is written in its Record XML text ([record-xml.md](record-xml.md#values)): `true`,
  `25000.5`, `NaN`, the enum option, the GUID in lower case. A value that cannot be read is
  `error: <reason>`. An empty cell (a Record without the field) is an empty field, as is an
  empty string. A list ([Several values](#several-values)) is written as the JSON array the
  JSON format writes for it, in a quoted field, e.g. `"[""fast"",""small""]"` or `[250.5,1200]`,
  so that a value holding `,` or `]` stays unambiguous.
- A text cell (a string, locale or enum value, or a heading) that starts with `=`, `+`, `-`, `@`,
  a tab or a carriage return gets a leading `'`, so that a spreadsheet does not run it as a
  formula ([ADR-0112](adr/0112-the-flat-value-export-is-csv-and-ndjson-and-offers-no-sqlite-output.md),
  [ADR-0159](adr/0159-every-csv-writer-follows-the-flat-export-rules-and-uses-one-cell-writer.md)).
  Numbers, booleans and GUIDs are never changed, so `-0.25` stays a number.
- `--csv-raw` (with `--format csv`) writes the exact values: no byte order mark and no leading
  `'`. Use it for scripts that read the file. `p4kx dcb fields` follows the same rules;
  `p4kx loc export` too, except that its file always starts with the byte order mark.

```text
record path,name,crew,mass
libs/items/box,Box,,-0.25
libs/ships/avenger,"Avenger, ""Titan""",2,25000.5
```

### JSON

One line of compact JSON ending in a line feed: an array with one object per row, in row
order. Each object has the key `record path` and one key per `select` path, in column order; a
path selected twice appears once, as its values are the same. The values are typed:

| Field type | JSON |
|------------|------|
| bool | `true` or `false` |
| int8 … int64, uint8 … uint64 | A number with every digit; a parser that reads numbers as doubles, such as JavaScript, rounds beyond 2^53 |
| float, double | A number in its Record XML text (`25000.5`, `1e21`, `-0`); `NaN`, `INF` and `-INF` as strings |
| string, localisation key, enum | A string: the text, the key as stored, the option name |
| GUID | A string, lower case, `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` |
| a value that cannot be read | An object `{"error": "<reason>"}` |
| a Reference to a Record that does not exist | An object `{"error": "<reason>", "guid": "<guid>"}` with the GUID of the missing Record |
| a list of a path through `[*]` or `[TypeName]` | An array of the values, each typed by this table, `null` for a null; `[]` when there is none |
| an empty cell | `null` |

Strings escape `"` and `\`, line feed, carriage return and tab as `\n`, `\r` and `\t`, and the
other control characters below U+0020 as `\u00XX`; every other character is written as UTF-8.

```json
[{"record path":"libs/items/box","name":"Box","crew":null,"mass":-0.25},{"record path":"libs/ships/avenger","name":"Avenger, \"Titan\"","crew":2,"mass":25000.5}]
```

## Grammar

```text
query      = selection [ "select" path { "," path } ] [ "where" condition ] ;
selection  = "from" name [ "at" string ] [ "referencing" string ]
           | "at" string [ "referencing" string ]
           | "referencing" string ;
path       = name { segment } ;
segment    = "." name
           | "[" index "]"
           | "[" "*" "]"
           | "[" name "]"
           | "->" name ;
index      = digit { digit } ;
condition  = term { "or" term } ;
term       = factor { "and" factor } ;
factor     = "not" factor
           | "(" condition ")"
           | path compare value
           | path match string ;
compare    = "=" | "!=" | "<" | "<=" | ">" | ">=" ;
match      = "contains" | "starts" "with" | "like" ;
value      = number | string | "true" | "false" ;
number     = [ "-" ] digit { digit } [ "." digit { digit } ]
             [ ( "e" | "E" ) [ "+" | "-" ] digit { digit } ] ;
name       = bare-name | quoted-name ;
bare-name  = ( letter | "_" ) { letter | digit | "_" } ;    (not a keyword)
quoted-name = "`" char-but-backtick { char-but-backtick } "`" ;
string     = '"' { char-but-quote | '""' } '"' ;
```

- `letter` is an ASCII letter and `digit` an ASCII digit.
- Whitespace (any Unicode whitespace) may stand between any two tokens and is ignored.
- The clauses come in the order `from`, `at`, `referencing`, `select`, `where`; each appears at
  most once, and at least one of `from`, `at` and `referencing` is given.
- A Query is at most 65,536 bytes long.
- A path has at most 64 segments behind its first name, and at most 16 of them are `->`
  (`MAX_PATH_REFERENCES`).
- An index is a whole number from 0 to 4,294,967,295 without sign, fraction or exponent;
  leading zeros are allowed, so `[007]` is `[7]`.

### Keywords

`from`, `at`, `referencing`, `select`, `where`, `and`, `or` and `not` are keywords; they ignore case, so
`FROM` and `From` are `from`. A type or field whose name is a keyword, or is no bare name, is
written in backticks: `` `select` ``, `` `Size-X` ``, e.g. ``from Tag select `select` ``. A quoted
name holds any characters but a backtick and is never empty.

The word operators `contains`, `starts with` and `like` and the values `true` and `false` also
ignore case, but they are words only where an operator or a value stands: everywhere else they
are names, so ``from Tag where like = true`` compares the field `like`. A word in backticks is
always a name.

### Strings

A string holds any characters between double quotes; a doubled quote `""` stands for one quote,
e.g. `"Box ""large"""` is `Box "large"`. Nothing else is an escape: a backslash is a backslash.

### Names

Type and field names are case-sensitive and match the names DataForge declares exactly. An
unknown name is an error that names the closest known one, if any, e.g.
``did you mean `mass`?``.

- `from <name>` names a struct. It selects the Records of that struct and of every struct that
  derives from it, directly or through other structs.
- A path starts with a field of the Record's type, inherited fields included. With `from`, the
  fields are those of the named struct. Without `from`, each Record's own type is used, and a name
  that no struct declares is an error.
- A name behind `.` is a field of the struct the path has reached, a name behind `->` a field of
  the struct a Reference links to, and a name in brackets is a struct; they follow
  [Paths into nested data](#paths-into-nested-data).

### Record-path glob

The string of `at` matches record paths with the rules of a Filter: with `*` (any run of
characters, separators included) or `?` (exactly one character) it is a glob over the full
record path; otherwise it matches when it occurs anywhere in the record path. Both forms ignore
ASCII case and treat `/` and `\` alike. The record path has no `.xml` extension.

### Records that refer to a Record

`referencing "<text>"` keeps the Records that hold a Reference to a Record: the Reference
fields and weak pointers to another Record, wherever they stand in the Record, nested structs and
array elements included. The text is the GUID of the Record, or its record path (ignoring ASCII
case, `/` and `\` alike and a trailing `.xml`); a record path that several Records share names all
of them, and the Query selects the Records that refer to any of them. A Record that refers to
itself is not selected for that. A weak pointer to a nested instance of another Record, not to
that Record itself, is not a Reference and selects nothing. It combines with `from` and `at`:
both keep only the Records that also meet them. The clauses stand in the order `from`, `at`,
`referencing`, `select`, `where`; `at "…"` behind `referencing` is an error that says so. An
Archive with more than 8,388,608 References is indexed up to that many, the first ones in Record
order. The first Query with `referencing` of an Archive reads every Record once to
index the References ([ADR-0029](adr/0029-references-are-indexed-once-per-archive-from-record-trees.md));
the window keeps that index for the Archive, and `p4kx dcb refs` lists the References themselves
with their fields.

Text that is no GUID and names no Record is a compile error marked on the string, e.g.
``no Record has the GUID or record path `libs/nowhere` ``.

## Paths into nested data

A path walks from a field of the Record into the values it holds, segment by segment, and has to
end at a scalar: a boolean, integer, float, string, localisation key, GUID or enum value.

| Segment | Behind | Reaches |
| --- | --- | --- |
| `.name` | an inline struct, or a strong pointer | the field `name` of the struct, or of the struct the pointer owns |
| `[n]` | an array | the element at index `n`, counted from 0; `null` when the array has `n` or fewer elements |
| `[*]` | an array | every element, in stored order |
| `[TypeName]` | an array of inline structs or strong pointers | every element whose struct is `TypeName` or derives from it, in stored order |
| `->name` | a Reference or a weak pointer | the field `name` of the Record the Reference links to, or of the instance the weak pointer names ([Following References](#following-references)) |

- Behind a strong pointer the fields are those of the struct the field declares, also when the
  pointer owns an instance of a struct deriving from it; behind `[TypeName]` they are those of
  `TypeName`, so `Components[VehicleComponentParams].crewSize` reads a field the base type of the
  components does not have.
- A null pointer reaches `null`, and so does a path through it; so does an index out of range.
  `[TypeName]` skips null pointers, as they have no type.
- A pointer to an instance of a struct that does not derive from the declared one, an array that
  lies outside its values, a pointer to a missing instance or struct (also under `[TypeName]`)
  and an instance shorter than the layout the path reads it with reach an error value, shown as
  `error: …` at its place. A layout that cannot be read never reads as `null` or `[]`.
- References and weak pointers are links, not owned values: a path continues behind them only
  with `->`, and since they are no scalars, it cannot end at them either.

These are compile errors, marked on the segment or name that causes them:

- a field the struct does not declare, or a `TypeName` no struct has, with a suggestion;
- `.name` behind a scalar, behind an array (select its elements first) or behind a Reference or
  a weak pointer (the message points to `->name`);
- `->` behind a value that is neither a Reference nor a weak pointer, behind an array of them
  (select its elements first) or behind one whose field declares no struct, marked on the `->`;
- `[n]`, `[*]` or `[TypeName]` behind a value that is no array, and `[TypeName]` behind an array
  of scalars or links;
- a `TypeName` that neither is nor derives from the struct of the elements, nor is one of its
  bases, so no element can have it;
- a path that ends at a struct, an array, a pointer or a link.

Without `from`, the path is resolved for each Record's own type: a type that does not resolve it
gives an empty cell and no value to compare, and the path is a compile error only when it
resolves in no type; the error is then the one of the first struct that declares its first
field.

### Following References

`->name` follows a Reference to the Record it links to, or a weak pointer to the instance it
names, and continues the path there with the field `name`. `->` can be chained, as in
`maker->home->name`, and stands behind array elements, as in `escorts[*]->name`, which follows
each element and gives one value per element. It works alike in `select` and in `where`.

- The fields behind `->` are those of the struct the Reference or weak pointer field declares,
  also when the Record it links to is of a struct deriving from it. A Record or instance of a
  struct that neither is nor derives from the declared one reaches an error value, as behind a
  strong pointer.
- A null Reference, a Reference to a GUID no Record has (a dangling one) and a null weak pointer
  reach `null`, and so does the rest of the path behind them. Each such link the `select` paths
  of a row meet counts as one unresolved Reference, however many paths and elements reach it, and
  [the result](#output-formats) reports their number; in `where`, the comparison is unknown, as for any `null`. A weak pointer to an
  instance that does not exist reaches an error value, as a strong pointer does.
- A path follows each Reference it names once and names at most 16 (`MAX_PATH_REFERENCES`), so
  References that link in a cycle cannot make it follow more. `->` counts no array element
  against `MAX_PATH_VALUES`.

### Several values

A path through `[*]` or `[TypeName]` can reach any number of values, so its cell is always a
list, also when it reaches one value or none: the values in the order the path reaches them,
nested arrays flattened, e.g. `Components[*].name` of a ship with a null first component is
`[null, Wall]`. A null or error value stays at its place, so two lists over the same array line up
element by element. A path without `[*]` and `[TypeName]` always gives one value, never a list.

JSON writes a list as an array of the values, each typed as a single value would be, and CSV
writes that same JSON array in its field. The table writes the values in their text form,
separated by `, ` in brackets, e.g. `[250.5, 1200]` or `[]`, shortened after 8 values; the
values are not quoted there, so a string holding `, ` or `]` reads ambiguously in the table only.

The `select` paths of one row share a budget of 4,096 array elements (`MAX_PATH_VALUES`), as
cells retain their values. `[n]` counts one element, `[*]` and `[TypeName]` every element of their
array, including those a type selector skips; nested brackets count at every level. A bracket
that would exceed the remaining budget produces the error value `error: the paths of the Record
reach more than 4096 array elements` in its cell. The cells after it can use the remaining budget.

Comparisons in `where` stream values without retaining a list or imposing an element-count cap.
An existential comparison stops when a value proves it true. Cancellable selection checks its
callback at Records, path steps and array elements, including elements skipped by `[TypeName]`.
An observed stop is latched and drops the entire selection.

## Conditions

`where` keeps the Records whose values meet a condition. `not` binds tightest, then `and`, then
`or`: `a = 1 or b = 2 and not c = 3` means `a = 1 or (b = 2 and (not c = 3))`. Parentheses group
differently. Parentheses and `not` nest at most 32 deep.

### Comparisons

A comparison reads the value at its path and compares it with the value written after the
operator. Which values a field can be compared with depends on its type; any other pairing is an
error marked on the comparison, e.g. ``"`=` cannot compare field `dps` of `Weapon`, which holds
Double values, with a string"``.

| Field type | Value | Operators | Rule |
| --- | --- | --- | --- |
| integers (`Int8` … `UInt64`) | a number without fraction or exponent | `=` `!=` `<` `<=` `>` `>=` | by value, exactly, across all integer types |
| `Single`, `Double` | any number | `=` `!=` `<` `<=` `>` `>=` | by value as IEEE 754 floats, see below |
| `String`, `Locale` | a string | `=` `!=` `<` `<=` `>` `>=` | exactly: case-sensitive, ordered by Unicode code point |
| `Enum` | a string | `=` `!=` `<` `<=` `>` `>=` | by the option name, as for a string |
| `String`, `Locale`, `Enum` | a string | `contains` `starts with` `like` | ignoring ASCII case |
| `Guid` | a string `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` | `=` `!=` `<` `<=` `>` `>=` | by value, ordered as the lower-case text form |
| `Bool` | `true` or `false` | `=` `!=` `<` `<=` `>` `>=` | `false` before `true` |

- An integer value beyond every integer type (more than about 39 digits) is an error. A value
  outside the range of the field's own type is no error: `size8 = 300` just never holds, and
  `count > -1` always holds for an unsigned field.
- A number compared with a float field is first rounded to the field's precision, `Single` or
  `Double`, as the value would be stored, so `mass = 0.1` finds a `Single` field that holds 0.1.
  A number beyond the range of that precision is an error.
- Floats compare as IEEE 754 does: `-0` equals `0`, and NaN equals nothing, not even NaN, and is
  neither less nor greater than any number. So every comparison with NaN is false except `!=`,
  which is true.
- A GUID string with letters in either case is accepted; anything else is an error marked on the
  string.
- `contains` holds when the string occurs in the value, `starts with` when the value begins with
  it, and `like` when the whole value matches the string as a glob: `*` matches any run of
  characters and `?` exactly one. All three ignore ASCII case only; other characters match
  exactly. An empty string is contained in every value.

The type that counts is the one of the scalar the path ends at, e.g. `Double` for
`Components[Weapon].damage`; the message names the whole path. Without `from`, a comparison is
an error only when the path resolves in no type to a scalar that can be compared with the value;
a Record whose type resolves it otherwise, or not at all, has no value for it.

### Comparing several values

A comparison over a path through `[*]` or `[TypeName]` holds when **any** of the values the path
reaches holds: `Components[Weapon].damage > 1000` keeps a ship with at least one weapon above
1000. It is false when every value compares false, also when the path reaches no value at all,
and unknown when none holds but one is unknown, e.g. null. So `not Components[*].name = "x"`
keeps the Records where no component is named `x` and every name is known (no null component),
and a condition that has to hold for **all** values is written as the negation of its opposite:
`not Components[Weapon].damage <= 1000` keeps the ships whose every weapon does more than 1000
damage, ships without weapons included.

### Missing values

A comparison is unknown when the Record has no value for its path: its type does not resolve the
path (without `from`), the path ends at a type the value cannot be compared with, reaches `null`
through a null pointer or an index out of range. An unknown
comparison never matches, and the conditions combine as in SQL:
`not` of unknown is unknown, `false and unknown` is false, `true or unknown` is true, and any
other combination with unknown is unknown. A Record is kept only when the whole condition is
true, so `x = 1` and `not x = 1` together never select a Record without a value for `x`.

A demanded value that cannot be read is a selection failure, carrying the Record definition
index, record path, comparison index and original value-reading error. It does not become unknown
or a successful empty result. A true existential witness, `true or ...`, or `false and ...` proves
the result without demanding irrelevant read failures, including failures reached before that
proof. Without such a proof, a reached read failure is reported even alongside ordinary unknowns.
A stop remains authoritative over a proof or read failure.

`CompiledQuery::run` returns `Result<QueryRun, SelectionError>`; `run_while` returns
`Result<Option<QueryRun>, SelectionError>`, with `Ok(None)` for a stop. Successful indexes alone
may be cached or resumed. CLI query and field statistics return DataForge error status 5 on a
selection failure before writing output; MCP returns a tool error and the GUI reports a failed
Query. Existing atomic exports remain intact when selection fails or is stopped.

A Record whose entire layout or instance cannot be read retains its visible failed row. This is
separate from an unreadable value reached while deciding a condition. For candidates with readable
whole-Record layouts and instances, successful selection completes demanded condition evaluation
under SQL/Kleene semantics. Whole-Record failures remain explicit failed rows; successful selection
does not certify global source completeness.
The contract is recorded in [ADR-0358](adr/0358-query-selection-is-complete-or-explicitly-failed.md).

## Field statistics

`p4kx dcb fields <archive> [<type or Query>] [--path <glob>] [--format table|csv|json]
[--csv-raw]` and `FieldStatistics` in `p4kx-dataforge` list the **field paths** the Records have, to find the field
a Query needs. They read every selected Record once, in parallel
([ADR-0049](adr/0049-field-statistics-are-counted-per-path-in-one-parallel-pass.md)).

A field path is the field names from the Record down, joined by `/`, with no array positions.
The target of a strong pointer adds the name of its actual struct as a segment, so
`Components/SCItemWeaponComponentParams/fireActions/SWeaponActionFireRapidParams/fireRate` is
the `fireRate` of the rapid-fire action in a weapon component; an inline struct adds no segment.
It is not a Query path: the same value is
`Components[SCItemWeaponComponentParams].fireActions[SWeaponActionFireRapidParams].fireRate`;
`FieldStatistics::query_path` turns the one into the other. A path is listed for every field that
holds a value, also for the ones that hold fields (kind `Class`) or a pointer, so a path and the paths below it can be followed.

| Column | Meaning |
| --- | --- |
| `path` | The field path. |
| `kinds` | The data types of the values (`Single`, `Int32`, `String`, `Reference`, `StrongPointer`, `Class`, …), `[]` after an array's elements; several when Records of different types share the path. The JSON has the count of each. |
| `values` | How many values the path has: one per field, one per array element. An empty array has none. |
| `records` | In how many Records the path occurs. |
| `empty` | How many values are empty: a null pointer or Reference, an empty text or a null GUID. |
| `distinct` | How many distinct values the path has, without the empty ones; `32+` when there are more than 32. The values of a Reference are the GUIDs it links to; a strong or weak pointer and a `Class` have none, so their `distinct` is 0. |
| `min`, `max` | The smallest and the largest number, for integers and floats; empty for other kinds. |

An empty value is a value of its path, counted in `values` and `empty`, but it adds to no `distinct`,
`min` or `max`, so a null never reads as a zero. A real zero is a number like any other. `-0.0`
and `0.0` are one value: they count once in `distinct` and the number prints as `0`. NaN is no
number. Distinct values are told apart by a 64-bit fingerprint, so the count is exact up to a
collision of two fingerprints, which has a chance of about 2^-64 per pair of values. A float of
1e16 or more, or below 1e-5, prints with an exponent.

The selection is a type name, which takes the Records of that type and of the types deriving
from it, or a Query whose `from`, `at`, `referencing` and `where` clauses select the Records and
whose `select` clause is ignored; without it every Record is counted. `--path` keeps only the
paths that match, with the rules of a Filter (a glob over the whole path with `*` or `?`,
otherwise text anywhere in it, ignoring case), e.g. `--path "*/fireRate"`. The paths are sorted
segment by segment ignoring ASCII case, so a path is followed by the paths below it. The result
is the same for every number of threads.

A Record whose layout or instance cannot be read, or that has more than 1,048,576 values, is not
counted at all, not even the values found before the limit (the paths it walked still count
towards the path limit below), and is reported on stderr with exit
code 1. A value that cannot be read (a bad array
range, a pointer to no instance, an instance owned twice, a struct nested deeper than 64) is not
counted and is reported as a number on stderr and in the JSON. An Archive whose Records have more
than 1,048,576 distinct paths is an error (exit code 5), which bounds the memory
([ADR-0049](adr/0049-field-statistics-are-counted-per-path-in-one-parallel-pass.md)).

## Errors

A Query that cannot be parsed or compiled is rejected before any Record is read. The error names
the problem and its position: the CLI prints the line of the Query and marks the span with `^`:

```text
error: unknown field `mas` of `Item`; did you mean `mass`?
  from Item select mas
                   ^^^
```

`p4kx dcb query` exits with code 2 for an invalid Query, and with code 5 when the struct named
by `from` has a layout the DataForge cannot give, since that DataForge is corrupt.

## Canonical form

A parsed Query prints in one canonical form: keywords and word operators in lower case, one
space between tokens, `, ` between paths, no spaces around `.` and `->` and inside or before
brackets,
indices without leading zeros, backticks only where a name needs them, numbers as written,
strings with each quote doubled, and parentheses only where precedence needs them, e.g.
``from Ship select Components[Weapon].damage, tags[0], `a b`[*], maker->name``. Parsing the canonical form gives the same Query.
