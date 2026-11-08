# Resource source verification

The resource oracle compares the production catalog with the raw DataForge from the same
retained DataForgeEntry. It checks commodity identity and nullable shared resource links,
then samples canonical ResourceType packaging, validation, volatility and owned quality values.
It does not use a reconstructed resource DTO as its expected output.

## Synthetic checks

Run the oracle's pure and generated-Archive tests without game data:

~~~powershell
cargo nextest run --locked -p p4kx-export --test real_archive -E 'test(real_archive_resource_oracles)'
~~~

The generated-Archive cases call write_catalog_from and then compare its actual JSON with the
same raw DataForge. They cover concrete and base override-entry declarations. Mutation checks
reject a wrong canonical value even when an additive sibling carries the correct value, wrong
nested quality values, invalid immediate encryption markers, incomplete source sets and altered
physical source metadata. Other synthetic resource tests cover absent, null, empty, dangling,
unreadable and non-finite owned values.

## Installed Archive check

Run only when the installed-Archive acceptance has been authorised for the current session.
Use an isolated absolute P4KX_FOLDERS_ROOT and task-local TMP/TEMP as described in
[profile-folders.md](profile-folders.md) and CLAUDE.md. Set P4KX_DATA_P4K to the primary Archive
and optionally P4KX_DATA_P4K_B to a second Archive. The existing real_archive_key helper reads
the configured key; do not put a key in the command, fixture or report.

~~~powershell
cargo nextest run --locked -p p4kx-export --test real_archive --run-ignored ignored-only -E 'test(commodity_identity_and_resource_owned_values_match_the_observed_build)'
~~~

Each Archive must retain complete commodity and ResourceType source inventories. The oracle
checks that the catalog's branch, version, changeNumber and buildId match the observed Build.
It emits only a SHA-256 Build tag, aggregate cardinalities, aggregate resource coverage and
the exact-cardinality pin state. It emits no Record names, paths, GUIDs or gameplay values.

The source comparison uses the common frozen provenance contract to check all observed DCB
source and metadata-use fields, including ancestor sources. Unknown additive object members
remain acceptable. Values are compared against their exact physical Record owner and field
path. Attempted reads cannot contribute to a retained value. The immediately materialized
containsEncryptedSource, sourceSetComplete and sourceUsesRefs must agree with the observed
input and contributing uses.

## Bounds and evidence limits

The oracle accepts at most 2,000,000 DataForge Records, 16,384 commodity rows and 16,384 resource
rows. It checks at most the first 64 canonical resource rows in output order, 65,536 raw values
and 32 owned-value nesting levels. Source-use inventories are limited to 262,144 rows; the
physical source contract comparison also has a 65,536-node and 32-level budget. Budget
exhaustion fails rather than silently claiming coverage.

A successful sample must include declared cargo slots, volatility properties, crafting
properties and genuinely compared quality scalar or reference values. An empty crafting tag
does not provide quality-value coverage. Unresolved sampled boxes are reported by status.
The finite sample does not establish the completeness of every ResourceType field in an Archive.

Portable wire metadata cannot distinguish different opaque Archive-input handles when every
serialized observation is identical. The production matches_archive guard owns that live
input-instance check. Equality of raw values and retained field origins also does not prove
the producer's entire execution history; exhaustive contributor capture remains the common
provenance implementation's responsibility.

## Exact cardinality baselines

P4KX_COMMODITY_CARDINALITY_BASELINE can name a private JSON file with identity and counts
objects. identity requires the complete branch, version, changeNumber and buildId. counts
requires rows, uniqueGuids, distinctSubtypesIncludingNull, nullSubtypes, sharedSubtypeGroups
and rowsInSharedSubtypes. Reading is limited to 16 KiB and rejects unknown members.

An exact matching Build must have exactly matching counts. A different complete Build is
reported as differentBuild, and a missing baseline as unavailable; neither is an exact pin.
A version prefix or an incomplete identity is never accepted as matching Build evidence.

The historical LIVE 4.10.2 observation in #1044 recorded 170 commodity rows and 167 distinct
subtypeGuid values. Its original export and complete Build identity are no longer available.
Those numbers remain an unpinned historical observation. They must not be assigned to a current
Build, regenerated from another Build or presented as a passed historical cardinality test.
The current-Build invariants and sampled source-value checks are independent of that missing
historical evidence.
