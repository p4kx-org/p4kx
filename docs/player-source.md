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

## Shared source bundle

`p4kx character source <archive> --body female --body male --bundle --out <new-folder>`
shares the source inventory and motion across the requested bodies in caller order.
Each body can be selected once. The output directory must be new. It is complete
when `manifest.json` appears. A failed write leaves any foreign destination untouched.
The original single-body command still writes the v1 JSON file.

The [bundle manifest schema](../schemas/player-source-bundle.schema.json) and
[JSONL record schema](../schemas/player-source-bundle-record.schema.json) describe
bundle version 1. This version is independent of the single-file schema and the
catalog schema. The manifest binds all of `records.jsonl` by byte length, SHA-256
and line count. The first record is a format header. The last record counts the
preceding lines and repeats the total number and length of distinct raw artifacts.

Each `body` and `loadout` has a capture-local index. `motionComponent` records
retain their authored component ordinal and a stream index that relates their
`motionOptions`, `motionOption`, `databaseGraph`, `databaseBranch`,
`databaseOption` and `databaseLink` records. The branch and child indexes keep
authored order. `adbFragment` and tag-definition records refer to original document
indexes from the same capture. `source` and `sourceUse` share one identifier space
for all field receipts.

A `document` or `documentRejection` links its physical source to an opaque file
under `originals/`. The file holds the exact decoded Archive payload accepted by
the verified reader and carries an independent byte length, SHA-256 and CRC-32C.
Several observations of one physical Entry share one file. A failed read has a
source disposition and no original file. The consumer can verify references and
attributes while reading lines. Source preparation still retains the selected typed
projection in memory. It writes original XML bytes directly instead of building a
second full XML node table. The bundle contains source data and preserves unresolved
clip candidates without rendering.

`adbTagSelectors` refers to an `adbFragment` by `document` and `index`.
It retains original `Tags` and `FragTags` values with their attribute occurrences.
A whole empty value selects no tags. Empty tokens within a nonempty `+`-separated
value are retained and marked invalid for lookup. An attribute with multiple original
occurrences stays invalid and all occurrences remain visible.

Global tokens are compared with the ADB's own `TagDef`. Fragment tokens first
match the authored FragmentID in the ADB's `FragDef`, then use that declaration's
`subTagDef`. The comparison ignores ASCII case and preserves both authored
spellings. Non-ASCII spellings have an explicit unsupported result. See
[CryEngine Mannequin Tags & Tag Definitions](https://www.cryengine.com/docs/static/engines/cryengine-5/categories/23756816/pages/29450874).

`adbTagScope` rows retain the shared original evidence for a lookup, including
imports, contributing source-use references and whether the consulted scope is
complete. Several import paths to one physical tag return one candidate. Separate
physical declarations stay separate. An incomplete scope cannot establish a unique
or missing match. Candidate coordinates point to the original definition. A lookup's
encryption marker covers its original selector and referenced scope evidence.
These records describe authored matches and make no runtime selection.

`controllerTopology` describes one physical ControllerDef with its ordered
scope contexts, scopes, fragment declarations and scoped overrides. The records
preserve XML node and attribute coordinates, authored attributes and matching
scope/context declaration indexes. Duplicate definitions stay distinct. A
`motionControllerTopology` links a component to its selected controller document.
The component's physical rig owner stays unresolved when the authoring sources
provide a scope/context relationship without an observed model binding. See
[CryEngine Mannequin Scope Contexts](https://www.cryengine.com/docs/static/engines/cryengine-5/categories/23756816/pages/29450870).

`adbBranchSelectors` preserves each original SubADB selection at its owning
ADB document and local `databaseLink` index. Its global tags use that document's
`TagDef`. Direct `FragmentID` children retain original order and name occurrences.
Each usable name compares against the owning `FragDef`, with the matching
definition positions and source origin. Matches record authored narrowing rules.
The archive graph still retains every explicitly referenced branch.

`actorRigCandidates` reports availability of the original actor geometry inventory.
The ordered `actorRigCandidate` records retain every original geometry declaration,
its owning component, its stored SubGeometry positions, and its local Tags and
model path with source receipts. Original tag hints and exact matches for the
standard body skeleton paths are separate values, so contradictory hints and
repeated paths remain visible. These candidates do not establish which rig is
assigned to any controller context.

### Installed source verification

With `P4KX_DATA_P4K` set to the installed archive and the usual archive key
configured, run the focused witness locally.

```text
cargo test --locked -p p4kx-blender --lib installed_player_bundle_matches_selected_original_sources -- --ignored
```

It compares one two-body capture against the actor DataForge entry and the selected
ControllerDef, Loadout, ADB and TagDef originals. A separate DataForge read
reconciles the physical weapons and their complete ordered Loadout candidates.
The test follows original ADB TagDef roots and imports to reconcile each
scoped weapon and option token and checks field-use coordinates and encryption.
The selected source graph is read without writing a bundle.
The diagnostics report structural mismatches without printing source values.

`weaponMotionSource` records identify observed personal weapon AttachDef
occurrences by physical record index, GUID and component ordinal, and preserve the authored Tags. `weaponMotionLoadout`
references the source candidates of each original loadout item occurrence,
including competing physical item records. `weaponMotionTag` links original weapon
token occurrences and ADB global tag occurrences through the option's own
TagDef scope. Each source value carries its own origin. The ADB token evidence
addresses the original attribute, and the weapon token evidence addresses the
original DataForge field. The scoped match basis distinguishes unique, competing and
incomplete definitions. These shared lists retain each original occurrence
without materializing the combinations of weapons and options. The relationships
describe authored source candidates. Runtime selection and physical rig binding
require separate evidence.

For each requested body, `rigClipRig` records the physical lookup of the
standard skeleton and its adjacent CHRPARAMS. A parsed CHRPARAMS value links to
the original `document`. `rigClipRule` retains original AnimationList and child
ordinals and all name and path attribute occurrences. Supported masks use
the preceding valid `#filepath` in the same AnimationList with a plain `*` alias
and a recursive `*/*.caf` mask. Other mappings remain visible as original rules.

`rigClipCandidate` links a rule and a physical CAF image record by chunk and
record ordinal and retains its full original path. Its ASCII-insensitive
`aliasKey` can be compared with the matching key of a single original
`adbFragment` animation name. These comparisons describe possible matches for
each requested body. Controller scope and runtime playback selection remain
unresolved. Multiple physical alternatives are retained.

The CAF image's zero DBA checksum is an exact loose archive lookup. Other
checksums select only authored `$TracksDatabase` declarations in the rule's
AnimationList. Each matching declaration links to a `rigClipBank` row by index.
A bank record matches only by its full normalized path and keeps its physical
metadata ordinal, exact name and Entry-relative byte range. Bank rows carry
their physical lookup and read outcome so empty candidate rows can be
distinguished from unavailable or incomplete input.
