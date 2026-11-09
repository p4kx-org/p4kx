# Canonical catalog Record headers

Schema 3 uses one shared header for definitions.tags, declarations.tagRoots,
declarations.entityTags, definitions.manufacturers, definitions.resourceGroups,
definitions.commodities and definitions.resourceTypes. Its eight fields are flattened into each
owning row, with the ordinary required state/value/containsEncryptedSource/sourceUsesRefs/
sourceSetComplete carrier on each field. This shared contract adds no second flat view or v2
writer. Minimal membership, parent and convenience-path owner references retain their own scope.

| Field | Exact meaning | Original source and role |
| --- | --- | --- |
| guid | Authored Record GUID; nominal identity remains separate from physical index | Header guid, payload |
| recordName | Exact stored name; a no-string sentinel is null and stored empty text is present | Header recordName, payload |
| className | Existing nonempty type-prefix removal, otherwise the existing original name; a missing name keeps the empty convenience value | Present name: header recordName conversion and sourceType selector; no-string name: recordName selector only |
| recordPath | Existing path with only a trailing .xml removed, ASCII-case-insensitively | Actual header fileName, conversion |
| recordIndex | Original physical Record position within this Build, independent of output sorting | Header recordIndex, selector |
| sourceType | Actual original Record struct type | Header sourceType, selector |
| fileName | Exact stored filename, preserving suffix, case, separators and absence of a suffix | Header fileName, payload |
| metadataTag | Raw Record-table metadata tag, separate from the Tag taxonomy and Entity tags | Header metadataTag; selector for layout absence, payload for explicit sentinel or stored text |

The v6 layout has no metadata-tag word, so metadataTag is absent. A v8 no-string sentinel is
null. A v8 actual empty string is present with value "". A readable nonempty string is present
unchanged. The format reader exposes failed string queries as their original errors; canonical
tree admission still rejects invalid filename/name/tag offsets in the established order. It
does not manufacture a successful row with an unreadable metadata field. The existing tolerated
v8 missing-tag issue does not acquire a new publication refusal. Legacy RecordTree tag access
still filters stored empty tags; the new borrowed metadata snapshot preserves that raw fact.

Header coordinates use the existing SourceLocation subresource variant, namespace
dataforgeRecordHeader, one original physical Record index in indices (a decimal string), and
one of guid/recordName/recordIndex/sourceType/fileName/metadataTag as field. The same originally
observed DCB payload SourceScope supplies these uses. No GUID/hash/path lookup restores source
authority. Body fields keep SourceLocation dataForge even if named $name, recordName or $guid;
the producer explicitly chooses typed Header operations rather than inspecting string prefixes.
Selecting a normalized path retains both the original filename conversion and its selector;
a failed filename read retains only its attempted observation.

An authored Reference GUID remains the owning body slot's payload. Resolved target index/type
come from the actual target's header recordIndex/sourceType selectors; its normalized target path
comes from that target's fileName conversion. Missing target headers remain absent at the
original owning slot. A derived chosen-parent reference uses its actually selected physical
parent's header GUID; it is not presented as an authored reference slot.

Catalog and Entity identity inputs, selection, duplicate handling, order, counts and convenience
values remain unchanged. Correcting colliding Header/Body coordinates changes document-local
SourceUse IDs in unreleased v3; consumers resolve the accompanying inventory rather than keeping
old use IDs across documents. The SourceLocation union and origin protocol version remain
unchanged. metadataTag neither expands Entity Filter scope nor chooses public/Member access;
containsEncryptedSource remains the per-datum conservative index over actual used inputs.

The format-owned RecordText and RecordMetadata APIs borrow the original read strings. Successful
RecordTree construction captures them once without a second parse or registry. Their getters
do not add serializer fields: standalone Record JSON/XML/Flat dialects retain their existing
bytes and names. Historical v2 fixtures remain frozen test inputs, not a supported output mode.

Synthetic proofs cover both layouts, sentinel/empty/present states, exact filenames, error-order
and standalone-byte parity, equal-GUID physical records, explicit Header/Body label collisions,
precise derivation roles, plain/encrypted/unknown sources, required schema fields and unchanged
Filter scope. The installed-Archive acceptance remains deferred by the user.
