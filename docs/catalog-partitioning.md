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

A second unfiltered trial used the verified Windows AMD64 release producer for
`0e1da3f4a` on the same LIVE Archive. Peak sampled working set during `records`
was 32.37 GiB, down from 36.72 GiB. The run passed `project-contract-generators`
and reached `project-configurations`, where the 12 GiB physical-memory reserve
stopped it at 44.87 GiB working set and 11.96 GiB available on the host. It again
stopped before JSON sizing or publication. The reserves apply to the host's
available physical memory and system commit headroom in the measurement harness.
[Release producer](https://github.com/p4kx-org/p4kx/actions/runs/43813424470/job/133122281746)

A third unfiltered trial used the Windows AMD64 release artifact of merge `867e6b9d9`
on the same LIVE Archive. The peak sampled working set in `records` was 32.39 GiB.
The process reached 41.79 GiB during `project-contract-generators`, also the sampled
peak of that phase in the prior trial. Available physical memory reached 11.97 GiB
while system commit headroom was 38.01 GiB. The physical-memory reserve stopped the
process before JSON sizing or publication. The trial preceded the singleton Entity
class index and prefab-inventory ownership changes.
[Release producer](https://github.com/p4kx-org/p4kx/actions/runs/43826004006/job/133160321493)

A fourth unfiltered trial used the Windows AMD64 release artifact of merge
`bcd39e238` on the same LIVE Archive with four Rayon workers. Compared with
the third trial, the sampled working-set peak fell from 32.39 to 28.05 GiB
during `records` and from 41.79 to 36.37 GiB during
`project-contract-generators`. Available physical memory at launch was
50.41 GiB, compared with 50.34 GiB in the third trial. The new run passed
the contract and configuration phases and reached `project-weapon-animation`.
The local reserve stopped it at 40.61 GiB working set and 42.04 GiB private
bytes, with 11.81 GiB physical memory and 36.78 GiB system commit headroom
available. The writer had yet to size or publish JSON.
[Release producer](https://github.com/p4kx-org/p4kx/actions/runs/43837672769/job/133201840292)

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

Basetool at `470f3bc` admits LIVE Catalog v2 with a 64,000,000-byte upload limit.
Its default decoded limit is 1,000,000,000 bytes. The separate 128,000,000-byte default
limits the aggregate JSON spans the reader retains for known kinds. These are reference
values for the v3 sizing work. Basetool's v3 importer and its acceptance run are tracked
in its own repository. Its issue lists P4KX 0.5.0 as a prerequisite and keeps the LIVE
channel policy. Use a revision-bound LIVE release candidate for the positive import
and verify the expected PTU channel refusal separately.
[Upload gate](https://github.com/krt-profit/basetool/blob/470f3bc09a7a01bc7054da7a53243bc3f351b1aa/backend/src/main/java/de/greluc/krt/profit/basetool/backend/catalogue/internal/P4kUploadGate.java)
· [Import properties](https://github.com/krt-profit/basetool/blob/470f3bc09a7a01bc7054da7a53243bc3f351b1aa/backend/src/main/java/de/greluc/krt/profit/basetool/backend/catalogue/internal/P4kImportProperties.java)
· [Basetool v3 migration](https://github.com/krt-profit/basetool/issues/3107)

VerseKit 5.4.1 at `e526984` still pins P4KX 0.4.2. It reads Blueprints and Item values
from DataForge and builds filtered catalogs through the Rust library for data such as
contracts and ship slots. Its `GameData::catalog_all` library path returns a `Vec<u8>`
and refuses encrypted DataForge Entries. A v3 compatibility witness needs the actual
library or artifact path, exact producer and consumer revisions, measured size and
peak consumer memory.
[VerseKit dependency](https://github.com/Xharig/VerseKit/blob/e526984b7ee1946403670b193740de5a49fe2434/Cargo.lock#L2808-L2810)
· [Game data code](https://github.com/Xharig/VerseKit/blob/e526984b7ee1946403670b193740de5a49fe2434/crates/versekit-gamedata/src/lib.rs#L376-L402)

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

## 16 GiB total system RAM

P4KX 0.5.0 release acceptance requires a complete unfiltered LIVE Catalog and
a complete unfiltered current PTU Catalog on a system with 16 GiB of total
physical or assigned RAM. The operating system, desktop, background services
and every other running process share that RAM with P4KX. Use the normal
packaged command and ordinary OS paging settings. Keep the usual desktop
and background workload active throughout each run and record those
conditions with the baseline. Each result must retain every field,
language, cross-domain reference and source marker of the complete v3
document.

Record total system RAM, OS, CPU and the running system's baseline memory
before launch. Measure the producer's peak working set and private bytes,
the lowest available physical memory, system commit and paging, elapsed
time, process bytes written and peak temporary storage.
Check that the desktop remains usable for the usual light work throughout
the export. Record any stalls alongside the system paging measurements.
For a virtual machine, record the fixed guest RAM allocation and the RAM
visible to the guest. Keep that allocation fixed throughout the run and
check host memory pressure and paging separately.

Run the artifact replay separately. Derive the raw verification artifact from
the same compressed output so the test prepares the Catalog only once. A
process quota or reserve on a larger machine measures a different condition.
[Catalog work](https://github.com/p4kx-org/p4kx/issues/1151) ·
[Release acceptance](https://github.com/p4kx-org/p4kx/issues/1116)

## Measuring a complete export

For each Build, record the full Build identity, exporter revision and hash, and archive length
and modification time before and after the export. Run the normal `p4kx catalog` writer with a
fixed export time, default text selection, `--progress-phases` and one private `.json.zst` output.
The completed writer reports `catalog storage:` on stderr. It sums the logical bytes of the
temporary row spools for container instances, locations, Entity tag declarations,
Item and Blueprint definitions, particle libraries,
contract generators, contract templates, mission broker entries, item ports,
interactions, inventory item bounds, durability and weapon animation candidates, plus the
final bytes of the publication temporary. The spools stay
open until publication. The measurement includes open spools with no visible file name. On an
interrupted run, a dedicated scratch volume can provide sampled allocation evidence.
Capture stderr line by line to align process memory samples with preparation, health, provenance,
JSON size measurement and writing. Within input preparation, record `open-dataforge`,
`read-localization`, `index-localization` and `initialize-origins` as separate phase starts.
Within `projections`, record `resolve-loadouts`, `project-locations`, `project-missions`,
`read-mission-dependencies`, `project-mission-definitions`, `project-contract-templates`,
`project-mission-broker-entries`, `project-mission-location-templates`,
`project-contract-generators`, `link-mission-factions`, `project-configurations` and
`finalize-projections` as separate phase starts. Record the configuration phases `project-shops`,
`resolve-jump-operators`, `read-independent-configurations`, `project-particle-libraries` and
`project-weapon-animation` separately. During finalization, distinguish
`project-blueprint-tables`, `resolve-cargo-capacities`, `project-facility-loadouts`,
`project-facility-interiors` when present, `project-resource-topology` and
`project-starmap-placements`.
Pending localization keys retain per-language lookup outcomes and their source uses.
Finalizing the text registries reconstructs the text forms from the in-memory localization index.
These lookups add CPU work and require no temporary storage or additional Archive reads.
Catalog health retains the original coordinates and target identity in compact reference proofs.
It constructs the full diagnostic record when reconciliation needs to report a reference.
Clean field inspections store one optional findings pointer. An allocation holds the
diagnostic vectors only when they contain findings. Captured Record subjects share their
key with the original header receipt and clone the handle. Layout tests verify both
representations and preserve the original evidence and authority checks.

Item definitions and Entity tag declarations are compressed individually as the Record
scan produces them. The publisher replays the indexed frames in canonical GUID and
Record order for each JSON pass. The weapon animation join keeps its original sourced
attachment fields in memory.
Measure both the spool bytes and the cost of seeking to the ordered frames.
Inventory item bounds are staged during the same scan in physical Record and Component order.
Inventory storage selections stay in memory for the capacity and facility joins.

After each Record and ContractGenerator window and at selected projection boundaries, the
source registry packs unheld DataForge uses into compact coordinates. It keeps live handles and their original authority. A later observation
at the same coordinate reuses the original ID and evidence. The publisher streams the
sorted live and packed rows together with the same IDs, scope, role and encryption.
This storage uses RAM and adds no provenance spool.

Record peak working set and peak private bytes separately.
Sample host commit and paging counters alongside the process write counter to distinguish process
writes from system memory pressure. Record the exit status, elapsed time, file length and decoded
byte count from the text summary. Count section and decoded bytes by streaming the output through a Zstd
decoder. Reconcile section totals and JSON framing with the writer's byte count. Measure reader
memory in a separate process. Diagnostic results and build or source changes invalidate the
complete measurements.

A normal publication check on LIVE `13165946` at `c9066f14a` refused three required
Starmap container enumeration operations and 30 separate Loot container enumeration operations.
A read-only source audit at `bfc242553` followed the original authoring routes and checked the
root and containing directories. The three Starmap selectors had no candidate in those
directories. Twelve Loot selectors shared a basename with a root Entry. Basename equality
alone does not establish a valid authored target. The verifier inspected the refusal directly
and created no Catalog file. The comparable probe on PTU `13201184` at `bfc242553`
found ten required Starmap enumerations and 37 distinct Loot enumerations. Twelve Loot
selectors shared a basename with a root Entry. The attempts establish the failure scopes
for the two Builds.

The older unfiltered LIVE trial at `bed4144d` peaked at 22.53 GiB working set and
23.31 GiB private bytes during the Record scan, then 33.09 GiB working set and
34.36 GiB private bytes during facility interior projection. It refused publication
after preparing the catalog and recording health. The 16 GiB acceptance still requires a
complete normal export with the current packaged producer on the specified system.

The discard-sink witness at `4c601317a` checked both installed Archives with four Rayon
threads and stable Archive stamps. It refused normal publication for each Build after health
assessment. The values below are sampled process memory in GiB on a larger host.

| Build | Record scan working set | Record scan private bytes | Peak working set | Peak private bytes | Packed source uses at health |
| --- | ---: | ---: | ---: | ---: | ---: |
| LIVE `13165946` | 15.47 | 16.08 | 26.22 | 27.60 | 68,920,610 |
| PTU `13201184` | 16.30 | 16.95 | 28.02 | 31.73 | 72,144,536 |

LIVE reached a sampled 26.22 GiB working set during health assessment. PTU reached its
peak working set during provenance. Both sampled private-byte peaks occurred during
provenance. The packed source-use counts grew by 17,325,165 and 17,974,970 between the
Record scan and health. The pair logged 0.756 GiB of logical process writes and produced
no Catalog file or encoder sizes. The later projection compaction checkpoints require
a new measurement on a producer that includes them.

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
A `catalog retained:` line reports distinct registered source uses and the packed subset
at the start of `records`, `projections`, selected projection phases and `export-health`.
The inspected ledgers at each checkpoint supply the field inspection count, capacity
and reserved inline bytes.
Each inspection's nested allocations
and the source registry's own allocation footprint require separate measurements.
The witness reports decoded and encoded sizes on stdout without creating a catalog file.

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
Run the provenance replay as a separate process.

```text
cargo test --release --locked -p p4kx-export --test real_archive -- --ignored --exact --nocapture real_archive_catalog_size::replay::replay_completed_catalog_provenance_and_sections
```

Replay checks the measurement's matching archive stamps and the export's Build, Channel, archive
length and modification time. It requires schema v3, complete publication health and an unfiltered
selection. The two decoding passes check the receipt lengths and compare SHA-256 digests of the
decoded bytes. Replay checks source-use IDs, original physical source ancestry, localization selector
roles and per-value encryption markers against the retained inventory. It rejects complete source
sets that contain opaque coordinates.

The report includes exact section byte lengths, provenance inventory counts and sourced-value
counts. Row counts cover `/items`, `/definitions/items/rows`, `/definitions/blueprints/rows` and
`/placements/containerInstances/rows`. Replay opens no installed Archive and creates no raw file.
Cross-domain link validation and consumer import measurements remain separate release evidence.

To produce the raw artifact from the same completed measurement, run the reconciliation test.

```text
cargo test --release --locked -p p4kx-export --test real_archive -- --ignored --exact --nocapture real_archive_catalog_size::artifact::reconcile_completed_catalog_artifact
```

One streaming decode produces the section and reference-set census and copies the exact
UTF-8 bytes to `p4kx-catalog.json` in that directory. Publication requires the encoded
and decoded lengths to agree with the writer receipt. An existing raw file is preserved.
The installed Archive is unnecessary for this check.
