# Resource source verification

The resource oracle compares production table output with the raw DataForge from the same
retained DataForgeEntry. It checks commodity identity and nullable shared resource links, then
compares packaging, validation, volatility and owned quality values for every admitted ResourceType.

## Synthetic checks

Run the oracle's pure and generated-Archive tests without game data:

~~~powershell
cargo nextest run --locked -p p4kx-export --lib -E 'test(resource_oracle)'
~~~

The generated-Archive cases compare the full catalog output with the same raw DataForge.
They cover concrete and base override-entry declarations. Parity checks compare the scoped and
normal catalog tables, including Blueprint-cost fallback, quality-quantization fallback and a
catalog with a discovered BlueprintPool reward. Mutation checks reject wrong canonical and nested
quality values, missing and duplicated physical owners, invalid immediate encryption markers,
incomplete source sets and altered physical source metadata. Other synthetic tests cover absent,
null, empty, dangling, unreadable and non-finite owned values.

## Installed Archive check

Run only when the installed-Archive acceptance has been authorised for the current session.
Use an isolated absolute P4KX_FOLDERS_ROOT and task-local TMP/TEMP as described in
[profile-folders.md](profile-folders.md) and CLAUDE.md. Set P4KX_DATA_P4K to the primary Archive
and optionally P4KX_DATA_P4K_B to a second Archive. The existing real_archive_key helper reads
the configured key; do not put a key in the command, fixture or report.

~~~powershell
cargo nextest run --locked -p p4kx-export --lib --run-ignored ignored-only -E 'test(commodity_identity_and_resource_owned_values_match_the_observed_build)'
~~~

The installed oracle builds a private view of the commodity, ResourceType and ResourceTypeGroup
catalog tables through the production scanner. It reads every admitted resource Record and keeps
unreadable selection candidates. The unfiltered Blueprint costs and quality quantizations supply
fallback evidence. BlueprintPool reward discovery is outside the resource proof. Authored
quality-location links retain their referenced StarmapObject records. The same archive supplies
source locale keys and original provenance. The full catalog transport has its own verification.
The oracle requires complete resource inventories and checks branch, version,
changeNumber and buildId. Its report contains a SHA-256 Build tag, aggregate cardinalities and
resource coverage, and the exact-cardinality pin state.

The source comparison uses the common frozen provenance contract to check all observed DCB
source and metadata-use fields, including ancestor sources. Unknown additive object members
remain acceptable. Values are compared against their exact physical Record owner and field
path. Attempted reads cannot contribute to a retained value. The immediately materialized
containsEncryptedSource, sourceSetComplete and sourceUsesRefs must agree with the observed
input and contributing uses.

## Coverage and evidence limits

The original Record inventory determines the expected physical owners. Every owner must match
one emitted row. The check walks the original packaging, validation and property trees, reporting
the declared slots, observed volatility and crafting properties, and compared quality scalars and
references. An empty crafting tag contributes zero compared quality values. Unresolved boxes are
reported by status. The comparison covers these named fields and the original Record header.

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

The installed comparison independently measured the following Builds. It verified each full
manifest identity and retained its SHA-256 tag in the private receipt.

| Build | Commodity rows and unique GUIDs | Distinct subtype values including null | Null links | Shared subtype groups | ResourceType rows | Compared resource values | Resource groups |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| LIVE `13165946` | 170 | 167 | 1 | 2 | 207 | 2,919 | 19 |
| PTU `13189690` | 170 | 167 | 1 | 2 | 209 | 2,925 | 19 |

In each Build, five commodities share the two subtype groups. The oracle checked 1,950 declared
cargo slots, 559 null slots, one volatility property and 44 crafting properties with 122 owned
quality values. The unresolved cargo references were seven dangling links in LIVE and fourteen
in PTU. These states are reported from the original references.

The historical LIVE 4.10.2 observation in #1044 recorded 170 commodity rows and 167 distinct
subtypeGuid values. Its original export and complete Build identity are unavailable. The table
above records new measurements for the two named Builds. The historical observation remains
unpinned.
