# P4KX catalog

The P4KX catalog exports authored game data and derived projections as versioned JSON.
The standard marker document identifies itself with `format: "p4kx-catalog-markers"`
and `formatVersion: 1`. Its `catalog` object contains the version 3 data. A sourced
value carries `state`, `value` and `containsEncryptedSource`; the marker remains
`true`, `false` or `null` according to the observed origin.

Use `--provenance full` in the CLI, the full-provenance export action in the GUI,
or `CatalogOptions::with_provenance_mode(ProvenanceMode::Full)` in Rust to write
the standalone version 3 catalog with its source inventory and proof references.
Its JSON Schema is [`schemas/p4kx-catalog.schema.json`](../schemas/p4kx-catalog.schema.json).
The CLI generates the marker document's separate draft 2020-12 schema with
`p4kx catalog schema`. Use `p4kx catalog schema --provenance full` for the
standalone version 3 schema.

The [Rust SDK](catalog-sdk.md) also reads supported catalog data directly from an opened Archive.
`CatalogOptions::starmap_budget` defaults to complete Child traversal without a fixed count quota.
Callers may independently limit physical declarations and instantiated visits per system with
`StarmapTraversalBudget`; exhaustion remains incomplete and normal publication is refused before
output creation. The CLI and GUI select no Child-count allowance.

## System memory

The complete unfiltered Catalog must use less than 40 GB of P4KX memory.
Individual SDK queries and generation of the reduced Basetool catalog must
use less than 2 GB. Use measured application peaks to estimate the headroom
for the operating system and desktop on 64 GiB and 16 GiB systems.
[System memory acceptance](catalog-partitioning.md#system-memory-acceptance) specifies the tests.

## Media files

The catalog carries media references and metadata, such as Archive paths, physical Entry
identities, material settings, animation selection and sound-event bindings. The normal file
and media exporters write texture pixels, 3D model geometry and audio payloads on request,
including the supported converted formats. Catalog references retain their existing path and
metadata types. The Catalog schema has no shared Game-Media annotation for these references.

Use the Archive view's file export in the GUI. In the CLI, use `p4kx extract` for original
Entries, `p4kx texture png` or `p4kx texture merge` for textures, `p4kx model glb` or
`p4kx entity glb` for geometry, and `p4kx audio wav` or `p4kx audio ogg` for supported audio
conversions. `p4kx media export` supports targeted inventories and payload
export through `--kind` and `--filter`. See the [command reference](commands.md) and the [media export contract](media-export.md).

## Versioning and consumers

See the [released field migration](catalog-v2-migration.md) for the Blueprint Category and Gameplay Property mappings.

Version 3 is the only catalog output contract for 0.5.0 (ADR-0339). There is no version-2
export mode, adapter or fallback. Consumers must accept version 3 before importing it. Tagged payload unions are closed: a new variant requires a version decision, unlike
an unknown additive field or kind. The schema describes serialization, so emitted nullable payload
fields are required even when their value is null. The previous released schema is retained as a
historical version-boundary fixture. Moving ResourceTypes to their canonical table and adding typed value
carriers is part of the breaking v3 migration, not an additive change to the released v2 shape.

Every catalog kind and the shared CLI/GUI JSON and compressed catalog writers use version 3.
Canonical definition tables cover Blueprints, BlueprintCategories, ContractGenerators,
ContractTemplates, MissionBrokerEntries, LootPoolFilters, GameplayProperties, QualityDistributions,
QualityQuantizations, Tags, ResourceGroups, ResourceTypes, Commodities and Manufacturers.
Their authored values retain immediate source metadata. Entity GUID-tag declarations and
documented memberships, presentation choices and text rows use their corresponding canonical tables.
Generator logical identities and derived mission texts use sourced projections.
Further contract facts and additional catalog and tool-output carriers remain in progress.
The complete target is [ADR-0356](adr/0356-catalog-v3-uses-canonical-source-typed-tables.md).
Other exported formats retain their independently documented versions.


The implemented canonical families use the [shared eight-field Record header](catalog-record-headers.md),
including exact fileName and separately stateful metadataTag. Header provenance uses the explicit
dataforgeRecordHeader namespace; body paths keep their original DataForge coordinate. Consumers
must resolve document-local SourceUse IDs in the accompanying inventory.

The rule is ADR-0054 (amending ADR-0002):

- **Additive changes keep `schemaVersion`:** a new kind, a new field on an entity, on `source` or
  on an ingredient, a new kind in `completeness`. Example: a `playable` field on every ship.
- **Breaking changes raise `schemaVersion`:** renaming, removing or retyping a field, a changed
  meaning, restructuring a kind, or a changed selection or ordering of an existing kind. Example:
  renaming `crewSize`.
- **A consumer ignores kinds and fields it does not know,** and still rejects a `schemaVersion` it
  does not know. The JSON Schema therefore allows additional properties everywhere; every field it
  lists is required.
- **Field and selection guarantees:** an existing field never
  changes its type, its meaning or whether it can be `null`; a new kind never changes the
  selection or the ordering of an existing kind; the meaning of `completeness` stays.
- **A new source-vocabulary value is additive,** e.g. a new playability value or a new
  `itemClass`. A reader preserves it as unknown or maps it to `null`; it does not infer a known
  meaning or precedence. A vocabulary derived from a Rust enum stays a string with documented
  `x-values` (`value` and optional `description`) and `x-p4kx-vocabularyClass: "open"`.
- **Fixed control states are closed in schema 3** (ADR-0339): publication mode, export/stage
  health, source mode, diagnostic severity/stage/unit, source role and positive Reference-result
  status types reject unknown values. Their definitions carry `x-p4kx-vocabularyClass: "control"`.
  Unknown controls never become successful reads, resolved links or import/deletion permission.
- **Loadouts (#393), paints, item configurations and the starmap are kinds of the same catalog.** The
  `.json.zst` form compresses the complete JSON. See [Compressed form](#compressed-form).
- **Version 2 (ADR-0161)** is breaking against version 1 in one respect: a paint also needs its own
  `RequiredTags` offered by the port or the ship (see "Paints"), so `paints[].shipGuids` lists
  fewer ships than version 1 did (40 fewer on LIVE 4.7). No field changed its name or type. The
  Historical artifacts from those releases use the version-2 schema.
  Version 2 also fixes a port whose name several parts of the vehicle file share (#677): the first
  such part gives the port its sizes, types and flags, as in the tree, so the `Radar_Helper` port
  of eight Ursa ships on LIVE 4.10 no longer lists `start_hidden` in its `flags`.
  Version 2 was released with 0.4.0 and has gained the kinds and fields below since, the last ones
  with 0.4.2. It has the kind `qualityQuantizations` (#692, ADR-0177), the
  ship field `insurance` (#323, ADR-0218), the kinds `mineableCompositions`, `miningGlobalParams`,
  `harvestableProviderPresets`, `subHarvestableMultiConfigs` and `harvestableLocations` (#320,
  ADR-0236), the kinds `factionReputations`, `reputationScopes`, `reputationStandings`,
  `reputationContexts`, `missionTypes`, `missionLocalities` and `contractScenarios` and the
  contract fields `minStandingGuid`, `maxStandingGuid` and `standingRequirements` (#319,
  ADR-0237), and the kinds `lootTables`, `lootArchetypes`, `lootPoolFilters` and `lootContainers`
  and the field `lootConfig` of a sub harvestable (#322, ADR-0238), the kind `mineableEntities`
  (#830, ADR-0242), the kinds `legacyLootTables` and `legacyLootArchetypes` (#818, ADR-0238), the
  contract fields `cooldownLayer`, `hasPersonalCooldown`, `personalCooldownTime`,
  `personalCooldownTimeVariation`, `abandonedCooldownTime` and `abandonedCooldownTimeVariation`
  (#832, ADR-0244), the kind `crafters` (#831, ADR-0243), the contract fields `timeToComplete`,
  `buyInAmount`, `canBeShared`, `illegal`, `onceOnly`, `maxPlayersPerInstance`, `availableInPrison`,
  `canReacceptAfterAbandoning`, `canReacceptAfterFailing`, `hideInMobiGlas` and `requiredIntros`
  (#898, ADR-0263), the kind `cargoManifests` with the contract field `haulingOrders` (#814,
  ADR-0265), and the kinds `missionLocationTemplates`, `contractShipSpawns` and
  `contractLocationQueries` with the contract fields `shipSpawns`, `locationQueries` and
  `locationPrerequisites` (#902, ADR-0266). The following fields also belong to the historical version-2 contract.
  The contract field `fixedRewards` (#888, ADR-0268) is one, and so is `cooldownLayers`
  (#909, ADR-0271), and the contract fields `offeredAt`, `giverFaction` and `blueprintRewards`
  (#915, ADR-0273), and so are the kind `contractTemplates` (#812, ADR-0313) and the kind
  `missionBrokerEntries` with the contract field `brokerEntryGuid` (#813, ADR-0320), and the kinds
  `contractNpcSpawns` and `contractEntitySpawns` with the fields `npcSpawns` and `entitySpawns` of
  a contract, a template and a broker entry (#815, ADR-0324), and the field `value` of the
  `contractProperties` of a template and the `missionProperties` of a broker entry (#1003).
- **A production import takes catalogs of the Channel `LIVE` only.** `PTU`, `EPTU`, `TECH-PREVIEW`
  and `HOTFIX` are confidential test Channels (ADR-0038).

### Game texts

Use each field's `x-gameText` annotation in the generated JSON Schema to identify protected game
text. For a `GameTextRef`, this protection includes the referenced `texts.gameTexts` row and both
forms of each translation. Public short names and UI format templates use `PublicNameRef` and
`texts.publicNames`. One original key used in both domains has two separate references.

Apply text protection independently of `containsEncryptedSource`. The encryption marker
describes the actual Archive source of the datum. See [Texts](#texts) for language selection
and [TextForms](catalog-text-forms.md) for exact values and provenance.

An extensible source vocabulary derived from a Rust enum is a definition of `"type": "string"`
with the known values in `x-values`, one `{"value", "description"}` entry per value (`description`
only where the value has one), e.g. for `FactLayer`:

```json
{"type": "string", "x-values": [{"value": "subContract", "description": "The parameter overrides of the sub contract."}, {"value": "contract"}], "x-p4kx-vocabularyClass": "open"}
```

`x-values` documents the set; it does not validate. Unknown source labels, kind identifiers,
Entry-operation identifiers, diagnostic codes and unresolved-reference reasons remain extensible.
An unknown annotation does not establish a new precedence or positive resolution. Archive-key
state is explanatory metadata; successful decryption and export health are separate evidence.

The fixed control definitions include `CatalogPublicationMode`, `ExportHealthStatus`,
`ExportStageStatus`, `ExportSourceMode`, `ExportDiagnosticSeverity`, `ExportDiagnosticStage`,
`ExportDiagnosticUnit`, `ExportSourceRole`, `DefinitionStatus`, `OwnedReferenceStatus` and
`PropertyReferenceStatus`. Resource origin adds `SourceValueState`, `SourceUseRole`, `InputScope`,
`SourceEncryptionState`, `SourceRecordFormat`, `SourceDeclarationKind` and `ResourceAvailability`. They retain the finite string alternatives generated from their
serializers and carry `x-p4kx-vocabularyClass: "control"`. New alternatives require contract review
and a version decision when they change interpretation. The schema-policy tests require an
explicit class, preserve unknown strings for open vocabularies and reject unknown control states.
This class annotation does not replace counter reconciliation, physical target ownership,
selection completeness or publication checks.

```powershell
p4kx catalog <archive> --out p4kx-catalog.markers.json [--filter <filter>] [--exported-at <seconds>]
p4kx catalog <archive> --provenance full --out p4kx-catalog.json.zst [--zstd-level <level>]
p4kx catalog schema > p4kx-catalog-markers.schema.json
```

### Compressed form

An output name ending in `.zst` (any case) writes the catalog as one zstd frame; a name ending in
`.gz` is refused with exit code 2, because gzip is not written; any other name writes plain JSON,
which stays the default. The zstd encoder is libzstd (ADR-0149). The JSON is serialised straight into the encoder and
never held in memory; it is serialised twice, the first time only to count its length. The frame
uses long-distance matching, carries a content checksum and declares its exact decoded size
(`Frame_Content_Size`). The default compression level is 19. Use `--zstd-level <level>` with a
`.zst` filename to select a level from 1 to 22. Lower levels can encode faster and produce larger
files. Use `--zstd-level 3` to reproduce the full export profiles in the
[catalog size investigation](catalog-partitioning.md#complete-live-and-ptu-measurements).
An explicit level with plain JSON output is invalid. The encoding caps the compression
window at 2^27 bytes. Plain
`zstd -d p4kx-catalog.json.zst` decodes the frame. The decoded
bytes are exactly the plain catalog for the same Archive and export time (tests check this with the
Rust decoder and read the window and content size from the frame header). The file is written to a
temporary file next to the target and renamed on success, so a failure or Ctrl+C leaves an
existing catalog as it was.

The CLI and desktop compare stored and decoded sizes against transfer reference thresholds.

| Size | Reference threshold | Constant |
|------|------:|----------|
| Stored file | 64,000,000 bytes (64 MB) | `UPLOAD_LIMIT` |
| The decompressed content | 1,000,000,000 bytes (1 GB) | `DECOMPRESSED_LIMIT` |

The notices start at 90 % of each threshold. Both encodings are written above the thresholds.
The CLI summary carries decoded size, compressed size (`null` for plain JSON), reference thresholds
and notices in `size`, `compressed_size`, `upload_limit`, `decompressed_limit`, `size_warning`
and `decompressed_warning`. Each importer determines its own transport and resource limits.

## Shape

The file is UTF-8 JSON in one line, ending in a newline. Field names are camelCase and mean the
same in every kind. Listed fields are always present. Existing family shapes use nullable values;
ResourceType source fields instead have the explicit state/value/origin carrier described below.

| Field | Meaning |
|-------|---------|
| `schemaVersion` | The version of the shape, now `3`; an import rejects a version it does not know |
| `source.channel` | The Channel: the folder holding the Archive, e.g. `LIVE` |
| `source.build` | `branch`, `version`, `changeNumber` and `buildId` from the build manifest, or `null` |
| `source.build.publicPatch` | The public patch version, e.g. `4.7.2`, from the RSI Launcher log line `SC <CHANNEL> <version>-<tag>.<change>` (`<version>`: two or three dot-separated numbers, kept as logged) whose change equals `changeNumber`; `null` when the log has no such line (never guessed from a neighbouring one) |
| `source.archiveSize` | The size of the Archive file in bytes |
| `source.archiveModified` | The modification time of the Archive file, UTC in RFC 3339 form |
| `source.archiveKey` | Whether the Archive key was there for the encrypted Entries the export read: `given`, `missing` (no key was set) or `wrong` (the key did not fit). Without a usable key the file is valid but incomplete: vehicle implementations, XML Loadouts, shop inventories and the like are absent |
| `source.encryptedEntriesUnread` | How many distinct encrypted Entries the export asked for and could not read because the key was missing or wrong; `0` when none was needed ([ADR-0353](adr/0353-exports-state-whether-the-archive-key-was-there.md)) |
| `source.toolVersion` | The version of P4KX that wrote the catalog |
| `source.exportedAt` | When the catalog was written, UTC in RFC 3339 form |
| `completeness.<kind>` | `{"status": "complete"}`, or `{"status": "partial", "filter": "Ã¢â‚¬Â¦"}` when a Filter selected the Records; global `tags` remains complete independently of that Filter |
| `exportHealth` | Measured source-read and projection health, directly sourced failed-Record and diagnostic-subject identities with shared text storage, separate Record/Archive Entry counters and bounded diagnostics; independent of Filter completeness ([contract](catalog-export-health.md)) |
| `definitions.tags`, `declarations.tagRoots`, `declarations.entityTags`, `projections.tagPaths` | Global physical Tag definitions, original root/child/Entity GUID arrays, and separately named first-selected tree paths with exact per-value origin; the taxonomy stays complete under an Entity Filter ([contract](catalog-tags.md)) |
| `definitions.missionItems` | Physical authored item definitions, ordered tags and localized variants, original weight and Entity reference ([contract](catalog-mission-items.md)) |
| `definitions.particleLibraries` | Original particle-library XML Entries, ordered elements and attributes, and typed `RainOcclusion` ([contract](catalog-particle-libraries.md)) |
| `definitions.flareLibraries` | Physical lens flare libraries, ordered effect elements, typed parameters and directory-only texture links with per-value provenance ([contract](catalog-flare-libraries.md)) |
| `configurations.radarAudio` | Authored radar audio XML with physical identity, typed fields and original sound event tokens. [Radar audio](catalog-radar-audio.md) |
| `configurations.electricalBehaviors` | Original electrical behaviour Records with ordered lightning effects and target modes. [Weather configurations](catalog-weather.md) |
| `definitions.manufacturers` | Source-family availability and directly sourced Manufacturer rows in GUID order, including typed Localization and canonical text references ([contract](catalog-manufacturer-types.md)) |
| `items` | The items, ordered by GUID |
| `ships` | The ships and ground vehicles, ordered by GUID |
| `definitions.commodities`, `projections.commodityPresentation` | Commodity source identities/authored bindings and explicit derived choices, paired in existing GUID order; see [Commodity contract](catalog-commodity-types.md) |
| `definitions.resourceTypes` | Closed source-family `availability` and canonical `rows`, independently selected and ordered by GUID; every source-backed field uses the typed value carrier ([contract](catalog-resource-types.md)) |
| `texts.publicNames`, `texts.gameTexts` | Canonical sourced-definition short-name and protected-description rows, each with sourced `id`, `key` and per-language `translations` containing separate source and display forms |
| `provenance.sources`, `provenance.uses` | Physical source/ancestry declarations and observed source uses referenced by the sourced definition carriers |
| `definitions.climateAssets` | Original ordered PlanetTech climate assets, sub-assets, growth buckets and declared GUID links ([contract](catalog-planet-assets.md)) |
| `definitions.creatures` | Original PlanetTech fauna definitions with exact Entity GUID links and authored spawn tolerance fields ([contract](catalog-planet-creatures.md)) |
| `definitions.geologyAssets` | Original PlanetTech geology assets with placement, substrate and mineable paths ([contract](catalog-planet-assets.md)) |
| `definitions.planetResources` | Original loose planet resource definitions and declared tint palette paths with physical source and link states ([contract](catalog-planet-resources.md)) |
| `definitions.geologySets`, `definitions.geologyTextureSets`, `definitions.soils`, `definitions.soilTextureSets`, `definitions.shadingClasses`, `definitions.ecosystems` | Original terrain environment definitions, layer and selector fields and sourced links. [Planet terrain definitions](catalog-terrain-definitions.md) |
| `configurations.flightControllers`, `configurations.ifcs`, `configurations.thrusters`, `placements.installedConfigurations` | Original flight configuration and each resolved vehicle Loadout occurrence. [Flight-control configurations](catalog-flight-configurations.md) |
| `definitions.proceduralPlanetAudio` | Physical planet audio records with separately sourced RTPC identifiers and original declaration collections. [Procedural planet audio](catalog-procedural-planet-audio.md) |
| `definitions.resourceNetworkGlobals` | Original global resource-network definitions, power modifier ordering and distribution defaults. [Resource components](catalog-resource-components.md#original-global-resource-network-definitions) |
| `definitions.ammoDefinitions` | Physical `AmmoParams` definitions with original projectile, physics and conversion fields and per-value source origin. [Ammunition definitions](catalog-ammo-definitions.md) |
| `configurations.ammoContainers`, `declarations.ammoBindings` | Physical container settings and authored ammunition and magazine slots, including ordered secondary conditions. [Ammunition definitions](catalog-ammo-definitions.md) |
| `projections.weaponMagazineResolutions` | Observed legacy primary selections, original magazine Record and container indices, and distinct target-read outcomes. [Ammunition definitions](catalog-ammo-definitions.md) |
| `projections.weaponProjectileAmmoSelections` | Physical direct projectile actions with original primary/secondary ammunition for a proven own container and explicit unresolved outcomes. [Ammunition definitions](catalog-ammo-definitions.md#projectile-ammunition-selection) |
| `projections.weaponCombatBases` | Versioned factory alpha, theoretical DPS and nominal flight distance for each physical weapon mode. [Crafting quality and combat values](catalog-combat-quality.md) |
| `definitions.weaponAnimationSources` | Physical Player animation components, archive Mannequin documents, TagDef and FragDef scopes, SubADB selectors, and original options. [Player Mannequin branches and options](catalog-ammo-definitions.md#player-mannequin-branches-and-options) |
| `projections.weaponPlayerAnimationCandidates` | Personal-weapon AttachDef linked to original Player branch and option coordinates by scoped authored tags, with explicit playback state. [Player Mannequin branches and options](catalog-ammo-definitions.md#player-mannequin-branches-and-options) |
| `configurations.weaponActions` | Physical weapon component owners, ordered action slots, launcher selectors, heat inputs, aim and operating modifiers, and original recoil and animation references. [Authored weapon actions and launchers](catalog-ammo-definitions.md#authored-weapon-actions-and-launchers) |
| `definitions.blueprints` | Physical Blueprint Records and their complete original value arenas in GUID order |
| `configurations.durability`, `configurations.degradation`, `configurations.distortion` | Original item component occurrences, raw values and provenance. [Item durability](catalog-durability.md) |
| `configurations.missileConfigurations` | Authored missile guidance, targeting and shake. Join inherited settings by physical slot in `configurations.explosives`. [Missile configurations](catalog-missiles.md) |
| `configurations.emp` | Original EMP timings, radii, effects and state links per physical component. [EMP configurations](catalog-emp.md) |
| `configurations.jumpDrives`, `configurations.jumpDriveFlightTuning`, `configurations.jumpTunnelForces` | Original jump-drive item components with linked tuning and tunnel-force Records, device states and presentation parameters. [Jump drives](catalog-jump-drives.md) |
| `configurations.quantumInterdiction` | Original jammer, pulse and visual graph configuration per physical component. [Quantum interdiction](catalog-quantum-interdiction.md) |
| `configurations.quantumDrives` | Original quantum drive components with independent standard, spline, thermal and boost values. [Quantum drives](catalog-quantum-drives.md) |
| `configurations.cargoLoadingGlobals`, `configurations.shopCommodityGlobals` | Original automatic cargo loading and unloading timing defaults, cargo deck time per SCU and fees by box size. [Cargo loading](catalog-cargo-loading.md) |
| `configurations.lootGeneration` | Original Entity loot components, ordered constraints, theme references and secondary-choice wrappers. [Loot source](catalog-loot-source.md) |
| `configurations.consumables`, `definitions.consumableTypes`, `definitions.consumableSubtypes` | Original composition, units, flags, subtype links and ordered effects by physical source. [Consumables](catalog-consumables.md) |
| `configurations.miningLasers`, `configurations.attachableModifiers`, `definitions.miningLaserGlobals` | Typed mining and attachment effects by physical component. Global laser settings use their own original Record. [Mining and modifiers](catalog-mining-configurations.md) |
| `configurations.suitArmor`, `configurations.clothing`, `configurations.itemRadars`, `definitions.damageResistanceMacros` | Original worn gear and item radar by physical owner. Shared resistance definitions retain their original Reference targets. [Worn gear and item radar](catalog-worn-gear.md) |
| `configurations.turretHealth`, `definitions.turretHealthModifiers` | Original turret health modifier References and ordered damage movement curves. [Turret health](catalog-turret-health.md) |
| `configurations.shieldGenerators` | Original shield generator component occurrences, typed values and provenance. [Shield generators](catalog-shield-generators.md) |
| `configurations.explosives` | Original sensor mine triggers, explosive ordnance, triggerable devices and priming by physical Item and component position. [Explosive components](catalog-explosives.md) |
| `configurations.countermeasureEmitters` | Physical launcher routes with references to canonical ammunition profiles and distinct container counts. [Countermeasure emitters](catalog-countermeasure-emitters.md) |
| `configurations.hackingChips` | Original chip charge counts and ordered removable modifier declarations by physical Item and component slot. [Hacking chips](catalog-hacking-chips.md) |
| `configurations.hackableDevices` | Original target-device timing, error checks and audio declarations by physical Entity and component slot. Hacking state links remain in `configurations.interactions`. [Hacking devices](catalog-hacking-chips.md#hackable-devices) |
| `configurations.jumpPointConfigurations` | Original physical jump-point component parameters. [Jump points](catalog-jump-points.md) |
| `configurations.globalJumpPointConfigurations` | Original global jump-point and tunnel-host setup by physical Record. [Jump points](catalog-jump-points.md) |
| `configurations.blueprintTiers` | Every interpreted tier with cost relationships, normalized values and Research |
| `projections.blueprintPresentation` | Source-backed name, category, product, default availability, dismantle and first-tier ingredient links |
| `definitions.items` | Physical original Item definitions with exact Record headers, source component slots and game/public text references. [Item definitions](catalog-item-definitions.md) |
| `definitions.resourceGroups`, `declarations.resourceGroupMembers`, `projections.resourceGroupParents` | Physical group definitions, every ordered authored membership and the first-selected parent with direct per-value origins ([contract](catalog-resource-groups.md)) |
| `definitions.blueprintCategories` | Source-family availability and physical fieldless category rows with the shared eight sourced header fields in GUID order |
| `declarations.blueprintCategoryLists` | Physical `BlueprintCategoryDatabaseRecord` owners selected at every path by the ordinary Filter, with eight sourced headers and ordered sourced category-reference slots ([contract](catalog-blueprint-category-lists.md)) |
| `definitions.gameplayProperties` | Physical gameplay-property definitions with shared eight-field headers, all four authored fields and local flat transformation trees ([contract](catalog-gameplay-properties.md)) |
| `definitions.qualityDistributions` | Physical distribution and location-override source declarations with exact numbers and immediate per-field origins ([contract](catalog-quality.md)) |
| `definitions.qualityQuantizations`, `projections.qualityQuantizationResources` | Ordered raw bands, explicit eligibility and the genuine first-eligible nominal-GUID resource inverse ([contract](catalog-quality.md)) |
| `definitions.blueprintPools`, `projections.blueprintPoolWeights`, `declarations.blueprintRewardSources` | Physical Blueprint pools, derived weight sums and original declarations that reference those pools |
| `definitions.dismantleProcesses` | Physical generic dismantle process Records with sourced original settings and derived duration |
| `paints` | The paints, each the `itemGuid` of an entry of `items` with the ships it fits, ordered by `itemGuid` |
| `loadouts` | The ports of every ship and of every item that has ports, with the Entity the Default Loadout attaches to each, ordered by `entityGuid` |
| `definitions.starmapObjects` | Authored starmap objects with original links, presentation and navigation settings, ordered by GUID and physical Record index |
| `placements.starmapPlacements` | The selected world placement of each physical starmap object |
| `placements.jumpEndpoints` | Each evidenced jump endpoint occurrence, its local and world poses and the physical SOCPAK gate declarations. [Jump points](catalog-jump-points.md) |
| `projections.jumpConnections` | Availability of authoritative endpoint relationships in the admitted declaration scope. [Jump points](catalog-jump-points.md) |
| `definitions.contractGenerators`, `projections.contracts`, `projections.missionTitles` | Physical Generator definitions, logical contract and sub-contract rows, and their sourced titles and descriptions ([contract](catalog-contract-generators.md)) |
| `definitions.contractScopes` | Physical `ContractScopeGlobalParams` Records with seven individually sourced labels and icons. [Contract scope vocabulary](catalog-location-vocabulary.md#global-contract-scopes) |
| `definitions.subsumptionEvents` | Physical Subsumption XML documents, typed direct event declarations and original XML provenance. [Subsumption event declarations](catalog-subsumption-events.md) |
| `definitions.missionModuleDeclarations` | Physical `ModuleDeclaration` Records and authored location acquisition, spawned entities and variable bindings. [Mission module declarations](catalog-location-configuration.md#physical-module-declarations) |
| `definitions.locationResourceSlots` | Physical `LocationResourceSlot` Records with independently sourced counts, consumption flags and Tag links. [Resource slot definitions](catalog-location-configuration.md#resource-slot-definitions) |
| `definitions.locationEntityDeclarations` | Physical `LocationEntityDeclaration` Records with ordered entity tags, resource-slot links and delivery types. [Location entity declarations](catalog-location-configuration.md#location-entity-declarations) |
| `definitions.missionScenarios` | Physical `MissionScenario` Records with authored variables, cycles, schedule and flags. [Mission scenario definitions](catalog-contract-generators.md#physical-mission-scenario-definitions) |
| `configurations.missionBoards` | Authored mission-board components with physical Entity and component identity, declared board type, and ordered faction reputation links. [Mission location vocabulary](catalog-location-vocabulary.md) |
| `configurations.localContractGenerators` | Ordered authored Handler scope and mission-board availability. [Contract generator sources](catalog-contract-generators.md#local-generator-configuration) |
| `declarations.contractScenarioRequirements` | Ordered authored scenario references at their original Generator, Handler and Contract slots. [Contract generator sources](catalog-contract-generators.md#authored-scenario-requirements) |
| `mineableElements` | Mineable elements with their original tuning values and sourced `resourceType` links, ordered by GUID and physical Record index |
| `definitions.refiningProcesses` | Physical refining processes with sourced labels and complete original field arenas, ordered by GUID and physical Record index |
| `mineableCompositions` | The elements a rock or deposit is made of, with the range and chance of each, ordered by GUID |
| `configurations.miningGlobal` | Physical mining configurations with exact sourced numbers, owned blocks and original references, ordered by GUID and physical Record index |
| `harvestableProviderPresets` | What a place can spawn: groups of harvestables followed to their composition, and areas, ordered by GUID |
| `subHarvestableMultiConfigs` | What the slots of an object can hold by tag, ordered by GUID |
| `harvestableLocations` | The containers whose `.soc` names a provider preset, with the starmap object each hangs below, ordered by preset, container, system and object |
| `shops` | The shops of the shop registry with kind, brand, location and the link to a starmap object, ordered by `key` |
| `definitions.factionReputations` | Physical faction reputation records, presentation, relations, blackboard values and original perk-list links |
| `definitions.reputationScopes` | Physical reputation scope definitions with authored presentation and ordered standing maps |
| `definitions.reputationStandings` | The physical reputation standings with authored presentation and progression fields, ordered by GUID and original Record index |
| `definitions.reputationPerks` | The physical perk lists with ordered original scope/standing associations and owned reward or category data |
| `definitions.reputationTriggerRecords` | Reusable physical sandbox trigger Records with ordered manual trigger definitions |
| `definitions.reputationRewardAmounts` | Physical reputation reward Records with their authored names and signed amounts |
| `definitions.infractions` | Physical infraction definitions with ordered source triggers and authored defaults. [Law definitions](catalog-law.md) |
| `definitions.infractionSets` | Physical reusable sets of ordered infraction links and overrides. [Law definitions](catalog-law.md) |
| `definitions.jurisdictions` | Physical jurisdictions with authored parent links, fines, limits, prison and penalty configuration. [Law definitions](catalog-law.md) |
| `definitions.npcArchetypes`, `definitions.npcClassPools`, `definitions.autoSpawns`, `definitions.autoSpawnSets` and `definitions.encounterIndices` | Physical NPC and encounter definitions with ordered identity branches, weighted entries, class alternatives and declared References. [NPC spawn definitions](catalog-npc-spawn-definitions.md) |
| `definitions.locationClasses`, `definitions.locationFunctions`, `definitions.locationSizes` and `definitions.locationRegistries` | Physical definitions and ordered registry members for mission location classification. [Location vocabulary](catalog-location-vocabulary.md) |
| `definitions.reputationContexts` | Physical reputation contexts with authored scope order, original references and display properties |
| `definitions.missionTypes` | Physical mission-type definitions, linked by `contracts[].missionTypeGuid`, in GUID order |
| `definitions.missionLocalities` | Physical locality Records with ordered original location references and per-field origins |
| `contractScenarios` | The contract scenarios with the reward tiers of each faction, ordered by GUID |
| `cargoManifests` | `definitions.cargoManifests`, the physical cargo manifests with their fill and spawn rules |
| `missionLocationTemplates` | The mission location templates with the tags a location query matches, ordered by GUID |
| `contractShipSpawns` | The distinct ship spawn descriptions of the contracts, ordered by identifier |
| `contractLocationQueries` | The distinct location queries of the contracts, ordered by identifier |
| `contractTemplates` | The contract templates (`ContractTemplate`), the targets of `contracts[].templateGuid`, ordered by GUID |
| `missionBrokerEntries` | The mission broker entries (`MissionBrokerEntry`), the targets of `contracts[].brokerEntryGuid`, ordered by GUID |
| `contractNpcSpawns` | The distinct NPC spawn descriptions of the contracts, templates and broker entries, ordered by identifier |
| `contractEntitySpawns` | The distinct entity spawn descriptions of the contracts, templates and broker entries, ordered by identifier |
| `missionPropertySets` | The authored Generator property arrays, ordered by stable source identifier; contract rows join them through `propertySetGuids` |
| `lootTables` | The loot tables (`LootTableV3Record`) with the weighted archetypes of each, ordered by GUID |
| `lootArchetypes` | The loot archetypes (`LootArchetypeV3Record`) with the weighted selectors of each, ordered by GUID |
| `definitions.lootPoolFilters` | Complete original pool-filter fields and values, ordered by physical source GUID |
| `definitions.lootSecondaryChoiceSingleLayers`, `definitions.lootSecondaryChoiceMultiLayers`, `definitions.lootThemes` | Original ordered choices, layers and owned theme values with physical Record identities |
| `definitions.hierarchicalContextualDataPresets`, `definitions.lootGenerationGlobalParams` | Original context arrays, selectors, category order and global parameter fields |
| `definitions.subHarvestableConfigs`, `definitions.subHarvestableMultiConfigs` | Full physical harvestable configurations with original references and provenance |
| `legacyLootTables` | The older loot tables (`LootTable`) with the weighted archetypes of each, ordered by GUID |
| `legacyLootArchetypes` | The older loot archetypes (`LootArchetype`) with the weighted tags of each, ordered by GUID |
| `lootContainers` | The containers placed with a multi config, with the starmap object each hangs below, ordered by config, container, system and object |
| `definitions.mineableEntities` | The physical mineable rocks and deposits with original mining and radar declarations, ordered by GUID and Record index |
| `definitions.crafters`, `projections.crafterHosts` | Authored crafting stations and the derived physical default Loadout mounts. |
| `shipStoreSlugs` | The RSI store slugs that map to a ship of `ships`, with the retired ones flagged, ordered by `slug` (curated data, ADR-0117) |
| `shipStorePledges` | Reviewed non-flight-ready Store pledges in ascending RSI ship ID order with their shared source and scope (ADR-0117, ADR-0245) |
| `configurations.craftingGlobal` | Not a kind: original physical crafting-global configurations |
| `configurations.itemRecoveryEconomy`, `configurations.itemRecoveryOverrides` and `configurations.itemRecoveryPolicy` | The selected recovery policy's economy parameters and original override declarations. See [Item recovery rules](#item-recovery-rules). |

An unfiltered kind with complete catalog-wide `exportHealth` lists its successfully read sources,
so an import can use absence as removal evidence within that kind's selection policy. A partial
kind lists only what the Filter selected. Incomplete health, including global blocking diagnostics,
cannot establish removal even when a kind's Filter scope is complete.

### Per-value source origin for canonical definitions

`definitions.resourceTypes.rows` replaces the root `resourceTypes` array. Commodity facts and choices
use `definitions.commodities.rows` and `projections.commodityPresentation.rows`; the old root
`commodities` view is removed. See [Commodity mapping](catalog-commodity-types.md). There is no duplicate legacy view. Its source-backed members and nested authored list elements carry five required keys:

| Key | Meaning |
| --- | --- |
| `state` | `present`, `absent`, `null`, `unreadable`, `unsupported` or `invalid` |
| `value` | Typed present payload, otherwise null; false, zero and empty values stay present |
| `containsEncryptedSource` | True for any proven encrypted contributor/ancestor; false only for a complete all-unencrypted set; null otherwise |
| `sourceUsesRefs` | Opaque document-local references to actual contributing `provenance.uses` rows |
| `sourceSetComplete` | Completeness of this value's input set, independent of Filter and export health |

A complete input set may have unknown encryption, and an incomplete set may contain a known
encrypted contributor. Successful decoding, an Archive key or a plaintext standalone DCB does not
prove an unencrypted original source. Payload, selector and conversion uses contribute; attempted
reads and untouched reference targets do not. Sources keep distinct physical identity even when
paths, GUIDs or decoded bytes are equal. Nested container ancestry affects the result. Directory
metadata and Entry payload are separate scopes.

Every `provenance.uses` row requires `effectiveEncryption`: `encrypted`, `unencrypted` or
`unknown`, captured for that original use's byte domain. It includes the applicable containing
Entries and can differ from the leaf's `storedEncryption`. Consumers can check a value's direct
marker against these contributing-use facts; the inventory does not replace per-value markers.

Fixed Single resource numbers retain `rawBits` (eight lowercase hexadecimal digits) plus nullable
`finite`; Double payloads use sixteen digits. Infinity and NaN remain present, including their
original bits. Generic numeric fields add the source-width `type` tag; Int64/UInt64 values use
bounded canonical decimal strings. The exact payloads, states and reference contracts are in the
[ResourceType contract](catalog-resource-types.md#immediate-value-and-origin-carrier).

Name and description values reference separate canonical text tables. Their `translations.value`
maps each actually available supported language directly to `TextForms`: independently sourced
`sourceText` and `displayText`, without a redundant language-group carrier. Source text preserves
the exact winning decoded INI value; display text keeps the existing escape decoding and trimming.
An empty winner stays present in both forms; padding remains in source text while display text is
present and empty. Missing keys and placeholders have two absent forms. A missing language Entry
creates no map key. The [TextForms contract](catalog-text-forms.md) defines exact inputs, summary
accounting and consumer paths. Game-text protection remains independent of source encryption;
consumers cannot infer per-value proof from the global `source.archiveKey` or unread-entry counts.

This coverage is implemented for ResourceTypes, Commodities and Manufacturers, their canonical
text rows and Commodity presentation. Other families and
structured/raw tool outputs still require their own migration under
[#1125](https://github.com/p4kx-org/p4kx/issues/1125); the presence of `provenance` is not proof that
all current output fields have origin metadata.

### Ordering Builds

`source.build.changeNumber` is the Perforce change number the Build was made from
(`RequestedP4ChangeNum` of the build manifest). It is a decimal integer written as a string; a
consumer compares it as a number, never as text.

- **Within a Channel it is monotonic.** Every Build a Channel published has a higher change number
  than the one before, so a consumer may refuse a catalog whose `changeNumber` is lower than that of
  the last applied catalog of the same Channel. The same number means the same Build; re-applying it
  is safe.
- **HOTFIX** is a Channel of its own and monotonic too. A HOTFIX Build later becomes the LIVE Build
  unchanged, so HOTFIX and LIVE then share one change number, one `buildId` and one `version`. A
  HOTFIX Build lies between the LIVE Builds around it, and the LIVE Build that replaces it is the
  same or higher, so ordering a HOTFIX catalog against a LIVE one by `changeNumber` is also sound
  as long as equal numbers count as the same Build.
- **Across Channels it orders the code, not the time of publication.** PTU, EPTU and TECH-PREVIEW
  Builds come from other branches: a LIVE Build with a lower change number can be published after a
  PTU Build with a higher one. A consumer keeps one last-applied `changeNumber` per
  `source.channel` and does not compare numbers of different Channels. A Build promoted to another
  Channel keeps its number, so it is never rejected there.
- **`buildId` does not order.** It is a random version-4 UUID. It identifies a Build for equality
  only.
- **`version` does not order well.** The first three parts follow the Builds, but the fourth is a
  counter of the build farm, so a later Build can carry a lower fourth part than an earlier one.
  Use it for display, not for order.

Evidence for the rule is in the pull request that closed p4kx-org/p4kx#402: launcher log lines
(`SC <CHANNEL> <version>-<tag>.<change>`, the same lines as for `source.build.publicPatch`) with no
inversion in any Channel, and the installed manifests. The logs rotate, so the claim covers the
Builds the log kept; a rollback of a Channel to an older change would break it and would show as a
lower number than the last applied one.

### Entities

Entity families expose the following information where it applies. Their sections below specify
the owning tables and source carriers.

| Field | Meaning |
|-------|---------|
| `guid` | The Record GUID as canonical lower-case UUID text; entities are ordered by it |
| `className` | The Record name without its type, e.g. `ACAS` for `SCItemManufacturer.ACAS` |
| `recordPath` | The record path, without `.xml` |
| `name`, `description` | Text references where documented for the family. The original key and language forms are in the canonical registry. |

A resource group carries `name` and protected `description`.

Manufacturers use [their canonical definition contract](catalog-manufacturer-types.md). The authored
`Code` string and owned `Localization` retain their source states independently of resolved text.

An Item projection keeps the shared eight-field, immediately sourced Record header. Read
`guid.value`, `className.value`, `recordPath.value` and `recordIndex.value`; use each cell's own
state and origin. [Item definitions](catalog-item-definitions.md#item-presentation) documents the
physical source joins. A GUID or path can name more than one physical declaration.

| Field | Source |
|-------|--------|
| Shared Record header | Original metadata and conversions; the same sourced cells appear on the corresponding `definitions.items` row |
| `selection.attachmentComponentOrdinal` | Sourced original slot of the first `SAttachableComponentParams` |
| `selection.physicsComponentOrdinal` | Sourced original slot of the first `SEntityPhysicsControllerParams` |
| `selection.purchasableComponentOrdinal` | Sourced original slot of the first `SCItemPurchasableParams` |
| `itemClass` | Sourced classification of the actual English description; known values are `Civilian`, `Competition`, `Industrial`, `Military` and `Stealth` |
| `playability` | Sourced `player`, `mission` or `special`, or a non-present result; see "Playability" below |
| `variantOf` | Physical base choice and original comparison sources, computed over the unfiltered Build |

The root does not duplicate `type`, `subType`, `size`, `grade`, `mass`, `manufacturerGuid` or
`tags`. Join `recordIndex.value` to `definitions.items.rows[].recordIndex.value` and follow the
corresponding sourced component ordinal. The selected attachment's original `itemType`,
`itemSubtype`, `size`, `grade`, `manufacturer`, `tags` and `requiredTags` are in its definition
payload. The selected physics payload contains the original `mass`. Attachment constraints
retain their separate admissibility and restriction meaning; a foreign authored AttachDef can
have original fields without a row in `configurations.itemAttachmentConstraints`.

Selection always identifies the first physical family component, including a null, invalid or
unreadable definition. It never skips forward to a later successful value. An earlier unknown
component keeps the selection and any dependent result's source set incomplete. Negative
selection, a present component with null payload and unreadable source remain distinct.

`itemClass.value` uses the options of `EItemClass` other than `UNDEFINED`. No DataForge field
carries this classification: the rule reads the first English display-text line beginning with
`Class:`, trims its value and matches ignoring case. Other text, no matching line or a genuinely
absent English description produces no class. The original Description selector, actual English
lookup/display conversion and component-selection evidence contribute directly to that result.
A missing English input must be established by an actual admission and directory proof; an
unconsulted language is not absence. Requested output languages do not change this semantic
English dependency. Key-only playability uses the original Locale tokens and Record path,
without adopting unrelated translation payloads.

`itemClass` and `playability` use the shared sourced-cell envelope. Their present value domains
remain open strings in the schema. Read state, value, source completeness and encryption
independently. Use [Variant groups](#variant-groups) for the separate `variantOf` contract.

#### Semantics of `grade`

The attachment definition's `grade.value` retains the original numeric storage and value of
`AttachDef.Grade`; the catalog carries no letter.
The letter the game shows is not in the DataForge. For ship components it is the `Grade:` line of
the English `description`, and it follows the integer: **1 = A, 2 = B, 3 = C, 4 = D**, so A is the
lowest integer. Checked on LIVE 4.10.2 (change 12913470) against that line of every item that has
one:

| Item `type` | Integer to letter |
|-------------|-------------------|
| `Cooler`, `PowerPlant`, `QuantumDrive`, `Radar`, `Shield` | 1 = A, 2 = B, 3 = C, 4 = D, uniform |
| `JumpDrive`, `LifeSupportGenerator` | only 3 = C is consistent; grade 1 items name no letter or contradict it |
| FPS gear (`Char_Armor_*`, `Char_Clothing_*`, `WeaponPersonal`) | no `Grade:` line; grade is 1, on a few helmets and shirts 2, with no letter evidence |
| `WeaponAttachment` (ship weapon parts) | no `Grade:` line; grade 3 to 7 follows the size of the part |
| every other `type` | no `Grade:` line; grade is 1 almost everywhere |

Of the 350 items with a `Grade:` line, 331 name A to D and agree with the integer. 19 do not, and
all 19 have grade 1:

- 10 name B or C: a `Cooler` (B, and another with C), a `Radar`, a `JumpDrive`, a
  `QuantumInterdictionGenerator`, two `Misc` computers and three `WeaponMining` lasers. The text and
  the integer disagree; the Archive does not say which one the game shows.
- 9 name `Bespoke`: the ship-specific components of the Reclaimer, 890 Jump and Polaris.

For the five component types, `grade` maps `1 = A` through `4 = D`. Treat grade 1 as unknown
elsewhere and for those 19 items. The `Grade:` line is protected game text.

Distribution of `grade` over the 23280 items of LIVE 4.10.2:

| `grade` | Items |
|---------|-------|
| 1 | 21325 |
| 2 | 153 |
| 3 | 170 |
| 4 | 76 |
| 5 | 8 |
| 6 | 8 |
| 7 | 4 |
| `null` | 1536 |

`null` is an item without `AttachDef`: all 1536 also have a `null` `type`. 5 to 7 occur only on
`WeaponAttachment` parts (`Barrel`, `FiringMechanism`, `PowerArray`, `Ventilation`), 3 and 4 on
those parts too. No value is below 1 or above 7.

#### Item stats

The catalog exposes component facts in canonical tables. Join an Item definition's
`recordIndex.value` in `definitions.items` to `ownerRecordIndex.value` in a component table.
`componentOrdinal.value` identifies the original slot within the same physical Record. A logical
GUID can match several physical Records. The examples below use JSON paths through sourced cells.
A sourced field keeps its own encryption marker and source references. Numeric fields preserve
their stored unit, finite numeric value and exact binary representation, including nonfinite values.

| Domain | Canonical tables |
| --- | --- |
| Health and durability | [`configurations.durability`](catalog-durability.md) |
| Shields and quantum drives | [`configurations.shieldGenerators`](catalog-shield-generators.md) and [`configurations.quantumDrives`](catalog-quantum-drives.md) |
| Weapons, ammunition and magazines | [`configurations.weaponActions`, `configurations.ammoContainers`, `declarations.ammoBindings` and `definitions.ammoDefinitions`](catalog-ammo-definitions.md) |
| Missile targeting, guidance and explosion | [`configurations.missileConfigurations` and `configurations.explosives`](catalog-missiles.md) |
| Suit armor, clothing and device radar | [`configurations.suitArmor`, `definitions.damageResistanceMacros`, `configurations.clothing` and `configurations.itemRadars`](catalog-worn-gear.md) |
| Mining lasers, modules, gadgets and salvage attachments | [`configurations.miningLasers`, `definitions.miningLaserGlobals` and `configurations.attachableModifiers`](catalog-mining-configurations.md) |
| Resource flows and signatures | [`configurations.resourceComponents`](catalog-resource-components.md) |
| Derived weapon base values and quality calculations | [`projections.weaponCombatBases` and the versioned formula contract](catalog-combat-quality.md) |

The tables retain separate ordered action nodes and authentic weapon component slots. A fire slot's
value identifies an entry of the same row's `actionNodes`. Wrapper nodes identify their own child slots
and preserve their own inputs. Ammunition choices and capacity use the physical container chosen by
[`projections.weaponMagazineResolutions`](catalog-ammo-definitions.md#physical-containers-and-slots),
then the matching container declaration and the exact linked Ammo Record. Capacity belongs to
`configurations.ammoContainers.rows[].maxAmmoCount`. The definition stores baseline projectile
speed and lifetime, bullet damage, and projectile detonation damage and radii.

##### From a gameplay property to a stat field

A `CraftingGameplayPropertyDef` describes a property without referencing an Item component field.
Resolve a modifier's original `gameplayPropertyRecord` Reference by its physical target index in
`definitions.gameplayProperties`. The definition's `className.value` selects the mapping below.
Use the versioned formula to combine contributions for the selected physical value in its documented
unit. A missing component or field leaves the corresponding value unavailable. [Formula contract](catalog-combat-quality.md)

In the table, component fields are relative to the row of the named table. A node body is the
selected `configurations.weaponActions` row's `actionNodes[].kind.value` for the applicable action.

| Property class | Canonical inputs |
| --- | --- |
| `GPP_Armor_DamageMitigation` | Join `suitArmor.damageResistance.value.targetRecordIndex.value` to `damageResistanceMacros.source.recordIndex.value`. Each applicable `damageResistance.value.<channel>.value.multiplier` retains the original value. |
| `GPP_Armor_TemperatureMin`, `GPP_Armor_TemperatureMax` | `clothing.temperatureResistance.value.minResistance` and `maxResistance`. |
| `GPP_Armor_RadiationCapacity`, `GPP_Armor_RadiationDissipation` | `clothing.radiationResistance.value.maximumRadiationCapacity` and `radiationDissipationRate`. |
| `GPP_Health_MaxHealth` | `durability.health`. |
| `GPP_Weapon_Damage` | Bullet damage from the selected Ammo definition's `projectileParams.value.kind.value.damage`, or the beam node's `damagePerSecond`. |
| `GPP_Weapon_FireRate`, `GPP_Weapon_Spread` | Node `fireRate` and `launchParams.value.spreadParams`. Charged actions link to child nodes by `weaponAction.value`. A beam's original spread declaration remains in its authored fields when present. |
| `GPP_Weapon_Recoil_Kick`, `GPP_Weapon_Recoil_Handling`, `GPP_Weapon_Recoil_Smoothness` | Authored recoil settings and their linked data. A scalar reduction requires a consumer rule. |
| `GPP_Weapon_ReloadSpeed` | Authored weapon motion inputs. A single elapsed time requires a consumer rule. |
| `GPP_ItemResource_PowerGeneration`, `GPP_ItemResource_CoolantGeneration` | `resourceComponents.states.value[].value.deltas.value[].value.generation` for `Power` and `Coolant`. Apply the property to its own stored amount and unit. |
| `GPP_Shield_MaxHealth` | `shieldGenerators.maxHealth`. |
| `GPP_Radar_MinAimAssistDistance`, `GPP_Radar_MaxAimAssistDistance` | `itemRadars.aimAssist.value.distanceMinAssignment` and `distanceMaxAssignment`. |
| `GPP_Quantum_Speed`, `GPP_Quantum_FuelRequirement` | `quantumDrives.standard.value.driveSpeed` and `quantumDrives.fuelRequirement`. |
| `GPP_Weapon_Tractor_Force`, `GPP_Weapon_Tractor_MaxDist`, `GPP_Weapon_Tractor_FullStrengthDist`, `GPP_Weapon_Tractor_MaxVolume` | Tractor beam node `maxForce`, `maxDistance`, `fullStrengthDistance` and `maxVolume`. |
| `GPP_Weapon_HullScraping_Radius`, `GPP_Weapon_HullScraping_Speed`, `GPP_Weapon_HullScraping_Efficiency` | `attachableModifiers.modifiers.value[].value` weapon branch. Its `value.weaponModifier.value.weaponStats.value.salvageModifier.value` holds `radiusMultiplier`, `salvageSpeedMultiplier` and `extractionEfficiency`. |
| `GPP_Crafter_CraftSpeed`, `GPP_Crafter_DismantleEfficiency` | These properties apply to crafting stations. |

Armor damage mitigation has one input modifier. The source keeps six independent resistance channel
values and provides no rule for choosing affected channels. For resources, the authored order and the
separate consumption and generation sides stay visible. The consumer can reproduce the former flat
power or coolant summary by choosing the first applicable generation amount in original state and
delta order. Match the resource identifier case-insensitively and keep the matching source unit.

### Ship DCB source fields

Each Ship has the shared [eight-field Record header](catalog-record-headers.md). Read
`recordIndex.value` for physical joins within the original DataForge, including Records with
equal GUIDs and paths. Header fields and selected source values carry
their own immediate origin cells.

A ship takes its text fields from the first exact `VehicleComponentParams` in the original
`Components` array. Each field is a sourced reference. Use its `value` to join the matching
`id.value` in the indicated text table. The table carries separate `sourceText` and `displayText`
forms for every available language. An empty key yields an absent reference. A placeholder keeps
its key and has absent translation forms.

| Field | Source |
|-------|--------|
| `name` | `vehicleName` → `texts.publicNames`. When the vehicle name is absent for a supported exosuit Actor, use the original `AttachDef.Localization.Name` on its selected attachment component. |
| `role`, `career` | `vehicleRole`, `vehicleCareer` → `texts.publicNames` |
| `description` | `vehicleDescription` → `texts.gameTexts` (`x-gameText`) |
| `crewSize` | Sourced original `crewSize` as ResourceNumber, independently of CrewManifest membership |
| `vehicleManufacturer` | The selected `VehicleComponentParams.manufacturer`, with its original component ordinal |
| `attachmentManufacturer` | The independently authored `AttachDef.Manufacturer`, with the original attachment component ordinal and AttachDef state |
| `vehicleDefinition` | Sourced original String/Enum selector, including whitespace, case, separators and empty values. The Archive reader interprets it for lookup. |
| `playability` | Sourced `player`, `ai`, `mission` or `special`; see "Playability" below |
| `variantOf` | Physical base choice and original comparison sources, computed over the unfiltered Build |
| `vehicleKind` | Sourced `spaceship` or `groundVehicle`, or an absent kind. A readable fallback can carry incomplete source evidence. |
| `size` | Sourced original `Components/SAttachableComponentParams/AttachDef/Size` as ResourceNumber with explicit absent, null and unreadable states |
| `mass` | Immediately sourced hull mass in kilograms; use `value` when `state` is present. A positive total rounding to zero stays present. See "Ship base data" below. |
| `massBasis` | Sourced selection of `parts` or `physics`; present exactly when `mass` is present. |
| `hullHealth` | Immediately sourced structural hull hit points; see "Ship base data" below. |
| `insurance` | Sourced first `SEntityInsuranceProperties` declaration and its independent parameter state |
| `playabilityInputs` | Selected original `crewManifestComponentOrdinal` and authored `crewManifest` Record link with target header state |

#### Ship insurance

`insurance` selects the first exact `SEntityInsuranceProperties` in the original
`StaticEntityClassData` array. Its sourced state distinguishes a known missing declaration
from an unreadable possible earlier candidate. A present declaration retains the original
`staticElementOrdinal`, sourced `restoreOnClaim` and sourced `shipInsuranceParams`. A null or
unreadable parameter pointer belongs to the same selected declaration.

| Field under `insurance.value` | Source |
| --- | --- |
| `staticElementOrdinal` | Original zero-based StaticEntityClassData array position |
| `restoreOnClaim` | Original Bool, including a present false |
| `shipInsuranceParams` | Original Class or StrongPointer container and its independent state |
| `additionalFields` | Unprojected or rejected original declaration members |

| Field under `insurance.value.shipInsuranceParams.value` | Source |
| --- | --- |
| `shipEntityClassName` | Original String/Enum, including a present empty value |
| `baseWaitTimeMinutes` | Original ResourceNumber in minutes |
| `mandatoryWaitTimeMinutes` | Original ResourceNumber in minutes |
| `expediteFee` | Original `baseExpeditingFee` ResourceNumber in aUEC |
| `additionalFields` | Remaining original parameter members, preserving exact numeric bits |

All independent fields are sourced cells. A present parameter container carries its child
values. Earlier unknown static candidates remain visible on the selected declaration,
parameter container and typed leaves. Source-set completeness is independent of the encryption
marker. The wait and restore meanings are inferred from source names. Related multipliers are
in the [item recovery configuration tables](#item-recovery-rules).

#### Ship base data

`size` is the exact sourced DCB value described above. `mass`, `massBasis` and
`hullHealth` use the five-field immediate source carrier. Read each cell's `state` and `value`.
Their numeric payloads and rounding use the same `p4kx_export::ShipReader` calculation as
`p4kx ships` ([ship-base-data.md](ship-base-data.md), ADR-0061). The totals are calculated once;
the opt-in observer captures the actual Parts, Modification, patch, override and eligibility
inputs from that same interpretation. The raw `vehicleDefinition` remains separate from the
normalized Archive lookup path.

Known empty Parts and missing optional attributes yield complete absent totals when the fallback
also establishes absence. An unreadable Vehicle, patch or override yields `unreadable` for a
metric whose value could not be computed. Available results stay present with incomplete source
sets when a requested input is unreadable. An authored numeric field that cannot be interpreted
yields `invalid` when its metric has no usable value or fallback. Missing original XML
coordinates affect `sourceSetComplete` independently of the value state. The encryption marker includes every known
contributing encrypted payload. Failed reads are recorded as attempts.

Each aggregate joins its actual current physical Ship caller with the original selected file,
Parts membership and the numeric and selector fields that influenced that metric. Parts mass
includes the consulted Parts values and selectors. Physics fallback includes the rejected Parts
choice and the original Physics conversion. `massBasis` uses selection evidence. Hull health
uses its own damage and structural-port eligibility inputs. Each physical Record retains its
own DCB contributions, including when its XML is reused or its GUID or path is shared.

Cargo is an interpreted projection in `projections.shipBaseFacts`. The Ship row carries
the interpreted `vehicleKind`.
The standalone `p4kx ships` output reports detailed sources, thresholds, Loadout and cells.
The payloads of `vehicleKind` and `massBasis` are extensible strings in the schema.
The `exosuit` code covers the ATLS, ATLS GEO, ATLS IKTI, ATLS IKTI GEO, ATLS IKTI Akuma,
ATLS IKTI Rad and the three ATLS GEO Collector Actor owners.
It requires the matching physical class, authored control components and the vehicle's
driver seat in the original default loadout. [ADR-0417](adr/0417-exosuits-share-the-vehicle-catalog.md)
defines the admission rule.

The manufacturer remains the original reference on each selected attachment component.
The ATLS stores null, the GEO and three Collector owners target `ARGO`, the IKTI,
IKTI GEO and IKTI Rad target `BANU`, and the IKTI Akuma targets `UNKN`.

The three Collector owners each have a separate playable row. They resolve the same
original driver seat as the GEO and each declares the GEO class in
`insurance.shipInsuranceParams.shipEntityClassName`. The original insured class name and
its source are part of the insurance data. The Ship variant decision uses vehicle
implementation membership.

An original source check on LIVE `13220821` and PTU `13260598` found one readable
physical Record per GEO and Collector owner. It opened four Actor bodies and one
driver-seat body per Build. All four owners satisfied the same GEO control predicate.
Each Collector owner had its own full-name key, and all four shared the GEO short-name key.

Eighteen isolated comparisons verified all nine owners on LIVE `13220821` and
PTU `13260598`. Each canonical SDK selection matched one exact Normal Catalog row
with equal source-use closure and unchanged installed Archive identity. The comparisons
also resolved each original name key and validated the original manufacturer state
or Reference target. The maximum observed private commits were 1,377,472,512 bytes
on LIVE and 1,409,744,896 bytes on PTU. Peak working sets were 1,238,863,872 and
1,271,222,272 bytes respectively.

A scoped verification at revision `54d5e8d00` matched all four owners on LIVE `13220821`
and PTU `13248297`. The SDK ship-source reader and scoped Normal Catalog agreed on
Record index, GUID, class name, Record path, vehicle kind and playability. The
comparison included physical source references and encryption states for the
classifications. The standalone vehicle output returned the same owners and kind.

A second scoped comparison at `152e7c2ad` verified each of the four physical owners on the
same LIVE and PTU Builds in a separate process. Normal Catalog, the ship-source SDK,
and both `p4kx ships` formats agreed on the six identity and classification values and
their physical source evidence. The standalone JSON and NDJSON forms also agreed on
all ten emitted row-relative origins and their original Archive Entry ancestry.

At revision `9e2ecedea6b60d0b45a98dc5323a7d592dcabaef`, 16 fresh-process checks
covered the four owners on LIVE `13220821` and PTU `13248297`. Each owner passed
the cold SDK query and matched the exact scoped Normal Catalog row, field origins
and physical references in a separate parity check. The highest observed private
commit was 1,410,633,728 bytes, and the highest working set was 1,267,974,144 bytes.
The installed Archive identity stayed stable in all checks.

A header census of older EPTU `13025019` and TECH-PREVIEW `13040440` found all four original
owners by GUID and Record path. It visited 117,952 and 120,857 headers respectively, with every
path classifiable and zero Record bodies opened. Playability on the older Builds requires
their authored control and driver-seat evidence.
The absent-owner acceptance uses two DataForge fixtures for the same physical GEO
identity. One has no matching Record and permits Normal publication. The other has an
unreadable body with a readable header. Normal publication refuses it, Diagnostic Partial
retains its original GUID and Record index, and the SDK reports that index in its body
read coverage. The installed and retained older Builds supply positive presence evidence.

| Field | Rule and source |
|-------|-----------------|
| `mass` | The sum of the `mass` of every part of the vehicle implementation (`vehicleDefinition`) after the ship's Modification is applied, rounded to four decimals; a sum too large to round to a finite number counts as unknown (so does `hullHealth`). Landing gear, Loadout items and docked vehicles are not part of it (#305). Without part masses, the mass of the rigid physics controller (`Components/SEntityPhysicsControllerParams/PhysType/Mass`, `massBasis` `physics`) |
| `hullHealth` | The sum of the positive `damageMax` of the parts, except parts holding an item port flagged `uneditable` or `invisible`; `null` without a vehicle implementation or when no part states one |
| `vehicleKind` | `groundVehicle` under `entities/groundvehicles` uses the original Record path. Under `entities/spaceships`, `spaceship` uses the same path and the effective vehicle Parts up to the first wheel (`SubPartWheel`), or the complete Part selection when checking for a tread (`Tread`) after ruling out wheels. The classification is absent for a wheeled or tracked spaceship. Matching exosuits use the original Actor and driver's seat evidence. The existing spaceship fallback for unavailable implementations remains present with incomplete evidence. |

#### Projected cargo

Join `projections.shipBaseFacts.rows[].shipRecordIndex.value` to the original Ship Record
index in the same Build. `shipGuid.value` carries the nominal identity. Different physical
Ship Records can share a GUID and retain separate rows. Each mount's `position.value` uses the
physical Loadout Record index and ordered nested port ordinals. Its attachment, authored
storage Reference, first physical InventoryContainer Record and SCU operands have independent
source origins. The projection retains every reached occurrence in Loadout order.

`cargo.scu.value` sums the readable capacities of default Loadout `CargoGrid` Entities, including
grids flagged `miningOnly` or `invisible`. It excludes docked descendants and templates. Open
capacity uses `x * y * z / 1.953125` from `interiorDimensions`. Closed capacity is normalized to
SCU. Each known mount and the final sum are rounded to four decimals. Zero can mean no reached
grid or a readable sum of zero. The state is `unreadable` when every reached grid has unavailable
capacity. `cargo.gridsWithIssue.value` counts those grids. A present sum with a positive count is
a lower bound. `cargo.unresolvedLoadout` reports resolver gaps that can omit mounts from both
values. Aggregate origins retain the observed contributors and flag incomplete exclusion
evidence. Individual grid flags are in [facility configurations](catalog-ship-facilities.md#cargo-grid-facilities).

LIVE anchors, read by the real-Archive test: Harbinger 230,404 kg, 48,490 hull, 0 SCU; Freelancer
MAX 341,408 kg, 35,650 hull, 120 SCU; Carrack 3,275,858 kg, 88,000 hull, 456 SCU; Constellation
Andromeda 386,857 kg, 180,844 hull, 96 SCU (without the docked snub craft). The Javelin and the
Hornet F7C have null SCU payloads because their cargo grids refer to unavailable
`InventoryContainer` Records.

In the LIVE survey before exosuit admission, the 1,066 rows split into 1,027 `spaceship` and 39
`groundVehicle` rows, with no `null` value. The survey found no wheel or tread in Records under
`entities/spaceships`. The folder `entities/groundvehicles`
holds 40 Records; the 40th, `RSI_Ursa_Rover_Unmanned_Template`, is left out of the catalog as a
development leftover (the whole word `template`, see "Selection").

**Minimum crew.** The game has none. A survey of the field names of all 1,144 ship Records on LIVE
finds only `crewSize` (on `VehicleComponentParams` and on `StaticEntityClassData/displayParams`) and
`crewManifest` (the NPC crew of AI variants, used by `playability`). No field named minimum, seat
or crew minimum exists, so `crewSize` stays the only crew value.

**Inventory dimensions.** The [inventory storage and item bounds contract](catalog-inventory-storage.md)
exports original AttachDef occupancy, vehicle box size and metadata bounds flags in
`configurations.itemBounds`. These fields describe authored inventory and definition-space
bounds. `definitions.inventoryContainers` and `configurations.inventoryConfigurations`
expose source capacity and the physical item or vehicle components that select it.
[Authored inventory interactions](catalog-inventory-interactions.md) are in
`configurations.interactions`. Their original condition lists, tasks and state links retain
physical owners and per-value provenance.

[Ship facility configurations and placements](catalog-ship-facilities.md) link the authored
seat, medical and cargo grid components and storage configurations to their resolved
Loadout contexts and authored SOC entities in ship rooms and rooms on resolved attachments.
Each placement row groups its configuration links in original component order. Room inspection
records its container occurrences and coverage in
`placements.facilityInteriorCoverage`. Medical tier and resource consumption definitions retain
their own physical Record identity.

#### Playability

The catalog lists every ship and item Record of the Build, among them AI, mission and event
variants that no player can own, buy or fit. `playability` tells them apart. The schema uses a
string payload, so a reader can retain an unknown classification (ADR-0054). A fully inspected
entity that matches no rule has state `absent`. An unreadable linked crew or Tag Record is
`unreadable`. An unproven predicate can leave the cell `unreadable` or allow a `present` value
with `sourceSetComplete: false`. The rules use Record components, fields, references, tags and
record-path folders. Class names and Record names do not enter the decision.

| Value | Meaning |
|-------|---------|
| `player` | Has the structural markers of a player entity |
| `ai` | Part of the AI; ships only |
| `mission` | Part of a mission: an unmanned ship or an item of a mission folder |
| `special` | Neither: a ship with `inclusionMode` `DoNotInclude`, or an item without a player-visible name |

For a ship or ground vehicle the first rule that holds decides. `inclusionMode` is
`StaticEntityClassData[EAEntityDataParams].inclusionMode` (`EAEntityDataParams`, believed to
govern Electronic Access / Arena Commander inclusion). It correlates on LIVE with hangar-listed
ships; it is used as evidence, not as a definition. Any value other than `ReadyToInclude` and
`DoNotInclude` leaves `playability.value` null unless rule 1 or 2 holds.

1. `ai` when `Components[SEntityComponentCrewManifestParams].crewManifest` links to a
   `CrewManifest` Record whose `crew` array is not empty (the NPC crew; the empty manifest
   `crewmanifest_null` does not count). For a Record under `entities/spaceships/` also when
   `Components[VehicleComponentParams].criticalPartExplosionChance` is above zero, or when
   `Components[VehicleComponentParams].aiEngineeringOptions.powerPlantsCanExplodeOnDeath` is true
   while `inclusionMode` is not `ReadyToInclude`. Ground vehicles all carry these two hull values,
   so they are not read there. Also, for every vehicle, when the Record tag (`$tag` of the Record
   JSON) is `SystemsDesign`: on LIVE all 15 ships with it are AI variants (the `AI_Super` classes),
   and no `player` ship carries it (ADR-0245).
2. `mission` when one of the Record's `tags` links to a `Tag` Record whose `tagName` is `Unmanned`
   (the salvage, derelict and wreck targets of missions).
3. `special` when `inclusionMode` is `DoNotInclude` (templates, wrecks, sentries, event props).
4. `player` when `inclusionMode` is `ReadyToInclude`. A hull that explodes on death does not
   count against it: on LIVE every `ReadyToInclude` ship with that hull value and no AI marker
   above is a ship the owner has flown (Basher, Sabre Raven EX, Skylark, Railen, Stingray, Tyilui,
   Constellation Mk V Centaurus; ADR-0245).
5. `playability.value` is null when the crew manifest or a tag the Record links to could not be read (the
   catalog reports that Record as a failure), since its evidence is missing; checked before
   rule 1.
6. Otherwise `playability.value` is null if there is no `EAEntityDataParams` or another `inclusionMode` applies.

For an item the first rule that holds decides:

1. `player` when `Components[SCItemPurchasableParams].displayName` is a localisation key that is
   not a placeholder key: the item has a shop display.
2. `mission` when the record path lies under `entities/scitem/mission_entities/` or
   `entities/scitem/missionstorage/`.
3. `special` when `AttachDef.Localization.Name` is no key or a placeholder key: the item has no
   player-visible name (doors, props, decals, internal parts).
4. Otherwise `null`: a named item without a shop display (loot, attachments, parts); the Records
   do not say whether a player can obtain it.

No item is `ai`: items carry no AI marker the rules can read.

The development-leftover token rule (see "Selection") runs first: a Record it leaves out is no
entity and has no playability. No structural marker sets the leftover-like ships apart from the
ships a player owns, so a ship is also left out by name when its record path or name holds the
whole word `temp`, `display` or `hijacked` (21 ships on LIVE, 15 of them `player`; ADR-0167).

`player` ships and items are candidates for a player inventory. `player`
means the Records carry the structural markers of a player entity (ships: hangar-ready inclusion
mode and no AI marker; items: a shop display name). Availability requires separate evidence.

`p4kx catalog` prints the count per value and the unclassified count for items and ships; `--json`
lists them under `playability`. LIVE, 2026-10-21:

| Kind | `player` | `ai` | `mission` | `special` | `null` |
|------|---------:|-----:|----------:|----------:|-------:|
| ships | 324 | 690 | 25 | 35 | 13 |
| items | 7,954 | 0 | 522 | 8,570 | 6,234 |

##### Comparison with the RSI ship matrix

A development check, never an export source (ADR-0245): `playability` of the ships is compared with
the public RSI ship matrix, which the owner saves by hand as
`local-data/ship-matrix/ship-matrix.json` (excluded from git locally, never fetched by P4KX or CI).
The `#[ignore]`d test `real_archive_ship_matrix` in `p4kx-export` reads the file named by
`P4KX_REF_SHIP_MATRIX`. LIVE, ship matrix of 2026-10-28 (256 entries, 227 `flight-ready`, 29
`in-concept`):

| Question | Result |
|----------|--------|
| Flight ready in the matrix, but not `player` | 0 ships |
| `player` ships the matrix does not call flight ready | 96 of 315, all unknown to the matrix and none called not ready: store-less variants such as Collector, Exec, Teach, BIS2950, Showdown and unmanned editions |
| `null` ships | 0 of 1,066 |
| Flight ready in the matrix, no class mapped | 4 (Mustang Alpha Vindicator and three ATLS entries) |

Before rule 1 took the Record tag, the `AI_Super` classes of `ANVL_Hornet_F7C_Mk2` and
`RSI_Perseus` were `player` and those of Corsair, Polaris and Fury `null`; the matrix knows none of
them. The seven flight-ready base ships that were `null` (Basher, Sabre Raven EX, Skylark, Railen,
S-65 Stingray, Tyilui, Constellation Mk V Centaurus) and `KRIG_S65_Stingray_BALLISTIC` are
`player` since rule 4 no longer reads the hull. Ships by `playability`, LIVE 2026-10-28:

| State | `player` | `ai` | `mission` | `special` | `null` |
|-------|---------:|-----:|----------:|----------:|-------:|
| Before | 309 | 689 | 22 | 35 | 11 |
| After | 315 | 694 | 22 | 35 | 0 |

A matrix entry maps to a ship through the `shipStoreSlugs` (RSI id, then the slug of the `url`);
18 matrix names the slugs do not map are mapped by an explicit name table in the test. A catalog
class counts as flight ready when any of its matrix entries is, so a flight-ready variant mapped
to the same class (Mole editions, Carrack w/C8X) hides a base entry that is not. The test warns
when a name-table class is no catalog ship and when a catalog ship or store slug entry cannot be
read.

Commodity identity and authored facts are in `definitions.commodities.rows`; explicitly derived
choices are in `projections.commodityPresentation.rows`. The root `commodities` array is removed.
Both tables keep the existing Commodity selection, GUID identity, resource-only fallback and
canonical ordering. Every source-backed value, including identity, text references, links and
nested authored members, has its own immediate origin carrier.

`guid.value` in the definition remains the unique Commodity key. Nullable, non-unique
`subtypeGuid.value` in the presentation is a ResourceType relationship. A `resourceGuid` join can
return zero, one or several Commodity alternatives; never use `subtypeGuid` as a primary key.
Independent resource facts join `definitions.resourceTypes.rows[].guid.value`.

The [Commodity contract](catalog-commodity-types.md) defines the authored bindings, canonical
text domains, exact first-resource/first-group/name/density/refinement recipes and reconstruction
of old nullable values. Each fallback records the actual contributing selection uses.
The source verification guide records Build-bound LIVE and PTU cardinalities and physical source
checks. [Installed resource verification](resource-source-verification.md#exact-cardinality-baselines)
A Blueprint (`CraftingBlueprintRecord`) uses the [Blueprint v3 contract](catalog-blueprints.md).
Its independently sourced Record header and complete original `blueprint` body are in
`definitions.blueprints`. `configurations.blueprintTiers` interprets the tiers and costs by
physical owner and original occurrence. `projections.blueprintPresentation` keeps the established
name, category, produced item, default availability and dismantle choices. Its `firstTier` and
ordered `ingredients` navigate the canonical tier and costs.

A resource group (`ResourceTypeGroup`) is the type of a commodity: the commodity's `typeGuid`
keeps the existing first selected group that lists its `ResourceType` directly. Schema v3 separates
these source facts and choices:

| Dataset | Fields and source |
| --- | --- |
| `definitions.items` | Physical original Item definitions with exact Record headers, source component slots and game/public text references. [Item definitions](catalog-item-definitions.md) |
| `definitions.resourceGroups` | Original physical header, declared `recordName`, className, recordPath, recordIndex and sourceType; name references from displayName, protected description references and exact thumbnail paths |
| `declarations.resourceGroupMembers` | Each selected physical owner's original groups/resources arrays, retaining nulls, repetitions, wrong/missing targets, all parent edges and cycles in authored order |
| `projections.resourceGroupParents` | The first selected parent with a different GUID in physical Record order; nominal child-GUID matching retains duplicate child definitions' shared choice and excludes same-GUID edges even across different physical Records |

Every source-backed datum has its immediate encryption marker and actual source-use references.
Names use `texts.publicNames`; descriptions and their references use protected `texts.gameTexts`.
Missing/null/empty/unreadable/invalid facts stay distinct. Equal GUIDs do not merge physical Records.
The parent choice retains its actual earlier selection inputs and original chosen slot; later unused
parents do not taint it. Importers must guard cycles and must not assume a fixed depth or a forest.
The authoritative membership arrays remain separate from singular convenience choices.

Only selected groups affect these choices and `definitions.resourceTypes.rows[].groupGuids.value`;
additional health-only reads do not create group rows or memberships. ResourceType group GUIDs keep
all distinct selected direct memberships sorted by GUID. Thumbnail paths remain source facts;
cataloging does not extract the media. No parallel root resourceGroups array is emitted. See the
[ResourceGroup contract](catalog-resource-groups.md) for identity, provenance and health details.

`definitions.blueprintCategories` holds the selected readable physical `BlueprintCategoryRecord`
definitions as an `availability`/`rows` table. A declared family with no selected rows is
`available` with an empty list; a missing family is `absent`. Unavailable family metadata remains
`unavailable`, independently of source-read failures reported in `exportHealth`. Categories use
the existing path/type, Filter and development-leftover selection. Rows remain in GUID order;
equal GUIDs retain their original physical order and distinct `recordIndex` values.

The Record declares no body fields. Each row therefore contains only the
[shared eight-field Record header](catalog-record-headers.md), with the immediate per-datum
origin marker on every field. There is no fabricated `name` or text-table row, parent or category
rule. A readable unexpected body field is diagnosed and blocks normal publication; diagnostic
publication retains the header row and the exact field diagnosis. A failed strict tree read
produces no category row and remains a source-read failure.

The old flat root list is absent. Its `guid`, `className` and `recordPath` values are now the
corresponding sourced fields' `value` in `definitions.blueprintCategories.rows`. The additional
raw name, exact filename, physical index, source type and metadata-tag state come from the actual
header. `declarations.blueprintCategoryLists` independently retains each selected physical
`BlueprintCategoryDatabaseRecord` and its ordered, sourced `categories` reference slots
([contract](catalog-blueprint-category-lists.md)). These declarations do not change the category
definitions' selection or GUID order and declare no category rules or hierarchy.

A Commodity's `typeGuid.value` joins `definitions.resourceGroups.rows[].guid.value`.
A Blueprint's presentation `category.value` joins `definitions.blueprintCategories.rows[].guid.value`
when the category is readable and selected. A selected leftover or unreadable category clears the
presentation choice. Its authored reference remains in the Blueprint definition. A filtered export
may contain a choice whose target lies outside the selection. Only selected groups affect group
and Commodity membership.

### Tiers

Each `configurations.blueprintTiers.rows[]` carries the physical Blueprint reference, the original
tier occurrence, `craftTimeSeconds`, `mandatory`, `optional`, flat `costs` and `research`. A
selection's `options` reference costs in stored order. The `meaning` union distinguishes
`select`, `resource` and `item`. Resource and item quantities retain their original units before
multipliers. The cost context retains the effective multiplier, original composition option,
and the ordered modifiers and ranges it recognizes. All authored context and cost values are
also available through the exact source arena. The reader follows complete valid selections
iteratively.

Research distinguishes an absent declaration, a null pointer, a readable envelope, an unsupported
shape and an unreadable value. A readable envelope retains both original members and their
declared strong-pointer bases. A populated exact fieldless base has a known present
interpretation. Other populated payloads retain their original structure and get an unsupported
diagnosis.

### Quality, pools and dismantle

Five kinds hold what a blueprint or resource refers to. Canonical quality definitions use the
shared sourced Record header; the other lists keep their own identity fields. A `--filter`
matches the record path and marks selected-kind scope partial independently of source health.

| Kind | Fields |
|------|--------|
| `definitions.gameplayProperties` | `propertyName` and `unitFormat` are sourced public-name references with source/display TextForms; `displayTransformation` preserves original operation occurrences through local links, and `nameOverrides` retains every ordered name/condition declaration. Scale numbers retain exact original width/bits. These definitions remain the targets of modifiers' nominal `propertyGuid` values; no transformation or condition is evaluated ([contract](catalog-gameplay-properties.md)) |
| `definitions.qualityDistributions` | Shared sourced header, independent `qualityDistribution`/`locationOverride` declarations and ordered nullable override positions; exact numeric storage and inline/recordRef/emptyBase forms |
| `definitions.qualityQuantizations`, `projections.qualityQuantizationResources` | Complete raw band definitions and a separate genuine inverse, described below |
| `definitions.blueprintPools` | Physical `BlueprintPoolRecord` owners with the shared sourced header and ordered `blueprintRewards`. Each slot keeps its concrete type, original `blueprintRecord` Reference and exact `weight`. Local `additionalFields` arenas preserve remaining owner and slot declarations. References can resolve to Blueprint owners outside the current selection. |
| `projections.blueprintPoolWeights` | `pool` identifies the physical owner by input, original `recordIndex` and nominal `guid`. `totalWeight` is the sourced f32 sum of positive finite decoded weights in slot order. It carries raw bits even when accumulation overflows. |
| `declarations.blueprintRewardSources` | One row per observed direct scalar Record link to a selected pool. `poolStorage` distinguishes `reference` from a foreign Record root `weakPointer`. Local and self weak pointers are excluded. The sourced header identifies the enclosing Record and `sourceOrdinal` identifies the occurrence within it. `pool` preserves the authored link and `chance` keeps its original numeric storage and source state. |
| `definitions.dismantleProcesses` | `blueprint.value.processSpecificData.value` carries the authored `efficiency` and the four original `dismantleTime` components with numeric storage and origins. Derived `timeSeconds` cites the converted inputs and is a signed decimal string |

`contractId` comes from the nearest enclosing owner with a readable GUID `id`, including the
Record root. `contractName` takes that owner's `debugName`. When no owner has a readable GUID
`id`, the nearest readable nonempty `debugName` supplies the name.

Join definitions and their weight projections within the same Build by the DCB input and
original Record index. Keep the nominal GUID as a consistency check. Same-GUID aliases retain
separate definitions and weight rows. Reward declarations retain their actual target state; a
nominal GUID alone cannot establish a physical target. The selected scope and availability
remain explicit, including empty tables. See [ADR-0412](adr/0412-blueprint-pools-separate-original-slots-weights-and-reward-sources.md).

A consumer can estimate a slot's weight share when both its weight and the total are finite
and positive. Keep the authored `chance` separately when it is missing or outside 0 to 1. The
quality distributions carry no link to a blueprint or source:
the Records do not hold one, so the catalog lists them as they are. Resource-owned inline and
linked forms preserve their source ownership in
[`definitions.resourceTypes.rows[].properties`](catalog-resource-types.md#quality-source-wrappers-and-exact-curves).
Standalone quality definitions and resource-owned declarations use the same typed exact-width
numeric readers. The catalog preserves f32/f64 bits, wide integer widths, signed zero and
nonfinite raw tokens. Separate declared Int32 and configured-range violations remain blocking
field diagnoses without dropping a readable raw owner. The public Rust QualityCurve and
read_crafting interfaces retain their independent finite f32 interpretation contract.

#### Quality quantizations

`definitions.qualityQuantizations` holds every selected readable physical quantization, including
complete empty bands and field-invalid raw data. `projections.qualityQuantizationResources`
contains only minimal original owner identity, explicit sourced eligibility and sorted unique
inverse resource GUIDs. It preserves the first eligible nominal-GUID policy independently of a
raw reference's physical target. Required resource dependencies remain outside the output Filter;
the aggregate inverse retains all consulted original relationships. Each nested member and its Commodity fallback retain their own matching source inputs.

`CatalogSummary::quantizations().unused` and the CLI summary count eligible unassigned results;
`unresolved_links` retains its eligible-target/dedup/leftover scope. D row counts instead count
all retained source definitions. No second flat quality list or reduced payload mirror is
written. The [quality contract](catalog-quality.md) specifies source states, exact per-field
capture proof, reference health, inverse contributors and publication behavior.

#### Dismantle returns

A present `projections.blueprintPresentation.rows[].dismantle` has `processGuid` and `tier`.
The process applies to the produced item. `tier` is the source occurrence of the first tier and
is null when the Blueprint has no tier. `configurations.craftingGlobal` is a table of physical
`CraftingGlobalParams` owners. Each row retains its original `defaultBlueprintSelection`,
`dismantleBlacklistResources` and `dismantleBlacklistEntityClasses` with immediate per-field
origins and ordered nullable target slots. Selection concrete types and additional original fields
remain available; physical aliases and repeated references are not merged. The old merged root
object is absent. The Blueprint rule interpreter still unions the original owners' readable
default/exclusion GUIDs. The contract:

- The Build must hold exactly one dismantle process. With several, no blueprint gets a
  `dismantle`, the processes are still listed, and `p4kx catalog` warns and exits with code 1.
- The item returns the share `efficiency` of the costs of the first tier, except
  the resources selected by the original `dismantleBlacklistResources` slots.
- A `select` is not resolved: the Records do not say which options a dismantle returns, so the
  consumer applies the `count` itself. The `quantityMultiplier` is not applied.

These are assumptions of the library, not verified against the game.

#### Variant groups

The structural comparison uses the unfiltered Item or Ship family from the original Build.
An output filter limits returned owners. A returned owner can point to a base outside the output.
Structural grouping follows ADR-0066 and compares Record data and vehicle implementations.
`playability` contributes to choosing the base within a group.

`variantOf` is a sourced decision. Read `state` and `value` together.

| State | Meaning |
|-------|---------|
| `present` | `value.targetRecordIndex` and `value.guid` independently identify the selected physical base in the same Build |
| `absent` | Proven base, singleton, same-GUID alias of the base, or owner with no comparable key. `value` is null |
| `unreadable` | Original evidence is insufficient to prove a choice or absence. `value` is null and `sourceSetComplete` is false |

Resolve `sourceUsesRefs` and `failedSourceUseRefs` against the result's `provenance.uses`.
When `sourceUseSetRef` is an integer, also include `sourceUsesRefs` from the matching `id`
in `provenance.variantSourceUseSets`. Its ID is the smallest physical Record index in the cohort.
Any returned decision with additional original comparison uses retains its local references.
The presence of failed attempts and known encrypted contributors remains independently visible.
Results without shared sets can omit `variantSourceUseSets`.

**Ships** share a group when all of these are equal:

- `vehicleDefinition` (`VehicleComponentParams.vehicleDefinition`), compared as a path without
  regard to case, to `/` or `\`, and to a leading separator;
- the vehicle implementation after the Modification the ship selects (ADR-0050), compared as a
  whole: the digest covers every element of the patched `Vehicle` tree (parts, item ports and
  their children such as `Pitch` or `Yaw`, the other children of `Vehicle` such as
  `MovementParams`) with all its attributes, sorted by name, and the children in file order. The
  `Modifications` element, the path of the file, the names of the Modification and its patch file,
  the `name` of the root and the attributes `material`, `materials`, `filename` and `geometry`
  anywhere are not compared. Because the patch file and the `Elem` overrides are applied before
  the comparison, whatever they change is part of it. The item ports that take paints are part of
  it too, so a Modification that changes only the paints a ship accepts starts a group of its own.

The ship Record has no armour component on LIVE (no component struct name contains `armor` or
`armour`; build 4.10), so no armour term enters the rule. The default loadout and the names do not
count: editions that differ only in their loadout, or in a material, are variants. On LIVE `AEGS_Idris_P` therefore groups under
`AEGS_Idris_M` and `AEGS_Sabre_Comet` under `AEGS_Sabre`, because they differ only in their
default loadout. A ship without a `vehicleDefinition`, or whose vehicle
implementation cannot be read in full (a missing file or patch file, a broken document), has
no published base. A missing explicitly authored patch closes as sourced absence when the
complete original Archive directory proves the exact selected file absent and the other
original comparison inputs close. The member contributes its exclusion evidence to the cohort.
A failed payload read, malformed document or incomplete directory leaves the affected
comparison unreadable.

The implementation key is bound to the original physical ship Record within the current
DataForge input. Separate Records with an equal GUID keep their own selected implementation
keys. The chosen physical target retains both its GUID and Record index. Equal-GUID aliases
of that base are treated as bases for link publication.

**Items** share a group when all of these are equal:

- `type`, `subType`, `size`, `grade` and `manufacturerGuid` of the `AttachDef`;
- the original compact stat data, including shield fields and `resourceStates` (ADR-0065,
  ADR-0356). A different beam range, module modifier or shield value separates variant groups.
- the components: every component of the Record is compared with all its fields, including
  `SAttachableComponentParams` (without its `Tags`), the physics, the default loadout of an item
  that has ports (`SEntityComponentDefaultLoadoutParams`, unlike ships) and the geometry, whose
  model (the geometry file of every node) must be equal: a different model is not cosmetic.
  Left out are exactly these components: `SEntityInteractableParams`,
  `SInteractionStateMachineParams`, `SEntityComponentCarryableParams` and `SActorUsableParams`
  (per-item Records of equal content), and these fields: `Tags` of `SAttachableComponentParams`,
  `Material`, `Materials` and `Palette` of `SGeometryResourceParams`, and `openInventoryInteraction`
  and `openLootingInteraction` of `SCItemInventoryContainerComponentParams`. Localisation keys
  (names, descriptions, display names) are never compared. On LIVE the clothing and armour
  colour variants differ in exactly these: name, tags, material path, palette and the per-item
  interaction Records. A Reference counts by the GUID of its target and, for a weak pointer, by
  the location of the instance inside that Record.

An item with no compared component besides `SAttachableComponentParams`, without an `AttachDef`
type, or whose data holds a value the reader could not read, is ungrouped. Names, descriptions,
tags and the display type may therefore differ inside a group.

`variantOf` means "structurally identical", not only "cosmetic". Per-ship functional duplicates
that differ in their name alone are grouped too (e.g. the `Controller_Shield_<ship>` and
`Engineering_Buff_Modifier_<ship>` items), and so can fitment such as ATC managers, docking tubes,
intakes or quantum tanks, depending on whether the Records of the ships differ in a compared field
(LIVE, build 4.10: groups of 117 ATC managers, 75 docking tubes, 54 intakes and 38 quantum tanks
were measured under an earlier version of the rule; the counts shift with the rule and the
build). Fitment differences may or may not split a group. A consumer that wants only what a player
sees or fits combines `variantOf` with `playability`.

**Base.** The member with `playability` `player`, then the member with the smallest `className`
(compared byte-wise), then the smallest `guid`. Equal ranks use original physical Record order.
A group holds at least two members with different GUIDs. `p4kx catalog` reports variant and group
counts per kind. The JSON CLI summary also reports those counts.

### Paints

A paint (`paints`) is an item of the type `Paints`. The kind refers to the item and adds no
description, no name and no media: name, record and description stay on the entry in `items`, so
no GUID occurs twice among the `guid` fields of the catalog:

| Field | Source |
|-------|--------|
| `itemGuid` | The `guid` of the paint's entry in `items`; paints are ordered by it |
| `shipGuids` | The `guid` of every ship of `ships` the paint fits, ordered by GUID; empty when it fits none |

A paint is selected like an item (the same record-path prefixes, the same development leftovers
left out), whose `AttachDef.Type` is `Paints` ignoring case; LIVE holds 1,132 such Records, 1,064
after the leftovers (`paint_*_template` and the like). Two of them are weapon skins
(`Skin_Gold`, `Skin_Space_Kitty`), which no ship port takes.

The rule is structural and never reads a name. A paint fits a ship when one paint port of the ship
accepts it, i.e. when every tag in the port's required tags is a tag of the paint's `AttachDef.Tags`
or has a tag of the paint's `Tags` below it in the tag tree (`TagTree`, ADR-0052). Tag names compare
ignoring case; the paint's marker tag `@Name` is a tag of its own. A port without required tags
accepts nothing. The paint ports of a ship are the union of:

- the `SItemPortDef` entries of `SItemPortContainerComponentParams.Ports` of its Record, which hold
  an `SItemPortDefTypes` with `Type` `Paints`; the tags are `RequiredPortTags`; this is where the
  newer ships keep the port (e.g. the Paladin: `Paint_Paladin`);
- the `ItemPort` elements of its vehicle implementation (`VehicleComponentParams.vehicleDefinition`)
  that take `Paints` (a child `Types/Type` of `type` `Paints`), with the Modification named by
  `VehicleComponentParams.modification` applied. The implementation is read by
  `p4kx_assembly::VehicleImplementation` after the rules of ADR-0050 (patch file, `Elem` entries
  by id, fallback file names); the tags are the `requiredTags` the patched port has (the Omega
  variant of the Mustang requires `Paint_Omega` instead of `Paint_Mustang`).

The paint's own `AttachDef.RequiredTags` must be offered as well (ADR-0161): every tag in them is
one of the `portTags` or `requiredTags` of the same port, or a tag of the ship Record's
`AttachDef.Tags`, or has one of those below it in the tag tree. A paint without `RequiredTags`
passes. The Omega paint requires `Paint_Omega`, which the patched port of the Omega requires while
it still lists `Paint_Mustang` as its `portTags`. Measured on LIVE (4.7), the check removes 40
assignments of four paints and no paint loses its last ship: the Hornet F7A Mk II desert paint
(requires the `ANVL_Hornet_F7A_Mk2` ship tag) no longer fits ten F7C variants, the C8X Pisces
Expedition paint no longer fits the eight C8 and C8R variants, and the two Cutlass Black paints
that require `Paint_Cutlass_Black` no longer fit the eleven Red, Blue and Steel variants. A ship
whose `modification` is empty keeps the base port, so the Idris-M still fits the paints of the
Idris-P; the check does not change that, as the Idris paints require no tag of their own. The
game selects the Military paints elsewhere.

`p4kx catalog` counts the paints, those that fit a ship and those that fit none, the ships that
have at least one paint and the ships whose vehicle implementation cannot be read, or whose patch file or Modification is
not applied as it names, except that a patch file the Archive lacks is a fact of the game data; `CatalogSummary::paints()` returns them, and an unreadable vehicle file is also a warning of
`CatalogSummary::warnings()`. Measured on LIVE (4.7): 1,064 paints, 1,039 assigned, 25 not
assigned, 1,025 ships with paints (1,046 before ADR-0167). This survey predates admission of the
four exosuit owners. Of the 25, 16 are ARGO ATLS paints, 2 are weapon skins and 7 are the
default liveries of ships (`Paint_Ballista_Base`,
`Paint_Gladius_Default`, `Paint_Carrack_Default`, `Paint_Hurricane_Default`, the Vanguard
Harbinger, Sentinel and Hoplite) whose tags no port requires. With a `--filter` the paints and the ships are
both filtered, so a paint lists only ships the filter selected; both kinds are partial.

The development leftovers among the paints (68 on LIVE) are counted as skipped in `paints` and in
`items`, whose rule they follow; `p4kx catalog` shows the number in both lines, and the total of
skipped Records adds up the kinds that select Records only, so it counts them once. The catalog build
takes about 18 s on LIVE; deriving the paints takes well under 0.1 s of it, the ships' vehicle
files being read once by the record scan for the ship base data and shared through one path
index (`ArchiveFiles`). A cancelled build stops the derivation between two paints.

### Loadouts

A loadout (`loadouts`) is the Default Loadout of one ship or of one item that has ports, resolved
into a tree (ADR-0064). The kind refers to the entity and adds no name, no description and no
media: name and record stay on the entry in `ships` or `items`, so no GUID occurs twice among the
`guid` fields of the catalog. Loadouts are ordered by `entityGuid`.
A single resolver visits selected roots in that order and stages each completed row immediately.
The first request owns each original file-read observation. Later requests reuse the cached read.

| Field | Source |
|-------|--------|
| `entityGuid` | Sourced GUID from the original owner Record header. Its value joins to the owning `ships` or `items` entry. |
| `entityKind` | Sourced owner kind. Its string value is `ship` or `item`. The original inputs are the declared Record type, normalized filename and Record name used by the owner selection. |
| `issues` | The codes of what kept part of the Loadout from being read, sorted, without duplicates: `dangling_reference`, `invalid_reference`, `unknown_class`, `ambiguous_class`, `not_an_entity`, `unreadable_record`, `cycle`, `too_deep`, `no_loadout_files`, `xml_loadout`, `assortment`, `vehicle_implementation`; empty when all was read. `no_loadout_files` (an XML Loadout is named but no Archive was given to read it from) cannot occur in a catalog, which always has its Archive. A vehicle implementation or a patch file the Archive lacks is a `vehicle_implementation` issue too, a fact of the game data |
| `ports` | The ports of the entity, see below |

**Selection.** Every entry of `ships` has a loadout, also one without ports, with one exception: a
ship whose own Record cannot be resolved stays in `ships` without a loadout and is listed in
the failures (printed on stderr by `p4kx catalog`, under `failed` in `--json` and in the summary
of the window) with a reason that starts with `loadout: `; `completeness.loadouts` is not changed by it.
An entry of `items`
has one when its Record defines item ports (`SItemPortContainerComponentParams`) or names a default
Loadout (`SEntityComponentDefaultLoadoutParams`) and its tree holds at least one port: turrets,
racks, weapons with attachment ports. With a `--filter` the ships and items are filtered first,
so a loadout exists only for the entries the filter selected and the kind is partial.

`sourceRecordIndex` identifies the original physical DataForge Record with its immediate header
origin. Use `sourceRecordIndex.value` to distinguish Loadout rows that share an `entityGuid`.

**Placement paths.** A placement address refers to a physical root and an ordered path through
the resolved Loadout for the same input.

| Field | Join |
|-------|------|
| `loadoutRecordIndex` | Match `loadouts[].sourceRecordIndex.value` from the same DataForge input. The physical index is scoped to that input and survives catalog filtering. |
| `portOrdinals` | Start with the root's `ports` array. At each step, select the zero-based index and use that port's `ports` array for the next step. An empty array addresses the root. |

The address is used for resource-network contexts and nodes, installed configurations,
Loadout facility sources, and facilities reached through item rooms.

**Ports.** Each port has:

| Field | Source |
|-------|--------|
| `name` | A sourced cell containing the port name as the Loadout or port definition spells it; read `name.value`. Port names match ignoring ASCII case. Original DCB/XML coordinates, source uses and encryption state belong to this cell |
| `source` | Where the attachment comes from: `loadout` (the entity's own Default Loadout), `override` (the Loadout of the entry that attached the entity, which replaces the entity's own per port), `port_default` (the default item of the port definition), `unset` (defined, holds nothing) or `vehicle` (known only from the vehicle implementation, holds nothing). Closed set, string |
| `minSize`, `maxSize` | Sourced cells with the size in `.value` and its independent state and origin. The selected `SItemPortDef` supplies both cells. Ports without a DataForge definition use the effective XML `ItemPort` attributes. |
| `types` | `{type, subTypes}` in stored order from `SItemPortDef.Types` or XML `ItemPort/Types/Type`. XML subtype tokens come from the comma-separated `subtypes` attribute |
| `flags` | Sourced flag words from the selected `SItemPortDef.Flags` or XML `ItemPort@flags`, with leading `$` removed. The words share the original field or attribute evidence. |
| `portTags`, `requiredTags` | Separate sourced cells from `SItemPortDef.PortTags` and `RequiredPortTags`, or XML `ItemPort@portTags` and `@requiredTags`. Their `.value` preserves order, repeats, case and leading `$`. An item fits a port when it carries the required tags. |
| `displayNameKey` | Sourced original localisation key from `SItemPortDef.DisplayName` or XML `ItemPort@display_name`. The cell retains explicit empty and placeholder keys. Use the [physical port definition](catalog-item-port-alignment.md) for canonical text from DataForge. |
| `visibility` | `visible`, `invisible` or `unspecified`: whether the vehicle implementation's part of that name carries the flag `invisible`; without such a part `invisible` when the port definition carries that flag, else `unspecified`. Closed set, string |
| `itemGuid` | The `guid` of the entry in `items` (a docked vehicle: in `ships`) that the port holds by default; `null` for an empty port. For an attachment that cannot be resolved the GUID the Loadout names, or `null` when it names a class name only |
| `itemClassName` | The class name of the attached entity, or the class name the Loadout names; `null` for an empty port and an unresolved Reference |
| `dockedVehicle` | `true` when the attached entity is a docked vehicle (`AttachDef.Type` starts with `NOITEM_Vehicle`, ignoring case, e.g. a snub craft in a hangar port). It is a reference to a ship and is not expanded: `ports` stays empty (#305) |
| `unresolved` | The code of the error that kept the attachment from being resolved (`issues` lists the codes), otherwise `null` |
| `ports` | The ports of the attached entity under the Loadout that attached it, or the ports of the nested Loadout of an empty port; empty otherwise |

The name cell retains the original copied name with its own Payload, Selector and Conversion uses. Missing original proof keeps the resolved spelling present and its origin incomplete/unknown. This does not certify the source choice or membership of the whole port tree; the other resolved-port fields remain separate migration boundaries.

The size cells distinguish an absent declaration, an explicit DataForge null, an unreadable
source, and an invalid value. A missing size in a selected DataForge definition stays absent.
An XML cell retains the effective attribute or checked absence, the current Record's
vehicle lookup and its Modification selectors. Its XML source set includes the ordered
search through the winning Part and its first ItemPort child. A reused XML document cites
the current physical Record's lookup. Missing original evidence or an unavailable
interpretation stage leaves the source set incomplete.
The flag and tag cells use the same selection rule. Authored empty text gives a present empty
list, while an absent, null, unreadable or invalid source has a null `.value`.

The ports of an entity are, in this order: those a Loadout names in Loadout order, the other
ports the Record defines in definition order, and the `ItemPort` elements of the vehicle
implementation (the Modification applied, ADR-0050) that no port above names, in file order, with
the source `vehicle`. The definition of a port is the Record's `SItemPortDef`; a port the Record
does not define takes it from the `ItemPort` of that name. The visibility comes from the
vehicle implementation (#289), else from the flags of the port definition (#748). A ship's
Loadout overrides the default Loadout of the turrets and racks it carries per port, so the nested ports of a ship are the ones that apply to that ship;
the entry of the turret itself shows the turret's own defaults. The walk is the one of
`EntityNode::loadout` (#316).

**References.** Every `entityGuid.value` is a GUID of `ships` or `items`. An `itemGuid` is one of
them too, except for an Entity outside both kinds: a development leftover, or an Entity of another
folder (`p4kx catalog` counts them as the Entities without an entry in items or ships,
`CatalogSummary::loadouts().outside`; the key `outside` of `--json`). Such a GUID stays, so the
consumers can distinguish an unknown Entity from an empty port. An `itemGuid` of an unresolved attachment
may name no Record at all. With a `--filter` the references need not resolve.

`p4kx catalog` counts the loadouts of ships and items, the ports (the nested ports of a ship are
counted for every ship that carries them), the ports that hold an Entity, the docked vehicles, the
unresolved attachments, the Entities outside the kinds and the loadouts with an issue; `--json`
lists them under `loadouts`.

#### Matching tags

`tags` of an item and `portTags` and `requiredTags` of a port are free strings that the game
matches by name; they are not names of tags in the tag tree, so the catalog carries no tag GUIDs
for them. Checked on LIVE: of 70,629 port tag words 65,133 name no tag of the tree or several
(18,891 tags, 1,745 names used more than once), and of 28,575 item tag words 21,146 do not
resolve; the few unique hits are coincidental. An item fits a port when it carries every word of
the port's `requiredTags`, compared as stored (case-sensitive, a leading `$` is part of the word).
The tag tree rule of the paints (a tag below a required tag also counts) applies only to the
`Tag` Records that paints and Tags fields refer to (see "Paints").

### Contracts

`projections.contracts.rows` lists the logical contracts and sub contracts of selected
`ContractGenerator` Records. A contract with sub contracts has its own row and one more per
sub contract. The rows share the original contract `guid.value`, and each sub-contract row
has its own `subContractGuid.value`. The parent row has an absent sub-contract ID. Rows
are ordered by contract ID, sub-contract ID and Generator Record GUID, with the parent first
and the first duplicate logical ID pair kept. Each `source` joins its original physical
Generator and its actual handler, contract and sub-contract slots.

A contract is an element of the arrays `contracts`, `introContracts`, `legacyContracts` and
`PVPBountyContract` of a handler in `generators`. One without an `id` (or with a nil one), and one
whose debug name or Record is a development leftover, is left out and counted in `skipped`; so is a
sub contract without an `id` (it cannot be told apart) or with a leftover debug name, and an entry
that the rule on equal `guid` and `subContractGuid` above drops.

| Field | Meaning |
|-------|---------|
| `guid`, `subContractGuid` | Sourced `id` of the contract and sub contract. The parent has an absent sub-contract ID |
| `source` | Physical Generator reference and the original handler, contract and optional sub-contract sites. The definition carries the sourced Record GUID and path |
| `debugName`, `handler` | The `debugName` of the contract and of its handler |
| `templateGuid` | The `ContractTemplate` Record the contract names, an entry of `contractTemplates` |
| `brokerEntryGuid` | The `MissionBrokerEntry` Record the contract names in `missionBrokerEntry`, an entry of `missionBrokerEntries`; `null` when it names none (on LIVE the 419 contracts that name one all resolve) |
| `notForRelease`, `workInProgress` | The flags as stored, `null` when absent; the catalog does not filter on them |
| `missionTypeGuid`, `missionType` | The selected `MissionType` Record GUID and its sourced `GameTextRef` into `texts.gameTexts`. The reference keeps the exact original type name even when the definition is outside the Filter. It is `null` if the selected Record is unavailable. |
| `contractor` | The sourced protected text reference for the `Contractor` parameter |
| `pickupPlaces` | Every eligible resolved starmap object the contract is offered at, each `starmapObjectGuid`, `objectType` and the canonical name in `place` |
| `offeredAt` | Every place the contract is offered at, uncapped (see "Offered at"): `localityGuids` (physical GUIDs in `definitions.missionLocalities.rows`) and `locations` (starmap objects, each `starmapObjectGuid`, `objectType`, `place`), and `brokerFallback` (`true` when `locations` is the broker entry's location because no place is stated) |
| `faction` | The `factionReputation` of the handler as stored: `guid`, Record `name` and `displayName` (game text), `null` when the handler names none |
| `giverFaction` | The faction of the contract and where it was found: `source` (`handler`, `reputationResult` or `reputationPrerequisite`) and `faction` (`guid`, `name`, `displayName`), `null` when none names one (see "Giver faction") |
| `blueprintRewards` | The blueprint pools the contract hands out, each `poolGuid` (a nominal GUID in `definitions.blueprintPools`), `source` (`result` or `temporaryModifier`) and `chance` as stored, `null` for a modifier (see "Blueprint rewards") |
| `reputationGains` | Sourced list of success reputation changes. The faction, scope and amount each have source evidence. `amount` is a signed decimal string. See [Reputation gains](catalog-contract-generators.md#reputation-gains). |
| `minStandingGuid`, `maxStandingGuid` | The `minStanding` and `maxStanding` of the contract: the standings between which it is offered, rows of `definitions.reputationStandings`; `null` when absent. A sub contract shares the contract's |
| `standingRequirements` | The reputation prerequisites (see "Standing") |
| `cooldownLayer` | Sourced nearest level among the five cooldown choices (`subContract`, `contract` or `handler`) |
| `cooldownLayers` | A sourced level for each of the five cooldown fields |
| `hasPersonalCooldown` | Sourced personal cooldown flag |
| `personalCooldownTime`, `personalCooldownTimeVariation` | Sourced personal cooldown and its variation |
| `abandonedCooldownTime`, `abandonedCooldownTimeVariation` | Sourced cooldown after abandonment and its variation |
| `timeToComplete`, `buyInAmount` | Sourced `timeToComplete` (floating point) and `contractBuyInAmount` (integer) from the parent contract's `contractResults`. Sub contracts share the same original inputs. The Archive states no unit |
| `canBeShared`, `illegal`, `onceOnly`, `maxPlayersPerInstance`, `availableInPrison`, `canReacceptAfterAbandoning`, `canReacceptAfterFailing`, `hideInMobiGlas` | Each contains a sourced `value` and a sourced `layer`. The layer value is `subContract`, `contract`, `handler` or `template` when selected. See [Layered contract facts](catalog-contract-generators.md#layered-contract-facts). |
| `requiredIntros` | Sourced ordered completion tag requirements and sourced lists of intro contracts that grant them. See [Intro prerequisites](catalog-contract-generators.md#intro-prerequisites). |
| `fixedRewards` | Sourced list of stored success payouts. Each `currency`, `amount`, `max` and `plusBonuses` has its own sourced value. `amount` and `max` are signed decimal strings. See [Fixed payouts](catalog-contract-generators.md#fixed-payouts). |
| `haulingOrders` | What the contract has to haul (see "Hauling orders"); empty for a contract that hauls nothing |

**Layers.** Sub contract, contract and handler each override the template: the first of them that
states a value wins, the template last. A level states its overrides in `paramOverrides` (sub
contract and contract) or `contractParams` (handler); a sub contract that has no `paramOverrides`
is read for the same fields directly. The `ContractTemplate` Records are found by their type, not
by their folder: on LIVE two sit outside `contracts/contracttemplates`.

On LIVE (2026-10-23, 109 `ContractGenerator` Records, 281 handlers, 2,988 contracts, 2,190 sub
contracts) the layout this reading assumes holds: `contractResults` exists on the contract only,
never on a sub contract or a handler; `factionReputation` (and `reputationScope`) exists on the
handler only, on 215 of the 281; the `contractParams` of a handler and the `paramOverrides` of a
contract have the same nine fields; a sub contract has no `debugName`. No handler `debugName` is a
development leftover, so only the contract and the Record are checked for that.

**Title.** The `Title` string parameter of the layers, then the first display string of the
template's `contractDisplayInfo`, then the Broker title. `projections.missionTitles.rows`
holds both sourced game texts for the same physical contract occurrence. A `~mission(Token)`
or `~mission(Token|Variant)` in it is
replaced, in each language, by what the first layer (sub contract, contract, handler, template
`contractProperties`) with that `extendedTextToken` says:

- a string-hash property with the first usable authored `textId`;
- an organisation property: the first organisation's `stringVariants` entry, with a variant the one
  whose tag's `tagName` equals it, without a variant the only entry of an organisation that has
  exactly one (on LIVE 31 of 69 organisations), else none, so the token stays unresolved: the first
  of several entries is tied to a tag (`_from`, `_title_001`, `_desc_001`) and is not the plain
  name of the organisation;
- a location, AI name or ship spawn property, which the game picks when the mission starts:
  `[LOCATION]`, `[TARGET]` or `[SHIP]`;
- anything else, a token no layer states and a text the language lacks: the token in upper case in
  brackets, e.g. `[CARGO]`.

Nested readable replacements are expanded independently in each available language. An active localization
cycle leaves the bracketed token at the cyclic reference. Missing text and tokens supplied at
mission start retain their placeholders. Leading and trailing blanks are removed from the result.
The sourced key names the canonical GameText row. Each available language has its own derived
value and exact origin. An allocation or cancellation failure stops publication.

**Description (ADR-0264).** The `Description` string parameter of the first layer (sub contract,
contract, handler), else the `description` of the broker entry; the template states none, its
`displayString` slots carry no label. `~mission(Ã¢â‚¬Â¦)` expansion and trimming follow the rules for the
title. It is a game text (`x-gameText`) like the title, and like every game text it must not be
passed on to third parties.

**Broker entry.** A contract that names a `missionBrokerEntry` and has no template (on LIVE the 419
`Covalex_HaulCargo_Career` contracts and their two sub contracts) takes what the layers and the
template leave open from that `MissionBrokerEntry` Record: its `title`, its `type` and, when no
pickup place is left from the prerequisites (none named, or only stars or objects that are not in
`starmapObjects`), its `locationMissionAvailable` as the pickup place.

**Mission type.** `missionTypeOverride` of the first layer that states one, else the `type` of the
template's `contractDisplayInfo`, else of the broker entry; `missionType` is the
`LocalisedTypeName` of that Record.

**Reputation.** `reputationGains.value` lists the changes from successful
`ContractResult_LegacyReputation` results in stored order. A sub contract uses its own result list
when that list has entries and otherwise uses its parent's results. Each row has separately sourced
faction and scope GUIDs and path basenames. `amount.value` is the exact signed decimal string of
the selected Reward Record's `reputationAmount`. Amounts can be negative. On LIVE the 48 negative amounts (of 2,809 success amounts) all belong to
the `CFP_OOS`, `CFP_OOS_Intro`, `HH_OOS` and `HH_OOS_Intro` handlers, twelve contracts each. In
every one the success result names the rival faction (a `CFP_` handler deducts
`factionreputation_unlawful_headhunters`, a `HH_` handler `factionreputation_lawful_citizensforprosperity`)
with the reward `repuationrewardamount_negative_bigdrop` (-190500); they are listed as stored, next
to the handler's own faction in `faction`. The catalog does not add the amounts up (ADR-0065): a
faction scope and an affinity scope are separate entries.

**Contractor.** A sourced reference into `texts.gameTexts` for the authored giver label. The
renderer-independent projection chooses the nearest usable `Contractor` string parameter from the
sub contract, contract and handler in that order. Each layer contributes only its first matching
parameter. A missing, non-key or placeholder value falls back to the next layer. A present
`contractor.value` references `texts.gameTexts[].id.value`, with source uses for the selected key
and the decisions that chose it. When no key resolves, the cell has a null value and reports its
source state.

**Pickup places.** The starmap objects named by the prerequisites of the sub contract
(`additionalPrerequisites`), or, when it names none, of the contract and then of the handler
(`defaultAvailability.prerequisites`): the `availableLocations` of each
`ContractPrerequisite_Locality` (using the exact physical `MissionLocality` Record resolved from its Reference)
and the object of each
`ContractPrerequisite_Location`, in stored order, none twice. A star (`type` `Star`) is left out and
what is not in `starmapObjects` is skipped. The list is ordered by rank, stably: the star system
(`SolarSystem`) first, then planets and moons (`Planet`, `Moon`), then every other type. Every
eligible resolved place is retained. Each has the `starmapObjectGuid`, its `objectType` and the
exact `name` cell from the selected row in `definitions.starmapObjects` as `place`. A present
`place.value` references `texts.gameTexts[].id.value`. The cell retains the source state,
source-use references and encryption marker of the object name. A Filter that leaves the starmap
objects out leaves `pickupPlaces` empty.

**Offered at (ADR-0273).** The same prerequisites as the pickup places, read as authored and
without a filter on the object type. `localityGuids` lists the `MissionLocality` Record of each
`ContractPrerequisite_Locality`, in stored order and none twice. The GUID list retains nominal identities.
The original prerequisite Reference carries the resolved physical Record index. A consumer reads
available places in
`definitions.missionLocalities.rows[].availableLocations.value` and the names in `definitions.starmapObjects`. `locations` lists the
object of each `ContractPrerequisite_Location` as stated. A star stays. For a selected object
`place` carries the same canonical name cell as in `pickupPlaces`. An unresolved or filtered
object retains its `starmapObjectGuid` and has `objectType` and `place` set to `null`. As for
the pickup places, the places of a sub
contract replace those of the contract and its handler. A contract whose prerequisites name no place
at all takes the `locationMissionAvailable` of its broker entry as the only entry of `locations`,
and `brokerFallback` is `true`; otherwise it is `false` and every entry is a stated place. The
difference to `pickupPlaces`: those fall back to the broker location whenever no place resolves
(for example when the prerequisites name only stars), `offeredAt` only when none is stated.

**Giver faction (ADR-0273).** `faction` stays the `factionReputation` of the handler. `giverFaction`
states a faction for the contracts whose handler names none, together with the place that names it
(`source`): `handler` (the same faction as `faction`); else `reputationResult`, the faction of the
first success amount of `reputationGains` with a positive `amount`; else `reputationPrerequisite`,
the faction of the first reputation prerequisite (order of `standingRequirements`) that is not an
exclusion (`exclude` is not `true`); its name is the last part of the Reference's record path, as
for `reputationGains`. The `displayName` on both `faction` and `giverFaction.faction` carries the
sourced reference for the original faction reputation display-name field. A present
`displayName.value` references `texts.gameTexts[].id.value`. An unavailable faction Record or
one outside the faction folder has a null display-name cell while retaining its GUID and name.
An admitted Record without a usable key carries its source state with a null value. A negative
amount names a rival that loses standing and never counts. The catalog
picks the first stated faction and does not weigh several against each other; on LIVE no
`reputationResult` entry has more than one positive faction. The gains are those of the entry's own
resolved results (see "Blueprint rewards"), like `reputationGains`.

**Blueprint rewards (ADR-0273).** The `BlueprintRewards` results (first `missionResults` flag, the
success, set) of the entry's own resolved results: those of the sub contract when it states
results, else those of the contract; `source` is `result`. After them come the enabled
`MissionModifier_TemporaryBlueprint` entries of `modifierOverrides` of the nearest level (sub
contract, contract, handler) that states one, `source` `temporaryModifier` and `chance` `null`.
Each has the `poolGuid` of its `blueprintPool` and, for a result, its `chance` as stored. Which
sub contract hands out which pool is therefore stated per entry, where
`declarations.blueprintRewardSources` names only the `id` that a contract and its sub contracts share.
Three Blueprint reward declarations are in no entry: `BlueprintRewards` results that fire on the
second outcome, not on success (on LIVE the contracts `TheCollector_Vehicle_L22_Wolf`,
`TheCollector_Vehicle_DrakeClipper` and `ORS_Cr_Large`). Every other source of an exported contract
is in its entries; no contract `id` is stated twice with a reward, so the rule that drops an equal
`guid` and `subContractGuid` loses none.

**Known gaps (LIVE, 2026-10-23, 5,175 entries before the template fix).** None of them is stated
elsewhere in the Archive:

- 27 entries have no title: 23 name a title key that has no text in the language, and four name no
  title at all (two `FoxwellEnforcement_MWTEST` contracts whose template Record does not exist, and
  two whose template holds only `@LOC_UNINITIALIZED`).
- 4 entries had no mission type: the two contracts that named a template outside the template
  folder now have one; the other two (`FoxwellEnforcement_MWTEST`) name a template Record that does
  not exist and a type nowhere else.
- 22 titles keep a token in upper case brackets: `[RACETYPE]` (14) and `[ITEM]` (2) are
  `MissionPropertyValue_MissionItem` properties (a `MissionItem` Record, e.g. the race type, or
  an empty list the game fills in). The original item definitions appear in [`definitions.missionItems`](catalog-mission-items.md);
  `[CONTRACTOR]` (6) is an organisation property whose list of organisations is empty (chosen at
  start) or whose organisation has several string variants and the title names none. The tokens
  `[CARGOGRADETOKEN]` and `[REPUTATIONRANK]` (411 entries each), `[CLAIMNUMBER]` and `[TARGETNAME]`
  are run-time tokens the game fills in.
- 642 entries have no pickup place: 206 have no location prerequisite and no broker entry, 175
  name only a star, 87 only a broker entry located at a star, 83 a broker entry without a
  location, 85 only a `ContractPrerequisite_LocationProperty` (a variable the game fills in) and
  6 a locality that lists no object of the starmap.
- Three `BlueprintRewards` results (the contracts `TheCollector_Vehicle_L22_Wolf`,
  `TheCollector_Vehicle_DrakeClipper` and `ORS_Cr_Large`) fire on the second outcome, not on
  success, so no `blueprintRewards` entry lists them although `declarations.blueprintRewardSources`
  does (ADR-0273).

**Reward (ADR-0268).** `fixedRewards.value` lists the parent contract's stored `MissionReward`
payouts in source order. Each payout is selected by the first outcome of a `ContractResult_Reward`.
Sub-contract rows share the parent's list. Calculated rewards are computed outside the stored
data. A contract with only calculated rewards has an empty `fixedRewards.value` list.

**Standing (ADR-0237).** `minStandingGuid` and `maxStandingGuid` are the `minStanding` and
`maxStanding` References of the contract, rows of `definitions.reputationStandings`; a sub contract has none
of its own and shares the contract's. `standingRequirements` lists the
`ContractPrerequisite_Reputation` prerequisites, each with `level` (`handler`, `contract` or `subContract`; `handler`: the
`defaultAvailability` of the handler; `contract` and `subContract`: their
`additionalPrerequisites`; in that order and as stored), `exclude` as stored, the faction, the scope
and the standings it names (`null` for what it leaves out). The catalog does not decide how the
levels combine or what `exclude` inverts. On LIVE 4.10 (2026-10-28) 3,976 of the 5,175 entries
have a range, always both ends; 638 entries have requirements (607 at the contract, 306 at a sub
contract, 21 at a handler; 145 with `exclude`). The Wikelo contracts have a requirement and no
range.

**Cooldowns (ADR-0244, ADR-0271).** Blueprints have no lockout of their own; they are rewards of
contracts, and the contracts carry the cooldowns. Each of the five fields resolves like the contract
facts: the nearest of the sub contract, the contract and the handler that overrides the parameter
of the same name (`HasPersonalCooldown`, `PersonalCooldownTime`, `PersonalCooldownTimeVariation`,
`AbandonedCooldownTime`, `AbandonedCooldownTimeVariation`; case is ignored; the game states the
times as integer overrides and the flag as a bool override), else the `defaultAvailability` of the
handler (`ContractGeneratorHandler_*`, all handler types). The overrides are partial, so one entry
can take its fields from different levels. Each cooldown value, each level in `cooldownLayers`
and the nearest level in `cooldownLayer` carry an immediate source state and provenance. The
[ContractGenerator contract](catalog-contract-generators.md#cooldown-values-and-levels) describes
the contributing inputs and fallback states. On LIVE 4.10, 215 entries
take a field from the contract's overrides, 6 from the sub contract's, and 4,954 from the handler
alone. The numbers are as stored, in floating point. The Archive does not state their unit: no
type, attribute or game text shows it. The range (`personalCooldownTime` up to 2880 in a handler
and 10080 in a contract override, `abandonedCooldownTime` up to 60 at every level) fits minutes,
2880 being 48 hours and 10080 a week, but that is a reading, not a fact of the Archive. On LIVE 4.10 all 5,175 entries state a cooldown.

**Contract facts (ADR-0263).** The sourced `timeToComplete` and `buyInAmount` cells carry the
original results block selection and each selected number's payload or conversion use. See the
[ContractGenerator catalog contract](catalog-contract-generators.md#run-terms) for source states.
Each of the other eight facts contains separate sourced cells for its value and selected layer.
The nearest usable boolean or integer parameter wins (`CanBeShared`, `Illegal`, `OnceOnly`,
`MaxPlayersPerInstance`, `AvailableInPrison`, `CanReacceptAfterAbandoning`,
`CanReacceptAfterFailing`, `HideInMobiGlas`, matched without regard to case) of the sub contract,
the contract (`paramOverrides`) or the handler (`contractParams`). The handler's
`defaultAvailability` provides the fallback for `onceOnly`, `maxPlayersPerInstance`,
`availableInPrison`, the two re-accept flags and `hideInMobiGlas`. The Template supplies
`illegal` from `contractDisplayInfo` and `canBeShared` from the contract class's additional
parameters. A readable contract without a template or applicable overrides has absent cells
for `illegal` and `canBeShared`. A negative `maxPlayersPerInstance` is retained as stored.

`requiredIntros.value` lists the `ContractPrerequisite_CompletedContractTags` required tags of the handler
(`defaultAvailability`), the contract and the sub contract (`additionalPrerequisites`), in that order
and as stored. Each row carries sourced `level` and `tagGuid` cells and a sourced `introContracts`
list. The grant IDs come from the intro contracts whose success result grants the tag
(`ContractResult_CompletionTags`). IDs are ordered and deduplicated by GUID. Each lookup and its grant
IDs retain the authored tag occurrence that selected them. An empty list records that no matching
grant was observed in the admitted scan. Relevant read failures make the list's source set
incomplete. The excluded tags are not exported, and the catalog does not decide how the levels combine. On LIVE 4.10 (2026-10-30) 2,464 of the 5,175 entries
have a required tag and 598 have one an intro contract grants; the facts are stated by the handler
for most values (`onceOnly` 5,049 of 5,175), by the contract for 96 `onceOnly` and 365 `illegal`
values, and by the template for 4,501 `canBeShared` values.

**Hauling orders (ADR-0265).** `haulingOrders` lists what a contract has to haul. The Archive
states it in three places: a `MissionPropertyValue_HaulingOrders` property in the overrides of a sub
contract or contract, the parameters of the handler or the `contractProperties` of the template;
the hauling objectives (`ObjectiveHandler_Hauling`) of the template; and the same objectives of the
`MissionBrokerEntry` the contracts without a template use. An order has

| Field | Meaning |
|-------|---------|
| `source` | `subContract`, `contract`, `handler` or `template` for a property of that level, `templateObjective` or `brokerObjective` for the orders of an objective |
| `variableName` | The `missionVariableName` of the property; `null` for an objective order |
| `requirements` | What is hauled: per requirement `options`, the alternatives of which one is hauled, each a list of items that are all hauled. A plain order is one option with one item; an `or` has one option per alternative |

Of the property levels the nearest wins per `variableName` (the layer rule above; a property without a name shadows nothing); the objective
orders are those of the template, else those of the broker entry. An item has a `kind` and the
fields of that kind, the others `null`:

| `kind` | Fields |
|--------|--------|
| `resource` | `resourceGuid` (the `subtypeGuid` of a commodity), `minScu`, `maxScu`, `maxContainerSize` |
| `entityClass` | `entityClassGuid` (the `EntityClassDefinition` Record), `minAmount`, `maxAmount` |
| `entityClasses` | `entityClassesGuid` (a `Hauling_EntityClasses` Record, a list of entity classes of which any counts; no kind lists these Records), `minAmount`, `maxAmount` |
| `missionItem` | `minAmount`, `maxAmount`; the item itself is a mission property the game fills in |

The numbers are as stored: `maxSCU` and `maxAmount` of 0 and a `maxContainerSize` of -1 mean no
limit. The pickup and drop-off of an objective order are references to template properties that
the game fills in when the mission starts, so they are not exported; where a contract is offered
stays in `pickupPlaces`. Objective orders that only name a drop-off, point at a property or name a
mission item are left out. On LIVE 4.10 (2026-10-30) 1,966 of the 5,175 entries haul: the
orders come from the contract (1,440), the template objective (369), the broker objective (419),
the template (53) and the sub contract (18); the items are 2,587 resources, 647 entity classes,
109 entity class lists and 988 mission items; 5 of the `entityClassGuid`s name an entity class that no kind of the catalog lists (items, ships, commodities). 11 of the resources are no commodity of the catalog
(a `ResourceType` that no commodity Record carries).

### Factions, reputation and missions

Each reputation definition table keeps original Record identity and per-value origins.
Record links preserve the original GUID and expose the state of unresolved source references.
Catalog filters can omit a linked target while retaining its stored identity.

**Faction reputation.** The `definitions.factionReputations.rows` table holds physical
`FactionReputation` Records in GUID and physical Record order. The technical `name`, the
logo paths, the original flags and the decimal `geid` value are sourced fields. `background`
retains its original String and carries `x-gameText`. `displayName` refers to the protected
text registry. `description` distinguishes an authored Locale reference from literal String
content and carries `x-gameText`.

The `allies`, `enemies`, `reputationContextPropertiesUi` and `perkReputationRewardList`
fields retain their original Record links. Each `hostilityParams` and `alliedParams` body keeps
its scope, standing and owned relation marker. The marker retains its protected description and
icon path. In `propertiesBb`, the original `entityLawful` property carries a Boolean
`dynamicProperty` value. The table preserves the other typed blackboard values in source order,
including the exact Single bits and protected Locale references. Original unprojected fields
remain in `authoredFields`.

#### Faction definitions

`definitions.factions` holds physical `Faction` Records keyed by GUID, in GUID and physical
Record order. Its selection uses the exact owning type and the normal Record path Filter. The
table's `availability` and `completeness.factions` describe source declaration and Filter
coverage independently.

Every row holds `defaultReaction`, `factionType`, the stored `alliedFactions` and
`enemyFactions` Reference lists and `friendlyFireBehaviorOverrides` in source order. Each
override gives its original reaction label and `shouldAllowFriendlyFire`. The source flags
`ableToArrest`, `policesLawfulTrespass`, `policesCriminality` and `noLegalRights` keep
false distinct from an absent declaration. `factionReputationRef` links the original independent
reputation GUID and keeps unresolved identities. These are static Record facts.

The sourced `name` and `description` hold a `localized` canonical text Reference or a
`literal` authored String. The name uses the public short-name domain and the description uses
the protected `x-gameText` domain. `authoredFields` holds remaining field occurrences with
their source declarations and values. The same rules apply to each owned override body.

**Scopes, standings and contexts.** A scope in `definitions.reputationScopes.rows` retains its
physical Record identity, `scopeName`, protected `displayName` and `description` references, and
`icon` path. The owned `standingMap` preserves the original `reputationCeiling` and
`initialReputation` in their authored numeric widths and each `standings` reference in stored
order. Filtered and dangling targets retain their original GUIDs. Source fields and links carry
individual provenance. The standing definitions under `definitions.reputationStandings.rows` retain physical
Record identity and join to stored standing GUIDs from scopes, scenarios and contracts.
Each definition includes the original `name`, `description`, `displayName`, `perkDescription`,
`icon` and `missionBoardIcon` fields. The rank name and perk text use the protected
localization table. The literal description also carries `x-gameText`. Icon values are paths.
The progression fields preserve `minReputation` and `driftReputation` with their original
numeric widths and `gated` with its source state. `driftTimeHours` keeps the original
Single in hours with exact IEEE bits and a nullable finite projection. Each sourced field
has its original contributors. Additional fields remain in the shared source arena.
Each row in `definitions.reputationContexts` preserves the authored `sortOrderScope`,
`primaryScopeContext` and ordered `scopeContextList` with their original scope links and
dynamic display properties. The scope of a gain in `reputationGains` is an entry of
`definitions.reputationScopes`.

**Reputation perks.** Each row of `definitions.reputationPerks` is an original
`SPerkReputationListParams` Record. The `perks` array preserves the authored entry order,
scope and standing References, and the concrete body of each `perk` pointer. Item perks
retain their reward item Reference. Category perks retain their original selectors and
the authored discount Single with its exact bits when present. The technical `name`
and `icon` path are sourced strings. The literal `description` and localized
`displayName` and `displayDescription` carry `x-gameText`. Unclassified concrete perk
variants retain their original fields in the row's source arena.

**Sandbox triggers.** The `configurations.reputationTriggers` table contains the ordered
`sandboxTriggers` occurrences owned by each physical faction reputation Record. Join the
sourced `owner.guid` and `owner.recordIndex` to the faction definition. A trigger
can contain an inline manual definition or a Reference to a reusable trigger Record.
Null slots and present empty lists retain their own source state. Unclassified variants
retain their original fields and produce a coverage diagnostic.

**Sandbox trigger definitions.** Each row in `definitions.reputationTriggerRecords` is a
physical `SandboxTriggerRecord` with its ordered `triggerManualParams`. A manual trigger
carries the original event label or infraction Reference, the `triggerOnInnocentsOnly`
flag and ordered outcomes. Each outcome keeps its scope and reward amount References.
Faction outcomes also keep the original faction reputation Reference. The
`definitions.reputationRewardAmounts` table contains the physical
`SReputationRewardAmount` rows, each with an authored `editorName` and a signed Int32
`reputationAmount`. Each exported value carries its own source provenance.

The ignored `original_reputation_owners_perks_and_triggers_match_the_scoped_catalog`
source test reads the seven physical reputation definition families and their owned
configuration from the installed DataForge. Its Record Filter is
`libs/foundry/records/*reputation/*`. The test compares physical identity, authored order,
field states, original values and selected source coordinates in one diagnostic export.

**Mission types (`definitions.missionTypes`).** Match `contracts[].missionTypeGuid` to
`definitions.missionTypes.rows[].guid.value`. Each row uses the canonical sourced Record header.
`typeName` is a sourced protected reference from
`LocalisedTypeName`. Join its `value` to `texts.gameTexts[].id.value` to read the original key and
the available source and display forms. A missing, placeholder or literal name provides no
reference. Every `missionTypeGuid` of `contracts` points to a mission type.

**Mission localities (`definitions.missionLocalities`).** Match each `offeredAt.localityGuids` value
to `rows[].guid.value`. The authored `availableLocations` array retains its original order,
including empty, repeated and dangling slots. A resolved slot carries the original reference GUID
and separate target-header evidence. A dangling slot retains its GUID and resolution status.
The game's name for the locality is its sourced `className`. Contract pickup places continue to
use the resolved original members.

**Contract scenarios (`contractScenarios`).** Each `ScenarioProgress` Record carries the canonical
physical header. Rows use GUID order with Record index as the tie-breaker. Read
`factionRewardTiers.value`, `progressions.value` and `tiers.value` as ordered arrays of sourced
slots. A null or failed slot retains its position and source state.

A faction slot keeps the original `faction` Reference, including its GUID, physical target and
resolution status. `factionName` is the last segment of the selected compatible Faction path.
`factionReputation` is the original `factionReputationRef` of that physical Faction. Its field
and the selecting Reference supply its origin. A missing compatible target leaves the derived
facts absent. A wrong-family target makes those facts invalid. A failed body read is unreadable.

A progression retains its sourced multiplier, `periodMissionScenario` Reference and concrete
`completionType` from the declared scalar StrongPointer. Its `authoredFields` arena also keeps the
original completion pointer declaration, ordered contents and references, including
`completionTags` and additional or unrecognized members. Unsupported storage does not produce
a scalar completion type and remains available in that arena. A reward tier keeps its original
numeric storage and ordered `blueprintPools` References, including dangling and null slots.
The original badge label `None` denotes no awarded badge.

Contract templates are described in "Contract templates", mission broker entries in "Mission broker
entries", ship spawns and the locations of contracts in "Ship spawns and location queries" and NPC
and entity spawns in "NPC and entity spawns".

### Ship spawns and location queries

Three kinds and three contract fields (#902, ADR-0266) state
what a contract spawns and where it can take place, as the Archive stores it. The catalog never
resolves what the game picks when the mission starts.

On LIVE (2026-10-31) the kinds hold 564 ship spawn descriptions, 1,742 location queries and 2,092
location templates (12 more are development leftovers, skipped). They add about 5.1 MB of JSON
(3.0 MB templates, 0.7 MB ship spawns, 1.4 MB queries) and the three contract fields about 2.2 MB
(7,901 ship spawn links on 2,285 entries, 8,681 location query links on 4,732 entries, 479
location prerequisites).

**Properties of a contract.** A mission property has a `missionVariableName` and a value; a
`MissionPropertyValue_ShipSpawnDescriptions` states ships, a `MissionPropertyValue_Location` or
`_Locations` a location. They are found at four levels, resolved like the other fields of a
contract: the first of sub contract, contract, handler and template that states a variable wins
(`level` names it: `subContract`, `contract`, `handler`, `template`, and `broker` for the
properties of a mission broker entry). A property without a variable
name is skipped. A ship spawn description without a group still takes the place of the same variable
at a lower level and is itself left out.

| Field of `contracts` | Meaning |
|----------------------|---------|
| `shipSpawns` | `variable`, `token` (`extendedTextToken`, `null` when empty), `level` and `poolGuid`, an entry of `contractShipSpawns`; ordered by variable |
| `npcSpawns`, `entitySpawns` | The same links for the NPC and entity spawn descriptions (see "NPC and entity spawns"): `poolGuid` is an entry of `contractNpcSpawns` or `contractEntitySpawns` |
| `locationQueries` | `variable`, `token`, `level` and `queryGuid`, an entry of `contractLocationQueries`; ordered by variable |
| `propertySetGuids` | Links to `missionPropertySets` in handler, contract and sub-contract order, preserving source declarations without computing runtime precedence |
| `locationPrerequisites` | The `ContractPrerequisite_LocationProperty` prerequisites of the handler, the contract and the sub contract, in that order, as stored: `level` (`handler`, `contract`, `subContract`), `variable` (the `propertyVariableName`, a `variable` of `locationQueries`; on LIVE 4 of the 479 name a variable no query states), `token` and `locationLevelType` |

**Identifiers.** The Archive gives these inline structures no GUID, so equal content shares one
entry and its `guid` is the first 16 bytes of the SHA-256 of the canonical JSON of the content,
written as a UUID. It is the catalog's own identifier, not a GUID of the Archive; it holds as long
as the content does and must not be stored across a change of the description (ADR-0266). Both
kinds are ordered by it. Every link resolves.

**Ship spawn descriptions (`contractShipSpawns`).** The Archive has no ship pool Record: the game
searches ships by tags. An entry has `allowedForMissionRestrictedDeliveries` and `groups`
(`SpawnDescription_ShipGroup`: `name`, `null` when empty, and `slots`; a slot is a
`SpawnDescription_ShipOptions`, the alternatives for one ship, each an `options` entry with
`weight`, `concurrentAmount`, `includeLocationAISpawnTags`, the tag sets `tags`, `negativeTags`,
`markupTags` and `entityTags`, `initialDamageGuid` and `namePropertyVariable`). Tags are full tag
paths (`AI.Ship.SpawnFlags.PoweredOff`, `Ship.Model.Cutlass_Black`, `AI.Faction.Criminal`); what a
tag means (ship, role, faction, cargo manifest, wave) is not stated by the Archive and the catalog
does not interpret it. A weak pointer to another property of the Record (`nameProperty`) is the
`missionVariableName` of that property, `null` when it has none. The embedded AI names of 7 groups
are not exported.

**Location queries (`contractLocationQueries`).** A query has `count` (`one` for
`MissionPropertyValue_Location`, `many` for `_Locations` with `minLocations`, `maxLocations` and
`failIfMinNotFound`; `null` for `one`), `logErrorOnSearchFail`, `resourceTags` and `conditions`
(`matchConditions`, all of which a location satisfies), a closed list tagged by `condition`:
`tagSearch` (`tagType` and `terms` with `positiveTags` and `negativeTags`), `excludeNearbyLocations`
and `excludeDistantLocations` (`rangeKm` and `locationProperty`, a variable name),
`excludeProperty`, `propertyTags`, `specificOrganizations` and `specificItems`. A condition of a
struct the catalog does not know fails the Record that holds the query (ADR-0326). The schema
still lists the deprecated condition `other` (`structName`); the catalog never writes it. A
condition's weak pointer is the `missionVariableName` of the property it points to; on LIVE 83 of
431 distance conditions point to a property that has no variable name and have `null`. The game
runs the query against the location templates when the mission starts, depending on the other
properties of the mission (distance, organisation, items); the catalog exports the query as stored
and never a list of resulting places or starmap objects. The Archive links a query to a starmap
object only through the pickup `MissionLocality` Records (see "Contracts").

**Location templates (`missionLocationTemplates`).** A `MissionLocationTemplate` Record under
`missiondata/pu_locations` (one more of the type lies outside that folder, a test platform, and is
not part of the kind): `guid`, `className`, `recordPath`, `disabled`, `isSecurityNetworkHost`, the
`tags` (`generalTags`) a query matches, `producesTags`, `consumesTags` (`positiveTags` and
`negativeTags`), `aiSpawnTags`, `names` and `entityClusterMemberGuid`. Each `names` entry keeps
its `tag` operand and a sourced protected `displayName` reference from that original string variant.
The four tag fields are sourced cells. Each list preserves the original slot order, including
null, repeated and dangling entries. Each operand carries a sourced `path` and an optional sourced
`danglingGuid`. A present path cites its original operand and the Tag graph inputs that produced it.
When the original operand supplies the fallback GUID because the graph has no readable path,
the original GUID is the payload. An incomplete lookup retains `sourceSetComplete: false`.
Each `producesTags` and `consumesTags` term carries sourced positive and negative operand lists.
Join each `displayName.value` to `texts.gameTexts[].id.value` to read the original key and the
available source and display forms. Repeated keys share a text row and retain their individual
occurrences. The sourced `classification` holds the three
original location class, function and size References. See [the location vocabulary contract](catalog-location-vocabulary.md).
The sourced location configuration retains authored NPC auto-spawn settings, mission limits and
the default Reference to [security network manifests](catalog-security-network-manifests.md),
plus authored mission modules and their initialization inputs.
See [mission location configuration](catalog-location-configuration.md).

Not exported: SCMDB's own pools, waves and region lists, which are derived and not stated by the
Archive.

### Authored mission property sets

`missionPropertySets` preserves the complete typed `propertyOverrides` declarations of Generator
handlers, contracts and sub-contracts. Each source array is emitted once and contract rows refer
to it through ordered `propertySetGuids`. The owner GUID, Record path and struct, exact authored
location and level remain available, along with ordered nullable declarations containing exact
variable/token strings and the shared `PropertyValue` union. Empty strings stay empty in this
source kind. Stable identifiers depend on the owner GUID and authored array position, so a value
edit does not change identity. See [the full field and identity contract](mission-property-sets.md)
and ADR-0345. Template and broker property fields keep their existing meanings.

### Contract templates

`contractTemplates` lists the `ContractTemplate` Records (#812, ADR-0313) in Catalog schema 3,
ordered by GUID. `contracts[].templateGuid` is the `guid` of an entry; every one
resolves. A template is the structure a contract lays the overrides of its handler, itself and its
sub contract over. The Records are selected by the struct, in any folder: on LIVE (2026-10-31)
496 Records, 488 of them under `contracts/contracttemplates`, 335 named by a contract, none left
out. A development leftover is told by the words of the usual rule except `template`, which live
templates such as `handyman_template` carry. An entry states the fields of the Record as stored:
identifiers, flags and numbers, links, and the texts the Archive keeps with them as game texts
(`x-gameText`). Where a field holds one of several structs, the entry names the struct in a
discriminator: `type` for the value of a property, a flow condition and a flow action, `modifier`
for a modifier and `structName` for a handler; the other fields depend on it. A condition, an action
or a modifier of a struct the catalog does not know fails the Record (ADR-0320), and so does the
value of an objective property (ADR-0324). The unions lost their `other` variants before the kind
`contractTemplates` was first released (in 0.4.2), so no consumer has seen them and the
additive rule of ADR-0054 is not touched.

| Field | Meaning |
|-------|---------|
| `guid`, `className`, `recordPath` | The Record |
| `notForRelease`, `workInProgress` | The flags as stored, `null` when absent |
| `title` | The first display string (game text); `null` texts for the placeholder most templates have |
| `missionTypeGuid` | The `MissionType` of the display info, an entry of `definitions.missionTypes.rows` |
| `illegal`, `showLifeTimeInMobiGlas`, `preShowObjectives` | The flags of the display info |
| `classType`, `settings` | The struct of the contract class (`ContractClass_Contract`, one template is a `ContractClass_PVPBounty`) and its `ActiveContractSettings`: `canBeShared`, `hasCompleteButton`, `handlesAbandonRequest`, `displayAlliedMarkers`, `onlyOwnerCanComplete`; `null` when absent |
| `autoFinish` | The `ContractAutoFinishSettings`: `failIfSentToPrison`, `failIfBecameCriminal`, `failIfLeavePrison` and the deadline as stored: `completionTime`, `autoEnd`, `resultAfterTimerEnd` (an objective state), `remainingTimeToShowTimer`; the Archive does not state the unit of the times; `null` without the block |
| `contractProperties` | The mission properties in stored order: `variable`, `token` (`null` when empty), `valueType`, the struct of the value without `MissionPropertyValue_` (`StringHash`, `Location`, `ShipSpawnDescriptions`, `HaulingOrders`, ...), and `value`, the union of the objective properties (see "Objective properties"); `null` when the property has no value |
| `shipSpawns`, `npcSpawns`, `entitySpawns`, `locationQueries` | The links of ADR-0266 and ADR-0324, one per variable the template states, with `level` always `template`, into `contractShipSpawns`, `contractNpcSpawns`, `contractEntitySpawns` and `contractLocationQueries` |
| `haulingOrders` | The hauling orders of the template as in "Hauling orders": the properties (`source` `template`) and the orders of its hauling objectives (`templateObjective`) |
| `objectives` | The objective tokens depth first, each phase after its parent (see "Objectives") |
| `partialPayout` | The `PartialContractRewards`: `thresholds`, each `minPercentage`, `maxPercentage`, `currencyRewardMultiplier` and `reputationMultipliers` (`scopeGuid`, a row of `definitions.reputationScopes`, and `reputationRewardMultiplier`); `null` without one |
| `modifiers` | The modifiers in stored order, tagged by `modifier`: `securityClearance` (`clearanceTokenGuid`), `requestRentShip` (`itemGuid`, `durationSeconds`, `clearRentalOnFail`), `shipKioskFilter` (`terms`, each `positiveTags` and `negativeTags`), `factionHostility` (`factionGuid`, a `Faction` Record no kind lists, `myReaction` and `theirReaction` as `friendly`, `neutral` or `hostile`, `ignoreCriminalHostility`), `hostileMission` and `ignoreMissionPlayerCriminality` (`brokerEntryGuid`, an entry of `missionBrokerEntries`; `legalToAttack` for the first) and `lawLicense` (`licenseType`: `illegalGoodsLicenseClassA`, `B` or `C`); each with the shared `modifierName`, `enabled` and `locationProperty`, the variable of the property the weak pointer names |
| `owner` | The `DevOwner` Record: `guid` and `className` (e.g. `MDT`); `null` without one |
| `missionFlow` | The triggers in stored order (see "Mission flow") |
| `startComms`, `endComms` | The `contractStartCommsNotification` (`delay`, `communicationTag` as a tag path; `null` without one) and the `contractEndCommsNotifications` (`reason` `complete`, `fail` or `abandon`, `delay`, `communicationTag`) |

**Objectives.** Each entry has `id`, `parentId` (`null` at the top level), `debugName`,
`startsActive`, `variableName` and `weight` (set on a child phase the flow may pick at random),
`phaseTag` (a tag path), `display` (`category` `primary`, `hideOnHud`, `markerLabel`,
`shortDescription`, `longDescription`; the three texts are game texts), `overrideDisplay`
(`titleOverride` and `descriptionOverride`, game texts), `rewardContribution` (`structName`,
`percentContribution`, `doPartialRewardPayout`; `null` without one), `comms` (`trigger` an objective
state, `timing` `simultaneous`, `commsNotificationFirst` or `hudNotificationFirst`, `delay`,
`communicationTag`), `objectiveProperties` (see "Objective properties") and the `handler`. Most
texts of a template are the placeholder `@LOC_UNINITIALIZED`; such a text has a `null` key and no
texts.

The `handler` has `structName` (`ObjectiveHandler_Hauling`, `_Local`, `_NearLocation`,
`_PlayerAttached`, `_TransitToInstance` or `_EntityAttached`) and the fields of the struct; a field
the struct does not have is `null`: `module` (a mission scripting file), `moduleHierarchyGuid`,
`disableTravelObjectives`, `disableReturnObjectives`, `travelRadiusKm`, `allPlayersLeftGracePeriod`,
`locationProperty`, `organizationProperty`, `haulingLimitProperty` and `targetProperty` (the
`missionVariableName` of the property the weak pointer names), `minDistance`, `maxDistance`,
`hideMarkers`, `navPoint` (`entityClassGuid`, `displayName`, a game text,
and `extendedTextTokens`), `travelObjective` and `returnObjective` (displays as above), `hauling`
(the `Hauling_ObjectiveSettings`: five extended text tokens and seven displays) and `instance`
(`completePhaseOnPlayerEnter`, `enterTravelRadiusKm`, `hudObjectiveText`, a game text,
`tagIdentifier`, `searchTags`, `objectiveMarker`). The orders of a hauling handler are
`haulingOrders` of the template. The local handler's original `securityManifestToOverride`
Reference joins the [canonical security network manifest](catalog-security-network-manifests.md#joining-an-objective-override).

**Objective properties.** An entry of `objectiveProperties` has `kind` (`referenced`, `output`,
`embedded` or `input`, the struct `ObjectiveProperty_*`), `variable` and `token` (the
`missionVariableName` and `extendedTextToken`, `null` when empty), `contractProperty` (for a
referenced property the `missionVariableName` of the property of the template it points to),
`value` (for an output, embedded or input property) and `linkedOutput` (for an input: the
`objectiveId` of the objective that holds the output and its `variable`). The `value` is tagged by
`type`: `object` and `deliveryOrder` (no fields), `boolean` (`value`), `stringHash` (`options`, each
`displayName`, a game text, `value`, fractional `weighting`, `debugForceChooseThisOption` and
structured `dependentProperties` with the property and selected option links), `integer` and
`float` (`options`, each with the same metadata and `value`/`variation`; integer fields retain
exact `Int32` values), `tags` (`tags`, `negativeTags`, raw `sourceTags`/`sourceNegativeTags` and
complete `options`),
`shipSpawnDescriptions` (`poolGuid`, an entry of `contractShipSpawns`, `null` for a description
without a group), `location` (`queryGuid`, an entry of `contractLocationQueries`),
`npcSpawnDescriptions` and `entitySpawnDescriptions` (`poolGuid`, an entry of `contractNpcSpawns` or
`contractEntitySpawns`, `null` for a description without a group) and `organization` (`conditions`,
the `matchConditions` as for a location query, and `forcedOrganizationGuid`, the
`debugForceChosenOrganization`, a `MissionOrganization` Record no kind lists). The contract-level
properties add `missionItem` (`conditions`, the `matchConditions` as for a location query,
`forcedItemGuid`, the original debug-only `debugForceChosenItem` reference to a `definitions.missionItems` row when selected, `minItemsToFind`
and `maxItemsToFind`), `aiName` (`randomName`, `randomLastName`, `randomNickName`,
`givenName`, `givenLastName`, `givenNickName`, `chanceOfNickName` and `nameDataGuid`),
`combinedDataSetEntries` (`entryProperties`, the variables
of the `dataSetEntryProperties`), `haulingOrders` (`requirements` as in "Hauling orders"), `reward`
(`reward`, the `rewardDef` as the `reward` of a broker entry, `null` without one), `timeTrialRace`
(`targetSplits`, ordered structures with `targetSplit`) and `starMapLocation` (no fields).
`autoSpawns` retains its definition Reference and concrete closet-search options; `investigation`
retains journal entries, suspects, locations and every clue/item subtype. Their complete fields,
null conventions and owner-local weak references are described in
[Typed mission property values](mission-property-values.md). The spawn descriptions and location
queries are read as for contracts and added to their kinds. An unsupported value or data-set
condition fails its owning Record instead of being represented as absent data.

**Scoped location properties (schema 3).** `scopedLocation` retains all six optional source
filters: `factionFilter`, `locationClassFilter`, `locationSizeFilter`, `locationFunctionFilter`,
`otherLocationFilter` and `relativeLocationFilter`. Their concrete required/excluded/same/different
variants, raw enum names, ordered definition References and owner-scoped weak links are described
in [Scoped mission-location values](scoped-locations.md), including null/empty states, source
identity and unresolved/error statuses. No runtime-selected location is calculated. These values
use the same reader in objective, template, broker and Generator source-set mission properties.

**Contract-level properties.** The `value` of an entry of `contractProperties` of a template and of
`missionProperties` of a broker entry is read by the reader of the objective values and states the
same union; `valueType` stays. On LIVE every property that has a value struct states one, and the
values grow the catalog from 208.5 MB to 214.1 MB decompressed and from 3.82 MB to 3.89 MB as
`.json.zst`.

**Mission flow.** Each `configurations.missionFlows` row points to the original Template or Broker
owner and its flow declaration. The trigger, condition and action meanings are stored on nodes
of that owner's `source.values` arena. References identify those owner-local nodes. Triggers retain
the source description and the selected condition and action. Conditions and actions are tagged
by `type`. A leaf action can be a `stateChange`, `tokenStateChange` or
`pickRandomMissionPhase`. The last stores ordered phase references. Weak objective links carry
the original resolution state and owner binding.

- `multipleActions` stores the original `actionsField` and its ordered `actions` references.
- `pickRandomAction` stores the original `actionsField` and its ordered `entries`. A declared
  entry links its original `value`, `actionField`, optional child `action` and
  `probabilityField`. `probabilityInterpretation` describes the original Single value. Its
  exact IEEE-754 bits and protection marker are read through the field reference.

The arena preserves repeated occurrences, authored nulls and readable prefixes of malformed
data. Action and condition graphs are traversed iteratively. Canonical reader resource-policy
errors and unsupported required meanings prevent complete Normal publication. Diagnostic
publication retains readable owner data and the original error in `exportHealth`.

### Mission broker entries

`missionBrokerEntries` lists the `MissionBrokerEntry` Records (#813, ADR-0320) in Catalog schema 3,
ordered by GUID: the offers of the mission board. `contracts[].brokerEntryGuid`
is the `guid` of an entry. The Records are selected by the struct, in any folder (on LIVE
2026-10-31 all 2,584 lie under `missionbroker`); the 156 whose path or name holds a word of the
usual leftover rule are left out, so 2,428 entries remain, 419 of them named by a contract, every
link resolves. An entry states every field of the Record as stored, and the texts as game texts
(`x-gameText`); a Record of a struct the catalog does not know in a modifier, a flow condition or
action, or the reputation requirements fails (none on LIVE). The objectives, handlers, hauling
orders, partial payout, modifiers, mission flow, owner and the `settings` have the shapes of the
contract templates (see there).

| Field | Meaning |
|-------|---------|
| `guid`, `className`, `recordPath`, `notForRelease` | The Record and its flag as stored |
| `debugName`, `title`, `hudTitle`, `description`, `missionGiver`, `commsChannelName`, `giverFragmentTags` | The `playerFacingDebugName`, the five game texts and the `missionGiverFragmentTags` |
| `missionTypeGuid`, `locationGuid`, `localityGuid` | Links to `definitions.missionTypes.rows`, `starmapObjects` (the `locationMissionAvailable`) and `definitions.missionLocalities.rows` |
| `giverGuid` | The `missionGiverRecord`; no kind lists these Records |
| `invitationMissionGuid`, `linkedMissionGuid`, `associatedMissionGuids`, `requiredMissionGuids`, `unlessAvailableMissionGuids` | Links to entries of this kind (`invitationMission`, `linkedMission`, `associatedMissions`, `requiredMissions`, `onlyAvailableIfAllMissionsNotAvailable`); a GUID the Archive lacks is kept |
| `journalEntriesOnComplete`, `journalEntriesOnRemove` | GUIDs of journal Records; no kind lists these |
| `module`, `modulePerPlayer`, `moduleHandlesOwnShutdown` | The mission scripting file and its two flags |
| `completionTags`, `missionTags`, `requiredAreaTags`, `excludedAreaTags`, `requiredCompletedTags` | Tag paths; the last is a list of terms with `positiveTags` and `negativeTags` |
| `owner` | The `DevOwner` Record: `guid` and `className`; `null` without one |
| `difficulty`, `preShowObjectives`, `showLifeTimeInMobiGlas`, `buyInAmount`, `refundBuyInOnWithdraw` | The values as stored |
| `settings` | The `canBeShared`, `hasCompleteButton`, `handlesAbandonRequest`, `displayAlliedMarkers` and `onlyOwnerCanComplete` flags |
| `availability` | The flags `initiallyActive`, `showAsOffer`, `requestOnly`, `onceOnly`, `notifyOnAvailable`, `availableInPrison`, `lawfulMission`, `tutorial`, `canReacceptAfterAbandoning`, `canReacceptAfterFailing`, `applyCompletionTagsOnAbandoned` and `applyCompletionTagsOnFailed`, and the `wantedLevelMin` and `wantedLevelMax` the player may have |
| `schedule` | The `availableDateSchedule` windows: `start` and `end` (`year`, `month`, `day`, `hour`, `minute`, `second`), `repeatEnd` (the same without a time of day) and `repeatType` (`repeatYearly`); a month is `january` to `december` |
| `cooldowns` | `hasPersonalCooldown`, the personal and abandoned cooldown times with their variations and the respawn time with its variation; the Archive does not state the unit |
| `instances` | `maxInstances`, `maxInstancesPerPlayer`, `maxPlayersPerInstance`, `hasLifeTime`, `lifeTime` and `lifeTimeVariation` |
| `reward` | The `missionReward`: `currency` as stored (`UEC`, `MER` or `INVALID`), `reward`, `max`, `plusBonuses` and `reputationBonusGuid`; `null` without one |
| `finish` | `failIfSentToPrison`, `failIfBecameCriminal`, `failIfLeavePrison` and the deadline: `autoEnd`, `completionTime`, `endReason` (game text), `resultAfterTimerEnd` (an objective state) and `remainingTimeToShowTimer` |
| `standingRequirements` | The reputation requirement expression in stored order, tagged by `type`: `requirement` (`comparison`, `factionReputationGuid`, `scopeGuid`, `standingGuid`), `and`, `leftParenthesis` and `rightParenthesis`; the elements form an infix expression and the catalog does not evaluate it |
| `reputationResults` | The `missionResultReputationRewards`: one element per stored position (`index`, the Archive names no result) with its `amounts` (`factionReputationGuid`, `scopeGuid`, `rewardGuid` and the `amount` of that reward Record) |
| `missionProperties` | The mission properties in stored order: `variable`, `token`, `valueType` and `value`, as for the contract templates |
| `shipSpawns`, `npcSpawns`, `entitySpawns`, `locationQueries` | The links of ADR-0266 and ADR-0324 with `level` `broker`, into `contractShipSpawns`, `contractNpcSpawns`, `contractEntitySpawns` and `contractLocationQueries`, which hold the descriptions of the entries too |
| `haulingOrders` | The orders of the hauling objectives (`source` `brokerObjective`) as in "Hauling orders" |
| `objectives`, `partialPayout`, `modifiers`, `missionFlow` | As for the contract templates; `comms` of an objective is empty, an entry has no start or end comms |

A comparison is `equalTo`, `greaterThan`, `greaterThanOrEqualTo` or `lessThanOrEqualTo`. A stored
name the catalog does not know (a comparison, a month, a repeat type, a reaction, a licence type)
is `null` and counted in the warning `unknown_template_values`, like those of the templates. The
tokens of the objective values are the same as there. The values of the `properties` are read like
those of the contract-level properties of a template.

On LIVE (2026-10-31) the kind holds 2,428 entries with 703 objective tokens, 26,741 properties,
949 modifiers, 2,097 flow triggers and 1,666 requirement terms, 16.9 MB of JSON. The catalog grows
from 187.2 MB to 205.9 MB decompressed and from 3.44 MB to 3.78 MB as `.json.zst`; the upload
limit is 64,000,000 bytes.

### NPC and entity spawns

Two kinds and the fields `npcSpawns` and `entitySpawns` of a contract, a contract template and a
mission broker entry (#815, ADR-0324) state the people and the
objects a contract spawns, as the Archive stores them. The catalog never resolves which NPC or
entity the game picks. The Archive has no Record for these descriptions: a
`MissionPropertyValue_NPCSpawnDescriptions` or `_EntitySpawnDescriptions` sits in the properties of
a contract, a template or a broker entry. The kinds follow the design of `contractShipSpawns`
(ADR-0266): equal content shares one entry, its `guid` is the content hash of the entry written as a
UUID (the first 16 bytes of the SHA-256 of the canonical JSON of the groups; not a GUID of the
Archive, valid as long as the content, not to be stored across a change of it), and the kinds are
ordered by it. The hash is made over the localisation keys of the names and not their texts, so it
does not depend on a language. The links are the `SpawnPoolLink` of "Ship spawns and location
queries": `variable`, `token`, `level` and `poolGuid`, resolved by the same rules (the nearest level
wins, a description without a group is left out and still hides the same variable at a lower level).
Every link resolves.

On LIVE (2026-10-31) the kinds hold 411 NPC spawn descriptions (2,074 options) and 43 entity spawn
descriptions (556 options). They add 2.1 MB of JSON; the links on 5,175 contracts (544 NPC and 182
entity links), 496 templates (7 and 10) and 2,428 broker entries (793 and 146), every entry with the
two fields, add 0.5 MB. The catalog grows from 205.9 MB to 208.5 MB decompressed (limit
1,000,000,000 bytes) and from 3.78 MB to 3.82 MB as `.json.zst` (upload limit 64,000,000 bytes).

**NPC spawn descriptions (`contractNpcSpawns`).** An entry has `guid` and `groups`
(`SpawnDescription_NPC_Group`: `name`, `null` when empty, and `options`). An option
(`SpawnDescription_NPCOption`) is one alternative of the group and has `priority` (`-1` as stored
means none), `includeLocationAISpawnTags`, `weight`, `autoSpawnSettings` (see below),
`identifierTags` (tag paths), `namePropertyVariable` (the `missionVariableName` of the property the
weak pointer `nameProperty` points to, `null` when it points to none) and `embeddedName`.

The `autoSpawnSettings` (`AutoSpawnSettings`) have `name` (a developer's label such as
`SC_CE -> DA_I (Grenadier x1)`, `null` when empty), `initialActivity` (as stored: `Sentry`,
`Guard`, `GuardPatrolling`, ...; `null` when empty), `excludeShipCrew`, `excludeSpawnGender` (the
enum `EExcludeSpawnGender` as stored: `Male`, `Female` or `None`, listed in `x-values`),
`minGroupSize`, `maxGroupSize`, `maxConcurrentSpawns`, `maxSpawns`, `minSpawnDelay` and
`maxSpawnDelay` (the numbers as stored, `-1` for none; the Archive states no unit of the delays),
`missionAlliedMarker`, `isCritical`, the tag pairs `characterTags`, `closetTags`, `roomTags`,
`defendAreaTags` and `scheduleAreaTags` (each `positiveTags` and `negativeTags`, from the fields
`positiveCharacterTags` and `negativeCharacterTags`, `closetPositiveTags` and so on), `entityTags`,
`inventoryItems` (`entityClassGuid`, `quantity`, `amountInventoriesToFill` and `markAsMissionItem`)
and `factionOverrideGuid` (a `Faction` Record no kind lists). Tags are full tag paths; what a tag
means is not stated by the Archive and the catalog does not interpret it.

The `embeddedName` (`MissionPropertyValue_AIName`, `null` when absent) has `randomName`,
`randomLastName`, `randomNickName`, `chanceOfNickName` and `nameDataGuid` (the
`CharacterRandomNameParams` Record). The fields `givenName`, `givenLastName` and `givenNickName`
are sourced references into `texts.gameTexts`. Each refers to its exact original field occurrence
in the selected pool, and its target provides the original key and all installed translation forms.
Empty and placeholder keys have an absent reference with the original field evidence. Pool
identifiers use the untranslated key-only content. Translations and provenance do not participate
in the identifier (ADR-0324, ADR-0356).

**Entity spawn descriptions (`contractEntitySpawns`).** An entry has `guid` and `groups`
(`SpawnDescription_EntityGroup`: `name` and `slots`; a slot is a `SpawnDescription_EntityOptions`,
the alternatives for one entity, each an `options` entry with `amount`, `weight` and the tag sets
`tags`, `negativeTags`, `markupTags` and `entityTags`).

### Cargo manifests

`definitions.cargoManifests` owns the physical `CargoManifest` Records under
`libs/foundry/records/cargomanifest/`. The table uses the same `cargoManifests` filter and
completeness kind. Rows are ordered by GUID and physical Record index. Each header and stored
value has its own source state and origin. These declarations describe cargo fill and spawn
configuration.

| Field | Original source |
| --- | --- |
| `lockCargoGridsOnSpawn` | The authored flag on the Record. |
| `cargoFillCapacity` | The original fill definition with its concrete `sourceType`, `minCapacityRange`, `maxCapacityRange` and ordered `resources`. Known types include `CargoFillCapacityValue_Even`, `CargoFillCapacityValue_Random` and `CargoFillCapacityValue_Custom`. |
| `cargoFillCapacity.value.resources` | Original resource slots. A direct resource has `resource` and `probability`. An allocation has `minResourceAllocationPercentage`, `maxResourceAllocationPercentage` and its nested `resources` value. |
| `cargoSpawnRules` | Ordered original rules, each with concrete `sourceType`, `probability`, authored `entities` and `resources` references and the original `quantityAlgorithmType`. |
| `quantityAlgorithmType` | The authored algorithm's concrete `sourceType`, integer `quantity` or numeric `proportion` where declared. |
| `authoredFields` | Ordered members of this owner outside the typed fields, including additional fields of new concrete variants. |

Numeric cells preserve stored widths and IEEE bits. The decimal projection is available for finite
floats. References retain their own stored GUID, target evidence and resolution state. A
`ResourceType` identity remains usable when no commodity carries it. Null and repeated
occurrences retain their positions.

Manifest tags and description text or localization keys retained in `authoredFields` use the
protected `gameText` domain with x-`gameText`. They preserve the exact source text/key and its
encryption origin. The authored member capture adds no translation or public display name.

For a consumer migrating from v2, read `definitions.cargoManifests.rows` for the former
`cargoManifests` rows. Use the sourced fill type in `cargoFillCapacity.value.sourceType` for the
former `fillType`, and the corresponding array inside the fill for former `resources` rows.
Read the nested allocation resource and its outer bounds separately. Replace `spawnRules` with
`cargoSpawnRules`, and follow the sourced reference cells for former GUID arrays. ADR-0265
records the independent encounter source and why contracts have no manifest join.

### Starmap objects

`definitions.starmapObjects` owns the authored `StarMapObject` Record. The original type,
parent, jurisdiction, affiliation, location hierarchy tag and ordered amenity links carry exact
GUIDs and source states. Original flags, overrides, text and owned settings use independent
source cells. The [starmap field map](catalog-starmap-definitions.md) lists the original fields.

`placements.starmapPlacements` contains one row per selected object. `objectGuid.value` and
`objectRecordIndex.value` identify the physical definition. The original definition's `parent`
and the derived placement's `placedUnderGuid` can differ.

| Placement field | Source |
| --- | --- |
| `system` | The star system folder under `Data/ObjectContainers/PU/system`. |
| `entityName` | The name of the Child element that places the object. |
| `placedUnderGuid` | The nearest starmap Record above the placement in the container hierarchy. |
| `position` | `[x, y, z]` in metres in the frame of the system container, rounded to a millimetre. |

Each placement cell cites its actual original root and XML Child route. Position cites the
leaf and ancestor translations and the ancestor rotations that affect it. The physical
owner identity has independent DataForge evidence. Sources with undeclared encryption can
yield a null encryption marker even when the contributor set is complete.

`placements.containerInstances` keeps a separate row for each resolved physical SOCpak
occurrence. The instance key contains `rootEntryIndex` and the ordered document-local
`childNodeIndices`. The required `externalChildStepIndices` array holds the ascending zero-based
positions in that Child sequence where the Child was read from an external descriptor reached at
the preceding step. An inline-only route has an empty array. Parent keys use the same form.
Reused tiles keep separate local and world transforms, the exact declaring path, label and entity
name. See [ADR-0414](adr/0414-container-occurrence-keys-record-external-child-steps.md).

`placements.unresolvedContainerLinks` carries each reached external Child with no usable unique
target body: zero/multiple winning physical Entry candidates, or one target whose body cannot be
read or parsed. `instanceId` is the system root and original Child route.
`parentInstanceId` joins to the enclosing `placements.containerInstances` row. The declaration
retains the exact XML descriptor input and Child node position, authored path, independent local
position and rotation, optional Starmap Record GUID and all original attribute name/value cells
in source order. The same physical declaration can appear under several system routes.
The world cells use the valid composed transform when available.

`targetStatus.value` is `resolved` for one physical directory target even when its body is unavailable,
`notFound` for an exhausted supported lookup in an intact Archive directory,
`ambiguous` when the winning path names multiple physical entries, `unsupportedSelector` when
the authored grammar cannot be searched, or `incompleteDirectory` when directory membership is
uncertain. This sourced cell cites the authored selector and consulted directory survey, retaining
their encryption marker and completeness. `targetCandidates` identifies the actual winning
Entries with local input references.
An empty candidate set with incomplete origin is insufficient to prove absence. `targetReadStatus.value` separately records `notAttempted` for a nonunique lookup, `unreadable` for
a target without a readable descriptor body, or `unparsed` for an intrinsically invalid descriptor.
It retains the actual directory/target selectors and claims no retained payload from failed reads.
Configured missing, ambiguous or unreadable dependencies continue to affect `exportHealth` and
normal publication.

`placements.locationInstances` contains the typed location Children reached by the same walk.
It uses the system root and physical Child route for each occurrence. `parentLocationId`
refers to the nearest authored location ancestor. `containerInstanceId` refers to the
enclosing physical SOCpak occurrence. The original Child's GUID is a declaration and can
repeat across uses of one container. The local transform and composed system transform use
the same conventions as container instances.

The `declaration` preserves the stored numeric `locationType`, `streamingRoot`, original
`class` string, independently sourced GUIDs, label, entity name, tag text and ordered GUID tokens. Invalid typed attributes retain the
original text with its source. `instanceMetadata` retains ordered component element
occurrences, original node/parent positions and attributes. A component element records its
authored occurrence. A Harvestable preset link requires a separately authored binding.

The optional `declaration.hierarchicalContext` is emitted for a typed Child with an
asserted or malformed `hasHierarchicalContextData` flag. A valid asserted flag uses
the Child's nonzero `guid` to look up an exact GUID filename in an `hcd` directory
within the same physical SOCpak. False or absent flags leave the lookup null.
`candidates` cites the original directory and preserves all matching physical
members in directory order. `failure` distinguishes `dangling` (zero matches) and
`ambiguous` (multiple matches). An invalid flag or lookup GUID yields an invalid
`document` without candidates. A unique member is read to completion and checked
against the Archive's checksums. The `document` distinguishes unreadable bytes,
invalid XML or root, and a readable context header.

The context header retains `__version`, the original `preset` text, its typed
`presetGuid`, `presetRecordName` and extra root attributes in source order. A
consumer can join `presetGuid` to `definitions.hierarchicalContextualDataPresets`.
That table has its own selection scope. Context field provenance comes from the
companion Entry. Lookup provenance retains the owning Child and directory sources.

The optional `declaration.entXml` inventories matches for the original typed Child
within its physical SOCpak. It checks `.entxml` members under an `entdata` path
segment and joins the Child's nonzero `guid` to the decoded root `EntityCryGUID`.
Each candidate contains its physical `entry` and a `document` sourced from that Entry.
The original file name, class GUID and display text are independent declarations.

`inventoryComplete` describes the running lookup's source-read coverage. `selected`
contains the single physical Entry after a complete lookup. No match is absent,
multiple matches or an invalid Child identity are invalid, and a failed or unsupported
candidate can leave selection unreadable. The readable candidates remain available
in directory order. Candidate-set and selection carriers retain their observed
source references with incomplete source-set proof when the survey cannot be
reconstructed from the retained references alone.

The document retains the root and class GUIDs, original root-owned
`harvestableComponents`, and independently ordered `areaEntities` with their
containing sphere node indices. The typed `subConfigGuid` and original
`subConfigText` belong to the individual component occurrence. The GUID joins the
canonical row in `definitions.subHarvestableMultiConfigs` from the same Build when
selected. The area entries are authored declarations with their own positions in
the XML. No per-component mapping to one area entity is defined.

Placement selection follows the existing container inventory scope. An unrestricted catalog
or any selected Starmap or HarvestableProviderPreset path requests the walk. Other Record
filters leave this table unavailable. Walk and archive diagnostics report incomplete source
discovery separately from whether the table is available. Discovery reads each reachable
physical container once. Placement follows every authored occurrence in document order and
reports a cycle when an external link points back to a container already on that route.


**Where a position comes from.** The main container of a star system is
`Data/ObjectContainers/PU/system/<folder>/<folder>system.socpak` (else the first `*system.socpak`
of the folder). It is a container (ZIP) whose XML, named like the container, lists
`ChildObjectContainers/Child` elements. A Child places an object with `starMapRecord` (the GUID of
a `StarMapObject`), `pos` (`x,y,z`) and `rot` (a quaternion `w,x,y,z`), may nest further
`ChildObjectContainers`, and with `external="1"` or `external="true"` names another container (`name`) whose Children
are placed in the frame of this Child. Positions compose from the top: the position of a child is
rotated by the rotation of its container before it is added, and the rotations multiply, so
`world = parent.position + parent.rotation * child.pos` and `rotation = parent.rotation *
child.rot`. A Child without `rot` has the identity rotation, one without `pos` the origin. Children
without a `starMapRecord` only take part in the composition. ScDataDumper adds the child positions
without rotating them by the container rotation (`Commands/LoadStarmap.php`); the catalog does
not.

A record that more than one Child places keeps its position when every placement lies within 1 m
of the first (the same place reached twice); when two lie further apart the record has no position,
because no placement is the place (`StarmapSummary::ambiguous` counts these records; on LIVE
all are asteroids and wreck sets of the Nyx ring segments, whose containers are instanced several
times). A Child whose `pos` or `rot` is malformed (not numbers, not three or four parts, a
quaternion without length), or whose composed position is not a finite number or lies beyond
1e15 m on an axis (about a tenth of a light year, far outside any star system), and everything
below it are excluded from placement. The default traversal visits all source declarations and
placement occurrences. Callers can set independent allowances for uniquely parsed Child declarations
and placement visits per system through `StarmapTraversalBudget`. Exhaustion marks the affected
scope incomplete and refuses normal publication. Child traversal in admitted binary CryXml and
text XML is iterative. Text XML must be well-formed and contain no DTD. A container already open
on the active route counts as a cycle.
Shared external containers keep each independent use. [ADR-0163](adr/0163-the-starmap-containers-are-parsed-once-and-composed-in-memory.md)

`missing_containers` and `unreadable_containers` count references, so a single external target
named by ten Children contributes ten occurrences. `malformed_children` counts affected Child
occurrences. A required target with a complete supported
lookup and no candidate has `targetStatus.value` of `notFound`. Normal publication retains the
original Child and lookup evidence in `placements.unresolvedContainerLinks`. Starmap, Loot
Containers and Harvestable Locations each report a warning when they depend on the verified
lookup. Unsupported, ambiguous and unreadable required targets block normal publication. The
rendered `CatalogSummary::starmap_report()` keeps up to 300 container lines and 300 issue lines per star
system, each up to 300 characters. [Catalog export health](catalog-export-health.md)
**Rotation order.** `rot` is read as `w,x,y,z`. The raw values of LIVE settle it: the identity occurs as
`1,0,0,0`, the rotations of the Nyx ring segments are `a,0,0,b` (about the z axis, the axis of the
ring) and the small tilts of surface structures `0.98,~0,~0,0.17`; read as `x,y,z,w` the identity would
be a half turn about x and the ring segments rotations about x. Everus Harbor lies 1.04e6 m from
Hurston.

**What no container places.** On LIVE (2026-10-22) 2,093 objects are selected, 1,239 have a
position and 854 have none. Searching all 9,668 object containers of the Archive for the 854 GUIDs gives:

- **816 appear in no object container at all.** No container places them, so no position exists to
  export. By type: Outpost 416 (350 hidden), Asteroid 279 (249 hidden), Asteroid_ValidQT 57 (34
  hidden), Manmade 49, Manmade_VisibleOnInteraction 12, Anomaly 12 (8 hidden; the
  jump point records of Pyro and Nyx), PointOfInterest 23, the 3
  SolarSystem records, and one each of Moon, Planet and Outpost_InvalidQT; 666 of the 854 are
  `hideInStarmap`. They are the Nyx and Pyro points of interest (`Nyx_Kaboos_*`, the Pyro Lagrange
  `*_Enterance` points), mining and quantum-travel markers, rest-stop records, quantum beacons of
  players and missions (`PARTY_MEMBER_QT_Beacon`, `MISSION_QT_*`) and the star systems themselves:
  the game positions them at run time or by other data than an object container.
- **9 appear more than 1 m apart** in the system containers (the instanced Nyx ring segments, see
  above) and are left without a position on purpose.
- **29 appear only in containers that no system container reaches**: `nyx/delamar/delamar.socpak`
  (the private mining points and asteroid clusters of Delamar), the Pyro rest-stop modules under
  `loc/mod/pyro/station/ser/reststop_ext` (the clinics), the Orison Fleet Week 2022 platforms, the
  New Babbage IAE 2023 lobby, and the Early Access maps (`EA/...`, which place `Ellis3` and
  `Stanton4_L1_02`). Their own position in the star system is unknown, because no `external`
  Child of a system container names them, so the position of what they place cannot be composed.

These objects have an absent `position` cell. The existing container traversal reads a complete
system scope for its position projection, independently of the object Filter. Selection uses
`libs/foundry/records/starmap/pu/` and the exact `StarMapObject` type with the usual development
leftover policy. The original `type` Reference targets a [canonical type](catalog-starmap-definitions.md).

Size, measured on LIVE (2026-10-22): the 2,093 objects are about 1.2 MB of JSON (570 bytes each), 4.5 % of the 26.5 MB of the
catalog, and a smaller share compressed.

**No released marker.** The Archive states no structural marker that tells a released starmap
object from one the game has not released (ADR-0067, #417): the three star system Records carry the same flags,
`hideInStarmap` is set on helper content of the released systems too, and the type suffixes
(`_ValidQT`, `_InvalidQT`) describe quantum travel. The catalog therefore has no such field; the
Consumers can use `system`, `position`, `hideInStarmap` and `type` in a display policy.

### Mineable elements and refining processes

The raw facts only (ADR-0065): the catalog states no instability per commodity, because the
relation is not one to one.

A mineable element (`mineableElements`) is a `MineableElement` Record under `mining/mineableelements`.
The Record header and each tuning property are sourced cells. The `resourceType` cell retains the
authored Reference, including its GUID and resolution status when the target is missing. A resolved
resource GUID can overlap a commodity's `subtypeGuid` when that commodity was included in the
catalog. Several elements may share one ResourceType, so every element retains its own tuning.

For numeric presentation use each property's `value.finite`. Its `value.rawBits` preserves the
original IEEE 754 Single, including nonfinite values. Read the sourced cell's `state` to distinguish
absence, null and unreadable source facts. The table gives the observed finite ranges on LIVE.

| Field | Record field | LIVE range |
|-------|--------------|------------|
| `instability` | `elementInstability` | 0 to 1000 |
| `resistance` | `elementResistance` | -1 to 1 |
| `optimalWindowMidpoint` | `elementOptimalWindowMidpoint` | 0.3 to 1 |
| `optimalWindowMidpointRandomness` | `elementOptimalWindowMidpointRandomness` | 0 to 0.3 |
| `optimalWindowThinness` | `elementOptimalWindowThinness` | -1 to 10 |
| `explosionMultiplier` | `elementExplosionMultiplier` | -1000 to 260 |
| `clusterFactor` | `elementClusterFactor` | 0 to 1 |

A refining process in `definitions.refiningProcesses.rows` is a physical `RefiningProcess`
Record under `refiningprocess` (9 on LIVE, one per pair of speed and quality). Its sourced
Record header retains physical identity, and `fields` retains the complete owner-local original
arena. `refiningSpeed` and `refiningQuality` are independently sourced open String/Enum labels;
`Slow`, `Normal`, `Fast` and `Wasteful`, `Normal`, `Careful` are observed labels, not closed
exporter-control vocabularies. Their original spelling and empty values remain exact.
`processName.value` resolves against `texts.gameTexts[].id.value`; the text row retains the
original Locale key in `key.value` and sourced `sourceText`/`displayText` forms in
`translations.value`. On LIVE (2026-10-22) the English `global.ini` holds all nine keys and the
German one none, leaving German display values absent for those keys. That LIVE source has no
yield per commodity and method; these yields remain external data.
See the [mining/refining contract](catalog-mining-refining.md) for source states and typed SDK access.

The 46 element Records of LIVE give 43 entries: the development-leftover rule leaves out
`MinableElement_Ship_template`, `MinableElement_GroundVehicle_template` and
`MinableElement_FPS_template`, counted as skipped. The 9 `RefiningProcess` Records are all entries.

### Mining deposits and harvestable locations

Facts only (ADR-0236): numbers, GUIDs, names of Records and paths. Texts other than `depositName`
are not exported and no media. The loot of a slot is its `lootConfig` (see "Loot").

**Compositions (`mineableCompositions`).** A `MineableComposition` Record under
`mining/rockcompositionpresets` (249 on LIVE 4.10, 231 entries; the leftover rule skips the rest).
`depositName` is a sourced reference to the deposit's HUD label in `texts.gameTexts`. For both
mining labels, the carrier's `value` matches the text row's `id.value`. The row holds the original
`key.value` and per-language `sourceText` and `displayText` in `translations.value`. These
references replace the inline `key/en/de` values of the v2 catalog.
The original Record header and `minimumDistinctElements` carry direct source evidence. `parts`
is the sourced `compositionArray`. Each array index is the original zero-based position, including
slots with a non-present source state. Each part has its own origin and sourced `mineableElement`,
`minPercentage`, `maxPercentage`, `probability`, `curveExponent` and `qualityScale`.
`mineableElement.value.guid.value` preserves the authored Reference GUID even when its target is
dangling. Target metadata carries its own source evidence alongside the resolution status. Values
use their source width. Floating-point values retain `rawBits` and a nullable `finite` number,
and 64-bit integers use decimal strings. The local mining reader shares this sourced shape.

**Parameters (`configurations.miningGlobal.rows`).** The `MiningGlobalParams` Records under
`mining` (7 on LIVE, 6 entries) retain the sourced physical header and complete original `fields`
arena. All 26 root numbers keep their original names, including `cSCUPerVolume`. Every projected
number is a `SourcedValue<ResourceNumber>`: integers retain their width, Int64/UInt64 use exact
decimal strings, and Single/Double retain raw IEEE bits plus a nullable width-correct finite value.
Absent declarations, explicit nulls and unreadable values remain distinct; no plain-number or
null shortcut is emitted. The historical LIVE 4.10 example represented
a missing `compositionMassMultiplier` with a plain null; current rows retain the actual sourced
state rather than inferring it from that legacy representation.

The independently sourced `hitConsistencyParams`, `mineableInstabilityParams` and
`mineableExplosionParams` blocks retain all 11 typed child numbers under their original names,
including `instabilityCurveFactor`, `instabilityWavePeriod` and `instabilityWaveVariance`, and
`dangerPoolFactor`/`defaultVolume`. Effects, sound and default-explosion declarations remain in
the original arena without claiming decoded media or engine semantics. `wasteResourceType` is
the sourced original Record link: `wasteResourceType.value.guid.value` retains the authored GUID,
with physical target metadata and resolution status alongside it. No ResourceType body is
required for this link. See [the complete contract](catalog-mining-refining.md) and
[ADR0408](adr/0408-canonical-mining-and-refining-tables-preserve-original-owner-data.md).

**Entities (`definitions.mineableEntities`).** The table contains each selected
`EntityClassDefinition` Record under `entities/mineable`, using the usual leftover rule.
Rows carry sourced GUIDs and physical Record indices. Equal GUIDs retain independent rows ordered
by Record index. An entity without matching components still has a definition.

The `mining` and `radar` arrays keep each matching component at its original `sourcePosition`.
Each mining component retains the original `composition` and `globalParams` Record links.
Radar components retain the original radar properties, including `contactType` and
`baseSignatureParams`. The owner-local `authoredFields` values preserve additional authored fields
without assigning runtime scanner behavior to them.

In each base-signature declaration, `signatures` retains the original ordered slots, each with a
source state, raw IEEE 754 bits and a finite projection when available. The sourced
`scanSignature` is derived from the slot whose `displayName` in
`RadarSystemGlobalParams.signatureSystemParams.signatureTypeParams` is
`@hud_scanning_info_rs_signature`. The value cites the selected number and the radar slot
selector. No finite scan value is emitted when the slot is unavailable or ambiguous, signature
counts differ, or all values are zero. Unfiltered builds retain the established warning for an
unknown slot when entities contain signatures.

**Provider presets (`harvestableProviderPresets`).** The provider Record identity and all authored
fields use sourced cells. The groups, elements, geometries, areas, object preset paths and
modifiers are arrays of sourced slots in authored order. This retains the positions of null,
unreadable and unsupported elements. Probabilities and modifiers retain their declared numeric
width and raw representation. Original Record references are available as `harvestable`,
`harvestableEntityClass`, `harvestableSetup`, `clustering` and geometry `tag`.

Each element's `spawn` reports the selected preset class and how its entity was selected. A
resolved explicit entity reference takes precedence. When the preset's `entityClass` is
consulted, `presetEntityClass` also retains that selector. The selected entity has a physical
Record index and GUID and contains the first authored `MineableParams` occurrence. Its
`miningParams` cell carries the component position and its original `composition` and
`globalParams` references. The source uses of the nested values include the selector route
that chose this entity.

An area's `areaType` retains the concrete declared type and ordered object preset path cells.
Each modifier retains its original Record or instance pointer and, for an exact pointer to a
group element in the same physical provider Record, the zero-based group and element positions.
The original pointer's Record identity and instance path remain available when that slot projection
is absent.

**Multi configs (`subHarvestableMultiConfigs`).** The `SubHarvestableMultiConfigRecord` Records
under `harvestable/slotpresets` (103 on LIVE). `defaultConfigGuid` is the Reference of a default
config that is a reference, `taggedConfigs` keep `name`, the tag GUIDs, `configGuid` for a config
that is a reference, and for a config written out `respawnTimeMultiplier`,
`initialSlotsProbability` and the `subHarvestables` (`harvestable` with preset, entity and mining Record GUIDs, `setupGuid`, multiplier,
`relativeProbability`, `relativeProbabilityDeepest`, geometry tags and `lootConfig`).

**Locations (`harvestableLocations`).** ScDataDumper's `SocpakMappingGenerator` finds the preset
of a container in the first `.soc` file of the container: the GUID in the string that follows the
string `preset` (strings are separated by a zero byte). The catalog does the same for every
container below `Data/ObjectContainers/PU/system` and keeps the rows whose GUID is a kept
provider preset. The `.soc` is read in bounded chunks up to the first preset match or end of
the entry, with cancellation checked between chunks. The finding can be anywhere in the
stream. A read failure is reported as an unreadable container. A row has `presetGuid`,
`system` (the folder of the star system), `container` (the Archive
path) and `starmapObjectGuid`: the walk through the system containers (see "Starmap objects")
reaches a container through a Child element, and the nearest object at or above that Child names the
location. A mining claim marker gives way to the object above it (ScDataDumper skips the private
mining points by name; the catalog uses a field instead): the marker is a `StarMapObject` whose
`description` is the key `@MiningClaims_Desc_001`, which all 188 `PrivateMiningPoint_*` Records of
LIVE 4.10 have and no other of the 2,123 starmap objects. A harvestable's `HarvestablePreset` or
entity Record that is missing or cannot be built is warned about (`unreadable_harvestables`) and
its link fields are `null`. A container no Child reaches, or whose object is not in
`starmapObjects`, has `null`. One container under several objects gives one row each.

On LIVE 4.10 (2026-10-28) 489 rows name 19 of the 49 presets, 488 of them with an object. The 25
planet presets, `HPP_Lagrange_G`, `HPP_Lagrange_Occupied`, `HPP_ResourceRush_Gold`,
`HPP_ResourceRush_Gold_HighDensity` and `HPP_SpaceDerelict_General` are named by no container:
the Archive links them to a place by no `.soc`. `HPP_Pyro_AkiroCluster` is named by `pyro/akirocluster.socpak`,
which no Child reaches (ScDataDumper pins it to `Pyro_AkiroCluster` by hand; the catalog states no
object). `HPP_Stanton2c_Belt` is named by `stanton2c.socpak` and, a copy in the data, by
`pyro6.socpak`.

No Record or container links the surface presets to a body: the Archive holds no planet container,
and only the Hurston presets name a body in the folder of their `objectPresetPaths`, so they stay
without a location (ADR-0236). When an object is recorded above a container but no object results
(it is not in `starmapObjects`, mining claim markers form a cycle above, or a marker has no
object above it) the warning `harvest_locations_without_object` counts the rows with a line per
reason. A container with no object recorded above it (no Child reaches it, or it is reached with
no object above, as a system root) is the normal case and deliberately silent.

### Loot

Facts only (ADR-0238): GUIDs, Record names, weights, counts and tag links. The `name` of an entry
is the designer's label, not a text the game shows; no field of these kinds is a game text.

**Tables (`lootTables`).** A `LootTableV3Record` under `lootgeneration/loottables` (59 on LIVE
4.10, 54 entries; the leftover rule skips five). The folder also holds the older `LootTable`
Records (161), which are the kind `legacyLootTables` (see "Older loot Records"). `entries` follow
`lootTable.lootArchetypes` in stored order, each with `name`, `weight` and the archetype it draws
from: `archetypeGuid`, an entry of `lootArchetypes`, for a `LootArchetypeV3_RecordRef`, else
`archetype`, the entries of an archetype written out in place (as in "Archetypes"); the other one
is `null`. The `optionalData` of an entry gives `priorityCount`, `choiceLimit`, `dupeLimit` and
`chanceToExist` (0 to 1), each `null` when the entry has no such data. An entry is drawn with the probability `weight` over the sum of the weights
of the table; the Archive does not state how `priorityCount` and `choiceLimit` change that, and the
catalog does not decide it.

**Archetypes (`lootArchetypes`).** A `LootArchetypeV3Record` under `lootgeneration/lootarchetypes`
(34 on LIVE, all entries; the older `LootArchetype` Records, 233, are `legacyLootArchetypes`). An
entry has `name`, `weight` and a `selector`, and the optional data `stackSize` (`min` and `max`)
and `spawnWith`: items spawned with it, each with `name`, `chance`, `mode`, `amount` and a
selector. A selector selects either by tags or by entity classes, read by its fields and not by
the name of its struct: `positiveTagGuids` and `negativeTagGuids` (`Tag` Records of the
`TagsDNFTerm`; an item fits when it carries every positive and no negative tag) or
`entityClassGuids` (the classes of a manual list). `selectorType` states the struct as stored and
is informational. The tags are the `Tag` Records of the tag tree, not the free item tags of
"Matching tags".

**Secondary choices and themes.** `definitions.lootSecondaryChoiceSingleLayers`
keeps the original ordered Class entries. `definitions.lootSecondaryChoiceMultiLayers`
keeps original ordered Reference slots to those layers. `definitions.lootThemes`
keeps the authored theme type and its owned fields. Each table has independent
availability, source-state and selection accounting. The [Loot source contract](catalog-loot-source.md)
describes the provenance and field representation.

**Pool filters (`definitions.lootPoolFilters`).** The canonical `CatalogTable` contains each
selected physical `PoolFilterRecord` under `lootgeneration/filters`, with its independently
sourced eight-field header. `fields` lists every original root declaration in stored order;
`values` contains one flat entry for every actual owned value occurrence. A field holds its
sourced `name` and exact `declaration.dataType`, `isArray` and `target`, plus a structural
`valueIndex` reference encoded as a canonical unsigned decimal string. Struct payloads retain
the actual sourced `sourceType` and ordered fields; Array payloads retain every ordered element
reference, including null and unreadable slots. Equal subtrees and duplicate names are separate
occurrences. References do not expand other Record bodies.

`values[].value` is the immediate sourced cell for that original datum. Its typed scalar encoding
retains widths, IEEE bits, exact strings/keys and GUIDs. Known designer labels/modes and enum
labels remain public; unclassified String/Locale domains conservatively use `gameText` with
`x-gameText`, retaining the original text without inventing a translation. This text-domain choice
is independent of each datum's encryption marker. `originalError` is required and null for ordinary
or null values. An unreadable value keeps its original typed error and every retained parameter
outside its null semantic payload, independently of sampled Health examples. Every independent
source-backed declaration, concrete type, target fact and error parameter has its own marker.

The exporter adds no depth limit or recursive JSON value chain. Original canonical-reader resource
failures remain explicit Health failures. The owner-local graph and original-preorder source ordinal
are distinct concepts; serialized indices never establish physical source authority. Stored Reference
GUIDs and pointer-derived Weak GUIDs retain their different original contributors. See
[catalog-loot-source.md](catalog-loot-source.md) for the value graph and original-source contract.

**Older loot Records (`legacyLootTables`, `legacyLootArchetypes`).** The two arrays retain their
existing dataset paths. Rows carry individually sourced Record header fields including
`recordIndex`. Equal GUIDs at distinct physical indices remain distinct, ordered by GUID then
Record index. Each row embeds its original owner-local `fields` and `values` arena. It retains
the exact authored occurrences even when a weighted view cannot interpret them. Existing
term keys use sourced cells. A present numeric term carries its storage width and exact IEEE
bits when applicable. Original arrays retain their authored order and every null or unreadable
slot. The SpawnWith and stack-size projections register their actual selection inputs.
A dangling Reference retains its stored GUID in the typed cell. The attempted lookup is marked
incomplete, and the matching owner-local value preserves the original error and target GUID.

The `LootTable` (161 on LIVE)
and `LootArchetype` (233) Records of the same folders, which 635 of the 1,958 slots with a
`lootConfig` still name in `lootTableGuid`. A table has `entries`, each with `weight`,
`archetypeGuid` (an entry of `legacyLootArchetypes`), and `minResults` and `maxResults` of
`numberOfResultsConstraints`. An archetype has `excludedTagGuids` (items with such a tag are never
drawn), `entries` (the primary group: `name`, `weight`, `tagGuid`, `additionalPositiveTagGuids`,
`additionalNegativeTagGuids`, `stackSize` and `spawnWith` with `name`, `mode`, `min`, `max` and the
tag lists of `tagsToMatch`) and `secondaryGroups` (`groupName` and entries of `weight` and
`tagGuid`). The Archive does not state how the groups combine, and the catalog does not decide it.

**Slots (`lootConfig`).** A sub harvestable of a multi config has `lootConfig` when its Record
states one (1,958 of 2,896 on LIVE): `lootTableV3Guid` (an entry of `lootTables`; 1,321 slots),
`lootTableGuid` (an entry of `legacyLootTables`; 635 slots; a slot names one of the two),
`poolFilterGuid` (an entry of `definitions.lootPoolFilters.rows`), `totalResultsLimit`, `chanceToGenerate`,
`chanceToGenerateAdditionalInventories` and the `fullness` range of `fullnessFactorRange`.

**Placed containers (`lootContainers`).** A container below `ObjectContainers/PU/system`, or one
it places, holds the property file of an entity (`entdata/<n>.entxml`) that names the container it
places (`objectContainer`) and a `multiConfigRef`, a `SubHarvestableMultiConfigRecord`. The placed
container holds tag points, entities that carry tags, in its `.soc` files. The tags of those points
are the `tagGuids` of the tagged configs of the multi config (the 13 points of a Stanton derelict
carry the Rare, Common, Uncommon and Reward tags of `LootGeneration_MultiSlotPreset_Derelict` 4, 4,
2 and 3 times); the slots of a tagged config carry the `lootConfig`. The catalog states these
facts. A row also carries the system, the optional Starmap parent, and the tag counts summed over
the placed container's `.soc` files.

`entityCount` carries the observed state, original source references and encryption marker.
`present` includes zero when the complete nested directory and its verified `.soc` files
establish an empty result. `absent` means the original placement lookup proved the container is
missing. `unreadable` means the target or a required input could not be read or validated. A
completed count cites the placement selector, the nested path directory, and the layouts and
entity XML ranges of the selected SOC files. Failed inputs appear in export health.

`tagCounts` uses the same survey and source states. A verified survey without any valid tag GUID
has `state: present` and an empty array. Rows are ordered by GUID. Each `tagGuid` cites the first
accepted `TagId` in physical SOC member order, then chunk and XML order. The `socEntityTag.cryXml`
and `socEntityTag.text` coordinates are zero-based chunk ordinal, decoded chunk start and exclusive
end, XML node ordinal and attribute ordinal. The field records the original attribute spelling.
The numeric `entities` counts each physical entity once per tag and cites the full verified survey,
including members without a match. Its encryption marker can differ from the tag GUID's marker.

`multiConfigGuid.value` retains the first valid original configuration attribute.
`system.value` is the normalized folder of the physical main system container or the
physical fallback container. Its origin describes the selected directory path and folder
conversion independently from the original XML and SOC payloads.
`container.value` retains the path of the selected Archive Entry or the original authored text.
Each field carries its own contributing source references and encryption marker. A repaired
path also cites the actual authored selector conversion. The declaration table retains each
original occurrence and its lookup status.

`starmapObjectGuid.value` identifies the final admitted non-marker Record. Its source
references retain the selected Child route, the actual Record admission and each mining-claim
marker decision. A known root without a parent is absent. An unreached fallback or a GUID
outside the admitted Records remains absent with an incomplete source set. Cycles and
known ambiguous marker placements are invalid. A preserved GUID with an unreadable marker
predicate carries an incomplete source set.

The walk starts at every container below `ObjectContainers/PU/system` and follows the containers
the entity files place, each container read once. Entity XML and SOC reads consume the complete
declared payload with cancellation checks. An Entry that ends early or exceeds its declared
size causes a read failure. An authored link to a missing container retains its selector as a
failed enumeration, including when the linking entity has no multi config.
The location is that of the container the walk
started in: `starmapObjectGuid` is the object the Child that reaches it hangs below (as for
"Locations" above, with `null` for a container no Child reaches or an object the catalog lacks). A
container placed under several roots gives a row for each, and containers that place each other
(A places B places A) are read once. The warning `unreadable_loot_containers` names a container or
entity file that cannot be read, a `.soc` that cannot be counted, or a placed container the Archive
lacks, with the reason for each. An entity file that names
several different containers or multi configs uses the first of each in document order and is
warned about (`ambiguous_loot_entity_files`). The cost of the walk is the size of the subgraph below
each distinct root; it is not memoised, as it is negligible on LIVE.

On LIVE 4.10 (2026-10-28) 3,004 rows name 41 multi configs and 189 containers in 3 systems, 2,797
of them with an object.

### Crafters

`definitions.crafters` contains the original Entity Records with a
`CrafterComponentParams` component. The canonical header identifies each physical Record
within the Build. Equal GUIDs remain separate rows. A station can also appear as a ship, item,
commodity or mineable Entity. The source path filter applies to the owning Record.

| Field | Original source |
| --- | --- |
| `component` | The first readable exact crafting component and its original Components position. |
| `component.value.maxJobsInHistory`, `baseCraftingTime` | Authored limits and time with exact integer widths or float bits. |
| `component.value.queues` | Original ordered queue slots, names, limits, category availability and process-type flags. Each flag keeps its own source and position. |
| `component.value.modifiers` | Original ordered modifier slots, including the time multiplier and independent category condition. |
| `attachment.value.attachDef` | The selected attachable component's original type, subtype and size for matching a port. |

Each sourced cell distinguishes absent, null, empty, invalid and unreadable state where applicable.
Known whitelist and category condition structures keep their original ordered Reference slots.
Unclassified polymorphic structures keep their concrete source type and original members.
Process-type flags use their authored positions. The Archive supplies no names for those positions.

`projections.crafterHosts` groups actual default Loadout mounts by physical ship Record and
physical crafter Record. `shipRecordIndex` joins the original ship and Loadout within the Build.
`crafter.targetRecordIndex` joins the station definition. Each independently sourced
`mountPositions` cell identifies the actual Loadout root and nested port ordinal route. The same
station mounted in several ports has one pair row and several positions. A ship that owns a
crafting component has its own definition. Its component alone yields no mount relationship.

### Shops

A shop (`shops`) is one `ShopBackup` entry of the shop registry `Data\Libs\CosmoTools\
LayerRecovery\LayerBackups.xml` (a CryXml file), one placement of an `SCShop` Entity in the world
(Catalog schema 3, ADR-0069). The kind is facts only: identifiers, internal
names and places. It carries no price, no stock and no description; the item GUIDs of a shop come only from an
inventory file that its containers name (below). The names in it
are names of containers and entities, not localisation keys, so no field is a game text.

Read the registry-derived fields `key`, `entityGuid`, `system`, `entityName`,
`registryName`, `defaultValue`, `containers`, `body`, `location` and
`commodityExchange` as sourced values. Each container in `containers.value` is sourced
in original order. Resolve the source-use references to the exact XML node and attribute
ordinal of the selected registry declaration or its owning layer. An absent attribute
retains the observed absence at that node. An authored blank normalized to an absent
value retains the original attribute occurrence. The selected declaration is the first
valid occurrence of the normalized key.

| Field | Source |
|-------|--------|
| `key` | The `ID` attribute of the entry: the CryGUIDs of the containers from the system root down to the `SCShop` Entity, comma separated, each element in lower case. Unique (1,184 distinct on LIVE) and the identity of the shop |
| `entityGuid` | The last element of `key`, the CryGUID of the `SCShop` Entity, lower case. A shop module placed in several places keeps it (235 distinct for 1,184 shops on LIVE), so it is no identity; it is the CryGUID that `metadata\entitycomponentshop.xml` of the shop container names |
| `system` | `PlatformName` of the `LayerBackup` without the suffix `system`, lower case: `stanton`, `pyro` |
| `entityName` | The last element of `SuperName`, the name of the `SCShop` Entity, e.g. `SCShop_CommEx_TDD_NewBabbage` |
| `registryName` | `ShopName`, e.g. `Pharmacy_RestStop_ARC-LEO1`; absent when empty. A name, not a localisation key; many shops share one (360 distinct non-empty on LIVE) |
| `defaultValue` | `DefaultValue`, a GUID; absent when empty. Shops with the same `registryName` share it (1 to 1 on LIVE). What it refers to is not known |
| `containers` | The elements of `SuperName` before the entity, each without its `_{GUID}` or `_{GUID}_socpak` suffix and surrounding blanks, from the system root down |
| `body`, `location` | The second and third element of `containers` (`OOC_Stanton_4_Microtech`, `Area18_City_objectContainer`; Pyro: `pyro5`, `rs_ext_pyro6_l3`); absent when the chain is shorter |
| `starmapObjectGuid` | Sourced GUID selected by the Shop registry location. The location matches the original class name or selected `Child.entityName` ignoring ASCII case. Physical candidates with the same nominal GUID keep a single winner. Distinct matching GUIDs are `invalid` and retain their matching sources. A missing registry location is `absent`. A supplied location with no observed match is `unreadable` with incomplete negative coverage. |
| `kind`, `brand` | Sourced strings from the outer directory path of each container whose nested `CryGUID` matches `entityGuid`. Under `Data\ObjectContainers\PU\Shops`, the first folder is the kind and the second, where present, is the brand. `shopcomm` and `shopfood` folders under `PU\loc` have a kind and no brand. Both cells are `invalid` when the matching containers disagree on the kind or brand. A completed survey can report `absent`. An incomplete survey reports `unreadable` for an undetermined value and retains observed consistent values with an incomplete source set. Matching path and GUID selectors retain their individual origins. When no classified container matches, the field carries an incomplete source set if proving absence required consulting nested declarations. |
| `commodityExchange` | `true` when `entityName` starts with `SCShop_CommEx_TDD_`, or starts with `SCShop_Platinumbay` and ends with `_Commodity` (ignoring case): the commodity exchanges and terminals |

**What the registry holds.** LIVE has two `LayerBackup` elements in the one file: `stantonsystem`
(813 shops) and `pyrosystem` (371), 1,184 entries in all. An entry whose `ID` and `SuperName` chains
differ in length, hold an element of `ID` that is no GUID or an empty container name is left out,
and so is an entry whose `ID` an earlier one already has after lower-casing
(`ShopSummary::malformed` and `duplicate`; on LIVE the 1,184 `ID`s are distinct and the two chains
agree in length everywhere). The chains have 4 to 9 elements.

**Warnings.** An empty `shops` must not read as "the shops were removed", so `p4kx catalog` warns
(exit code 1, `warnings` in the JSON summary) when the registry exists but cannot be read
(`unreadable_shop_registry`), when the Archive holds starmap objects but no registry at the known
path (`missing_shop_registry`, the registry may have moved in the Build) and when shop containers
cannot be read (`unreadable_shop_containers` with a `count`; their shops may lack kind and brand)
or inventory files cannot be read (`unreadable_shop_inventories` with a `count` of files and
`details`, the path and the reason of each; a file with no content at all, such as
`inv_shopping_test_featuretest.json` on LIVE 4.10.197, is one of them, ADR-0141), when
inventory files that containers name are not in the Archive (`missing_shop_inventories` with a
`count` of distinct files; a Build may have moved or renamed them) and when shop containers name
shops but no `.soc` holds a shop component (`no_shop_components`; the layout may have changed).
`ShopSummary::registry_missing` and `registry_unreadable` carry the same facts. With a Filter no
shop is read and none of these warnings is raised.

**On LIVE (Build 4.10.197)** 1,000 of the 1,184 shops link to a starmap object (Pyro: 307 of 371)
and 184 do not; 54 of those have a location name that more than one object carries. The unlinked
locations are mostly Pyro asteroid-cluster and stash containers (`pbay_cluster_001_sand`,
`ab_pyro_asmbl_01_enctr_001a`) and some Stanton ones (`ObjectContainer_RestStop`,
`Jumptown_EchoIsle`) that no starmap record names. 590 shops have a kind and a brand, 60 have a
`null` kind because containers of different kinds name their Entity, and 534 because no container
below `PU\Shops` or the `shopcomm`/`shopfood` folders names it. 16 are commodity exchanges. The 360
distinct `defaultValue`s are no DataForge Record GUIDs.

**The link to the starmap** goes by the name of the location container, as the spike (#398)
found. The summary lists the distinct location names without a link (up to 200) in
`CatalogSummary::shops()`. A shop's exact position inside the interior is not exported: it needs the
`.soc` of the container.

**Kind and brand** need the shop containers: the catalog opens every socpak below `PU\Shops` and
the `shopcomm` and `shopfood` folders below `PU\loc` and reads `metadata\entitycomponentshop.xml`
(CryXml or text). A container that cannot be read is counted, not an error
(`containers_unreadable`).

**Items** (ADR-0071). The inventory files `Data\Scripts\ShopInventories\*.json` list item GUIDs of
`EntityClassDefinition` Records. The `ShopID` chain inside a file is no link to a shop (ADR-0069).
The link is set on the shop side: the `.soc` of a container holds the `SCShop` component of the
Entity (`EntityComponentShop`, `SShopComponentParams`) with the parameter `offlineInventoryJSON` or
`offlineInventoryXMLName`, whose value is a path of an inventory file. The file stores each string
once, so a later component of a file repeats neither the component name nor, often, the GUID of its
Entity; the catalog ties a path to the Entity whose `CryGUID` (`entitycomponentshop.xml`) or object
name (`<container>_editor.xml`) is the closest anchor before it, with only known structure strings
in between (at most one position, and a GUID only right behind `SCShop`), and leaves everything else unknown (ADR-0073). It reads the `.soc` of every socpak below
`PU` and `EA`; kind and brand still come from the shop folders only. The `offlineInventory*`
attributes of the shop prefabs below `Data\Prefabs\Shops` name the Entity through its editor `Id`,
which is the container `CryGUID` with the bytes reordered; a prefab is one more place. A shop gets an
item list when every place that names its `entityGuid` names the same file (compared by lower-case
stem; `.xml` and `.json` name the same file), the Archive holds it and it is no placeholder. Several
`.soc` files of one container follow the same rule. A `.soc` that cannot be read leaves only the
inventory of its container unknown; kind and brand do not depend on it.

The selected inventory Entry is read to completion before parsing. Catalog export imposes no
fixed byte limit on that inventory JSON. Archive read failures leave the inventory unresolved
and are recorded in export health.

| Field | Source |
|-------|--------|
| `inventory` | The lower-case stem of that file, e.g. `inv_admin_newbabbage`; `null` when no place names a file, the places disagree (or only some name one), the file is a placeholder, or the file is missing or unreadable |
| `itemGuids` | The `ID.ID` GUIDs of the elements of the file (`Collection.Inventory`, or `Inventory` in the shape without `Collection`), lower case, ordered, without repeats, and possibly `[]` for a file without items; an `ID` value that is no GUID is left out and counted in `item_ids_dropped`; `null` exactly when `inventory` is. The parent carries the selected registry, component and physical file contributions. Each member carries the exact JSON item ID alternatives in that file. Filtering a shop-to-item association checks both the parent list cell and the item cell. The file's exact design-time fields are in `definitions.shopInventories`. |

### Physical offline shop inventories

`definitions.shopInventories` has `meaning: "offlineDesignTime"`. It covers every direct
`.json` Entry in `Data\\Scripts\\ShopInventories`, including orphan and placeholder files.
This physical selection is global under a Record filter and independent of the shop registry.
A required read failure in this table prevents Normal publication. Its diagnostics distinguish
`archiveKeyRequired`, `wrongArchiveKey` and other read failures.
Rows are ordered by normalized path and physical directory index. The original `sourcePath`,
normalized path, `inputRef` and `entryIndex` keep each file independently addressable.

A present `document` holds the complete ordered, owner-local `source` arena and typed `fields`
that reference it. The arena retains repeated members and lines, original array order, and every
number as its exact authored token string. `fields.inventory` and
`fields.collection.value.inventory` address the two inventory layouts. Line fields directly
address each ID alternative, buy and sell prices, current and maximum inventory, and complete
rental offering values. Absent, null, empty and invalid typed fields retain separate states.
Unfamiliar rental variants and their nested values remain available in the same source arena.
Valid GUID alternatives and GUID-only `ShopID` chains use the public text domain. Other
authored string values, including malformed identifier text, use `x-gameText`. Encryption
markers are independent and come from each actual source use.

An unreadable Entry retains its directory identity and an unreadable document state. Nonzero
invalid JSON retains its identity and an invalid document state. A complete read of a physical
zero-byte Entry gives a sourced absent document. The selected shop resolution reports
`emptySource` with that physical target and its payload evidence. The `inventory` and `itemGuids` cells share their selection evidence and state. The states are
`absent` for no declared file, a placeholder, a verified missing file or a verified zero-byte
file, `invalid` for conflicting declarations, and `unreadable` for an incomplete survey or
unreadable inventory. A valid empty `Inventory` array produces a present `itemGuids` value of
`[]`. A missing physical target has no definition row. The zero-byte read is a
complete observation and still contributes to the legacy `unreadable_shop_inventories`
warning. Archive-entry health retains other read or projection failures. Price, stock and
rental values describe offline design settings. Stored values and unit-bearing source fields
retain their original form.

### Client profile settings

`configurations.clientProfileSettings` contains the original general and VR profile
documents with independent selection and read outcomes, authoring order and source
origins. `completeness.clientProfileSettings` records whether a Filter scoped the Archive
Entry selection. Each document reports its own availability. [Client profile settings](client-profile-settings.md)

### Shop inventory resolution

A structured SOC `offlineInventoryJSON` attribute with a direct shop-inventory path ending in
`.jsonn` can select the same-stem `.json` Entry. The Archive directory must establish that the
original path is absent, exactly one canonical JSON Entry exists, and no same-stem XML or path
alias competes. The candidate retains the original attribute value and the selected stem cites
the physical directory path that supported the correction. Inventory contents pass through the
ordinary JSON reader. Other malformed attributes retain their source diagnostics.

`declarations.shopInventoryResolution` has one row for each emitted shop. Its `shopKey` is
derived from the actual registry `ID` attribute. `candidates` retains the original declared
paths and normalized stems. A readable component with no declaration and an unscanned
component keep distinct original observations. A populated rejected path has its own candidate
and protected authored text. An undecodable value has an unreadable path state. Rejected
declarations make the candidate source set incomplete. `physicalTargets` links to actual directory
occurrences by `inputRef` and `entryIndex`.

In decoded SOC entity chunks, shop inventory references are read from the original XML
attribute slots and joined to one matching Entity GUID in the same container metadata.
The declaration evidence identifies the exact CryXML attribute or text XML byte range.
Repeated placements and declaration slots retain their source positions. Unsupported or
unreadable entity declarations leave the inventory evidence incomplete.

| Status | Observed resolution |
|---|---|
| `resolved` | The component and prefab declarations agree on one readable inventory. An authored empty list resolves. |
| `placeholder` | The agreed stem identifies a designer placeholder. |
| `ambiguous` | The observed places disagree, or multiple physical files have the same normalized stem. |
| `missing` | The actual agreed name has no physical target in the enumerated directory. |
| `unreadable` | The actual target is present and its item list cannot be read or interpreted. |
| `emptySource` | The unique physical target was fully read and has zero decoded bytes. |
| `unscanned` | The available component or directory evidence cannot establish a declaration. |
| `noDeclaredFile` | An observed readable component declares no inventory file. |

Each candidate, target and status has its own source marker. Resolved and empty-source statuses include the
consumed inventory content. An attempted unreadable payload remains in source health. Directory
identity and the original authored path retain their independent origins. Source-set completeness
describes the coverage of the actual contributors. A Record filter omits shop resolutions while
the physical file definitions remain selected.

The placeholders are `temp_outpost_shopinventory` (the same six commodities for bars, refineries,
showrooms and an expo hall; 264 shops) and `inv_cryastro_dummy` (three GUIDs no Record has; 136
shops): they say nothing of what a shop sells, so those shops have no list
(`inventory_placeholder`).

`ShopSummary` counts `with_items`, `inventory_ambiguous`, `inventory_placeholder`,
`inventory_unscanned` (no place has the component in a readable `.soc`: nothing is known, which
differs from a component that names no file), `inventory_missing` and `inventory_unreadable`
(shops), `soc_components` (found components), `inventories_named` and `inventories_in_archive`,
`inventories_missing`, `inventories_unreadable` (files), `item_ids_dropped`, and the containers
(`other_containers_*`) and prefabs (`prefabs_*`, `prefab_entities`) read besides the shop folders.
On LIVE (Build 4.10.197) 294 of the 1,184 shops get an item list (Stanton 275, Pyro 19) from 33
distinct files; 399 shops name a placeholder, 43 have ambiguous places, 64 are unscanned and one
names an empty file. 57 of the 119 inventory files are named by a container or prefab; the other 62
(event sales, `mining_*`, most landing-service, city and station shop files) are named by nothing
in the Archive and by no Record, and the `ShopID` they carry is a stale design-time value, so those
shops have no item list. 2,395 of the 2,422 item GUIDs of the exported files (distinct per file) are
the `guid` of an entry of `items`, `ships`, `commodities`, `paints` or `loadouts`; the other 27 (all
21 of `inv_commex_tdd_area18`, 6 of `inv_astroarmada_area18`) are no Record of the DataForge. Prices,
stock and demand are never exported.

**Filter.** Shops are selected by no record path. With a Filter no shop is read and `shops` is empty
with the status `partial`.

### Crafting data

`p4kx_export::read_crafting(&forge, filter, cancel)` returns the standalone interpreted
`Crafting` datasets. It shares Blueprint selection and cost interpretation with the catalog
and reads Records in parallel. A Record it cannot read is reported in `failures`. The catalog
additionally retains the original source arena and per-datum provenance.

The blueprints are selected by the one rule behind the `blueprints` kind (`selects_blueprint`:
prefix, type, Filter, leftover tokens). A `Filter` keeps only the blueprints and pools it matches;
the other lists are always read, and the catalog then keeps the properties, quality distributions
and dismantle processes whose record path matches.

The reward sources come from one scan of the Record tree of every Record for a Reference to a pool
(ADR-0154), so nothing is capped. A Record the scan cannot read is no `failures` entry, because it
is not known to be selected; it could hide a source, so `declarations.blueprintRewardSources` may be incomplete exactly
when `Crafting::warnings` holds `UnreadableRewardRecords` with the count. Only an unreadable Record
whose struct type is that of a Record with a reward source counts, and when no source is found at
all every unreadable Record counts. `p4kx catalog` prints the warning and exits with code 1
(`CatalogSummary::warnings()`; `is_complete()` is then `false`); the windows of the GUI list it.

A modifier range (`valueRanges` of a gameplay-property modifier) is read only for the two types
`CraftingGameplayPropertyModifierValueRange_Linear` and `..._LinearIntegerAdditive`. A range of any
other struct type is left out of the modifiers of its tier and counted:
`Crafting::warnings` then holds `UnknownModifierRanges` with the number of such ranges over all
selected blueprints and the sorted, distinct struct types in `details`
(`BlueprintTiers::unknown_range_types` has them per blueprint). A build without such a range has
no warning. The shape of the catalog does not change, so this follows ADR-0054 as an additive
extension: only the `warnings` of `p4kx catalog --json` gain the kind `unknown_modifier_ranges`.

The Rust names of the library:

| Field | Source |
|-------|--------|
| `blueprints` | Each Blueprint has `produced_item` and ordered `tiers`. Each tier has `craft_time_seconds`, `research` and a flat `Cost` arena. `mandatory` and `optional` are owner-local `CostIndex` values. Use `Tier::cost`, `Tier::costs` or `Tier::mandatory_cost` to read them. |
| `Cost::Select` | `count` ("choose n") with its `options`, `slot` and `slot_key`; `chooses_fewer()` tells whether the player may choose fewer than offered |
| `Cost::Resource`, `Cost::Item` | As an ingredient (`resource` / `item`, `quantity_scu` / `quantity`, `min_quality`), before the context |
| `CostContext` | `quantity_multiplier` (`CraftingCostContext_QuantityMultiplier`), `composition_inclusion` (`Include` or `Exclude`) and `modifiers` (`CraftingCostContext_ResultGameplayPropertyModifiers`): per gameplay property the linear ranges `start_quality`, `end_quality`, `at_start`, `at_end` as a factor (`Multiplier`) or a summand (`Additive`) |
| `properties` | `CraftingGameplayPropertyDef` Records under `crafting/craftedproperties`: GUID, `class_name`, `record_path` and the localisation key of the name |
| `quality_distributions` | The Records under `crafting/qualitydistribution`: a default distribution (`min`, `max`, `mean`, `stddev` of a normal distribution) or the list of location overrides, each a location GUID with its distribution |
| `pools` | `BlueprintPoolRecord` Records under `crafting/blueprintrewards`: entries with a blueprint GUID and a `weight`; `total_weight()` is the sum of the positive weights |
| `reward_sources` | Every struct of any Record that holds a Reference to a pool (on LIVE the `BlueprintRewards` result of a contract): the Record, the `id` and `debugName` of the nearest enclosing struct that has an `id` (both from the same struct), the pool and the `chance` |
| `reward_chances()` | Per blueprint the chance of each source: `chance` x `weight` / `total_weight()`; a source whose chance is outside 0 to 1 and an entry with a weight of zero or less give none |
| `dismantle` | The `GenericCraftingProcess_Dismantle` Records under `crafting/blueprints/dismantle`: `efficiency` and `dismantleTime` in seconds (LIVE: one process, 0.5 and 15 s) |
| `global` | `CraftingGlobalParams`: the default blueprints (`defaultBlueprintSelection`), the resources dismantling never returns and the entities that cannot be dismantled |
| `dismantle_return()` | Links a blueprint to the only dismantle process and to the item it produces, unless the item is on the item blacklist; `None` for none or several processes. It carries the `tier` (the first) and the `excluded_resources` |

LIVE holds exactly one tier per blueprint; the library
reads as many as a Record holds. The dismantle Record is generic, not one per item: the link from
an item to its returns runs through the blueprint that produces it.

The assumptions of `dismantle_return()` are those under "Dismantle returns" above: the first tier,
no resolved `Select`, no applied quantity multiplier, the resource blacklist exposed as
`excluded_resources` and never applied to an amount.

A blueprint's `name` is almost always a placeholder key. `producedItemGuid` identifies the item
that provides the display name.

Manufacturer GUIDs are always taken from References, never from embedded Records (ADR-0004).

### Ship store slugs

The Archive does not carry the slugs of the RSI pledge store, and the class names of the ships do
not match them. `shipStoreSlugs` maps the slug of a store ship to the catalog ship (ADR-0117). It
is curated data that ships with the tool (`crates/p4kx-export/data/ship-store-slugs.json`), not
extracted data. Only identifiers are kept: no description, price or media.

| Field | Meaning |
|-------|---------|
| `slug` | The last segment of the ship's pledge URL, e.g. `Aurora-Mk-I-ES`; the entries are ordered by it |
| `rsiId` | The ship id of the RSI store |
| `storeName` | The name the RSI store shows |
| `className` | The `className` of the mapped ship; `null` for a retired slug |
| `shipGuid` | The `guid` of the mapped entry of `ships`; `null` for a retired slug |
| `retired` | `true` when the slug's ship no longer exists in the game data |

Resolve a pledge by `rsiId` or `slug`, then join `(shipGuid, className)` to the same
Build's `ships[]` sourced `(guid.value, className.value)`. For the reverse lookup,
index the pair to a list of Store rows. Each row retains its own RSI identity even
when multiple pledges resolve to the same Ship. `storeName` is display metadata.
All mappings that share a GUID agree on the class and retirement state.

The kind lists only slugs that map to a ship: the slugs of ships the game does not have (the
store lists concept ships) are not exported. A slug is exported when its `shipGuid` and `className`
resolve to a ship of the catalog, or when it is `retired`; a retired slug stays in the data file
and in the catalog so that a pledge of it is recognised as retired and not as unknown. A mapping
that no longer resolves and is not retired is *stale*: it is not exported, `p4kx catalog` prints
it and lists it in `store_slugs.stale` of `--json`, and the real-Archive test
`every_mapped_store_slug_resolves_to_a_ship_of_the_archive` fails. With a `--filter` the ships are
a selection, so only the slugs that resolve are exported, no retired one, and none is stale; the
kind is `partial` then. When slugs are mapped but none resolves (an Archive that is not the
game's, such as a test fixture), every mapping would be stale: the check is skipped, `stale` stays
empty and `store_slugs.stale_check_skipped` is `true`.

The mapping is made by name, manufacturer and variant against the ships of the `LIVE` Archive whose
`playability` is `player`, and it never guesses:

- the store name and the catalog `name` are compared after dropping the manufacturer, punctuation,
  case and the word "Edition";
- when several player ships share the name, the one whose `className` carries none of the
  suffixes of a display, test, event or reward variant (`_BISÃ¢â‚¬Â¦`, `_Display`, `_TEMPÃ¢â‚¬Â¦`, `_Unmanned`,
  `_AI_Super`, `_Tutorial`, `_Collector_Ã¢â‚¬Â¦`, `_Prison`, `_NoInterior`, `_HijackedÃ¢â‚¬Â¦`, `_Exec_Ã¢â‚¬Â¦` and the
  like) is taken; if more than one is left, the slug stays unmapped;
- a store name that is part of exactly one catalog name can map after individual review.
  A pledge confirmed to include the same primary Ship can share its mapping. For example,
  `Carrack-Expedition` and `Carrack-Expedition-W-C8X` both resolve to the Carrack Expedition.
  The mapping carries only the primary Ship identity. Use of the mapping establishes no
  entitlement to a package's other contents.

The LIVE `13220821` refresh uses the four Gold Standard Records for the current
Constellation Mk IV pledges. Taurus and Phoenix also have older player Records.
RSI's published Mk IV rework and the matching physical Gold Standard owners establish
the curated selection. The S-65 Stingray pledge matches the standard laser loadout.
Its other player Record uses ballistic weapons and shares the localization key.
MOLE Carbon Edition and Talus Edition remain unmapped. The Archive has their
named Paint Records and the base MOLE, and the reviewed RSI data leaves the
physical pledge binding unverified.

To refresh the file, a human fetches the public ship list of the store, reduces it to the
identifiers above (`id`, name, pledge URL), and adds new slugs to `ship-store-slugs.json` with
`className` and `guid` `null`; the tool never fetches anything. Then `p4kx catalog` on the
current `LIVE` Archive and its `store_slugs.stale` and `unmapped` lists show what is left to do: a
new ship is mapped by the rules above, a mapping that went stale is corrected, or marked
`"retired": true` when the ship is gone. The real-Archive test must pass before the file is
committed.

### Reviewed Store ship pledges

Catalog v3 includes `shipStorePledges.snapshot` and `shipStorePledges.rows`. The snapshot records
`source`, `captureDate`, `sourceRevision` (the capture's SHA-256), `reviewedDate`, positive
curated `revision` and `scope`. The `reviewedCandidates` scope covers the manually checked
cohort from the dated capture. Every Catalog includes all bundled rows. That population is
independent of the Archive, Build and Filter and may omit pledges outside the reviewed cohort.

Each row has `rsiId`, `slug`, `storeName`, `manufacturerCode` and `productionStatus`.
The current published status vocabulary includes `in-concept`. The dated Matrix review
supplies that production status, while an original Ship Record supplies its own physical
identity and `playability`. For example, Javelin (63) has a reviewed `in-concept` row and a
`special` Archive Ship. A retired mapping is identified by `shipStoreSlugs[].retired`.
A pledge absent from both tables has no reviewed status or validated mapping in this output.

The installed LIVE `13220821` Normal export matched all 27 bundled pledge rows
and all 227 Store mapping rows in order. The comparison also verified the
physical Javelin and AI siblings, active player Store bindings and the separate
unbound MOLE outcomes. The matching producer used revision
`23addc5e164f65a28a6f258302b30453b331edce`.

### Item recovery rules

`configurations.itemRecoveryEconomy` and `configurations.itemRecoveryOverrides` contain the
economy and override declarations of the selected physical policy. A policy is a readable
`ItemRecoveryConfigurationParams` Record under `libs/foundry/records/entitlementpolicies/`
whose Record path matches the Filter when one is set. When several readable policies qualify,
the smallest Record path wins. Rows are empty when none qualifies, with unreadable selected
Records listed in export health. Dataset availability measures the declared Record family.
The economy row has a canonical Record header. The override and policy rows identify the
same physical owner by Record index and GUID.

The economy row's `economyParams` holds sourced values with exact original coordinates.
The `legacy` object retains the following fields only where they are authored directly in the
economy source.

| Direct source fields |
|----------------------|
| `globalBrickTimer`, `baseReclaimTime`, `baseDeliveryTime`, `scalingPriceFloor` |
| `aUECperSecond`, `deliverySecondsPerAUEC`, `expeditedDeliverySecondsPerAUEC` |
| `claimCostMultiplier`, `maxExpeditedClaimCostMultiplier` |
| `defaultLoadoutCooldownMultiplier`, `defaultLoadoutDeliveryTimeMultiplier`, `defaultLoadoutExpeditedDeliveryTimeMultiplier` |
| `vehicleChassisRepairCostMultiplier`, `vehicleComponentRepairCostMultiplier`, `vehicleRepairMissingItemCostMultiplier` |
| `flatMinimumDeliveryTimerSeconds` |

The nested groups remain independent even where their source member names match.

| Sourced group | Original members |
|---------------|------------------|
| `cooldownParams` | `baseTime`, `scalingPriceFloor`, `aUECperSecond`, `defaultLoadoutMultiplier` |
| `deliveryParams` | `baseTime`, `scalingPriceFloor`, `aUECperSecond`, `defaultLoadoutMultiplier`, `expeditedTimeMultiplier`, `expeditedDefaultTimeMultiplier` |
| `repairParams` | `vehicleChassisRepairCostMultiplier`, `vehicleComponentRepairCostMultiplier`, `vehicleRepairMissingItemCostMultiplier` |
| `escalatingClaimsParams` | `escalatingClaimChainLimit`, `escalationTimerMultiplier`, `escalatingClaimCostMultiplier`, `escalatingDeliveryTimerMultiplier` |

The first three groups retain original Single bits and a nullable finite number. Older direct,
escalating and override scalars retain their authored numeric storage. Missing, explicit null,
unreadable, unsupported and invalid source values have separate states. Invalid known fields and
unsupported concrete groups appear in diagnostic exports with their original field receipts. The
nominal source declaration is checked for owned recovery blocks and empty override arrays.

The sourced `overrides` array retains each `ItemRecoveryOverrideGroupDef` in authored order.
Its sourced `classes` links keep their original positions, including explicit null and
unresolved references. The remaining members are `claimCooldownMultiplier`,
`claimCostMultiplier`, `deliveryTimeMultiplier`, `maxExpediteCostMultiplier`,
`expeditedDeliveryTimeMultiplier`, `flatMinimumDeliveryTimerSecondsMultiplier`,
`escalationTimerMultiplier`, `escalatingClaimCostMultiplier` and
`escalatingDeliveryTimerMultiplier`. A source that removed the flat minimum multiplier has
an absent cell. The catalog preserves source rates and multipliers as authored. The fee stated
directly in ship source data remains `insurance.value.shipInsuranceParams.value.expediteFee`.

The policy row retains the seven independent ordered condition arrays `Insurable`,
`ReplenishOnRearm`, `ReplenishOnClaimAndRepair`, `NeverRepair`, `NeverLTP`, `ForceLTP` and
`DontEquipWhenBricked`. Each original slot has its own source state and preserves its concrete
condition type. An entity-class condition contains its authored class reference. An entity-tag
condition retains the original `TagList` and its ordered tag links. An item-type condition
retains both original enum names. A fieldless base condition remains explicit.

The sourced `notificationParams` block preserves all ten authored locale-key fields as
protected game-text references. These are `itemBricked_Title`, `itemBricked_Body`,
`vehicleBrickedWithItems_Body`, `itemBricking_Title`, `itemBricking_Body`,
`vehicleBrickingWithItems_Body`, `onBrickedItemUsed_Title`, `onBrickedItemUsed_CannotEquip`,
`onBrickedItemUsed_CannotFire` and `onBrickedItemUsed_CannotUse`. The keys and available
translations use the canonical
`texts.gameTexts` table. The original `onBrickedItemUseNotificationBuffer` Single retains
its own exact bits and source receipt. Source arrays and notification values are preserved
without evaluating a policy.

The ignored `item_recovery_cooldown_delivery_and_repair_match_original_source_fields` check
opens the selected Archive and compares the filtered catalog against the original DataForge
source. Set `P4KX_EXPECT_NESTED_RECOVERY=1` for an Archive that must exercise all three
nested groups. The legacy and nested source generations can be checked in separate runs.

### Texts

The catalog loads each available supported `global.ini` once. The canonical registry has a public
table (`texts.publicNames`) and a protected table (`texts.gameTexts`). Follow an owner's typed
reference to the registry row with the matching `id.value`. That row retains the exact authored key.
A key used in both domains has a separate reference in each domain. Protected fields carry
`x-gameText`. The encryption marker records source origin independently of the text domain.

The supported language codes are `en`, `de`, `fr`, `it`, `es-ES`, `es-419`, `pt-BR`, `ja`, `ko`,
`zh-Hans` and `zh-Hant`. Each available language has a member in `translations.value`. A missing
language has no member. An available language without a matching key has absent text forms with
lookup evidence. A present empty value keeps its present state. The catalog does not substitute
text from another language.

Each language has two sourced forms. `sourceText` preserves the winning decoded value after
`=`, including any markup, spaces and literal backslash-n. `displayText` converts literal
backslash-n to a line feed and trims edge whitespace. Lookup matches keys case-insensitively in
ASCII, with an optional initial `@`. A plain key wins over its suffixed form unless the plain
value is empty and the suffixed one is not. Admitted placeholder keys retain the key and have
absent forms without consulting a language Entry. Individual owner fields can have stricter
key admission rules, as documented for those fields.

The source uses identify the authored key, the actual lookup and the winning language Entry where
applicable. A malformed encoding or skipped line is reported as a localization note. The catalog
reads and indexes each available `global.ini` in full, subject to Archive integrity checks and
available memory. See [ADR-0394](adr/0394-localization-is-complete-by-default.md). The summary uses the display result
for resolved and unresolved counts and reports placeholders separately. See [TextForms](catalog-text-forms.md)
for the wire structure, state rules and per-form source evidence.

## Selection

The mining selectors `refiningProcesses` and `miningGlobalParams` below are Kind names, not v3
root-array paths. They emit `definitions.refiningProcesses.rows` and
`configurations.miningGlobal.rows` respectively.

| Kind | Record-path prefix | Type |
|------|--------------------|------|
| `manufacturers` | `libs/foundry/records/scitemmanufacturer/` | `SCItemManufacturer` |
| `items` | `libs/foundry/records/entities/scitem/`, `libs/foundry/records/entities/legacyitems/` | `EntityClassDefinition` |
| `ships` | `libs/foundry/records/entities/spaceships/`, `libs/foundry/records/entities/groundvehicles/` | `EntityClassDefinition` |
| `definitions.commodities`, `projections.commodityPresentation` | Commodity source identities/authored bindings and explicit derived choices, paired in existing GUID order; see [Commodity contract](catalog-commodity-types.md) |
| `blueprints` | `libs/foundry/records/crafting/blueprints/crafting/` | `CraftingBlueprintRecord` |
| `resourceGroups` | `libs/foundry/records/resourcetypedatabase/` | `ResourceTypeGroup` |
| `blueprintCategories` | `libs/foundry/records/crafting/blueprintcategories/` | `BlueprintCategoryRecord` |
| `blueprintCategoryLists` | Every original path, subject to the ordinary Filter | `BlueprintCategoryDatabaseRecord` |
| `gameplayProperties` | `libs/foundry/records/crafting/craftedproperties/` | `CraftingGameplayPropertyDef` |
| `qualityDistributions` | `libs/foundry/records/crafting/qualitydistribution/` | `CraftingQualityDistributionRecord` and `CraftingQualityLocationOverrideRecord` |
| `qualityQuantizations` | `libs/foundry/records/crafting/qualityquantization/` | `CraftingQualityQuantizationRecord` |
| `blueprintPools` | `libs/foundry/records/crafting/blueprintrewards/` | `BlueprintPoolRecord` |
| `dismantleProcesses` | `libs/foundry/records/crafting/blueprints/dismantle/` | `CraftingBlueprintRecord` whose process is a `GenericCraftingProcess_Dismantle` |
| `paints` | Those of `items`; only the items whose `AttachDef.Type` is `Paints` | `EntityClassDefinition` |
| `loadouts` | Those of `ships`, and of `items` whose Record defines ports or names a default Loadout | `EntityClassDefinition` |
| `starmapObjects` | `libs/foundry/records/starmap/pu/` | `StarMapObject` |
| `contracts` | `libs/foundry/records/contracts/contractgenerator/` | `ContractGenerator` |
| `mineableElements` | `libs/foundry/records/mining/mineableelements/` | `MineableElement` |
| `refiningProcesses` | `libs/foundry/records/refiningprocess/` | `RefiningProcess` |
| `mineableCompositions` | `libs/foundry/records/mining/rockcompositionpresets/` | `MineableComposition` |
| `miningGlobalParams` | `libs/foundry/records/mining/miningglobalparams` | `MiningGlobalParams` |
| `harvestableProviderPresets` | `libs/foundry/records/harvestable/providerpresets/` | `HarvestableProviderPreset` |
| `subHarvestableConfigs` | Any Record path | `SubHarvestableConfigRecord` |
| `subHarvestableMultiConfigs` | Any Record path. The top-level convenience projection uses `libs/foundry/records/harvestable/slotpresets/`. | `SubHarvestableMultiConfigRecord` |
| `harvestableLocations` | None: the `.soc` files of the containers below `ObjectContainers/PU/system`, no Records | |
| `shops` | None: the `ShopBackup` entries of `LayerBackups.xml`, no Records | |
| `factionReputations` | `libs/foundry/records/factions/factionreputation/` | `FactionReputation` |
| `reputationScopes` | `libs/foundry/records/reputation/scopes/` | `SReputationScopeParams` |
| `reputationStandings` | `libs/foundry/records/reputation/standings/` | `SReputationStandingParams` |
| `reputationPerks` | Any path | `SPerkReputationListParams` |
| `reputationTriggerRecords` | Any path | `SandboxTriggerRecord` |
| `reputationRewardAmounts` | Any path | `SReputationRewardAmount` |
| `infractions` | Any path | `InfractionDefinition` |
| `infractionSets` | Any path | `InfractionSet` |
| `jurisdictions` | Any path | `Jurisdiction` |
| `starmapTypes` | Any path | `StarMapObjectType` |
| `starmapPresets` | Any path | `StarMapPreset` |
| `starmapAmenities` | Any path | `StarMapAmenityTypeEntry` |
| `reputationContexts` | `libs/foundry/records/reputation/contexts/` | `SReputationContextUI` |
| `missionTypes` | `libs/foundry/records/missiontype/` | `MissionType` |
| `missionLocalities` | `libs/foundry/records/missiondata/pu_missionlocality/` | `MissionLocality` |
| `contractScopes` | Any Record path | `ContractScopeGlobalParams` |
| `locationEntityDeclarations` | Any Record path | `LocationEntityDeclaration` |
| `missionModuleDeclarations` | Any Record path | `ModuleDeclaration` |
| `locationResourceSlots` | Any Record path | `LocationResourceSlot` |
| `missionScenarios` | `libs/foundry/records/` | `MissionScenario` |
| `contractScenarios` | `libs/foundry/records/contracts/contractscenarios/` | `ScenarioProgress` |
| `cargoManifests` | `libs/foundry/records/cargomanifest/` | `CargoManifest` |
| `missionLocationTemplates` | `libs/foundry/records/missiondata/pu_locations/` | `MissionLocationTemplate` |
| `locationClasses` | Any Record path | `MissionLocationClass` |
| `locationFunctions` | Any Record path | `MissionLocationFunction` |
| `locationSizes` | Any Record path | `MissionLocationSize` |
| `locationClassRegistries` | Any Record path | `MissionLocationClasses` |
| `locationFunctionRegistries` | Any Record path | `MissionLocationFunctions` |
| `locationSizeRegistries` | Any Record path | `MissionLocationSizes` |
| `contractShipSpawns` | None: derived from the properties of the contracts and their templates, no Records | |
| `contractLocationQueries` | None: derived from the properties of the contracts and their templates, no Records | |
| `contractNpcSpawns` | None: derived from the properties of the contracts, templates and broker entries, no Records | |
| `contractEntitySpawns` | None: derived from the properties of the contracts, templates and broker entries, no Records | |
| `missionPropertySets` | None: derived from the authored property overrides of selected Generator Records | |
| `contractTemplates` | `libs/foundry/records/` (every folder; see "Contract templates") | `ContractTemplate` |
| `missionBrokerEntries` | `libs/foundry/records/` (every folder; see "Mission broker entries") | `MissionBrokerEntry` |
| `lootTables` | `libs/foundry/records/lootgeneration/loottables/` | `LootTableV3Record` |
| `lootArchetypes` | `libs/foundry/records/lootgeneration/lootarchetypes/` | `LootArchetypeV3Record` |
| `lootPoolFilters` | `libs/foundry/records/lootgeneration/filters/` | `PoolFilterRecord` |
| `lootSecondaryChoiceSingleLayers` | Any physical Record path | `LootV3SecondaryChoicesSingleLayerRecord` |
| `lootSecondaryChoiceMultiLayers` | Any physical Record path | `LootV3SecondaryChoicesMultiLayerRecord` |
| `lootThemes` | Any physical Record path | `LootThemeRecord` |
| `hierarchicalContextualDataPresets` | Any Record path | `HierarchicalContextualDataPresetRecord` |
| `lootGenerationGlobalParams` | Any Record path | `LootGenerationGlobalParams` |
| `legacyLootTables` | `libs/foundry/records/lootgeneration/loottables/` | `LootTable` |
| `legacyLootArchetypes` | `libs/foundry/records/lootgeneration/lootarchetypes/` | `LootArchetype` |
| `lootContainers` | None: the `entdata` files of the containers below `ObjectContainers/PU/system` and the containers they place, no Records | |
| `mineableEntities` | `libs/foundry/records/entities/mineable/` | `EntityClassDefinition` |
| `crafters` | `libs/foundry/records/entities/`, after the kinds above that take their own entities; only an entity with a `CrafterComponentParams` | `EntityClassDefinition` |

The blueprints and the five crafting kinds below them are read by `read_crafting`, with the one
selection rule behind it (`selects_blueprint` for the blueprints); `Kind::rule()` of
`qualityDistributions` names the first of its two types.

The resource-group prefix also holds the 207 `ResourceType` Records and the `ResourceTypeDatabase`
Record of LIVE, and the category prefix a `BlueprintCategoryDatabaseRecord`; the type selects only
the groups and the categories.

The blueprint prefix leaves out `crafting/blueprints/dismantle/`, whose Record is no recipe. The
`$templates` folders below it hold 27 real blueprints, each the only recipe for its item, and
stay: the token `templates` is not `template`.

The 46 `mining/mineableelements` Records hold no commodity of their own and are the kind
`mineableElements`; the `ResourceType` Records enter through the ingredients that use them.

An item without an `SAttachableComponentParams` component, such as a door or a shop counter, is
still an item; its definition fields are then `null` and `tags` is empty.

The prefix ignores case and the kind of separator; the type is the Record's struct, matched
exactly. `--filter` further selects by record path with the rules of `p4kx search` and marks
every kind partial.

Development leftovers are left out and counted as skipped: a selected Record whose record path or
name holds one of the tokens `test`, `template`, `dummy`, `debug`, `deprecated` or `dev` as a
whole word, ignoring case. Words are the runs of ASCII letters and digits, so `paint_test_01` is
left out while `contestant` stays. For the ships kind the words `temp`, `display` and `hijacked`
count as well, so `glsn_shiv_temp_cosa` and `aegs_avenger_stalker_display` are left out; the other
kinds keep their `display` and `temp` Records (radar displays, shop displays and the like). This
changed the selection of `ships` for `schemaVersion` 2 (ADR-0167): 21 ships on LIVE are skipped
that version 1 listed.

A selected Record that cannot be read or projected is recorded in the measured `exportHealth`,
with its physical source index, GUID, readable path, stage and stable diagnostic category. A ship
whose loadout fails can retain its original ship row; that secondary failure is distinct from an
omitted ship source. Unreadable owned fields also remain visible when an optional mapper retained
the row. Source warning codes retain their producer counts and explicit units.

Normal publication refuses incomplete health before creating or replacing the destination.
Explicit `--diagnostic-partial`, or the separate diagnostic action in the desktop window, permits
the diagnostic file and records that mode inside it. A successful diagnostic write remains an
incomplete result. `completeness` retains its Filter-selection meaning. Consumers require both an
unfiltered selection and complete export health before treating absent rows as removals; preserve
existing data when a diagnostic artifact cannot establish absence. The complete field and counter
contract is [catalog export health](catalog-export-health.md) and ADR-0347.

## Determinism and size

Equal Archives and an equal export time give byte-identical catalogs. The export time is the
current time unless `--exported-at` or the `SOURCE_DATE_EPOCH` environment variable gives it in
seconds since 1970-01-01 UTC.

### Measuring output size

The compression test measures stored and decoded sizes, checks the reference thresholds and
compares the decoded frame with plain JSON. The Catalog v3 investigation records exact section
sizes for selected installed builds. [Catalog size investigation](https://github.com/p4kx-org/p4kx/issues/1151)

The [domain partition assessment](catalog-partitioning.md) describes an optional package with
reference-safe selective imports.

### Historical measurements

The following measurements describe earlier catalog revisions. Use the size investigation for
measurements of Catalog v3. The LIVE capture from 2026-10-26 included all kinds and English
and German texts. Bytes is the whole decoded catalog. Reproduce the Full-profile CLI summary with `p4kx catalog --provenance full --json`.

| Channel | Bytes | Manufacturers | Items | Ships | Commodities | Blueprints | Resource groups | Blueprint categories | Properties | Distributions | Pools | Dismantle | Paints | Loadouts |
|---------|------:|--------------:|------:|------:|------------:|-----------:|----------------:|---------------------:|-----------:|--------------:|------:|----------:|-------:|---------:|
| LIVE | 152,812,265 | 1,201 | 23,280 | 1,066 | 145 | 1,618 | 19 | 21 | 29 | 21 | 160 | 1 | 1,064 | 10,687 |

Measured on 2026-10-22, LIVE held 10,708 loadouts (1,087 ships, 9,621 items; 10,687 and 1,066 once ADR-0167 leaves 21 ships out) with 293,451 ports
(nested ports counted for every ship that carries them), 152,081 of them holding an Entity, 65 docked vehicles and 505
unresolved attachments (`unknown_class` 446, `dangling_reference` 54, `cycle` 5); 61 attached
Entities have no entry in `items` or `ships` (templates of doors, screens, beds and the like, and
HUD projectors); 193 loadouts list an issue (`unknown_class` 153, `dangling_reference` 30,
`xml_loadout` 4, `cycle` 3, `vehicle_implementation` 3; a loadout can list several).

Written as `.json.zst` (level 19, window log 27), measured on LIVE on 2026-10-26:

| Channel | Uncompressed | Compressed | Ratio |
|---------|-------------:|-----------:|------:|
| LIVE | 152,812,265 | 2,725,280 | 56.1 |

The compressed catalog is 4.3 % of the 64 MB upload limit; the decompressed one is 15.3 % of the
1 GB cap. The level trades time for size (measured before the loadouts, LIVE, window log 27; the
plain build then took 2.1 s):

| Level | Compressed | Ratio | Compression time |
|------:|-----------:|------:|-----------------:|
| 15 | 2,005,158 | 13.2 | about 2.6 s |
| 17 | 1,807,476 | 14.7 | about 7.0 s |
| 19 | 1,700,181 | 15.6 | about 12.7 s |

On that historical sample, level 19 took about 12.7 seconds and used 15 % fewer bytes than
level 15. See the [full export measurements](catalog-partitioning.md#complete-live-and-ptu-measurements)
for current runtime and storage costs.

Of the 26.5 MB of LIVE, the paints take 0.5 MB (the ship lists), the blueprints take 5.5 MB (4.4 MB more than before: the `tiers` repeat
the cost tree, every line with its null fields), the pools 0.43 MB (824 entries, 1,039 reward
sources), the quality distributions 29 kB and the properties 8 kB. The kinds written before are
unchanged: items 16.9 MB, manufacturers 1.1 MB; the ships grew from 1.4 MB to 1.5 MB.

On 2026-10-22 the loadouts took about 105 MB of the 132 MB of LIVE. A catalog builds on LIVE in 4.7 s with the
ship base fields and the loadouts (5.5 s before both). Without the loadouts, with the ship base fields,
LIVE builds in 3.2 s in a release test run (the `p4kx ships` export alone: 2.9 s). A first version
of the ship base fields that gave every split of the parallel scan its own ship reader took
10.5 s, because each reader began with empty Entity caches; the scan now keeps the readers in a
pool, so a reader keeps its caches across splits. Writing the `.json.zst` takes 19 to 22 s per
Channel in all. Left out as development leftovers on
LIVE: 2 manufacturers, 1,145 items, 78 ships (57 before ADR-0167) and 1 quality distribution (1,226 Records).

### Verification against the live data

Checked on all five Channels; `crates/p4kx-export/tests/real_archive/real_archive_verification.rs` repeats it
on `P4KX_DATA_P4K` and `P4KX_DATA_P4K_B`.

- The record-path prefixes and types match the live DataForge: of 29,693 `EntityClassDefinition`
  Records on LIVE, 25,704 lie under the item, ship and commodity prefixes (1,202 of them are left
  out as leftovers); the rest are actors, mineables, job boards, interiors, audio, decorations,
  lights and the like and are no catalog kind unless they carry a `CrafterComponentParams`, which
  makes them a `crafters` entry. All 1,203 `SCItemManufacturer` and 1,619
  `CraftingBlueprintRecord` Records are selected or left out as leftovers.
- The token list gained `debug`, `deprecated` and `dev`, which mark 20 further items on LIVE (the
  Vlk spewgun test weapons under `fps_weapons/dev`, `*_deprecated` bar counters, a debug hacking
  chip). `placeholder` stays out of the list: the manufacturer `PLACEHOLDER` is referenced by an
  item, and the manufacturers are complete by decision. The rest of the vocabulary
  (`fake`, `old`, `legacy`, `temp`, `prototype`) names real Records and stays in the catalog by
  decision.
- Every item's `manufacturerGuid` and the ship's vehicle manufacturer link resolve to a
  Manufacturer entry. Every `producedItemGuid`, ingredient `resourceGuid` and `itemGuid`
  resolves to a catalog entry. No GUID occurs twice.
- Every commodity `typeGuid` resolves to a `definitions.resourceGroups` entry, every chosen group
  parent in `projections.resourceGroupParents` to another one, and every blueprint `categoryGuid`
  to a `definitions.blueprintCategories.rows` entry. On LIVE the
  catalog holds 19 groups (8 of them nested) and 21 categories, 14 of which carry blueprints.
- Texts that carry a key (placeholder keys excluded), checked per kind against the Archive's own
  `global.ini` files (ADR-0270). An available language without the key has absent text forms.
  The verification prints results per kind, checks the original source and display forms, and
  compares the unresolved share with the historical English and German LIVE baselines below.
  The baseline shares count each occurrence of a keyed name or game text and round up to
  the hundredth.

  | Kind | Keyed texts | Missing in en | Missing in de |
  |------|------------:|--------------:|--------------:|
  | manufacturers | 2,248 | 8 | 116 |
  | items | 31,104 | 118 | 11,653 |
  | ships | 2,130 | 12 | 657 |
  | commodities | 474 | 0 | 75 |
  | blueprints | 1 | 0 | 1 |
  | resourceGroups | 12 | 0 | 0 |
  | gameplayProperties | 29 | 0 | 28 |
  | starmapObjects | 2,041 | 73 | 1,357 |
  | contracts | 49,956 | 706 | 26,425 |
  | refiningProcesses | 9 | 0 | 9 |
  | mineableCompositions | 231 | 1 | 36 |
  | factionReputations | 37 | 0 | 27 |
  | reputationScopes | 39 | 9 | 12 |
  | reputationStandings | 213 | 17 | 44 |
  | missionTypes | 41 | 0 | 23 |
  | missionLocationTemplates | 3,211 | 274 | 3,160 |
  | contractTemplates | 6,765 | 12 | 6,651 |
  | missionBrokerEntries | 17,733 | 566 | 17,636 |

  A contract text that holds a `~mission(Ã¢â‚¬Â¦)` token is rewritten by the exporter, so the
  verification only checks that such a text is present. The location templates have 3,297 name
  variants; 86 of them carry a placeholder key or none, which is not counted as a keyed text, so the
  German file lacks 3,246 of the 3,297 variants and 3,160 of the 3,211 keyed texts.

  The contract templates have 6,765 keyed texts (the titles, the objective, handler, override and
  option texts, the marker names; most are the placeholder `@LOC_UNINITIALIZED` and carry no key):
  the English file lacks 12 of them (`@MissionPhaseDebug_*` among them) and the German file 6,651,
  so the baselines of the kind are 0.01 and 0.99.

  The mission broker entries have 17,733 keyed texts (the titles, descriptions, givers, comms
  channels and end reasons, the objective texts): the English file lacks 566 of them
  (`@procedural_text_null` and the texts of the `cfp_*` delivery entries among them) and the German
  file 17,636, so the baselines of the kind are 0.04 and 1.0.

  The `p4kx catalog` summary counts them over all texts of the catalog: English 79,504 resolved,
  597 unresolved, German 31,495 resolved, 48,606 unresolved, 23,717 placeholders in each.
- Crafting, LIVE (the same on every Channel but for the counts): every blueprint has exactly one
  tier, equal to its `craftTimeSeconds`; 3 blueprints let the player choose fewer than offered
  (`BP_CRAFT_thp_light_legs_01_01_01`, `_03` and `_12`), 3 carry the factor 1.15 (the slaver arms
  `_01`, `_13` and `_15`), 1,555 carry quality modifiers of both range kinds; all modifier
  `propertyGuid` resolve, all 824 pool entries point at a catalog blueprint, 49 of the 160 pools
  have no reward source; 8 blueprints are available by default, 1,617 link to the one dismantle
  process (the exception, `BP_CRAFT_COOL_S04_CNOU_Pioneer`, has no produced item).

`p4kx catalog` warns on stderr when a catalog reaches 90 % of the limit and when it exceeds it;
the file is written either way.
