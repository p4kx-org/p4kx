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
Split large tables at deterministic boundaries in their existing row order. Start a new part at
system changes for world placements where practical. A system may span several parts. Keep a row
and all its nested values together. Measure a row larger than the part target separately. Retain
the complete single-file Export.

## Proposed domains

| Domain | Canonical data | Example cross-domain links |
| --- | --- | --- |
| Shared definitions | Tags, manufacturers and resource types | Entity tags, manufacturers and resource references |
| Equipment and vehicles | Items, ships, loadouts, ports, equipment configurations, ammunition and weapon modes | `itemGuid` in blueprint results and shop inventory |
| Crafting and industry | Blueprints, tiers, ingredient positions, quality, mining and refining | Reward pools, resource types and equipment outputs |
| Contracts and reputation | Mission generators, templates, property sets, factions, requirements and rewards | Locations, spawn targets, items and blueprint pools |
| Economy and loot | Shops, inventory, loot tables and cargo manifests | Item, resource, mission and world-location identifiers |
| World and locations | Starmap definitions and placements, physical container and location occurrences, terrain and planetary configuration | Starmap definitions, parent routes, contextual presets and harvest configurations |

Assign each dataset one owning domain. Check an exhaustive manifest registry of JSON Pointers
against the Schema. Each entry records its shape, row order and key type for arrays and for
`{availability, rows}` objects. Include singleton values and empty datasets so reconstruction
retains every property. Keep parent routes and child instances in the world domain. Shared text
and provenance keep their namespaces across the snapshot. Their independently compressed parts
can be fetched in the dependency closure of a selected domain. Protected `gameTexts` use
independent parts and retain the `x-gameText` policy.

## Selective imports

The domain is the requested scope. Its dependency closure consists of the parts that contain
the rows it actually references. A table-level dependency tells the importer where to look. A
separate, independently readable index maps typed target keys to parts and row ordinals. Shard
this index by table and key range so small imports can fetch only the ranges they need.

- Index targets by table and its documented row identity. A GUID may map to several physical
  rows. Use the occurrence qualifier where the source gives one and preserve ambiguity where
  the logical link is ambiguous. An ordinal by itself only locates bytes in its own table and
  generation.
- Distinguish links that need a target's identity from links that need its values. A contract's
  Starmap identity can resolve in the index. Load the definition when the consumer needs its
  details. For a physical location, fetch its placement part, any required parents and the
  associated source evidence. The index stores locators and keeps protected text and authored
  payloads in their owning parts.
- Route `sourceUsesRefs`, physical source references and text references through the same
  generation. Fetch referenced inventory parts, then follow the dependencies of the rows used.
  Stage resolved link targets even when domains refer to each other. A consumer that only opens
  the requested domain can report an authored link as pending until those parts arrive.

For example, importing a Crafting recipe can resolve its finished item through
`definitions.items` and its resources through `definitions.resourceTypes`. A contract can use
the same recipe and fetch a location separately. The existing IDs and source markers travel
with each owning row. One complete bundle reconstructs the logical single-file Export.

## Referential integrity

- Prepare Catalog v3 once. Keep table names, selection, availability, ordering and row data. A
  separate `bundleVersion` describes packaging. `schemaVersion` describes the logical Catalog.
- The manifest binds one generation to the source Build tuple, archive receipt, export options and
  each part's relative path, table or row range, row count, stored and decoded length and SHA-256.
  Bind each table to its total row count and assign nonoverlapping half-open row ranges that
  cover that total. Validate unique part IDs and physical row keys while preserving repeated
  authored values at distinct positions. Include publication mode, `exportHealth`, source and
  selection coverage per dataset, and the table dependency index. Bind the separately readable
  target index to the same manifest with a digest for each shard. Validate parts against their
  manifest before combining them.
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
- Stage the component files at immutable generation-specific paths and publish the manifest
  last. Verify digests and reference closure before making an imported scope visible. Failure
  preserves the previous usable snapshot and its part paths.

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
publication and partial replacement. Check that an equipment-to-crafting join loads zero world
placement or terrain parts, and that resolving a single world location loads just its indexed
parts and dependencies. Measure stored and decoded bytes, generation time and peak producer and
consumer memory. Record selective bytes transferred, including indexes and dependency parts.
The current writer streams serialization from a prepared catalog.
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
