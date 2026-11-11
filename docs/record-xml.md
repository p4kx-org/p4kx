# Record XML

Record XML is the XML form of one Record of DataForge, written by `p4kx dcb export --format xml`
and by the `RecordXml` writer of `p4kx-dataforge`. It serialises the canonical Record tree, the
same tree [Record JSON](record-json.md) and the Comparison use: owned children are nested, References are links and never
embedded ([ADR-0004](adr/0004-own-record-xml-with-references.md)). It is not compatible with the
XML of unp4k.

The same Record always gives the same bytes, so two exports can be compared with ordinary diff
tools.

## Example

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Vehicle xmlns:p4kx="https://github.com/p4kx-org/p4kx/record-xml" guid="cbee34fd-6441-4000-b99f-924f7f26e19f" path="libs/foundry/records/vehicles/cart" p4kx:name="Vehicle.Cart" p4kx:tag="SystemsDesign" mass="250.5" label="@vehicle_Cart">
  <maker>
    <p4kx:reference guid="50e8a9f6-f956-4000-8ace-7c0ca9a91f5d" path="libs/foundry/records/makers/woodworks" type="Manufacturer"/>
  </maker>
  <wheels>
    <Wheel size="16"/>
    <Wheel size="17"/>
  </wheels>
  <spare>
    <Wheel size="15"/>
  </spare>
  <trailer>
    <p4kx:null/>
  </trailer>
  <front>
    <p4kx:reference guid="cbee34fd-6441-4000-b99f-924f7f26e19f" path="libs/foundry/records/vehicles/cart" type="Wheel" at="wheels[0]"/>
  </front>
  <tags>
    <p4kx:value value="wood"/>
    <p4kx:value value="old"/>
  </tags>
  <p4kx:field name="path" value="hangar"/>
  <p4kx:error field="name" reason="offset 4294967295 in the text table is not a valid string"/>
</Vehicle>
```

## Document

- UTF-8 without byte order mark, LF line endings, a final line ending.
- The first line is `<?xml version="1.0" encoding="UTF-8"?>`.
- One root element per Record, named after the Record's type (its struct), e.g.
  `EntityClassDefinition`.
- Every element is on its own line, indented by two spaces per level below the root. An element
  without child elements is self-closing.

## Root attributes

In this order:

1. `xmlns:p4kx`, with the value `https://github.com/p4kx-org/p4kx/record-xml`; left out only
   when a wrapper element around many Records declares it.
2. `type`, only on a `p4kx:record` root (see [Names](#names)).
3. `guid`: the Record's GUID, lower case, `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`.
4. `path`: the record path without `.xml`, as DataForge stores it.
5. `p4kx:name`: the Record name, e.g. `EntityClassDefinition.AEGS_Gladius`; empty for a Record
   without a name. Several Records can share one record path, the name tells them apart. It is
   namespaced so that a field called `name` stays an attribute of its own.
6. `p4kx:tag`: the Record tag of a version 8 DataForge, e.g. `SystemsDesign`. Version 6 has no
   tags, and a Record without a tag or with an empty one has no `p4kx:tag`.
7. One attribute per scalar field, in declaration order: the fields of the root ancestor first,
   then those of each struct down to the Record's own type.

Attributes are separated by one space, values are in double quotes.

## Values

| Field type | Text |
|------------|------|
| bool | `true` or `false` |
| int8 … int64, uint8 … uint64 | Decimal, with `-` for negative values |
| float, double | The shortest text that reads back to the same value; with an exponent (`1e21`, `1.5e-7`) from 1e21 and below 1e-6 in magnitude, otherwise without (`0.1`, `25000.5`, `3`); `-0` for negative zero, `NaN`, `INF` and `-INF` |
| string | The text |
| localisation key | The key as stored, e.g. `@item_Name`; the text is added only with `--lang` (see [Resolved texts](#resolved-texts)) |
| GUID | Lower case, `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` |
| enum | The name of the option |

In attribute values `&`, `<`, `>` and `"` are written as `&amp;`, `&lt;`, `&gt;` and `&quot;`, and
tab, line feed and carriage return as `&#9;`, `&#10;` and `&#13;`, so they survive attribute
normalisation. Characters XML 1.0 cannot hold (control characters other than tab, line feed and
carriage return, U+FFFE and U+FFFF) are written as U+FFFD.

## Child elements

After the attributes, one child element per field that is no attribute, in declaration order:

- `<p4kx:field name="…" value="…"/>` for a scalar field whose name cannot be an attribute (see
  [Names](#names)).
- `<p4kx:error field="…" reason="…"/>` for a value that cannot be read, with `guid="…"` after
  `reason` for a dangling Reference (see [Errors](#errors)).
- A field element for a nested struct, an array, a Reference or a null: named after the field,
  or `<p4kx:field name="…">` when the name cannot be an element name. It holds the field's value
  element; for an array, one value element per element in stored order, and an empty array is a
  self-closing field element.

Value elements:

- **Struct**: an inline class or the struct a strong pointer owns, named after its type, e.g.
  `<Wheel size="16"/>`, with the attributes and child elements of its fields under the same rules
  as the root. A strong pointer may own a struct derived from the declared type; the element
  carries the actual type. When the type name cannot be an element name, the element is
  `<p4kx:struct type="…">`.
- **Reference**: `<p4kx:reference guid="…" path="…" type="…"/>` with the GUID, record path and type
  of the target Record. The target's content is never written.
- **Weak pointer**: a Reference into the Record that holds it, with the same `guid` and `path`, the
  type of the target and its location in `at`. The location names the fields from the Record down
  to the target, separated by `.`, with `[index]` after an array field, e.g.
  `at="Components[23].AnimationJointControls.variables[0]"`. A weak pointer to the Record's own
  instance has no `at`; one to the instance of another Record is a Reference to that Record. In
  the game data every weak pointer points inside its own Record.
- **Null**: `<p4kx:null/>` for a null strong or weak pointer and a null Reference.
- **Scalar** in an array: `<p4kx:value value="…"/>`.
- **Error** in an array: `<p4kx:error reason="…"/>`, with `guid="…"` for a dangling Reference.

## Resolved texts

`p4kx dcb export --lang <language>` writes the text of every localisation key next to the key, in
the `global.ini` of that language. Without `--lang` none of this is written and the output is
byte for byte the output described above, free of any language. With `--lang` the root element
always declares `xmlns:p4kx`, and a localisation key that is no placeholder (`@LOC_EMPTY`,
`@LOC_PLACEHOLDER`, `@LOC_UNINITIALIZED`, `@blank_space`) and not empty is annotated:

- A key written as an attribute, `label="@item_Name"`, is followed by `p4kx:text-label="Name"`
  holding its text, or by `p4kx:missing-label="true"` when the language has no text for it (no
  `global.ini` line, or an empty text). The prefix `p4kx:` cannot collide with a field name.
- A key in a `<p4kx:field name="…" value="@key"/>` element or a `<p4kx:value value="@key"/>`
  element gets `text="Name"` or `missing="true"` on the same element.

The text is as the `global.ini` holds it with its `\n` escapes turned into line feeds, escaped like
any attribute value. A placeholder or empty key stays alone: it stands for no text and is not a
missing translation. Only localisation keys are annotated, never strings that look like keys. The
same Records and the same language always give the same bytes.

## Tag paths

Many fields (`Tags`, `RequiredTags`, `PortTags`, …) refer to a Tag Record, whose record path is the
same for every Tag. `p4kx dcb export --tags` writes the tag path of the Tag (`Weapon.Gun`, the names
from the root of the tag tree down, joined by `.`) next to such a Reference in the `tag` attribute:

```xml
<p4kx:reference guid="aa687902-f049-47c0-9045-8725ecf395cf" path="libs/foundry/records/tagdatabase/tagdatabase.tagdatabase" type="Tag" tag="Ship.Series.Avenger"/>
```

The attribute follows `type` and `at`. Only a Reference of type `Tag` gets it, and every Tag Record
has a tag path: a Tag that no `TagDatabase` reaches is a root of its own and its path is its name.
A Reference to a Record that does not exist is an error value and has none. Without `--tags` no
`tag` attribute is written and the output is byte for byte the output described above. A tag path
is not unique (sibling Tags may share a name), so the `guid` stays the identity. The same Records
and the same tag tree always give the same bytes; `--lang` and `--tags` combine.

## Errors

A value that cannot be read becomes an error at its place and the rest of the Record is still
written. Errors are a string offset outside the text table, an enum or struct that does not
exist, a bool byte other than 0 and 1, a pointer to a struct or instance that does not exist, an
array outside the values stored for its type, a Reference to a Record that does not exist, a weak
pointer outside the Record, an instance owned twice in one Record (which also cuts a cycle of
strong pointers) and a cyclic scalar inline Class declaration. The reason is English text for
people, not a stable code.

A Reference to a Record that does not exist (a dangling Reference) also carries the GUID of the
missing Record, in lower case, in the attribute `guid` after `reason`, in a field and in an array
alike (ADR-0269); no other error has it:

```xml
<p4kx:error field="entityClass" reason="the Reference refers to the Record 3cf6753c-d69c-4980-a45e-724151407096, which does not exist" guid="3cf6753c-d69c-4980-a45e-724151407096"/>
```

A Record whose struct layouts cannot be read is not written. Canonical construction and writers
use heap continuations without an owned-depth or mixed value/work cutoff; see
[canonical owned values](canonical-owned-values.md) for ownership, costs and consumer boundaries.

## Names

A field is written as an attribute only when its value is a scalar and its name is an XML name
without `:` that does not start with `xml` in any case, is not `guid`, `path` or `type`, and no
earlier field written as an attribute has the same name. Any other scalar field is a `p4kx:field`
element, so no value is lost or mistaken for another. A field element is named after its field
when the name is an XML name without `:` that does not start with `xml` in any case; otherwise it
is a `p4kx:field` element with the name in `name`.

When the type name is no such name, the root element is `p4kx:record` and the type is its `type`
attribute; a nested struct is then a `p4kx:struct`.

## Files

`p4kx dcb export` writes one file per Record below the target folder
([ADR-0008](adr/0008-record-files-by-record-path.md)):

- `<record path>.xml` for a Record that is alone at its record path,
- `<record path>/<Record name>.xml` for Records that share a record path, e.g.
  `libs/foundry/records/tagdatabase/tagdatabase.tagdatabase/Tag.Weapon.xml`,
- `<record path>/<Record name>.<GUID>.xml` when such a file collides with another one ignoring
  case, and `<record path>/<GUID>.xml` when a Record name is no plain file name.

Which file a Record gets depends on the whole DataForge, never on what `--filter` selects; folders
that differ only in case are one folder, spelt as the first Record in definition order needs it.
Record paths and names that could leave the target folder are rejected.

`p4kx extract --convert` and the Conversion switch of the Extraction window write the DataForge
Entry the same way, one file per Record, below the folder of that Entry instead of the `.dcb`:
the Records of `Data\Game2.dcb` go below `Data/`. Only the Entry the DataForge is read from is
converted, `Game2.dcb` before `Game.dcb`; another `.dcb` is written unchanged. A DataForge that
cannot be read or opened, or is larger than 2 GiB, fails the Entry; no raw `.dcb` is written in
its place.

## Combined document

`p4kx dcb export --combined` writes every selected Record into one file in the target folder,
named after the DataForge Entry with `.xml`, e.g. `Game2.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<p4kx:records xmlns:p4kx="https://github.com/p4kx-org/p4kx/record-xml">
  <Manufacturer guid="50e8a9f6-f956-4000-8ace-7c0ca9a91f5d" path="libs/foundry/records/makers/woodworks" p4kx:name="Manufacturer.Woodworks" code="WOOD"/>
  <Vehicle guid="cbee34fd-6441-4000-b99f-924f7f26e19f" path="libs/foundry/records/vehicles/cart" p4kx:name="Vehicle.Cart" p4kx:tag="SystemsDesign" mass="250.5">
    <maker>
      <p4kx:reference guid="50e8a9f6-f956-4000-8ace-7c0ca9a91f5d" path="libs/foundry/records/makers/woodworks" type="Manufacturer"/>
    </maker>
  </Vehicle>
</p4kx:records>
```

- The rules of [Document](#document) apply to the whole file.
- The root element `p4kx:records` declares `xmlns:p4kx` once; it is self-closing when no Record
  is selected.
- One child element per Record, written as the root of a single Record but indented by one level
  and without its own `xmlns:p4kx`.
- Records are in Record order ([CONTEXT.md](../CONTEXT.md)): by record path ignoring ASCII case
  with `/` and `\` alike, then by Record name ignoring ASCII case, then by their order in
  DataForge; a Record selected twice is written once.
- A Record whose tree cannot be built is left out and reported as failed.
