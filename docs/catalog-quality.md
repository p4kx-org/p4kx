# Canonical quality source data

Schema 3 has one source owner for every selected readable physical quality Record:

| Dataset | Original owner | Payload |
| --- | --- | --- |
| `definitions.qualityDistributions` | `CraftingQualityDistributionRecord` and `CraftingQualityLocationOverrideRecord` | Independent `qualityDistribution` and `locationOverride` source values |
| `definitions.qualityQuantizations` | `CraftingQualityQuantizationRecord` | `qualityQuantization` source value |
| `projections.qualityQuantizationResources` | Each selected physical quantization definition | Minimal owner `guid`/`recordIndex`, evaluated `eligibility`, and genuine inverse `resourceTypeGuids` |

Each dataset uses the shared closed `availability` and `rows` contract. Both D families flatten
the [shared eight-field Record header](catalog-record-headers.md). Rows are ordered by nominal
GUID text and original physical Record index. Equal GUIDs, names and paths do not collapse rows.
Schema 3 emits no flat quality root lists and no full reduced interpretation mirrors.

## Authored declarations

The same typed source readers serve standalone quality definitions and the quality fields in
`definitions.resourceTypes.rows[].properties`. Each wrapper retains its sourced concrete
`sourceType` and sourced `payload`. Inline curves do not repeat that concrete type as `kind`. The closed payload tags are `inline`, `recordRef` and
`emptyBase`. Inline content has its own sourced `value`. A record reference retains its authored
GUID and independently sourced target headers without following the target payload to create
an interpretation. Known fieldless bases are present `emptyBase` declarations. Unsupported
concrete types retain the actual type string with an unsupported payload and blocking coverage
diagnostics; they are not invented successful union variants.

Every field, wrapper, array and array member uses the ordinary five-key
`state`/`value`/`containsEncryptedSource`/`sourceUsesRefs`/`sourceSetComplete` carrier. Missing,
explicit null, present empty, unreadable, unsupported and invalid remain distinct. False and zero
remain present. Location overrides preserve every original nullable position and repeated entry.
Distribution rows retain the other root declaration as absent when that physical source type
does not declare it. No name, Locale, description, blueprint link or runtime result is inferred.

All distribution bounds, parameters and quantization band numbers use the existing exact-width
`ResourceNumber` carrier. Signed and unsigned integer variants keep their original widths;
64-bit integers use canonical decimal strings. Float variants retain `type`, exact lowercase
`rawBits` (eight hex digits for f32, sixteen for f64), and an optional finite numeric projection.
Infinity, NaN payload/sign bits, signed zero and f64 precision outside f32 remain present raw
facts. A null `finite` member is not an absent original value.

The independent declared Int32 contract still applies to `min`/`max` and band
`start`/`end`/`mappedValue`. Another numeric storage kind is retained losslessly and diagnosed
at its exact original field. Required band values that are absent, null, invalid or unreadable
remain blocking field failures. A configured end below its start retains all original bands
and a malformed-source diagnosis at that band's `end`. Extra unmapped fields remain explicit
coverage failures. A source decoder error does not invent recovered numeric bits.

## Inverse relation and eligibility

The resource relation is authored in the opposite direction:
`ResourceType.properties[].qualityQuantization.qualityQuantizationRecord` names a quantization.
The V table gives a sorted, deduplicated inverse. It does not copy authored bands or curves.
`eligibility` is a sourced closed result: `{status: "eligible"}` or
`{status: "notEligible", reason: ...}`. Reasons are `noInlineBands`, `emptyBands`, `invalidBands`
and `unavailable`. A complete empty authored band list is healthy raw data and explicitly
ineligible for this lookup; no minimum-length source format rule is fabricated.

The established first-eligible nominal-GUID policy is preserved. Among physical definitions
with one GUID, only the first eligible occurrence consumes that GUID's inverse resources;
later eligible occurrences have an evaluated empty result. Raw authored references still expose
their actual physical targets, which can be an earlier ineligible occurrence. These are separate
facts. The exact earlier rejection/winning eligibility inputs contribute to the choice; unused
later candidate payloads do not. Earlier different-GUID candidates contribute their actual
GUID inequality and admission selectors, without unrelated band/eligibility payloads. The original
ResourceType property order and repetitions remain
in D even though the V relation sorts and deduplicates GUID values.

With a selected non-leftover quantization, or uncertain target metadata, the inverse reads all
non-leftover resource dependencies, including ones outside the Entity Filter. It publishes only
selected D definitions. Without such a target, it avoids outside-Filter resource link reads.
Actual positive and negative list/wrapper/reference selectors are retained. Unrelated loaded
property names and unused target payloads are not contributors. A nested resource GUID retains
its own actual resource-header payload contribution independently of the shared inventory proof.

Optional decision proof capture shares Commodity's existing limits: 4096 retained input uses
and 1 MiB of variable metadata per proof group. Actual enumeration and inverse calculation
continue after a capture cut. A cut retains an opaque original-source witness and explicitly
sets `sourceSetComplete: false`; it never fabricates an inventory coordinate or a missing value.
Positive GUID payload proofs have independent retention, so a shared inventory cut cannot starve
a late child. Known encrypted witnesses remain `containsEncryptedSource: true` even after a cut.
Incomplete optional coordinate retention is independent of complete source-read health.

## Health, counts and publication

Every successfully read retained D owner is Projected, counted in `KindSummary.count` and in
written rows, even when a separate field error makes the export incomplete. Such a field failure
does not also label that physical owner omitted after projection failure. Strict tree/header
read errors still omit the unreadable owner and retain its original failure identity. Target
materialization, field validity, raw reference status and inverse eligibility are separate axes.
Normal publication still refuses actual incomplete health; DiagnosticPartial retains readable
facts and their diagnoses.

Nonfinite finite-projection diagnostics are excused only by actual emitted raw capture of the
same original physical Record index, field path and f32/f64 token bits. The proof is generated
locally in the original typed field reader and travels privately with its successfully returned
D definition, including the actual source type and numeric field contract. Thin original-token
requirements share their owning inspection identity; full owner paths and GUIDs are copied only
into bounded final diagnostic examples after coverage is checked. Actual retained receipts are
indexed once by physical Record, with fresh consuming state for each inspection. The canonical
inspection is borrowed during role replay instead of cloning all its token requirements. Typed NumberPosition only chooses staging on supported raw traversal routes; unknown or
unmapped paths keep ordinary bounded unsupported-value diagnostics and full counts. It does not
prove capture, and the existing field walk still visits every materialized value. A kind name, row presence, filtered dependency, same-GUID alias, same number bits
at another field or uncaptured extra field cannot substitute. One retained named DTO cell supplies at most one original-slot receipt, independently of getter
count. Requirements consume receipts once per original inspection; the same actual read may be
replayed under selected/dependency roles without sharing that consumption state. Duplicate
declared names that a named DTO cannot retain have their own explicit coverage diagnosis; the
format reader and public interpreted API remain permitted to read such declarations. No serialized
source-use ID, GUID/path/hash lookup or second provenance registry restores authority.

`CatalogSummary.quantizations().unused` counts selected inverse-eligible quantizations with no
assigned resource GUIDs, including evaluated later duplicate occurrences. `unresolved_links`
keeps deduplicated resource links without an eligible nominal-GUID target plus selected-leftover
links. A dangling Reference that yields no resolved Record adds no inverse link. These counters
do not count all raw D definitions and do not themselves change publication health. Commodity
fallback uses only the actual selected eligible inverse assignments and retains their source
decision inputs.

The public Rust `read_crafting`/`Crafting`/`QualityCurve` API is a separate current interpreted
contract: finite f32 parameters, exact nullable Int32 bounds and its existing typed failure
semantics remain. It does not implement another catalog schema. Historical v2 fixtures are
frozen test inputs; the catalog output contract is v3 only.

Synthetic source oracles cover raw widths/IEEE bits, nullable/empty/reference forms, retained
malformed ranges, duplicate physical identity, first-eligible prefix, Filter/dependency scope,
exact numeric capture negatives, immediate plain/encrypted/unknown origins and a genuine
optional-capture cut with a late positive value. Installed-Archive acceptance is deferred by
the user. No source proposal or synthetic fixture establishes an installed Build result.
