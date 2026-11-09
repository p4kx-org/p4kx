# Canonical ResourceGroup source tables

P4KX 0.5.0 emits only catalog schema v3. `definitions.resourceGroups` owns every selected physical
ResourceTypeGroup's original header, `name` from displayName, protected `description` and original
thumbnail paths. Selection remains the existing resourcetypedatabase path/type rule, Entity Filter
and development-leftover policy. Definitions sort by GUID then physical Record index. Equal GUIDs
and names do not merge physical definitions; ambiguous catalog identities remain health findings.

`declarations.resourceGroupMembers` has one row for the same selected physical owner, addressed by
GUID and Build-local recordIndex. It owns the original `groups` and `resources` arrays, not expanded
or deduplicated memberships. Both arrays, each original slot, authored GUID and independently read
physical target header have the shared immediate SourcedValue carrier. Missing, explicit null,
present empty, unreadable and invalid shapes stay distinct. Null slots, repeated edges, multiple
parents, self edges, cycles and unavailable/wrong-family targets keep their original position.
Source resolution and target catalog materialization remain separate facts. No recursive expansion
or additional source resolver is introduced.

`projections.resourceGroupParents` is the explicitly named first-selected, non-self parent choice.
Its parent carrier is present with a sourced physical Record reference, or present with null when
the existing algorithm finds none. This is a convenience result, not an authored parent field or
a claim that the graph is a tree. The existing physical scan order decides; output GUID sorting
does not change it. Child lookup remains keyed by nominal GUID, so duplicate physical child
definitions share the same convenience choice. An edge with the same child and parent GUID is
ignored even when it resolves to a different physical Record; the raw declaration still retains it. A two-node cycle retains both chosen parents. All original edges remain in the
declaration table, including the self edge that the parent choice ignores. The root flat
`resourceGroups` array and its embedded parentGuid are not emitted in parallel.

Parent selection provenance is captured during the existing group-map construction. It includes
the child's matching GUID, actual required candidate headers and array controls, original prior
slots and the chosen incoming occurrence. The chosen parent's GUID contributes as payload; its
returned target header fields keep their own immediate sources. Later unused groups and suffix
slots do not contribute or make an established choice incomplete. An earlier unreadable required
candidate remains an actual attempted observation and makes the choice's contribution set
incomplete; it does not become an invented payload contribution. No GUID/hash lookup restores
source authority and no whole-registry origin aggregate substitutes for these inputs.

The original arrays are prepared from the actual canonical tree before map construction. Existing
commodity first-group/typeGuid and ResourceType groupGuids algorithms, physical input order,
selection and IDs remain unchanged. Reads performed only for health do not add selected definition
or declaration rows and do not supply extra parents. Group definition rows alone count toward
Kind::ResourceGroups; membership and parent projection rows have no additional selected-source unit.
Readable source facts survive malformed field values in diagnostic publication, with exact field
and shared source/read/projection health evidence. Normal incomplete exports retain their typed
refusal before destination creation. The added source-array, map-insertion and decision-prefix loops poll the existing CancelToken.
The existing file adapter owns cancellation, atomic publication
and preservation of the prior complete file.

All header/data/list/link values expose containsEncryptedSource directly with their actual
sourceUsesRefs and sourceSetComplete. Original DCB inputs, localization Entries and container
ancestry use the existing common provenance registry. Source attempts and untouched references are
observations rather than contributions. Public short names use texts.publicNames; descriptions and
their owner references use texts.gameTexts and x-gameText. The same key used in both domains stays
two separate text rows. Languages retain their actual source Entries and exact shared decoding
rules. Unknown provenance remains unknown; encryption alone grants no publication permission.

The known owning declaration inventory has six ResourceTypeGroup fields: displayName, description,
defaultThumbnailPath, defaultThumbnailPathSVG, groups and resources. New declared fields, including
zero-valued ones, produce unsupported coverage findings rather than silently disappearing. This
migration adds original recordName, recordIndex, sourceType, full per-datum origins and physical
reference identities beyond the flat payload. It does not claim that a deferred installed-Archive
run has verified every current Build declaration. The existing build-bound resource oracle is
extended for all selected group source facts without publishing source identities; execution remains
subject to the user's installed-Archive acceptance deferral.

See [resource definitions](catalog-resource-types.md), [catalog health](catalog-export-health.md),
[the v3 family map](catalog-v3-data-map.md), [ADR-0356](adr/0356-catalog-v3-uses-canonical-source-typed-tables.md)
and [ADR-0357](adr/0357-source-encryption-provenance-is-explicit-per-data-value.md).
