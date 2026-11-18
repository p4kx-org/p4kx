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
