# Selective Catalog v3 data

## Pick an entry point

Use `p4kx_extraction::Archive` for the opened input and `p4kx_export` for Catalog output and
canonical queries. Choose the route that matches the required output.

| Need | Rust entry point |
| --- | --- |
| Generate a versioned Catalog v3 document | `p4kx_export::build_catalog` for in-memory bytes or `write_catalog_file` for a staged file. Choose `catalog_schema_for(provenance_mode)` for the publication profile. `catalog_marker_schema()` matches the default output and `catalog_schema()` matches Full. |
| Generate the reduced Basetool upload | `p4kx_export::write_basetool_catalog_file` with `BasetoolCatalogOptions`. See the staged file example and scope below. |
| Read one canonical dataset or Record | A domain reader under `p4kx_export::catalog_queries`, such as `read_blueprint_categories` for an owned snapshot or `with_item_ports` for a borrow scoped to a callback. |
| Query the same Archive repeatedly | Open one `CatalogQuerySession`, call `parsed` once and use the resulting view to reuse decoding and lookup indexes. [Session example](#reuse-an-archive-session) |
| Build a consumer-owned equipment index | Use an `equipment` visitor such as `visit_item_definitions` or `visit_loadout_owners` to process readable owners one at a time. Publish staged results after successful traversal and the required coverage checks. |

Return owned values from a `with_*` callback if they must outlive the query. A visitor can call
the consumer before a later failure or cancellation. Stage changes until the operation returns
successfully and review its coverage before replacing cached data. Build ship, parts and mining
indices on the same parsed session. The [equipment selection guide](#choose-the-equipment-scope)
lists the queries for original definitions, declared constraints and resolved default equipment.

For a file export, use `CatalogFile::create_protected` inside the lazy file factory with the
Archive's write protection. Create an `ArchiveWatch` for the same opened Archive and pass it
to `file.commit_if_current(&watch)` after writing succeeds. The commit checks the source stamp
immediately before replacing the destination. A changed Build or cancellation preserves the
prior file and reports any separate cleanup failure. A generic `Write` sink leaves staging
and rollback with the caller.

## Export a reduced Basetool file

`write_basetool_catalog_file` builds the fixed Basetool scope from one opened
Archive. It takes a caller-owned scratch directory and a lazy `CatalogFile`
factory. The source reads and decoded size check complete before the factory
runs. The returned summary carries the format and wire version plus the
decoded and compressed byte counts. Publish the returned file with a watch
created before the export.

```rust,no_run
use std::error::Error;
use std::path::Path;
use std::time::SystemTime;

use p4kx_export::{BasetoolCatalogOptions, BasetoolCatalogSummary, CatalogFile};
use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken};

fn export_basetool(
    archive: &Archive,
    scratch: &Path,
    output: &Path,
) -> Result<BasetoolCatalogSummary, Box<dyn Error>> {
    let cancel = CancelToken::default();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    protection.ensure_writable(scratch, cancel.flag())?;
    let watch = ArchiveWatch::new(archive);
    let mut options = BasetoolCatalogOptions::new(SystemTime::now());
    options.cancel = Some(&cancel);
    let (summary, file) = p4kx_export::write_basetool_catalog_file(archive, &options, scratch, || {
        CatalogFile::create_protected(output, &protection, &cancel)
    })?;
    file.commit_if_current(&watch)?;
    Ok(summary)
}
```

The archive-to-file operation uses the reduced producer's memory acceptance
limit. The per-query memory limit applies to individual scoped data reads.
The CLI takes an explicit `.json.zst` destination using
`p4kx catalog basetool <archive> --out <file.json.zst>`. Use
`p4kx_export::basetool_catalog_schema()` for structural validation of the wire
format. [Reduced format and selected fields](catalog-basetool-scope.md)

## Blueprint categories and gameplay properties

Use `p4kx_export::catalog_queries::read_blueprint_category_lists` to read canonical Blueprint
category lists directly from an opened Archive. Supply `CatalogRecordSelection::All` to
read the domain, `Guid` to read all matching physical declarations, or `RecordIndex` to read a
known physical Record in the current DataForge. The physical index is local to the selected
input. The `_from` form reuses an already decoded `DataForgeEntry` from the same Archive.

The query opens the complete DataForge and projects the selected category-list bodies.
Category links retain their target header and resolution state. Their order, duplicates, null
slots and dangling links match `declarations.blueprintCategoryLists` in the complete Catalog.
The snapshot carries family availability and unreadable Record positions. Full provenance adds
the source uses and source inventory for the returned cells. The query opens no localization or
placement payloads.

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
source uses through each full-provenance result's own inventory.

## Provenance detail

Selective queries default to `ProvenanceMode::MarkersOnly`. The outputs retain the typed game
values, canonical text and media references, selection coverage and each sourced value's
`containsEncryptedSource` marker. The marker is true for a known encrypted contributor, false
when all contributors are known and unencrypted, and null when the contributors are insufficient
to reach either conclusion.

Use `CatalogQuerySession::with_provenance_mode(ProvenanceMode::Full)` before calling
`parsed` or a session method to retain contributor references and physical coordinates.
Archive-only readers that expose a `*_with_provenance_mode` function accept the mode on
that function. Descriptions of source uses, source inventories and physical coordinates in this guide use this mode.

## Archive source metadata

Read `source.channel` with `p4kx_channels::channel_name(archive.path())`. Use
`Archive::file_len` and `Archive::file_modified` for `source.archiveSize` and
`source.archiveModified`. Format the optional modification time with
`p4kx_extraction::utc_rfc3339` for the catalog wire value.

`Archive::build` and `CatalogQuerySession::build` return the optional manifest
Build. Its `branch`, `version`, `change_number` and `build_id` methods supply the
corresponding catalog fields. For `source.build.publicPatch`, call
`PublicPatches::of(build)` on the launcher-log snapshot supplied to the exporter.
This lookup requires the same change number.

The complete Catalog replay on LIVE 13220821 and PTU 13248297 verifies the
channel, size, modification time and four manifest Build fields against the
same receipt and the installed Archive. The installed Launcher logs had no
entry for either exact change number. Both catalogs carried `publicPatch: null`.
Synthetic tests cover a matching Launcher entry and an entry for a different
change number.

To repeat the installed Launcher comparison, set `P4KX_DATA_P4K`,
`P4KX_CATALOG_MEASUREMENT_DIR` and `P4KX_LAUNCHER_LOGS`.

```text
cargo test --locked -p p4kx-export --test real_archive real_archive_catalog_table_parity::installed_public_patch_matches_the_same_catalog_build -- --ignored --exact --test-threads=1
```

The catalog's writer version, timestamp and Archive-key usage describe its export.
Each selective result carries its own `SourceInventory` for resolving per-value
origins.

## Original location vocabulary

Use `catalog_queries::location_vocabulary::with_location_vocabulary` or its `_from` form
for the canonical `definitions.locationClasses`, `locationFunctions`, `locationSizes`,
`locationResourceSlots` and `locationRegistries` rows. Query sessions and parsed views
expose the same operation.

Choose one `LocationVocabularyFamily`, then select all admitted Records in that family,
every physical occurrence of a GUID, or a single `CatalogRecordSelection::RecordIndex`.
The snapshot carries the matching typed row variant, Build, family availability,
selection coverage and source inventory. The rows follow the Catalog's GUID and physical
Record order. Scope labels, resource slot tags and registry members keep the authored
values and original positions. Resolved registry members retain target header evidence.
Use the target family's query to inspect the reached definition.

The query reads selected DataForge bodies and any required target headers. It opens
no locale or placement payloads. Before publishing consumer-owned results, check outer
query success and the selected coverage.

The Windows AMD64 `real_archive` test
`real_archive_scoped_equipment_sdk::location_vocabulary_parity::installed_location_vocabulary_cold_query`
ran each family with an `All` selection in a fresh process. Each run used the
installed Archive and checked complete source coverage and the contributing
source closure. All 14 Archive and manifest stamps matched before and after
the runs.

| Catalog dataset | LIVE `13220821` availability and rows | PTU `13260598` availability and rows |
| --- | --- | --- |
| `/definitions/locationClasses` | `Absent` (0) | `Available` (9) |
| `/definitions/locationFunctions` | `Absent` (0) | `Available` (15) |
| `/definitions/locationSizes` | `Absent` (0) | `Available` (5) |
| `/definitions/locationResourceSlots` | `Available` (3) | `Available` (66) |
| `/definitions/locationRegistries/classes` | `Absent` (0) | `Available` (1) |
| `/definitions/locationRegistries/functions` | `Absent` (0) | `Available` (1) |
| `/definitions/locationRegistries/sizes` | `Absent` (0) | `Available` (1) |

The six `Absent` LIVE families each had no matching original type declaration.
The LIVE resource slots and every PTU family traced contributing values to
one physical source per family.

| Build | Maximum Windows lifetime private commit (bytes) | Maximum Windows lifetime working set (bytes) |
| --- | ---: | ---: |
| LIVE `13220821` | 1,091,608,576 | 1,021,853,696 |
| PTU `13260598` | 1,114,972,160 | 1,032,101,888 |

Both lifetime peaks stayed below 2,000,000,000 bytes in every process.
The test executable was built from `1ea6a27fa8a430fd1d7cb1aef97640a846d19fea`
with the location vocabulary witness. Its SHA-256 was
`fe34e0e11643b00a44c21a7f77eec0b039323faf5fae1a8f0cc03f29d1219e01`.
The same executable compared all seven ordered tables with each installed Build's
completed Normal Catalog. The comparison matched the availability states above,
70 contributing uses on LIVE and 1,457 on PTU, each resolving to one physical
source.

| Build | Normal producer | Elapsed seconds | Peak private commit (bytes) | Peak working set (bytes) |
| --- | --- | ---: | ---: | ---: |
| LIVE `13220821` | `23addc5e164f65a28a6f258302b30453b331edce` | 4,126.206 | 1,091,653,632 | 1,023,537,152 |
| PTU `13260598` | `93d92ca20bef8a8c3731339214a3301124ade7ba` | 4,107.900 | 1,114,607,616 | 1,033,596,928 |

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

## Original ecosystems and terrain shading classes

Use `terrain::read_ecosystems` or `terrain::read_shading_classes` for
`definitions.ecosystems` and `definitions.shadingClasses`. The shared
`LooseDefinitionSelection` accepts all admitted Entries, a physical Entry index,
an Archive path, or an authored GUID. Direct selections open only their
selected owner definitions. Duplicate-owner issues describe that scope.
GUID selection reports unreadable possible aliases separately.

The queries preserve original protected labels, sound declarations,
texture and mask selectors, numeric tokens and additional raw fields with
their own origins. Texture paths consult directory Entries and never open
texture payloads. These queries require no DataForge or localization. The
returned data describes the selected original definitions.

## Original geology definitions

`catalog_queries::terrain::read_geology_sets` and
`read_geology_texture_sets` expose the canonical `/definitions/geologySets` and
`/definitions/geologyTextureSets` rows. Both accept all, GUID, Archive path or
physical Entry index selection and return field origins and scoped diagnostics.

Geology sets retain the original asset order, numeric tokens and candidate evidence.
Nonzero linked GUIDs use the canonical four terrain families. Texture sets retain
the ground and biplanar layers and use the Archive directory to resolve texture
selectors. Both queries work without DataForge and leave texture payloads unopened.

## Original soil definitions

Use `soils::read_soils` or `soils::read_soil_texture_sets` for `definitions.soils`
and `definitions.soilTextureSets`. Both accept a `LooseDefinitionSelection` by
all admitted Entries, physical Entry index, Archive path, or authored GUID.
They retain physical Entry order and preserve original number tokens and field evidence.

A direct Entry or path selection opens only its selected owner definitions.
Owner duplicate diagnostics apply to returned rows. A direct selection gives no
evidence of uniqueness outside that scope. A GUID selection discovers all possible
physical owners and reports unreadable candidates separately.

A nonzero texture-set GUID in a selected soil consults the original GUID fields of the
four canonical terrain target families. Matching Entries retain their physical identities
and families. An unreadable possible target can leave the link unavailable while a
readable owner is complete. The per-value origin reports which original sources were
actually recorded. Scope completeness describes the owner selection.

Texture sets resolve texture paths from Archive directory metadata. Neither query
opens DataForge, localization or texture payloads.

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

## Original climate assets

`catalog_queries::climate::read_climate_assets` reads `/definitions/climateAssets`
by owner GUID, Archive path, physical Entry index or all owners. The typed rows
include authored growth buckets, climate ranges, shading roles and original
sub-asset slots. The matching loose resource and shading GUIDs use the same
candidate evidence as the Catalog.

The standalone query opens DataForge when a selected sub-asset declares a nonzero
Record GUID. The SDK correlates the GUID with the original Record headers.
Applications that already have a query session can reuse its parsed DataForge.
Geometry, material and audio fields retain their authored values. Media payloads
stay unopened.

## Original geology assets and creatures

`catalog_queries::terrain::read_geology_assets` and
`catalog_queries::creatures::read_creatures` expose
`/definitions/geologyAssets` and `/definitions/creatures` by GUID, Archive path,
physical Entry index or all owners. Both share the canonical Catalog projection
and return the original per-field evidence for the selected rows.

Entity class references use original DataForge Record headers. The standalone
queries open DataForge when a selected owner declares a valid nonzero Entity
class GUID. Query sessions can reuse a parsed DataForge. Geology asset references
use Archive directory lookups for geometry, material and mineable paths. The
queries leave models, materials, textures and audio payloads unopened.

### Installed Terrain cold reads

On Windows AMD64, each standalone query ran in its own process on LIVE
`13220821` and PTU `13260598`. Numeric counts below are Available results.

| Definitions | LIVE | PTU |
| --- | ---: | ---: |
| `ecosystems` | 131 | 163 |
| `shadingClasses` | Absent | 13 |
| `geologySets` | Absent | 6 |
| `geologyTextureSets` | Absent | 3 |
| `geologyAssets` | Absent | 68 |
| `creatures` | Absent | 6 |
| `climateAssets` | Absent | 35 |

All 14 reads completed with stable Build and Archive stamps. The largest
private commit was 1,137,717,248 bytes and the largest working set was
1,058,566,144 bytes. The verifier was built from
`eed6c4b45f7faf4ce325c3a99f6f97281a33b572`.

### Comparison with the complete Catalog

On both Builds, the Terrain verifier streamed the complete Normal Catalog
and matched all seven definition tables, including availability, ordered
rows, and the source uses and physical sources referenced by those rows.
LIVE matched 3,669 uses from 132 physical sources. PTU matched 35,554 uses
from 391 physical sources. The largest private commit was 1,138,044,928
bytes and the largest working set was 1,064,734,720 bytes.

The matching Normal Catalog producer revisions were
`23addc5e164f65a28a6f258302b30453b331edce` on LIVE and
`93d92ca20bef8a8c3731339214a3301124ade7ba` on PTU. The verifier used the
same revision as the standalone reads and checked the Build and Archive
stamps before and after each comparison.

## Weather configurations

Use `catalog_queries::weather::with_electrical_behaviors` for the canonical
`configurations.electricalBehaviors` rows. Select all owners, an original GUID or a physical
Record index. The query shares the catalog's source projection for lightning effects and
their ordered target modes. Reuse a parsed session or an already decoded DataForge Entry
for repeated lookups. [Weather configurations](catalog-weather.md)

## Particle libraries

Use `catalog_queries::particle_libraries::read_particle_libraries` to select the
canonical `definitions.particleLibraries` rows by physical Entry index or across
the family. Exact Entry selection opens only the selected eligible XML or CryXml
payload. The typed rows retain ordered markup, typed rain occlusion and original
field origins. Use the returned Entry counts and source inventory to check
read and selection coverage. [Particle library sources](catalog-particle-libraries.md)

## Radar audio configuration

Use `catalog_queries::radar_audio::read_radar_audio` to read original rows from
`configurations.radarAudio` by physical Entry or across the selected configuration family.
The SDK retains ordered nodes, source values, and explicit selector states without decoding
DataForge. [Radar audio](catalog-radar-audio.md)

## Client profile documents

Use `read_client_profile_settings` for both documents or `read_client_profile_settings_for` with
`ProfileKind::General` or `ProfileKind::VirtualReality` for one payload. The typed rows match
`configurations.clientProfileSettings.documents` in General then VR order. A single-profile
query retains the unselected document's directory candidates and status. Both calls return the
Archive Build, values and encryption markers. Candidate `inputRef` values identify physical
Entries within the report. Use `read_client_profile_settings_with_provenance_mode` or
`read_client_profile_settings_for_with_provenance_mode` with `ProvenanceMode::Full` to
resolve those keys and field origin references through the report's inventory.
[Profile document contract](client-profile-settings.md).

## Localization Entry metadata

Use `catalog_queries::localization::catalog_language_policy` for the static
`texts.languagePolicy` object. The function requires no Archive or session.
Use `selects_available_supported_languages()` to check whether the Catalog selects the
supported languages with localization Entries available. `uses_translation_fallback()`
returns false because a missing translation stays absent in its language. Individual SDK
queries can request an explicit language subset.

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

### Texts referenced by selected records

Localized owner queries supply the typed `texts.publicNames` and `texts.gameTexts` rows
referenced by their selected records. For example, use the query for manufacturer definitions
or `catalog_queries::law::with_law` for the original infraction records and their texts.
Follow a `PublicNameRef` or `GameTextRef` to the corresponding row's `id().value()` and
retain that query's source inventory. The text domain is set by the original owner field.
The encryption marker describes the contributing inputs independently of that domain.

Each row's provenance covers the authored key occurrences in the selected scope. The same
identifier in a complete Catalog can cite additional occurrences from other owners. Use
`CatalogLanguageSelection::AllAvailable` for all available supported locales or `Selected`
for a language subset. Missing translations stay absent in their language.

## Offline shop inventory definitions

Use `catalog_queries::shops::read_shop_inventories` to read original offline inventory
definitions. `EntryIndex` reads one physical Archive directory Entry. `ArchivePath` matches
every eligible Entry at a normalized Archive path. `All` includes every eligible inventory
file. Use the physical index from the opened Archive to read an individual file.

The snapshot uses the same rows as `definitions.shopInventories`. It keeps the original JSON
token order and numeric digits, typed navigation to inventory fields and per-value origins.
A verified empty file retains an absent document. An invalid or unreadable selected Entry
retains its own document state and scoped failure count. `coverage.failures` identifies each
failed Entry by physical index, stage and diagnostic code. Key-required and wrong-key failures
retain their original reason. An indexed lookup cites the selected
Entry path and position or, for an out-of-range index, the admitted root directory count.
Path and full inventory searches cite the paths they consulted. The query opens neither the
shop registry nor DataForge.

Set `P4KX_DATA_P4K` and `P4KX_DATA_P4K_B` to the installed Archives to verify one
original inventory from each Build and an index outside each admitted directory.

```text
cargo test --locked -p p4kx-export --test real_archive real_archive_shop_inventory_v3::sdk::selective_shop_inventories_match_an_original_inventory_and_missing_index -- --ignored --exact --nocapture --test-threads=1
```

## Offline shop inventory resolution

Use `catalog_queries::shops::read_shop_inventory_resolutions` to read the canonical
`declarations.shopInventoryResolution` rows. `All` returns all admitted registry rows.
`Key` matches the exact canonical `shopKey` value, which contains the full normalized registry
ID chain. Rows sharing a normalized key use the first valid registry occurrence. The snapshot's
availability describes the source table. A missing key in an available registry returns no rows.

For a selected shop, the query searches all admitted shop container and prefab declaration
sources. An unreadable or unattributable declaration can leave any dependent result incomplete.
The query surveys the original inventory directory, then reads only uniquely targeted
non-placeholder inventory bodies. Duplicate physical targets stay ambiguous and retain their
individual identities. Per-value origins, including encryption and completeness, use the
snapshot's own `SourceInventory`. Query coverage reports registry admission, declaration
discovery and required read outcomes. A lookup with no matching registry row skips declaration
discovery.

This query reads the Archive directly without loading DataForge or resolving starmap links.
Use `read_shop_inventories` for the original contents of a specific inventory file.

On LIVE 13220821 and PTU 13248297, the complete table and one exact key per Build
matched the same-Build Catalog, including the reachable source inventory.
A separate cold `All` query on each Build stayed below one billion bytes for
both working set and private commit.

To repeat the parity check, set `P4KX_DATA_P4K` and
`P4KX_CATALOG_MEASUREMENT_DIR` to the Archive and its complete Catalog measurement
directory. For a cold query, set the Archive path;
`P4KX_SHOP_RESOLUTION_KEY` optionally selects a single key.

```text
cargo test --locked -p p4kx-export --test real_archive real_archive_shop_resolution_sdk::installed_shop_resolution_matches_the_same_complete_catalog_and_exact_key -- --ignored --exact --nocapture --test-threads=1
cargo test --locked -p p4kx-export --test real_archive real_archive_shop_resolution_sdk::installed_shop_resolution_cold_query -- --ignored --exact --nocapture --test-threads=1
```

## Physical shop rows

Use `catalog_queries::shops::read_shop` to select a `/shops` row by the original registry
Entry index and the XML node index of a `ShopBackup`. Resolve the latter from the first
coordinate of the `@ID` use referenced by the Shop's sourced `key`. Only the first eligible
occurrence of a duplicate normalized key has a row.

The result uses the same sourced fields as the normal catalog. Its compatibility list
follows the catalog's last matching physical inventory rule. The inventory resolution
table can independently report multiple candidates. A shop without a registry location
needs no DataForge lookup. Resolve each field's source-use references in the returned
inventory.

`read_shops` accepts selections in request order and retains duplicates and missing
positions. Its rows share one source inventory and at most one Starmap name survey.
The corresponding methods on `CatalogQuerySession` and `ParsedCatalogQueries` reuse the
session's decoded DataForge and, in the parsed view, its indexes.

Use `visit_shops` for the complete `/shops` array in canonical key order. It reads the
registry and shop declarations once, opens the last matching physical inventory for
each needed stem, and shares one Starmap survey across located shops. The terminal
summary reports source availability, row count and coverage. In Full mode, resolve
the emitted rows' source-use references in the summary's source inventory. Retain
callback output only after checking the terminal result and its coverage.
`visit_shops_from` reuses a decoded DataForge Entry, and sessions and parsed views
expose the same visitor. A standalone visit with no located shops needs no DataForge.
An exhaustive Archive directory with no registry yields `Absent` in this query.
The complete catalog producer can require the registry when exporting Starmap data.

Synthetic fixtures compare the selected Shop row and the complete ordered array
with catalog output and exercise incomplete and ambiguous location evidence. Isolated Windows debug test processes built from
`0dbe2469c` resolved the original registry coordinate, a complete sourced Starmap
link and one nonempty inventory for one located row per Build.

| Build | Selection | Rows | Peak private commit (bytes) | Peak working set (bytes) | Seconds |
| --- | --- | ---: | ---: | ---: | ---: |
| LIVE 13220821 | Located | 1 | 1,139,449,856 | 1,078,960,128 | 71.426 |
| LIVE 13220821 | Absent | 0 | 953,368,576 | 956,153,856 | 7.035 |
| PTU 13260598 | Located | 1 | 1,165,918,208 | 1,091,411,968 | 67.627 |

The absent LIVE node completed without inventory or Starmap name traversal. The
PTU row matched a fresh nomination from its original registry. An installed
comparison with a Normal producer that emits the same sourced Shop representation
remains open.

## Resolve field origins

In Full mode, use each query's own `SourceInventory`. `SourcedValue::origin` supplies the encryption filter,
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
scope and retains the source detail selected for the session. The session retains the decoded
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

An application that needs to admit the decoded DataForge input by size can pass
`DataForgeReadOptions` to an Archive DataForge read and bind the result through
`CatalogQuerySession::from_entry`. The allowance covers decoded input bytes for that read.
Measure the query process separately, including indexes, selected data and any helpers.

Run the built executable directly on an available host and verify that the
peak footprint of each SDK workflow stays below 2 GB. Record the exact SDK
revision and executable digest, process and helper peaks, minimum available
system RAM, paging and temporary disk usage. The measured SDK budget informs
the headroom estimate for a desktop with 16 GiB total RAM. Check the integrated
desktop workflow and record its total footprint separately.
[Local consumer RAM acceptance](catalog-partitioning.md#system-memory-acceptance)

A Windows AMD64 harness built at `b8ad606ce` read LIVE `13220821` and
PTU `13248297` with English selected and one Rayon worker. The executable
SHA-256 was
`fb31c11dade76eadaa4a484842737d4dffbadf892f36eecaa4d165c75b4094e5`.
Windows reported 1,226,883,072 bytes peak working set and 1,228,382,208 bytes
peak private commit. The capture sampled a minimum of 56,292,515,840 bytes
available on the host. The initial refresh took 138.775 seconds and the
changed-Build refresh took 143.946 seconds. The warm query reused the cache.
Cancellation and an unavailable input both retained that cache.

The private commit peak leaves an estimated 15,951,486,976 bytes for the OS
and other processes on a 16 GiB system. This measures the mining SDK harness
on the available host. Paging, scratch writes and the integrated application's
total footprint need their own counters.
The Windows AMD64 harness at `4557f2851fd18beac00e829dd2e459165af132f6`
also exercised LIVE `13220821` and PTU `13260598`. The executable SHA-256 was
`e56384e1cf1c3d3d349d80564b0af1f97dc7e3228220aab530d72323f38c9ced`.
The same process retained its previous snapshot across the changed-Build read.
The cold phase took 139.628 seconds and the changed-Build refresh took 141.867
seconds. The warm request reused the PTU snapshot in under one millisecond.
Cancellation and unavailable input both retained the PTU snapshot. Both Archive
and Build stamps matched before and after the run.

Windows reported lifetime peaks of 1,238,855,680 bytes of private commit and
1,222,098,944 bytes of working set. The run took 295.830 seconds, with 247
process samples, a minimum 25,831,206,912 bytes of available RAM and zero
sampled temporary bytes. Windows counted 2,925 process-write bytes. Shared-host
paging advanced by 132,054 reads and 56 writes while the independent Catalog
checks were running. The private peak leaves an estimated 15,941,013,504 bytes
for the OS and other processes on a 16 GiB system.
[Reported integrated consumer trial](catalog-partitioning.md#embedded-consumer-trial)

Installed parity reads the same Build's measured normal Catalog directly from
the compressed artifact. Both checks matched the mineable elements,
compositions, provider presets, harvestable locations, reachable resources,
Starmap links and original localization used by the mining SDK.

| Build | Mining verifier revision | Matching Catalog occurrence routes | Additional SDK occurrences |
| --- | --- | ---: | ---: |
| LIVE `13220821` | `046aa1980` | 1,287 | 1 |
| PTU `13248297` | `6b0d55175` | 1,305 | 1 |

The two checks use the measured Windows portable producer at `25287d184`.
Additional SDK occurrences follow the separate SDK reach described above.
Decoded stream lengths matched the producer receipts.

On PTU `13260598`, the shared Mining query matched the full ordered Catalog
arrays for `/mineableElements` (43 rows), `/mineableCompositions` (231 rows)
and `/harvestableProviderPresets` (49 rows), including embedded source-use
references. The verifier `e35bde456` read the Normal Catalog from producer
`5d3d917f4` in 1,877.068 seconds with 1,869,099,008 peak private bytes,
including the stream replay. This proves the shared mining-location row projection.

Separate verifiers used seven standalone `All` selectors against each matching
complete Normal Catalog. They compared all rows in order and bound the original
source uses to one physical source on each Build.

| Catalog dataset | PTU 13260598 rows compared | LIVE 13220821 rows compared |
| --- | ---: | ---: |
| `/mineableElements` | 43 | 43 |
| `/mineableCompositions` | 231 | 231 |
| `/harvestableProviderPresets` | 49 | 49 |
| `/configurations/miningGlobal` | 8 | 6 |
| `/definitions/refiningProcesses` | 9 | 9 |
| `/definitions/resourceTypes` | 209 | 207 |
| `/definitions/mineableEntities` | 247 | 231 |

The PTU comparison matched 125,893 contributing uses against the Normal Catalog
from `93d92ca20bef8a8c3731339214a3301124ade7ba`. The Windows AMD64 test executable
built from `aca28691148f61a44a825db6b3dcd7979eaeb032` with the standalone Mining
comparator had SHA-256
`fc7ef084e475c03bc51ccaf4e0df56e8c876870d40a12a1b09eee2c4adc446da`.
The SDK queries and one retained Catalog stream took 5,081.889 seconds.
Windows lifetime peaks were 1,368,436,736 bytes of private commit and
1,371,025,408 bytes of working set. The minimum sampled available host memory
was 14,267,056,128 bytes.

The LIVE comparison matched 119,822 contributing uses against Normal producer
`23addc5e164f65a28a6f258302b30453b331edce`. The guard completed in 4,353.446
seconds. Windows lifetime peaks were 1,317,343,232 bytes of private commit and
1,299,329,024 bytes of working set. The verifier used the exact executable
identified with the cold checks below.
[Measured producer and streaming method](catalog-partitioning.md#system-memory-acceptance)

Each of the seven standalone `All` queries also ran in its own Windows AMD64
process on LIVE `13220821` and PTU `13260598` with one Rayon worker. Each process
opened the installed Archive and decoded and parsed DataForge without opening a
Normal Catalog frame. It checked owner and dependency coverage and contributing
source references through their
physical ancestry. Each result traced to one physical source. PTU cold row
counts matched the PTU column above. The Archive and manifest matched their
sealed Build stamps before and after all 14 runs.

| Build | Private commit peak range (bytes) | Working set peak range (bytes) | Minimum sampled host physical available (bytes) | Minimum sampled host commit headroom (bytes) |
| --- | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 1,091,407,872–1,091,612,672 | 1,021,788,160–1,048,432,640 | 34,148,630,528 | 59,509,202,944 |
| PTU `13260598` | 1,114,513,408–1,114,980,352 | 1,031,688,192–1,091,035,136 | 30,681,665,536 | 59,481,092,096 |

Windows reported both lifetime peaks below 2,000,000,000 bytes in every
standalone process. The test executable was built from
`485f28b17ae99af1b7872958f1628105ef3b76b2` with the Mining witnesses and Catalog
query changes. Its SHA-256 was
`b582efebce5f6bf1f6040e53946ea38e64dd184f1a57a1597c8f6184bbca2676`.

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
cache data after outer success. [ADR-0406](adr/0406-direct-quality-queries-preserve-canonical-definitions-without-inverse-discovery.md)

### Quantization-to-resource membership

Use `catalog_queries::quality::with_quantization_resources`, its decoded-Entry form, or the
session's `with_quality_quantization_resources` method for
`projections.qualityQuantizationResources`. All, GUID and RecordIndex select physical
output rows. The first eligible quantizer in original Record order still receives the
members, including when that candidate is outside the output selection or is a skipped
development leftover. Later same-GUID definitions keep their own eligibility and empty
membership. Distinct matching resource GUIDs are in canonical GUID order.

The snapshot's `projection` supplies rows, schema availability, source inventory and
selected owner coverage. Each row retains its physical index, eligibility evidence,
member-specific origins and the complete membership survey when it won the first match.
Read `dependency_coverage` separately to detect unsuccessful dependency lookups. Publish
consumer state only after outer success and the necessary coverage checks.

A request without an admitted quantizer performs no dependency survey. A request with
admitted owners surveys original quantizer candidates and only opens ResourceType bodies
when a selected quantizer is eligible. It shares the Catalog's original relationship
rules and opens no unrelated crafting, locale, world or media bodies.

## Mining globals and refining processes

`catalog_queries::mining` exposes `with_mining_global_params` and `with_refining_processes`,
with `_from` functions and the corresponding session/parsed methods. They return the canonical
`configurations.miningGlobal` and `definitions.refiningProcesses` rows with complete immutable
original arenas, exact numeric storage, text and physical source inventories. All/GUID/RecordIndex
preserve physical aliases. Only selected owner bodies and demanded requested refining locales are
read; ResourceType/group/world discovery and a complete Catalog are unnecessary. Examine sourced
states and owner/dependency coverage and publish caches only after outer success. See the
[complete contract](catalog-mining-refining.md).

## Choose the equipment scope

| Consumer data | Method on the parsed session |
| --- | --- |
| Resolved default equipment and nested ports for ship roots | `visit_loadout_owners(LoadoutOwnerSelection::Ships, …)`. |
| Definitions of all admitted parts, including unused parts, clothing and paints | `visit_item_definitions(ItemDefinitionSelection::Records(CatalogRecordSelection::All), …)`. Keep the consumer's mining selection across these Items when building its mining index. |
| Authored port containers and attachment constraints for all admitted Ships and Items | `visit_declared_equipment(CatalogRecordSelection::All, …)`. Use this when the index needs authored types, sizes, tags and restrictions. |
| Resolved ports, XML overrides and default equipment for standalone Items as well as Ships | `visit_loadouts(CatalogRecordSelection::All, …)`. All selects the canonical Ship and Item Loadout roots, including Items with an authored Loadout or port slots. |

A `RecordIndices` batch derived from ship Loadouts contains only the selected physical Items
reached as default equipment. For a complete parts index, run the all-Item definition visit.
Compare ordered identities, coverage and the emitted business data before switching a
consumer from resolved loadouts to declarations. Stage the cache until all requested visits
succeed.

## Declared ship and item equipment

Use `equipment::visit_declared_equipment`, its decoded-Entry variant, or the
parsed session method to build a declared equipment index in one traversal.
Select all Ship and Item Records, every physical occurrence of a GUID, or one
physical Record index. Each callback carries the original Record identity and
canonical kind, the ordered port containers and the attachment declarations.
It also carries coverage, requested port labels and an owner-local source
inventory. Readable owners without declarations still have an identity row.

The port rows include authored type and subtype restrictions, sizes, tags and
rules. The attachment rows carry the original type, subtype, size and tags.
Their values describe authored constraints. Use `equipment::visit_loadouts`
when the consumer needs resolved XML ports, Modification overrides or default
equipped items. The declared visitor reads no vehicle XML or shop inventory.
Commit consumer-owned cache changes after the traversal succeeds and its coverage
meets the requested scope.

## Authored item ports

Use `with_item_ports` or `with_item_ports_from` to read `configurations.itemPorts`
for the dataset, a GUID or a physical Record index. A parsed session can call
`with_item_ports` through the same DataForge view as the other domain queries.
The callback borrows the canonical rows and original value arenas. Return
consumer-owned values from the callback and publish them after the query succeeds.

The snapshot carries Record, component and port positions, original value states,
scope coverage, public labels in the selected languages, and the field origin inventory.
`with_item_ports` loads requested locale payloads when it reaches the first readable selected
owner. A selection without a readable owner reports the requested languages and an empty
available-language list. An unreadable requested locale fails a query with a readable owner.
The archive-backed loadout resolver supplies equipped-item and XML-override data.

Use `visit_item_ports` or `visit_item_ports_from` to process one readable physical Record
at a time. Requested locale payloads load when traversal reaches the first readable owner.
In a traversal with no readable owners, the visitor reads no locale payloads.
The visitor borrows its port rows, public labels and source inventory for the current owner.
Empty snapshots count toward the terminal owner count. An unreadable component selection
is included in the terminal
coverage. Commit consumer-owned output only after a successful traversal whose coverage
meets the consumer's requirements.

`ItemPort::alignment_axis()` borrows the original axis label.
`alignment_axis_option_status()` borrows its optional `SourcedValue<AttachmentAxisOptionStatus>`.
`Declared` and `Undeclared` describe membership in the original axis enumeration; an undeclared
label remains unchanged. An unavailable label has no membership cell, so inspect the label's own
state to distinguish absent and unreadable input. A retained membership cell preserves its own
value state, encryption marker, completeness and original source uses, including unavailable
option evidence. This status describes the declaration and does not infer an alignment transform.

## Original tag definitions and root declarations

Use `catalog_queries::tags::with_tag_declarations` or a parsed session's
`with_tag_declarations` for `/definitions/tags` and `/declarations/tagRoots`.
Choose `TagDeclarationFamily::Definitions`, `Roots` or `All`, then select all
physical Records, every occurrence of a GUID, or one physical Record index.
Definition rows preserve duplicate GUIDs and order by GUID and Record index.
TagDatabase rows order by physical Record index.

Each authored array retains null, dangling, wrong-type and repeated references.
A resolved reference identifies the original target by its physical header. To
read a target's values, use a second declaration query for that Record index.
An empty selection keeps its source family's availability. Check scoped
coverage before treating an empty selection as established absence. The query
reads the selected bodies and resolves target headers without building the
convenience path tree or loading localization.

## Canonical tag paths

Use `catalog_queries::tags::with_tag_paths`, its `_from` form or the
parsed session method for `/projections/tagPaths`. The query reads the original
Tag and TagDatabase graph and shares the Catalog's first-readable GUID and depth-first
path selection. It reads no localization and invokes no other Catalog domain.
The full Tag graph supplies ordered roots, ancestors, aliases and competing
edges even when a caller selects a single Record or GUID.

Paths appear in canonical traversal order. A GUID or selected physical Record
returns the winner path. A later physical alias can still be queried through
`with_tag_declarations`. Use the path row's `fields()` method for its typed
path, depth, listed flag and selected parent header. Field origins retain only
the selected original contributors. The snapshot's inventory also records
the original graph observations, and the graph outcome counts explain
dangling or cut edges independently of the returned rows. Check
`graph_inventory_complete()` before treating a missing row as an absence.

### Installed Tag cold queries

The Windows AMD64 verifier at `8db10c610d05c52fa77e725b9fc97e99b88e7b9a`
ran each complete Tag query in a fresh process with one worker. It used the
installed Archive and preserved the same verified Archive and Build stamps
throughout. Each result cited one physical source and skipped zero owners.

| Dataset | LIVE `13220821` rows | PTU `13260598` rows |
| --- | ---: | ---: |
| `/definitions/tags` | 18,961 | 19,688 |
| `/declarations/tagRoots` | 1 | 1 |
| `/projections/tagPaths` | 18,961 | 19,688 |

| Build | Largest private commit (B) | Largest working set (B) |
| --- | ---: | ---: |
| LIVE `13220821` | 1,091,600,384 | 1,038,782,464 |
| PTU `13260598` | 1,114,959,872 | 1,045,348,352 |

The six queries took 6.039–10.458 seconds. The executable SHA-256 was
`27a81c331df0ede8f2bd66c1a26f19a615d5884e7133cf88fedc8e0f1a7191d`.
The cold tests measure the SDK scope. The retained comparison checks the same
tables and contributing source inventories against the complete Catalog.

On PTU `13284307`, the signed Windows AMD64 verifier at
`84e0a9ca95ca03da3cae281e24240be9377c7ea5` repeated the Full SDK queries in
three separate, one-worker processes. Each verified the same installed
Archive before and after the query, reported one physical source, and
skipped zero owners. The executable SHA-256 was
`a7d90528bd267b98823076568bf82f1a08d9f06a3b342fd2dbd358b147fd33f9`.

| Dataset | Rows | Source uses | Elapsed (s) | Private commit peak (B) | Working-set peak (B) |
| --- | ---: | ---: | ---: | ---: | ---: |
| `/definitions/tags` | 19,688 | 255,815 | 8.832 | 1,115,107,328 | 1,032,069,120 |
| `/declarations/tagRoots` | 1 | 356 | 5.557 | 1,115,103,232 | 1,032,069,120 |
| `/projections/tagPaths` | 19,688 | 315,026 | 8.811 | 1,115,070,464 | 1,043,304,448 |

All three SDK processes stayed below 2,000,000,000 B of private commit.

### Installed Tag retained parity

The Windows AMD64 verifier at `00e050aa9f487b54a3c4310981f299612e454d7e`
compared the same three complete SDK tables against the retained, unfiltered
Normal Catalog from `27da392284eed8c67cf58c15ea473089d6c0a20c` and the
installed LIVE Build `13220821`. In canonical table order it matched 18,961
Tag definitions, one root declaration and 18,961 Tag paths. The comparison
verified row digests, 284,425 distinct contributing source uses and one
physical source from a 78,149,434,545-byte decoded frame.

The one-worker readback completed in 644.916 s with 1,091,432,448 bytes
of peak private commit and 1,029,189,632 bytes of peak working set.
Minimum sampled host RAM available was 35,967,827,968 bytes. The verifier
process recorded 141,143,604 bytes of Windows write transfers, including
temporary comparison data.

The Rust verifier exited successfully. Its launcher's case-sensitive
summary label check rejected the transcript. A separate readback verified
the original transcript, successful exit, process peaks, stable Archive
and Build, signed verifier, and complete compressed input. With the exact
capitalization, the transcript reader accepted the three family counts
and rejected a deliberately mismatched summary count. The original
launcher receipt remains available unchanged. The retained `13260598`
frame still needs a matching installed Archive.

On PTU `13284307`, the verifier and complete Normal Catalog both came from
signed revision `84e0a9ca95ca03da3cae281e24240be9377c7ea5`. The replay
matched 19,688 Tag definitions, one root declaration and 19,688 Tag paths in
canonical order. It matched 295,330 contributing source uses and their one
physical source. The installed Archive and compressed frame seals verified.

The one-worker comparison took 2,206.339 s with 1,114,779,648 B peak private
commit and 1,046,933,504 B peak working set. Minimum available host memory
was 36,210,360,320 B and minimum commit headroom was 59,859,345,408 B.
The elapsed time includes streaming the retained Catalog.
[Same-revision producer and offline replay](catalog-partitioning.md#offline-ptu-replay)

## Original Entity tag declarations

Use `catalog_queries::tags::with_entity_tags` or a parsed session's
`with_entity_tags` to read `/declarations/entityTags`. Select all Entities
admitted by the normal Catalog policy, all physical Records with a GUID, or
one physical Record index. The snapshot contains Build identity, declaration
availability, Record coverage and the original source inventory.

The shared projector preserves each root `tags[]` occurrence in authored order,
including repeated, null and unresolved references. The Entity Record owns the
occurrence. A readable physical Tag Record supplies its own target-header
evidence. The query uses DataForge and resolves these original headers directly.
See the [Tag contract](catalog-tags.md) for the declaration shape.

## Mission type definitions

Use `with_mission_types`, its decoded-Entry variant or the parsed session method to
read `definitions.missionTypes` by GUID, physical Record index or for the dataset.
Each row has the physical Record header and the protected `LocalisedTypeName`
reference. Use `snapshot.game_text(reference)` to look up its requested translations.
The query reads the selected mission types and their requested localization inputs.
Use the per-field source uses and the snapshot's inventory to trace their origin.

## Mission locality definitions

Use `with_mission_localities`, its decoded-Entry variant or the parsed session
method to read `definitions.missionLocalities` by physical Record index, GUID or
for the dataset. The query reads selected locality Records and resolves original
reference targets from their headers. The snapshot includes the Build, source
inventory and Record coverage.

Each row's `available_locations()` preserves its authored order, repeated
occurrences, nulls and unresolved GUIDs. A resolved reference carries separate
source evidence for the array slot and its target header. The query reads no
localization or contract pickup projections.

## Authored mission definitions

Use `mission_properties::with_mission_properties` or its decoded-Entry and session
forms for `configurations.missionProperties` and `configurations.missionFlows`. Select
original Contract Template or Mission Broker Entry Records by physical index, GUID
or the complete owner family. The snapshot supplies property rows in `rows`, flow
rows in `flows` and the exact source arena of each selected owner. Join using the
owner family, input reference, Record index and GUID. Resolve declarations, objectives,
properties, flow triggers and referenced fields with the arena's typed `field` and
`value` methods. Shadowed declarations and objectives retain their physical positions
and original values. The query reads no localization, mission
dependency graphs or contextual projections. Use `with_contracts` for the separate
Generator-authored `missionPropertySets`.

Use `mission_properties::visit_mission_properties` for bounded reads of both tables.
The Archive, decoded-Entry, session and parsed forms visit Contract Templates before
Mission Broker Entries in canonical GUID order, with physical Record order for equal
GUIDs. `CatalogRecordSelection::All` uses the Normal producer's path, type and
development-leftover policies. GUID and physical-index selections apply the same
admission rules. Each callback sees that owner's rows, arena and source inventory.
Stage consumer changes until the visit returns successfully and its terminal coverage
meets the caller's requirements. The query retains the selected Record headers and
reads one body and its original source closure at a time. Coverage follows Normal
projection rules. Informational unresolved references retain their original
identity and error evidence. Unreadable values and unsupported required flow
interpretations remain incomplete.

Use `mission_definitions::with_mission_definitions` or its decoded-Entry and session forms
to read `definitions.missionScenarios`, `definitions.missionModuleDeclarations`,
`definitions.missionItems`, `definitions.contractTemplates` or
`definitions.missionBrokerEntries`. Choose a `MissionDefinitionFamily` and a physical
Record selection in `MissionDefinitionSelection`. The result preserves canonical
row order, original source coordinates and coverage for the selected family.
For Contract Templates and Mission Broker Entries, `All` and `Guid` follow
Normal Catalog admission and count excluded development leftovers. `Guid`
retains each admitted physical Record sharing that GUID. `RecordIndex` can also
inspect an excluded original Record when its type and path match the family.

Use `mission_definitions::owner_visit::visit_mission_owner_definitions` for an
owner-at-a-time read of Contract Templates or Mission Broker Entries. The
`_from` variant accepts a decoded Entry of the same Archive. Catalog query
sessions and parsed queries expose the same method.

```rust
use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::mission_definitions::owner_visit::{
    MissionOwnerDefinitionRow, MissionOwnerDefinitionSelection,
};

let selection = MissionOwnerDefinitionSelection::MissionBrokerEntries(CatalogRecordSelection::All);
let summary = parsed.visit_mission_owner_definitions(selection, Some(&cancel), |snapshot| {
    if let MissionOwnerDefinitionRow::MissionBrokerEntry(broker) = &snapshot.row {
        stage_broker(broker, &snapshot.source_inventory)?;
    }
    Ok::<(), ConsumerError>(())
})?;
```

The callback borrows the canonical definition, including its source arena
and flow and property links, together with that owner's source inventory.
Resolve the row's source-use references in that inventory and retain the
contributing uses and physical source ancestry when staging the row. Rows are
visited in canonical GUID order, with physical Record order for equal GUIDs.
The selection policies match the materialized query.

Supply a consumer error type that implements `From<CatalogError>` and stage
changes inside the callback. Publish them after a successful visit with the
expected availability, owner count and coverage. A callback error, cancellation
or Archive change prevents a terminal summary. A readable owner with partial
fields is available to the callback with `fields_complete` set to `false` and
its Record index in terminal `incomplete_fields`.

Contract Template and Mission Broker Entry definitions expose the physical header,
the original source arena and links to that owner's flow and property declarations.
Read those linked declarations with `mission_properties::with_mission_properties`.
The Broker arena also retains authored title keys, reward references, standing
requirements and spawn data. The catalog's `configurations.missionBrokerEntries`
table gives the separately interpreted Broker context. Use
`mission_broker_context::with_mission_broker_context` or its decoded-Entry and
session forms to read those rows. The query retains each original Broker owner
and returns its contextual row only when required context interpretation succeeds.
A reached unreadable Reward body remains an unreadable amount with its original
source evidence. The query binds the authored target's exact physical Record and
reports Reward read failures separately. Use `snapshot.owner()` to resolve
contextual links into the original source arena.

The snapshot's `ship_spawns`, `npc_spawns`, `entity_spawns` and `location_queries`
contain the distinct content rows from successfully projected Brokers in this selection.
They retain canonical content-ID order. A contextual read failure that prevents a Broker
row also excludes its pools. An unreadable reached Reward still permits the Broker row
and its pools, with the unreadable amount and Reward failure reported separately.
Pool NPC name references resolve through the same snapshot's protected text rows and
source inventory. Each list's length counts the rows after content deduplication.
The family availability describes declaration of the Broker source family, including
when no owner matches the selection.

Use `contract_template_context::with_contract_template_context` or its decoded-Entry
and session forms for `configurations.contractTemplates`. Select by physical Record
index, GUID, or the complete Template family. The context row's `template()` points
to the original owner returned by `snapshot.owner()`. Resolve its title, objective,
Tag and localization bindings against that owner's source arena.

The snapshot also returns the selected Templates' distinct `ship_spawns`, `npc_spawns`,
`entity_spawns` and `location_queries` in canonical content-ID order. These are the
content rows the same Templates contribute to `contractShipSpawns`, `contractNpcSpawns`,
`contractEntitySpawns` and `contractLocationQueries` in the Catalog. Matching content
from several selected owners appears once. Each list's length counts only the content
emitted for this selection. Family `availability` describes the presence of the Template
source declaration and can be available for an empty selection. The scope excludes
independent Generator and Broker declarations, including their distinct pool IDs.
NPC name references resolve in the same snapshot's protected `game_texts`, with its
own `source_inventory` and requested locale policy.

This query interprets the selected Templates and their reached Record dependencies.
It reads the requested available locales for admitted owners and returns the protected
text rows registered by those owners. An actual external Tag reference starts one
canonical Tag-domain ancestry survey. Local, null and dangling references preserve
their original states. Check selection omissions and partial reads in `coverage` and
the separate contextual and Tag failure lists. A readable owner can retain a partial
context row in the query and in a diagnostic Catalog.

Scenario rows expose the original field arena. Module rows expose exact module type, property,
location acquisition, spawn and variable declarations. The typed views preserve original slot
order and reference states. References use target header evidence and leave target bodies
for independent selection.

Mission items expose their original tags, entity reference, weighting and localized alternatives.
The query reads the requested available locales for an admitted mission-item selection. Use
`game_text` to resolve the protected references in that result. Scenario and module queries
read the selected DataForge Records and leave unrelated Archive payloads unopened.

To verify a physical mission-item query on an installed Archive, set `P4KX_DATA_P4K`
and use the configured Archive key. The test compares one selected original Record with
the filtered Catalog producer and the SDK, including the row's resolved source contributors.
It keeps the filtered output in memory.

```powershell
cargo test --locked -p p4kx-export --test real_archive -- `
    real_archive_mission_items::mission_item_definition_matches_original_variants_and_targets `
    --ignored --exact --test-threads=1
```

## Authored NPC spawn definitions

Use `npc_spawns::with_npc_spawn_definitions`, its decoded-Entry form, or a
parsed session's `with_npc_spawn_definitions` to query one family by GUID,
physical Record index or all admitted Records in that family.

| Family | Catalog dataset |
| --- | --- |
| `Archetypes` | `definitions.npcArchetypes` |
| `ClassPools` | `definitions.npcClassPools` |
| `Spawns` | `definitions.autoSpawns` |
| `Sets` | `definitions.autoSpawnSets` |
| `Encounters` | `definitions.encounterIndices` |

The snapshot returns canonical typed rows, Build when available, physical
Record coverage and the source inventory. Rows sort by GUID then physical
index. Authored arrays preserve duplicate occurrences, order and null slots.
Use each sourced value's origin and the returned inventory to trace the original
body, linked Record header and encryption independently.

The query reads selected bodies and the headers needed to describe their
references. Query a referenced family separately to read its authored body.
It opens no locale or world placement inputs and leaves runtime NPC selection,
conditions and spawning to the game.

On Windows AMD64 all five standalone families returned Absent on LIVE
`13220821`. On PTU `13260598`, they returned 27 archetypes, 314 class pools,
44 auto-spawn configurations, 37 sets and 105 encounter indices, all Available.
The ten separate processes completed with stable Build and Archive stamps.
The largest private commit was 1,115,009,024 bytes and the largest working set
was 1,032,044,544 bytes. These reads used the verifier at
`eed6c4b45f7faf4ce325c3a99f6f97281a33b572`.

The same verifier matched all five tables against the complete Normal
Catalog for each Build. It checked availability, ordered rows, and the
referenced source uses and physical sources. On LIVE, all tables were
Absent and referenced zero sources. On PTU, the rows referenced 25,103
uses from one physical source. Both runs reported zero ignored development
leftovers. The largest private commit was 1,115,025,408 bytes and the
largest working set was 1,043,886,080 bytes.

The Normal Catalog producer revisions were
`23addc5e164f65a28a6f258302b30453b331edce` on LIVE and
`93d92ca20bef8a8c3731339214a3301124ade7ba` on PTU. The verifier checked the
Build and Archive stamps before and after each comparison.

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

## Law and security definitions

Use `catalog_queries::law::with_law` or `with_law_from` to read the canonical
`definitions.infractions`, `definitions.infractionSets`, `definitions.jurisdictions` and
`definitions.securityNetworkManifests` rows. The query session and parsed view expose
`with_law` to reuse the admitted DataForge. Choose one family or `LawFamily::All`, then
select all matching Records, one GUID with its aliases, or a physical Record index.

The result contains the requested tables in canonical GUID and physical Record order,
plus original coverage and a source inventory. A requested family with no matching
rows still has an availability state. Unrequested tables are absent from the result.
Infraction sets retain repeated uses, overrides and unresolved links. Jurisdictions
retain their original parent and possession rules. Security manifests keep protocol,
hostility and variable-effect declarations. A reference records the original target
resolution and does not cause the target body to be read.

Requested locale inputs supply public names and protected descriptions, warnings and
impounding names referenced by admitted rows. A security-manifest-only query or one
with no admitted rows opens no locale payload. Query the corresponding target family
separately when you need facts from a referenced definition. The returned records
describe authored data and do not evaluate in-game enforcement or jurisdiction inheritance.

## Reputation

Use `with_reputation`, `with_reputation_from` or the parsed session's `with_reputation` to
read `definitions.factionReputations`, `definitions.reputationScopes`,
`definitions.reputationStandings`, `definitions.reputationContexts`, `definitions.reputationPerks`
and `definitions.reputationTriggerRecords`. Each requested family supports `All`, `Guid` and
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

The reputation rows serialize with the original values and field-level source evidence. Use
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
remain available through the borrowed values.

Use `ReputationSelection::default()` and set `contexts`, `perks` or `trigger_records`
to select one of those families independently. Contexts and perk lists can retain protected
text references. A standalone trigger-record request opens no localization payloads.
Read the primary scope with `primary_scope_context()` and the other authored
scopes with `scope_context_list()`. Each scope selection exposes its physical
link through `scope()`. Use `ReputationPerk::details()` for the authored
reward item and category selectors. Perk entries retain their original scope
and standing links, and each trigger retains its ordered manual slots. The linked Records can
be queried separately by their original physical Record indices.

Use `with_reputation_reward_amounts`, `with_reputation_reward_amounts_from` or the session
method to read `definitions.reputationRewardAmounts` without opening localization payloads.
`All` traverses the admitted family in GUID and physical Record order. `Guid` retains every
admitted physical occurrence of the same GUID. `RecordIndex` selects one physical owner in the
current DataForge. The snapshot reports source-family availability separately from its matching
rows and selection coverage.

Each `ReputationRewardAmountDefinition` exposes its original header, `editor_name()`, signed
`reputation_amount()` and additional authored fields through `authored_fields()`. Resolve its
per-value source-use references in the snapshot's `source_inventory`. This lookup supplies
the authored reward amount for a resolved reference. Contract reward computation has its own
source operands.

Use `with_reputation_triggers`, its decoded-Entry form or the session method to read
`configurations.reputationTriggers` from the physical faction reputation Records. An
unlocalized request supports `All`, `Guid` and `RecordIndex` owner selection. The ordered
slots retain null pointers, record links, authored variants and their original source
evidence. The `trigger_records` selection in `with_reputation` reads the reusable targets.
Faction relationships are part of `definitions.factionReputations` through
`FactionReputationDefinition::relations()`.

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

Use `with_contract_generator_facets` for `configurations.localContractGenerators` and
`declarations.contractScenarioRequirements`. The unlocalized query reads the selected
Generator owners and preserves the original ordered Handler settings and scenario
reference arrays. Join each row to its Generator's raw arena by `input_ref()` and
`record_index()`. The `handler()`, `owner()` and `scenarios_value()` links identify
occurrences within that arena. Retrieve the arena for that owner through
`with_contract_generators` when needed. Scenario references retain their original target
headers for independent lookup.

See [Contract queries](catalog-contract-queries.md) for callback lifetimes, parsed-session reuse,
global duplicate and intro semantics, complete original views and source-inventory ownership.

## Older loot Record queries

`CatalogQuerySession` and `ParsedCatalogQueries` expose `with_legacy_loot_tables` and
`with_legacy_loot_archetypes`. A typed snapshot returns the same physical rows as
`legacyLootTables` or `legacyLootArchetypes`, with source inventory and selection coverage.
Pass a physical Record index for one occurrence or a GUID for all admitted occurrences
sharing that identity. The family-specific type and Record path selection stays in effect.
Neither query opens linked target bodies, reads placement files, nor creates a Catalog.
Use each datum's `containsEncryptedSource` and resolve its source-use references through the
snapshot's inventory. The row's original arena is local to that physical owner.

## Original Loot definition queries

`with_loot_definitions` reads the selected definition kind by physical Record index,
GUID or family. Read `rows`, `availability`, `coverage` and the source inventory in the
same callback. A declared family with zero selected rows has availability `available`.
An undeclared family has availability `absent`.

Fourteen Windows AMD64 cold checks used a separate process for each family and
Build. All original Archive stamps stayed stable. Peak private commit across the
checks was 1,114,980,352 bytes and peak working set was 1,032,081,408 bytes.

| Definition | LIVE 13220821 | PTU 13260598 |
| --- | ---: | ---: |
| `lootTables` | 54 | 58 |
| `lootArchetypes` | 34 | 36 |
| `lootPoolFilters` | 15 | 14 |
| `lootSecondaryChoiceSingleLayers` | 10 | 11 |
| `lootSecondaryChoiceMultiLayers` | 15 | 0 available |
| `lootThemes` | 0 absent | 51 |
| `lootGenerationGlobalParams` | 1 | 1 |

The same verifier matched the seven ordered tables and their availability to
each completed Normal Catalog. It bound 30,858 contributing uses on LIVE and
38,965 on PTU, each with one original physical source. The binary SHA-256 was
`8779adcee241c5c1ad8d98d6880a0879b306276a48ae8d19e6824b7932610152`.

| Build | Normal producer | Elapsed seconds | Peak private commit (bytes) | Peak working set (bytes) |
| --- | --- | ---: | ---: | ---: |
| LIVE `13220821` | `23addc5e164f65a28a6f258302b30453b331edce` | 3,846.365 | 1,145,274,368 | 1,151,479,808 |
| PTU `13260598` | `93d92ca20bef8a8c3731339214a3301124ade7ba` | 3,972.778 | 1,216,712,704 | 1,221,267,456 |

## Harvestable slot-preset queries

Use `catalog_queries::loot::with_loot_definitions` with
`LootDefinitionKind::SubHarvestableMultiConfig` to read by physical Record
index, GUID or family. The snapshot's `rows` match the original definitions
under `definitions.subHarvestableMultiConfigs`. Its
`sub_harvestable_multi_configs` contains the Catalog's slot-preset
projection in the same order. A source outside the slot-preset selection
can return its original definition with no projected row.

The convenience rows follow the selected harvestable presets and Entities.
`harvest_dependency_warning` carries failed dependency reads from that
projection. Use the original definitions for authored values, reference
dispositions and field-level provenance.

### Installed query memory

On Windows AMD64, a separate fresh process queried all admitted slot presets for each
Build using one Rayon worker and a clean profile. The harness at source revision
`fb86aff58bd10574cb9429acf016cd66399511e4` checked Build identity, available projection,
complete selected headers, bodies and fields, and successful dependency reads.

| Build | Projected rows | Peak private commit (bytes) | Peak working set (bytes) | Elapsed (s) |
| --- | ---: | ---: | ---: | ---: |
| LIVE 13220821 | 104 | 1,091,567,616 | 1,156,431,872 | 19.399 |
| PTU 13248297 | 106 | 1,115,308,032 | 1,077,690,368 | 21.151 |
| PTU 13260598 | 106 | 1,114,275,840 | 1,072,050,176 | 17.566 |

Each cold query stayed below the 2 GB ceiling and wrote zero scratch bytes.
Star Citizen was running during the measurements, and the operating-system
file cache stayed in normal use.

The ordered replay for LIVE 13220821 matched all 104 rows in the complete
Normal Catalog from producer `300226ceeeaf522b7d9af790e8fa3698752cf575`.
The combined SDK query and replay took 730.014 s, with peaks of 1,091,502,080
bytes of private commit and 1,023,156,224 bytes of working set. The same
producer's complete PTU 13260598 Catalog matched all 106 rows in order.
The SDK query and replay took 642.495 s, with 1,114,333,184 bytes of private
commit and 1,059,139,584 bytes of working set. Both replay workers wrote
zero scratch bytes.

## Loot container row scope

`catalog_queries::loot::read_loot_container` and the session methods read one
canonical `/lootContainers` key. Pass the physical system Archive Entry
index, the authored multi-config GUID, the exact canonical container path
and either a physical Starmap Record index or the null location key. The
optional Catalog filter and placement budget apply to the original source walk.

The query uses the writer's order for physical seeds and reached placements.
The occurrence identifies the system, seed, outer Archive Entry and nested
EntXML Entry that contributed the row. The source inventory also carries
placement and link survey inputs and the references for failed reads.

`Found` identifies a row with complete competing placement and owner evidence.
`NotFound` requires complete coverage of the relevant selection.
`Ambiguous` identifies competing admitted physical Records for the requested
Starmap identity. `Unavailable` can carry an observed row when coverage is
incomplete. A result stays scoped to the requested key.

A focused Normal producer compiled from `c7a033bd5` nominated one original
container occurrence per installed Build. A fresh SDK process matched the whole
row, the original EntXML coordinates and the resolved source-use closure.

| Build | Rows | Peak private commit (bytes) | Peak working set (bytes) | Seconds |
| --- | ---: | ---: | ---: | ---: |
| LIVE 13220821 | 1 | 1,095,925,760 | 1,084,387,328 | 18.812 |
| PTU 13260598 | 1 | 1,479,503,872 | 1,463,521,280 | 27.827 |

The timings and peaks are from isolated Windows debug SDK processes.
Full installed scope enumeration and final package verification remain open.

## Loot container-document source scope

Catalog v3 `declarations.lootEntityContainerDocuments` retains original attribute sites only from
EntXML documents actually reached by the existing Loot container walk. It is not a whole-Archive
Entity inventory. The source strategy is the physical containing Entry plus nested Entry and local
node/attribute ordinals; repeated placement roots and equal GUIDs do not replace that identity.
The normalized nodes/configuration references and exact lookup candidates are documented in
[the Loot source contract](catalog-loot-source.md#original-container-link-documents-reached-by-the-loot-walk).
Use `catalog_queries::loot::read_loot_documents` or the corresponding Archive session or parsed
view. `LootDocumentSelection` permits the complete selected scope, a physical containing Entry,
or an exact nested Entry under that containing Entry. Equal paths do not identify an occurrence.
By default the query applies the normal Catalog placement admission. Supply `catalog_filter` to
match a filtered Catalog's original Record scope. The result includes the exact admission Record
when one activates the Loot scope.

The query reads the shared placement descriptors, system-folder fallback sources and original
EntXML reachability. A positively reached exact owner can be read even when another route is
unknown. A missing row under incomplete coverage has unavailable status. The diagnostic Catalog
can still report the documents it observed under partial reachability. Rows use physical Entry
order. The query opens the DataForge Record header index and skips Record bodies, mission and
reward projectors, geometry, location physics grids and SOC tag counts. The complete selection stores the returned typed rows in
memory. Exact selection stops the Loot search as soon as its containing physical Entry is admitted.

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

Four Windows AMD64 cold checks ran the joint definition and membership query and
the separate parent query in fresh processes on LIVE 13220821 and PTU 13260598.
Each Build returned 19 definitions, 19 membership rows and 19 parent rows with
complete available coverage. The two direct tables shared one query per Build.

The peak private commit was 1,114,550,272 bytes and the peak working set was
1,031,684,096 bytes across all four processes. The Archive and Build stamps
remained stable. The exact test executable SHA-256 was
`6be54c14baffb1f42b44ac22530c0f1890d444b776860ceccb57eaa320b1adf0`.

## Original equipment definitions and configurations

Use `catalog_queries::equipment::with_item_definitions`, `with_attachment_constraints` or
`with_item_bounds` for the corresponding canonical datasets. Each has a decoded-Entry `_from`
form and the same-named session/parsed-view method. Select the dataset, all physical declarations
with a GUID, or a physical Record index. Physical indices are local to the supplied DataForge.

Use `catalog_queries::equipment::visit_item_definitions`, `visit_item_bounds` or
`visit_attachment_constraints` to process one readable physical Record at a time. Each has
a decoded-Entry `_from` form and a session or parsed-view method. Rows and their original
source inventory are scoped to the current owner. The terminal summary reports availability,
owner and row counts, and selection-wide coverage. Commit consumer-owned output only after
a successful traversal whose coverage meets the consumer's requirements.

Use `ItemDefinitionSelection::RecordIndices(&indices)` to select the listed physical Item
owners in one visit. Collect present `target_record_index()` values from the resolved ports'
`source_constraints().attachment()` results. Both queries must use the same DataForge.
Duplicate indices are visited once in ascending order. The Item definition callback receives
the physical index even when it has no projected row. A readable Record of another kind is
outside the selection. The summary reports absent indices through `missing_record_indices()`.
Use `ItemDefinitionSelection::Records(CatalogRecordSelection::All)` to visit all Item definitions.
The same wrapper accepts selection by one GUID or physical Record index.

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
The definition query reports coverage for the full Item definition. Use `equipment::with_items`
or `with_items_from` for the canonical Item row and its original variant decision. The session
and parsed-view methods provide the same selection and result shape. Read `coverage()` for
owner-level gaps and use `provenance()` for the query's inventory and shared variant use sets.
See
[ADR-0410](adr/0410-equipment-sdk-shares-original-family-readers-and-canonical-variant-choices.md).

An unclassifiable original component slot or unreadable possible occupancy definition keeps the
queried source scope incomplete. Readable prefix rows remain available; `coverage.incomplete_fields`
identifies the affected physical Record. An explicit null target and a proven foreign declaration
remain separate known exclusions. No unrelated configuration projector runs to manufacture that
coverage result.

## Direct inventory container definitions

Use `catalog_queries::equipment::with_inventory_containers` for the physical rows under
`definitions.inventoryContainers`. The `_from` entry point accepts the already decoded Entry
from this Archive. The query session and parsed view also expose `with_inventory_containers`.
Select all records, every physical owner of a GUID, or one physical Record index in this DataForge.
The result retains the Catalog's GUID order and physical tie order, including duplicate GUIDs.

The query supplies original interior dimensions, ordered item-type restrictions, concrete open
or closed storage policies and additional authored fields. Check family availability and the
selected header, body and field coverage independently. An empty result may follow a valid
selection even when the source family is available. Use the snapshot's own inventory to decode
field origins. This query opens the original DataForge with no locale, loadout or placement reads.
For resolved Cargo Grid capacity and its associated storage component, use `with_cargo_grids`.

## Original medical item tier definitions

Use `catalog_queries::medical::with_medical_item_tier_configs` for
`/definitions/medicalItemTierConfigs`. `CatalogQuerySession` and
`ParsedCatalogQueries` expose the same query and can reuse the decoded DataForge.
Select every original definition, all physical Records with a GUID, or one
`CatalogRecordSelection::RecordIndex`. The snapshot supplies Build identity,
availability, physical coverage and the source inventory for its values.

The rows use the same projector and order as the Catalog. They retain the
ordered bed tiers and drug declarations, original resource references and
amounts in their concrete units, plus additional authored fields. The query
reads the selected family directly from DataForge. Facilities, placements, referenced
resource definitions and localization have their own source scopes.

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

## Installed Item gameplay component verification

Each of these nine `/configurations` datasets ran in its own fresh Windows AMD64
process with `CatalogRecordSelection::All` on LIVE `13220821` and PTU `13260598`.
The standalone queries used only the installed Archive. All 18 runs returned
complete available coverage in physical Record and component order.

| Dataset | LIVE rows | PTU rows |
| --- | ---: | ---: |
| `attachableModifiers` | 101 | 101 |
| `clothing` | 4,445 | 5,124 |
| `consumables` | 308 | 308 |
| `hackableDevices` | 14 | 14 |
| `hackingChips` | 38 | 38 |
| `itemRadars` | 110 | 110 |
| `miningLasers` | 52 | 52 |
| `quantumInterdiction` | 6 | 6 |
| `suitArmor` | 2,405 | 2,782 |

One retained comparison per Build verified all nine datasets in a single stream of
the corresponding complete Normal Catalog. Each comparison checked availability,
ordered physical Record and component identities, full serialized row digests, and
the transitive source-use and physical-source closure of those rows. LIVE matched
1,846,727 contributing source uses and PTU matched 2,054,471. Each closure reached
one physical source. The measured scope is the canonical component selection.
The hacking-chip and quantum-interdiction queries each reported 1,160 skipped
leftover candidates on LIVE and 1,172 on PTU.

| Build and mode | Processes | Peak private commit bytes | Peak working set bytes |
| --- | ---: | ---: | ---: |
| LIVE standalone cold | 9 | 1,443,868,672 | 1,419,403,264 |
| PTU standalone cold | 9 | 1,522,737,152 | 1,499,852,800 |
| LIVE retained comparison | 1 | 1,792,245,760 | 1,783,627,776 |
| PTU retained comparison | 1 | 1,884,553,216 | 1,875,632,128 |

All processes stayed below 2,000,000,000 bytes in both native peak counters
and retained matching initial and final Archive and Build stamps. The
`installed_item_gameplay_cold_query` and
`installed_item_gameplay_tables_match_one_complete_catalog` witnesses were
built from `2e31f098fb85a7c70e2180d44b014080545b3517`. The executable SHA-256 was
`6e5389edf2b628292e9f6ce1fc7db1ffab52528d63ab5eac32bea7b7f7345e29`.
The retained Normal producer revisions were
`23addc5e164f65a28a6f258302b30453b331edce` for LIVE and
`93d92ca20bef8a8c3731339214a3301124ade7ba` for PTU.

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

## Mission boards

Use `equipment::mission_boards::with_mission_boards` or its decoded-Entry and session
forms to read original board providers in `configurations.missionBoards`. Select a
physical Entity by Record index, all physical matches of its GUID, or all providers.
The query selects the original provider component even when its Entity is outside
the items and ships datasets.

Each row retains its physical owner and original component ordinal. The sourced
board type exposes Base, Career, Notice, and other authored variants. Career rows
retain the guild reference and the entire ordered faction list, including null,
duplicated, and dangling slots. The snapshot supplies immediate source uses for
original values and resolved target headers. The query reads the selected owners
that carry a provider component and uses the original header for referenced factions.

### Installed mission board and contract scope queries

The independent `location_vocabulary::with_contract_scopes` query reads
`definitions.contractScopes` by original Record selection. On Windows AMD64,
both queries ran separately in fresh processes and matched their Build's
complete Normal Catalog for availability, ordered rows and source provenance.

| Build | Contract scopes | Mission boards | Maximum cold private commit (bytes) | Retained private commit (bytes) |
| --- | ---: | ---: | ---: | ---: |
| LIVE `13220821` | Absent | Absent | 1,091,493,888 | 1,091,588,096 |
| PTU `13260598` | 1 | 24 | 1,114,501,120 | 1,114,578,944 |

The verifier `fdbebda92b998a21a4b61f0c5bfa6ca2b7103701` checked the declared
family state even when it emitted zero rows. The Normal producer revisions were
`23addc5e164f65a28a6f258302b30453b331edce` and
`93d92ca20bef8a8c3731339214a3301124ade7ba`, respectively.

## Loot-generation components

Use `catalog_queries::equipment::loot_generation::with_loot_generation_components` for
`configurations.lootGeneration`. The decoded-Entry `_from` form, Archive session and parsed
view accept all matching owners, a GUID including physical aliases, or one physical
Record index. Compare the original component ordinal within each owner to identify an
occurrence. Owners outside the item and ship families are eligible.

The snapshot supplies the same ordered authored-field arena as the Catalog, along with
source inventory and coverage. Resolve arena-local links within their own row and
provenance references through this snapshot. Authored theme and secondary references keep
their physical selectors for a separate definition query. The query projects the selected
owner's configuration without reading world containers or computing runtime loot outcomes.
See [Loot source definitions](catalog-loot-source.md).

## Item recovery

`catalog_queries::item_recovery::with_item_recovery` supplies the canonical
economy, override and policy projections from one physical
`ItemRecoveryConfigurationParams` Record. Selection admits records by the same
type and entitlement-policy path rules as the catalog. The original path and
then physical index order admitted readable candidates. The first candidate
supplies all three projections. Use an exact Record index to inspect that
policy independently of other candidates.

A snapshot includes the selected original fields, ordered condition and
override slots, protected notification text and a common source inventory.
Record links retain header evidence. The query reads the admitted policy
bodies and requested locales and reports unreadable candidates through
`coverage`. Copy any values needed beyond the callback, and publish changes
after the query succeeds.

### Installed Item Recovery verification

The Windows AMD64 verifier at `7e54dec59bc0399fc5c5242147f6c46f49823f27` read the complete
Item Recovery scope with all available locales in a fresh process for each Build. Both returned
one economy, override and policy row for the same physical Record, with ten notification texts.

| Build | Elapsed ms | Peak private bytes | Peak working set bytes |
| --- | ---: | ---: | ---: |
| LIVE `13220821` | 8,046 | 1,091,620,864 | 1,021,710,336 |
| PTU `13260598` | 8,270 | 1,114,910,720 | 1,031,933,952 |

`installed_item_recovery_matches_one_complete_catalog` compared those three complete tables,
the selected notification texts and their contributing source identities against the matching
retained Normal Catalog from producer `24c08bcbff2299b88eff84bf3f7e8f592760b99b`.
The LIVE and PTU comparisons matched 1,016 and 1,020 contributing source uses respectively,
each across twelve physical sources. Their separate replay peaks were 1,634,037,760 and
1,662,300,160 bytes of private commit. Both Archive stamps and retained frame digests matched.

Set `P4KX_DATA_P4K` and `P4KX_NEW_SDK_BUILD` for `installed_item_recovery_cold_query`. The
parity check also needs `P4KX_CATALOG_MEASUREMENT_DIR` and
`P4KX_ITEM_RECOVERY_CATALOG_PRODUCER_REVISION` for the corresponding completed Normal run.

## Authored hacking chips and devices

Use `equipment::hacking::with_hacking_chips` for chip components on admitted
Item Records. Match the typed `HackingConfiguration` to inspect its charge
count or original ordered removable-chip modifiers.

Use `equipment::hacking::with_hackable_devices` for hackable components on
physical Entity Records. A device row retains its timing and error-check
settings, audio triggers and parameters, and additional authored members.
Both queries accept a physical Record index or GUID and preserve the original
Components ordinal and per-value provenance. Use an existing decoded Entry
or parsed query session to avoid reopening the DataForge Entry. These routes
require no locale inputs.

### Installed equipment component cold reads

The Windows AMD64 verifier at `2e31f098fb85a7c70e2180d44b014080545b3517`
ran each component query in its own process without opening a complete Catalog.
Each query selected all eligible owners for its table.

| Configuration | LIVE `13220821` | PTU `13260598` |
| --- | ---: | ---: |
| `attachableModifiers` | 101 | 101 |
| `clothing` | 4,445 | 5,124 |
| `consumables` | 308 | 308 |
| `hackableDevices` | 14 | 14 |
| `hackingChips` | 38 | 38 |
| `itemRadars` | 110 | 110 |
| `miningLasers` | 52 | 52 |
| `quantumInterdiction` | 6 | 6 |
| `suitArmor` | 2,405 | 2,782 |

All 18 calls returned Available and stable Build and Archive stamps. The
largest lifetime private commit was 1,522,737,152 bytes, and the largest
working set was 1,499,852,800 bytes, both on the PTU Clothing query.

## Authored item durability

Use `equipment::durability::with_durability`, `with_degradation` or
`with_distortion` for the three canonical item configuration tables. The
Archive, decoded-Entry and parsed-session forms accept a physical Record index,
GUID or all eligible Item owners. A GUID can select more than one physical
Record. Rows preserve each original Components slot, including repeated Health
components. The Health query supplies the same rows as the staged Catalog table.

These queries project the selected component family on eligible Items.
They require no localization or other Asset reads. The snapshot includes
source-family availability, read coverage and per-value origin evidence. Use
an exact physical index when the caller already knows which Item to inspect.

### Installed Item condition cold reads

On Windows AMD64, each query ran in its own process without opening a complete
Catalog. The tested processes read every eligible owner for their one table.

| Configuration | LIVE `13220821` | PTU `13260598` |
| --- | ---: | ---: |
| `durability` | 6,472 | 6,515 |
| `degradation` | 1,199 | 1,213 |
| `distortion` | 1,225 | 1,228 |
| `emp` | 7 | 7 |

All eight queries returned Available and stable Build and Archive stamps. The
largest lifetime private commit was 1,579,773,952 bytes and the largest working
set was 1,566,150,656 bytes. Both peaks occurred in the PTU durability query.
The verifier was built from `cae92b947dd0b133e0e35fadd4cec8904a47ffd3`.

The four tables also matched each Build's complete retained Normal Catalog
in one Windows AMD64 process. The comparison verified ordered rows, availability
and original contributors. The canonical exclusions were 1,160 on LIVE and 1,172
on PTU and were accounted for before row comparison.

| Build | Normal producer revision | Peak private commit (bytes) | Peak working set (bytes) |
| --- | --- | ---: | ---: |
| LIVE `13220821` | `23addc5e164f65a28a6f258302b30453b331edce` | 1,598,033,920 | 1,577,988,096 |
| PTU `13260598` | `93d92ca20bef8a8c3731339214a3301124ade7ba` | 1,631,551,488 | 1,607,401,472 |

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

The original-source witness at `0b64a4a3c` checked one physical occurrence and 19 nested
values per Build on LIVE `13165946` and PTU `13228337`.

For complete table parity, set `P4KX_DATA_P4K` and `P4KX_CATALOG_MEASUREMENT_DIR` to the
matching installed Archive and completed Normal measurement. Set
`P4KX_VEHICLE_ARMOR_CATALOG_PRODUCER_REVISION` to the measurement's exact producer revision.
This test reads every typed Vehicle Armor row and compares original component positions, row
order, availability and source uses with `/configurations/vehicleArmor` in the retained artifact.

```powershell
cargo test --locked -p p4kx-export --test real_archive -- `
    real_archive_vehicle_armor::parity::installed_vehicle_armor_matches_retained_complete_catalog `
    --ignored --exact --nocapture --test-threads=1
```

Cold direct-query checks on LIVE `13220821` and PTU `13260598` each
read all 205 physical occurrences with complete typed origins, 16,824
source uses and two physical sources. Both finish in a fresh process
without temporary output. All rows also matched the complete LIVE
Normal Catalog in the [combined replay](#installed-flight-configurations).

| Build | Wall time (s) | Peak private commit (B) | Peak working set (B) |
| --- | ---: | ---: | ---: |
| LIVE `13220821` | 8.356 | 1,091,776,512 | 1,022,832,640 |
| PTU `13260598` | 8.274 | 1,114,603,520 | 1,032,642,560 |

Set `P4KX_DATA_P4K` and `P4KX_NEW_SDK_BUILD` for the installed Build
and measure the ignored test in its own optimized process.

```text
cargo test --locked -p p4kx-export --test real_archive real_archive_scoped_equipment_sdk::vehicle_armor_cold::installed_vehicle_armor_full_cold_query -- --ignored --exact --test-threads=1
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

## Original dismantle process definitions

Use `crafting::with_dismantle_processes`, its decoded-Entry variant or a parsed
session to read `definitions.dismantleProcesses` by GUID, physical Record index,
or for the dataset. The query checks the process subtype inside each admitted
Blueprint Record and retains its original owned fields and remaining authored
members. Each duration component retains its numeric representation and source
origin. The derived seconds cite the components used by the shared conversion.

This query uses the selected process Records, with original referenced target
headers where applicable. The snapshot's Record coverage distinguishes unreadable
bodies or incomplete fields in that selection.

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
The same physical selection includes the supported ATLS Actor owners. Their sourced vehicle kind and
playability retain the original control and driver-seat selectors. For those exosuits,
`name()` uses the selected attachment's `AttachDef.Localization.Name` when the original
vehicle name is absent. `attachment_manufacturer()` preserves the authored reference and
its resolution state. Vehicle-only inputs keep their sourced absent or unreadable states.
The [installed LIVE and PTU comparisons](catalog.md#ship-base-data) cover all nine
supported ATLS owners, one physical Record at a time.

This query shares the Catalog's original reader, Archive-backed ShipPool and per-call
Capture. It uses the Entity header index to open the dependencies reached by the selected owners'
Ship, loadout and cargo resolvers. An all-owner query prepares the original Record bodies
in groups of up to 32 and retains the selected result rows for the callback. An empty
physical selection returns an empty source inventory before locale and Entity index setup.
Use the result's text resolvers and source inventory to resolve original keys and text identities
in the requested languages.

Use the ignored `installed_ship_sources_marker_timing` test for a fresh-process
measurement of the all-owner source query. Set the installed build, the
independently verified row count in `P4KX_SHIP_SOURCE_SDK_ROWS`, and the
consumer's comma-separated locales in `P4KX_SHIP_SOURCE_SDK_LANGUAGES`.
Record private-commit and working-set peaks in the same process.

Check `coverage()` for owner selection/read/field gaps and `dependency_coverage()` for actual
failed Record/Entry operations, incomplete source coordinates and directory scope. Known absent,
unreadable and available-but-incomplete cells remain distinct. Preserve the three-valued
encryption marker independently of source completeness, and resolve every use against this
result's own `source_inventory()`. Return consumer-owned values from the callback, and replace
persistent state only after the outer operation succeeds and the required coverage is accepted.

The sourced `playability()` getter retains the canonical classification and the original
rule inputs, including linked crew or tag results and their physical Record lookup. A computed
classification can carry incomplete source coverage. A failed linked Record read that blocks
the decision yields `unreadable`. Use `equipment::with_ships`, `with_ships_from` or the session
and parsed-view methods for canonical Ships including variant decisions. Each selected
`ShipSource` exposes its canonical `catalog_row()` and `variant_of()`. The snapshot provides
original field coverage through `sources()` and shared variant evidence through `provenance()`.
See
[the Ship contract](catalog.md#ship-dcb-source-fields) and
[ADR-0410](adr/0410-equipment-sdk-shares-original-family-readers-and-canonical-variant-choices.md).

On PTU `13260598`, each of the four ATLS roles resolved to one exact original
physical Record header. The isolated canonical Ship SDK query and scoped
Normal producer agreed on each playable exosuit row, its local variant
decision and the recursive contributing sources. Each of the four cold
queries ran in its own process. The largest peak private commit was
1,114,398,720 bytes for a cold query and 1,409,536,000 bytes for a scoped
Normal comparison. Admission uses the original evidence in
[ADR-0417](adr/0417-exosuits-share-the-vehicle-catalog.md).

The unfiltered canonical Ship query on PTU `13260598` checked 1,036
rows. The complete original directory proved that three selected
vehicle patches were absent. These owners retained sourced absence for
`variantOf`, and their comparison cohorts retained complete variant
evidence. The query reported zero unreadable variant decisions across
the roster. LIVE `13220821` also reported zero unreadable variant decisions
across its 1,028 canonical Ship rows.

`ShipSource::base_facts()` returns the canonical typed row published in
`projections.shipBaseFacts`. It carries the physical Ship Record index, GUID and cargo
facts with their sources in the query snapshot's source inventory. The `cargo()` getter
returns the same row's SCU total, issue count and mount evidence.
Each reached mount keeps its resolved port path, selected physical grid Record, original
storage reference, first physical inventory Record and capacity inputs. Separate mounts of
the same grid contribute separately. With known capacity on only some reached mounts, the
reported sum is a lower bound. If no reached mount has readable capacity, the sum is
unreadable. A Loadout without reached cargo grids retains the canonical zero. Check
`has_unresolved_loadout()` for unresolved or undecided branches that can leave other grids
unaccounted for. A physical mount's `position()` keeps the original Loadout Record index
and nested port ordinals. The aggregate origin lists observed contributors and has incomplete
coverage of exclusion decisions. This state is independent of the result's owner and
dependency coverage and the marker can still be true when a known contributor is encrypted.

`with_ship_store_slugs` and `with_ship_store_slugs_from` supply the same curated
`shipStoreSlugs` rows as the Catalog. Session and parsed views provide the same selection.

The optional `Filter` selects original Ship paths using Catalog matching. Unfiltered
queries include retired mappings whose original Ship GUID is absent from the admitted
Ship rows. Filtered queries publish only active mappings to selected, readable Ships.
The snapshot retains the exact Filter text, curated counts and read coverage of demanded
physical Ship candidates. Check that coverage before treating a missing row as absence.
The query resolves candidates for curated GUIDs, including all admitted physical aliases.

A scoped normal Catalog comparison matched the selected physical Ship, Store
rows and curated summary on installed LIVE `13220821` and PTU `13248297`.
The literal Record path was anchored and its full physical cohort was
checked before generation. Complete unfiltered Catalog parity remains separate
from the cold unfiltered API measurements below.

On installed LIVE `13220821`, the RSI binding refresh matched exact scoped Normal
and SDK rows for Liberator (224), Galaxy (244), both Carrack Expedition Store
identities (205 and 206) and ATLS IKTI Akuma (327). The largest combined Normal
and SDK process peaked at 4,315,688,960 private bytes. The separate cold,
unfiltered Store SDK query returned 227 rows in 63.102 s with 1,572,540,416
peak private bytes and 1,560,834,048 peak working-set bytes.

`catalog_queries::equipment::ship_store_pledges()` loads the bundled `ShipStorePledges`
section shared with Catalog v3. Call `snapshot()` for the source, date, revision and reviewed
candidate scope, `rows()` for all bundled rows in ascending RSI ID order, or
`by_rsi_id(id)` for an exact lookup. The function reads embedded metadata without
an Archive, Filter, session or network. An absent ID has no row in this reviewed snapshot.
Use Archive-backed Ship queries for physical Record identity and structural `playability`.

Installed variant checks at revision `9e2ecedea6b60d0b45a98dc5323a7d592dcabaef`
covered a selected physical Item and an ordinary Ship on each of LIVE `13220821`
and PTU `13248297`. Each cold SDK query returned one row with a present variant
target. The scoped Normal Catalog matched the full row and its source-use closure.
Across the eight separate processes, private commit peaked at 1,421,991,936 bytes.
The highest cold-query peak was 1,115,369,472 bytes.

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

### Installed cargo projection

The physical Ship cargo witness compares the selected SDK row with the normal
scoped Catalog, original Loadout and original capacity operands. Its original
path filter is checked against the entire physical path cohort before producing
the file shape. The comparison then selects the nominated physical Record.
Aliases at the same path retain their distinct identities.

Set `P4KX_DATA_P4K` and `P4KX_CARGO_SDK_BUILD` for the installed
Archive and its change number. Set `P4KX_CARGO_SDK_RECORD` to a physical
Record index or unique literal `class:<name>`. Set `P4KX_CARGO_SDK_RECEIPT`
to a new absolute path within the configured temporary directory.

```powershell
cargo test --locked -p p4kx-export --test real_archive -- `
  real_archive_cargo_grid_capacity::sdk::installed_ship_cargo_matches_scoped_producer_and_original_loadout `
  --ignored --exact
```

Run `installed_ship_cargo_sdk_cold_query_replays_one_verified_root` in the same
test module in a separate process with that verified receipt. This path
opens the same Archive and reads one physical Ship through the SDK. It does
not generate a Catalog.

On Windows AMD64, the native Debug test against production code `170144907`
confirmed the scoped producer and original Loadout comparison in LIVE
`13220821` and PTU `13248297`. This revision includes original CargoGrid
admission inputs and consumed port comparisons. A new process with no SDK
cache measured these peaks for the same selected rows.

| Build | Peak private bytes | Peak working set bytes | Elapsed |
| --- | ---: | ---: | ---: |
| LIVE `13220821` | 1,091,833,856 | 1,024,466,944 | 10.777 s |
| PTU `13248297` | 1,116,164,096 | 1,034,682,368 | 10.397 s |

Each cold process wrote 304 bytes. Available host memory stayed above
61.19 GB during the two SDK processes.

The observed page-file counters were stable across both SDK processes.
On a 16 GiB machine the higher process peak leaves an estimated 16.06 GB
for the OS and other applications. The test covers one selected Ship per
Build. Full-catalog parity and the integrated consumer's memory remain
separate acceptance checks.

## Resource topology placements

Use `equipment::resource_topology::read_resource_topology_placements` or its
`_from`, session and parsed variants. Give the physical Ship Record index in
the queried DataForge. The result contains the resolved root `loadout`, the
canonical `resourceNetworkNodes`, `resourceNetworkLinkBindings` and
`roomAssociationBindings` tables, and their shared source inventory.

Use `loadoutRecordIndex` to identify the root. Follow `portOrdinals` through
that root's resolved ports. The source inventory resolves the immediate uses
of endpoint selectors, candidates, resource components and vehicle boundaries.
For full original declaration bodies, use the existing resource-link and room
declaration queries by physical Record index.

Check `root_state`, `coverage`, `loadout_dependencies` and
`declaration_dependencies` before replacing a cached result. Table availability
describes original declaration types. A resolved Ship can have zero rows, and
failed or incomplete dependency reads stay visible independently of row counts.
The query checks structures and only resolves the selected root and its reached
configuration roles. Docked ships can be queried independently. The host keeps
the docking occurrence and any attached resource components it uses.

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

`source_item_guid()` and `source_item_class_name()` borrow the immediate
`itemGuid` and `itemClassName` cells. A resolved class name comes from the
selected Record header. A dangling Reference can still supply its GUID, and an
unresolved class lookup can supply its original name. An unreadable target
keeps the unreadable state. Empty slots keep the absent state. Their origin
remains incomplete when resolution has no observation for the empty selector.

`source_visibility()` borrows the resolved `visibility` cell. The selected
vehicle flags take precedence over the DataForge flags. When the resolver has
no vehicle winner, it retains the available fallback flag evidence and keeps
the selection incomplete.

`source_types()` borrows the ordered `types` cell. Each accepted item type
has separate `type` and `subTypes` cells for the original DataForge
fields or effective vehicle XML attributes. Each DataForge subtype cites
its original array slot. Subtypes split from one XML attribute share that
attribute's conversion evidence. The XML collection remains
incomplete when the capture cannot prove the entire membership survey.
The value getters `item_guid()`, `item_class_name()`, `visibility()`,
`types()`, `item_type()` and `sub_types()` keep their existing signatures.

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

### Visit Loadout owners incrementally

Use `equipment::visit_loadouts`, `equipment::visit_loadouts_from`, or a session or
parsed view's `visit_loadouts` method for a large owner selection. The visitor
follows the canonical GUID order and preserves distinct physical Records and
stable ties. It receives one admitted owner, the canonical row when resolution
emits one, and the original source inventory and dependency diagnostics for that
owner. The scan reuses the entity and XML directory indexes. Visits of multiple
owners also reuse completed canonical Entity trees and decoded XML payloads
from the same inputs. The per-visit cache keeps up to 2,048 Record trees within
a 32 MiB estimated retained-tree budget and 256 XML payloads within 8 MiB of
decoded byte capacity. All-owner and batch visits can retain the admitted Item
trees already decoded during selection in the same tree cache. Seeding fills
spare capacity. Resolver reads can still evict trees and reread them. Each owner
receives its own original source observations, file dependencies and alignment
coverage. The visit releases the owner's resolved row and source
inventory before resolving the next owner.

The original visitor accepts the same all-owner, GUID and physical Record selections as
`read_loadouts`. Use `visit_loadout_owners(LoadoutOwnerSelection::Ships, ...)` for Ship roots,
including their resolved default equipment and nested Item ports. This selection skips standalone
Item root admission. Use `RecordIndices(&indices)` to resolve a batch of physical owners in a
single visit. Duplicate indices are resolved once and the canonical GUID order and physical ties
are preserved. Check `missing_record_indices` for absent batch indices before committing cache
changes. The new visitor is available on both session views and as `equipment::visit_loadout_owners`
or `visit_loadout_owners_from`. The ordinary Catalog selection is available through `Records`. A Ship can have an empty row. An admitted Item that resolves to
no ports carries its identity and succeeds without a row. An admitted owner
with a failed resolution can supply diagnostics without a row. Docked vehicles
retain their root occurrence and can be visited independently.

Check `coverage` and `dependencies_complete` on the terminal summary before
committing a staged consumer result. Visitor errors and cancellation stop later
callbacks. The session checks the bound Archive again before returning success.

For a timing run with the default Marker mode, use the ignored
`installed_loadout_visitor_marker_timing` test in a fresh release-profile
process. Set `P4KX_DATA_P4K`, the expected Build in `P4KX_NEW_SDK_BUILD`
and an independent `P4KX_LOADOUT_OWNER_CENSUS`. Run with `--nocapture`.
The test reports Archive and DataForge setup, time to the first owner and
total visitor time with a minimal callback. Measure private-commit and
working-set peaks externally. Use the separate retained Catalog parity
test to compare values, coverage and source references.

The Ship-scope verifier on installed LIVE `13220821` compared resolver input
reuse on and off at signed revision `0ff026d07`. It ran fresh Windows AMD64
processes with Rust 1.100.0, one Rayon worker and Marker provenance. It used
the test profile with `p4kx-export` at optimization level 1 and `serde_json`
at level 3. The operating system file cache was left in place.

| Selection and input reuse | Elapsed s | Peak private bytes | Peak working-set bytes |
| --- | ---: | ---: | ---: |
| Ships with shared inputs | 899.546 | 1,091,772,416 | 1,024,266,240 |
| Input reuse disabled by the verifier | 1,015.841 | 1,091,702,784 | 1,024,122,880 |
| Ships and Items with shared inputs | 1,062.033 | 1,091,813,376 | 1,024,188,416 |

Both runs delivered 1,033 Ship rows and reported complete owner dependency
scopes. The canonical row, per-owner dependency and selection coverage
digests matched. Both reported 67 incomplete field positions and 80 skipped
leftovers. The broader visit admitted 10,955 owners and delivered 10,812 rows.
Its 1,033 Ship rows and their owner dependency digests matched the Ship
selection.

On installed LIVE `13220821`, the signed `be4719c55` debug verifier ran each
selection in a fresh Windows AMD64 process. The GUID and physical Record index
selected the same one-row owner with 30 source uses from one physical source.

| Selection | Result | Elapsed s | Peak private bytes | Peak working-set bytes |
| --- | --- | ---: | ---: | ---: |
| GUID | Complete | 8.320 | 1,091,813,376 | 1,024,466,944 |
| Record index | Complete | 7.759 | 1,091,801,088 | 1,024,356,352 |
| All | Measurement deadline | 1,800.164 | 1,895,276,544 | 1,775,017,984 |

The all-owner process reached the measurement time limit before returning the
terminal census for its 10,955 expected owners. These peaks establish the
resource use of the observed interval. Complete Loadout parity against the
same-Build Normal Catalog requires a terminal comparison.

A later one-worker Windows AMD64 verifier from signed `e2ec43eef` completed Full
Loadout parity on the same LIVE Build. It visited 10,955 owners, retained 10,812
ordered rows, reported 1,240 skipped leftovers and counted 226 rows with issues.
The retained Normal Full Catalog from `e59c7410f6411302e8101f180751177dd0d6adfb`
matched the rows, 2,379,065 contributing source uses and 374 physical sources.
The visit and replay took 4,624.868 s with 1,667,289,088 peak private bytes and
1,651,183,616 peak working-set bytes. The verifier used a direct JSON conversion
for typed Loadout rows. An exact synthetic check compared it with the wire
roundtrip on a nested Full row, including the canonical hash and its source closure.

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

Use `equipment::visit_weapon_sources` and its decoded-Entry, session or parsed forms for a
full or GUID-scoped scan. It calls the visitor in physical Record order with one owner index
and that owner's typed rows, field coverage and original source inventory. The callback can
consume or serialize each snapshot before the next owner is read. Repeated physical source
identities can occur across snapshots. Use the final availability, owner and row counts, and
combined coverage before concluding that an owner or component is absent. Keep any results
staged until the visitor returns success if a callback error, cancellation or Archive change
must preserve the previous output. `All` selects admitted Catalog Items. `Guid` keeps every
admitted physical occurrence of the requested GUID. `RecordIndex` addresses one owner.

The full visitor ran in fresh Windows AMD64 processes without opening a Normal Catalog.
The measured verifier executable had SHA-256
`b582efebce5f6bf1f6040e53946ea38e64dd184f1a57a1597c8f6184bbca2676`.
The Archive and manifest were unchanged at the end of each run.

| Build | Owners | Actions | Containers | Bindings | Elapsed (ms) | Peak Private Commit (bytes) | Peak Working Set (bytes) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 974 | 907 | 483 | 1,390 | 56,275 | 1,091,649,536 | 1,021,804,544 |
| PTU `13260598` | 991 | 924 | 490 | 1,414 | 58,173 | 1,114,898,432 | 1,032,060,928 |

Both process peaks stayed under 2,000,000,000 bytes. The scope is the directly authored
rows and their per-owner inventories.

The same executable matched all three ordered datasets and their contributing source
uses against each Build's complete Normal Catalog. It also checked the producer's
separately linked magazine scope. Each comparison resolved its contributing uses to one
physical source.

| Build | Normal producer | Contributing uses | Elapsed seconds | Peak private commit (bytes) | Peak working set (bytes) |
| --- | --- | ---: | ---: | ---: | ---: |
| LIVE `13220821` | `23addc5e164f65a28a6f258302b30453b331edce` | 6,282,132 | 4,771.626 | 1,786,421,248 | 1,787,920,384 |
| PTU `13260598` | `93d92ca20bef8a8c3731339214a3301124ade7ba` | 6,428,761 | 4,462.640 | 1,819,320,320 | 1,821,450,240 |

The weapon component exposes its fire and aim slots, local action nodes and their original
ordinals. `mode_kind()` classifies each action. For single and rapid projectile actions,
`combat_inputs()` borrows the original rate, projectile count and damage multiplier cells.
Inspect their states and origins independently. An incomplete component selection remains
visible in the snapshot coverage even when the readable components produce rows.

The authored query reads components owned by the selected Item. An admitted magazine Item
can be queried separately by its physical Record index. Magazine and Ammo References
report original header resolution. A filtered Catalog can include a linked container outside
the ordinary Item selection as a dependency. The authored query covers the selected owner
and leaves those dependency rows to the primary magazine dependency query. Use the projectile
ammunition query for a resolved action route and its reached Ammo.

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

Use `equipment::with_weapon_magazine_dependencies` to include the identical primary resolution
and the canonical `configurations.ammoContainers` and `declarations.ammoBindings` dependency
rows. The optional Catalog Item filter applies to both the physical weapon and the reached
magazine. With no filter, an ordinary Item magazine can be read with
`equipment::with_weapon_sources`. A linked magazine outside the ordinary Item selection
contributes the container slot and its binding to this dependency query. Those two rows share
the magazine's physical Record index and component ordinal, and carry each original field's
source evidence. Both rows belong to the snapshot's source inventory.

This query follows the Catalog's legacy first primary route. Read the typed resolution and
dependency coverage to distinguish a proved absence from unreadable or incomplete source
data. For a GUID alias, the physical target is the one the original primary Reference
resolves through DataForge. Action-specific admission remains in the projectile ammunition
query.

Verify the primary magazine SDK against one original own container and one
original linked magazine without generating a Catalog. Set `P4KX_DATA_P4K_B`
to include a second installed Build.

```powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_weapon_combat::magazine::weapon_magazine_sdk_matches_original_primary_routes -- --ignored --exact --nocapture --test-threads=1
```

### Installed ammunition container scope

On the installed PTU `13260598`, an independent original-field check selected
a weapon's own container and a linked magazine container. The typed SDK returned
the two physical configuration rows and their two original binding rows with
complete scoped coverage. The selected capacities matched their authored signed
values. The same process ran one worker on Windows AMD64. It measured 13,735 ms,
1,114,701,824 peak private bytes, 1,032,351,744 peak working-set bytes and a
minimum of 60,785,295,360 host bytes available. Its scratch directory stayed empty.

The separate archive test
`installed_physical_shop_and_ammo_containers_match_retained_complete_catalog`
checks these four rows and a physical Shop in one pass through the exact
same-Build Normal Catalog, including the resolved source-use inventory. The selected four
rows and one Shop row passed on PTU `13260598` against the complete retained package output.
The Shop digest agreed with its standalone SDK query.
[PTU package and replay measurements](ptu-export-compatibility-audit.md#complete-packaged-normal-catalog)

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

Read `ProjectileAmmoOwner::countermeasure_emitters` for the selected Item's
`configurations.countermeasureEmitters` rows. One row identifies a proved physical
weapon component, ammunition role, container component and countermeasure Ammo
Record. Repeated actions using that same combination share one row. The sourced
route and its fields retain the original contributor evidence. Join the route's
`ammo_record_index()` to `ammo_definitions` for the projectile parameters.
Check `dependency_coverage` before treating an empty emitter list as absence.

With `P4KX_DATA_P4K` set to an installed Archive, this verification checks one independently
selected weapon action, its original route and its reached Ammo. Set `P4KX_DATA_P4K_B` to
check a second Archive in the same run. Each check reads DataForge once and creates no Catalog.

```powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_weapon_combat::projectile::projectile_ammunition_sdk_matches_original_sources -- --ignored --exact --nocapture --test-threads=1
```

## Entity animation controllers

Use `equipment::entity_animation::with_entity_animation_controllers` or its decoded-Entry
and session forms to read `configurations.entityAnimationControllers` for the selected
physical Entity Records. The selection accepts a Record index, all physical aliases of a GUID,
or all admitted owners. Each row identifies the original component slot and exposes the
authored Mannequin controller and animation database locators with their immediate source
evidence. Additional authored component members remain accessible in the same snapshot.
The query reads the owner bodies that contain the requested component.

### Installed Entity component queries

Each of the six `All` selectors below ran alone in a fresh Windows AMD64
process against LIVE `13220821` and PTU `13260598`. All twelve queries
reported Available and preserved unique physical Record and component slots.

| Configuration | LIVE rows | PTU rows |
| --- | ---: | ---: |
| `entityAnimationControllers` | 8,076 | 8,219 |
| `explosives` | 99 | 100 |
| `flightControllers` | 277 | 275 |
| `ifcs` | 277 | 275 |
| `lootGeneration` | 1,314 | 1,698 |
| `thrusters` | 1,485 | 1,485 |

The explosives queries counted 1,160 LIVE and 1,172 PTU development
leftovers separately from admitted rows. Build and Archive stamps matched
before and after all twelve reads. The largest private commit was
1,410,879,488 bytes and the largest working set was 1,382,797,312 bytes.
The verifier was built from `914ca2681ec7dc0a4420350c7ba6da0a88237fbc`.

The six tables also matched a single retained complete Normal Catalog per Build.
The witness compared ordered values, availability and contributing source
uses against the original producer, including the exact canonical exclusions.
Each comparison used one Windows AMD64 worker.

| Build | Normal producer revision | Elapsed (s) | Peak private commit (bytes) | Peak working set (bytes) |
| --- | --- | ---: | ---: | ---: |
| LIVE `13220821` | `23addc5e164f65a28a6f258302b30453b331edce` | 1,799.522 | 1,898,024,960 | 1,883,656,192 |
| PTU `13260598` | `93d92ca20bef8a8c3731339214a3301124ade7ba` | 1,857.965 | 1,918,173,184 | 1,904,377,856 |

## Player weapon animation candidates

Use `equipment::with_weapon_animation_candidates` to read the canonical
`projections.weaponPlayerAnimationCandidates` rows for one physical Item.
The decoded-Entry, session and parsed forms share the Catalog's AttachDef,
Player database, scoped tag and option projection. Rows retain both Entry
identities, original branch and option coordinates, ordered tag matches,
the tool-owned `playback` status, and their own source inventory. `tagEvidence`
carries the complete per-candidate source proof and immediate encryption marker.
`playback` is the literal `unverified` status and has no input origin.
The SDK accessor `playback()` returns the typed status by value.

The query reads the selected Item and the Player context and reachable
Mannequin dependencies required by its personal-weapon attachments. It
matches the source declarations within that graph. Scoped results include
the physical owner coverage, Player and graph observations, and failed
Entry operations. An empty result can be interpreted using those observations.
The query creates no Catalog or temporary row spool and opens no localization
payloads.

Use `equipment::weapon_animation_sources::with_player_weapon_animation_sources` for
the authored graph at `definitions.weaponAnimationSources`. The direct, decoded-Entry
and session forms expose the Player context, actor bindings, controllers, databases,
tag definitions, scopes, branches and fragment selectors as typed values. They read
the exact Player Record scope and follow its selected verified Mannequin documents.
The query does not read Item bodies, localized strings or animation clips. Inspect
the Player and graph coverage and the failed dependency operations before inferring
absence. The source inventory belongs to this graph read.

The graph query can read Player sources when the selected Catalog would omit the
graph because no personal-weapon attachment was admitted. Candidate playback and
weapon matching belong to the separate Item-scoped query.

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

## Weapon procedural definitions

Use `equipment::weapon_procedural::with_weapon_procedural_animations` and
`with_weapon_procedural_clips` to read the corresponding `definitions` tables.
Direct, decoded-Entry and session methods select all physical definitions,
a GUID or one physical Record index. The animation row exposes ordered
setup slots, actor-state filters, and original clip References. Pass a resolved
clip's physical Record index to the clip query for its original field arena.

Both queries use the canonical producer row types and preserve the original
source evidence. An animation query reads original definition bodies only for
its selected type. Clip References retain their header evidence and state.
Join through the separate clip query to read a clip Record body. Neither
query reads animation media or localized texts.

## Actor procedural recoil definitions

Use `equipment::actor_procedural_recoil::with_actor_procedural_recoil_configs` and
`with_actor_procedural_recoil_modifiers` for the two actor recoil definition tables.
The direct, decoded-Entry and session forms select all definitions, a GUID or one
physical Record index. They retain all physical aliases in original definition
order. Recoil configurations expose ordered setups, source-backed filters and
original modifier links. Join a resolved modifier's physical target index with
the modifier table. The modifier exposes its original field arena.

Both queries share the producer's projection and retain the original evidence
for each value. Weapon references are unnecessary for admission to these source
families. The queries read neither Item bodies nor localization and leave modifier
bodies unopened when reading a configuration. Check header, body and field
coverage alongside the family's availability.

The Windows AMD64 verifier at `5976b21ebfe85f2c5152b3e0ce3be769ded77d79` compared
all four SDK tables with each matching Normal Catalog from producer
`24c08bcbff2299b88eff84bf3f7e8f592760b99b`. The Archive identity stayed stable
and the retained frame hash was verified for both Builds.

| Catalog definition | LIVE `13220821` rows | PTU `13260598` rows |
| --- | ---: | ---: |
| `actorProceduralRecoilConfigs` | 33 | 33 |
| `actorProceduralRecoilModifiers` | 33 | 33 |
| `weaponProceduralAnimations` | 57 | 57 |
| `weaponProceduralClips` | 287 | 287 |

The ordered rows matched, including 261,601 contributing source uses from one
physical source on each Build. The eight separate cold queries also completed
below 2 GB.

| Build | Retained comparison (s) | Peak private bytes | Peak working set bytes |
| --- | ---: | ---: | ---: |
| LIVE `13220821` | 3,696.182 | 1,091,645,440 | 1,024,065,536 |
| PTU `13260598` | 3,576.380 | 1,114,599,424 | 1,033,732,096 |

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

## Authored Starmap definitions

Use `starmap::with_starmap_definitions` or the corresponding session methods to select
one physical definition family. `StarmapDefinitionFamily::Objects`, `Types`,
`Presets`, and `Amenities` return the same typed rows as
`definitions.starmapObjects`, `definitions.starmapTypes`,
`definitions.starmapPresets`, and `definitions.starmapAmenities`. The object family uses
the Catalog's original PU path and retains its authored links, presentation,
and navigation. Placement queries have their own inputs. Use
`CatalogRecordSelection` to choose all admitted Records, all physical Records with a
GUID, or one Record index. The rows retain original field evidence and link statuses.
Presets include their owned display, visibility, travel, location, and radar settings.

The query returns only the referenced public names and protected texts under the
session's language policy. It reports family availability, selected Record coverage
and a separate source inventory. References can consult the original target headers.
The only payloads read are the selected Record bodies and the requested localization. A parsed session reuses the opened DataForge across repeated selections.

## Starmap object placement

Use `starmap::read_starmap_object_placement` with a physical DataForge Record index
from the queried Build. It returns the canonical `placements.starmapPlacements` row for
that Record. Records with the same GUID have separate physical indices and share the
world placement selected by that GUID.

The query checks the selected Record and all canonical system roots in catalog order.
It reads the reachable placement descriptors without building the complete catalog or
reading localization, location grid, or jump endpoint payloads. The returned status
distinguishes a Record outside the dataset, a complete traversal with no placement,
a selected placement, conflicting placements, and unresolved input. An admitted Record
with a conclusive result has one canonical row, including any absent placement fields.
An unresolved input has no row and keeps its Record or traversal coverage. The caller
may provide a `StarmapTraversalBudget`; exhausted allowances require an unresolved result.

## Jump endpoint inventory

Use `starmap::read_jump_inventory` for `placements.jumpEndpoints` and
`projections.jumpConnections`. It keeps each physical endpoint in canonical system and
Child order. Use the root Entry index, Child ordinals, and external-step positions from
`JumpEndpoint::instance_id` to identify an occurrence, including repeated placements
that share a Record and world position.

The query checks every admitted system root and reads the Starmap classification,
container descriptors, and original operating Entity dependencies. The snapshot
keeps observed endpoints and coverage when some sources are unavailable. Both tables
share the returned source inventory. The relationship table currently has an
uninhabited row type. Its availability reports surveyed absence of authored partners
or an unavailable result when the required endpoint or partner scope is incomplete.

## Starmap container placements

Use `starmap::read_container_system_roots` to enumerate the original root directory choices
in the Normal writer's system order. Each normalized system folder retains the physical
Entries at its winning path, including ambiguous choices, with original directory and Entry
evidence. Check `directory_complete()` for complete enumeration and `is_complete()` for
a unique observed candidate in every system. The root query reads directory metadata only.

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

Use `starmap::visit_container_placements` for both complete, unfiltered placement tables.
Its visitor receives a single typed resolved or unresolved row in Normal traversal order.
After each uniquely admitted root, `RootCompleted` supplies that root's coverage and original
source inventory. The terminal summary retains the directory choices, row counts and table
availability. An observed missing target with complete lookup evidence contributes an
unresolved row without making traversal incomplete. Commit caller-owned results after the
visit succeeds and its terminal coverage passes the caller's requirements.

A fresh Windows AMD64 process visited both tables for each installed Build.
The unresolved counts contain only links with conclusive missing-target evidence.

| Build | Resolved instances | Unresolved links | Peak private commit (bytes) | Peak working set (bytes) |
| --- | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 196,706 | 3 | 952,983,552 | 955,281,408 |
| PTU `13260598` | 230,321 | 9 | 973,496,320 | 975,523,840 |

The standalone verifier `c69801a87ad6a7a129e2af69a4473b53f408c2ba` checked
three complete system roots on each Build. Each run used the original Archive and
created no Catalog or row spool.

One retained comparison per Build verified both complete Normal tables in the same stream.
It checked physical occurrence identities, encounter order, authored values and the exact
contributing source uses and physical sources. The original Archive and Build stamps
remained stable, and both runs verified their sealed Normal frames.

| Retained Build | Contributing source uses | Contributing physical sources | Peak private commit (bytes) | Peak working set (bytes) |
| --- | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 690,538 | 10,121 | 953,085,952 | 956,334,080 |
| PTU `13260598` | 910,628 | 10,685 | 973,496,320 | 989,425,664 |

## Typed location occurrences

Use `starmap::read_location_instances` for the canonical `placements.locationInstances`
rows of one physical system root. `ContainerPlacementSelection` selects the root, an
ordered Child route, or an exact route with its external descriptor steps.

The query checks the reachable descriptor graph and same-container HCD and EntXML
companions for that root. An exact occurrence limits the materialized rows. Source
coverage still describes the selected root. Input payloads are scoped to those
descriptors and companions. Route keys, authored class names, transforms, metadata and
the companion lookup states match the producer. Inspect `coverage.is_complete()`
before interpreting empty results and retain the snapshot's own source inventory.

## Authored facilities

Use `equipment::with_facilities`, its decoded-Entry form or the equivalent session methods.
Select by physical Entity Record index, GUID or the entire facility dataset. A GUID retains
all admitted physical owners carrying that identifier. The result preserves the canonical
`configurations.facilities` row for each Seat, Medical Bed, Cargo Grid, Logout and Outfit Hanger
component in its original order. It resolves the same authored Cargo Grid capacities as the
Catalog. When a Grid is present, the owner supplies the co-owned storage rows and any inventory
definition admitted to the capacity calculation. Check owner and dependency coverage before
caching the result. Use the typed kind to distinguish facility families. A serialized Facility
row includes all authored fields and provenance for that family. The Cargo Grid accessor also
provides typed capacity and its operands.

## Authored inventory configurations

Use `equipment::with_inventory_configurations`, its decoded-Entry form or the equivalent
session methods to read canonical Item and Vehicle storage configurations. Choose all matching
owners, one GUID with its physical occurrences, or a physical Record index. The rows preserve
component order, authored container References and their provenance. The Vehicle row uses its
declared `inventoryContainerParams`; the item row retains the original item storage component.

Use `item_container_params` or `vehicle_container_params` for the typed original Reference
and `with_inventory_containers` to read the linked inventory definition. The storage query
projects its own component fields. Bounds and room associations have their own datasets.

The installed Windows AMD64 verification ran each table as a standalone cold query and compared all
four tables in one complete retained Normal Catalog stream per Build. The port, bounds and attachment
queries used `visit_item_ports`, `visit_item_bounds` and `visit_attachment_constraints`. Storage used
`with_inventory_configurations`. All four routes accounted for 1,332 canonical exclusions in LIVE
`13220821` and 1,341 in PTU `13260598`.

| Configuration | LIVE rows | PTU rows |
| --- | ---: | ---: |
| `itemPorts` | 14,024 | 14,751 |
| `itemBounds` | 25,607 | 26,417 |
| `itemAttachmentConstraints` | 23,201 | 23,977 |
| `inventoryConfigurations` | 3,506 | 3,779 |

The retained comparison verified physical owner and slot order, authored values and contributing
original provenance. It verified 5,179,572 distinct source uses in LIVE and 5,788,426 in PTU, with one contributing
physical source in each Build. Across the eight cold runs, the largest native lifetime private
commit was 1,745,113,088 bytes and the largest working set was 1,757,732,864 bytes.

| Retained Build | Peak private commit (bytes) | Peak working set (bytes) |
| --- | ---: | ---: |
| LIVE `13220821` | 1,658,818,560 | 1,672,790,016 |
| PTU `13260598` | 1,748,770,816 | 1,761,230,848 |

Both retained runs bound the original Archive and Build stamps and the sealed Normal frames from
producers `23addc5e` (LIVE) and `93d92ca2` (PTU).

## Authored interactions

Use `equipment::interactions::with_interactions`, its decoded-Entry form or the
equivalent session methods for the canonical `configurations.interactions` table.
Select a physical Entity Record index, a GUID with all its admitted occurrences,
or the entire component family. Each row retains its original Components slot.
The typed accessors expose interactable declarations, state machines, carryable
grips and the authored kiosk flag. On kiosk rows, `support_walk_to_align_in_vr()`
returns the sourced Bool carrier. Its state distinguishes a stored value, a
missing field and invalid storage. On a carryable row, `grip_data()` retains
the ordered original grip array, hand helper references and carry dimensions.
The dimensions retain the authored Single bits. Serialize the row for the
complete original component, including authored Tag references and additional
fields. [Interaction contract](catalog-inventory-interactions.md)

Use the snapshot's protected-text lookup for reached display names and keep its
source inventory with the result. References retain the authored target and
resolution state. Use the target's physical Record index with the Tag declaration
query to read a linked Tag definition.

For an incremental read, use `visit_interactions` or
`visit_interactions_from` in the same module, or their session and parsed
methods. Each callback borrows one readable physical Record's rows, protected
text definitions and source inventory. Callbacks run in physical Record order,
including readable candidates with no rows. Text origins cover the uses reached
by that Record. A canonical Catalog text row can combine uses from several
Records. The terminal summary reports family availability, total owner and row
counts, and full selection coverage. Stage consumer changes until the visit
returns successfully and the coverage meets the caller's requirements.

The Windows AMD64 cold runs of verifier `691a8791` read `All` directly from
each installed Archive with an empty locale selection. Both reported
`Available`, exact owner and row order, and complete selection coverage
after accounting for the verified canonical exclusions.

| Build | Owners | Rows | Peak private commit (bytes) | Peak working set (bytes) | Elapsed (ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 16,747 | 30,066 | 1,091,502,080 | 1,022,038,016 | 262,359 |
| PTU `13260598` | 17,524 | 31,521 | 1,114,439,680 | 1,031,843,840 | 285,355 |

Verifier `1114c40cd222c428cc917d05e64f214835b38789` compared every interaction
row and its contributing provenance with the matching sealed Normal exports
from producer revision
`24c08bcbff2299b88eff84bf3f7e8f592760b99b`. Both comparisons used an empty
locale selection. The scoped English fixture separately checks the protected
text and its exact source uses.

| Build | Matched rows | Contributing uses | Peak private commit (bytes) | Peak working set (bytes) | Elapsed (ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 30,066 | 34,120,354 | 1,091,485,696 | 1,052,872,704 | 4,837,122 |
| PTU `13260598` | 31,521 | 36,137,915 | 1,114,509,312 | 1,033,232,384 | 4,888,600 |

Both retained runs verified the frame hash and a stable Archive. These
measurements cover the dedicated interaction visitor.

## Paint compatibility

Use `equipment::paints::read_paint_compatibility` with a physical Paint
Record index, or with a GUID when the original Paint owner is unique. The
query compares that owner with the eligible Ship Records and their effective
Record and vehicle XML ports. The result keeps the canonical `paints` row,
physical Ship positions in the same order, and the referenced source inventory.

A `Found` row has complete observed inputs for this scope. A `Partial` row
contains the matches supported by the inputs read so far. The snapshot reports
incomplete owner, Ship and vehicle dependencies separately. A vehicle Record that
authors an empty modification string retains that original selector for the base
implementation. The Paint query also closes a missing explicit patch lookup when
the complete original Archive directory proves the exact attempted Entry absent
and the other XML selection evidence is complete. Failed reads and recovered
directory scopes remain incomplete.

An isolated Windows debug query selected one physical Paint and matched its one
observed fitting Ship in each Build. The test resolved the attachment and
vehicle definition selectors to their original DataForge Entry. Both results
reported `Found` and complete fleet scope. The source census reported zero
unreadable or incomplete Ship and vehicle inputs and a complete Archive
directory.

| Build | Matching ships | Incomplete vehicle Records | Peak private commit (bytes) | Peak working set (bytes) | Seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| LIVE 13220821 | 1 | 0 | 1,203,261,440 | 1,176,604,672 | 75.647 |
| PTU 13260598 | 1 | 0 | 1,228,677,120 | 1,199,923,200 | 75.721 |

The signed verifier at `5c1602209` bound both original coordinates independently
and produced the same one-row digest on both Builds. Set `P4KX_DATA_P4K` and
`P4KX_NEW_SDK_BUILD` for the installed source, then run the scoped witness.

```powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_scoped_equipment_sdk::paint_compatibility::installed_paint_compatibility_keeps_original_record_evidence -- --ignored --exact --nocapture --test-threads=1
```

Full Normal parity and Paint enumeration remain part of release acceptance.

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
totals have their own projection.

## Contract scenario progressions

Use `catalog_queries::contract_scenarios::with_contract_scenarios`, the decoded-Entry form or
the session and parsed-view methods to read the `contractScenarios` rows. Select all applicable
Records, the original Record index, or all physical owners of one GUID. The query shares the
normal Catalog projection and does not need a complete Catalog or localized strings.

The rows borrow the original DataForge and include the selected Faction's physical reputation
Reference when available. Resolve their original-use references against the snapshot's own
inventory. A scoped query opens only the selected scenario bodies and their reached compatible
Faction bodies. Completion pointers, BlueprintPool targets and periodic scenario References
retain their authored structure or target headers without opening target Records for those links.
Check selection and field coverage before interpreting an empty response.

## Facility placements from a Loadout

Use `equipment::read_loadout_facility_placements`, its decoded-Entry form or the equivalent
session methods with a physical Ship Record index from the same DataForge. The snapshot returns
the Loadout-kind rows of `placements.facilityInstances` for that ship, including authored Facility
and Inventory storage owners. Use each position's physical root and ordered port ordinals.
The same component can contribute an instance at several distinct positions.
Check the selected Loadout state, its coverage, and the reached configuration dependencies
before interpreting an empty result. Retain the snapshot's own source inventory.

Use `equipment::visit_loadout_facility_placements` to read every admitted physical Ship
in canonical Loadout order. Each callback carries the original Record index, placements,
its Loadout state and dependency coverage, including for empty and unreadable Loadouts.
The result's source inventory resolves its placement references in Full mode. Each Ship
is resolved once and shared configuration dependencies are reused within that owner.
Use the decoded-Entry form or the equivalent session and parsed-view methods to reuse
inputs. Check terminal coverage before committing callback output.

For complete-visit acceptance, run the ignored tests
`installed_all_loadout_facility_placements_cold_visit` and
`installed_all_loadout_facility_placements_match_retained_complete_catalog`
with the exact installed Build. The retained comparison also requires
`P4KX_FACILITY_LOADOUT_CATALOG_PRODUCER_REVISION` and the matching completed
Normal Catalog measurement.

### Installed memory measurements for Store mappings and Loadout placements

Run the ignored tests under `real_archive_scoped_equipment_sdk` in the
`p4kx-export` real-archive test target. Set `P4KX_DATA_P4K` and
`P4KX_NEW_SDK_BUILD` for the installed Archive and its expected change
number. Use the configured archive key. Set `P4KX_STORE_SLUG_SDK_RECORD`
for the selected Store parity test or `P4KX_FACILITY_SDK_RECORD` for the
physical facility Ship. The Facility producer comparison also requires an
anchored `P4KX_FACILITY_SDK_FILTER` and its complete original Record set in
`P4KX_FACILITY_SDK_SELECTED_RECORDS`. Its preflight checks that the selection
includes all reached configuration and alignment owners.

A Windows AMD64 Debug test driver built with production code `5c775c446`
opened the installed Archive in a fresh process for each query. The Store
queries were unfiltered. Each Facility query returned 13 placements for its
selected physical Ship and resolved the row origins through its scoped
source inventory.

| Build | SDK query | Peak private bytes | Peak working set bytes | Elapsed |
| --- | --- | ---: | ---: | ---: |
| LIVE `13220821` | Store mappings | 1,459,482,624 | 1,456,234,496 | 44.03 s |
| PTU `13248297` | Store mappings | 1,482,338,304 | 1,478,729,728 | 45.47 s |
| LIVE `13220821` | One Ship's Facility placements | 1,091,895,296 | 1,024,172,032 | 14.46 s |
| PTU `13248297` | One Ship's Facility placements | 1,116,172,288 | 1,034,334,208 | 14.53 s |

The host had 100,573,523,968 bytes of physical memory. Available memory
started between 62.23 and 62.31 GB and stayed above 60.75 GB. All four
processes remained below 2 GB. The largest peak leaves an estimated 15.70 GB
for the operating system and other applications on a 16 GiB host. Installed
producer parity for the complete Facility scope and the integrated
consumer's memory measurements are separate checks.

### Interior room coverage

`read_facility_interior_coverage(archive, physicalShipIndex, cancel)` reads the
selected ship's `placements.facilityInteriorCoverage` rows. The direct ship
room declarations are read independently of the Loadout. Attached rows keep
their original ordered port path. The query reads both declaration families,
the reached room descriptor and SOC streams, and the physical DCB aliases
of observed classes. It uses no room model geometries.

The result includes Loadout, declaration and class dependency coverage.
`complete` describes the selected scope and is conservative about traversal
findings, unreadable declarations and unresolved operations. Known rows
and findings remain available with their original field sources.

### Interior facility placements

Use `equipment::read_facility_interior_placements`, its decoded-Entry form or
the equivalent session methods to read the Interior-kind rows of
`placements.facilityInstances` for a physical Ship. The query uses the canonical
room model helpers and SOC transforms. It returns direct placements before
placements reached through attached rooms. The separate coverage query avoids
opening room models when an application needs only the traversal results.

`FacilityInstance::interior_placement()` exposes the room owner, attachment
position, physical SOC coordinates, class and transforms. The instance's
configuration links retain the original physical owner identities. Check
`availability` and `coverage` before interpreting empty rows. Resolve the
source references of both row sets through `coverage.source_inventory`.

Use `equipment::visit_facility_interior_placements` for all admitted physical
Ships. The free function, decoded-Entry form, session and parsed session share
the same fallible callback. Each callback borrows one Ship snapshot, including
Ships with no rooms and Ships with incomplete coverage. The final summary
counts Ships, placements, room coverage rows and incomplete Ships alongside
the original Ship admission coverage. The placement and room rows have
owner-local source inventories when Full provenance is requested.

Callbacks follow canonical Loadout owner order, with GUIDs compared by their
text form. Each Ship's direct rooms precede rooms reached through its attached
items. To reproduce the full Catalog's global order, emit all direct rooms in
physical Ship Record order first, then emit the attached rooms in callback
order with their original port traversal order.

For an installed coverage comparison, set `P4KX_DATA_P4K`, the normal stored
key and the expected change number in `P4KX_NEW_SDK_BUILD`. Select a physical
Record in `P4KX_FACILITY_INTERIOR_SDK_RECORD`. The ignored
`installed_facility_interior_nominate_record` test accepts a unique original path
in `P4KX_FACILITY_INTERIOR_SDK_RECORD_PATH`. Use the physical index for the
separate `installed_facility_interior_cold_query` and
`installed_facility_interior_placements_cold_query` measurements in fresh processes.
`installed_facility_interior_placements_match_retained_complete_catalog`
compares the selected Interior placements and coverage in one stream. The
retained comparisons read `P4KX_CATALOG_MEASUREMENT_DIR` and verify the nominated
producer revision from `P4KX_FACILITY_INTERIOR_CATALOG_PRODUCER_REVISION`.
The ignored `installed_all_facility_interior_placements_match_retained_complete_catalog`
test compares both datasets and their referenced physical sources for all Ships
in one stream. It reconciles the resolved Ship roots with the published Loadout
owner sequence, including published Ships with no Interior rows. The separate
`installed_all_facility_interior_placements_cold_visit`
test measures the default provenance mode with the same installed Build.

On installed LIVE `13220821`, the selected placement query verified
`AEGS_Hammerhead_GS` at physical Record index `98717`. An unoptimized native
Windows AMD64 test binary with one Rayon worker returned 18 room coverage rows,
one Interior placement, 396 contributing uses and 19 physical sources. Its
lifetime peak was 1,091,923,968 B private commit and 1,024,147,456 B working set.
The test process elapsed time was 15.657 s with the archive-free verifier
running on the same host. The binary SHA-256 was
`879b1b59a248c712fb154672b8d52ac7e7b106cdc7ef4fc16a4a2b90562cb6e7`.
The release-profile test at signed revision `4aa8bcfa2` matched those 18
coverage rows, one placement, 396 source uses and 19 physical sources against
the complete same-Build Catalog from `e59c7410f`. The test verified the
archive and retained producer receipt, then compared the two tables and their
source closure in one streamed read. Its SHA-256 was
`8bfc79fcc0227d0a419762cb12181f89ea190af6b8dfc044ad078ae6f8b2f951`.
This verification took 554.417 s with a 1,091,801,088 B private-commit peak
and a 1,039,769,600 B working-set peak. The elapsed time includes reading
the complete retained frame.

The same release verifier matched all 1,033 resolved Ship roots on that
Build, including 76 with incomplete Interior coverage. The streamed
comparison matched 4,519 coverage rows, 2,447 placements, 62,094 contributing
source uses and 944 physical sources. Elapsed time was 938.526 s, including
the retained Catalog decode. Lifetime peaks were 1,468,084,224 B private
commit and 1,352,679,424 B working set. This run establishes LIVE parity
against the nominated producer and includes the verifier overhead.

On PTU `13284307`, the same one-worker release verifier matched 1,041
resolved Ship roots against the retained Normal Full Catalog from
`eada15a2`. It verified 4,583 coverage rows, 2,414 placements, 62,232
contributing source uses and 949 physical sources. The SDK and Catalog
agreed on the 86 incompletely covered Ships. End-to-end verification took
1,074.500 s with 1,491,918,848 B lifetime private commit and 1,373,360,128 B
working set. The time includes the streamed Catalog read. The bound
producer and replay evidence are recorded in the
[PTU replay](catalog-partitioning.md#completed-ptu-13284307-replay).

## Installed flight configurations

Use `equipment::read_installed_configurations`, its decoded-Entry form or the equivalent
session methods with one physical Ship Record index. The query returns that vehicle's
`placements.installedConfigurations` rows for Flight Controller, IFCS and Thruster
components. The same Loadout resolver selects each original attachment and its physical
Record. Occurrences retain their distinct root and port-ordinal coordinates. The query
reads the flight configuration owners reached by that resolver and reuses a physical
owner across repeated ports. A partially readable component list retains its valid rows
and reports the unresolved original member in configuration dependency coverage.

Read `loadout_state`, `loadout_coverage`, `loadout_dependencies` and the per-owner
`configuration_dependencies` before interpreting omitted or empty rows. The
`availability` field describes the declared flight component families. Preserve the
same Build and source inventory when interpreting source references. Compare a selected
Ship with the rows for its physical Record in the complete producer or an equivalent
scope whose Record selection includes all reached flight owners.

Use `equipment::visit_installed_configurations` for all admitted Ships in canonical
Loadout order. Each callback receives the same snapshot shape as the selected
query, with its physical Ship Record index, rows and local source inventory.
An empty resolved Loadout can return zero configuration rows with complete
coverage. A failed Loadout retains its physical owner and unreadable state.
Use the decoded-Entry form or the equivalent session and parsed-view methods
to reuse inputs. Check the final count and Ship admission coverage before
committing staged results. Cancellation or a callback error stops the visit.

A cold query on installed PTU `13260598` matched the twelve independently
nominated Flight rows. It completed in 14.623 s with 1,114,664,960 peak private
bytes and 1,032,646,656 peak working-set bytes. The witness retains the original
attached Record, the ordered Loadout position and the owning component.

The retained Normal Catalog from producer
`5d3d917f424657990fd775b8fae8ac843a3fd528` matched all twelve ordered rows
for the same physical Ship on PTU `13260598`, including source-use and
physical-source identities. The optimized verifier completed in
1,723.431 s with 1,114,714,112 peak private bytes and 1,048,502,272 peak
working-set bytes.

On PTU `13273298`, a Windows AMD64 verifier from signed revision
`e2ec43eef7eb6217c695102d824ce5f6b5aa09e9` checked one independently nominated
physical Ship with Full provenance and one worker. The twelve selected rows
matched in both queries. The retained comparison against the complete Normal
Catalog from `4d334e716cbc87e27525e8e095410a19103c51f5` also verified their order,
contributing source uses and physical-source identities.

| Flight verification | Elapsed s | Peak private bytes | Peak working-set bytes |
| --- | ---: | ---: | ---: |
| Cold selected-Ship query | 10.728 | 1,114,898,432 | 1,048,842,240 |
| Retained complete Catalog comparison | 1,752.497 | 1,114,927,104 | 1,032,990,720 |

On LIVE `13220821`, one replay of the complete Normal Catalog from producer
`23addc5e164f65a28a6f258302b30453b331edce` matched the 12 selected Flight rows,
all 205 Vehicle Armor rows and all 14,024 resource network link rows. The three
tables resolved 1,048,291 source uses to one physical source. The verifier
completed in 1,991.235 s with 6,538,014,720 peak private bytes and
6,376,349,696 peak working-set bytes. This measurement includes the combined
comparison and the streaming Normal replay.

For the installed checks, set `P4KX_DATA_P4K` for the original Archive,
`P4KX_NEW_SDK_BUILD` for its exact change number and
`P4KX_FLIGHT_SDK_RECORD` for the physical Ship. Choose a Ship with a complete
Loadout and an attached Flight configuration whose nonempty port-ordinal path
and original owner have complete source evidence.
The parity check also uses `P4KX_FLIGHT_SDK_FILTER` and
`P4KX_FLIGHT_SDK_SELECTED_RECORDS` to bind the exact producer scope.

```text
cargo test --locked -p p4kx-export --test real_archive real_archive_scoped_equipment_sdk::installed_configuration::installed_flight_cold_query -- --ignored --exact --test-threads=1
cargo test --locked -p p4kx-export --test real_archive real_archive_scoped_equipment_sdk::installed_configuration::installed_flight_matches_exact_scoped_normal_producer -- --ignored --exact --test-threads=1
```

For a retained complete Normal Catalog, set `P4KX_CATALOG_MEASUREMENT_DIR` to its measurement
directory and `P4KX_FLIGHT_CATALOG_PRODUCER_REVISION` to the exact producer revision. The comparison
binds the installed Archive and matches each row for the selected physical Ship in producer
order, including source-use and source identities. The combined check compares every Vehicle
Armor row in the same Catalog stream.

```powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_scoped_equipment_sdk::installed_configuration::retained_parity::installed_flight_matches_retained_complete_catalog -- --ignored --exact --nocapture --test-threads=1
cargo test --locked -p p4kx-export --test real_archive real_archive_scoped_equipment_sdk::installed_configuration::retained_parity::installed_flight_and_vehicle_armor_match_retained_complete_catalog -- --ignored --exact --nocapture --test-threads=1
cargo test --locked -p p4kx-export --test real_archive real_archive_scoped_equipment_sdk::installed_configuration::retained_parity::installed_flight_vehicle_armor_and_resource_links_match_retained_complete_catalog -- --ignored --exact --nocapture --test-threads=1
```

### Complete Flight fleet verification

Run both fleet tests in separate processes for each installed Build. For both,
set `P4KX_DATA_P4K`, `P4KX_NEW_SDK_BUILD` and
`P4KX_FLIGHT_CATALOG_PRODUCER_REVISION`. Set
`P4KX_FLIGHT_FLEET_MARKER_RECEIPT` to a new JSON path in the private temporary
directory. The Marker test writes this receipt. Keep the same receipt and
set `P4KX_CATALOG_MEASUREMENT_DIR` to the matching completed Normal
Catalog measurement before running the Full test.

```powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_scoped_equipment_sdk::installed_configuration::fleet_witness::installed_flight_fleet_cold_marker_visit -- --ignored --exact --nocapture --test-threads=1
cargo test --locked -p p4kx-export --test real_archive real_archive_scoped_equipment_sdk::installed_configuration::fleet_witness::installed_flight_fleet_matches_retained_complete_catalog -- --ignored --exact --nocapture --test-threads=1
```

The Full test compares the physical Ship census, ordered values and immediate
markers with the Marker receipt, then checks the original contributors against
the retained Catalog. The terminal census reports incomplete owners separately
from parity. Measure memory in the cold process independently of the retained
comparison.

A native Windows AMD64 verifier at `a5908ee05513f35c4a790b4f7ea106a8eb7e6b5c`
visited every admitted Ship with one Rayon worker and an empty application
cache. Its executable SHA-256 was
`b5f8c29863b2b226c1a03de9a920938bda92626198849acc0457ee5af1bbf274`.
Both Archive stamps remained stable. The measurements cover the cold Marker
visitor and its verification receipt. The PTU run overlapped unrelated
compiler processes, so its elapsed time describes that shared host run.

| Build | Ship owners | Placements | Incomplete owners | Elapsed (s) | Peak private commit (bytes) | Peak working set (bytes) | Minimum available host RAM (bytes) | Peak scratch (bytes) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 1,033 | 20,630 | 67 | 517.133 | 1,091,563,520 | 1,021,820,928 | 50,312,564,736 | 872 |
| PTU `13284307` | 1,041 | 20,905 | 79 | 561.211 | 1,114,730,496 | 1,031,749,632 | 39,858,872,320 | 870 |

Both processes stayed below the 2 GB query ceiling. The larger peak leaves
an estimated 16,065,138,688 bytes for the OS and other processes on a 16 GiB
system. The cold results establish the census and process footprint.
The retained Full comparison independently verifies canonical occurrence
parity and contributor coverage.

A verifier built from `a518fa0628038efc361148c29878f910ac0464a6` reused the
DataForge component probes across owners. It ran once per Build with one
Rayon worker and a stable Archive. The executable SHA-256 was
`698f46d4b32c965c1aea04accf17bd089379be79e8781936a064ca8b453f2f46`.

| Build | Elapsed (s) | Peak private commit (bytes) | Marker receipt SHA-256 |
| --- | ---: | ---: | --- |
| LIVE `13220821` | 280.140 | 1,091,465,216 | `968ef32394fb180e3d19cdb927304b93893aa532f922c29b09b054c567f979b5` |
| PTU `13284307` | 315.902 | 1,114,738,688 | `2a2c194522cc0d7f1e26f492802c1bc59567daf40acf20a9701eb1acff5dfcb4` |

Each Marker receipt was byte-identical to its baseline. The independent LIVE
processes showed 45.8% lower elapsed time. The earlier PTU measurement shares
the host with other builds, so its elapsed time is contextual. The separate
Full retained comparison covers occurrence and contributor parity.

The retained LIVE 13220821 comparison used the same a518 verifier, its exact
Marker receipt, and the complete Normal Full Catalog from
`84e0a9ca95ca03da3cae281e24240be9377c7ea5`. The installed Archive stayed stable
and the 1,576,271,368-byte compressed frame hash was verified before replay.

The test matched all 20,630 ordered placements for 1,033 physical Ships,
including the terminal census, values and immediate encryption markers. It
reconciled 108,961 distinct contributing source uses with one original
physical source. The census retains 67 incomplete owners, 56 owners with
empty present placements and 80 skipped leftovers.

The scoped Full visit and evidence comparison took 1,177.618 s. The bounded
process, including the 95,109,379,589-byte decoded Catalog replay, took
3,271.411 s. It peaked at 1,091,399,680 bytes of private commit and
1,023,107,072 bytes of working set. The receipt SHA-256 is
`768ed7de2c51a782a2f1d8b055cc84da6802d880947b2df047614555b7935e51`.

## Original cargo manifests

Use `catalog_queries::cargo_manifests::with_cargo_manifests` for
`definitions.cargoManifests`. The `_from` form accepts a decoded Entry from the
same Archive. Reuse `with_cargo_manifests` on a query session or parsed view. Select
all original manifests, every admitted physical occurrence of a GUID, or one
`CatalogRecordSelection::RecordIndex` in this DataForge. Equal GUIDs and paths retain
each physical Record index. The canonical table orders rows by GUID and Record index.

Typed getters expose original fill settings, ordered resource slots and allocation
bounds, ordered spawn rules, quantity algorithms and remaining authored fields.
The snapshot includes Build identity, family availability, selected Record coverage
and the source inventory for its values. Resource and entity references retain original
target identities. Query the target's own dataset to read its body. This route reads
only admitted manifest bodies and the metadata needed to resolve reached references.
Publish consumer-owned results after query success and after checking the required coverage.

## Crafting stations

Use `catalog_queries::crafting::with_crafters` to read `definitions.crafters`.
The decoded-Entry form and the session methods support All, GUID and physical Record index
selection. The query reads admitted Entity Records and their original authored crafting
component, plus the selected attachable component. It does not read localization or Loadouts.
Check record coverage and the snapshot's own source inventory before caching a result.

Crafting station rows carry their exact original values and independently sourced array slots.
Use `crafting::read_crafter_hosts` or the decoded-Entry, session or parsed-view form for
`projections.crafterHosts` on one physical ship Record index. The same Loadout resolver
and mount projector serve both the full Catalog and this query. The query reads the selected
ship's Loadout plus relevant resolver dependencies and attached station candidates. Each physical
target is checked once. Repeated mounts retain distinct sourced positions in their original order.

A present host state with an empty row list means the selected Loadout resolved. Check
`loadout_coverage`, `crafter_coverage` and the resolver dependencies before treating the list as
complete. An absent host state means the physical selection has no admitted ship. An unreadable
host state distinguishes an unreadable header or a selected ship whose Loadout could not be
produced.

For an installed Crafter witness, configure `P4KX_DATA_P4K` and
`P4KX_CRAFTER_WITNESS_BUILD` with the matching archive and expected changelist. The
station test selects one original component using metadata and a component probe.
Use the configured Archive key or set `P4KX_KEY_FILE` or `P4KX_ARCHIVE_KEY`.
Optionally set `P4KX_CRAFTER_WITNESS_STATION_INDEX` to a private exact physical index.

The positive host witness also requires `P4KX_CRAFTER_WITNESS_SHIP_INDEX` and
`P4KX_CRAFTER_WITNESS_FILTER`. It resolves the selected original Loadout and proves
the single filter selects exactly that ship and all independently reached crafter Records
before generating the filtered Catalog. If the scope cannot be expressed by a single
filter, use a current complete producer artifact for a same-Build scoped comparison.
These tests emit aggregate counts and retain no exported files.

```powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_crafter_sdk::installed_crafter_station_matches_original_sdk_and_literal_producer -- --ignored --exact --nocapture --test-threads=1
cargo test --locked -p p4kx-export --test real_archive real_archive_crafter_sdk::installed_crafter_hosts_match_original_sdk_and_exact_filtered_producer -- --ignored --exact --nocapture --test-threads=1
```

For the cold host measurement, set `P4KX_CRAFTER_WITNESS_NOMINATION` to
an unused absolute receipt path in a private output directory. The first
test identifies an original complete Loadout with at least one Crafter
mount. `P4KX_CRAFTER_WITNESS_SHIP_INDEX` can pin a particular original host.
The second test requires that same receipt and Build and checks the
public SDK's physical mounts and contributing source closures.

```powershell
cargo test --locked -p p4kx-export --test real_archive real_archive_crafter_sdk::nomination::nominate_original_crafter_host -- --ignored --exact --nocapture --test-threads=1
cargo test --locked -p p4kx-export --test real_archive real_archive_crafter_sdk::nomination::installed_crafter_host_cold_query -- --ignored --exact --nocapture --test-threads=1
```

On PTU `13260598`, the original source nominated one mount group and one
port route. An isolated Windows AMD64 debug process checked the group in
13.068 seconds. Its peak private commit was 1,114,697,728 bytes and its peak
working set was 1,033,297,920 bytes. Exact producer parity is a separate
acceptance requirement.

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

## Authored EMP and ordnance configurations

Use `equipment::ordnance::with_emp`, `with_explosives` and
`with_missile_configurations` for the original selected Item components. Each
result retains physical Record and Components positions. The explosive and
missile tables represent distinct projections of the same physical missile
component. Missile configuration uses authored ordnance field states to retain
only genuinely unmapped additional fields.

Use `with_explosion_definitions` in the same module for independently selected
`ExplosionRecord` definitions. A reference in another dataset can provide its
physical Record index. The query preserves original null, unresolved and
unreadable reference states without materializing foreign definition bodies or
media. Every route has a decoded-Entry form and can reuse a parsed session.

Rust callers can inspect the canonical payloads directly. Match
`ExplosiveConfiguration` for the authored component variant, and read
`ExplosionDefinition::explosion_params` for an independent explosion Record.
EMP settings and missile guidance, targeting and shake expose the original
`SourcedValue` fields, including absence, null and per-datum provenance.

To compare selected Missile and Explosive rows with the same installed
producer and original field contributors, set `P4KX_DATA_P4K` to the installed
Archive and use the normal stored key lookup.

```powershell
cargo test -p p4kx-export --test real_archive real_archive_missile_configuration::source_selected_missile_signal_families_match_original_guidance_and_targeting -- --ignored --exact --test-threads=1
```

The test selects one physical Item per original missile signal family. It
builds filtered Catalog excerpts in memory and checks each against a query
for the same Record. This verifies selected rows and their field origins.

The all-Record cold queries ran separately in fresh Windows AMD64 processes.
Missile configuration was available with 67 rows on both Builds. The Explosion
definition type was declared with no admitted rows on either Build, so its
availability was `Available` with an empty table.

| Build | Missile peak private commit (bytes) | Explosion definition peak private commit (bytes) |
| --- | ---: | ---: |
| LIVE `13220821` | 1,091,457,024 | 1,091,448,832 |
| PTU `13260598` | 1,114,476,544 | 1,114,427,392 |

The verifier was `c69801a87ad6a7a129e2af69a4473b53f408c2ba`.

One complete retained Normal Catalog comparison per Build verified both
tables in the same stream. It compared all 67 Missile rows in physical
owner order, the declared empty Explosion table, and 13,578 distinct
contributing source uses backed by one original physical source on each Build.
The Missile selection accounted for 1,160 canonical exclusions in LIVE and
1,172 in PTU.

| Build | Retained peak private commit (bytes) | Retained peak working set (bytes) |
| --- | ---: | ---: |
| LIVE `13220821` | 1,091,563,520 | 1,023,234,048 |
| PTU `13260598` | 1,114,963,968 | 1,041,694,720 |

Both comparisons retained stable Archive and Build stamps and used the
sealed Normal frames from producers `23addc5e` (LIVE) and `93d92ca2` (PTU).

## Jump drives and quantum interdiction

Use `equipment::ship_systems::with_jump_drives` and
`with_quantum_interdiction_generators` for `configurations.jumpDrives` and
`configurations.quantumInterdiction`. Select an Item Record by GUID or physical index,
or read the complete component family. Results preserve the physical owner and original
Components ordinal. Device settings, weak interaction links, and ordered visual states
use the same sourced fields as the Catalog.

Use `equipment::component_definitions::with_jump_drive_flight_tuning` for
`configurations.jumpDriveFlightTuning`. Its selection addresses the definition's own
Record. A device's `flightTuning` reference provides the target Record index, which can
be passed as `CatalogRecordSelection::RecordIndex`. The definition returns its original
ordered field arena. All three queries accept the original decoded Entry through `_from`
and are also available on the query session and parsed view. Each result has its own
source inventory and coverage. [Jump-drive definitions](catalog-jump-drives.md).
[Quantum interdiction](catalog-quantum-interdiction.md).

## Authored jump-travel configurations

Use `equipment::jump_points::with_jump_point_configurations` for the
`configurations.jumpPointConfigurations` table. It selects original Entity
Records and returns each authored `SJumpPointParams` component with its
physical Components slot and original values.

Use `equipment::component_definitions::with_global_jump_point_configurations`
for the union of global point and global tunnel-host Records, and
`with_jump_tunnel_forces` for the tunnel-force definition linked by a jump drive.
These two queries return the selected definition's original field arena.
Choose an exact physical Record index from a reference when it is available.
The Archive, decoded-Entry and parsed-session forms use the original DataForge
and report availability, selection coverage and per-value source evidence.

Ten standalone Windows AMD64 checks ran each exact family selector in a fresh
process on LIVE 13220821 and PTU 13260598. Both Builds returned the same
family counts and complete owner and definition discovery.

| Dataset | Rows per Build |
| --- | ---: |
| `jumpPointConfigurations` | 1 |
| `globalJumpPointConfigurations` | 2 |
| `jumpDrives` | 8 |
| `jumpDriveFlightTuning` | 5 |
| `jumpTunnelForces` | 7 |

The largest observed private commit was 1,114,955,776 bytes. The largest
working set was 1,032,163,328 bytes. The Archive and Build stamps remained
stable throughout. The exact test executable SHA-256 was
`4c0431fef6a45951d3cf5e5597865f53656e6aeb0a5699ed0017dbace3cf0ccf`.

The installed comparison `installed_jump_configurations_match_one_complete_catalog`
streamed each Build's complete Normal Catalog once and checked all five tables'
availability, ordered values and contributing source uses. The LIVE and PTU
producer revisions were `23addc5e164f65a28a6f258302b30453b331edce` and
`93d92ca20bef8a8c3731339214a3301124ade7ba`, respectively. Each comparison
matched 28,359 uses from one physical source. The larger private commit was
1,151,631,360 bytes and the larger working set was 1,161,601,024 bytes.
The strict SDK ledger recognizes these five source tables.

## Recorded comparisons with the complete Catalog

The verifier at `7fcf8ee5ceb64eb2570223e320fe3da1cb0ebd9f` passed
`installed_catalog_sdk_matches_seventeen_tables_in_one_pass` for installed LIVE and PTU.
Both complete Normal Catalogs came from the packaged producer at
`300226ceeeaf522b7d9af790e8fa3698752cf575`. Each comparison bound the installed
Archive to its matching Build and streamed its stored Catalog once. It checked table availability,
ordered row values, and the source uses and physical sources referenced by those rows.

The compared configurations were `cargoLoadingGlobals`, `quantumDrives`, `radarAudio`,
`radarOptions`, `resourceNetworkParameters`, `shieldGenerators`, `shopCommodityGlobals`
and `turretHealth`. The declarations were `itemRoomAssociations`, `resourceNetworkLinks` and
`roomAssociations`. The definitions were `infractionSets`, `infractions`, `jurisdictions`,
`resourceNetworkGlobals` and `securityNetworkManifests`. The projection was
`qualityQuantizationResources`. Both Builds had an empty `infractionSets` table and two
physical sources referenced by the compared rows.

| Build | Rows compared | Source uses compared | Elapsed ms | Peak private bytes | Peak working set bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 15,993 | 2,016,887 | 4,399,018 | 13,502,959,616 | 12,778,622,976 |
| PTU `13248297` | 16,749 | 2,069,882 | 4,319,906 | 13,810,839,552 | 13,098,971,136 |

The SDK coverage registry accepts installed parity for the two cargo globals, the four law
definitions, `qualityQuantizationResources`, `quantumDrives`, `shieldGenerators`,
`turretHealth`, `radarOptions`, `resourceNetworkParameters`, and `resourceNetworkGlobals`
through these same-Build comparisons.

Each run used one worker. The timing and peaks cover the combined verifier and Catalog replay.
For the separate 2 GB SDK acceptance, measure each nominated query in its own cold process.

### ATLS owners and configuration parity

The LIVE `13220821` census read all nine ATLS owner bodies and found no
components that could add rows to `roomAssociations` or `itemRoomAssociations`.
The complete same-Build comparison above therefore covers both declarations
with the expanded ATLS owner selection. All nine owners had an original port
container with network-link members. The complete LIVE `13220821` comparison
covers all 14,024 `resourceNetworkLinks` rows with those owners included.

On PTU `13260598`, the [resource supply comparison](catalog-fuel-capacity.md#installed-sdk-parity)
covered all five tables before the ATLS selection expanded from four to nine owners.
The census read the five newly selected owner bodies and found no components that
could add rows to `fuelTanks`, `fuelIntakes`, `resourceContainers`, `resourceComponents`
or `resourceConnections`. The retained comparison covers all five tables.

### Store slug comparison

The installed Store slug test ran with one worker on Windows AMD64 against the matching
LIVE and PTU Archives. It queried every `/shipStoreSlugs` row, streamed the complete Normal
Catalog from producer `300226ceeeaf522b7d9af790e8fa3698752cf575`, and verified that every
ordered row agreed. The verifier was built from `65f8caf1c2d435c31242dde85d7b16367cfb6fa2`
with the comparator tests. The two executables used different build profiles.

| Build | Rows compared | Elapsed ms | Peak private bytes | Peak working set bytes |
| --- | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 205 | 3,641,070 | 1,511,899,136 | 1,500,471,296 |
| PTU `13248297` | 203 | 1,838,634 | 1,532,948,480 | 1,532,145,664 |

The measurements above include SDK execution and complete Catalog replay.
`installed_store_slugs_cold_unfiltered_query` ran separately for each Build in a new native
Windows AMD64 process with one worker. Each returned the full corresponding row count.

| Build | Rows | Elapsed ms | Peak private bytes | Peak working set bytes |
| --- | ---: | ---: | ---: | ---: |
| LIVE `13220821` (earlier mapping) | 205 | 38,367 | 1,512,280,064 | 1,493,921,792 |
| PTU `13248297` | 203 | 38,613 | 1,531,928,576 | 1,514,065,920 |
| LIVE `13220821` (current mapping) | 227 | 46,036 | 1,571,835,904 | 1,558,986,752 |
| PTU `13260598` | 225 | 58,167 | 1,591,414,784 | 1,581,993,984 |

All four fresh processes stayed under 2 GB and created no scratch files.
The earlier LIVE `13220821` and PTU `13248297` cold executables had SHA-256
`07e7e6cdddec71523c305330088c4d79f7f35f2612b00690e9f9b0be245a35b5`.
The current LIVE `13220821` and PTU `13260598` cold checks used the Store SDK from
`b3c0e21abea216d676b7921c64ef6a45a2153f9b` and had SHA-256
`5273ad4eb7fb673b440cde789239af3b1c92390a1d38cb6c2d5dbf0409768475`.
The installed Archive and manifest matched before and after the standalone query.
The complete ordered Store comparison against current PTU Normal producer
`93d92ca20bef8a8c3731339214a3301124ade7ba` matched all 225 rows. The guarded
run took 4,378.194 seconds, with peaks of 1,590,845,440 bytes of private commit
and 1,579,806,720 bytes of working set. Its executable SHA-256 was
`b582efebce5f6bf1f6040e53946ea38e64dd184f1a57a1597c8f6184bbca2676`.
The current LIVE comparison matched all 227 rows alongside the pledge data.
[Store and pledge evidence](catalog.md#reviewed-store-ship-pledges)

The test `installed_mining_and_store_slugs_match_retained_complete_catalog`
shares one stream of the verified Normal Catalog for the mining consumer and the
complete ordered Store slug array. Set `P4KX_CATALOG_MEASUREMENT_DIR`,
`P4KX_STORE_SLUG_CATALOG_PRODUCER_REVISION` and `P4KX_DATA_P4K` to the matching
measurement, producer revision and installed Archive. Use the standalone cold
queries to measure each workflow's process peak.

### Radar Audio in a separate process

On Windows AMD64, `installed_radar_audio_cold_query` ran the all-Entries Radar Audio
SDK query in a new process with one worker at `8d6cab94e`. Each run returned one
configuration with 98 source uses and two physical sources. The executable SHA-256 was
`263881e9be9a6ed90779c3b89db0329df80bf6defe4b5a7259ed9e8f77b119b9`.

| Build | Elapsed ms | Peak private bytes | Peak working set bytes | Minimum host RAM available in bytes |
| --- | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 3,020 | 953,016,320 | 953,585,664 | 61,809,094,656 |
| PTU `13248297` | 3,342 | 973,185,024 | 973,549,568 | 61,868,814,336 |

The test left the scratch directory empty. System-wide counters recorded 229 pages
read from disk on LIVE and 118,587 on PTU, with zero pages written in either interval.

### Mission definitions, crafting metadata, reputation and factions

The Windows AMD64 verifier compared each group's typed SDK routes with one stream
of the same Build's complete Normal Catalog. It checked availability, ordered
values, original physical Record identity and contributing source uses. The
reputation and faction comparison also checked the selected localized values
and their source contributors.

The row counts below follow these table orders.

- Mission definitions include `missionScenarios`, `missionModuleDeclarations`
  and `missionItems`.
- Crafting metadata includes `craftingGlobal`, `blueprintCategories` and
  `blueprintCategoryLists`.
- Reputation and faction data includes `factionReputations`, `reputationScopes`,
  `reputationStandings`, `reputationContexts`, `reputationPerks`,
  `reputationTriggerRecords`, `reputationTriggers`, `reputationRewardAmounts`,
  `factions` and `factionIdentities`. The LIVE `factionIdentities` declaration
  was absent.

| Group | Build | Ordered row counts | Source uses | Physical sources | Peak private bytes | Peak working set bytes |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| Mission definitions | LIVE `13220821` | 15, 8, 228 | 13,989 | 1 | 1,091,522,560 | 1,022,967,808 |
| Mission definitions | PTU `13260598` | 15, 52, 263 | 21,925 | 1 | 1,114,521,600 | 1,042,501,632 |
| Crafting metadata | LIVE `13220821` | 1, 21, 1 | 324 | 1 | 1,091,538,944 | 1,031,643,136 |
| Crafting metadata | PTU `13260598` | 1, 21, 1 | 324 | 1 | 1,114,558,464 | 1,042,477,056 |
| Reputation and faction | LIVE `13220821` | 39, 46, 380, 27, 2, 3, 39, 58, 60, 0 | 40,700 | 12 | 1,445,834,752 | 1,337,544,704 |
| Reputation and faction | PTU `13260598` | 49, 46, 380, 28, 2, 3, 49, 70, 92, 14 | 47,823 | 12 | 1,495,089,152 | 1,385,889,792 |

The standalone cold measurements each ran one full route in a fresh process
with one worker and no retained Catalog. The table reports the largest
lifetime process peaks over the LIVE and PTU queries in each group.

| Group | Cold runs | Peak private bytes | Peak working set bytes |
| --- | ---: | ---: | ---: |
| Mission definitions | 6 | 1,114,861,568 | 1,055,703,040 |
| Crafting metadata | 6 | 1,114,869,760 | 1,055,703,040 |
| Reputation and faction | 20 | 1,114,898,432 | 1,045,016,576 |

The complete Normal producer revisions were
`23addc5e164f65a28a6f258302b30453b331edce` on LIVE and
`93d92ca20bef8a8c3731339214a3301124ade7ba` on PTU. Mission definitions and
crafting metadata used the signed verifier source
`e21ded16a9b2e72ff04fef458e0daf98a17e0e4d` and executable SHA-256
`c6497b77771f5553b5cde4e04b3d5a5e9a7cea095e3c17a4a812b5590b20deee`.
Reputation and factions used signed verifier source
`6c917387ba663a8f06643d1de9fe64fcbe0d6877` and executable SHA-256
`d8be9805427c93f78fd6090e5d45e03c0fbca9d86eb6abd43c4e135eb9e52b18`.

### World definitions

The retained comparisons use one Windows AMD64 worker and the complete Normal
Catalog for the same installed Build. They compare Starmap objects, types,
presets and amenities, soil definitions, soil texture sets, planet resources
and procedural planet audio in that order. An absent dataset remains distinct
from an available empty dataset. LIVE had no Starmap presets, soils, soil
texture sets or planet resources. Both archives had 30 Starmap object leftovers.

| Build | Ordered row counts | Source uses | Physical sources | Elapsed ms | Peak private bytes | Peak working set bytes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 2,094, 21, 0, 25, 0, 0, 0, 2 | 492,373 | 12 | 1,864,872 | 1,360,826,368 | 1,266,413,568 |
| PTU `13260598` | 2,094, 22, 3, 31, 9, 2, 24, 2 | 496,579 | 48 | 2,004,311 | 1,373,339,648 | 1,267,765,248 |

The retained verifier source was `be4c401b02dbcb91e54396397b439108cb4c3404`.
The producer revisions were `23addc5e164f65a28a6f258302b30453b331edce` on
LIVE and `93d92ca20bef8a8c3731339214a3301124ade7ba` on PTU. The retained
frame digests and installed Archive stamps matched in both runs.

The 16 cold runs opened one route per process with verifier source
`b7863d56c089f4ed19d9fd847b20d933392b7952` and the same two installed Builds.
The largest cold private commit was 1,114,873,856 bytes and the largest
working set was 1,111,523,328 bytes. Each cold run passed the strict
2,000,000,000-byte process limit.

### Economy definitions and mission configuration

The Windows AMD64 verifier at `1114c40cd222c428cc917d05e64f214835b38789`
read installed LIVE `13220821` and PTU `13260598` with the matching complete
Normal Catalogs. The producer revisions were
`23addc5e164f65a28a6f258302b30453b331edce` on LIVE and
`93d92ca20bef8a8c3731339214a3301124ade7ba` on PTU. Each run used one worker
and a 2,000,000,000-byte process cap, preserved the Archive stamp and verified
the retained frame digest.

The economy comparison covered `manufacturers`, `commodities`,
`commodityPresentation`, `cargoManifests`, `gameplayProperties`,
`qualityDistributions` and `qualityQuantizations`. The mission visitor compared
`missionProperties` and `missionFlows` plus the original `contractTemplates`
and `missionBrokerEntries` owner arenas that those rows refer to. Counts follow
those orders.

| Group | Build | Ordered row counts | Source uses | Physical sources | Elapsed ms | Peak private bytes | Peak working set bytes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| Economy definitions | LIVE | 1,237, 170, 170, 81, 29, 21, 62 | 181,145 | 12 | 1,953,847 | 1,308,413,952 | 1,219,248,128 |
| Economy definitions | PTU | 1,239, 170, 170, 81, 29, 21, 62 | 181,282 | 12 | 2,086,697 | 1,321,877,504 | 1,237,241,856 |
| Mission configuration | LIVE | 4,340, 2,925, 497, 2,428 | 6,047,646 | 1 | 2,426,895 | 1,091,637,248 | 1,023,356,928 |
| Mission configuration | PTU | 4,669, 3,039, 611, 2,428 | 6,251,579 | 1 | 2,611,972 | 1,114,521,600 | 1,064,378,368 |

The peaks cover the typed queries and the retained Catalog replay. The
verifier executable SHA-256 was
`0bc4d9a475361f84cb2a24f857bbddccdd08dcb433db6cfbe7055f362182ae3f`.

### Installed Mission Owner Definition visitor parity

The Windows AMD64 verifier at `00e050aa9f487b54a3c4310981f299612e454d7e`
used `visit_mission_owner_definitions` on installed LIVE `13220821` and
compared both tables against the complete Normal frame from
`27da392284eed8c67cf58c15ea473089d6c0a20c`. It matched 497 Contract
Templates and 2,428 Mission Broker Entries in canonical order with their
canonical row payloads. The verifier compared 6,064,858 distinct
contributing source uses and one physical source against that frame.

The exact one-worker process completed in 805.117 s with a 1,112,264,704-byte
peak private commit and 1,043,263,488-byte peak working set. Minimum sampled
host RAM available was 36,252,774,400 bytes. Windows recorded 4,722,132,583
bytes of process write transfers, including temporary comparison data.
These counters include the retained replay. The materialized whole Broker
table and the current PTU frame are separate measurement scopes.

### Loot and Harvest cold queries

The Windows AMD64 verifier at `56e65a9bbd0370283c0d68f1fd3346f822ea6883`
ran seven cold queries against each installed Build. The multi-config query
also covered the slot-preset projection and checked the field-origin markers
on its rows. Each run used one worker, kept the Archive stamp stable and
stayed below 2,000,000,000 bytes.

| Build | Slot preset rows | Inventory uses for multi-config and slot rows | Peak private bytes across all seven queries | Peak working set bytes across all seven queries |
| --- | ---: | ---: | ---: | ---: |
| LIVE `13220821` | 104 | 575,025 | 1,265,541,120 | 1,251,733,504 |
| PTU `13260598` | 106 | 612,834 | 1,331,302,400 | 1,317,150,720 |

### Loot and Harvest LIVE comparison

The Windows AMD64 verifier at `56e65a9bbd0370283c0d68f1fd3346f822ea6883`
compared all eight paths with the sealed LIVE `13220821` Normal Catalog
from producer `27da392284eed8c67cf58c15ea473089d6c0a20c`. The Archive stamp
stayed stable and the retained frame hash matched its measurement receipt.

| Dataset | Matched rows |
| --- | ---: |
| Hierarchical contextual data presets | 0 (Absent) |
| Sub-harvestable configurations | 118 |
| Sub-harvestable multi-config definitions | 104 |
| Loot tables | 54 |
| Loot archetypes | 34 |
| Legacy loot tables | 161 |
| Legacy loot archetypes | 233 |
| Sub-harvestable slot preset projection | 104 |

The comparison matched 688,154 contributing source uses from one physical
source. It took 3,980.811 seconds with peaks of 1,349,705,728 bytes of private
commit and 1,343,963,136 bytes of working set.

### Component definition cold queries

The Windows AMD64 cold verifier ran each of five standalone SDK queries
against LIVE `13220821` and PTU `13260598`. Both Builds returned 12
damage-resistance macros, two mining-laser globals, one turret health modifier,
six consumable types and 83 consumable subtypes. The turret query completed
physical discovery on both Builds. The subtype query resolved ten public names
and ten protected effect descriptions and opened all 11 requested locales on
each Build.

| Build | Peak private bytes across the five queries | Peak working set bytes across the five queries |
| --- | ---: | ---: |
| LIVE | 1,091,608,576 | 1,038,618,624 |
| PTU | 1,114,984,448 | 1,032,249,344 |

Each run used one worker and verified a stable Archive stamp. The cold
executable was built from the test overlay with Git tree
`d245a55901713a9d4953c2b1953e015e29f93abd` over
`7f78af9a2636d137461bab9e101f8cd9bddeed65`.

### Component definition comparisons

Verifier `ac572e896cfaf9e13b27e41f4eabec9164e4c36c` compared all five definition
tables with the sealed same-Build Normal Catalogs from producer
`24c08bcbff2299b88eff84bf3f7e8f592760b99b`. Both matched 104 ordered rows and
11,105 contributing uses from 12 physical sources, including the selected
locale values. Each Archive stayed stable and its retained frame hash matched
its measurement receipt.

| Build | Retained comparison (s) | Peak private bytes | Peak working set bytes |
| --- | ---: | ---: | ---: |
| LIVE `13220821` | 4,012.130 | 1,235,394,560 | 1,134,284,800 |
| PTU `13260598` | 4,063.266 | 1,250,463,744 | 1,149,874,176 |

## Verify dataset coverage

The SDK coverage audit checks canonical dataset boundaries against calls to the public typed
queries. It compares the generated row schemas with the corresponding Catalog schemas. A route
with partial selection or missing parity evidence remains open in the audit.

```text
cargo test --locked -p p4kx-export --test catalog_schema dataset_ownership::sdk_dataset_coverage::every_catalog_v3_dataset_has_a_verified_sdk_query -- --ignored --exact
```

The report names every dataset with an absent witness or outstanding acceptance evidence.
The ordinary schema suite rejects any canonical dataset without a typed query witness,
any unclassified schema boundary and any incompatible row shape. The explicit release
audit also requires complete selection and same-Build parity evidence.
