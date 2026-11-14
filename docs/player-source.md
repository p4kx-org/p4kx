# Player character source

`p4kx character source <archive> --body <male|female> --out <file>` writes a
[p4kx-player-source v1 document](../schemas/player-source.schema.json). The explicit body
selects the corresponding conventional default CHF. Each item port retains its original
parent, sibling ordinal, CRC and item GUID. The joined class retains the physical DataForge
record ordinal and original GUID. Duplicate class matches are preserved, and empty slots,
missing targets and unreadable sources have distinct states. Each non-root CHF port also
links its original CRC to the matching port definitions on its immediate parent item class.
The link retains each candidate's physical component and port ordinal. The original flags,
attachment type and ordered helper alternatives retain their tags and local offsets. The authored
`noRotation` constraint is available as a source value.

The export inventories every XML entry under the archive's player default-loadout directory
in archive order. Each `Items` group or `Item` has its original XML coordinate and an
optional index of its direct parent. An empty `itemName` and an omitted attribute remain
distinct. The `documents` table retains original attributes and element order, including
sections outside the typed item tree. Consumers can select loadouts without assuming
which one is used at runtime. A nested XML item's port definition is looked up on the class
explicitly named by its immediate parent item. Root items and items beneath an empty or omitted
parent selector report that owner state. A duplicate class or port name retains its candidates.

`motion` identifies the DataForge Player actor and its original animated-character component
slots. `actionController` and `animDatabase3P` select original archive XML documents. The
document indexes reference controller scopes, fragment alternatives and ADB subdatabase
declarations in their original order. When both XML roots are readable, the component includes
`authoredOptions` for direct root `FragmentList` options in ADB order, with separate original
`Tags` and `FragTags` values. The controller `Fragments@filename` and ADB `FragDef` must
identify one physically observed shared definition document before an option links to same-name
controller fragment candidates. Each unambiguous controller fragment retains its original
scope tokens and their candidate XML coordinates. An empty candidate list is an observed miss.
When the selected ADB is readable, its component includes `databaseGraph`, with the root document
and reachable physical branch documents in first-visit order. Each branch keeps its original root
definition selectors, fragment options and SubADB declarations. `links` retain ordered File and
Tags values and explicit lookup outcomes. `traversal` marks a link to an active ancestor as a cycle.
Several authored links may reference the same completed physical branch. Physical clips and gameplay
variant selection require additional source context.

Source-backed values expose `containsEncryptedSource`, `sourceSetComplete` and
`sourceUsesRefs`. A join preserves both the original selector and observed target source.
Raw XML values use their document's physical identity and their original local position.

## Shared Player source capture

Library callers can prepare several explicitly requested bodies with a shared source
inventory and a `PlayerDocumentSink`. The sink receives CRC-verified original XML bytes
while they are available. It can also retain verified bytes when XML parsing fails or
when the selected resource requires a different root. These observations carry the
physical source identity and the rejection evidence, so repeated declarations can refer
to the same retained bytes. The same physical source can be observed both as a parsed
document and as a rejected resource selection. An unreadable or unresolved source remains
represented by its disposition. Its bytes are unavailable to the sink. The CLI document's
schema and projected source identifiers are unchanged.

The shared capture's `adbDeclarations` retains each selected root or reachable branch
document once. Each fragment option keeps its original list, group and option positions,
GUID occurrences and ordered `AnimLayer` and `Animation` declarations. Each animation
name keeps its attribute position and source origin. Omitted names, empty names and
duplicates have separate states. Document indexes refer to this capture; physical
source identities and XML coordinates identify the original declarations. A declared
animation name can be retained before a physical clip candidate is resolved.

The shared capture's `tagDefinitions` links selected controller and ADB declarations
to authored Tag Definitions and FragmentID Definitions. Each source keeps its ordered
Tags sections, loose tags, groups, priorities and imports. A FragmentID's `subTagDef`
has its own scoped link. An original document can serve both roles and is retained
once. Links keep their own positions, source evidence and lookup dispositions, and
`cycleBackEdges` records where a traversal encounters an active ancestor. The graph
preserves authored topology without applying runtime tag-selection rules. See the
[CryEngine Tag Definition](https://www.cryengine.com/docs/static/engines/cryengine-5/categories/23756816/pages/23308473)
and [FragmentID Definition](https://www.cryengine.com/docs/static/engines/cryengine-5/categories/23756816/pages/23308474)
format references.
