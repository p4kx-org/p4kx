# Subsumption event declarations

`definitions.subsumptionEvents` is the Catalog v3 table for physical XML entries under
`Data/Libs/Subsumption/Events/`. It retains one row per admitted Archive entry. Each row contains
the physical directory identity, the original and separator-normalized path, and the sourced
XML or CryXml encoding. The `elements` array preserves the original element hierarchy in
preorder, ordered attributes, namespace declarations and direct text. Source coordinates identify
the original text ranges or CryXml node and declaration ordinals.

`events` contains every unqualified `Event` directly under the unqualified `Subsumption` root
in document order. Its `elementIndex` selects the original element in the same document. The
typed `id` canonicalizes a GUID with or without braces. The `name` retains the original symbol.
Physical entry and element positions distinguish repeated declarations with the same authored
identifier. Cross-build readers can compare the authored ID to detect possible moves or renames
and compare the declarations and source locators separately.

The typed controls include authoring flags, propagation token, limits, range, volume and visual
inputs. Unknown attributes and descendants remain available in `elements`. Each typed value
carries its source state and origin. Duplicate known attributes and invalid typed tokens report
`invalid`. Omitted controls report `absent`. Original numeric and boolean spellings remain in
the XML. The Catalog preserves declarations and does not infer runtime event delivery or
links from DataForge mission modules.

## SDK

`catalog_queries::subsumption_events::read_subsumption_events` shares the Catalog projection and
returns the selected physical documents, their original source inventory and observed entry
coverage.

- `All` inspects the admitted event family in original root directory order.
- `EntryIndex` selects one physical root entry.
- `ArchivePath` compares the exact Archive-relative path with ASCII case and separator normalization.
- `EventId` inspects event XML within the admitted family and returns all physical documents with
  a matching direct declaration. Matching documents retain their other declarations. Use
  `event.id()` to identify the matching declarations within those documents.

Exact path and index selections open only their directory candidates. An identifier is stored in
the XML, so an ID query reads the event family. It checks for a matching authored ID before
materializing each document's full tree. The query opens no DataForge or model assets. Exact
lookups distinguish `Unique`, `Ambiguous`, `NotFound` and `Unavailable`. Required read and
parse failures can make a zero- or one-match result unavailable. Multiple observed matches remain
ambiguous. The reported inventory and entry counts identify the examined scope.
