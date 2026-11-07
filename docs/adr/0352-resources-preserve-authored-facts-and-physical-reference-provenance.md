# Resources preserve authored facts and physical reference provenance

## Decision

Export selected ResourceType Records as an independent `resourceTypes` catalog kind. Keep commodity
Record GUIDs as primary identities and nullable ResourceType GUIDs as non-unique `subtypeGuid`
links. Preserve the existing commodity fallback and singular first-selected-group meanings.

Preserve authored ResourceType and ResourceTypeGroup media paths, density units, molar mass,
validation flags, named cargo slots, ordered properties and all direct memberships. A lack of a
consumer is not an omission reason. Do not convert units, infer container capacities, choose one
property, or denormalize unrelated mineable-element values into resources.

Keep concrete source types and inline-versus-Record-reference ownership in quality source wrappers.
Use one shared QualityCurve with exact nullable Int32 bounds and finite f32 mean and standard
deviation, reflecting the confirmed source declarations. Wrong numeric types and unrepresentable
float values have explicit projection failures rather than default values. Known fieldless bases
are explicit; unsupported or extended shapes retain blocking source-health evidence.

Resolve catalog materialization through the existing bounded field inspection, source observations
and actual final rows. A resolved physical source reference is distinct from an emitted catalog
row. Record indices disambiguate same-GUID aliases within a Build. A different emitted physical
Record cannot satisfy the original target, and ambiguous emitted identities block publication.
Source-only Entity references used by thumbnails and packaging do not require catalog rows.

## Consequences

The catalog remains version 3 under ADR-0339; adding ResourceTypes is independently additive.
The public Rust QualityCurve bounds are `Option<i32>` through both existing reexport paths. The
released schema-v2 compatibility fixture remains frozen.

Only selected groups affect legacy joins and all-membership indexes. Source lists retain ordering,
repetitions, null positions and cycles. Missing, false, zero and empty states remain distinct where
the source representation distinguishes them.

Reference capture shares the canonical cancellable walk, with limits of 4096 captured references
and 1 MiB of logical metadata per physical Record. Exceeding either limit yields incomplete field
inventory and blocking limit evidence rather than a complete empty inventory. No second provenance
ledger or additional Record traversal is introduced.

The [resource contract](../catalog-resource-types.md) owns field mappings, source-state policies
and the declaration-only LIVE/PTU compatibility evidence. Synthetic fixtures support native
four-target validation; acceptance evidence is recorded separately. They publish no game Record
identities or values.
