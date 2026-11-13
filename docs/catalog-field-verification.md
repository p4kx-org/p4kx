# Typed catalog field verification

The development-only `field_verification` test binary checks which declared source fields the typed
mapper retains and whether owning raw fields agree with an opt-in local published scunpacked
snapshot of the same Build. Runtime catalog serialization and shipped network behavior are unchanged.

[Generic coverage](coverage.md) proves readability. A readable Record may still contain a populated
tagged variant the mapper rejects, or a populated nested block that the typed projection omits.
Neither a successful exit nor component fill rates prove field retention.

## Reviewed inventory and accounting

A version-1 inventory identifies the declaring owner type and field, with a separate primitive-array
element identity. Each entry records its expected declared target, disposition, typed path and whether
the last reviewed baseline had no populated instances.

| Disposition | Required information |
| --- | --- |
| `exported` | Catalog group and explicit typed projection path |
| `referenceOnly` | Reason the owning fact is represented by a link or structural relationship |
| `metadata` | Reason the field is presentation/media metadata |
| `unsupported` | Concrete missing projection capability or source interpretation, with a reason |

Lack of a consumer is not an exclusion reason. Unreviewed declarations remain visible even with
zero instances; removed declarations also remain visible. Domain tickets add exact mappings and
fixtures incrementally. The infrastructure does not wait for every export expansion.

The initial [inventory](../crates/p4kx-export/tests/field_verification/baseline.rs) covers existing
shield and quantum-drive projections. New fields found in these owners and their relevant targets
require review. Initial published mappings cover an owning shield field and tagged rapid-fire rate.
This is a baseline contribution, not a claim of exhaustive gameplay coverage.
The canonical routing checks also cover the global `tags` kind and sourced Tag GUID joins;
[Tag contract tests](catalog-tags.md#verification-scope) exercise the actual source fields,
raw-array multiplicity, physical duplicates, source states, global Filter scope and strict new-field
diagnostics. The ignored Tag oracle compares raw physical definitions and owning Entity arrays
without treating convenience paths as source identity.

Schema collection includes parents, owned targets and relevant strong-pointer descendants.
Reference and weak-pointer declarations retain their target type. Auditing those targets
requires their own inventory roots. A changed
declared base target with supported concrete instances is a schema-contract change, not observed
data loss. A schema-declared resource override without populated instances remains distinct from
a populated family.

Reports separate declared/present/populated/exported/unhandled/unreviewed/rejected counts, generic
source errors, missing target References, missing export GUIDs, selection exclusions, target changes
and newly populated fields. Zero and false are populated. Primitive array elements are surveyed.
Missing source targets are not parser failures; missing export GUIDs are not selection exclusions.

Typed projection steps are `{"kind":"field","name":"stats"}`, `{"kind":"each"}` and
`{"kind":"matching","field":"kind","value":"SyntheticVariant"}`. An empty path selects the root.
Array multiplicity is preserved. Paths never resolve Record links as filesystem names.

For raw authored arenas, the audit can select Record References by exact
concrete owner type and original member name using
`{"kind":"ownedArenaReference","owner":"<type>","name":"<field>"}`.
It follows the arena's local indices from its roots and rejects out-of-range or
repeated or unreachable nodes. Same-named fields in other owners do not count.
The quantum profile mapping applies this to its original effect-tag Reference.

## Real source survey

Use existing private Archive/key configuration and task-local `TMP`/`TEMP`:

~~~powershell
cargo test --release -p p4kx-export --test field_verification --locked real_archive::field_inventory_observes_declared_populated_and_exported_source_values -- --ignored --exact --nocapture
~~~

`P4KX_FIELD_INVENTORY` may name a private inventory JSON; otherwise the baseline is used. Mapped
catalog kinds reuse public `Kind::rule` selection. Set `P4KX_FIELD_RECORD_FILTER` to the original
Record path or a catalog Filter pattern for a scoped survey. The same Filter applies to the
producer and raw observation walk. Declaration review still covers the inventory's full type
closure, and the test checks selected GUIDs for aliases outside the Filter. An empty selection
fails. The offline published comparison uses its independent snapshot selection.
Development leftovers are separately accounted
for. The check reads canonical source trees and the actual catalog. Missing mapped
projections, typed rejection, new population, changed targets and unreviewed declarations fail the
survey. Missing source targets retain their own source-evidence category.
The independent source audit explicitly requests `DiagnosticPartial` so a failure in an unrelated
kind does not prevent examining mapped kinds. Full production health remains attached. Every mapped
kind must still have complete source/field inventories, reads and projections, with no blocking
source diagnostics or failed physical Records/Entries. Unscoped failures fail the audit because their
independence cannot be established. Source reads, coverage regressions and numerical differences
remain failures. Physical Records sharing a mapped GUID, including aliases outside the mapped
selection, fail with both table indices instead of merging observations or masking selected data.

## Metadata-only declaration surveys

The ignored `resource_metadata::resource_owned_type_declarations_only` test writes only DataForge
schema declarations for `ResourceType`, `ResourceTypeGroup`, `ResourceTypeProperties`, their relevant
descendants, parents and owned Class/StrongPointer types. Reference and WeakPointer fields retain
their declared target type names without following those target declarations. Enum fields retain
the enum type name without options. Missing requested roots are explicit; an entirely absent scope
fails.

The ignored `mission_property_metadata::mission_property_owned_type_declarations_only` test uses
the same collector for every struct name beginning with `MissionPropertyValue`, including types
with no Records. It adds their parents and owned Class/StrongPointer closure under the same
bounds. It shares the output and Archive-change guards with the resource survey.

The artifact contains type names, parents, declared field names, data types, array flags and target
type names, plus the DataForge format version and optional source Build branch/version/change.
The collector never enumerates Records, builds canonical trees, reads field values, resolves
Reference targets or emits Record names, paths, GUIDs, instance counts, localization text or enum
options. This check provides declaration evidence; it provides no populated-field or numerical
agreement evidence.

Both output paths must be explicitly configured. The task root and existing output parent must be
direct directories, the destination must remain contained, and creating an existing file fails.
The artifact is bounded to 8 MiB. Review the collector and exact command before any real execution:

~~~powershell
$env:P4KX_METADATA_TASK_ROOT = 'D:\Coding\Agents\Temp\codex\p4kx-050-implementation\ref-1050'
$env:P4KX_RESOURCE_SCHEMA_OUT = Join-Path $env:P4KX_METADATA_TASK_ROOT 'resource-schema-LIVE.json'
cargo test --release -p p4kx-export --test field_verification --locked --offline -- resource_metadata::resource_owned_type_declarations_only --ignored --exact --nocapture
~~~

Use existing private Archive/key configuration and task-local `TMP`/`TEMP`. For another Channel,
select its Archive explicitly and choose a new output filename. Synthetic tests verify that Record
values/identities and enum options do not reach the artifact, owned declarations remain visible,
linked target declarations stay outside scope, and output paths cannot escape or overwrite.

For the mission-property declaration scope, use a new explicit task root and output filename:

~~~powershell
$env:P4KX_METADATA_TASK_ROOT = 'D:\Coding\Agents\Temp\codex\p4kx-050-implementation\guard\mission-declarations'
$env:P4KX_MISSION_PROPERTY_SCHEMA_OUT = Join-Path $env:P4KX_METADATA_TASK_ROOT 'mission-property-schema-PTU.json'
cargo test --release -p p4kx-export --test field_verification --locked --offline -- mission_property_metadata::mission_property_owned_type_declarations_only --ignored --exact --nocapture
~~~

Select the private Archive and task-local `TMP`/`TEMP` explicitly, and review the collector and
exact invocation before execution. Declaration names and field shapes explain schema support;
they do not establish which values are populated or exported.

## Offline published reference

Published input has a separate provenance contract from [controlled producer runs](differential-verification.md).
Do not fabricate a producer receipt, DCB hash or generator revision for third-party published data.

The reviewed publication is
[scunpacked-data 1340c830](https://github.com/StarCitizenWiki/scunpacked-data/commit/1340c8303709904165e2da0fa7060bce0624260b),
whose commit states `4.10.2-LIVE.12881860`. Separately inspected
[ScDataDumper 120a7286](https://github.com/octfx/ScDataDumper/commit/120a728639bdd1b06525a0f8574cb0e91d0fbf8d)
does not establish the exact generator revision used for that data commit.

Prepare an offline snapshot outside tracked files with selected individual JSON files and
`.p4kx-scunpacked.json`. Preserve published bytes. The version-1 metadata records:

- `provenance.repository`, immutable `dataRevision` and `publishedBuild` with `gameVersion`, exact
  `channel` and `changeNumber`;
- explicit `generatorRevision` and `dcbSha256` availability, such as
  `{"kind":"unknown","reason":"not published with the dataset"}`;
- selected GUIDs and reasoned exclusions;
- each selected file's relative path, byte count, canonical SHA-256, family, row selector and GUID
  identity selector;
- owning raw-field mappings, typed selectors, units, comparison policy and optional resource join.

The [strict DTOs](../crates/p4kx-export/tests/field_verification/published_model.rs) define the exact
shape. File hashes are recomputed locally and rechecked after comparison. They identify supplied
snapshot bytes against the supplied identity; they do not attest foreign execution or prove the
origin of arbitrary local files. Metadata cannot add a new publication to the source-reviewed
revision/Build registry. Unknown generator/DCB evidence remains explicit.

An empty GUID selection declares the complete family represented by the snapshot; an explicit
list declares a subset. Files outside that selection are counted. Missing selected files fail.
A selected identity absent on both sides produces `missing`, never equality.

For an individual item file, rows are the root (`[]`); the owner GUID selector is `Item/reference`.
Owning raw components are under `Raw/Entity/Components/<OwnerType>`. `Item/stdItem` contains formatted,
normalized, rounded, derived and sometimes curated values. Nested magazine/manufacturer/ammunition
GUIDs are different identities.

Catalog-v3 verification reads Commodity definitions from `definitions.commodities.rows`, using
`guid.value` as the owner identity, and presentation recipes from
`projections.commodityPresentation.rows`, using `commodityGuid.value`. Each family is required and
validated independently even though both use the `commodities` audit group. A root-level
`commodities` property cannot supply either family. Text references resolve through the canonical
text tables; a missing translation remains distinct from an authored empty string.
GUID multimaps preserve commodity Entity, shared ResourceType, MineableElement and placement
relationships. A resource join declares both ResourceType selectors and retains every member.
A single shared raw fact may compare against several members. Ambiguous many-to-many facts are
`uncomparable` rather than overwritten or paired by an unsupported guess.

~~~powershell
$env:P4KX_REF_SCUNPACKED = '<private offline snapshot root>'
cargo test --release -p p4kx-export --test field_verification --locked real_archive::published_scunpacked_raw_fields_match_only_an_established_same_build -- --ignored --exact --nocapture
~~~

An unset reference variable skips before opening an Archive. A configured empty/malformed root
fails. Numerical assertions require an exact reviewed Channel, public game-version and change
number. Mismatches and unknown publications are reported with zero agreement and no numerical
assertions. Add publications only after reviewing the pinned primary commit and wrong-Build tests.

The published Build differs from the installed LIVE baseline recorded in [the audit](scunpacked-data-audit.md).
No same-Build numerical result is claimed here.

## Outcomes, units and bounds

Summaries distinguish compared/equal/different, reference-only, P4KX-only, missing, unhandled,
derived, uncomparable and selection-excluded outcomes. Only equal/different count as comparisons.
Both-missing, derived-only and empty runs provide no independent comparison evidence.

Integers, booleans and strings compare exactly. `f32RoundTrip` compares finite numbers in the same
source `f32` rounding cell rather than applying a broad relative tolerance. Units must agree:
seconds, rounds/minute, metres, SCU and ResourceType-specific quantities are distinct. Standard
resource units and power segments retain separate representations even for the same ResourceType. An explicit
microSCU-to-SCU policy carries its normalization and reviewed reason. Derived formulas and curated
overrides carry reasons and remain `derived` rather than raw equality.

| Input | Bound |
| --- | --- |
| Snapshot/inventory metadata | 4 MiB before parsing |
| One selected JSON / total selected bytes | 64 MiB / 256 MiB |
| Snapshot files / schema owners | 10,000 |
| Fields, values, selector results and GUID identities | 1,000,000 |
| Selector steps and source/inheritance depth | 64 |
| Relative path and selector text | 4,096 UTF-8 bytes |

The inspected `items.json` exceeds the per-file limit; prefer individual files. Common testkit
offline primitives reject escapes, linked ancestors, junctions, physical aliases, case collisions
and unreadable inputs. Walks, reads, schema closure, source observation, selectors and joins check
cancellation.

~~~powershell
cargo test -p p4kx-export --test field_verification --locked
cargo test -p p4kx-extraction --test reference --locked
cargo clippy -p p4kx-export --test field_verification --locked -- -D warnings
~~~

Synthetic tests exercise all result classes, the three structural regression classes, primitive
arrays, wrong Build, duplicate joins, malformed input, units, file mutation and cancellation.
The existing synthetic spawn fixture exercises real public typed-mapper rejection.
Game data and real manifests stay outside tracked fixtures and CI. Publish aggregate results only.
