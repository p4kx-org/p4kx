# Catalog domain partitioning

## Complete LIVE and PTU measurements

The packaged Windows AMD64 producer at `37605ecae` completed a normal, unfiltered export for
installed LIVE `13165946` with two Rayon workers, all eleven installed languages, and Zstandard
level 3. It wrote 1,206,312,950 encoded bytes with 82,668,352,262 decoded bytes pledged in the frame.
A streaming read verified the checksum, decoded length and single-frame boundary, and measured
the canonical sections. It counted 93,806 physical sources and 88,689,710 source uses. The
section census does not read an independent raw export or validate reference closure.

The producer ran for 19 minutes 27 seconds on a Windows host with 93.67 GiB of system-visible RAM.
Windows recorded a 26.28 GiB process working-set peak. Sampled available system RAM stayed at
or above 25.99 GiB, with at least 69.52 GiB of commit headroom. The sealed row spools held
472,249,284 bytes. Windows reported 1,678,571,916 logical process write bytes, including the
compressed publication. The two historical working-set peaks are below the
current 40 GB limit. Use the candidate Build and binary with all required
memory counters for release acceptance.

| Decoded section | LIVE bytes | PTU bytes |
| --- | ---: | ---: |
| `loadouts` | 30,674,479,268 | 30,687,183,746 |
| `provenance` | 22,358,692,968 | 23,679,210,011 |
| `projections.weaponPlayerAnimationCandidates` | 8,587,490,937 | 8,877,122,157 |
| `configurations.interactions` | 5,000,644,330 | 5,316,102,335 |
| `definitions` | 4,875,971,186 | 5,036,210,389 |
| `placements` | 3,226,353,067 | 3,508,867,924 |
| `texts` | 1,663,834,064 | 1,787,491,424 |

Loadouts, provenance and weapon animation candidates together account for 74.54% of the decoded
LIVE document. The target index and source-use inventory need independent parts. Whole-table grouping
alone would leave several tables above the 1,000,000,000-byte decoded reference limit. At that
limit LIVE would need at least 83 parts and PTU at least 86, before bundle overhead.
Measure each encoded part separately because separate frames change compression.

The size comes from JSON records and their source evidence. A resolved Loadout includes the
ports and attachments under its specific owning entity, including contextual overrides. Each
weapon animation candidate retains the weapon, actor, original database and tag-token match
coordinates. The provenance inventory records 88.69 million source-use relationships separately
from the physical sources. Texture pixels, 3D mesh buffers and audio samples are exported on
request through the [normal file and media exporters](catalog.md#media-files).

The first normal PTU `13228337` run stopped at the publication check on two unmapped
mission-phase values. After mapping the original values, the packaged Windows AMD64 producer at
`1e704c0ae` completed the normal, unfiltered export of the same installed Build. At Zstandard
level 3, the file is 1,287,882,399 encoded bytes, with 85,600,368,941 decoded bytes pledged in
the frame. With two Rayon workers, the producer took 21 minutes 14 seconds and reported a
27.54 GiB Windows working-set peak. Sampled host availability and commit headroom remained at
or above 29.30 GiB and 74.57 GiB respectively. Windows reported 1,807,985,951 logical process
write bytes. The PTU byte census verified the frame and counted 114,513 physical sources and
93,861,565 source uses. [Size investigation](https://github.com/p4kx-org/p4kx/issues/1151)

## Reducing the uncompressed document

The normal writer already emits compact JSON. Domain partitioning changes which data a consumer
fetches. Reducing the total decoded size requires a more compact representation of repeated data.

| Target | Lossless representation | Evidence |
| --- | --- | --- |
| `loadouts` | Store each observed Vehicle XML selection sequence once. A field keeps its local uses and a reference to the exact prefix of that sequence. | The 30.67 GB section contains 10,803 trees and 284,449 ports including descendants. The producer shares selected prefixes in memory and expands them in the JSON of multiple port attributes. |
| `provenance.uses` | Use a typed table with a column declaration and compact row arrays. Encode the three closed scope, role and encryption vocabularies with one-digit codes. | The exact 88,689,710 use rows repeat six required keys of 62 bytes per row. The keys account for 5,498,762,020 bytes. The three vocabularies save at least 2,128,553,040 more bytes with the proposed codes. |
| `projections.weaponPlayerAnimationCandidates` | Store physical weapon, actor, branch and clip-option endpoints once and retain the ordered match relationships by reference. | The 8.59 GB section contains candidate relationships with repeated endpoints and original tag evidence. |

The model for the provenance table alone reduces the current 82.67 GB LIVE document to about
75.04 GB before the small table declarations. Its values, exact coordinates and original IDs
would remain recoverable. Shorter opaque IDs and dictionaries for repeated coordinate names are
additional candidates. Each byte saved per use ID removes 88,689,710 bytes from the use inventory,
plus its occurrences in references. Measure those occurrences and the complete dictionary cost
before choosing an encoding.

A byte census of the completed LIVE file attributed 29,873,701,416 bytes to the
`sourceUsesRefs` arrays for `displayNameKey`, `flags`, `maxSize`, `minSize`, `portTags` and
`requiredTags`. Those arrays contain 690,731,520 reference occurrences. They account for 97.39%
of `loadouts` and 36.14% of the complete decoded file. The census verified the frame and
reconciled its disjoint counts against all 30,674,479,268 bytes in `loadouts`.

In PTU `13228337`, the same six arrays occupied 29,871,113,599 bytes across 10,340 trees and
290,711 ports. The string-compatible encoder at `e29c1ffd5` emits shorter opaque IDs for
known physical CryXml and text XML coordinates. The PTU census priced 509,833,062 eligible
ID occurrences in those arrays using their original coordinate shapes and exact replacement
lengths. The modeled reduction in those six fields is 11,303,107,764 bytes. Applying only
that reduction yields 74,297,261,177 decoded bytes for the complete PTU Catalog.

The paired native Windows AMD64 producer at `8a23763e`, from integration head `e29c1ffd5`,
completed on the same PTU Archive stamp and options. The decoded size fell from 85,600,368,941
to 74,174,978,158 bytes, a reduction of 11,425,390,783 bytes (13.35%). The Zstandard level 3
file fell from 1,287,882,399 to 1,282,450,554 bytes, a reduction of 5,431,845 bytes (0.42%).
A separate read of the completed candidate frame verified the checksum and exact decoded size.
It reconciled both runs at 114,513 physical sources, 93,861,565 source uses, 10,340 loadouts,
290,711 ports and 690,838,290 references across the six dominant fields. The six arrays saved
exactly 11,303,107,764 bytes. Source-use IDs are document-local strings and each field carries
its original schema-v3 encryption and completeness markers.

On the compact PTU baseline, `provenance.uses` occupies 23,581,982,024 bytes.
Applying the positional-row and vocabulary model to its 93,861,565 observed rows
would save at least 8,072,094,590 bytes before table declarations. `loadouts`
occupies 19,384,075,982 bytes, including 18,568,005,835 bytes in the six port-field
reference arrays. Pooling the shared selector prefixes targets that remaining
repetition. The candidate animation relations occupy 8,877,122,157 bytes.

A deterministic one-to-one remap to nine-character ASCII identifiers would retain the current
string schema. It bounds the six arrays at 8,292,191,628 bytes including quotes, separators
and brackets, a reduction of at least 21,581,509,788 bytes for this LIVE inventory. The
identifier strings would change. Each must still resolve to the same source, location, role
and encryption state throughout the generation. The calculation excludes the rest of the
Catalog and the runtime cost of the mapping. It does not reduce the number of reference
occurrences. Sharing prefixes reduces that number, so the savings from the two approaches
overlap.

In a synthetic 128-port model, six attributes per port used the same growing selector prefix
and different local uses. All 768 complete lists were distinct. Sharing the prefixes reduced
1,451,401 bytes to 134,582 bytes and expanded to the exact original values, immediate markers
and sorted unique use IDs. The LIVE census measures the current arrays. Installed-build
savings depend on the observed prefix boundaries, local-use sizes and replacement encoding.

Keep `containsEncryptedSource` and `sourceSetComplete` directly at each datum. The compact
representation must explicitly identify its encoding and resolve every local use, exact
selector prefix, physical source and ancestor in the same generation. Match a prefix against
the observed selection sequence. Provenance inventory order is an independent sort order.
Whole-list interning and prefix sharing can coexist. Text and content dictionaries preserve
the existing public and protected text classification.

Compare the full normalized rows, physical occurrence order, original IDs, expanded use sets,
health and encryption states against the existing writer on synthetic fixtures and the same
installed Builds. Measure decoded and encoded sizes, reader and writer memory, and selective
reference closure. A required wire-shape change needs updated contract and consumer fixtures.
The typed SDK can keep returning the existing domain values and source observations.

## Earlier sizing runs

The installed LIVE `13165946` Starmap diagnostic export was 2.811 GB of decoded JSON. The
PTU `13189690` Nyx Starmap diagnostic export was 2.995 GB. Both Filter-scoped measurements
predate compact source-use IDs.

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

The Windows AMD64 native package at `9ef12b019` completed preparation for installed
LIVE `13165946` with two Rayon workers. Windows reported a 25.34 GiB process
working-set peak. The host retained at least 21.07 GiB of sampled available RAM.
The process reached `export-health` after about 15 minutes and stopped on an
accounting error in the harvest dependency of a verified missing Starmap target.
The memory reserve remained available. No Catalog was published, so decoded and
compressed sizes still require a complete run. The regression is covered by the
Normal publication test on the integration branch. [Measurement](https://github.com/p4kx-org/p4kx/issues/1151#issuecomment-7009858377)

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

### Local consumer refresh

VerseKit 5.4.1 embeds the P4KX 0.4.2 Rust libraries. It uses raw DataForge and
localization readers and path-filtered calls for contracts, mission locations
and ship inventory. Its overlay also rebuilds the local mining-location cache
when the installed Archive stamp or cache format changes. That refresh uses
`GameData::catalog_all` to build the entire v2 JSON in memory. Its mining projection
uses mineable elements, compositions, provider presets, harvestable locations
and Starmap objects, plus names from DataForge and localization.

For v3, read the authored Starmap definition and its parent chain from
`definitions.starmapObjects`. Resolve placement rows where the projection needs
the physical placement. The existing path Filter selects Records and still
permits substantial world and shared data preparation. Streaming the full
Catalog removes the in-memory JSON buffer while retaining full preparation.

The local mining-location SDK uses `read_mining_locations` for the reachable
Starmap dependencies and the actual mining-resource joins. Its coverage counts verified
missing external references by placement occurrence. Those references were authored, and an
exhaustive directory lookup found no target. Call `has_complete_placement_inventory()` before
using missing placements to remove entries from a consumer cache. Check harvest and definition
coverage separately before replacing those results. `with_mineable_entities` reads physical
scanner definitions and their original scan selectors. The
standalone `read_planet_resources` query reads terrain resource definitions by
physical Entry, path or GUID. The selected records carry immediate origin markers.
Compare the local projection with the same Build's complete Catalog.
[SDK queries](catalog-sdk.md) An alternative is a
Build-bound derived cache produced on the producer host and imported locally.
Measure cold generation or import, changed Build, unchanged cache, failure
recovery and peak process and system memory. Keep this gate separate from the
full Catalog upload and interchange checks.

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

## System memory acceptance

Measure the peak P4KX application footprint on an available test host. The
budgets are strict upper bounds in decimal bytes.

| Operation | Peak P4KX memory below | Target system capacity |
| --- | ---: | ---: |
| Individual SDK queries | 2,000,000,000 bytes (2 GB) | 16 GiB total |
| Reduced Basetool catalog generation | 2,000,000,000 bytes (2 GB) | 16 GiB total |
| Complete unfiltered Catalog v3 export | 40,000,000,000 bytes (40 GB) | 64 GiB total |

Subtract the measured peak from the target capacity to estimate the headroom
for the operating system, desktop and other processes. This measurement is
the release criterion and can run on a host with more installed memory.

Measure P4KX and any helpers that run with it. Record peak resident memory
and peak private committed bytes where available. Use the higher observed
value for the budget and identify the counters and sampling method. Also
record the exact Build and binary, host RAM, OS, CPU, baseline workload,
minimum available RAM, paging, elapsed time, process writes and scratch peak.

For cached SDK workflows, exercise cold, changed-Build and warm cases.
Verify the selected data and dependency coverage and confirm that cancellation
or failure retains the previous cache. Measure the SDK harness and record
whole-application memory separately when measuring an embedded integration.
Verify generation and import of the agreed reduced export.

Run the complete normal packaged exporter for installed LIVE and current PTU.
Each complete v3 export preserves all fields, languages, cross-domain links
and per-value origins. Use a single compressed output and verify the decoded
stream from that artifact in a separate replay, so the measured producer runs
once per Build.

The Windows AMD64 portable package at integration revision `25287d184`
completed the normal export for both installed Builds. The package was built
from merge revision `300226ceeeaf522b7d9af790e8fa3698752cf575` with Rust 1.100.0.
Its executable SHA-256 was
`b65c751e4e35e7e9fa9dedc91419af39a67dae4715f3087ea1a7b0438687db70`.
Both runs used one Rayon worker at below-normal priority, Zstandard level 3
and a fixed Unix-epoch export time.

| Build | Wall time (s) | Peak working set (B) | Peak private commit (B) | Minimum available host RAM (B) |
| --- | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 1,236.869 | 28,416,077,824 | 29,528,719,360 | 25,586,094,080 |
| PTU `13248297` | 1,389.108 | 29,741,670,400 | 30,899,863,552 | 25,083,817,984 |

| Build | Compressed file (B) | Declared decoded size (B) | Peak temporary bytes (writer) | Process writes (B) |
| --- | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 1,197,813,494 | 71,467,922,615 | 1,671,675,932 | 1,671,685,712 |
| PTU `13248297` | 1,245,958,540 | 74,427,393,008 | 1,772,329,634 | 1,772,339,724 |

The host ran Windows build 26300.9974 on a Ryzen 9 9950X3D with
100,573,523,968 bytes of physical memory. Windows reported the two process
lifetime peaks after exit. Host headroom and background activity were
sampled once per second with an 8 GiB reserve. System-wide page reads
increased during both exports and page writes stayed unchanged. The archive
stamps, Build manifests and executable digest were stable across both runs.

The installed mining parity checks streamed both artifacts to completion and
confirmed the recorded decoded lengths. The independent all-field provenance
replay and final integrated release-candidate measurement are separate gates.
The larger private peak leaves an estimated 37,819,613,184 bytes for the OS
and other processes on a 64 GiB host. This measures production with the
specified package. Replaying the raw stream and measuring the final release
candidate are separate acceptance checks.
[Catalog work](https://github.com/p4kx-org/p4kx/issues/1151) ·
[Release acceptance](https://github.com/p4kx-org/p4kx/issues/1116)

### LIVE package rehearsal for integration revision `54121ab3e`

The Windows AMD64 portable package built from merge revision
`23addc5e164f65a28a6f258302b30453b331edce` completed an unfiltered Normal Catalog
on unchanged LIVE `13220821`. The packaged CLI used Rust 1.100.0, one Rayon
worker at below-normal priority, Zstandard level 3 and the Unix-epoch export time.
[Native Windows AMD64 package](https://github.com/p4kx-org/p4kx/actions/runs/45359023253/job/138324296731)

| Wall time (s) | Peak working set (B) | Peak private commit (B) | Minimum available host RAM (B) |
| ---: | ---: | ---: | ---: |
| 2,114.990 | 35,562,864,640 | 38,409,072,640 | 9,234,272,256 |

Windows reported both process lifetime peaks after exit. The larger peak leaves an
estimated 30,310,404,096 bytes for the OS and other processes on a 64 GiB system.
The measured host had 100,573,523,968 bytes of RAM and reached a minimum
commit headroom of 23,345,311,744 bytes. The host's page-read and page-write
counters increased by 434,785 and 2,821 during the run.

The Catalog contains 77,896,476,922 decoded bytes and 1,466,041,457 compressed bytes.
The writer reports 1,915,716,829 peak temporary bytes and Windows reports 1,915,726,571
process write bytes. The separate replay uses this same sealed compressed output.

### Packaged Normal measurement on PTU 13260598

Windows AMD64 producer `93d92ca20bef8a8c3731339214a3301124ade7ba` completed
the unfiltered Normal export from the unchanged PTU `13260598` Archive.
The run used one Rayon worker at below-normal priority, Zstandard level 3 and
the Unix-epoch export time.
[Native packaged producer](https://github.com/p4kx-org/p4kx/actions/runs/45375154494/job/138382476262)

| Wall time (s) | Peak working set (B) | Peak private commit (B) | Minimum available host RAM (B) |
| ---: | ---: | ---: | ---: |
| 2,314.908 | 36,488,916,992 | 39,266,893,824 | 14,729,175,040 |

Windows reported the process lifetime peaks after exit. The larger peak leaves
an estimated 29,452,582,912 bytes for the OS and other processes on a 64 GiB
system. The measured host had 100,573,523,968 bytes of RAM and a minimum commit
headroom of 37,464,530,944 bytes. The host counted 554,038 additional page
reads and 218 additional page writes during the export.

The verified frame has 80,616,097,191 decoded bytes and 1,717,255,370 compressed
bytes. The writer reports 2,215,745,998 peak temporary bytes and Windows
reports 2,215,756,050 process write bytes. The separate replay uses this same
sealed compressed output.

## Measuring a complete export

The installed regression checks verify decoded and stored byte counts against the actual writer
output and report the corresponding size warnings. Catalog schema, references and replay are
checked at any artifact size. Verify each consumer's import with its own measured limits.

For each Build, record the full Build identity, exporter revision and hash, and archive length
and modification time before and after the export. Run the normal `p4kx catalog` writer with a
fixed export time, default text selection, `--progress-phases` and one private `.json.zst` output.
The completed writer reports `catalog storage:` on stderr. It sums the logical bytes of the
temporary row spools for container instances, locations, Entity tag declarations,
Item and Blueprint definitions, particle libraries,
contract generators, contract templates, mission broker entries, item loadouts, item ports,
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
`project-contract-generators`, `project-configurations` and
`finalize-projections` as separate phase starts. Record the configuration phases `project-shops`,
`resolve-jump-operators`, `read-independent-configurations`, `project-particle-libraries`,
`project-flare-libraries`, `project-subsumption-events` and `project-weapon-animation` separately. During finalization, distinguish
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

The observed ship base reader visits selected ships in physical Record order. Other entity checks
run in parallel within each scan window. The first ship to read a shared file owns the observation.
One observed loadout resolver visits roots in stable GUID order and stages each result before the
next root. Item rows go straight into one sequential compressed spool. Ship loadouts keep their
original Catalog array position and source-bearing port data for facility and resource topology joins.
The writer interleaves both sets in canonical GUID order. Record the write-counter
change during loadout resolution and the retained private bytes on entry to
location projection.

After each Record and ContractGenerator window and at selected projection boundaries, the
source registry packs unheld DataForge uses into compact coordinates. It keeps live handles and their original authority. A later observation
at the same coordinate reuses the original ID and evidence. The publisher streams the
sorted live and packed rows together with the same IDs, scope, role and encryption.
Each packed row uses the registered ID as its key and shares the original physical source
reference with rows from the same source allocation. The 40-byte map entry on 64-bit
targets holds the ID handle, source handle and typed coordinate with its scope,
role and encryption. The change from 48 bytes saves about 0.54 GiB of inline entry
storage at the observed PTU count of 72,126,576 rows. The Working Set and private-byte
effect require a new measurement.
Sealing moves the packed map into the immutable inventory. The normal publisher can count and
serialize the same sorted rows without copying them into a contiguous array. This storage
uses RAM and adds no provenance spool.

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
after preparing the catalog and recording health. Producer acceptance still requires a
complete normal export with the current packaged producer on the specified producer host.

The discard-sink witness at `4c601317a` checked both installed Archives with four Rayon
threads and stable Archive stamps. It refused normal publication for each Build after health
assessment. The values below are sampled process memory in GiB on a larger host.

| Build | Record scan working set | Record scan private bytes | Peak working set | Peak private bytes | Packed source uses at health |
| --- | ---: | ---: | ---: | ---: | ---: |
| LIVE `13165946` | 15.47 | 16.08 | 26.22 | 27.60 | 68,920,610 |
| PTU `13201184` | 16.30 | 16.95 | 28.02 | 31.73 | 72,144,536 |

LIVE reached a sampled 26.22 GiB working set during health assessment. PTU reached its
peak working set during provenance. Both sampled private-byte peaks occurred during
provenance. The first PTU provenance sample rose by 4.85 GiB in private bytes. The packed
array for 72,144,536 rows at 72 bytes each accounts for 4.84 GiB of that reserve.
The packed source-use counts grew by 17,325,165 and 17,974,970 between the Record scan
and health. The pair logged 0.756 GiB of logical process writes and produced
no Catalog file or encoder sizes.

The follow-up discard-sink witness at `0d10b5901` used the same installed Builds, four Rayon
threads and stable Archive stamps. It included the later compaction checkpoints, 48-byte
packed rows and ownership transfer at sealing. The key census was disabled.

| Build | Record scan working set | Record scan private bytes | Peak working set | Peak private bytes | Packed source uses at health |
| --- | ---: | ---: | ---: | ---: | ---: |
| LIVE `13165946` | 13.51 | 14.10 | 21.75 | 22.72 | 68,904,364 |
| PTU `13201184` | 14.27 | 14.87 | 22.70 | 23.71 | 72,126,576 |

Both sampled peaks occurred during facility interior projection. Minimum sampled host
available RAM was 28.81 GiB for LIVE and 24.65 GiB for PTU. The pair logged 0.756 GiB of
logical process writes. Both Builds reached the normal health decision and refused publication
before serialization. Encoder sizes and the complete normal run on the producer host remain
unmeasured.

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

Set `P4KX_CATALOG_PACKED_KEY_CENSUS=1` on the sink-only witness for the packed key count and
UTF-8 bytes at export health. The counters distinguish direct fields and typed sites, and keys
held solely by the registry or shared by a source reference. This pass can affect timing and
resident memory. Measure the producer's peak with the key census disabled.

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

The verifier at `01ae3cb78` completed both passes on the installed outputs from
producer `25287d184`. It checked the immediate origin of every sourced cell
present in each artifact and reconciled the decoded bytes with the writer receipts.

| Build | Decoded bytes | Sourced cells checked | Source uses | Peak private bytes | Process write bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 71,467,922,615 | 85,520,379 | 88,973,850 | 8,884,805,632 | 23,057 |
| PTU `13248297` | 74,427,393,008 | 89,964,616 | 94,272,938 | 9,227,853,824 | 23,260 |

Each readback took about 87 minutes. The results identify those two artifacts
and that verifier. The release candidate needs its own producer and replay receipts.

The installed SDK parity check for law, resource-quality quantization, cargo,
turret and radar configuration, resource networks and room associations shares
one streaming read per Build. Use the same measurement directory, set
`P4KX_DATA_P4K` to its Archive and `P4KX_LAW_SDK_BUILD` to its changelist.

```text
cargo test --release --locked -p p4kx-export --test real_archive -- --ignored --exact --nocapture real_archive_batched_catalog_sdk::installed_law_quality_cargo_and_resource_sdk_match_one_complete_catalog
```

It checks availability, ordered physical rows and their exact source references
across the 14 selected tables. Record peak memory for individual SDK queries
separately.
To produce the raw artifact from the same completed measurement, run the reconciliation test.

```text
cargo test --release --locked -p p4kx-export --test real_archive -- --ignored --exact --nocapture real_archive_catalog_size::artifact::reconcile_completed_catalog_artifact
```

One streaming decode produces the section and reference-set census and copies the exact
UTF-8 bytes to `p4kx-catalog.json` in that directory. Publication requires the encoded
and decoded lengths to agree with the writer receipt. An existing raw file is preserved.
The installed Archive is unnecessary for this check.
