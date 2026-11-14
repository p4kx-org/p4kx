# Catalog Tag taxonomy and Entity GUID tags

The [shared Record-header contract](catalog-record-headers.md) defines the eight sourced header
fields, exact original filename/metadata-tag states and separate Header/Body provenance coordinates.

Catalog 0.5.0 writes these datasets only in schema version 3. The released v2 fixtures remain
historical contract tests; there is no v2 writer, compatibility alias or parallel flat tag view.

## Dataset ownership

| Dataset | Source and identity | Selection and ordering |
| --- | --- | --- |
| `definitions.tags` | Every physical `Tag` Record; Build-local `recordIndex` distinguishes equal GUIDs | Complete global inventory, independently of the Entity Filter and development-leftover tokens; GUID then physical index |
| `declarations.tagRoots` | Each physical `TagDatabase` and its original `tags` array | Complete global inventory; physical Record order, then unchanged array order |
| `declarations.entityTags` | The root `tags` array of an owning selected `EntityClassDefinition` | Same selection as the owning item, ship, commodity, mineable Entity or crafter; GUID then physical index |
| `projections.tagPaths` | Existing `TagTree` first-selected depth-first paths | Existing tree pre-order, including unlisted fallback roots; independent of the Entity Filter |

Every table has the existing typed `availability` and `rows` contract. `available` with no rows,
source family `absent`, and metadata `unavailable` are distinct. Selection completeness is separate
from read/projection health: `completeness.tags` is `complete` even when source failures make
`exportHealth` incomplete. A consumer must inspect both before treating absence as deletion.
`declarations.entityTags` does not import extra dependency Entities excluded by the owning selection.

## Direct source fields

Definition and declaration headers retain `guid`, `recordName`, `className`, `recordPath`,
`recordIndex`, `sourceType`, `fileName` and `metadataTag` as separate sourced values. `recordName` preserves the declared
namespace; `className` uses the existing class-name conversion. No name or path becomes identity.

A Tag additionally stores:

- `tagName`: the original String, including empty text and duplicate names;
- `legacyGuid`: the original UInt32 `legacyGUID`, including `4294967295`; the convenience tree's
  absent-legacy-ID interpretation does not erase the stored sentinel;
- `children`: the original Reference array, preserving nulls, errors, repeats and positions.

A TagDatabase and an Entity declaration each store their original `tags` array. The owning parser
materializes these as References directly; no source `{value: ...}` wrapper is assumed. The JSON
sourced-value carrier introduces its own `value` keys.

Each present Reference stores a sourced authored `guid`, independently sourced `targetRecordIndex`,
`recordPath` and `targetType`, and the closed source-resolution control `status`: `present`,
`dangling` or `unexpectedTargetType`. Missing target headers remain explicitly absent while the
original dangling/wrong-family GUID remains present. Source resolution does not imply a uniquely
materialized catalog target; shared reference health separately reports ambiguous physical GUIDs,
failed targets, and selection outcomes.

Arrays and each element use the common `state`, `value`, `containsEncryptedSource`,
`sourceUsesRefs` and `sourceSetComplete` carrier. An absent array, explicit null, present empty
array, unreadable array and invalid shape remain distinct. Null slots retain their original
ordinal. A raw unsupported value is diagnosed rather than coerced to an empty successful list.

Entity GUID tags have a separate namespace from `items[].tags` (the whitespace-split words of
`AttachDef.Tags`), `RequiredTags`, port flags, and v8 Record metadata `$tag`. Those existing owners
keep their current payloads until their own canonical migration; they do not duplicate the stored
Entity root GUID array.

## Authoritative graph and convenience paths

Original root and child arrays are authoritative. They preserve multiple parents, repeated roots,
repeated children, cycles, dangling references, duplicate physical GUID definitions and unlisted
Records. A Tag with the same name as another remains a distinct definition. A path is neither a
GUID nor a unique key.

`tagPaths` reuses the existing `TagTree`: the first readable physical Tag for a GUID wins; the
first depth-first listing chooses its parent, later already-visited edges are cut, and unlisted
records are rooted by the existing orphan/cycle traversal. `parent` is a closed `root` or `tag`
projection; the latter retains its selected physical parent index and GUID. `depth` and `listed`
state the actual selected tree result. No additional walker or GUID-resolution algorithm is used.

The observed builder emits the chosen node position, parent position and typed incoming root/child
edge with its original owner and array ordinal. Null or malformed preceding slots never compress
that ordinal. Canonical reads occur once per physical Record in this pass, including duplicates;
the observer receives each original canonical result before convenience-tree cuts.

## Immediate encryption origin

Every direct header, name, identifier, parent array, element, authored GUID and target header has
its own immediate marker and actual source-use handles. A plain DCB value stays plain when only
an unrelated localization Entry is encrypted. Target headers use the actual physical target
context; untouched Reference observations do not taint the authored GUID or its parent array.

A computed path includes the actual selected ancestor names, actual selected incoming occurrences
and the headers that qualified those nodes. Name text contributes as conversion input; incoming
root/child slots and qualification headers contribute as selectors. Later unused parents, repeated
edges and duplicate definitions are not counterfactual contributors. The dot separator and closed
projection states are format/exporter controls. Depth, listing and selected-parent results use
the qualifying headers and selected root and child occurrences. The parent also uses its physical
Record GUID as payload. An unreadable name can produce a fallback path with an incomplete text
contribution set. The selected topology still has its original contributors.

These use the original DataForge source context and the one existing shared source-use registry.
No ID/hash lookup restores authority and no second registry or blanket whole-Record origin is used.
Known encrypted ancestors remain encrypted through the shared origin kernel; unknown original
metadata remains unknown. `x-gameText` protection and encryption origin are separate axes.

## Health, budgets and cancellation

The `tags` kind counts raw Tag definition rows only. TagDatabase Records are dependencies. Root
and path projection rows do not inflate selected Tag counts. Entity declaration diagnostics belong
to their actual owning selected Entity kind. Unclassified Records with unreadable type metadata
prevent a clean family-selection result; they remain separate from known Tag payload failures.
Canonical read failures, exact source-field issues
and ambiguous targets use the shared export-health pipeline.

The selected tree retains every readable winning Tag at its actual depth and preserves its full
name and joined path. The SDK stores local names and parent indices and expands contiguous text
on demand. Catalog rows store each selected node's local contributions and build one flat
contributor set as that row is written. The output still carries the full path and sorted
contribution references, so a chain's total JSON can grow quadratically with depth.

Relation diagnostics distinguish dangling, repeated and malformed edges. Physical duplicate
GUIDs have a separate diagnostic. Unlisted selection is visible on each path. A cycle cut
retains the original closing Reference.

Cancellation is checked by the observed callback and during new authored-array projection, in
addition to the existing TagTree/field-inspection polling. Cancellation propagates the typed stop
error. The unchanged shared catalog file adapter owns atomic publication, cleanup and preservation
of the last complete output; CLI and GUI prepare and write the same typed v3 result.

## Verification scope

Synthetic definitions cover same-name tags, repeated and multiple roots/parents, cycles, dangling
and wrong-family references, malformed/null slots, duplicate physical GUIDs, unlisted roots,
source states, exact chosen occurrence provenance, unknown/encrypted DCB origins, selection scope,
normal publication, source read failures, full depth/name/path output and the cumulative text boundary.
Generated-schema checks require canonical tables, nested carriers and closed payload correlation.
The minimal current-v3 schema fixture is distinct from the frozen released-v2 artifact.

The ignored `actual_tags_roots_paths_and_selected_entity_fields_match_the_installed_build` test
compares all readable physical Tag/TagDatabase sources, sampled owning Entity arrays and the actual
existing convenience tree against a fully identified installed Build. It watches for source changes
and reports aggregate counts/unresolved-reference counts only. It does not persist game identities
or assets. Installed-Archive acceptance remains subject to the user's deferral; synthetic tests and
this test definition do not establish that native acceptance has run.

See [catalog health](catalog-export-health.md), [the v3 family map](catalog-v3-data-map.md),
[ADR-0356](adr/0356-catalog-v3-uses-canonical-source-typed-tables.md) and
[ADR-0357](adr/0357-source-encryption-provenance-is-explicit-per-data-value.md).
