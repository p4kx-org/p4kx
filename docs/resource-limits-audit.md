# Resource limits and source completeness

A limit is not a format invariant because it is a constant, returns a typed error, or has not been
reached by one installed Build. Source retention, interpreted expansion, transport, presentation
and scheduling need different decisions. This audit tracks the remaining policies for v0.5.0;
its open entries do not describe completed fixes.

## Decision rules

- Retain every readable original occurrence, its order, exact value and immediate source origin.
  An implementation cutoff must not become an empty list, absence, an unknown target, or a complete
  inventory. Raw retention and successful interpretation are separate results.
- Prove format constraints from the actual representation: integer widths, remaining bytes, block
  layouts and valid indices. Do not replace a source-derived bound with a guessed aggregate count.
- Use iterative traversal and actual cycle detection for finite graphs. Preserve repeated source
  occurrences; a repeated reference is not necessarily a cycle. Shared source arenas avoid copying
  the same subgraph into every consumer. An acyclic graph can still have exponentially many paths.
- Charge resource admission to the work or storage it actually controls. Make a refusal precise and
  keep source/read/projection completeness separate. A larger constant alone is not a clean fix.
- Keep exact source data available independently of display quality and requested previews.
  Pagination needs an actual route to every tail item, including an oversized item; a total count
  alone does not provide that route.
- Retain useful IO chunks, cache eviction and concurrency controls when they change scheduling or
  residency without excluding source data. Device limits require actual capability checks.

## Audit coverage

The source inventory inspected 2,599 production Rust files across 31 workspace crates and retained
all 2,657 lexical candidates. Reconciliation covered all 488 previously classified, overlapping
candidate groups. Those are not 488 distinct artificial limits. The table groups the remaining
high-impact paths into 27 families. The inventory checkpoint verified 103 exact source bindings. Integrated fixes below have their own
regression evidence; the inventory counts are not a fresh census of later workspace additions. Inline tests and dependency defaults need separate interpretation; this is
not proof that every implicit arithmetic restriction or transitive dependency limit is absent.

## Remaining policies

All maxima below are implementation policies unless a separate format proof is stated. Their
presence here does not establish sufficiency for future Builds.

| Area and owning source | Current policy | Assessment and required direction |
|---|---|---|
| Canonical [DataForge builder](../crates/p4kx-dataforge/src/build.rs) and [layout](../crates/p4kx-dataforge/src/layout.rs) | No fixed owned-depth, materialized-Value or inherited-field quota; optional caller allowance per read | Shared physical declarations and borrowed layout cursors retain all inherited occurrences. Original positions use u64. [Materialization admission](dataforge-materialization.md) counts output Values, separately from metadata, scratch and platform capacity; [remaining consumer boundaries](canonical-owned-values.md) remain explicit. |
| [Reference index](../crates/p4kx-dataforge/src/reference_index.rs) | Complete by default. Callers can request an occurrence budget. | Retained links use their actual physical targets. Offset widths follow platform addressability. An explicit budget reports truncation at the first omitted link. |
| [Crafting duration](../crates/p4kx-export/src/blueprint.rs) | Rounded float components fit the signed i64 interval. Each factor and sum is checked. | The source test covers finite inputs above one billion, both representable boundaries, and real arithmetic overflow. The complete Blueprint arena retains original numeric values independently of the duration projection. |
| [Query](../crates/p4kx-dataforge/src/query/mod.rs) and [conditions](../crates/p4kx-dataforge/src/query/condition.rs) | 64 path segments; 16 Reference follows; condition depth 32; text 65,536 B; projected-cell work 4,096 | Input admission and materialized projection are separate from complete streaming predicate selection. Removing parser recursion and controlling materialization are different changes. |
| [Containers](../crates/p4kx-archive/src/container.rs) | Nesting 8; entries 2^20; decoded chain 512 MiB; expansion ratio 2,048 | Chosen decoding and index admission. A high compression ratio alone does not prove invalid content. Track real decoded storage and nested ownership without claiming universal sufficiency. |
| [Text XML](../crates/p4kx-cryxml/src/depth.rs) and [localisation](../crates/p4kx-localization/src/global_ini.rs) | XML depth 128; global.ini 64 MiB and 2^20 keys | Parser/index policies, not demonstrated source maxima. Preserve lexical errors separately from resource refusal and inspect dependency recursion defaults. |
| [Loadout](../crates/p4kx-assembly/src/lib.rs) and [XML includes](../crates/p4kx-assembly/src/xml.rs) | Depth 32; ports 65,536; include depth 8 and files 256 | Interpreted expansion can be partial. Distinguish actual cycles, source slots, shared models and expansion work; retain every original declaration independently. |
| [Prefab](../crates/p4kx-assembly/src/prefab/mod.rs) and [Interior](../crates/p4kx-assembly/src/interior/mod.rs) | Prefab depth 16, objects 2^19, visits 2^22, issues 4,096, library 64 MiB; Interior depth 16, parts 65,536, containers 1,024, file 128 MiB | Expansion and diagnostic storage need separate accounting. Keep authored source completeness independent of target model admission. |
| [Geometry variants](../crates/p4kx-assembly/src/geometry.rs) and [material choices](../crates/p4kx-assembly/src/material_variant.rs) | No additional SubGeometry or MaterialVariant depth/count fence | Both projections retain the complete materialized owned tree. Material construction, selection and tree operations are iterative; the upstream canonical policy remains separate. See [ADR-0366](adr/0366-material-choices-retain-the-complete-owned-source-tree.md). |
| [Audio source and indexes](../crates/p4kx-assembly/src/audio/mod.rs) | Bank 32 MiB; ATL 8 MiB and nodes 2^20; triggers 2^18; loose WEMs 2^22; languages 1,024 | Chosen source/index admission. Source size, decoded residency and index cardinality are different costs. The separate indexed-bank-count fence is removed. |
| [Audio graph](../crates/p4kx-assembly/src/audio/walk.rs) | Path 32; event/trigger chain 8; children 1,024; event objects 16,384; trigger work 2^21 | Replace recursive graph control with actual cycle handling and shared ownership before uncapping valid chains. Active decoded residency is not constant memory. |
| [Material source](../crates/p4kx-model/src/mtl_source.rs) and [raw view](../crates/p4kx-model/src/mtl_raw.rs) | Source 16 MiB; text 4 MiB; default depth 32 and configuration ceiling 64; raw depth 8 and elements 4,096 | Default authored element count is complete (`None`); explicit finite admission remains. Raw summaries must not substitute for exact authored source. |
| [Model prefab source](../crates/p4kx-model/src/prefab.rs) | Groups 64; prefabs 65,536; objects 2^21 | Depth exhaustion currently contributes invalid descendants. Distinguish unreadable syntax from chosen expansion refusal. |
| [DDS topology](../crates/p4kx-texture/src/header.rs) | No fixed depth/array count fence | Exact 64-bit aggregate image counts and direct image-plan tail access replace the device-derived 2,048 policy. Source byte extents, topology contradictions and target resource admission remain separate; see [ADR-0365](adr/0365-dds-source-topology-is-separate-from-device-admission.md). |
| [Shader](../crates/p4kx-shader/src/limits.rs) | Source 1 MiB; token/value 64 KiB; tokens 65,536; declarations 16,384; properties 4,096; depth 64 with hard ceiling 128 | Chosen lexical and storage policies. Exact shader source must remain distinct from successful typed interpretation. |
| [GFX text](../crates/p4kx-gfx/src/text_html.rs) and [parameters](../crates/p4kx-gfx/src/text_loc.rs) | Characters 4,096; styles 256; markup depth 32; localisation parameters 16 | Display projection must not replace complete source text. Report omitted display work and preserve its source. |
| [Mission Flow](../crates/p4kx-export/src/template_flow_plan.rs) | No consumer condition-depth quota | Template and Broker use the same iterative occurrence plan and flat owner arena. Required unknown meanings remain failures with retained original raw facts; explicit source-first deep cases and whole default-JSON/writer checks are separate acceptance evidence. |
| [Gameplay transformations](../crates/p4kx-export/src/gameplay_transformation.rs) | Iterative traversal of materialized Sequences. | The reader reuses the current field path and preserves original slot order. The valid StrongPointer graph has a source-derived u32 local index bound. A 257-level original fixture passes plain JSON, schema and compressed roundtrips. |
| [Crafting tiers](../crates/p4kx-export/src/crafting_tier.rs) | Selection depth 32 | A projection failure differs from absence; complete source traversal remains required. |
| [Reward sources](../crates/p4kx-export/src/crafting_sources.rs) | No fixed owned-struct traversal depth | Iterative borrowed frames retain the full owned walk. Two source-first 130/257-node cases prove the former silent tail loss and pass with the complete traversal; scope, order, duplicates and reference selection stay separate from source interpretation. |
| [Mission titles](../crates/p4kx-export/src/mission_title.rs) | Complete expansion of reachable acyclic replacements | The selected language graph is planned once per render. Acyclic length is checked before output allocation. An active-key cycle leaves its bracketed token. Capacity and cancellation errors stop publication. Lookup counts reflect actual selected language and key reads. |
| [Quality membership and Commodity fallback](../crates/p4kx-export/src/quality_resource_links.rs) | Complete admitted source relationships and source-use sets | The inverse retains its complete ordered input inventory. A Commodity receives the exact relationship evidence consulted for its own resource. Source-use allocation failure stops publication. |
| [MCP admission](../crates/p4kx-mcp/src/budget.rs) and [parameters](../crates/p4kx-mcp/src/params.rs) | Response 256 KiB; paths 1,024 B; names 256 B; type 64 B; Archive path 4,096 B | Explicit transport/input policies, not MCP format maxima. Returned exact identities must roundtrip; oversized items require a usable continuation or precise refusal. |
| [MCP Loadout](../crates/p4kx-mcp/src/loadout_shape.rs) and [Audio event](../crates/p4kx-mcp/src/audio_event.rs) | Types/subtypes 16; issues 64; event banks 10; resolution breaks 25; index issues 20 | Labelled summaries and full retrieval are separate contracts. Counts alone do not prove a tail is retrievable. |
| [MCP image](../crates/p4kx-mcp/src/image.rs), [Loadout](../crates/p4kx-mcp/src/loadout.rs), [audio preview](../crates/p4kx-mcp/src/audio_excerpt.rs) | Requested depth at most 32; image side 2,048; audio 10 s, 1 MiB output and 64 MiB decoded | Preview policies can remain when explicit, source-independent and reversible by a separate full-data operation. |
| [Media](../crates/p4kx-media/src/limits.rs) | Record 1 MiB; chunk 64 MiB; value depth 128 with hard ceiling 512; nesting 8; candidates 128; configured totals | Payload and provenance completeness need separate accounting. A limit/contract error must not become an unqualified unavailable value or a successful prefix. |
| [Render](../crates/p4kx-render/src/assembly/mod.rs), [GPU](../crates/p4kx-render/src/gpu/limits.rs), [skeleton](../crates/p4kx-render/src/skeleton.rs) | 4 M vertices; 8 M triangles; items 2^19; textures 256 MiB; image sides 4,096/8,192; bones 4,096 | Target projection and device work differ from original facts. Query device capabilities, expose quality/refusal, retain exact source independently. |
| [glTF images](../crates/p4kx-gltf/src/texture_plan.rs), [bake layers](../crates/p4kx-gltf/src/layer_plan.rs), [Blender collection](../crates/p4kx-blender/src/collect.rs) | Images 256; pixels 2^29; baked layers 8; collected layer depth 8 | No universal glTF/Blender maximum is established. Preserve complete material source and identify skipped/approximated layers separately from bake cost. |
| [Screens](../crates/p4kx-screens/src/screens.rs), [assets](../crates/p4kx-screens/src/assets.rs), [clip state](../crates/p4kx-screens/src/paint_state.rs), [variables](../crates/p4kx-screens/src/variables.rs) | Scan/walk 64; canvases 4,096; shapes 2^17; variables 64, names 128 B, values 256 B; no additional logical clip-count fence | Complete canvas selection, source facts and draw work differ. All required logical clips are retained; actual raster mask bytes/work and SVG admission remain separate. See [ADR-0367](adr/0367-logical-screen-clips-are-independent-of-raster-admission.md). |
| [Sound workers](../crates/p4kx-extraction/src/sound_workers.rs) and [leases](../crates/p4kx-extraction/src/budget.rs) | At most 8 workers; leased memory | Scheduling policies do not inherently truncate source lists. Tune throughput/residency without treating these as source format maxima. |

## Canonical depth and work: demonstrated failures

The original-source controls in [owned nesting](../crates/p4kx-dataforge/tests/it/owned_nesting_complete.rs),
[owned output](../crates/p4kx-dataforge/tests/it/owned_nesting_output.rs) and
[owned work](../crates/p4kx-dataforge/tests/it/owned_work_complete.rs) establish valid declarations,
instances, array counts and pointer targets before asking for a complete result. The before-product
execution established five genuine failures in retained nesting, statistics, JSON/XML,
copied-prefix work and the full scalar pool. These controls exercise valid original sources.
The iterative product must pass them together with ownership, traits and consumer checks;
source-only proposals do not supply that execution evidence.

The 512-branch, depth-64 fixture contains 66,050 semantic values exhausted the 1,048,576 work
counter because copied position prefixes are charged repeatedly. The Int8 fixture contains
1,048,577 valid original zero bytes and requires every slot to survive. The nesting tests include
65 and 257 levels; a failure at 65 does not establish successful execution of the later case.

For a chain of depth D, repeatedly storing each complete prefix costs the sum of 1 through D,
which is quadratic. Raising 64 to 128 can therefore approach four times that position-copy work,
rather than twice. This is an algorithmic cost argument, not an RSS or timing measurement. An
iterative builder alone is insufficient: Clone, Drop, Debug, equality, weak locations, writers,
statistics and downstream consumers must remain usable for the admitted structure.

## Representable layouts and materialization

The inherited-field policy of 65,536 is not an encoded format maximum. Two acyclic definitions
with 32,769 own UInt8 fields each describe 65,538 valid ordered fields. The complete reader and
Query controls retain the original tail; a separate padded source reports its actual declared-size
mismatch. The shared layout index stores each physical declaration once and flat ancestor
summaries; active cursors preserve repeated inherited occurrences without copying each prefix.
See [source-layout storage](dataforge-source-layout.md).

The private original-position chain and its actual Reference consumers use u64. A compact source
with 65,539 Struct rows and 65,535 shared zero-size Class declarations proves an inherited count
above u32::MAX using metadata and two cursor observations. It is never fully materialized in the
controls. These are valid synthetic representations, not claims about an installed Build.

Source bytes alone do not bound retained Values: an eight-byte Class-array descriptor can name
u32::MAX zero-size Class instances. The controls inspect its original schema and descriptor and
exercise only an explicitly tiny materialization allowance. They never request its full tree.

Canonical materialization is complete by default without a fixed quantity quota. A caller can
choose a per-read Value allowance through TreeBuilder::build_with_budget. Each actual field or
Array-element Value occupies one slot; the implicit Record root, metadata and scratch do not.
Admission occurs before the corresponding output reservation. Exhaustion returns an explicit
MaterializationBudgetExceeded error, never a complete prefix. Checked platform capacity, output
allocation refusal and source-layout metadata allocation refusal remain separate errors. This
allowance measures Value occurrences, not memory bytes, runtime or whole-process residency.
[ADR-0368](adr/0368-dataforge-materialization-budgets.md) defines the contract.

The [inherited Record controls](../crates/p4kx-dataforge/tests/it/layout_field_count.rs),
[Query controls](../crates/p4kx-dataforge/tests/it/layout_field_query.rs),
[wide consumers](../crates/p4kx-dataforge/tests/it/layout_field_consumers.rs) and
[budget controls](../crates/p4kx-dataforge/tests/it/materialization_budget.rs) pass with this
implementation. They cover complete 65,538- and 70,000-field outputs, exact first-error behavior,
Pointer/Probe/Statistics/Completion tails, recovery after refusal and the existing observer
priority. Other separately listed policies remain open.

## Removed cuts and remaining evidence

The Tag tree retains readable names and selected relationships across the former depth, name,
path and cumulative text boundaries. It stores local names and actual parent indices. The SDK
and nested CLI writer use iterative traversal. Catalog path rows expand each path and its
contributions during serialization. The fully expanded wire and many retained selected
contextual results can still grow with the total length of their paths.

Current integration sources already remove fixed source prefixes for Query selection, Search
lines, eligible pickup places, ItemStats owned traversal, selected MCP exact identities/text and
complete lists, typed Model texture/layer counts, export-owned Loot nesting, inheritance ancestry,
ATL actions, stingers, CDF attachments, requirement traversal and the default authored Material
element count. Indexed audio references can now cross every required bank, while preserving the
existing selection and first-failure contracts. These changes do not remove the other policies in
the table.

Installed Build prevalence cannot prove a future maximum. No new whole-process memory or runtime
benchmark is claimed by this inventory. Regression evidence, precise source completeness and
actual target capability checks remain necessary for each change.
