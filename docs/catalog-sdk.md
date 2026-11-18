# Selective Catalog v3 data

Use `p4kx_export::catalog_queries::read_blueprint_category_lists` to read canonical Blueprint
category lists directly from an opened Archive. Supply `CatalogRecordSelection::All` to
read the domain, `Guid` to read all matching physical declarations, or `RecordIndex` to read a
known physical Record in the current DataForge. The physical index is local to the selected
input. The `_from` form reuses an already decoded `DataForgeEntry` from the same Archive.

The query opens the complete DataForge and projects the selected category-list bodies.
Original category
links carry their target header, resolution state and source uses. Their order, duplicates, null
slots and dangling links match `declarations.blueprintCategoryLists` in the complete Catalog.
The snapshot carries family availability, unreadable Record positions and a source inventory for
the returned cells. The query opens no localization or placement payloads.

Use `read_blueprint_categories` or `read_blueprint_categories_from` to read the physical
Blueprint Category definitions in `definitions.blueprintCategories`. The result has the same
eight directly sourced header fields and category selection policy as the full Catalog.
Both Blueprint readers share the DataForge-only read path.

Use `read_gameplay_properties` or `read_gameplay_properties_from` to query
`definitions.gameplayProperties` with the same Record selection. The query reads the authored
name, unit format, transformation and ordered name overrides with their source evidence. It
neither evaluates transformations and conditions nor runs the crafting or full Catalog build.

Choose `CatalogLanguageSelection::Selected` for specific localization payloads, or
`AllAvailable` for every supported language present in the Archive. An empty selection
returns public-name keys without translations. The snapshot returns referenced public-name
definitions with decoded and display forms. The requested languages describe the session policy.
The available languages list the localization payloads read successfully for this query. A query
with no admitted gameplay-property Record opens no localization payloads and returns an empty
available list. Compare physical Record indices only within the same DataForge input and resolve
source uses through each result's own inventory.

## Resolve field origins

Use each query's own `SourceInventory`. `SourcedValue::origin` supplies the encryption filter,
completeness state and contributor IDs. Match those IDs to `iter_uses()`, then join each use's
`source()` to `iter_sources()` by `PhysicalSource::id`. Follow `ancestors()` within the same
inventory. `kind()` exposes the immutable original Entry, limited proof, directory, whole-Archive
file, standalone file or unknown variant. `stored_encryption()` describes the stored source;
`SourceUseDescription::effective_encryption()` describes the actual use's byte domain.

The original Entry/declaration views retain typed states, complete integer widths and available
raw bytes. Byte ranges and declaration value offsets are relative to the immediately owning
Archive, including nested containers. A limited proof retains `SourceDetails::CaptureLimited`;
it supplies no invented full path or declaration fields. An unavailable or unknown source never
becomes unencrypted merely because a detail was omitted. These views expose the retained facts
without serialization or changes to Catalog v3.

## Reuse an Archive session

Open `CatalogQuerySession` to decode the original DataForge once for several queries. Use
`CatalogQuerySession::from_entry` when the caller already has that Archive's decoded Entry.
A session retains the opened Build and exact DataForge origin. Each request checks that the
Archive's size, modification time and Build still match. Once a change is observed, reopen the
Archive and create another session. The caller owns completed snapshots across later failures.

```rust
use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::{Archive, Language};

fn query(archive: &Archive) -> Result<(), CatalogError> {
    let session = CatalogQuerySession::open(
        archive,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let categories = parsed.read_blueprint_categories(CatalogRecordSelection::All, None)?;
    let category_lists = parsed.read_blueprint_category_lists(CatalogRecordSelection::All, None)?;
    let properties = parsed.read_gameplay_properties(CatalogRecordSelection::RecordIndex(0), None)?;
    let _ = (categories, category_lists, properties);
    Ok(())
}
```

The language policy selects possible localization payloads for the session. Category and category
list requests open none. Gameplay property requests use the configured languages. Mining
requests use their requested language when it is in the session policy, and still return
untranslated keys when it is excluded. Each query reads its own source and dependency
scope and provides the source inventory for the returned values. The session retains the decoded
DataForge bytes. Session methods open a parsed view for one request. For consecutive requests,
call `parsed` once and use that borrowed view to retain the DataForge's layout and lookup indexes.
Other input payloads use the Archive's configured key when the query reads them.

Use `read_harvestable_provider_presets` or its session method to read
`harvestableProviderPresets` by GUID, physical Record index or dataset. The query follows the
provider's actual preset, entity and mining component selectors. It reads no localization,
Starmap or placement payloads. The mining locations query also returns these canonical provider
rows and adds logical and physical placements.

## Mining resource names

The mining-location query includes the `resource_types` reached from its mineable elements.
Join by the physical `targetRecordIndex` in the element's `resourceType` reference and
the ResourceType row's source `recordIndex`. The same snapshot includes the physical target
of each admitted ore's `refinedVersion` reference. Resolve the short-name IDs in `public_names`
and use the snapshot's own `source_inventory` for the selected language and field evidence.

A missing name can retain its original localization key. Consult `coverage` for unresolved
resource references and unreadable ResourceType Records when converting the snapshot to a cache.

## Container traversal allowances

`CatalogOptions::starmap_budget` and `MiningLocationOptions::starmap_budget` use
`StarmapTraversalBudget`. The default reads all selected Child declarations and placement
occurrences without a fixed count quota. Cancellation remains available during both walks.

`source_children` optionally counts declarations once per physical descriptor, shared across
roots. `placed_children_per_system` independently counts Child visits per system, including
repeated instances of the same source. For example, two references to a one-Child tile consume
three physical declarations and four instantiated visits. `Some(0)` is an explicit zero allowance;
`None` imposes no count quota. The builder methods set each allowance independently.

An exhausted allowance never certifies a complete snapshot. Catalog normal publication refuses
before the output callback and preserves the last complete generation; explicit diagnostic mode
retains readable rows with incomplete health. The local mining snapshot reports the affected
placement inventory or unavailable occurrences in its coverage. The allowances do not change
cycle detection, original source evidence, coordinate validation or cancellation.

## Mining Starmap ancestry

Use `MiningStarmapObject::parent` to follow the original DataForge parent declaration.
Its reference preserves the original target GUID and resolution state. Use
`placed_under_guid` to follow the nearest enclosing Starmap object in the selected
physical container route. Both relationships carry their own source evidence. They can
refer to different objects within the same Build.

The mining reader supplies mining-specific placements and coverage. Read
`placements.unresolvedContainerLinks` from the Catalog snapshot for original unresolved
external Child occurrences and their Archive lookup states.

## Direct quality definitions

Use `catalog_queries::quality::with_distributions` and `with_quantizations`, their `_from`
variants, or a parsed session's `with_quality_distributions` and `with_quality_quantizations`.
The endpoints supply `definitions.qualityDistributions` and `definitions.qualityQuantizations`
through the same immutable canonical types used by the Catalog. Distribution selection includes
both distribution Records and location-override Records. All, GUID and RecordIndex selection
retain the canonical source policy, every eligible physical alias, original nested positions and
GUID-text/physical-index row order.

Inspect `rows()`, `coverage()` and `source_inventory()` inside the callback. References retain
their original GUID, physical target header and resolution state without opening target bodies.
Numeric getters preserve integer widths, canonical Int64 digits, exact IEEE bits, nulls and
unsupported/unreadable states. Use `raw_bits()` to recover an exact authored Single before
promotion to binary64; `finite()` is a convenient typed value, while its JSON form follows the
ordinary shortest Single decimal representation. It is not an independent precision contract.

These unlocalized endpoints open selected owner bodies only. They compute no quantization
eligibility or inverse ResourceType relation and open no sibling quality, Resource, locale,
world or placement payload to obtain such a relation. A successful query is not a completeness
certificate: inspect header, body and field coverage separately before publishing consumer-owned
cache data after outer success. The independent `projections.qualityQuantizationResources`
SDK endpoint remains outstanding. [ADR-0406](adr/0406-direct-quality-queries-preserve-canonical-definitions-without-inverse-discovery.md)

## Mining globals and refining processes

`catalog_queries::mining` exposes `with_mining_global_params` and `with_refining_processes`,
with `_from` functions and the corresponding session/parsed methods. They return the canonical
`configurations.miningGlobal` and `definitions.refiningProcesses` rows with complete immutable
original arenas, exact numeric storage, text and physical source inventories. All/GUID/RecordIndex
preserve physical aliases. Only selected owner bodies and demanded requested refining locales are
read; ResourceType/group/world discovery and a complete Catalog are unnecessary. Examine sourced
states and owner/dependency coverage and publish caches only after outer success. See the
[complete contract](catalog-mining-refining.md).

## Authored item ports

Use `with_item_ports` or `with_item_ports_from` to read `configurations.itemPorts`
for the dataset, a GUID or a physical Record index. A parsed session can call
`with_item_ports` through the same DataForge view as the other domain queries.
The callback borrows the canonical rows and original value arenas. Return
consumer-owned values from the callback and publish them after the query succeeds.

The snapshot carries Record, component and port positions, original value states,
scope coverage, public labels in the selected languages, and the field origin
inventory. The query reads the selected components and requested localization.
The archive-backed loadout resolver supplies equipped-item and XML-override data.

`ItemPort::alignment_axis()` borrows the original axis label.
`alignment_axis_option_status()` borrows its optional `SourcedValue<AttachmentAxisOptionStatus>`.
`Declared` and `Undeclared` describe membership in the original axis enumeration; an undeclared
label remains unchanged. An unavailable label has no membership cell, so inspect the label's own
state to distinguish absent and unreadable input. A retained membership cell preserves its own
value state, encryption marker, completeness and original source uses, including unavailable
option evidence. This status describes the declaration and does not infer an alignment transform.

## Mission location templates

Use `with_mission_location_templates`, its `_from` variant or the parsed session method
to read `missionLocationTemplates` for the dataset, a GUID or a physical Record index.
The canonical rows retain authored flags, tag terms and name variants. The query
resolves each original operand against the captured DataForge Tag graph. Use
`MissionLocationTag::path` to read the path and its occurrence-local source evidence.
The snapshot reports Tag graph issues. Its source inventory resolves the operand and the
actual ancestry of each path.
It reads no ContractGenerator, Broker, world placement or mission-query payloads.
The tags describe authored templates. Their use in a search belongs to the consumer.

Borrow `classification()` and `configuration()` to access the existing dictionary references,
module inputs, original module properties, spawn settings, inventories and mission limits.
The immutable views preserve every existing nested field, original slot order, null slots,
repeated occurrences, target header and lookup state. `MissionModuleInputPayload` distinguishes
the existing input interpretations; additional source fields remain available through each
owner's `SourceArenaOutput`. `dangling_guid_source()` exposes the complete optional GUID cell.
Resolve every sourced cell through this snapshot's source inventory, including unknown encryption
states. `source()` supplies the independently sourced physical Record header. `disabled()` and
`is_security_network_host()` retain original Bool states. `entity_cluster_member()` supplies the
original sourced Record reference and its target-header state. `names()` preserves the sourced
array and each original slot. Absent, null and failed interpretations remain distinct.

A selection without an admitted template opens no locale payloads and builds no Tag graph.
The snapshot carries protected name rows for the requested locales and the source
inventory for the returned sourced cells. Read the callback's data into consumer-owned
values and publish them after the query succeeds.

## Reputation

Use `with_reputation`, `with_reputation_from` or the parsed session's `with_reputation` to
read `definitions.factionReputations`, `definitions.reputationScopes` and
`definitions.reputationStandings`. Each requested family supports `All`, `Guid` and
`RecordIndex`. A requested family can return an empty result with its own availability and
coverage. The faction reputation Records carry a separate identity from the game's Faction
definitions.

For a rank index, `ReputationSelection::ranks()` supplies scopes and the complete standing
inventory. A scope's `standings()` retains authored slot order and source states. Use
`snapshot.standing(link)` to join a compatible in-scope link by its original Record index and
GUID. For a separate lookup on the same session, use the compatible link's
`resolved_index()` as a `RecordIndex` selection on standings. The minimum reputation
retains its original numeric storage.

Resolve `display_name()` through `snapshot.game_text()`. The text retains its original key,
available translations and source evidence. An application can independently apply its rank
display and key-conflict rules. Return consumer-owned results from the callback and publish
them after the query succeeds. A selection that admits no Record opens no localization payloads.

The three reputation row types expose their complete original values. Use
`FactionReputationDefinition::presentation`, `relations`, `properties_bb` and `authored_fields`
to inspect the original presentation, typed references, dynamic Blackboard values and retained
additional occurrences. `FactionText` preserves a protected locale reference separately from a
literal String. Unknown Blackboard variants remain `BlackboardValue::Authored` with their
complete original fields in the body's arena.

`ScopeDefinition::standing_map` exposes the map's source state, original ceiling, initial value,
ordered standing links and retained extras. `StandingDefinition::presentation`, `progression`
and `additional_fields` expose the complete original standing. `drift_time_hours` remains the
original Single in hours; no conversion to days or ordinary floating-point normalization is
performed. Exact integer widths, IEEE bits, source states and result-local origin references
remain available through the borrowed values. These accessors add no new query families or
Catalog fields.

## Manufacturers

Use `with_manufacturers`, `with_manufacturers_from` or the parsed session's
`with_manufacturers` to read `definitions.manufacturers`. Supply `All` for canonical GUID
order, `Guid` to retain every physical declaration for that GUID, or `RecordIndex` to follow
a resolved manufacturer reference. Inspect the supplied row in the callback and return
consumer-owned results.

Use `snapshot.public_name` and `snapshot.game_text` for the typed references of a selected
manufacturer. The returned text entries preserve the original key, source and display forms
in the requested languages, and per-value origin. Resolve source-use IDs in that snapshot's
inventory. A lookup with no admitted manufacturer opens no localization payloads.

## Contract Generator sources

Use `with_contract_generators`, its `_from` form or the parsed session method to read
`definitions.contractGenerators` for the dataset, a Generator GUID or a physical Record
index. This selection addresses the owning Record. Contract and sub-contract IDs belong
to the derived contract rows.

The result retains every selected physical declaration in original Record order, including
development source owners and equal GUIDs. The definition has the same complete source arena
and completion-tag, physical-item reward and mission-item property interpretations as the
full Catalog. The query uses the shared canonical projector. A field interpretation can fail
while its original source remains in the returned arena. Check `coverage`, `projection_notes`
and `projection_failures` before publishing a consumer cache.

Read the arena's `roots()` and `values()`, then follow each field's `value()` through
`SourceArenaOutput::value`. A `SourceNode` provides its original `SourcedValue` and any typed
original error. Struct fields and array elements preserve order, repeated occurrences,
null values and unresolved links. Declaration bindings can resolve their original field with
`SourceFieldRef::resolve`. Each link is local to its originating arena. Resolve source-use
references through the snapshot's own `source_inventory`.

Raw text values preserve their original public or protected domain and localization key.
The selected DataForge is the only payload input. `requested_languages` retains the session's language
policy. Reference payloads preserve their target header and resolution state without opening
the target Record's body.

This endpoint supplies the source-definition dataset. Use `with_contracts` for canonical contracts
by physical Generator owner, or `with_contract` with `ContractKey` for one parent/sub-contract
identity. Both return the selected titles, property sets, location queries and spawn pools with
their actual source and dependency closure. Forward inputs are prepared only when the shared
canonical interpreter needs their typed lookup. Check coverage before replacing a consumer cache.

See [Contract queries](catalog-contract-queries.md) for callback lifetimes, parsed-session reuse,
global duplicate and intro semantics, complete original views and source-inventory ownership.

## Loot container-document source scope

Catalog v3 `declarations.lootEntityContainerDocuments` retains original attribute sites only from
EntXML documents actually reached by the existing Loot container walk. It is not a whole-Archive
Entity inventory. The source strategy is the physical containing Entry plus nested Entry and local
node/attribute ordinals; repeated placement roots and equal GUIDs do not replace that identity.
The normalized nodes/configuration references and exact lookup candidates are documented in
[the Loot source contract](catalog-loot-source.md#original-container-link-documents-reached-by-the-loot-walk).
This additive table has no direct-query SDK adapter yet. A future adapter must reuse the same
source-owned reader, reached scope, original content/directory surveys and failure handling. The
existing global first-pair convenience derivation remains a separate limitation.

## Mineable and full resource definitions

Use `with_mineable_elements`, `with_mineable_compositions` and `with_resource_types` for
All/GUID/physical Record queries without world placements or complete Catalog construction.
These return the complete canonical source rows and immutable nested views, with text and origin
resolution. ResourceType group membership demands a separate complete group-family survey.
Check owner and dependency coverage before publishing a consumer cache. See
[selective mining and resource queries](catalog-mining-resource-queries.md).

## Physical Blueprint definitions, tiers and presentation

`with_blueprints` and its `_from`/session forms return complete selected physical Blueprint
source arenas, tier configurations and canonical presentation. All/GUID/RecordIndex selection
preserves physical duplicates and original quality-slot positions. Rules load only the needed
global/category/dismantle inputs; source, text and coverage remain available without `read_crafting`
or a complete Catalog. See [Blueprint queries](catalog-blueprint-queries.md).



## Mission Location source cells

Mission Location rows expose the same [direct source cells](catalog-location-configuration.md#source-identity-flags-cluster-link-and-names)
as the complete Catalog. source() returns the canonical eight-field Header. disabled() and
is_security_network_host() return sourced Bool cells. entity_cluster_member() returns the original
sourced Record link, retaining its authored GUID even when resolution is dangling or incompatible.
names() returns a sourced collection whose individual slots retain their original states and
positions; present members expose the existing Tag operand and protected display-name reference.

Inspect state() and value() independently of origin().contains_encrypted_source(). Resolve
actual source uses only in the snapshot's SourceInventory. An incomplete first-owner selection
remains visible on each dependent cell, independently of complete intrinsic Header metadata.
These values share the same original reader, identities, ordering and text domains in direct
queries and full Catalog production. The old nullable entity_cluster_member_guid() projection is
replaced by the typed link; no legacy wire mirror is emitted.


## Direct resource groups and explicit parent projection

`with_resource_groups` and its `_from`/session forms return selected physical definitions and
all original group/resource membership slots. `with_resource_group_parents` queries the existing
first-parent projection separately through the complete canonical group-family survey, without
Resource/World bodies or locale payloads. Both retain physical GUID aliases, direct source cells,
result-scoped provenance and explicit coverage. See [Resource group queries](catalog-resource-group-queries.md).

## Original equipment definitions and configurations

Use `catalog_queries::equipment::with_item_definitions`, `with_attachment_constraints` or
`with_item_bounds` for the corresponding canonical datasets. Each has a decoded-Entry `_from`
form and the same-named session/parsed-view method. Select the dataset, all physical declarations
with a GUID, or a physical Record index. Physical indices are local to the supplied DataForge.

`definitions.items` contains original Item presentation, physics and purchase fields. Its nested
immutable getters preserve the original component order, numbers, null/unavailable states,
manufacturer reference or authored value arena and optional personal-weapon motion fields.
Attachment getters also expose original type/subtype/size and unsplit tags/required tags for
foreign authored AttachDef structures; constraint-family admission stays separate. Use
`CatalogLanguageSelection` for referenced public names and protected descriptions. An empty
language selection retains keys without reading locale bodies. These definitions are scoped
canonical facts rather than every value of an arbitrary Entity Record.

`configurations.itemAttachmentConstraints` exposes original type/subtype/size restrictions,
tags, required tags and alignment-axis declarations at each physical component. Both axis labels
retain their optional `AttachmentAxisOptionStatus` cells independently. Bounds expose original
occupancy, vehicle bounding size and object metadata branches, including exact coordinate/unit
values and retained unknown-unit original fields. The configuration queries accept no language
argument; their requested/available locale lists and text registries are empty even when the
session's unused language policy contains languages.

The borrowed `EquipmentSnapshot` supplies rows, family availability, source inventory, coverage,
Build and the request's text/language metadata. Definitions use GUID/physical order;
configurations use physical Record/component order. Join typed field contributions only through
that result's own inventory and follow its physical-source ancestors. Raw numbers and authored
value arenas keep their own typed states and origins; they are not row-level proof substitutes.

These readers invoke the same original family converters as the producer. They do not build a
Catalog, resolve loadouts, read vehicle XML, discover world placements or project other equipment
families. A selected-Record request does not materialize unrelated owner bodies. Dataset requests
still enumerate their canonical candidate scope and retain the requested rows; they do not imply
a fixed-memory bound. Respect incomplete header/body/field coverage and publish owned cache
updates only after outer query success. Full-Catalog normal-publication guards are unchanged.

The Item producer shares these original facts with its sourced header, first physical component
selections, English class and key-only playability. Semantic English admission is independent
of output-language requests; the original-definition query above still needs no English locale
when its requested language list is empty because it does not compute that classification.
A full root-Item SDK query, authenticated unfiltered variant choice, Ship and Loadout projection
coverage remain separate work under
[ADR-0410](adr/0410-equipment-sdk-shares-original-family-readers-and-canonical-variant-choices.md).
No legacy root-Item DTO is offered as a complete canonical substitute.

An unclassifiable original component slot or unreadable possible occupancy definition keeps the
queried source scope incomplete. Readable prefix rows remain available; `coverage.incomplete_fields`
identifies the affected physical Record. An explicit null target and a proven foreign declaration
remain separate known exclusions. No unrelated configuration projector runs to manufacture that
coverage result.

## Verify selected queries against an installed Archive

The ignored `installed_selective_sdk_has_original_owners_and_closed_sources` test opens one
session and reuses its parsed DataForge. It selects one physical owner per available family and
checks mining globals, refining processes, quality distributions and quantizations, ResourceGroup
definitions and membership, and the separate parent projection. The parent projection surveys the
canonical group family. The test checks original Record identity, per-value source references,
dependency coverage, and cancellation before and after a callback.

Set `P4KX_DATA_P4K` to the installed Archive and configure the Archive key through the normal
test configuration. Set `P4KX_SDK_WITNESS_FILE` to a new absolute JSON path inside the configured
temporary directory. The test writes only aggregate counts and a Build fingerprint to that file
after all checks pass.

```powershell
cargo test --locked -p p4kx-export --lib -- `
    catalog_query_session::installed_sdk_witness::installed_selective_sdk_has_original_owners_and_closed_sources `
    --ignored --exact --test-threads=1
```

## Original crafting-global configuration

`catalog_queries::crafting::with_global_params` and `with_global_params_from` return borrowed
`CraftingGlobalSnapshot` values from an Archive or a decoded Entry. Session and parsed-view
methods are named `with_crafting_global_params`. Each uses the same original reader as
`configurations.craftingGlobal`; no complete Catalog, Blueprint, resource body or localization
payload is prepared. A session's unrelated language policy is ignored.

`All`, `Guid` and `RecordIndex` select original physical owners. GUID selection retains every
physical alias. Rows keep canonical textual-GUID/physical-index ordering and expose their complete
typed header, original default-selection concrete type, default Blueprint slots, resource and item
exclusion slots, additional fields, source inventory, Build, availability and coverage. Arrays keep
nulls, repeats and unresolved/foreign target states; they are not deduplicated sets. Consumers can
union applicable readable GUIDs across owners when implementing the existing interpreted rules.

The Catalog has no merged root `craftingGlobal` compatibility view. The old
`dismantleExcludedResourceGuids` list corresponds to the applicable readable targets in original
`dismantleBlacklistResources`; `dismantleExcludedItemGuids` corresponds to
`dismantleBlacklistEntityClasses`. The original `defaultBlueprintSelection.blueprintRecords`
slots are now available directly. Source states and physical occurrences remain authoritative.
See [ADR-0411](adr/0411-crafting-globals-retain-physical-original-configuration-rows.md).

## Physical Blueprint pools

Use `catalog_queries::crafting::with_pools` or `with_pools_from` with an Archive or a decoded Entry.
The session and parsed-view methods are `with_blueprint_pools`. The borrowed snapshot returns
the original pool rows, weight projections in the same order, the Build, availability, source
inventory and coverage. The query reads the selected pool family; it needs neither Blueprint
bodies nor localization.

`All`, `Guid` and `RecordIndex` select physical pool owners. A GUID can select multiple original
Records, in textual-GUID and then physical-index order. Join each weight using the input and
physical Record index when combining results from different selections. A slot retains its
authored Reference and exact numeric storage. The projection sums the positive finite decoded
weights in slot order using f32 arithmetic. An overflowing sum still carries its f32 bits.

The complete Catalog also publishes `declarations.blueprintRewardSources`. This direct query does
not survey the other Record families that reference the pool. See
[ADR-0412](adr/0412-blueprint-pools-separate-original-slots-weights-and-reward-sources.md).
