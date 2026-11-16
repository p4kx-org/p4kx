# Catalog domain partitioning

The installed LIVE `13165946` Starmap diagnostic export was 2.811 GB of decoded JSON. The
PTU `13189690` Nyx Starmap diagnostic export was 2.995 GB. Both measurements predate compact
source-use IDs. These Filter-scoped exports establish section sizes. Current complete output size,
compression and peak memory still need to be measured. [Size investigation](https://github.com/p4kx-org/p4kx/issues/1151)

An unfiltered Windows AMD64 LIVE trial using the release producer for `2f134fbfc`
reached 46.68 GiB of sampled peak working set during `project-contract-generators`.
The host fell below the 12 GiB physical-memory reserve before JSON sizing or publication,
so this run provides no encoded or decoded size. The process working set grew from 6.10 GiB
to 36.70 GiB during `records` and from 40.38 GiB to 44.30 GiB during
`project-mission-broker-entries`. [Release producer](https://github.com/p4kx-org/p4kx/actions/runs/43803352398/job/133089995250)

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
and all its nested values together. Treat the requested part size as a target and record an
oversized row with its actual lengths. Test against decoded bytes, stored bytes and consumer
limits separately. Retain the complete single-file Export.

## Consumer reference limits

Basetool at `0621e8f` admits LIVE Catalog v2 with a 64,000,000-byte upload limit.
Its default decoded limit is 1,000,000,000 bytes and its default retained JSON span limit
is 128,000,000 bytes. These are reference values for the v3 sizing work. Basetool's
v3 importer and its acceptance run are tracked in its own repository. Its issue
lists P4KX 0.5.0 as a prerequisite. A revision-bound release candidate is needed
to run the cross-repository acceptance before final publication.
[Upload gate](https://github.com/krt-profit/basetool/blob/0621e8fca89fdb9188fe89d1d1ec391dea4c8eaf/backend/src/main/java/de/greluc/krt/profit/basetool/backend/catalogue/internal/P4kUploadGate.java)
· [Import properties](https://github.com/krt-profit/basetool/blob/0621e8fca89fdb9188fe89d1d1ec391dea4c8eaf/backend/src/main/java/de/greluc/krt/profit/basetool/backend/catalogue/internal/P4kImportProperties.java)
· [Basetool v3 migration](https://github.com/krt-profit/basetool/issues/3107)

VerseKit at `b25f486` depends on P4KX 0.4.2. It reads Blueprints and Item values from
DataForge and builds filtered catalogs through the Rust library for data such as contracts
and ship slots. Its v3 consumer verification needs to identify the actual library or
artifact path, with the exact producer and consumer revisions.
[VerseKit dependency](https://github.com/Xharig/VerseKit/blob/b25f486d1da9f7e633cab4f1e150c65d83cd8f42/Cargo.toml)
· [Game data code](https://github.com/Xharig/VerseKit/blob/b25f486d1da9f7e633cab4f1e150c65d83cd8f42/crates/versekit-gamedata/src/lib.rs)

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
retains every property. Register nested collections inside singleton objects independently
when they need file boundaries, including their parent path and collection order. Keep parent
routes and child instances in the world domain. Shared text and provenance keep their namespaces
across the snapshot and can be fetched within a domain's dependency closure. Protected
`gameTexts` use independent parts. Apply `x-gameText` throughout the bundle, including inline
values such as a container's authored label.

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
`definitions.items` and its resources through `definitions.resourceTypes`. A contract that
awards from a Blueprint pool can look up the pool and fetch any required locations separately.
The existing IDs and source markers travel with each owning row. A part is the fetch unit.
An importer follows the dependencies of the actual rows it needs from that part. Following
every row in a fetched dependency part would make a narrow query load unrelated domains.
One complete bundle reconstructs the logical single-file Export.

## Referential integrity

- Prepare Catalog v3 once and partition that prepared snapshot. A separate filtered Export per
  domain would change its selection, provenance inventory and document-local identifiers. Keep
  table names, selection, availability, ordering and row data. A separate `bundleVersion`
  describes packaging. `schemaVersion` describes the logical Catalog.
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
  own evidence. Preserve the entire provenance inventory in a complete bundle, including
  observations without an incoming reference. Keep health metadata with its atom table. Preserve
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
Smaller files alone leave that producer preparation cost in place. If the prepared catalog
exceeds memory before publishing, profile source reading and materialization as their own
work. Keep those measurements distinct from the package reader and the transfer benchmark.

## Measuring a complete export

For each Build, record the full Build identity, exporter revision and hash, and archive length
and modification time before and after the export. Run the normal `p4kx catalog` writer with a
fixed export time, default text selection, `--progress-phases` and one private `.json.zst` output.
Capture stderr line by line to align process memory samples with preparation, health, provenance,
JSON size measurement and writing. Within input preparation, record `open-dataforge`,
`read-localization`, `index-localization` and `initialize-origins` as separate phase starts.
Within `projections`, record `resolve-loadouts`, `project-locations`, `project-missions`,
`read-mission-dependencies`, `project-mission-definitions`, `project-contract-templates`,
`project-mission-broker-entries`, `project-mission-location-templates`,
`project-contract-generators`, `link-mission-factions`, `project-configurations` and
`finalize-projections` as separate phase starts.
Record peak working set and peak private bytes separately.
Sample host commit and paging counters alongside the process write counter to distinguish process
writes from system memory pressure. Record the exit status, elapsed time, file length and decoded
byte count from the text summary. Count section and decoded bytes by streaming the output through a Zstd
decoder. Reconcile section totals and JSON framing with the writer's byte count. Measure reader
memory in a separate process. Diagnostic results and build or source changes invalidate the
complete measurements.

The compressed writer prepares a typed Catalog once. It first counts serialized bytes for the
frame header and then streams serialization into the encoder. A second raw file adds storage
without changing the measured decoded bytes. A comparison with raw writer timing is a separate
experiment.

For byte sizing without creating a catalog file, set `P4KX_DATA_P4K` to LIVE and
`P4KX_DATA_P4K_B` to PTU with the ordinary installed-Archive key configuration. Run the focused
installed-pair test.

```text
cargo test --release --locked -p p4kx-export --test real_archive -- --ignored --exact --nocapture real_archive_catalog_size::normal_catalog_sizes_for_installed_pair
```

The test checks the source Build and Archive stamp, measures the same encoder with a fixed time
and normal health policy, and discards the frame as it is written. Capture the exact producer
revision and binary hash separately.

The sink-only library witness also reports the channel, Build change number and producer
phase on stderr. Use those events to align process memory samples with the producer stages.
It reports the decoded and encoded sizes on stdout without creating a catalog file.

```text
cargo test --release --locked -p p4kx-export --lib -- build::api::size_witness::normal_catalog_size_phases_for_installed_pair --ignored --exact --nocapture
```

Set `P4KX_CATALOG_SECTION_CENSUS=1` for section sizes and provenance counts in the same run.
The writer sends the finished Zstd stream through a bounded pipe. The reader checks the
encoded and decoded lengths, counts rows in both provenance inventories, and reports the exact
compact UTF-8 lengths of the top-level members and their registered section members.
Report framing separately and keep nested measurements inside their parent counts.
Source-use arrays are counted by byte length and reference occurrences. Repeated nonempty arrays
are grouped by SHA-256 fingerprint of the emitted array bytes. Their sorted, unique references
make identical arrays share a fingerprint. The measurement does not establish equality across
hash collisions.

The census keeps one counter per nonempty fingerprint and includes the decoder's use of memory
in the benchmark process. Use the sink-only run for producer-only memory measurements. Neither
mode requires another serialization pass. For atomic file publication and actual consumer import,
use the separate evidence above.

### Reconciling an existing measurement

Set `P4KX_CATALOG_MEASUREMENT_DIR` to the private directory containing the completed
normal writer receipt `receipt.private.json` and its `p4kx-catalog.json.zst` output.
Then run the artifact reconciliation as a separate process.

```text
cargo test --release --locked -p p4kx-export --test real_archive -- --ignored --exact --nocapture real_archive_catalog_size::artifact::reconcile_completed_catalog_artifact
```

One streaming decode produces the section and reference-set census and copies the exact
UTF-8 bytes to `p4kx-catalog.json` in that directory. Publication requires the encoded
and decoded lengths to agree with the writer receipt. An existing raw file is preserved.
The installed Archive is unnecessary for this check.
