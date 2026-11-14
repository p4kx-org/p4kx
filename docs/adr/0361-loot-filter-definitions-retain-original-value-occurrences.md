# Loot filter definitions retain original value occurrences

Catalog v3 retains every materialized original owned value of a selected PoolFilterRecord in an
owner-local flat arena, with ordered declarations, null/error slots and additional fields, because
recursive selected views lose source facts and impose a publication-depth convention.
This supersedes only the Pool-filter shape and sixteen-level cut in
[ADR-0238](0238-loot-comes-from-the-v3-records-and-the-entity-files-of-the-containers.md), within the
canonical tables of [ADR-0356](0356-catalog-v3-uses-canonical-source-typed-tables.md), direct per-datum
origins of [ADR-0357](0357-source-encryption-provenance-is-explicit-per-data-value.md) and original-read
bindings of [ADR-0360](0360-original-read-bindings-stay-separate-from-value-equality.md).

## Consequences

- Equal values remain distinct original occurrences; original preorder ordinal and output
  valueIndex are separate concepts and neither serialized value equality nor a local reference
  establishes physical source authority. References do not expand target bodies or simulate drop chances.
- Exact numeric widths/bits and all 15 current typed original-error variants/parameters are retained.
  Semantic Unreadable remains separate from those available facts; an unrepresented future decoder
  variant fails preparation explicitly instead of publishing a fallback cell.
- Each independent source-backed datum has its immediate origin. A scalar's typed representation
  shares its one node.value cell; separate declaration/type/target/error facts retain their own cells.
  Known short public facts/enum labels remain public; unclassified String/Locale domains conservatively
  retain exact protected Source text without invented translations. Text domain and crypto origin are independent.
- One iterative kernel serves the cached Inspector and Loot projection in two separate tree
  traversals without another decode or numeric-inspection pass. Canonical-reader resource policies
  remain separate explicit failures, not universal format guarantees or a new normal-output cutoff.

Entity-owned `LootGenerationComponentParams` are independent configurations selected by the
physical `EntityClassDefinition.Components` slot and ordinary Record filter. Each component's own
arena retains the authored `LootConfig`, `LootConstraints`, and ordered Reference and pointer
occurrences. A caller uses the original target header to join a canonical definition, when
selected. The exporter does not reclassify the owning Entity as an item or infer generated loot,
active themes or runtime constraint evaluation.

The [Loot source contract](../catalog-loot-source.md) defines the wire and contributor boundaries.
This decision establishes no runtime, generated-schema, installed-Build or complete-capture acceptance.
