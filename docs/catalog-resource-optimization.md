# Catalog and embedded SDK resource costs

## Recommendation

Keep game-text classification, typed game-media references and the immediate three-state encryption marker on the default path. Keep Full as an explicit proof/export option for now. On one matched LIVE Build, choosing markers saved **21 minutes, 29.69 GB peak private commit and 1.39 GB compressed output**. The selective SDK already used markers in the consumer regression. Its next gains come from the work it performs to answer each question. A complete marker export still peaked at 8.77 GB and took almost twelve minutes. It is unsuitable as a fallback for a 2 GB local refresh. [Matched producer measurements][matched] · [Consumer report][consumer] · [Current contract][contract]

For the embedded SDK, start with one parsed session, the compact contract fields the consumer uses, Ship-root Loadouts, and one incremental batch of physical Item definitions. Measure that whole refresh before changing more internals. Then remove redundant Contract owner interpretation and make ordinary marker observations cheaper. For the complete producer, first reduce the data and proof handles kept alive from Loadout resolution through the later projections. If Full remains close to its ceiling, spill cold proof data to disk using an exact index. Encoding and parallelism are secondary to those preparation changes. The priorities below are engineering judgments. Only the measurements explicitly attributed to a run are demonstrated gains. [SDK entry points][sdk] · [Contract interpreter][interpreter] · [Staged Loadouts][staged] · [Full producer requirements][full]

## Verified implementation status

The compact SDK fields and physical Item-definition visitor are in `244cebb99`.
The focused integration suite checked 25 contract, six Item-definition and three
Loadout selection cases. Code and synthetic checks for the next slices cover
direct marker binding, committing staged contract text registrations, packing
idle Full DataForge uses during Loadout resolution, and filtering archive names
before normalizing XML keys. The producer now stages Ship and Item Loadout rows
in original order during resolution and retains the typed Ship join facts until
the resource topology step. The synthetic spool check compares the exact
serialized bytes and preserves the preceding output after cancellation.
The sequential writer streams rows into zstd and keeps an exact length index,
so preparing a single large Loadout row no longer needs its full uncompressed
JSON in memory. Replay still needs its largest single row. The retained Ship
join tree uses the authored name proof when it supplies the same port name and
checks cancellation while copying the reduced port data.
After the streaming change, strict Clippy, all nine spool unit tests, all 49
targeted Loadout unit tests and all 1,694 synthetic Catalog integration tests
passed. The installed-only Loadout unit test requires its Archive.
The first paired SDK probe reused the same compiled workspace libraries and
served only as a repeatability check. A corrected LIVE 13220821 probe compiled
both signed revisions with the same test profile (export opt-level 1, serde_json
opt-level 3), used one Rayon worker and ran each trial in a fresh process.
It observed the operating system file cache as it stood. The trial verified
fresh workspace-library compilation, source paths and executable digests before
using an executable. [SDK measurement method](catalog-contract-queries.md#paired-sdk-measurements)

| LIVE 13220821, English, MarkersOnly | Reference | Candidate | Peak private commit and output check |
| --- | ---: | ---: | --- |
| Contract summaries, count only, `244cebb99` to `0b86fb6c5` | 108.126 s | 89.185 s | 1.165 to 1.164 GB. Both emitted 5,188 summaries and 1,886 authored location rules. |
| Contract summaries, semantic digest, same revisions in reverse order | 87.051 s | 80.681 s | 1.166 to 1.162 GB. Matching ordered digests of every summary field, location rule, text and query coverage. |
| Item definitions, snapshot to visitor at `0b86fb6c5` | 50.188 s | 50.937 s | 3.280 to 1.091 GB, a 66.7% reduction. Matching physical row digests and coverage for all 23,685 rows. |

Contract timings exclude the callback. The Item-definition times cover the
whole process, including Archive setup and the same per-row digest work for
both API routes. The TagPaths check matched 18,961 values, immediate encryption
markers, completeness and coverage across four processes. Its elapsed times
overlapped (reference 8.816–9.194 s, candidate 8.559–9.952 s).
These results apply to SDK queries in the test profile. Integrated consumer
refresh and complete native producer acceptance use their own measurements.

The detailed baseline measurements below predate the implementation.

## What the measurements establish

| Build, route and revision | Time | Peak private commit | Interpretation |
| --- | ---: | ---: | --- |
| LIVE 13220821 complete Full, packaged Windows AMD64 `e59c7410f` | 1,972.305 s | 38.457 GB | 78.149 GB decoded, 1,469.19 MB compressed, 1.919 GB peak publication temporary plus spools. |
| Same LIVE, same executable, MarkersOnly | 709.319 s | 8.766 GB | 9.132 GB decoded, 80.38 MB compressed, 0.218 GB temporary plus spools. One worker, level 3, fixed export time in both. |
| PTU 13273298 complete Full, earlier package `4d334e716` | 4,841.461 s | 39.516 GB | Verified completed baseline for this earlier Build. Separate replay peaked at 17.112 GB. |
| PTU 13284307 complete Full attempt, integration head `194592a2a`, packaged merge `4e2e192e8` | 1,134.586 s until stop | 40.042 GB | One worker, level 3. The isolated repeat reached the decimal 40 GB application ceiling in `project-facility-interiors`. No completed artifact or final size. |
| Same PTU, integration head `7fe6916e`, packaged merge `6a407804` | 1,684.650 s until stop | 29.509 GB last observed | The 12 GiB host physical reserve interrupted `resolve-loadouts`. No completed artifact, lifetime process peaks or final size. |

GB and MB here are decimal. The paired LIVE runs reduced elapsed time by 64.0%, private commit by 77.2% and compressed bytes by 94.5%. Working sets peaked at 35.868 and 8.329 GB respectively. Archive-free replay matched the normalized digest of 1,100,150,803 retained tokens and each of the 905,999 true, 79,612,651 false and 4,060,095 unknown encryption markers. Both schemas carried the same 118 `x-gameText` annotations. This is a one-Build pair with markers run first. It establishes neither a cross-Build speed ratio nor media annotation coverage. The Catalog documents typed media paths, identities, relationships and metadata. Texture, audio and mesh payload exports are separate. A common Game-Media annotation across those references would be a new schema deliverable. [Paired LIVE and previous PTU evidence][matched] · [Media contract][media] · [Historical PTU receipt summary][oldptu]

For PTU 13284307, the local measurement receipt for the isolated repeat beginning 2026-11-26 06:33:46 UTC identifies package run 45762183499, artifact 15092206325, binary SHA-256 `877f17474776bfc0cea1f778be76e8ea8fd8aeff6447f369814155cefdb3600f`. Its observed lifetime private peak was 40,042,229,760 B and working-set peak 37,195,284,480 B. Sampled private maxima were 19.10 GB during Records, 37.07 GB during Loadout resolution, 33.56 GB during locations, 36.58 GB during ContractGenerators, 37.92 GB during independent configurations, 39.42 GB during weapon animation and 40.04 GB during facility interiors. These phase totals include earlier allocations. Attributing 40 GB to interior processing alone would misdirect the first investigation. The earlier attempt stopped on the separate host reserve during Loadouts at 29.32 GB lifetime private commit. Both stops preceded final publication. A third run on the same archive and package revision `6a407804` also stopped before publication. Its final sample showed 12,884,312,064 B available, 589,824 B below the host reserve, with 35.789 GB of remaining commit headroom. At the stop its sampled Windows lifetime counters were 29,508,784,128 B private and 28,223,492,096 B resident. The run did not reach the application ceiling or the same phase as its earlier repeat. These runs cannot establish a change in the complete producer peak or wall time. The package link establishes binary identity. The memory figures come from the local measurement receipts. [Native package][package] · [Measurement method][method]

Another PTU 13284307 Normal Full run began at 12:49:26 UTC with the packaged
Windows AMD64 CLI from merge revision `eada15a2`. It stopped 968.666 s later
during `project-weapon-animation`. The last sample showed 11,330,998,272 B of
available physical memory against a 12 GiB host reserve. Windows lifetime peaks
were 35,498,344,448 B private and 32,887,623,680 B resident. The application
ceiling was 40,000,000,000 B. The run produced no complete frame for
archive-free replay. A separate text search started at 13:05:28 UTC. In the next
six seconds available physical memory fell from 16,424,726,528 B to 11,330,998,272 B
while private bytes in the producer stayed at 35,494,363,136 B. The search process
later held 29,323,603,968 B of private memory and was stopped. The concurrent
load prevents a comparison of completed producer peaks. [Packaged CLI](https://github.com/p4kx-org/p4kx/actions/runs/45811044734/artifacts/15116366395)

An isolated repeat of the same PTU Build and CLI began at 13:40:54 UTC and
completed in 2,296.843 s. The Windows lifetime private-commit peak was
36,286,316,544 B, below the 40,000,000,000 B application ceiling. The working-set
peak was 33,867,112,448 B. The completed zstd file was 1,596,847,563 B with
84,336,679,622 decoded bytes. These are producer measurements for that packaged
revision.

The archive-free replay started at 14:37:10 UTC. It verified the input seal
and began reading the frame. It stopped after 2,091.976 s when remaining
system commit headroom reached 17,039,216,640 B, below the 16 GiB reserve.
Available physical memory stayed at or above 21,842,841,600 B. The verifier
reached 17,130,360,832 B peak private commit and 10,398,920,704 B peak working
set. The interrupted
replay produced no final source-closure or section result. The completed
producer file and its size measurements remain available.

The second archive-free attempt ended after 2,386.990 s at the same host
reserve. The verifier reached 17,130,283,008 B private commit while system
commit headroom dropped to 17,090,404,352 B. Available physical memory remained
at or above 18,018,533,376 B. These are stopped verifier measurements. Both
attempts retain the producer's sealed output and lack a terminal replay digest.

The packed verifier at `9db4a8766` completed both replay passes over that same
sealed frame in 4,880.338 s. Its Windows lifetime peaks were 8,714,493,952 B
private commit and 8,651,968,512 B working set. Across 4,847 host samples,
available physical memory stayed at or above 39,446,003,712 B and remaining
system commit headroom at or above 56,382,038,016 B. These minima cleared
the 12 GiB physical and 16 GiB commit reserves. The installed Archive remained
closed during replay. The receipt rechecked the original frame seal after
the final pass and reconciled 84,336,679,622 decoded bytes, 133,034,573 source
uses and 96,392,635 sourced cells. [Completed PTU replay and section counts](catalog-partitioning.md#completed-ptu-13284307-replay)

| Fresh process on LIVE 13220821, consumer revision `7526b777` | Seconds | Reported peak private MB |
| --- | ---: | ---: |
| Open Archive and DataForge | 0.8 | 1,040 |
| Full contract *domain*, `with_contracts`, default marker mode | 81 | 1,417 |
| `with_item_definitions` | 10.4 | 3,129 |
| `read_loadouts` / `visit_loadouts` | 41 / 86 | 3,502 / 1,040 |
| Integrated contracts, 0.4.2 / tested SDK | 1.3 / 90 | 781 / 1,595 |
| Integrated ship slots, 0.4.2 / tested SDK | 20 / 136 | 2,106 / 3,509 |

The maintainer matched 20 consumer state areas plus ship, part and mining slot files, with five additional ATLS owners expected. The earlier source-only contract reader and the new canonical projection have different responsibilities. The user's elapsed-time comparison at the application boundary is valid. It does not establish which mandatory joins can be removed. Summary queries existed before the tested revision. In a separate opt-level-1, English, one-worker marker verifier at `2dabe115f`, ordinary contracts took 268.00 s / 1.488 GB and summaries 90.76 s / 1.164 GB. Both returned 5,188 contracts and 1,886 location rules with matching compared fields and coverage. The 81 s figure measures a different binary and call. Subtracting the published timings across harnesses would give a spurious speed estimate. [Consumer report][consumer] · [Contract summary comparison][summaries] · [Revision ancestry][ancestry]

At `7ca457080`, the first 128 Loadout owners took 54.452 s in MarkersOnly. The typed callback cost under 1 ms, 51.645 s was outside it and the first callback arrived around 29 s. The later `0ff026d07` verifier ran the same 1,033 Ship rows with shared inputs in 899.546 s and without them in 1,015.841 s (11.45% reduction). Peak private bytes differed by 69,632. Ships and Items together took 1,062.033 s with reuse. The Ship rows and their dependency digests matched across scopes. These verifier runs use an opt-level-1 producer and do their own checks. A later Full LIVE traversal and artifact replay reconciled 10,812 rows and 2,379,065 contributing uses in 4,624.868 s / 1.667 GB. None of these is an integrated release-profile slot refresh. [Bounded visitor][bounded] · [Input reuse and parity][visitors] · [Changes after the consumer trial][after]

A follow-up consumer trial used integration head `7fe6916e` and LIVE 13220821 with
the game closed, three runs per measurement and private process peaks.

| Consumer operation | Tested SDK | 0.4.2 or full-query comparison |
| --- | ---: | ---: |
| Entire refresh | 228.8 s (215.5–250.6 s) / 2.44 GB | 57.0 s / 2.35 GB (0.4.2) |
| Slot phase | 148.5 s / 2.38 GB | 19.9 s / 2.11 GB (0.4.2) |
| Contracts | 41.9 s (summaries) | 114.2 s (full query) |

The ship, part and mining slot files matched the 0.4.2 outputs. The compact and full
query returned the same 5,188 contract identities and agreed on their common fields.
The two release flags requested after that test were added and checked on the same
Archive separately. The parts index covered 3,951 parts, of which only 520 were
factory-installed. This explains why a batch derived from ship Loadouts is incomplete
for that consumer. Its slot phase includes the SDK, callbacks and construction of the
consumer files. The aggregate measurement leaves the individual costs open. [Release flag parity][summaries]

A third VerseKit trial reported equivalent application output on LIVE 13220821
with SDK revision `75db174a3`, including contract reputation and the ship, part
and mining files. The isolated SDK timings use a fresh process per invocation.
Opening the Archive and parsing DataForge cost about 1.2 s and 1.04 GB private
memory on that host.

| Operation | Seconds | Private peak |
| --- | ---: | ---: |
| `visit_loadouts(All)`, empty callback | 82.0 | 1.04 GB |
| `visit_item_definitions(Records(All))`, empty callback | 11.9 | 1.04 GB |
| `with_ship_sources(All)` | 10.3 | 2.04 GB |
| Writing the slot files | 0.06 | |
| Integrated slot refresh | 122 | 2.36 GB |
| Integrated contracts through summaries | 31 | |
| Entire integrated refresh | 202 | 2.44 GB |

The Loadout visit reported 10,955 owners and 10,810 rows. The corresponding
0.4.2 slot phase took 15 s with a 2.11 GB private peak and its whole refresh
took 45 s with a 2.25 GB peak. About 20 s of the new slot phase came from
consumer code. The empty callback isolates SDK cost and the isolated Ship
snapshot exceeds the query budget. The individual process peaks and the
integrated application peak measure different workloads. This external report
does not time the subsequent admission-tree reuse or original-field indexing
changes.

On installed LIVE `13220821`, the separately built Windows AMD64 verifier at
signed revision `4aa8bcfa2` ran each MarkersOnly query in a fresh release-profile
process with one worker. The executable SHA-256 is
`8bfc79fcc0227d0a419762cb12181f89ea190af6b8dfc044ad078ae6f8b2f951`.

| Query and outcome | Elapsed (seconds) | Peak private commit (bytes) | Peak working set (bytes) |
| --- | ---: | ---: | ---: |
| `visit_loadouts(All)`, completed | 245.9727 | 1,091,764,224 | 1,022,652,416 |
| `with_ship_sources(All)`, English, stopped at the budget | 12.2815 | 2,022,715,392 | 2,004,758,528 |

Loadouts opened the Archive and parsed DataForge in 0.700 seconds. It took
2.818 seconds after that to reach the first owner and 245.090 seconds to
complete the visit. The visitor reported 10,955 owners, 10,812 rows and complete
dependency coverage. Peak private commit of the separate measurement process was
133,214,208 bytes. Minimum observed available host memory was 50,501,722,112
bytes. The Ship run has no completed count or elapsed-time result. These single
worker trials and the reported VerseKit timings use different execution
conditions and cannot establish a comparable throughput change.


For the next Loadout comparison, measure time to first owner, admission,
Record and XML cache misses, semantic Loadout resolution, port construction and
alignment in the same release-profile run. A cache can reuse original parsed
facts while keeping each owner's selected sources and diagnostics local. For
the Ship query, measure the selected Record plan, global playability preparation
and row projection separately. The measured snapshot retained decoded owner
trees alongside the developing result. The bounded reader holds up to 32
decoded owner trees per batch and merges newly observed owner evidence once.
Compare its complete rows and coverage with the earlier result before
attributing changes in time or peak memory. An identity-only query has a
smaller dependency scope when the consumer only needs Ship identity and labels.

The original-value index uses one map lookup to insert each value and
stores each optional span endpoint in one machine word. The Item definition
visitor uses this index for every readable Item Record, including records
outside Ship Loadouts. The memory change applies to the active Record's span
table. Its effect on elapsed time and process peaks needs a paired measurement.

For an embedded slot refresh, measure the resolved Loadout visit, the all-Item
definition visit, and any declared equipment visit separately. Time the consumer
callbacks and cache serialization within those measurements. Keep the Archive
Build, SDK build profile, locale selection, provenance and consumer output fixed.
Compare the completed ship, part and mining outputs before comparing timing or
private memory peaks.

## Implementation order for the embedded SDK

| Order | Change and benefit | Cost, risk and first falsification test |
| --- | --- | --- |
| 1 | Bind one Archive, decoded Entry and `ParsedCatalogQueries` to a refresh. Use the already-supported language selection for SDK queries. Skip opening expensive domains until requested. Reuse cached consumer results on an unchanged Build. This avoids repeated decoding, lazy index creation and work on unused domains. | Session construction, `session.parsed()` and a query on the returned parsed view have different lifetimes. Hold the parsed view across calls. Keep empty, missing and unreadable locale outcomes distinct. Compare complete process lifetime with one-shot calls and count Entry decodes, index builds and localization reads. Full producer language coverage stays complete. [Session][session] · [SDK and cache lifecycle][sdk] |
| 2 | Use the all-Item definition visitor for a complete parts index. For ship loadouts select Ship roots, including their reached nested Item ports. Read declared constraints in one visit if the parts index needs authored ports and attachment rules. Use the all-owner Loadout visitor when standalone Items require resolved ports or default equipment. | Ship-derived batches are suitable for installed subsets. Preserve duplicate physical declarations, missing batch indices, failed resolutions, empty Ships and variant ties. Measure each SDK call separately from callback and file construction. Compare the emitted ship, part and mining files against the same consumer baseline. [Equipment selection](catalog-sdk.md#choose-the-equipment-scope) · [Item visitor][iv] · [Owner visitor][ev] |
| 3 | Use `with_contract_summaries` with the required rewards, titles, standing/guild references, source layers and location rules. Resolve protected text and external reward/standing references through the corresponding typed queries. The expanded fields and scoped Item visitor are available in the integration branch. The test-profile comparison above confirms SDK parity. The integrated refresh still needs a consumer measurement. | Keep legacy `with_contracts` for clients that consume its complete result. Verify all actually consumed fields and source states including failed/absent rows, hashes and measured integrated latency. A different field mask cannot by itself remove the cost of whole-owner validation. [Summary model][summarymodel] · [Summary measurements][summaries] · [Consumer report][consumer] |
| 4 | Count contract surveys, trees and transaction attempts. Optimize the `All` route first. It inserts every surveyed owner tree into the plan and later consumes them in physical order. Process in windows or one owner at a time while retaining compact logical keys, grants, aliases, pending winner inputs and coverage until the global finalizer. Keyed reads can instead consult a lazy compact reverse index within one parsed session. | Rebuilding owner trees to save RAM can regress elapsed time. Compare streaming/windowed survey+interpretation with retaining all trees. A keyed plan still requires proof of related outside owners, duplicates and intro grants. Never publish a winner before validation of every relevant physical contender. [Owner plan][plan] · [Consumption][read] · [Global joins][joins] |
| 5 | Remove redundant whole-owner interpretation. The reference implementation restarted on demand and replayed each terminal success. The current implementation commits the successful owner, exact consulted lookups and local text registrations together. The shared input exposes a borrowed prepared place lookup. Keep owner retries under measurement. | Count dependency misses, retry depth, tree visits, successful terminal replays, per-owner text publication and forward body reads. Preserve canonical failure, rollback, output order, source identity and exact consulted place dependencies. A resumable interpreter or batched *typed* demands is a later option when retries dominate. Eagerly collecting every raw Reference can activate unrelated failed inputs and change coverage. [Interpreter][interpreter] · [Dependencies][deps] · [Canonical dependency policy][joins] |
| 6 | Build the compact contract output earlier. The existing summary drops source arenas and several pools but still calls the common finalizer and converts finished `ContractWithTitles` rows via `ContractSummary::from_row`. Retain the winner/validation facts once, then project only the requested row shape and texts. Use the same required validators and joins for both projections. | Maintain a single meaning of every field. Compare summary to the corresponding full row for every winner, alias, linked rule, reward and protected text. Compile-time typed projections are preferable to a generic field mask until independent clients demonstrate different stable subsets. [Result finalization][finish] · [Summary fields][summarymodel] |
| 7 | Profile Loadout startup/admission, resolver, XML decode, tree sizing, per-owner inventory/alignment setup, callback and terminal coverage separately. The all-owner admission path builds Item trees to decide port eligibility. Reuse a compact physical admission index or focused original-component accessor, and share verified immutable resolver input across owners. | The post-trial path already shares Entity/path indexes, immutable trees (2,048 entries / 32 MiB estimated) and XML (256 entries / 8 MiB), plus enum options. Measure hits, misses, bytes, oversize exclusions, lookup and eviction costs. The record-cache sizing pass walks decoded values; charge size once during construction if it is material. Cache source reads separately from per-owner observations, contextual replacements, failures and diagnostic coverage. Identify repeated marks before changing FIFO eviction or caching resolved subtrees. [Admission][scope] · [Resolver and caps][resolver] · [Record cache][rcache] · [Reuse measurement][visitors] |

At 1.04 GB just to open on the reported Build, the 2 GB process gate leaves roughly 0.96 GB private-commit headroom in that observation. A process per query charges that base repeatedly. A shared session eliminates repeat work while keeping the base live. Running overlapping expensive query and callback pipelines can still breach the cap. For narrow reads, try a declaration-to-physical-header index and borrowed access to exact fields before building complete generic trees. Preserve actual source positions, required validation, order, dangling targets and unreadability, including validation of omitted sibling fields when the contract requires it. Track rows visited versus rows emitted to catch a cheap-looking projection doing a hidden full scan. [Open measurement][consumer] · [Tree materialization][trees] · [Query coverage requirements][allq]

## Implementation order for the complete producer

**1. Shorten the lifetime of large typed rows.** The reference producer spooled Item rows and retained full Ship Loadout trees for capacity, facility and resource-topology joins. The current producer spools both row types in canonical order and keeps a smaller typed Ship join tree. Measure its owned bytes and outstanding proof handles at the end of resolution and before each subsequent consumer. Alternatively, replay indexed typed Ship rows one at a time. Fold compatible consumers into a single owner pass once their prerequisites are available. Keep the original ordinal, empty owners, contextual replacements, transforms, evidence and deterministic tie handling. Calendar time may improve by eliminating repeated walks, or worsen if decoding and disk seeks dominate. A spool of externally visible marker JSON cannot reconstruct internal proof/authority used by later joins. Use exact internal join data, and compare the three downstream tables independently. [Staged representation][staged] · [Facility joins][facility] · [Producer staging and retention][method]

**2. Measure and reduce the earlier high-water marks.** Instrument Record windows, held rows versus projected facts, packed versus live uses and bytes by family, outstanding window tasks, and each spool buffer's peak capacity. At facilities, record per-host coverage rows, attachments, emitted instances, visited nested SOC/room occurrences, retained path index, helper geometry/model bytes and callback/flush times. The code collects root hosts, attachments and coverage and accumulates instances. The model helper keeps a most-recent host. Flush completed hosts/rows in stable order, index configuration links once where reused and evict large transient model/parser/serialization buffers at a real lifetime boundary. Factor immutable geometry/helper data from placement-specific occurrences if repeat reads dominate. An identical model path alone cannot prove equivalent physical placement, transform, selected variant or evidence. A path change or unreadable entry must remain visible. At PTU's sampled 39.42 GB during weapon animation, reducing interiors alone would provide little margin. [Interior projection][interior] · [Room helper][helper] · [Staging and prior memory trials][method]

**3. Avoid per-observation proof construction that markers do not need.** Marker union already aggregates encryption and completeness without retaining contributor arrays. Direct single-source value binding already avoids detailed reference construction in MarkersOnly. Paths that still require a `SourceUse` can derive an ID through `register_use` and `repeat_use` or allocate an `Arc<SourceUseDescription>` through `retain_use`. Attempted observations are deliberately registered because health verifies identity and role. Measure by role and consumer before adding a fast path. For internal marker-only calls that need solely contribution semantics, propagate a small authority-bound receipt carrying verified encryption, contributor role and opacity/completeness. Construct detailed IDs/coordinates lazily at API points that require them. Preserve foreign-capture rejection, attempted-versus-contributing distinctions, failed and absent source behavior, and checks for missing or ambiguous inputs. The source registry and the accumulator already own these rules. A separate reimplementation of the marker calculation risks disagreeing on mixed or incomplete evidence. This can benefit both SDK and producer without removing Full. Its absolute savings need a paired run. [Use registration][registry] · [Origin union and attempted validation][union] · [Marker accumulation][acc] · [Inventory sealing][freeze]

**4. Compact or spill the Full proof inventory before the final peak.** Existing compaction packs unheld DataForge uses at windows into a sorted representation, uses 40-byte entries on 64-bit targets for that measured layout, and transfers the map at sealing. A historic PTU seal temporarily added 4.85 GiB private bytes when copying a large packed array. Reusing sealed storage already addresses that copy. First measure current bytes in string IDs, coordinate segments, maps, allocator slack, per-cell shared prefixes, live handles and foreign/non-DataForge locations. Trial typed dense internal IDs, interned physical sources and site components, compact tables split by coordinate kind, and merge frozen sorted runs at serialization. Use existing IDs at the wire boundary if compatibility requires them. For a disk-backed alternative, evict sealed cold entries, keep a bounded hot index/cache and use a deterministic exact lookup for recurring coordinates. An append-only log alone cannot reconcile later reused coordinates. Compare sparse key/index cost, external-sort or keyed-store lookup/write amplification, spill volume and cancellation under a slow disk against in-memory compaction. Duplicate or conflicting inputs must be detected, IDs stable within the generation, ref closure and original ancestry recoverable, and disk-full/corrupt spill must refuse publication. Parallel shard-local IDs require a deterministic final remap or a canonical physical key. A per-owner evidence inventory alone cannot replace the global document. [Packed and sealed baseline][method] · [Registered uses][registered] · [Source registry][registry] · [Full gate][full]

**5. Reuse staged bytes during sizing and final emission.** Several tables already use compressed sequential or indexed row spools. The final zstd writer counts exact serialized JSON bytes and then serializes the same prepared snapshot again. Sequential spools retain a largest-row bound, and indexed writers record original and compressed lengths and sorted keys. Prototype accumulating exact serialized lengths for sealed rows/sections, including punctuation, wrapper and newline, so counting does not replay every row. Where the bytes are already validated and generated by the same canonical serializer, consider direct replay into the final writer. Verify raw byte equality and error propagation on nested escaping and very large rows. Internally compressed blocks trade CPU for scratch I/O. Test grouped frames versus per-row random seek for the real key distribution. Streaming subrows instead of buffering the largest row can help one pathological owner. A raw full-document spool would trade a second serializer traversal for about 9 or 78 GB of scratch on the paired files. Compare only when storage permits and a serializer profile justifies it. Keep pledged single-frame length, checksum, transactional output ownership, health refusal and cancellation. [Output passes][output] · [Sequential spool][spool] · [Indexed spool][ispool] · [Atomic publication][atomic]

**6. Change the wire only for measured consumer or producer benefit.** On an earlier LIVE artifact, six Loadout port attributes contained 690,731,520 repeated use references occupying 29.87 GB of decoded JSON. On an earlier PTU pair, shorter compatible ID strings reduced decoded size by 11.43 GB (13.35%) while compressed bytes fell only 0.42%. A separate PTU census modeled at least 8.07 GB decoded savings for positional provenance rows and small vocabulary codes. These are earlier Builds and sometimes modeled effects. They cannot be added to the paired MarkersOnly savings. Candidate Full encodings include a per-port base/prefix plus local use IDs, dictionary-coded physical coordinates, pooled animation endpoints and relationships, and positional source-use rows. Prioritize by residual time re-expanding/comparing/serializing and by importer cost. Preserve on-datum immediate markers and complete expansion to the original ordered physical facts, including distinct contextual paths. Dictionary/index memory can negate a size advantage. An optional domain bundle with an exact generation manifest and independent frames can improve selective downloads, with additional producer/reader contracts and compression overhead. Alternate binary/columnar layouts are most credible as optional projections or internal typed spools, with explicit version negotiation for any interchange change. The unfiltered marker file already exceeds both the 64 MB upload and 1 GB decoded reference limits. The separate reduced projection/import still needs its own acceptance. [Measured byte census and models][sizes] · [Transport and manifests][transport] · [Reduced consumer gate][reduced]

## Other options and when to spend engineering time on them

| Option | Trigger and upside | Cost or limiting condition |
| --- | --- | --- |
| Archive directory and DataForge backing | If isolated opener measurements dominate an individual query, profile directory rows/paths, normalized keys/indices, decoded Entry capacity, transient decoder buffer and lazy schema indices separately. Share immutable path/header dictionaries across related accesses. Consider compact path storage and bounded file-backed decoded pages if resident input remains the bottleneck. [Archive][archive] · [Decoded input][dcb] · [Existing baseline][perf] | The Archive keeps a read-only source and a directory of entries. The session owns or borrows a decoded byte vector. Mapping compressed Archive bytes does not expose decoded DataForge offsets. A decoded mapping requires checked decompression to protected, generation-bound local storage or a format-aware indexed decoder. It adds writes, faults, invalidation and lifetime/cleanup work. Measure working set, private commit, total host pressure and disk together. Current growth admits received bytes incrementally. Preserve size/EOF/cancellation checks and avoid preallocating arbitrary claimed sizes. [Buffer admission][buffer] · [Input identity][identity] |
| Typed field and join projections | Build a header/key index lazily per parsed session, including all physical candidates. Extract compact, source-bound facts once for repeated joins. Keep translated text lookup demand-driven for the selected language. Narrow original-field cursors or visitor callbacks can skip allocating unrelated subtrees. Cache negative/failure evidence only with its exact input and lookup scope. [Query contract][allq] · [Original field semantics][trees] · [Dependency reader][deps] | The caller may need exhaustive validation even when only a few values are output. Keep physical occurrence, null, absent, unreadable, ordering and global selection semantics. Count exact body reads, allocations avoided and retained index bytes. A one-off lookup can be cheaper without an index. |
| Shared data structures and allocation | Profile allocations and cycles by type. Replace repeated path/name/ID ownership with borrowed slices, interned strings or compact handles where lifetimes permit. Compare dense vectors/bitsets on physical ordinal domains with sorted arrays or hash maps on sparse keys. Retire oversized reusable capacities after outsized inputs. Pool scratch storage per owner/window with an aggregate byte budget. [Packed proof evidence][method] · [Record cache][rcache] · [Physical Item identity][items] | Interner metadata, hash work and longer lifetimes can offset savings; borrowed slices can pin an entire buffer. Preserve deterministic canonical sort and conflict checks. Changing allocator, hash implementation, compiler profile/LTO or adding profile-guided optimization belongs after allocation/file/CPU traces identify residual costs. Benchmark the shipped release binary on held-out Builds. |
| Bounded parallelism and I/O | After reducing per-owner state, sweep one, two and four workers under the same host reserve for independent Record windows, decompression or read-only lookups. Bound pending **bytes** and the reorder buffer. Single-flight a repeated immutable read if concurrent misses duplicate materialization. Batch nearby Archive reads when the physical Entry plan permits stable output. [Measured thread variation][perf] · [Producer phases][method] · [Existing input cache][rcache] | Owner-local provenance authority, global duplicate selection, stable first-observer semantics and cancellation still apply. Slow early owners can grow an ordered-result queue. More workers can multiply trees, decoder windows, scratch and memory bandwidth contention. Use bounded work submission, stable merge order and a one-worker baseline. SDK refreshes may be sequential under the 2 GB budget. |
| Serializer and compression policy | The paired measurements used zstd 3. Current `.zst` path defaults to 19, long-distance matching, a maximum window log of 27 and checksummed single frame. Measure raw/count-only and encoder-only passes of a retained snapshot, then levels 1, 3 and 19. Tune spool compression independently. A faster level can reduce publication time on a CPU-bound normal run. [Encoder][output] · [Published wire][contract] | Compression cannot reduce the pre-serialization PTU peak. Level, window, worker count and dictionaries change encoded size, encoder/reader footprint or compatibility. A new default or optional multi-frame output requires measured reader limits. RFC 8878 distinguishes frame content size, window and checksum. [Frame specification][zstd] |
| Prebuilt domain data and disk-backed incremental refresh | For repeated installs, produce a small versioned same-Build domain projection once and import it by streaming to an atomic local cache. A local dependency fingerprint can allow unaffected domains to be retained on update. Warm cache hits avoid opening DataForge where the caller has a verified matching receipt. Compare cold local read versus cold import, changed Build, warm hit, failed import and offline start. [Consumer scope][verse] · [Generation manifest][transport] · [Measured mining cache][mining] | Keep offline selective reads available. Bind producer/schema revision, exact Build/Archive, locale, domain and options, input dependencies, integrity and coverage. An added duplicate elsewhere can change a canonical winner, so a changed-row hash alone is insufficient for negative lookups or global joins. Stage immutable outputs and publish only complete validated generations. Distributing protected game data requires an explicit product decision about what may leave the local installation. A content digest detects damage; distribution from an untrusted source additionally needs origin authentication. |

## Retaining or retiring Full

| Option | What the user gets | Consequence and decision |
| --- | --- | --- |
| Markers by default, Full by explicit request | Existing values, classifications and immediate encryption in ordinary use. Investigators can reconstruct contributors, physical coordinates, roles and source ancestry for an exported generation, even without its Archive. | Recommended under the current contract. Optimize the marker path independently and run expensive Full validation as a distinct production/release job. Source selection and SDK scoping still matter. [Catalog profiles][contract] · [Full gate][full] |
| Markers in public builds, Full behind a separate producer feature/build or companion | Smaller public surface and an optional internal validation artifact. An on-demand exact-owner proof query can make individual investigations affordable. | Public standalone Full CLI/GUI/API behavior, packaged release gates, replay tooling and consumer diagnostics change. Engineering complexity remains wherever Full is supported. A targeted proof needs the same original Build, Archive and source closure. Record indices and detailed evidence cannot in general be reconstructed from the marker artifact alone. Specify support lifetime and proof artifact access. [Canonical decision][adr] · [SDK contract][allq] |
| Remove Full everywhere | Fewer proof data structures, code paths, fixtures and supported formats to maintain. People who formerly ran Full obtain the measured benefit of switching to markers on the paired Build. | The SDK and standard producer already select markers. Deleting dormant Full code has no demonstrated end-user elapsed/RAM benefit for them. Removal would retire archived per-value evidence, offline source-closure replay and the complete #1151/#1166 comparison workflow. It needs an explicit revision of the public contract, release evidence, migration and agreed replacement for independently checking source attribution. Preserve an independent marker oracle using original fixtures and actual DataForge/Archive observations. A marker cannot reveal the identities of its contributing sources. [Full requirements][full] · [Source semantics][adr] · [Installed comparison][matched] |

Across all options, game-text/game-media classification and encryption remain independent. Derive encryption from contributing inputs, roles, completeness, known opacity and ancestor state; preserve true for a known encrypted contributor, false only for completely known clear inputs, otherwise unknown. Preserve value state, public versus protected localized text, decoded/display forms, typed media references and the consumer's publication/filtering rules. If the product requires a uniform Game-Media annotation, enumerate its schema paths and non-media controls and verify both profiles; the earlier schema count refers only to game text. [Accumulator][acc] · [Catalog media][media] · [Per-value provenance decision][adr]

## Measurement and release sequence

1. **Reproduce the consumer path.** Pin SDK and consumer revisions, rustc and optimization profile, exact Archive/Build receipt, worker count, locale, mode, selection and cache state. On LIVE 13220821 if retained, compare the original #1185 call chain with the scoped route. Record cold and warmed filesystem cache policy, separate fresh processes versus one reused session, and measure time to first row, outside the callback, callback, and terminal success. Include 20 state areas and the ship, part and mining outputs. For local refresh distinguish cold install, changed Build, warm reuse, selected on-demand query and cache retention on cancellation or unreadable input. Report SDK-plus-helpers memory and integrated application footprint separately. [Issue reproduction][consumer] · [VerseKit acceptance][verse]
2. **Attribute before changing the representation.** Collect wall/CPU time, allocation samples and outstanding bytes at phase boundaries, exact registry counts/bytes by retention type, owner/Record counts, largest tree/row, attempts/demand misses, cache hits/misses/evictions and disk I/O. Sample cancellation latency during a large tree, a retry, a large row, spill merge and final flush. Keep traces aggregate and collect instrumented runs separately from release timings. Use fresh processes, alternate comparison order, repeat affordable scenarios and report medians/ranges plus individual peaks. Separate process logical writes, scratch high-water, publication temporary and actual storage/paging. Sampled directory size can miss anonymous spools or transient allocations. Measure simultaneous helper usage, resident and private-commit peaks plus host headroom. Windows supplies distinct lifetime peaks for working set and commit. [Measurement guidance][method] · [Windows counter definitions][memory]
3. **Prove each substitution.** Compare canonical order and occurrence-aware hashes including duplicate GUIDs, physical indices, context overrides, global winners and failed-owner rollback, incomplete scopes, missing/ambiguous/opaque inputs, combined clear/encrypted/unknown contributions, locales, protected texts, media reference classification, normal/diagnostic health and cancellation. Full changes also compare every expanded physical/use relationship and ancestor. Raw and compressed writers need byte-for-byte decoded equivalence on fixed export time where the contract is unchanged. A wire redesign needs a lossless normalized comparison plus old/new reader fixtures. Failures must leave the last complete cache/output usable. [Health semantics][health] · [Parity and independent replay][matched] · [Whole-owner dependency policy][joins]
4. **Run the release gates independently.** Individual API and SDK queries must stay below 2,000,000,000 bytes. Measure the embedded P4KX refresh below 2 GB including helpers and report total integrated VerseKit memory separately. The reduced Basetool producer may use up to 40,000,000,000 bytes. Complete Normal production must stay below 40,000,000,000 bytes on both installed LIVE and current PTU. For each workload, use the higher available private/resident measurement and report OS headroom on a 16 GiB query host or a 64 GiB producer host. Use an exact candidate package, all required fields/languages and normal health, with a separate decoded checksum, byte count and reference-closure replay. A stopped run gives a lower bound on the memory needed by that execution, and no completed wall-time or artifact-size result. Acceptance at the prior revision does not qualify a changed package or Build. [Producer criterion][full] · [SDK query criterion][allq] · [Integrated criterion][verse] · [Reduced export criterion][reduced]

The implementation decision for today is to keep the default markers, finish and measure the scoped consumer path, then remove repeated Contract work and shorten Ship/proof lifetimes. Revisit removal of Full only if the project explicitly retires portable source-level verification. Decide on a shared media annotation, a different interchange wire, prebuilt data distribution or a new compression default as separate product changes once their reader contracts and measurements are known. The research used repository source, existing installed receipts and first-party issue reports. The implementation measurements are reported above.

[matched]: catalog-partitioning.md#packaged-catalog-provenance-mode-comparison-on-live-13220821
[consumer]: https://github.com/p4kx-org/p4kx/issues/1185
[contract]: catalog.md
[sdk]: catalog-sdk.md
[interpreter]: ../crates/p4kx-export/src/contract_query_interpret.rs
[staged]: ../crates/p4kx-export/src/staged_loadouts.rs
[full]: https://github.com/p4kx-org/p4kx/issues/1151
[media]: catalog.md#media-files
[oldptu]: catalog-partitioning.md#packaged-full-measurement-on-ptu-13273298
[package]: https://github.com/p4kx-org/p4kx/actions/runs/45762183499/job/139714875582
[method]: catalog-partitioning.md#measuring-a-complete-export
[summaries]: catalog-contract-queries.md#installed-live-comparison
[ancestry]: https://github.com/p4kx-org/p4kx/compare/2dabe115f...7526b777
[bounded]: catalog-partitioning.md#bounded-resolved-loadout-visitor-timing-on-live-13220821
[visitors]: catalog-sdk.md#visit-loadout-owners-incrementally
[after]: https://github.com/p4kx-org/p4kx/compare/7526b777...194592a2a
[session]: ../crates/p4kx-export/src/catalog_query_session.rs
[scope]: ../crates/p4kx-export/src/equipment_loadout_scope.rs
[items]: catalog-item-definitions.md
[iv]: ../crates/p4kx-export/src/equipment_item_definition_visit.rs
[ev]: ../crates/p4kx-export/src/equipment_owner_visit.rs
[summarymodel]: ../crates/p4kx-export/src/contract_summary_query.rs
[plan]: ../crates/p4kx-export/src/contract_query_plan.rs
[read]: ../crates/p4kx-export/src/contract_query_read.rs
[joins]: adr/0401-contract-sdk-selection-retains-global-join-dependencies.md
[deps]: ../crates/p4kx-export/src/contract_query_dependencies.rs
[finish]: ../crates/p4kx-export/src/contract_query_finish.rs
[resolver]: ../crates/p4kx-export/src/equipment_loadout_read.rs
[rcache]: ../crates/p4kx-assembly/src/loadout_record_cache.rs
[trees]: dataforge-materialization.md
[allq]: https://github.com/p4kx-org/p4kx/issues/1166
[facility]: ../crates/p4kx-export/src/facility_loadout_placements.rs
[interior]: ../crates/p4kx-export/src/facility_interior_placements.rs
[helper]: ../crates/p4kx-export/src/facility_model_helper.rs
[registry]: ../crates/p4kx-extraction/src/source_registry.rs
[union]: ../crates/p4kx-extraction/src/source_origin_union.rs
[acc]: ../crates/p4kx-extraction/src/field_origin_accumulator.rs
[freeze]: ../crates/p4kx-extraction/src/source_inventory_freeze.rs
[registered]: ../crates/p4kx-extraction/src/source_registered_uses.rs
[output]: ../crates/p4kx-export/src/catalog_output.rs
[spool]: ../crates/p4kx-export/src/catalog_row_spool.rs
[ispool]: ../crates/p4kx-export/src/catalog_indexed_row_spool.rs
[atomic]: ../crates/p4kx-export/src/catalog_file.rs
[sizes]: catalog-partitioning.md#reducing-the-uncompressed-document
[transport]: catalog-partitioning.md#transport-options
[reduced]: https://github.com/p4kx-org/p4kx/issues/1177
[archive]: ../crates/p4kx-archive/src/archive.rs
[dcb]: ../crates/p4kx-extraction/src/dataforge.rs
[perf]: performance.md
[buffer]: ../crates/p4kx-extraction/src/decoded_buffer.rs
[identity]: dataforge-input-origin.md
[zstd]: https://www.rfc-editor.org/rfc/rfc8878
[verse]: https://github.com/p4kx-org/p4kx/issues/1165
[mining]: catalog-sdk.md#measure-a-local-mining-cache
[adr]: adr/0356-catalog-v3-uses-canonical-source-typed-tables.md
[memory]: https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex
[health]: catalog-export-health.md
