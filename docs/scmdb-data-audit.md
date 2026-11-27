# SCMDB comparison

Research date: 2026-11-10.

## Current comparison integrity

The baseline investigation below records the inspected sources before the comparison repairs.
The current development check validates required reference container shapes, reports empty
families explicitly, preserves conflicting contract projections and compares buy-in and standing
integers without conversion through f64 or float tolerance. Equal contract aliases remain valid.

Published reward occurrences are counted separately as uncomparable until their source mapping
is demonstrated. Success reputation follows the frontend's direct-array/pool precedence; invalid
pool indices are input errors. The matching-label LIVE rerun reported no value disagreements,
while explicitly leaving 1,708 money values, 1,828 success-reputation members and 49 other
reputation members uncomparable. This is partial comparison coverage, not reward acceptance.

Eight synthetic regressions failed against the baseline and pass with these corrections.
Four additional controls cover direct-empty precedence, invalid indices, equal aliases and the
difference between uncomparable coverage and a value disagreement. The complete non-ignored
Extraction suite passed 692 tests; 125 ignored tests were not executed in that synthetic run.
The targeted installed LIVE comparison was executed separately. Strict Extraction Clippy passed.

The remaining baseline findings still require source review: selection completeness, reference
identity validation, duplicate standing membership, float precision, blueprint property/mode
mapping and stronger Build matching. No PTU numerical agreement or complete catalog equivalence
is established by these repairs. Original reward/outcome export remains within the all-data v3
scope described below.

## Inspected baseline

The existing LIVE check passed with zero disagreements for the fields it actually compared.
It does not validate contract money or reputation rewards, and its legacy-contract numeric and
standing-link coverage is zero. The highest-value follow-up is to make comparison coverage and
source identity explicit, then add source-reviewed reward comparisons. No parsing or catalog
product bug is established merely by a value displayed on SCMDB.

## Scope and reproducibility

This investigation inspected P4KX source, SCMDB's published frontend and the structure of four
public JSON files already saved once by the coordinating agent. It did not fetch another data
copy, run Cargo, inspect credentials or keys, write a game folder, or change repository files.
The accompanying `inputs.json` records source hashes and frozen evidence copies. Reference JSON
stays outside the repository; only schema names and aggregate counts appear below.

| Primary source | What it establishes |
| --- | --- |
| [SCMDB](https://scmdb.net/) and its [published JavaScript](https://scmdb.net/assets/index-D1I8EJYN.js) | Current frontend normalization and endpoint usage; not the unpublished dump generator's formulas |
| Saved `data/versions.json` and root `version` fields | Published labels `4.10.2-live.12913470` and `4.10.3-ptu.13145606`; no publisher-authenticated DCB identity |
| [ADR-0267](adr/0267-scmdb-data-dumps-are-a-development-check-saved-by-hand.md) | Offline, development-only comparison; no reference data incorporated into P4KX or fixtures |
| [SCMDB runner](../crates/p4kx-extraction/tests/real_archive/scmdb.rs) and [comparison modules](../crates/p4kx-extraction/tests/real_archive/scmdb_compare.rs) | Actual current checks and selection |
| [Catalog field-verification guide](catalog-field-verification.md#outcomes-units-and-bounds) | Existing exact-integer/source-f32 and explicit-coverage vocabulary to reuse |

The JavaScript snapshot is 1,147,133 bytes; SHA-256 is
`9475a8dfadea52940351b25481e9a4972af50536306e00b8a9c3c3c19477be71`.
Character offsets below are zero-based offsets after reading this exact snapshot as text. They
are evidence locators, not an API contract. No dump generator revision or input DCB digest was
published in the inspected frontend/dump metadata.

## Actual LIVE baseline

The coordinating agent ran the existing ignored test to terminal completion. Its retained
`live-baseline.log` and `live-baseline.log.result.json` record exit 0 at
2026-11-10T13:51:00Z. This investigation independently read those artifacts. The observation is a
PASS under the existing label matcher and comparison policy, not reward acceptance or a complete
source/canonical/catalog equivalence claim. The runner compares canonical Record JSON, not an
exported catalog ([runner](../crates/p4kx-extraction/tests/real_archive/scmdb.rs)).

| Compared family/field | Compared occurrences | Other observed outcomes |
| --- | ---: | --- |
| Blueprint product | 1,618 | No disagreement |
| Blueprint craft time | 1,618 / 1,618 | No absence/disagreement |
| Blueprint option quantity and minimum quality | 4,316 / 4,316 each | No absence/disagreement |
| Blueprint modifier start/end quality and start/end modifier | 6,556 / 6,556 each | No absence/disagreement |
| Contracts: timeToComplete | 1,510 / 1,565 | 55 absent on one side |
| Contracts: buyIn | 38 / 1,565 | 1,501 absent on one side; 26 on both |
| Contracts: minStanding | 1,211 / 1,565 | 99 absent on one side; 255 on both |
| Legacy contracts: each of those three fields | 0 / 291 | 291 absent on one side per field; IDs were found |
| Mining element numeric fields | 39 / 39 per field, seven fields | Resource links also checked |
| Composition minimumDistinctElements | 63 / 63 | 172 parts checked |
| Composition numeric part fields | 172 / 172 per field, five fields | Element links also checked |
| Cluster probability | 23 / 23 | 38 parameter entries checked |
| Cluster parameter fields | 38 / 38 per field, five fields | No absence/disagreement |
| Standing minReputation | 38 / 38 unique standings | Only standings reached through contract min/max fields |

Category check totals combine parent/length/identity checks with field checks. For example,
30,540 modifier checks are not 30,540 independent modifier values. The numeric denominator is
6,556 occurrences for each of four fields. The [summary implementation](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/findings.rs)
reports visited occurrences; it does not enumerate all source Records or all catalog rows.

## Existing comparison coverage

| Domain | Exact implemented projection | Deliberately or currently outside that projection |
| --- | --- | --- |
| Blueprints | Product GUID or dangling-reference error substring; tier count; partitioned craft time; mandatory-cost slot/option counts; option quantity/minQuality; flattened modifier range numbers | Optional costs, cost kind/count/context, ingredient identities, property identity, additive/multiplier identity, arbitrary nested cost trees, catalog projection |
| Mining | Seven MineableElement `element*` numbers/resource GUID; minimumDistinctElements; composition part element GUID/five numeric fields; cluster probability/five parameter fields | Densities, quality distributions/bands, scan signatures, rarity, locations, equipment, refinery facts and any derived percentages |
| Contracts | Every valid GUID `id` recursively encountered in a contract-named Record; timeToComplete, contractBuyInAmount, minimum-standing GUID, only when both sides expose the field | rewardUEC, fixed reward currency/max/bonuses, dynamic reward calculation, success/failure reputation amounts, availability, scenario/item/blueprint rewards |
| Reputation | minReputation for unique standings mentioned by contract minStanding/maxStanding objects | Complete scopes/ranks, faction identities, initial/ceiling/drift/gating facts, reward Records and outcome masks |

Sources: [blueprints](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/blueprints.rs),
[mining](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/mining.rs),
[contracts](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/contracts.rs).
The exclusions identify the current projection. Under the user's all-data v3 scope, missing
original reward and outcome facts are a prioritized source-review and export task. SCMDB's UI
cannot alone establish field identity, units, formulas or outcome semantics.
[ADR-0068](adr/0068-contracts-are-a-catalog-kind-with-resolved-titles-and-success-only-reputation.md)
describes existing success-only reputationGains; additional source facts can preserve complete
authored results independently of that projection.

## Gaps that can make a green result misleading

1. **Empty or wrong-shaped containers can yield no comparison at all.** `elements` maps every
   non-array to an empty slice; mining `entries` maps every non-object to an empty iterator.
   `Findings::is_empty` asks only whether mismatches exist. Three `{}` values therefore leave
   no mismatch and no coverage. Correctly typed empty domains must remain valid with explicit
   zero coverage; missing/wrong-typed required containers must be reported as invalid input.
   This does not justify a rule requiring each domain to contain a record.
   Sources: [elements](../crates/p4kx-extraction/tests/real_archive/scmdb_compare.rs),
   [entries](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/mining.rs),
   [success predicate](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/findings.rs).

2. **Absence and malformed values collapse together.** Numeric/string conversions return `None`
   for absent, null and wrong-typed data. Contract fields present on just one side are tallied
   without disagreement. This explains a legitimate zero-coverage result, but does not distinguish
   omission-at-default from a selector/schema regression. The current renamed-field synthetic
   test explicitly checks a zero compared count; it never requires the overall check to fail.
   Sources: [field policy](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/findings.rs),
   [contract comparisons](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/contracts.rs),
   [renamed-field test](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/tests.rs).

3. **Some reference identities accept absence as equality.** The mining resource link and
   composition element link compare optional strings directly, so two absent/wrong-typed links
   satisfy the check. Those checks have no separate field-absence tally. Product matching uses
   an empty default expected string and substring matching against an error; any error contains
   an empty string. Require a valid expected GUID and a structurally identified dangling target
   before this can count as identity evidence.
   Sources: [resource link](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/mining.rs),
   [element link](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/mining.rs),
   [product matching](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/blueprints.rs).

4. **Current precision policy is broader than source-f32 equality and also covers integers.**
   Every numeric comparison uses max(absolute 1e-6, relative 1e-5). An integer buy-in or standing
   threshold can differ and still agree; adjacent synthetic integers above 2^24 are one concrete
   example. Converting every integer through f64 also loses distinction above 2^53. Integers must
   compare in their original integer representation. Source f32 fields should use finite same-f32
   rounding-cell comparison; an existing implementation/test already provides that policy.
   Sources: [tolerance](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/findings.rs),
   [buy-in reader](../crates/p4kx-export/src/mission_results.rs),
   [standing reader](../crates/p4kx-export/src/reputation.rs),
   [rounding-cell implementation](../crates/p4kx-export/tests/field_verification/compare.rs),
   [precision regression test](../crates/p4kx-export/tests/field_verification/mutations_compare.rs).

5. **The contract selector is heuristic and not the catalog selector.** The runner filters on
   a case-insensitive `contract` substring in type/path, then recursively accepts every object
   with a parseable GUID `id`, including sub-contracts without compared fields. This can miss
   contract-bearing records named otherwise and include unrelated embedded IDs. It provides
   no count of excluded source candidates. Catalog contracts have a ContractGenerator selection
   and authored entry/layer handling, with explicit skipped/failure accounting. Reconcile the
   selections with categorized exclusions; neither silently widening the scan nor replacing it
   with a catalog folder guess proves complete coverage.
   Sources: [runner selector](../crates/p4kx-extraction/tests/real_archive/scmdb.rs),
   [recursive collector](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/contracts.rs),
   [catalog selection](../crates/p4kx-export/src/selection_rules.rs),
   [catalog reader](../crates/p4kx-export/src/missions.rs).

6. **Duplicates are collapsed before comparison.** Contract IDs retain the first observation,
   standing GUIDs retain the last, dump Record GUIDs become a set, and source Records are looked
   up by GUID into a single-valued map. Conflicting physical occurrences cannot be assessed after
   this loss. Preserve source Record index/path and JSON selector for every occurrence, classify
   identical aliases separately, and report conflicting identities instead of choosing a winner.
   The inspected published merged files contain no duplicate contract-ID groups; that observation
   does not validate duplicate handling in source or future input.
   Sources: [contract insertion](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/contracts.rs),
   [standing map](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/contracts.rs),
   [requested GUID set](../crates/p4kx-extraction/tests/real_archive/scmdb_compare.rs),
   [Record lookup](../crates/p4kx-extraction/tests/real_archive/scmdb.rs).

7. **Nested comparison is shallow and position-dependent.** Blueprint slots/options and mining
   parts/parameters zip arrays only after equal lengths. A parent mismatch skips all children,
   so the field denominator omits those unreachable children. Blueprint modifier ranges flatten
   away their owning property and do not check the public `additive` field. Equal range numbers
   attached to a different property/mode can agree. Source crafting already retains recursive
   cost kinds, optional costs, gameplayPropertyRecord and typed range kind. A public propertyKey
   or resourceName must not be guessed to be a raw GUID: the inspected public property metadata
   has names/units/categories, not an attested source-GUID mapping.
   Sources: [range flattening](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/blueprints.rs),
   [typed costs](../crates/p4kx-export/src/crafting_tier.rs),
   [typed property/ranges](../crates/p4kx-export/src/crafting_tier.rs),
   [catalog cost contract](../crates/p4kx-export/src/contract_recipe.rs).

8. **Input bytes and identity are not frozen by the current runner.** `read_dump` reads an entire
   file and parses it without size/fingerprint/path-integrity checks; versions are selected from
   the first matching directory entry. No post-read fingerprint or Archive/reference mutation
   check is attached to a result. The recursive contract collector has no explicit work/depth
   accounting; the intermediate serde_json parse can reject deep canonical trees before the
   collector. The canonical JSON writer itself uses an iterative task stack. Reuse the existing
   offline seam and caller-owned reviewed budgets if robustness is added; do not add arbitrary
   magic limits or truncate source trees to manufacture success.
   Sources: [dump read](../crates/p4kx-extraction/tests/real_archive/scmdb.rs),
   [version enumeration](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/version.rs),
   [collector](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/contracts.rs),
   [iterative JSON writer](../crates/p4kx-dataforge/src/record_json.rs),
   [existing offline reader](../crates/p4kx-testkit/src/offline_seam/reader.rs).

## Frontend transformations and reward provenance

The [published frontend](https://scmdb.net/assets/index-D1I8EJYN.js) separates stored dump fields
from displayed rows. Its `Ov` function starts around character 206,064. Around 206,811 it reads
success reputation from direct `factionRewards`, otherwise `factionRewardsPools[factionRewardsIndex]`,
otherwise an empty list. It converts amount to displayed `repXP` with the small fragment
`repXP:$.amount||0`. Failure reputation is a separate direct `factionRewards_fail` list, also
normalized this way. Comparing displayed zero would therefore confuse a missing amount and an
authored zero. Names are resolved through faction/scope maps; displayed success entries retain
scopeGuid but not factionGuid, and displayed failure entries retain neither. Compare raw pool
members before display normalization, preserving faction/scope identity, outcome and multiplicity.

`Dv` starts at character 211,739. It accepts either a standing object or a GUID string and searches
scope rank lists. For an object that resolves to a rank, its name/minReputation/rankIndex come from
that rank. If it cannot resolve, the object keeps some original fields; an unresolved string
becomes null. The present comparator accepts only standing objects with `guid`, reads thresholds
from those objects and ignores `scopes`. It does not reproduce or validate this lookup. Current
LIVE/PTU minStanding/maxStanding fields observed here are objects or absent/null; string support
is a demonstrated frontend contract, not an observed populated case in these two files.

The frontend spreads `rewardUEC` through from the dump and displays it. That proves neither its
DataForge source field nor whether a fixed/currency-dependent/dynamic calculation produced it.
The `rewardIsDynamic` schema field alone is insufficient evidence of the generator's algorithm.
A browser value must not become a synthetic expected game value or be presented as raw equality
without identifying its owner and transformation.

Public structural observations, with counts only:

| Published merged file | Contract + legacy rows | Success pool references | Success pools | Rows with failure-reputation arrays |
| --- | ---: | ---: | ---: | ---: |
| LIVE | 1,565 + 291 | 1,599 | 191 | 47 |
| PTU | 1,577 + 291 | 1,610 | 192 | 21 |

Neither file populated direct success `factionRewards`; every populated success source uses the
pool-index representation. A direct-array-only implementation would miss the observed success
population. Pool members have `factionGuid`, `scopeGuid`, `amount`. These counts establish shape
and potential denominators, not agreement with local source values.

## Feasible next comparisons and explicit denominators

| Candidate | Owning P4KX source | Comparison contract and denominator |
| --- | --- | --- |
| Success reputation | `mission_results::gains`: ContractResult_LegacyReputation, missionResults first flag, contractResultReputationAmounts; reward Reference resolves to reputationAmount | Per authored result/amount occurrence, matched to raw success pool members by contract identity plus faction/scope/outcome with multiplicity retained. Preserve zero and negative integers. Count both-side, one-side, invalid/missing reward targets, ambiguous matches and excluded/derived references independently. Do not sum duplicate scopes. |
| Failure reputation | Same raw result family and missionResults representation, before the catalog's success filter | Preserve every authored missionResults occurrence with its original index, value and source type, along with result/amount References and resolved integer reward facts. Keep unproven outcome interpretation explicit: the current reader proves only its index-zero success selector. An unproven public failure-array mapping is uncomparable; original outcomes remain in the all-data v3 scope. |
| Fixed UEC reward | `contract_reward::fixed_rewards`: success ContractResult_Reward.contractReward currencyType/reward/max/plusBonuses | Exact raw integer comparisons only after the reference generator is shown to select the same fixed reward and currency/range/bonus semantics. Null/dynamic/ambiguous money is uncomparable or derived, never zero. Count fixed comparable occurrences separately from every observed rewardUEC row. |
| Complete standing thresholds | `reputation::reputation_scope` and `reputation_standing` | All scope/rank GUID occurrences published in scopes, with duplicate memberships retained. Count contract min/max mentions separately from unique standing Records. Confirm root/object/rank threshold disagreement rather than overwriting one representation. |
| Existing mining fields into the actual catalog | `mining::mineable_element` and `mineable_composition` | Preserve the existing raw field mapping, then independently require every selected source occurrence and catalog projection. Separate reference-only, source-only, selection-excluded and failed projection. Do not infer density/location formulas. |
| Blueprint range identity | `crafting_tier::modifiers` and `range` | Add additive/multiplier identity immediately for well-typed source-known ranges. Property identity requires a source-proven propertyKey mapping first; unknown key mappings remain uncomparable. Track every skipped descendant after parent mismatch. |

Sources: [reward selection and References](../crates/p4kx-export/src/mission_results.rs),
[reward index projection](../crates/p4kx-export/src/mission_index_read/projection.rs),
[sub-contract result inheritance](../crates/p4kx-export/src/mission_results.rs),
[actual reward caller](../crates/p4kx-export/src/mission_shared.rs),
[fixed money](../crates/p4kx-export/src/contract_reward.rs),
[scope/standing projection](../crates/p4kx-export/src/reputation.rs),
[mining projection](../crates/p4kx-export/src/mining.rs),
[crafting projection](../crates/p4kx-export/src/crafting_tier.rs).
These establish P4KX owners. They do not attest the unpublished SCMDB generator or authorize
conflating every outcome with success/failure. Start by representing unproven reference-to-source
mappings explicitly, rather than assuming an undocumented API/type semantic.

## Truthful Build handling

The present matcher compares filename game-version against the suffix of manifest branch and
compares change number. It parses but ignores channel. It does not compare any file's internal
`version`, BuildId, DCB digest, producer revision or source Archive fingerprint, and it returns the
first match in filesystem iteration order.
[Source](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/version.rs).

The current LIVE filename label is compatible with the coordinating agent's observed manifest
branch/change. The PTU public label says `4.10.3-ptu.13145606`, while that observed local manifest
branch says `sc-alpha-4.10.0` with change `13145606`. Equal change numbers do not resolve the label
mismatch. No numerical agreement should be claimed for PTU from this evidence. Do not rename a
file, loosen matching to change-only, or construct a public patch solely from the foreign label.

The Build type has an optional public_patch field whose documented source is an RSI Launcher log,
set explicitly by the caller. An actual independent observation could support a reviewed mapping;
the existence of the setter cannot do so.
[Build API](../crates/p4kx-archive/src/build.rs).

The inspected merged roots expose only `version` as a Build-related field. Blueprint/mining `meta`
objects contain total-count fields, not BuildId or source digests. Freeze exact input hashes, require
filename/internal-version consistency across all three files, explicitly preserve unknown generator
and DCB identities, reject ambiguity, and report mismatched/unproven identity with zero numerical
agreement. A local hash binds the inspected bytes; it does not authenticate their publisher or
prove that the publisher read the same Archive.

## Proposed synthetic regression sequence

No implementation or test changes were made by this research task. Keep every fixture wholly
synthetic and compare through the current `Dumps`, `P4kx`, `collect_contracts` and `compare` seam
where that seam can express the regression. Do not invent a new API in an alleged current test.

1. Distinguish missing/wrong-shaped root containers from valid typed empty families. The former
   produce explicit invalid-input outcomes; the latter retain zero coverage without falsely
   claiming numerical acceptance. Add selector-rename coverage without a per-domain minimum count.
2. Vary filename/internal version independently in each of three files; wrong game, change,
   channel, malformed label and multiple candidate labels remain explicit, with no change-only
   fallback. Identity mismatch must perform zero numerical comparisons.
3. Mutate a synthetic buy-in integer from 16,777,216 to 16,777,217 and mutate a standing integer;
   neither may pass through relative/f32 tolerance. Test exact integers above 2^53 without a f64
   conversion, finite f32 round trips and distinct adjacent source-f32 cells.
4. Feed repeated contract IDs in both orders, with equal and conflicting payloads, and repeated
   standing GUIDs with conflicting thresholds. Preserve source ownership and conflict diagnostics;
   never let encounter order decide the compared truth.
5. Prove current reward non-coverage using fully synthetic fixed-result and reputation-pool
   mutations. A money or reward amount mutation must either disagree after a supported mapping,
   or explicitly report uncomparable/unhandled coverage. It must never disappear as clean agreement.
6. Exercise inline/pool success reputation, index zero, invalid/out-of-range/fractional indices,
   direct-array precedence, missing amount versus zero, negative amount, multiple equal scope
   occurrences, unresolved reward links, parent/sub-contract inheritance and full outcome masks.
   Keep failure-only results separate from catalog reputationGains.
7. Mutate additive/multiplier kind and property identity with unchanged range numbers; remove
   ingredient/element/resource identities; use an absent/invalid expected product with an error.
   Parent-length mismatches must account for unvisited descendants. Unknown nested cost types
   and public-only name keys remain explicit rather than guessed.
8. Exercise deep valid canonical trees beyond serde_json's intermediate recursion envelope,
   source/dump mutation, linked input paths, duplicate JSON members and cancellation using the
   existing offline primitives and reviewed caller-owned bounds. Do not bless a raised limit or
   truncated tree merely because the current dataset passes.

The existing [eleven synthetic tests](../crates/p4kx-extraction/tests/real_archive/scmdb_compare/tests.rs)
cover branch/change filename matching, requested GUIDs, nominal equality, dangling product,
modifier disagreement, missing contract, broad tolerance and absence tallies. They do not exercise
malformed root shape, empty coverage, reward pools, duplicates, channel/internal-version identity,
recursive cost ownership or exact integer precision. Preserve the useful existing assertions while
adding evidence for those contracts.
