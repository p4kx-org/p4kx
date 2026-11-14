# Catalog domain partitioning

The installed LIVE `13165946` Starmap diagnostic export was 2.811 GB of decoded JSON. The
PTU `13189690` Nyx Starmap diagnostic export was 2.995 GB. Both measurements predate compact
source-use IDs. These Filter-scoped exports establish section sizes. Current complete output size,
compression and peak memory still need to be measured. [Size investigation](https://github.com/p4kx-org/p4kx/issues/1151)

| Section | LIVE | PTU |
| --- | ---: | ---: |
| `placements.containerInstances` | 1.734 GB | 2.046 GB |
| `provenance.uses` and `provenance.sources` | 0.678 GB | 0.659 GB |
| Definitions | 0.177 GB | 0.123 GB |
| `projections.tagPaths` | 0.146 GB | 0.153 GB |

Container placements alone exceed the 1 GB decoded reference threshold in both captures. Their
source-use arrays make up roughly three quarters of that section. Moving only that table would leave
1.077 GB in the LIVE file and 0.949 GB in the PTU file, including the full provenance inventory.
Split large tables at deterministic
row boundaries, by system and then original occurrence order for world placements. Keep a row and
all its nested values together. Retain the complete single-file Export.

## Proposed domains

| Domain | Canonical data | Example cross-domain links |
| --- | --- | --- |
| Shared definitions | Tags, manufacturers, resource types, lightweight Starmap identities, public names | Entity tags, location scopes and resource references |
| Equipment and vehicles | Items, ships, loadouts, ports, equipment configurations, ammunition and weapon modes | `itemGuid` in blueprint results and shop inventory |
| Crafting and industry | Blueprints, tiers, ingredient positions, quality, mining and refining | Reward pools, resource types and equipment outputs |
| Contracts and reputation | Mission generators, templates, property sets, factions, requirements and rewards | Locations, spawn targets, items and blueprint pools |
| Economy and loot | Shops, inventory, loot tables and cargo manifests | Item, resource, mission and world-location identifiers |
| World and locations | Physical container and location occurrences, Starmap placements, terrain and planetary configuration | Starmap definitions, parent routes, contextual presets and harvest configurations |

Assign each table one owning domain, with its exact mapping in the manifest. Keep parent routes
and child instances in the world domain. Shared text and provenance keep their namespaces across
the snapshot. Their independently compressed parts can be fetched in the dependency closure of a
selected domain. Protected `gameTexts` use independent parts and retain the `x-gameText` policy.

## Referential integrity

- Prepare Catalog v3 once. Keep table names, selection, availability, ordering and row data. A
  separate `bundleVersion` describes packaging. `schemaVersion` describes the logical Catalog.
- The manifest binds one generation to the source Build tuple, archive receipt, export options and
  each part's relative path, table or row range, row count, stored and decoded length and SHA-256.
  Bind each table to its total row count and assign nonoverlapping half-open row ranges that
  cover that total. Validate unique part IDs and physical row keys while preserving repeated
  authored values at distinct positions. Include publication mode, `exportHealth`, source and
  selection coverage per dataset, and the table
  dependency index. Validate parts against their manifest before combining them.
- Keep GUIDs, physical Record/component ordinals, tier/slot and Loadout paths, and container
  occurrence keys. Source-use IDs are opaque and valid within their generation. Follow
  `sourceUsesRefs` from selected rows, texts and retained inventories until no additional use,
  selector, source or ancestor remains. Physical source metadata and TextForms carry their
  own evidence. Keep health metadata with its atom table. Preserve
  `containsEncryptedSource` and `sourceSetComplete` on each datum.
- Track download state separately from source state. An unfetched part, a Filter-excluded target,
  a dangling link and unreadable source have distinct meanings. Resolve staged parts and cyclic
  links before applying records. Stage identities in their own generation. A physical Record index
  can only resolve in that generation. Deletion requires the intended build, unfiltered selection,
  every expected part, complete source inventory and Catalog-wide health, including global blockers.
- Stage the component files and publish the manifest last. Verify digests and reference closure
  before making an imported scope visible. Failure preserves the previous usable snapshot.

## Transport options

| Option | Benefits | Costs |
| --- | --- | --- |
| Manifest and independent `.json.zst` parts | Selective and resumable downloads, bounded file sizes | New writer, reader and verification contract. Separate frames lose cross-part compression. |
| Stream complete JSON into a staged import | Lower consumer peak memory with the existing contract | Transfers the complete catalog. Decoder limits and data buffering still need measurement. |
| Archive containing the parts | One artifact for distribution | Selective fetch depends on its index and range support. Packing compressed parts adds little compression. |

Use manifest and parts for selective imports. Measure the current complete export first. Prototype
the world domain with shared inventories and one equipment-to-crafting join. Compare reconstructed
logical Catalog v3 with the single-file export, including row order and source closure. Test
wrong-generation parts, missing dependencies and nested evidence, overlapping or missing row ranges,
dangling and cyclic links, protected text, a global health blocker alongside a healthy domain, cancelled
publication and partial replacement. Measure stored and decoded bytes, generation time and peak
producer and consumer memory. The current writer streams serialization from a prepared catalog.
Smaller files alone leave that producer preparation cost in place.

## Measuring a complete export

For each Build, record the full Build identity, exporter revision and hash, and archive length
and modification time before and after the export. Run the normal `p4kx catalog` writer with a
fixed export time, default text selection and one private `.json.zst` output. Record the exit
status, elapsed time, peak process memory, file length and decoded byte count from the text
summary. Count section and decoded bytes by streaming the compressed output through a Zstd
decoder. Reconcile section totals and JSON framing with the writer's byte count. Measure reader
memory in a separate process. Diagnostic results and build or source changes invalidate the
complete measurements.

The compressed writer prepares a typed Catalog once. It first counts serialized bytes for the
frame header and then streams serialization into the encoder. A second raw file adds storage
without changing the measured decoded bytes. A comparison with raw writer timing is a separate
experiment.
