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

## Location entity declarations

Use `with_location_entity_declarations` or `with_location_entity_declarations_from` for the
canonical `definitions.locationEntityDeclarations` rows. This API supports All, GUID, and physical
RecordIndex selection, with an optional ordinary Catalog path Filter. The same entry points are
available on a query session or its parsed view. The query reads the selected physical definitions
and requested localization payloads for delivery descriptions. An empty selection reads no locale.

The snapshot retains duplicate physical owners, selection coverage, referenced protected texts and
its own source inventory. `resolve` joins a compatible resolved Reference from the same DataForge
to a returned physical definition. Consumers distinguish a dangling source Reference from a
resolved target excluded by the active selection using the Reference's source status. A link does
not cause the target's payload to be read automatically.

The `hauling_target_bindings()` accessor on a ContractGenerator definition addresses the original
pickup and drop-off arrays through the same definition's source arena. It is available in both the
generator query and the logical contract query. Bindings point to individual owning orders at all
authored levels, including contract and sub-contract overrides.

## Lens flare libraries

Use `catalog_queries::flares::read_flare_libraries` to read the physical rows from
`definitions.flareLibraries` without decoding DataForge. Select all, an Archive Entry index,
or an exact Archive-relative path. The query reports original parameter origins, directory-only
texture links, and any required library reads or projections that failed. [Lens flare contract](catalog-flare-libraries.md)

## Original planet resource definitions

Use `catalog_queries::planet_resources::read_planet_resources` for the typed rows under
`definitions.planetResources`. `EntryIndex` takes a physical directory index in the opened
Archive. `ArchivePath` retains every occurrence of the same normalized path. `Guid` compares
the nonzero authored identity against each eligible resource document and retains duplicate
owners in physical directory order. `All` visits every eligible resource Entry.

The query keeps original GUID spelling, names, the mining flag, tint palette selectors and
both kinds of palette targets. Exact path and Entry selection leave other resource payloads
unread. GUID lookup reads candidate resource documents and resolves palette targets for the
matching rows. Only a selected DataForge palette path requires a DataForge payload. Use
`read_planet_resources_from` or the session and parsed-view methods to reuse an already
decoded Entry. Directory-only palette resolution leaves the target payload unread.

`PlanetResourceSnapshot` includes Build identity, physical Entry availability, original source
uses and unsuccessful read, projection or GUID-candidate counts. A healthy directory and a
complete selected scan allow the query to prove absence. Palette lookup can still report
unresolved or ambiguous references on readable rows. Identity and links describe the loose
definition, with no planet or biome assignment.

For the installed witnesses, set `P4KX_DATA_P4K`, the exact Build change number in
`P4KX_PLANET_RESOURCE_BUILD` and one original resource Entry path in
`P4KX_PLANET_RESOURCE_PATH`. Use `installed_planet_resource_sdk_matches_one_physical_definition`
when the path identifies one Entry. Use `installed_planet_resource_sdk_proves_exact_path_absent`
when the path is absent. Run either filter with
`cargo test -p p4kx-export --lib <test-filter> -- --ignored --nocapture`. The tests verify the
archived source or the complete directory lookup and report only the Build and definition count.

## Radar audio configuration

Use `catalog_queries::radar_audio::read_radar_audio` to read original rows from
`configurations.radarAudio` by physical Entry or across the selected configuration family.
The SDK retains ordered nodes, source values, and explicit selector states without decoding
DataForge. [Radar audio](catalog-radar-audio.md)

## Localization Entry metadata

Use catalog_queries::localization::with_language_sources for the canonical
texts.languageSources map. It reads and indexes the requested localization Entries directly
from the Archive and supplies each Entry's path, encoding, lossy-decoding flag, malformed-line
count and immediate source evidence. AllAvailable returns the same supported-code map as the
complete Catalog. Selected returns only the requested codes. The map distinguishes a requested
language absent from the Archive from a language outside the request.

The callback receives the Build, notes and the result's own SourceInventory. The query opens
no DataForge Entry and creates no Catalog output. A failed requested read returns an error.
Publish caller-owned state after the outer query returns successfully.

Verify the original localization Entry metadata on an installed Archive with
P4KX_DATA_P4K and the configured testkit key. Set P4KX_DATA_P4K_B for a second input.
The witness checks the original physical Entries and their independently read metadata.

~~~powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_language_sources_sdk::language_source_sdk_matches_original_archive_entries -- --ignored --exact --nocapture --test-threads=1
~~~

## Offline shop inventory definitions

Use `catalog_queries::shops::read_shop_inventories` to read original offline inventory
definitions. `EntryIndex` reads one physical Archive directory Entry. `ArchivePath` matches
every eligible Entry at a normalized Archive path. `All` includes every eligible inventory
file. Use the physical index from the opened Archive to read an individual file.

The snapshot uses the same rows as `definitions.shopInventories`. It keeps the original JSON
token order and numeric digits, typed navigation to inventory fields and per-value origins.
A verified empty file retains an absent document. An invalid or unreadable selected Entry
retains its own document state and scoped failure count. An indexed lookup cites the selected
Entry path and position or, for an out-of-range index, the admitted root directory count.
Path and full inventory searches cite the paths they consulted. The query opens neither the
shop registry nor DataForge.

Set `P4KX_DATA_P4K` and `P4KX_DATA_P4K_B` to the installed Archives to verify one
original inventory from each Build and an index outside each admitted directory.

```text
cargo test --locked -p p4kx-export --test real_archive real_archive_shop_inventory_v3::sdk::selective_shop_inventories_match_an_original_inventory_and_missing_index -- --ignored --exact --nocapture --test-threads=1
```

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

## Measure a local mining cache

The `mining_location_consumer` example exercises the public mining-location SDK using two
installed Archives with distinct Build manifests. Build it with
`cargo build --locked -p p4kx-export --example mining_location_consumer`. Run the built
executable as `mining_location_consumer <old Data.p4k> <new Data.p4k> [language]`.
The example uses the normal archive key lookup and defaults to English.

It keeps the old typed snapshot until a refresh succeeds. It checks that a pre-cancelled
refresh and an unavailable input preserve the last successful snapshot. A warm request
checks the same path, locale, Build and file stamp and reuses the snapshot. The cold phase
starts with an empty consumer cache. The OS file cache is left in its normal state.

The example prints one JSON line per phase with timing, Build or retained Build ID, and
aggregate row and source counts and coverage on successful requests. It writes no Catalog
or persistent cache files. A successful read includes its observed coverage. Inspect that
coverage when deciding which snapshots your application can use.

For a memory acceptance run, measure the built executable directly on the target host.
Record the exact SDK revision and executable digest, process and helper peaks, minimum
available system RAM, paging and temporary disk usage. A harness run measures the local
SDK and this cache policy. The integrated application also needs its normal desktop workflow
checked. [Local consumer RAM acceptance](catalog-partitioning.md#system-memory-acceptance)

The synthetic `consumer_refresh` integration cases show how one parsed view can serve several
queries in one refresh. They resolve a reward source to its physical pool and Blueprint and
read a mission location and Item ports. The joined mining case follows the provider,
composition, element, resource and placed location to its localized name. It also shows how a
consumer can retain its last usable cache when an authored composition member is dangling.

`cargo test --locked -p p4kx-export --test it -- consumer_refresh` runs these cases,
including locale invalidation, cancellable queries, failed cache replacement and the source
inventory joins.

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

The mining reader supplies mining-specific placements and coverage. Use
[the container placement query](#starmap-container-placements) for original unresolved Child
occurrences and their Archive lookup and target-body read states.

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

## Physical mineable entities

`with_mineable_entities` and `with_mineable_entities_from` accept `CatalogRecordSelection`.
The session and parsed methods are named `with_mineable_entities`. The returned rows match
`definitions.mineableEntities`. Selection by GUID keeps all physical aliases and the record
index identifies one occurrence. The query opens no localization payloads. When a selected
owner has a signature component, it reads the original global radar selector and reports any
unreadable dependency Record. An unresolved or ambiguous layout yields no scan value.
Selection applies the Catalog admission rules, including the development-leftover exclusion.
Original links retain the physical target even when that target is outside the admitted rows.

Set `P4KX_DATA_P4K` to an installed Build and run
`cargo test -p p4kx-export --test real_archive installed_mineable_entity_sdk_preserves_original_links_and_both_scan_sources -- --ignored --nocapture`
to verify original mining links, physical component positions, and both sources of the scan
signature without building a Catalog.

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
Use `equipment::with_item_sources`, `with_item_sources_from`, or the session and parsed-view
methods to read the Item header, first physical component ordinals, sourced `itemClass` and
`playability`. `RecordIndex` selects a physical owner and `Guid` retains each selected alias.
Join the component ordinals to `definitions.items` using the same DataForge input and Record
index. Consult English for classification. Look up presentation translations through the
definition query.

The Item source query reports coverage for its returned fields. An incomplete component
selection or unreadable input to classification or playability identifies the affected owner in
`coverage.incomplete_fields`. An unusable higher-priority purchase name preserves its source
state. The attachment name determines playability when the original decision reaches that rule.
The definition query reports coverage for the full Item definition. See
[ADR-0410](adr/0410-equipment-sdk-shares-original-family-readers-and-canonical-variant-choices.md).

An unclassifiable original component slot or unreadable possible occupancy definition keeps the
queried source scope incomplete. Readable prefix rows remain available; `coverage.incomplete_fields`
identifies the affected physical Record. An explicit null target and a proven foreign declaration
remain separate known exclusions. No unrelated configuration projector runs to manufacture that
coverage result.

## Direct flight configurations

Use `catalog_queries::equipment::flight` to query `configurations.flightControllers` with
`with_flight_controllers`, `configurations.ifcs` with `with_ifcs`, and `configurations.thrusters`
with `with_thrusters`. The free functions have decoded-Entry `_from` forms. The session and
parsed view provide the same methods for consecutive queries against one Archive.

The query selects original Entity owners by physical Record index, GUID or the complete
family. For each owner it keeps every admitted component at its original array position.
Follow the Record index and component ordinal in the same DataForge to link related rows.
Row getters expose original flight limits, IFCS sections and thruster configuration, including
source-backed numeric bits, authored fields and typed references. The result owns the source
inventory for those borrowed fields.

Use `coverage()` to check the selected owner scope before accepting a result. A missing
component slot, a malformed authored pointer, and an unreadable owner have distinct outcomes.
A different Record lies outside a physical lookup even if it has the same GUID. Family and
GUID queries inspect the admitted physical owners in order. These readers need no locales
or loadout resolution.

## Worn gear and item radar

Use `catalog_queries::equipment::worn_gear` for `configurations.suitArmor` with
`with_suit_armor`, `configurations.clothing` with `with_clothing`, and
`configurations.itemRadars` with `with_item_radars`. The free functions also accept an already
decoded Entry through their `_from` forms. The session and parsed view provide the same queries.

The rows keep each original Entity owner and its Components slot, including repeated components
and physical aliases of a GUID. Inline clothing and radar sections keep authored values,
presence and provenance. A suit's `damage_resistance()` gives the authored macro reference.
Pass its resolved physical Record index to `equipment::component_definitions::with_damage_resistance_macros`
to read that definition in the same session. The suit query resolves the reference identity without
reading the macro's body.

A single owner query projects the requested family for that physical owner. Check `coverage()`
before accepting the result. These queries do not read locale or model payloads.

## Direct mining equipment and consumable configurations

`catalog_queries::equipment::mining_tools` provides `with_mining_lasers` for
`configurations.miningLasers` and `with_attachable_modifiers` for
`configurations.attachableModifiers`. Use `equipment::consumables::with_consumables` for
`configurations.consumables`. All three have decoded-Entry `_from` forms and matching methods
on the Archive session and parsed view.

These queries retain physical owner and component indices, original ordered contents and
effect slots, and per-value source facts. The consumable capacity retains its authored unit.
Use the physical indices in the mining global and consumable subtype links with the queries
below. Subtype text selection belongs to the separate definition query. Component queries
expose read completeness through `coverage()`.

## Vehicle armor

Use `catalog_queries::equipment::vehicle_armor::with_vehicle_armor` for
`configurations.vehicleArmor`. The decoded-Entry `_from` form, session and parsed view select
physical owner Records or GUIDs. The query uses canonical Entity admission and keeps the original
component slot, signal values, concrete damage alternative, penetration and deflection fields, and
ordered health-curve points. A derived damage type with an unrecognized shape retains its authored
fields under `Unknown`.

Check `coverage()` before accepting the result. The query reads no locale or model payloads.
Vehicle armor placements in a loadout still join by owner Record and component identity.

## Direct resource components and connections

Use the queries in `catalog_queries::equipment::resource_network` to read these tables by physical
owner GUID or Record index.

| Catalog dataset | SDK query |
| --- | --- |
| `configurations.resourceComponents` | `with_resource_components` |
| `configurations.resourceConnections` | `with_resource_connections` |

Both queries support the decoded-Entry `_from` form, a session and a parsed session view. They keep
original component slots, per-value origin and source-use references. Resource components include
ordered states, deltas, control parameters and their physical null slots. Connections retain their
electrical or thermal field types. Inspect `coverage()` and resolve source-use references through
the snapshot's source inventory.

## Direct fuel and container configurations

Use these queries in `catalog_queries::equipment::fuel` to read original configurations by
physical owner GUID or Record index.

| Catalog dataset | SDK query |
| --- | --- |
| `configurations.fuelTanks` | `with_fuel_tanks` |
| `configurations.fuelIntakes` | `with_fuel_intakes` |
| `configurations.resourceContainers` | `with_resource_containers` |

The decoded-Entry `_from` form, Archive session and parsed view are available for each query.
Container capacity keeps its original unit and numeric representation. Default composition and
restriction arrays preserve physical order, null slots, unresolved original links and field
origins. Tank rows preserve their authored interaction and audio configuration. Inspect
`coverage()` and resolve source-use references through the snapshot's source inventory.

## Component definition Records

The functions in the following table are exported by
`catalog_queries::equipment::component_definitions`. Each also has a decoded-Entry `_from`
form and a same-named method on both the query session and its parsed view.

| Catalog v3 table under `definitions` | SDK method |
| --- | --- |
| `damageResistanceMacros` | `with_damage_resistance_macros` |
| `miningLaserGlobals` | `with_mining_laser_globals` |
| `turretHealthModifiers` | `with_turret_health_modifiers` |
| `consumableTypes` | `with_consumable_types` |
| `consumableSubtypes` | `with_consumable_subtypes` |

Select `All`, `Guid` or `RecordIndex`. Results follow physical DataForge Record order. A GUID
selection keeps every admitted physical alias. Use the original Record index in the same
DataForge to follow a resolved Reference. A physical read returns its own borrowed,
typed values, original field states and source inventory. A Reference retains its authored
target and resolution state. The caller queries the target's definition separately.

Check `coverage()` for selected-owner reads and field inspection. `definition_discovery()`
reports the required dependency survey. Direct Turret selections containing only known
base-definition Records, or containing no matching Records, need only their selected scope.
A selection involving a possible derived definition or all definitions surveys the physical
turret components that can admit additional compatible types. Unclassified owners and failed
reads remain visible in discovery coverage. A family can be available with no rows for the
current selection.

Consumable subtypes use the session's selected languages for public names and protected effect
descriptions. Resolve their typed text references within the snapshot. An empty language
selection retains keys. A read error in a required requested locale returns a query error.
The other four definition readers leave the language and text collections empty. Retain the
current consumer cache until the outer query succeeds and required coverage is complete.

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

For a direct Item check, also set `P4KX_CATALOG_ITEM_TEXT_RECORD` to an exact private Record
path with a readable English description. The Item test compares the 13 source fields with
the original Record and checks the English lookup, per-value provenance and physical decode
count. Use a fresh `P4KX_SDK_WITNESS_FILE` on each run.

```powershell
cargo test --locked -p p4kx-export --lib -- `
    catalog_query_session::installed_sdk_witness::item::installed_item_source_sdk_matches_one_original_record `
    --ignored --exact --test-threads=1
```

To verify the five component definition families, run the installed definition test. It chooses
the first suitable physical Record for each family in original order. The query session and
the independently read Records borrow the same decoded DataForge Entry. The test checks the
Archive Build, selected coverage, sampled original values and exact source positions. For a
specific sample, set `P4KX_DEFINITION_SDK_WITNESS` to a private selection file with the exact Build
identity and physical indices. The default run prints Build metadata and the aggregate count.

```powershell
cargo test --locked -p p4kx-export --test real_archive -- `
    real_archive_definition_sdk::installed_component_definition_sdk_preserves_selected_original_values `
    --ignored --exact --test-threads=1
```

To verify flight controllers, IFCS and thrusters, use the installed Flight test. It discovers
one physical owner for each family and compares the original component slots, representative
raw numeric values and exact source coordinates with the SDK. It checks the Archive Build
and uses one decoded DataForge Entry. The test prints Build metadata and the aggregate count.

```powershell
cargo test --locked -p p4kx-export --test real_archive -- `
    real_archive_flight_configuration::sdk::installed_flight_sdk_preserves_selected_original_components `
    --ignored --exact --test-threads=1
```

The installed worn-gear test compares one selected original suit, clothing and radar component
and one referenced damage-resistance macro to the SDK. It checks the physical component slots,
sampled numbers, reference identity and the original source coordinates. It reuses the decoded
DataForge Entry and prints the Build and aggregate result.

```powershell
cargo test --locked -p p4kx-export --test real_archive -- `
    real_archive_worn_gear::sdk::installed_worn_gear_sdk_matches_original_component_values_and_macro `
    --ignored --exact --test-threads=1
```

The installed equipment test compares selected mining laser, attachable modifier and consumable
components with their original fields and source coordinates. It checks linked mining-global and
consumable-subtype definitions separately when the original references resolve. It uses one decoded
DataForge Entry and prints aggregate counts and the Build.

```powershell
cargo test --locked -p p4kx-export --test real_archive -- `
    real_archive_mining_configurations::sdk::installed_mining_tools_and_consumables_preserve_original_values_and_links `
    --ignored --exact --test-threads=1
```

The installed vehicle armor test reads original physical component occurrences and compares
nested damage, penetration and deflection values and their source coordinates. It reuses one
decoded DataForge Entry and prints the Build and aggregate counts.

```powershell
cargo test --locked -p p4kx-export --test real_archive -- `
    real_archive_vehicle_armor::sdk::installed_vehicle_armor_sdk_matches_the_original_physical_tuning `
    --ignored --exact --test-threads=1
```

The installed resource-network test compares original resource-state order, power and heat fields,
and their physical source coordinates for selected catalog-eligible owners. It uses one decoded
DataForge Entry and prints the Build and aggregate counts.

```powershell
cargo test --locked -p p4kx-export --test real_archive -- `
    real_archive_resource_components::sdk::installed_resource_network_sdk_reconciles_original_components_and_connections `
    --ignored --exact --test-threads=1
```

The installed fuel test compares original tank multipliers, intake rates, container capacity and
linked source positions. It uses one decoded DataForge Entry and prints the Build and totals.

```powershell
cargo test --locked -p p4kx-export --test real_archive -- `
    fuel_capacity::sdk_witness::installed_fuel_sdk_preserves_original_values_link_slots_and_origins `
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

## Blueprint reward-source survey

Use `catalog_queries::crafting::with_reward_sources` or `with_reward_sources_from` to read
`declarations.blueprintRewardSources` directly. Session and parsed-view methods are named
`with_blueprint_reward_sources`. `BlueprintRewardSelection` distinguishes two costs and scopes:

- `SourceOwners(selection)` reads the selected original enclosing Records and classifies only the
  pool-GUID dependencies actually observed in their owned bodies. A source can belong to any
  Record family or path. GUID selection retains all physical source aliases.
- `TargetPools(selection)` selects physical pool targets and surveys every source Record when
  at least one target is admitted. An empty target selection skips that survey and reports zero
  source reads. Pool aliases needed for canonical nominal membership are checked separately.

Both modes share the ordinary reward traversal and original pool reader with the Catalog.
They keep scalar Record references, foreign root weak pointers, repeated physical occurrences,
local chance storage and nearest enclosing identity semantics. Raw Reference-array elements, local
weak pointers and nominal self-links retain the existing exclusions. Pool membership comes from
materialized admissible pool bodies, while target selection filters the actual physical
`targetRecordIndex`; the link is never rewritten to another same-GUID alias. A failed physical
target can still have an observed link if a readable alias establishes nominal membership. Its
pool coverage remains incomplete.

Rows expose every serialized field through typed getters and stay ordered by original source
Record index and occurrence ordinal. Filtering does not renumber `sourceOrdinal`. Resolve every
cell's contributors in the snapshot's own `source_inventory`. The query reads no localization,
Blueprint, global-configuration, resource or scene dependencies merely to provide these rows;
the inverse mode can read such a Record as a potential source because its survey is global.

Inspect both `source_coverage` and `pool_coverage`, together with `selection`,
`source_records_read` and `pool_records_read`, before interpreting an empty result. Unreadable
possible sources remain gaps even if their type has no readable reward owner. An opaque field
error remains incomplete on a readable zero-match body. The legacy crafting warning heuristic
is not the query's completeness policy. Availability describes the original pool family,
independently of selected row count. No complete Catalog or reverse Reference index is built.
Only the current source tree and the current demanded pool tree are materialized together.
The inventory retains the original uses for returned rows and inspected pool admissions.
Coverage retains failed physical indices. Publish consumer changes only after the outer
query succeeds.


## Ship source fields in the Catalog

The Catalog Ship root uses an original eight-field Header, exact sourced crewSize and size,
the unmodified authored vehicleDefinition selector, static insurance declarations/parameters
and an authored CrewManifest link. The shared DCB interpreter reads these values in a single
field walk and resolves the crew manifest to its Record header.

Use `equipment::with_ship_sources`, its `_from` form or the session and parsed-view
`with_ship_sources` method for the source-backed Ship fields. Choose `All`, `Guid` or
`RecordIndex`. GUID selection retains every admissible physical alias. Rows keep canonical
GUID order, then physical Record order for ties. The supplied `ShipSource` has immutable typed
getters for the complete original Header, name/description/role/career, crew size, both
manufacturer declarations, original vehicle selector, exact size, insurance and authored
crew-manifest inputs, plus canonical sourced vehicle kind, mass, mass basis, hull health and playability.

This query shares the Catalog's original reader, Archive-backed ShipPool and per-call
Capture. It uses the Entity header index to open the dependencies reached by the selected owners'
Ship, loadout and cargo resolvers. It returns the selected rows in memory. An empty physical
selection returns an empty source inventory and finishes before locale and Entity index setup.
Use the result's text resolvers and source inventory to resolve original keys and text identities
in the requested languages.

Check `coverage()` for owner selection/read/field gaps and `dependency_coverage()` for actual
failed Record/Entry operations, incomplete source coordinates and directory scope. Known absent,
unreadable and available-but-incomplete cells remain distinct. Preserve the three-valued
encryption marker independently of source completeness, and resolve every use against this
result's own `source_inventory()`. Return consumer-owned values from the callback, and replace
persistent state only after the outer operation succeeds and the required coverage is accepted.

The sourced `playability()` getter retains the canonical classification and the original
rule inputs, including linked crew or tag results and their physical Record lookup. A computed
classification can carry incomplete source coverage. A failed linked Record read that blocks
the decision yields `unreadable`. Use a Catalog snapshot for cargo projections, derived vehicle
kind and variant choices. See [the Ship contract](catalog.md#ship-dcb-source-fields) and
[ADR-0410](adr/0410-equipment-sdk-shares-original-family-readers-and-canonical-variant-choices.md).

The Catalog's `mass`, `massBasis` and `hullHealth` are immediate sourced aggregate cells.
Their input sets bind the current physical Record to the original Vehicle XML, the selected
Modification, patch and overrides, and the fields consulted for each metric. A Physics fallback
includes the operand used by ShipReader. Source sets distinguish a failed read from a successful
read whose original field coordinates are unavailable.

The installed Ship witness compares one original physical Record through the shared producer
and SDK. Set `P4KX_CATALOG_SHIP_SOURCE_RECORD` to a literal Record path or a unique
`class:<name>` selector. Set `P4KX_DATA_P4K` for the installed Archive and
`P4KX_SDK_WITNESS_FILE` for the private technical receipt. The receipt records evidence counts
and metric states after the Archive identity check.

```powershell
cargo test --locked -p p4kx-export --lib -- `
  catalog_query_session::installed_sdk_witness::ship_source::installed_ship_source_matches_scoped_catalog_projection `
  --ignored --exact
```

## Resolved Loadouts

Use `equipment::read_loadouts`, `equipment::read_loadouts_from`, or the
session and parsed-view `read_loadouts` methods. Choose `All`, `Guid`, or `RecordIndex`
to select the original Ship and Item Records. An admitted Ship can have an empty Loadout.
Items produce a row when resolution returns at least one port.

A row's `record_index()` identifies its physical Record in the queried DataForge.
Its `source_record_index()` gives the same position with its immediate header origin, matching
`sourceRecordIndex` on the Catalog wire. `entity_guid()` and `entity_kind()` return the original
owner identity used for joins. `source_entity_guid()` and `source_entity_kind()` give those values with
their immediate origins, matching the `entityGuid` and `entityKind` cells on the wire. Use
`port(&[0, 1])` for the second nested port of the first root port. The zero-based occurrence
path preserves repeated names and repeated instances of an attached item. It follows resolver
order for that Build.

The query uses the archive-backed Catalog resolver and returns the selected rows in memory.
It resolves original port and attachment definitions for the physical Records reached by those
ports, including dependencies outside the root selection. Stop-at-docked-vehicle rules apply
to the nested results.

`name()` borrows the canonical `SourcedValue<String>` name cell. Read its present value and immediate origin independently; repeated equal spellings can have different physical source uses. The unresolved membership/source-choice boundaries do not acquire a false complete row-wide origin.

`source_min_size()` and `source_max_size()` borrow the canonical size cells that match
`minSize` and `maxSize` in the Catalog. The value-only getters `min_size()` and `max_size()`
continue to return `Option<u32>`. Check the sourced state to distinguish absence, null,
unreadability and invalidity. XML-backed fields cite the selected vehicle and Modification,
the consulted Part and ItemPort candidates, and the actual attribute or checked absence.
Cached XML uses still bind the current physical DataForge Record. Unproved sources or
selection stages keep the field origin incomplete.

`source_flags()`, `source_port_tags()` and `source_required_tags()` borrow the separate
sourced list cells. Words within one list share the original field or attribute. The value-only
list getters return an empty slice for non-present cells.

`source_display_name_key()` borrows `displayNameKey`, including the original empty or placeholder
key and its origin. `display_name_key()` returns a key only when it is present, nonempty and not a
placeholder. Use the original port-definition link for the canonical text lookup.

`source_constraints()` exposes typed physical port and attachment links. Resolve their immediate
origin references against this result's `source_inventory`. `coverage` describes the requested
owner selection. `dependencies` records the reached resolver operations and each demanded
alignment role, including failed reads and known exclusions. Check both before replacing a
consumer cache. [ADR-0410](adr/0410-equipment-sdk-shares-original-family-readers-and-canonical-variant-choices.md)

The installed Loadout witness checks the owning GUID, category and physical Record index
against the original headers. It independently compares one port name with the original
DataForge field and checks the payload and declaration inputs. Set
`P4KX_CATALOG_LOADOUT_SOURCE_RECORD` to a literal Record path or a unique `class:<name>`
selector for an owner with a named DataForge root port. Set `P4KX_DATA_P4K` and
`P4KX_SDK_WITNESS_FILE` for the installed Archive and the private technical receipt.

```text
cargo test --locked -p p4kx-export --lib -- catalog_query_session::installed_sdk_witness::loadout_source::installed_loadout_sdk_matches_one_original_port_name --ignored --exact
```

## Direct ammunition definitions

`catalog_queries::equipment::with_ammo_definitions` reads independently admitted physical
`definitions.ammoDefinitions` rows using the canonical Ammo reader. Select one original
Record index, all physical aliases of one GUID, or the whole admitted definition family.
The callback supplies the same serializable rows, source inventory, requested localization
and header, body and field coverage. The typed speed and lifetime accessors retain the
original source bits and per-value origins. The name accessor resolves through the
snapshot's public names.

This query supplies `selected` Ammo definitions. A Catalog scoped to another owner can
instead retain Ammo as a `dependency`. Weapon-mode bindings, countermeasure bindings and
their dependency selection are separate datasets.

## Authored weapon actions and ammunition bindings

Use `equipment::with_weapon_sources`, its decoded-Entry form or the corresponding session
methods with one physical Item Record index. The snapshot contains the authored rows of
`configurations.weaponActions`, `configurations.ammoContainers` and `declarations.ammoBindings`
for that Item, with the same value states and source references as the Catalog.

The weapon component exposes its fire and aim slots, local action nodes and their original
ordinals. `mode_kind()` classifies each action. For single and rapid projectile actions,
`combat_inputs()` borrows the original rate, projectile count and damage multiplier cells.
Inspect their states and origins independently. An incomplete component selection remains
visible in the snapshot coverage even when the readable components produce rows.

Magazine and Ammo References in these authored rows report original header resolution.
Use the projectile ammunition query for a resolved action route and its reached Ammo.

## Primary weapon magazine resolutions

Use `equipment::with_weapon_magazine_resolutions` with the physical Item index.
The decoded-Entry, session and parsed forms return the same
`projections.weaponMagazineResolutions` rows and source evidence.

This query reports the primary route observed by the Catalog, including an
on-item container, a linked physical magazine and slot, a readable target
without a container, an undetermined container and an unreadable target.
An empty row set means the queried Item supplied no observed primary route
within its reported coverage. Read the authored bindings separately to inspect
links that were never followed. Source-body reads are scoped to the selected
Item and any magazine reached by its primary route. Projectile actions, Ammo
definitions and localization are outside the query scope.

Verify the primary magazine SDK against one original own container and one
original linked magazine without generating a Catalog. Set `P4KX_DATA_P4K_B`
to include a second installed Build.

```powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_weapon_combat::magazine::weapon_magazine_sdk_matches_original_primary_routes -- --ignored --exact --nocapture --test-threads=1
```

## Projectile ammunition

Use `equipment::with_projectile_ammo`, its decoded-Entry form or the corresponding session
methods with one physical Item Record index. The snapshot carries the original outcomes from
`projections.weaponProjectileAmmoSelections` and the readable `definitions.ammoDefinitions`
reached by those outcomes. Join the action to its authored weapon component and original
action occurrence ordinal. Join a selected Ammo cell to the definition's physical Record
index. Magazine admission checks the original reference's physical header and uniqueness
before opening the magazine body. The query reads only the requested Item and its admitted
magazine and Ammo dependencies. It performs no localization lookups.

Read the typed route outcome and its source completeness before treating a missing result
as absence. A proved route can still name an unreadable Ammo body in dependency coverage.
`AmmoSelection` reflects the unfiltered Catalog admission policy. The query returns the
source inputs for each action, while combat-base projections apply separate rules.

With `P4KX_DATA_P4K` set to an installed Archive, this verification checks one independently
selected weapon action, its original route and its reached Ammo. Set `P4KX_DATA_P4K_B` to
check a second Archive in the same run. Each check reads DataForge once and creates no Catalog.

```powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_weapon_combat::projectile::projectile_ammunition_sdk_matches_original_sources -- --ignored --exact --nocapture --test-threads=1
```

## Player weapon animation candidates

Use `equipment::with_weapon_animation_candidates` to read the canonical
`projections.weaponPlayerAnimationCandidates` rows for one physical Item.
The decoded-Entry, session and parsed forms share the Catalog's AttachDef,
Player database, scoped tag and option projection. Rows retain both Entry
identities, original branch and option coordinates, ordered tag matches,
playback evidence and their own source inventory.

The query reads the selected Item and the Player context and reachable
Mannequin dependencies required by its personal-weapon attachments. It
matches the source declarations within that graph. Scoped results include
the physical owner coverage, Player and graph observations, and failed
Entry operations. An empty result can be interpreted using those observations.
The query creates no Catalog or temporary row spool and opens no localization
payloads.

## Weapon recoil definitions

Read the sourced recoil links and interval from `WeaponAction::recoil()` on a
canonical weapon action. For a resolved regular or grappling link, pass its
physical target Record index to `equipment::with_weapon_recoil_definitions`, the
decoded-Entry form, or the same query on an existing session or parsed view.

The snapshot holds the canonical `definitions.weaponActionRecoilDefinitions`
rows with exact authored hands, aim, body and head values, remaining original
fields and the source inventory. `CatalogRecordSelection::Guid` retains all
matching physical Records, and `All` selects the definition family in original
Record order. Inspect scope coverage before treating a missing row as absent.
The query reads no unrelated Record bodies or localization payloads.

Verify the recoil SDK against an independently selected original action and its
physical definition. Set `P4KX_DATA_P4K_B` to check a second installed Build in
the same run. The witness compares the original references, interval bits,
recoil blocks and nested field origins without generating a Catalog.

```powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_weapon_recoil_sdk::weapon_recoil_sdk_matches_original_action_and_definition -- --ignored --exact --nocapture --test-threads=1
```

## Weapon combat bases

Use `equipment::with_weapon_combat_bases`, its decoded-Entry form or the corresponding
session methods with one physical Item Record index. The result retains every original
fire-action slot in Catalog order, with the same units, formula version, damage channels
and source cells as `projections.weaponCombatBases`. The same read supplies the projectile
route outcomes, reached Ammo definitions and their dependency coverage.

Beam, burst, charge and conditional modes retain their classifications even if the weapon
has no projectile route. Missing or unreadable action slots also retain their own outcomes.
Nominal bullet metrics carry separate source states for fire rate, damage per discharge,
nominal DPS and nominal flight distance. Crafting adjustments and ship loadout totals use
the rules in the [combat quality contract](catalog-combat-quality.md).

With `P4KX_DATA_P4K` set, this verification traces the original mode positions and checks a
nominal bullet mode for independent alpha, DPS and distance values. Set `P4KX_DATA_P4K_B`
to check a second installed Archive. The test reads DataForge once per Archive and creates
no Catalog.

```powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_weapon_combat::sdk::weapon_combat_sdk_matches_original_sources -- --ignored --exact --nocapture --test-threads=1
```

## Starmap container placements

Use `starmap::read_container_placements` to read the canonical `placements.containerInstances` and
`placements.unresolvedContainerLinks` rows for one original system root. Give a physical root Entry
index to read a whole root. `ContainerPlacementSelection::Occurrence` selects all rows with the
given root-to-leaf Child XML ordinal sequence. `ExactOccurrence` also selects the positions
where Children were read from external descriptors. Use this form with the three coordinates
from a returned `ContainerInstanceKey` when a join needs one physical route. These coordinates
belong to the queried Archive. Empty Child and external-step sequences address the root itself.

The query checks the selected root's reachable descriptor and placement graph. An occurrence
selector limits the rows that are projected. The query reads the Archive directory and the reachable
container descriptors and uses the canonical container projectors to produce the two tables.

The result carries Build, original provenance and selected-scope coverage. Check
`coverage.is_complete()` before treating an empty row list as a confirmed absence. Root availability,
Entry failures, malformed or blocked routes and caller-selected traversal allowances remain visible
even if the query could emit some rows.

## Cargo Grid configurations

Use `equipment::with_cargo_grids`, its decoded-Entry form or the equivalent session methods with one
physical Entity Record index. The snapshot returns the Cargo Grid rows of
`configurations.facilities`, the co-owned item storage rows and the one original
`definitions.inventoryContainers` row consulted by the capacity calculation. The capacity uses the
same original operands, states and provenance as the complete Catalog. Owner and dependency coverage
are reported separately. Before caching a capacity, check its typed status and source-set
completeness as well as coverage. The original Reference and capacity status describe candidates
rejected before a dependency body read.

This query covers authored configuration and the reached inventory definition. Installed ship
totals and `placements.facilityInstances` use the global Loadout order and a separate projection.

## Cargo loading and shop fees

Use `equipment::cargo_globals::with_cargo_loading_globals` and
`with_shop_commodity_globals` for
`configurations.cargoLoadingGlobals` and `configurations.shopCommodityGlobals`.
Both have a decoded-Entry `_from` form and equivalent session and parsed-view methods.
Select all rows, a GUID with all its physical occurrences, or a physical Record index.
The rows use canonical GUID order with physical Record index as the tie-breaker.

Both queries share the Catalog's authored projection. Typed getters return sourced exact Float32
times, signed Int32 fees, SCU-keyed box cells and the retained original fields outside the typed
projection. Resolve each cell's source references within the snapshot's own inventory. The queries
read selected DataForge bodies without localizing values or opening Loadouts. Check availability
and coverage separately. Publish consumer-owned changes after outer query success and after
checking the required coverage. [Cargo loading contract](catalog-cargo-loading.md).

## Authored shield generators and quantum drives

Use `equipment::ship_systems::with_shield_generators` and `with_quantum_drives` for
`configurations.shieldGenerators` and `configurations.quantumDrives`. Both support
the decoded-Entry `_from` form and equivalent session and parsed-view methods.
Select a physical Item Record index, all physical Item Records with a GUID, or the
complete authored component family. The rows keep physical owner and original
component order, including multiple components on the same Item.

The SDK and Catalog use the same authored projection. Shield rows expose health,
regeneration, reserve ratios, stun and ordered resistance and absorption slots. Drive
rows expose fuel and range, standard and spline profiles, heat and boost. Each cell
keeps its original storage and source evidence. Additional fields preserve original
members outside the typed projections. The queries use selected DataForge bodies
without opening locales or resolving installed ship loadouts. Check coverage and
resolve source references against the snapshot's own inventory.

For the consumer convention on quantum acceleration, see the
[quantum drive contract](catalog-quantum-drives.md).

## Verify dataset coverage

The SDK coverage audit checks canonical dataset boundaries against calls to the public typed
queries. It compares the generated row schemas with the corresponding Catalog schemas. A route
with partial selection or missing parity evidence remains open in the audit.

```text
cargo test --locked -p p4kx-export --test catalog_schema dataset_ownership::sdk_dataset_coverage::every_catalog_v3_dataset_has_a_verified_sdk_query -- --ignored --exact
```

The report names each dataset with an unregistered or partial witness. A dataset may already
have an SDK query while its coverage witness is pending. The ordinary schema suite checks
registered witnesses and rejects new unclassified schema boundaries and incompatible changes.
