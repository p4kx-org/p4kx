# Flat values

The flat value export is one row per leaf value of a Record, for tools outside P4KX: spreadsheets,
databases and scripts. It is written by `p4kx dcb export --flat` and by the Record export of the
desktop app. It reads the canonical Record tree, the same tree as [Record XML](record-xml.md) and
[Record JSON](record-json.md) ([ADR-0004](adr/0004-own-record-xml-with-references.md)), so every
value of those formats is a row here, except the ones listed under [Empty values](#empty-values).
The same Records always give the same bytes.

## Files

`--flat` writes one file into the target folder, whichever the Records are:

| Command | File |
|---------|------|
| `--flat` or `--flat --format csv` | `values.csv` |
| `--flat --format csv --csv-raw` | `values.csv`, exact values |
| `--flat --format ndjson` | `values.ndjson` |

The rows follow the order of `records.ndjson`: the Records by record path ignoring ASCII case with
`/` and `\` alike, then by Record name, then in definition order; within a Record the rows follow
the fields in declaration order, the fields of the root ancestor first, and the elements of an array
in stored order. The Records are built in parallel batches and streamed into a temporary file that
replaces the file only when complete, so memory does not grow with the number of Records; a
cancelled run leaves no file. `--conflict` applies to the file as it does to `records.ndjson`.
`--filter`, `--lang`, `--out` work as for the other forms; `--combined` and `--tags` are refused
with `--flat`, and so is `--format xml|json`.

## Columns

| Column | Content |
|--------|---------|
| `record_guid` | The GUID of the Record, lower case, `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` |
| `record_name` | The Record name, e.g. `EntityClassDefinition.AEGS_Avenger` |
| `record_path` | The record path without `.xml`, as DataForge stores it |
| `field_path` | The names of the fields from the Record down to the value, joined by `.`, with the positions of arrays removed |
| `array_indices` | The positions in the arrays on the way down, outermost first, joined by `.` (`12.0`); empty outside arrays |
| `kind` | `bool`, `int`, `uint`, `single`, `double`, `string`, `locale`, `enum`, `guid` or `reference` |
| `value` | The value, see below |
| `target_path` | For a `reference`, the record path of the target Record; empty for every other kind |
| `text` | Only with `--lang`: the resolved text of a `locale` value, empty otherwise |
| `record_tag` | The Record tag of a version 8 DataForge, e.g. `SystemsDesign`; empty for a Record without tag, with an empty tag, and for version 6 |

`record_tag` is always the last column: the ninth, or the tenth after `text` with `--lang`. It is
appended, so every column that existed before keeps its name and position; it is always in the
header, whichever DataForge version the Records come from
([ADR-0112](adr/0112-the-flat-value-export-is-csv-and-ndjson-and-offers-no-sqlite-output.md)).
It is the `$tag` of [Record JSON](record-json.md#record-and-struct-objects) and the `p4kx:tag` of
[Record XML](record-xml.md), and like them it repeats on every row of the Record.

`field_path` and `array_indices` identify the place: the second element of the `wheels` array of the
first element of `parts` is `parts.wheels` with `0.1`. A nested struct, an inline class or the
target of a strong pointer, adds its field names to the path. A field whose name starts with `$`,
or whose name an earlier field of the same struct, in declaration order, already has, is written
as `$<n>` with `n` its position among the fields of its struct in declaration order, counting from
0, as in [Record JSON](record-json.md#record-and-struct-objects); so two rows never share a place.

## Values

| Kind | `value` |
|------|---------|
| `bool` | `true` or `false` |
| `int` | A signed integer of 8 to 64 bits, exact decimal |
| `uint` | An unsigned integer of 8 to 64 bits, exact decimal; a value above the `i64` range stays exact |
| `single`, `double` | The shortest text that reads back to the same value (`0.1` for the single `0.1`, never `0.10000000149011612`), with an exponent below 1e-6 and from 1e21 in magnitude as in [Record XML](record-xml.md#values); `NaN`, `INF` and `-INF` as text |
| `string` | The string |
| `locale` | The localisation key as stored, e.g. `@item_Name` |
| `enum` | The name of the option |
| `guid` | The GUID, lower case |
| `reference` | The GUID of the target Record. A weak pointer is a `reference` to the Record that holds it; its place inside the Record is not written |

With `--lang`, a `locale` row whose key has a text in the language carries it in `text`, with the
`\n` escapes of the `global.ini` turned into line feeds. A placeholder key (`@LOC_EMPTY`, ...) or a
key without a text has an empty `text`.

## Empty values

A row is written only for a value that is there. No row is written for an empty string, an empty
localisation key or enum option, a null pointer, a null Reference, an empty array, or a value that
cannot be read (an error value, which the report of the export counts). The one error value that
keeps its row is a Reference to a Record that does not exist: it is a `reference` row with the
GUID it names in `value` and an empty `target_path`, as the Record JSON keeps the GUID in the
error. A real `0` or `false` is a value and has a row; there is no `0` in the place of an empty
value.

## CSV

- UTF-8 with a byte order mark, a header row with the column names and CRLF after every row.
  A cell is quoted with `"` when it holds a comma, a quote, a carriage return or a line feed,
  and a quote inside it is doubled.
- A `string`, `locale` or `enum` value, a `text` and a `record_tag` that starts with `=`, `+`, `-`,
  `@`, a tab or a carriage return would be run as a formula by a spreadsheet. It is written with a
  leading `'` (`'=1+1`). Every localisation key starts with `@`, so every `locale` value of this file starts
  with `'@`. Numbers, booleans, GUIDs and the other columns are never changed, so `-5` stays a
  number.
- `--csv-raw` writes the exact values: no byte order mark and no leading `'`. Use it for scripts
  and databases, and never open such a file in a spreadsheet that trusts it.
- `p4kx dcb export --flat --format csv-strict` writes ASCII CSV without a byte order mark
  and validates every field against [RFC 4180 §2](https://www.rfc-editor.org/rfc/rfc4180#section-2).
  Empty fields are quoted. An invalid field stops file publication and leaves an existing
  export intact. Use ordinary CSV or NDJSON to preserve values containing characters
  outside that grammar.
- Excel shows a `-INF` value as `#NAME?`, because the text is no number to it; `NaN` and `INF` show
  as text.

## NDJSON

UTF-8 without byte order mark, one compact object per row, ended by a line feed, with the keys
`record_guid`, `record_name`, `record_path`, then `record_tag` when the Record has a tag (the key
is left out otherwise), `field_path`, `array_indices` (the text above, `""` outside arrays), `kind`
and `value`, then `target_path` for a `reference` and `text` for a resolved `locale` value.
`value` is typed: a boolean, an exact number, or a string; a number beyond 2^53 loses precision in
readers that use doubles, as in [Record JSON](record-json.md#values). Strings are escaped as in
Record JSON. Values are never changed for spreadsheets.

## Not offered

SQLite is not an output of the export ([ADR-0112](adr/0112-the-flat-value-export-is-csv-and-ndjson-and-offers-no-sqlite-output.md)).

## Field origins

The CLI and GUI flat exports write a `.p4kx-origin.jsonl` sibling bound to the
complete CSV or NDJSON file. CSV addresses retain the emitted cell's byte
range and the logical field path `/rows/<row>/<column>`. The row ordinal
starts at zero and excludes the column header. NDJSON addresses use a
zero-based physical line and a JSON Pointer relative to it. Localized
cells retain the selected language lookup as well as the DataForge key.