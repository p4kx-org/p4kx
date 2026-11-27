# Loot comes from the V3 loot Records and from the entity files of the containers

Decided for issue #322. Evidence: LIVE 4.10, ScDataDumper and NovaSCDataExtractor (facts only).

Pool-filter output shape and its sixteen-level cut are superseded by
[ADR-0361](0361-loot-filter-definitions-retain-original-value-occurrences.md). Other Loot-family,
selection and placed-container decisions remain scoped here.

## Decision

- The catalog exports the V3 loot Records as three kinds of schema version 2 (`lootTables`,
  `lootArchetypes`, `lootPoolFilters`) and the placed containers as a fourth, derived one
  (`lootContainers`). Version 2 is not released, so no bump. Facts only: GUIDs, Record names,
  weights, counts and tag links. The `name` of an entry is a designer's label, so no field is a
  game text.
- A table or archetype keeps every weight, the `optionalData` the Archive has for it (priority,
  choice limit, dupe limit, chance to exist, stack size, spawn-with) and the selector of each
  entry. A selector is read by its fields (`tags`, `entityClasses`) and an optional data block by
  the field it holds, not by the name of its struct.
- The older `LootTable` and `LootArchetype` Records (161 and 233) are the kinds `legacyLootTables`
  and `legacyLootArchetypes` (#818): 635 of the 1,958 slots with a loot config on LIVE still name
  an older table in `lootTableGuid`, which resolves to `legacyLootTables`. They are two kinds of
  their own, not folded into the V3 kinds, as their shape differs (a primary group and secondary
  groups of tag entries instead of selectors). The two share the record folder with the V3 Records
  and are told apart by the struct. Secondary choices (manufacturer weights) are not exported; no
  slot names one.
- The loot of a slot is the `lootConfig` of a sub harvestable of a multi config. A slot without a
  `lootConfig` field has `null`.
- A placed container is found as the entity file (`entdata/*.entxml`) of a container that names a
  `multiConfigRef`; the container it places (`objectContainer`) holds the tag points. The walk
  starts at every container below `ObjectContainers/PU/system` and follows the containers the
  entity files place, each read once, so a loot area in `PU/loc` is located by the system
  container that reaches it. The catalog counts the tags of the entities of all `.soc` files of the
  placed container and does not state which tag point gets which slot. A container without a
  `.soc` has no tag points (`entityCount` 0) and is not warned about; a placed container the
  Archive lacks, a `.soc` that cannot be counted and a container that cannot be read are warned
  about (`unreadable_loot_containers`), as is an entity file that names several different
  containers or configs (`ambiguous_loot_entity_files`), of which the first in document order is
  used.
- The starmap object is that of the container the walk started in, as for the harvestable
  locations (ADR-0236): the nearest object at or above the Child that reaches it.

## Consequences

- A consumer joins `lootContainers[].multiConfigGuid` to `subHarvestableMultiConfigs`, a slot's
  `lootConfig.lootTableV3Guid` to `lootTables`, an entry's `archetypeGuid` to `lootArchetypes` and
  `lootConfig.poolFilterGuid` to `lootPoolFilters`, a slot's `lootConfig.lootTableGuid` to
  `legacyLootTables` and an older entry's `archetypeGuid` to `legacyLootArchetypes`; on LIVE every
  such link resolves.
- The build reads the entity files of every container the walk reaches, in parallel. The cost of
  collecting the loot below each root is the size of its subgraph, once per distinct root; it is
  negligible on LIVE and not memoised further.
- `nodeType` and `selectorType` are informational: a consumer reads the other fields.
