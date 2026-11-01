# Basetool catalog

The Basetool catalog is the JSON Export the basetool's P4K import consumes (ADR-0002). Its contract
is the JSON Schema (draft 2020-12) in
[`schemas/basetool-catalog.schema.json`](../schemas/basetool-catalog.schema.json), generated from
the Rust types in `crates/p4kx-export/src/contract.rs`. A test fails when the checked-in schema and
the types drift apart; `P4KX_BLESS=1 cargo test -p p4kx-export --test catalog_schema` regenerates
it.

## Versioning and consumers

The rule is ADR-0054 (amending ADR-0002):

- **Additive changes keep `schemaVersion`:** a new kind, a new field on an entity, on `source` or
  on an ingredient, a new kind in `completeness`. Example: a `playable` field on every ship.
- **Breaking changes raise `schemaVersion`:** renaming, removing or retyping a field, a changed
  meaning, restructuring a kind, or a changed selection or ordering of an existing kind. Example:
  renaming `crewSize`.
- **A consumer ignores kinds and fields it does not know,** and still rejects a `schemaVersion` it
  does not know. The JSON Schema therefore allows additional properties everywhere; every field it
  lists is required.
- **Guarantees the Basetool relies on;** breaking one is a breaking change: an existing field never
  changes its type, its meaning or whether it can be `null`; a new kind never changes the
  selection or the ordering of an existing kind; the meaning of `completeness` stays.
- **A new value in an existing closed value set is additive,** e.g. a new playability value or a
  new `itemClass`. A reader maps an unknown value to `null`. The schema documents such a set but
  keeps the type a string, so an older schema accepts the new value. A set derived from a Rust
  enum lists its known values in the `x-values` annotation of the definition (`value` and
  `description` per entry); a schema test rejects any `enum` or `oneOf` of constants.
- **Loadouts (#393), paints, item stats and the starmap are kinds of the one catalog;** the size is
  handled by compression (`.json.zst`), not by a second Export. The Basetool accepts `.json`,
  `.json.gz` and `.json.zst`; the 64 MB limit applies to the compressed upload, the decompressed
  content is capped at 1 GB. The compressed form is described under "Compressed form" below.
- **Version 2 (ADR-0161)** is breaking against version 1 in one respect: a paint also needs its own
  `RequiredTags` offered by the port or the ship (see "Paints"), so `paints[].shipGuids` lists
  fewer ships than version 1 did (40 fewer on LIVE 4.7). No field changed its name or type. The
  Basetool import must accept version 2.
  Version 2 also fixes a port whose name several parts of the vehicle file share (#677): the first
  such part gives the port its sizes, types and flags, as in the tree, so the `Radar_Helper` port
  of eight Ursa ships on LIVE 4.10 no longer lists `start_hidden` in its `flags`.
  Version 2 was released with 0.4.0 and has the kind `qualityQuantizations` (#692, ADR-0177), the
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
  `locationPrerequisites` (#902, ADR-0266). Later additions keep version 2 as long as they are
  additive. The contract field `fixedRewards` (#888, ADR-0268) is one, and so is `cooldownLayers`
  (#909, ADR-0271), and the contract fields `offeredAt`, `giverFaction` and `blueprintRewards`
  (#915, ADR-0273), and so are the kind `contractTemplates` (#812, ADR-0313) and the kind
  `missionBrokerEntries` with the contract field `brokerEntryGuid` (#813, ADR-0320), and the kinds
  `contractNpcSpawns` and `contractEntitySpawns` with the fields `npcSpawns` and `entitySpawns` of
  a contract, a template and a broker entry (#815, ADR-0324).
- **A production import takes catalogs of the Channel `LIVE` only.** `PTU`, `EPTU`, `TECH-PREVIEW`
  and `HOTFIX` are confidential test Channels (ADR-0038).

### Game texts

Identifiers, short names and fact values may reach third parties; descriptions and lore may not.
The game texts of the catalog are every `description` and every `displayType`, the `name` of a
starmap object (a place name that comes from localisation, by the owner's decision for #321), and
the `title`, `missionType`, `contractor`, the `displayName` of a contract's faction and the `place`
of a pickup place of a contract, the `processName` of a refining process and the `depositName`
of a mineable composition, the `displayName` of a faction, a reputation scope and a reputation
standing, the `displayName` of each name variant of a mission location template and the
`typeName` of a mission type, each with
its `key`, `en` and `de`. `name` of the other kinds, `role`, `career` and the other fields are
identifiers, short names or fact values. The catalog keeps the `description` fields for the
Basetool's logged-in members; the Basetool keeps them, `displayType` and the contract texts away
from third parties. Every such field carries
`"x-gameText": true` in the JSON Schema, and no other field does; a test checks this.
Every game text is trimmed of leading and trailing white space; see Texts.

A closed value set that the schema derives from a Rust enum is a definition of `"type": "string"`
with the known values in `x-values`, one `{"value", "description"}` entry per value (`description`
only where the value has one), e.g. for `FactLayer`:

```json
{"type": "string", "x-values": [{"value": "subContract", "description": "The parameter overrides of the sub contract."}, {"value": "contract"}]}
```

`x-values` documents the set; it does not validate. A reader treats a value it does not know as
additive: it maps it to `null` or ignores it. A schema test rejects any `enum` or any `oneOf` or
`anyOf` of constants.

```powershell
p4kx catalog <archive> --out basetool-catalog.json [--filter <filter>] [--exported-at <seconds>]
p4kx catalog <archive> --out basetool-catalog.json.zst
p4kx catalog schema
```

### Compressed form

An output name ending in `.zst` (any case) writes the catalog as one zstd frame; a name ending in
`.gz` is refused with exit code 2, because gzip is not written; any other name writes plain JSON,
which stays the default. The zstd encoder is libzstd (ADR-0149). The JSON is serialised straight into the encoder and
never held in memory; it is serialised twice, the first time only to count its length. The frame
uses zstd level 19, long-distance matching, a content checksum, and declares the content size
(`Frame_Content_Size`), so the window it needs is the content size, about 26.5 MB for LIVE, and not
the 2^27 maximum the encoder is allowed. A plain `zstd -d basetool-catalog.json.zst` is enough, no
`--long` option is needed, and a decoder's memory is about the size of the catalog. The decoded
bytes are exactly the plain catalog for the same Archive and export time (tests check this with the
Rust decoder and read the window and content size from the frame header). The file is written to a
temporary file next to the target and renamed on success, so a failure or Ctrl+C leaves an
existing catalog as it was.

The Basetool detects `.json`, `.json.gz` and `.json.zst` by magic bytes, not by name. Its limits
apply to two sizes, and `p4kx catalog` and the desktop window check both:

| Size | Limit | Constant |
|------|------:|----------|
| The file as uploaded, compressed if it is | 64,000,000 bytes (64 MB) | `UPLOAD_LIMIT` |
| The decompressed content | 1,000,000,000 bytes (1 GB) | `DECOMPRESSED_LIMIT` |

The Basetool rejects a file above either limit; `p4kx catalog` and the desktop window only warn,
from 90 % of a limit and again above it. The summary (`--json`: `size`, `compressed_size`,
`upload_limit`, `decompressed_limit`, `size_warning`, `decompressed_warning`) carries the
uncompressed size, the compressed size (`null` for plain JSON) and the warnings of both.

## Shape

The file is UTF-8 JSON in one line, ending in a newline. Field names are camelCase and mean the
same in every kind. Every field is always present; one that has no value is `null`.

| Field | Meaning |
|-------|---------|
| `schemaVersion` | The version of the shape, now `2`; an import rejects a version it does not know |
| `source.channel` | The Channel: the folder holding the Archive, e.g. `LIVE` |
| `source.build` | `branch`, `version`, `changeNumber` and `buildId` from the build manifest, or `null` |
| `source.build.publicPatch` | The public patch version, e.g. `4.7.2`, from the RSI Launcher log line `SC <CHANNEL> <version>-<tag>.<change>` (`<version>`: two or three dot-separated numbers, kept as logged) whose change equals `changeNumber`; `null` when the log has no such line (never guessed from a neighbouring one) |
| `source.archiveSize` | The size of the Archive file in bytes |
| `source.archiveModified` | The modification time of the Archive file, UTC in RFC 3339 form |
| `source.toolVersion` | The version of P4KX that wrote the catalog |
| `source.exportedAt` | When the catalog was written, UTC in RFC 3339 form |
| `completeness.<kind>` | `{"status": "complete"}`, or `{"status": "partial", "filter": "…"}` when a Filter selected the Records |
| `manufacturers` | The manufacturers, ordered by GUID |
| `items` | The items, ordered by GUID |
| `ships` | The ships and ground vehicles, ordered by GUID |
| `commodities` | The commodities, ordered by GUID |
| `blueprints` | The crafting blueprints with their ingredients, all tiers, default availability and dismantle link, ordered by GUID |
| `resourceGroups` | The groups of resource types, the targets of `typeGuid`, ordered by GUID |
| `blueprintCategories` | The blueprint categories, the targets of `categoryGuid`, ordered by GUID |
| `gameplayProperties` | The gameplay properties the quality modifiers refer to, ordered by GUID |
| `qualityDistributions` | The quality distributions and their location overrides, ordered by GUID |
| `qualityQuantizations` | The quantised quality steps per `ResourceType`, each linked to its `ResourceType`s by GUID, ordered by GUID |
| `blueprintPools` | The blueprint pools with the places that reward them, ordered by GUID |
| `dismantleProcesses` | The dismantle processes, ordered by GUID |
| `paints` | The paints, each the `itemGuid` of an entry of `items` with the ships it fits, ordered by `itemGuid` |
| `loadouts` | The ports of every ship and of every item that has ports, with the Entity the Default Loadout attaches to each, ordered by `entityGuid` |
| `starmapObjects` | The starmap objects (star systems, planets, moons, stations, jump points) with type, parent, name and world position, ordered by GUID |
| `contracts` | The contracts and sub contracts with resolved title, mission type, pickup places and the reputation granted on success, ordered by `guid` and then `subContractGuid` |
| `mineableElements` | The mineable elements with their instability and other plain numbers, each linked to a `ResourceType`, ordered by GUID |
| `refiningProcesses` | The refining processes (speed and quality pairs of a refinery), ordered by GUID |
| `mineableCompositions` | The elements a rock or deposit is made of, with the range and chance of each, ordered by GUID |
| `miningGlobalParams` | The mining parameters of each kind of miner (ship, ground vehicle, FPS), ordered by GUID |
| `harvestableProviderPresets` | What a place can spawn: groups of harvestables followed to their composition, and areas, ordered by GUID |
| `subHarvestableMultiConfigs` | What the slots of an object can hold by tag, ordered by GUID |
| `harvestableLocations` | The containers whose `.soc` names a provider preset, with the starmap object each hangs below, ordered by preset, container, system and object |
| `shops` | The shops of the shop registry with kind, brand, location and the link to a starmap object, ordered by `key` |
| `factionReputations` | The factions of the reputation system with their allies, enemies and relation standings, ordered by GUID |
| `reputationScopes` | The reputation scopes, each the ladder of standings one kind of reputation climbs, ordered by GUID |
| `reputationStandings` | The reputation standings, the ranks of the scopes with their thresholds, ordered by GUID |
| `reputationContexts` | The reputation contexts, the scopes a faction is shown with, ordered by GUID |
| `missionTypes` | The mission types, the targets of `contracts[].missionTypeGuid`, ordered by GUID |
| `missionLocalities` | The mission localities, named sets of starmap objects, ordered by GUID |
| `contractScenarios` | The contract scenarios with the reward tiers of each faction, ordered by GUID |
| `cargoManifests` | The cargo manifests (`CargoManifest`) with their fill and spawn rules, ordered by GUID |
| `missionLocationTemplates` | The mission location templates with the tags a location query matches, ordered by GUID |
| `contractShipSpawns` | The distinct ship spawn descriptions of the contracts, ordered by identifier |
| `contractLocationQueries` | The distinct location queries of the contracts, ordered by identifier |
| `contractTemplates` | The contract templates (`ContractTemplate`), the targets of `contracts[].templateGuid`, ordered by GUID |
| `missionBrokerEntries` | The mission broker entries (`MissionBrokerEntry`), the targets of `contracts[].brokerEntryGuid`, ordered by GUID |
| `contractNpcSpawns` | The distinct NPC spawn descriptions of the contracts, templates and broker entries, ordered by identifier |
| `contractEntitySpawns` | The distinct entity spawn descriptions of the contracts, templates and broker entries, ordered by identifier |
| `lootTables` | The loot tables (`LootTableV3Record`) with the weighted archetypes of each, ordered by GUID |
| `lootArchetypes` | The loot archetypes (`LootArchetypeV3Record`) with the weighted selectors of each, ordered by GUID |
| `lootPoolFilters` | The pool filters of the loot generation, ordered by GUID |
| `legacyLootTables` | The older loot tables (`LootTable`) with the weighted archetypes of each, ordered by GUID |
| `legacyLootArchetypes` | The older loot archetypes (`LootArchetype`) with the weighted tags of each, ordered by GUID |
| `lootContainers` | The containers placed with a multi config, with the starmap object each hangs below, ordered by config, container, system and object |
| `mineableEntities` | The mineable rocks and deposits (`EntityClassDefinition` under `entities/mineable`) with their composition, mining parameters and scan signature, ordered by GUID |
| `crafters` | The crafting stations (entities with a `CrafterComponentParams`) with the limits and accepted categories of their job queues, their crafting-time modifiers and the attach definition a port is matched against, ordered by GUID |
| `shipStoreSlugs` | The RSI store slugs that map to a ship of `ships`, with the retired ones flagged, ordered by `slug` (curated data, ADR-0117) |
| `craftingGlobal` | Not a kind: the global crafting parameters that apply to every dismantle |
| `itemRecovery` | Not a kind: the item recovery multipliers and rates behind claims, with the override groups of entity classes, `null` without the Record; see "Item recovery rules" below |

A complete kind lists every Record of its kind in the Build, so the import may treat a Record it
knows but no longer finds as removed. A partial kind lists only what the Filter selected.

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

Every entity carries:

| Field | Meaning |
|-------|---------|
| `guid` | The Record GUID as canonical lower-case UUID text; entities are ordered by it |
| `className` | The Record name without its type, e.g. `ACAS` for `SCItemManufacturer.ACAS` |
| `recordPath` | The record path, without `.xml` |
| `name`, `description` | `key` (the localisation key), `en` and `de` (the resolved texts), each `null` while unknown |

A resource group and a blueprint category carry `name` and no `description`.

A manufacturer adds `code`, e.g. `ACAS`.

An item takes its fields from the components of its `EntityClassDefinition`:

| Field | Source |
|-------|--------|
| `name`, `description` | `SAttachableComponentParams.AttachDef.Localization.Name` and `.Description` |
| `type`, `subType` | `AttachDef.Type` and `AttachDef.SubType`, e.g. `PowerPlant` and `Power` |
| `size`, `grade` | `AttachDef.Size` and `AttachDef.Grade`, integers; see "Semantics of `grade`" below |
| `mass` | `SEntityPhysicsControllerParams.PhysType.Mass`, a number |
| `manufacturerGuid` | The GUID the Reference `AttachDef.Manufacturer` links to, `null` for a null Reference |
| `tags` | The words of `AttachDef.Tags`, split at white space |
| `displayType` | A text: `SCItemPurchasableParams.displayType`, e.g. `@item_displayType_Pants` |
| `itemClass` | `Civilian`, `Competition`, `Industrial`, `Military` or `Stealth`, or `null` |
| `playability` | `player`, `mission` or `special`, or `null`; see "Playability" below |
| `variantOf` | The `guid` of the base of the item's variant group, `null` for a base and an ungrouped item; see "Variant groups" below |
| `stats` | The typed stat blocks of a weapon, shield, quantum drive, power plant, cooler, missile, suit armour, clothing, radar, tractor beam, attachment modifier, mining laser or mining module and gadget, `null` for any other item; see "Item stats" below |

`itemClass` takes the options of the DataForge enum `EItemClass` (without `UNDEFINED`). No
DataForge field carries it: it is read from the first line of the English `description` that
starts with `Class:`, its value trimmed of white space and matched ignoring case. Any other value
(FPS weapons use the line for the damage type, e.g. `Ballistic`), an item without that line and
an item without an English description leave it `null`. `p4kx catalog` counts the items with a
class, with another value, and without the line or an English description. The schema types the
field as a string; the five options are documented, not enumerated.

#### Semantics of `grade`

`grade` is the integer of `AttachDef.Grade`, exported unchanged; the catalog carries no letter.
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

The Basetool can take the letter from `grade` by `1 = A` to `4 = D` for the five component types,
and must treat grade 1 as unknown elsewhere and for those 19 items. It reads the integer, never the
`Grade:` line, which is a game text and stays away from third parties.

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

`stats` holds raw facts of the Archive and no derived value (ADR-0065): every number is the stored
field in the stored unit, `p4kx` computes no damage per second, no cycle time, no acceleration and
no fuel rate. The Basetool owns those formulas. `stats` is `null` unless the item has at least one
of the thirteen blocks; every block is `null` when the item does not have it, and a field inside a
present block is `null` when the Archive does not state it, never 0. None of the fields is a game
text. The additions are additive (ADR-0054); `kind` of a fire mode and
`trackingSignalType` are closed sets that the schema keeps as strings. The blocks `armor`,
`clothing`, `radar`, `tractorBeam` and `weaponModifier` and the fire-mode field `damagePerSecond`
follow ADR-0072; the blocks `miningLaser` and `miningModifier` and the fire-mode fields `hitType`,
`fullDamageRange` and `zeroDamageRange` follow ADR-0262.

| Block | Present when | Fields and sources |
|-------|--------------|--------------------|
| `weapon` | the item has `Components[SCItemWeaponComponentParams]` (ship weapons and FPS weapons) | `fireActions`: per element of `fireActions`, in stored order, with a sequence (`sequenceEntries`) flattened to its entries: `kind` (the struct, e.g. `SWeaponActionFireRapidParams`), `name`, `fireRate` (rounds per minute), `pelletCount` (as `pellets`; `pellets` where a Record states that instead) and `ammoCost` of `launchParams`, `heatPerShot`, `chargeTime`, `overchargeTime`, `cooldownTime` (each of the fire mode, when stated), and `damagePerSecond` (the damage types of the beam's `damagePerSecond`, the power of a mining laser; `null` for a mode without one), and `spread` (see below); a beam also has `hitType`, `fullDamageRange` and `zeroDamageRange` (the two ranges in metres); a parallel mode (`weaponActions`) is listed as its entries; a charged or wrapped mode reads `fireRate`, `launchParams` and `name` from its `weaponAction` when it has none of its own. `ammoCapacity`: `maxAmmoCount` of the ammo container, the magazine's capacity (see below). `ammo`: the `AmmoParams` Record that the container's `ammoParamsRecord` links to, `null` without a readable one: `speed`, `lifetime`, `damage` (the damage types of `projectileParams.damage`), `detonationDamage`, `detonationMinRadius` and `detonationMaxRadius` (from `explosionParams`) |
| `shield` | `Components[SCItemShieldGeneratorParams]` | `maxHealth` (`MaxShieldHealth`), `maxRegen` (`MaxShieldRegen`), `decayRatio` (`DecayRatio`), `downedRegenDelay` (`DownedRegenDelay`), `damagedRegenDelay` (`DamagedRegenDelay`) |
| `quantumDrive` | `Components[SCItemQuantumDriveParams]` | `fuelRequirement` (`quantumFuelRequirement`, as stored), `jumpRange`, `disconnectRange`, and of `params`: `driveSpeed`, `cooldownTime`, `spoolUpTime`, `stageOneAccelRate`, `stageTwoAccelRate`, `engageSpeed` |
| `powerPlant` | `AttachDef.Type` is `PowerPlant`, ignoring case | `powerOutput`: the `amount` of the first `generation` flow of `resourceStates` whose resource is `Power`, and `unit`: the `unit` of that flow |
| `cooler` | `AttachDef.Type` is `Cooler`, ignoring case | `coolingRate` and `unit`: as for the power plant, for the resource `Coolant` (a cooler's flow is an `ItemResourceDeltaConversion`) |
| `missile` | `Components[SCItemMissileParams]` | `damage` (the damage types of `explosionParams.damage`), `explosionMinRadius`, `explosionMaxRadius`, `trackingSignalType`, `lockTime`, `lockingAngle`, `lockRangeMin`, `lockRangeMax` (of `targetingParams`) and `linearSpeed` (of `GCSParams`) |
| `armor` | `Components[SCItemSuitArmorParams]` (armour pieces, 2,388 items on LIVE 4.10.2 out of 2,391 Records; the rest are development leftovers) | `integrityMilestoneToBreak`; `damageResistance`: the `damageResistance` struct of the `DamageResistanceMacro` Record that `damageResistance` links to, `null` without a readable one (a Record that is missing, has another struct or cannot be built): `ignoreMeleeDamage` (`IgnoreMeleeDamage`) and per damage type `physical`, `energy`, `distortion`, `thermal`, `biochemical`, `stun` (`PhysicalResistance` and so on) the entry `multiplier`, `threshold`, `damageCap`, `damageCapLimit` (`Multiplier`, `Threshold`, `DamageCap`, `DamageCapLimit`) |
| `clothing` | `Components[SCItemClothingParams]` (4,423 items on LIVE 4.10.2 out of 4,426 Records; armour carries this component too) | `temperatureResistanceMin` and `temperatureResistanceMax` (`TemperatureResistance.MinResistance` and `.MaxResistance`), `radiationCapacity` (`RadiationResistance.MaximumRadiationCapacity`), `radiationDissipationRate` (`RadiationResistance.RadiationDissipationRate`), `gForceResistance` (`Flight.gForceResistance`) |
| `radar` | `Components[SCItemRadarComponentParams]` | `aimAssistDistanceMin`, `aimAssistDistanceMax`, `aimAssistOutsideRangeBuffer` (`aimAssist.distanceMinAssignment`, `.distanceMaxAssignment`, `.outsideRangeBufferDistance`), `pingCooldownTime` (`pingProperties.cooldownTime`) |
| `tractorBeam` | the weapon component holds an `SWeaponActionFireTractorBeamParams` (ship tractor beams and salvage heads) | of the first such struct below `Components[SCItemWeaponComponentParams]`: `minForce`, `maxForce`, `minDistance`, `maxDistance`, `fullStrengthDistance`, `maxVolume`, `maxAngle` |
| `weaponModifier` | `Components[EntityComponentAttachableModifierParams]` holds an `SSalvageModifier` or an `STractorModifier` | `salvage`: `radiusMultiplier`, `salvageSpeedMultiplier`, `extractionEfficiency` of the first `SSalvageModifier`; `tractor`: `forceMultiplier`, `fullStrengthDistMultiplier`, `maxDistMultiplier`, `maxVolumeMultiplier` of the first `STractorModifier`; each `null` without that struct. Salvage modifiers and tractor attachments carry them in `modifiers[].weaponModifier.weaponStats` |
| `miningLaser` | `Components[SEntityComponentMiningLaserParams]` (ship mining lasers and the FPS mining multi-tool) | `isOptimalRateGood` (`miningLaserModifiers.isOptimalRateGood`), `throttleLerpSpeed`, `throttleMinimum` (a fraction of 1), `usesPowerThrottle`, `globalParamsGuid` (the `MiningLaserGlobalParams` Record `globalParams` links to), `filterModifier` (`filterParams.filterModifier.value`) and `modifiers` (the rest of `miningLaserModifiers`): each the `value` of a `FloatModifierMultiplicative` in percent as stored and `null` when unset, `instability` (`laserInstability`), `optimalChargeWindowSize`, `resistance`, `shatterDamage`, `clusterFactor`, `optimalChargeWindowRate`, `catastrophicChargeWindowRate`. The beams are fire modes of `weapon.fireActions`: the mining beam has `hitType` `ElectricArc`, the extraction beam `Extraction`; their ranges are in metres |
| `miningModifier` | `Components[EntityComponentAttachableModifierParams]` holds an `ItemMiningModifierParams`, an `ItemMineableRockModifierParams` or a `MiningFilterItemModifierParams` (mining modules and gadgets) | `activationMethod` (`ActivateOnAttach` for a passive module, `ActivateOnDemand` for an active module or a gadget), `charges` (-1 for a gadget that is not used up), `canInterrupt`, `isInterruptible`; `laser`: the first `ItemMiningModifierParams` as `lifetime` (`modifierLifetime.lifetime`, seconds, `null` for a modifier that lasts) and `isOptimalRateGood` and `modifiers` (the values of `MiningLaserModifier`, as for the laser); `rock`: the same for the first `ItemMineableRockModifierParams` (gadgets); `filter`: `lifetime` and `filterModifier` of the first `MiningFilterItemModifierParams`; `fireActionModifiers`: per `ItemWeaponModifiersParams` in stored order, `fireActionIndex`, `lifetime` and `damageMultiplier` (`weaponModifier.weaponStats.damageMultiplier`); each of `laser`, `rock` and `filter` is `null` without that struct |

##### From a gameplay property to a stat field

The Archive holds no link from a `CraftingGameplayPropertyDef` to the item field it changes: the
Record has a name (`propertyName`), a unit format and a display transformation, nothing that names a
field (ADR-0072). The catalog therefore states no link per entry of `gameplayProperties`; the
consumer joins a modifier's `propertyGuid` to the `className` of that entry and takes the stat
field from this table. The table covers the 29 properties of LIVE 4.10.2; `real_archive_gear_stats`
checks that every `className` of `gameplayProperties` has a row. A multiplier-kind modifier scales
the base value of the field, an additive one adds to it.

| `className` | `stats` field the modifier changes | Items |
|-------------|------------------------------------|-------|
| `GPP_Armor_DamageMitigation` | `armor.damageResistance.<type>.multiplier` for all six types; the SC Wiki's physical value is `physical.multiplier` | armour |
| `GPP_Armor_TemperatureMin` | `clothing.temperatureResistanceMin` | armour, clothing |
| `GPP_Armor_TemperatureMax` | `clothing.temperatureResistanceMax` | armour, clothing |
| `GPP_Armor_RadiationCapacity` | `clothing.radiationCapacity` | armour, clothing |
| `GPP_Armor_RadiationDissipation` | `clothing.radiationDissipationRate` | armour, clothing |
| `GPP_Health_MaxHealth` | `health` | ship components, weapons |
| `GPP_Weapon_Damage` | `weapon.ammo.damage` (guns), `weapon.fireActions[].damagePerSecond` (mining lasers) | weapons |
| `GPP_Weapon_FireRate` | `weapon.fireActions[].fireRate` | personal weapons |
| `GPP_Weapon_Recoil_Kick` | none: see below | personal weapons |
| `GPP_Weapon_Recoil_Handling` | none: see below | personal weapons |
| `GPP_Weapon_Recoil_Smoothness` | none: see below | personal weapons |
| `GPP_Weapon_ReloadSpeed` | none: see below | personal weapons |
| `GPP_Weapon_Spread` | `weapon.fireActions[].spread` | personal weapons |
| `GPP_ItemResource_PowerGeneration` | `powerPlant.powerOutput` | power plants |
| `GPP_ItemResource_CoolantGeneration` | `cooler.coolingRate` | coolers |
| `GPP_Shield_MaxHealth` | `shield.maxHealth` | shields |
| `GPP_Radar_MinAimAssistDistance` | `radar.aimAssistDistanceMin` | radars |
| `GPP_Radar_MaxAimAssistDistance` | `radar.aimAssistDistanceMax` | radars |
| `GPP_Quantum_Speed` | `quantumDrive.driveSpeed` | quantum drives |
| `GPP_Quantum_FuelRequirement` | `quantumDrive.fuelRequirement` | quantum drives |
| `GPP_Weapon_Tractor_Force` | `tractorBeam.maxForce` | tractor beams, salvage heads |
| `GPP_Weapon_Tractor_MaxDist` | `tractorBeam.maxDistance` | tractor beams, salvage heads |
| `GPP_Weapon_Tractor_FullStrengthDist` | `tractorBeam.fullStrengthDistance` | tractor beams, salvage heads |
| `GPP_Weapon_Tractor_MaxVolume` | `tractorBeam.maxVolume` | tractor beams, salvage heads |
| `GPP_Weapon_HullScraping_Radius` | `weaponModifier.salvage.radiusMultiplier` | salvage modifiers |
| `GPP_Weapon_HullScraping_Speed` | `weaponModifier.salvage.salvageSpeedMultiplier` | salvage modifiers |
| `GPP_Weapon_HullScraping_Efficiency` | `weaponModifier.salvage.extractionEfficiency` | salvage modifiers |
| `GPP_Crafter_CraftSpeed` | none: a property of a crafting station, not of an item | crafting stations |
| `GPP_Crafter_DismantleEfficiency` | none: a property of a crafting station, not of an item | crafting stations |

Properties with none: the three recoil properties and the reload speed have no single stat field.
A personal weapon's recoil is the weapon's `actorProceduralRecoilConfig` and the `recoil` of its
fire modes, Records with curves per stance and aim state, and its reload times are in further
Records; reducing them to one number is a model of the game, which ADR-0065 leaves to the
consumer. The SC Wiki has no field for them either. The two crafter properties modify the station
that crafts, which is not an item with `stats`. The spread has a field, `spread` of a fire mode
(`launchParams.spreadParams`, or `spreadParams` of a beam): `min`, `max`, `firstAttack`, `attack`
and `decay` as stored. No LIVE blueprint modifies the spread or the reload speed.

`GPP_Armor_DamageMitigation` is one factor; the Archive does not say which damage types it
scales, so the catalog gives the `multiplier` of all six (`physical`, `energy`, `distortion`,
`thermal`, `biochemical`, `stun`) and the SC Wiki's `physical` is `physical.multiplier`.

Items without the source component: some produced items carry no field for a property their
blueprint modifies, and the catalog states `null` for the block rather than a default. On LIVE
4.10.2 these are the backpacks, which have no `SCItemSuitArmorParams`, `SCItemClothingParams` or
health (16 items for the damage mitigation, 7 for the temperatures, 1 for the radiation
dissipation), `hdtc_shoes_01_01_01`, a piece of clothing with `clothing` but no `armor`, for the
damage mitigation, the 8 fuel nozzles for `GPP_Health_MaxHealth` (no health component) and the
personal weapons that only fire a beam (`none_smg_energy_01`) for `GPP_Weapon_FireRate` (no
`fireRate`). The clothing pieces of the issue's 1,346 mitigation targets are these items: where
`armor` is `null` the Archive holds no damage resistance for the piece, and a consumer treats the
base value as unstated. `real_archive_gear_trace` checks every modifier of every blueprint
against the produced item and prints these lists.

`resourceStates` lists the states of `Components[ItemResourceComponentParams].states` in stored
order: `name`, `emSignature` and `irSignature` (`signatureParams`: `nominal` is `nominalSignature`,
`decayRate`), and `flows`. A flow is one `generation` or `consumption` of a delta of the state:
`kind` (the struct of the delta, e.g. `ItemResourceDeltaGeneration`, `ItemResourceDeltaConversion`,
`ItemResourceDeltaConsumption`, `ItemResourceDeltaStorage`), `direction`, `resource`, `amount` (the
first number of `resourceAmountPerSecond`) and `unit` (the struct holding it:
`SPowerSegmentResourceUnit`, `SStandardResourceUnit`, `SMicroResourceUnit`, `SCentiResourceUnit`).
Units differ between structs and are not converted. A delta without a `generation` or
`consumption` (a network reflection) gives no flow.

Raw values that need care:

- `quantumDrive.jumpRange` is `f32::MAX` (3.4028235e38) on a drive the Archive leaves unbounded (a
  development drive of the Idris). It is exported as stored; a reader treats a value at that
  magnitude as unlimited.
- A number that is not finite (NaN, infinity) is `null`, never written.
- `weapon.ammoCapacity` is the capacity of the ammo container, which is the magazine and not the
  weapon body: a ship weapon holds `Components[SAmmoContainerComponentParams]` itself; a weapon
  without one (the personal weapons, ballistic and energy alike) links its magazine entity by
  `Components[SCItemWeaponComponentParams].ammoContainerRecord`, and the container of that entity,
  one hop, is read instead. `weapon.ammo` is read from the `AmmoParams` Record of that container's
  `ammoParamsRecord`. A Reference to a missing Record, to a Record of another struct, or to a
  Record that cannot be built leaves `ammo` `null`; the item stays in the catalog.
- The fire-mode fields `name`, `fireRate`, `heatPerShot`, `chargeTime`, `overchargeTime`,
  `cooldownTime` and `launchParams` are read from the fire mode and, where it has none of its own,
  from its `weaponAction`.

A damage object has `physical`, `energy`, `distortion`, `thermal`, `biochemical` and `stun`
(`DamagePhysical` and so on); a type the struct does not hold is `null`. An item with at least one
of the six blocks also carries `health` (`SHealthComponentParams.Health`) and `resourceStates`;
the signature and power draw inputs are in `resourceStates`, not in separate blocks. Items without a
block carry neither.

The block of a power plant or a cooler follows from the type because both share the resource
component; every other block follows from its component, and no class name or record name enters
a rule. A damage object is read from the first struct below the named parent that states a damage
type, and a generation from the structs of its type below the resource component, so a field the
game moves turns into `null` rather than into a wrong number; the real-Archive test
(`real_archive_item_stats`) counts the filled fields of each block.

Size: an item without stats adds `"stats":null` (14 bytes, about 0.3 MB for the 23,280 items of
LIVE); an item with a block adds a few hundred bytes, two to three thousand items about 1 MB,
which compresses well because the keys repeat.
The mining blocks and the three beam fields of every fire mode added 0.35 MB (349,210 bytes) to the
plain catalog of LIVE 4.10.2.

A ship takes its fields from `VehicleComponentParams`:

| Field | Source |
|-------|--------|
| `name`, `description` | `vehicleName` and `vehicleDescription` |
| `role`, `career` | Texts: `vehicleRole` and `vehicleCareer`, e.g. `@vehicle_class_lightfreight` |
| `crewSize` | `crewSize`, an integer |
| `manufacturerGuid` | The GUID the Reference `manufacturer` links to, `null` for a null Reference |
| `vehicleDefinition` | `vehicleDefinition`, the path of the vehicle XML |
| `playability` | `player`, `ai`, `mission` or `special`, or `null`; see "Playability" below |
| `variantOf` | The `guid` of the base of the ship's variant group, `null` for a base and an ungrouped ship; see "Variant groups" below |
| `vehicleKind` | `spaceship` or `groundVehicle`, or `null`; see "Ship base data" below |
| `size` | `Components/SAttachableComponentParams/AttachDef/Size`, an integer, `null` without an `AttachDef` |
| `mass` | The hull mass in kilograms; `null` when unknown, never 0; see "Ship base data" below |
| `massBasis` | What `mass` is the sum of: `parts` or `physics`; `null` exactly when `mass` is |
| `hullHealth` | The hull hit points, `null` when unknown; see "Ship base data" below |
| `cargoScu` | The cargo capacity in SCU: 0 without a cargo grid, `null` when unknown; see "Ship base data" below |
| `cargoGridsWithIssue` | How many cargo grids have no readable capacity; above 0, `cargoScu` is a lower bound |
| `insurance` | The claim and expedite values of the ship's `SEntityInsuranceProperties`, `null` when the Record has none; see "Ship insurance" below |

#### Ship insurance

The insurance data is part of the ship Record (ADR-0218, #323): the `SEntityInsuranceProperties`
struct in the `StaticEntityClassData` of the `EntityClassDefinition`, so the source path of a
ship's insurance is the ship's `recordPath`. There is no `ShipInsuranceRecord` in the Archive any
more (LIVE); the field is a fact only, no game text, and it is additive within version 2.

| Field of `insurance` | Source |
|----------------------|--------|
| `restoreOnClaim` | `restoreOnClaim`, a boolean (inferred meaning: a claim restores the ship with its Loadout) |
| `shipEntityClassName` | `shipInsuranceParams.shipEntityClassName`, e.g. `AEGS_Gladius`; `null` when empty |
| `baseWaitTimeMinutes` | `shipInsuranceParams.baseWaitTimeMinutes`, in minutes (inferred meaning: the claim time) |
| `mandatoryWaitTimeMinutes` | `shipInsuranceParams.mandatoryWaitTimeMinutes`, in minutes (inferred meaning: the shortest claim time) |
| `expediteFee` | `shipInsuranceParams.baseExpeditingFee` (renamed), in aUEC as stored |

The meanings marked inferred come from the field names; the Archive states none. Every field is
`null` when the Record states none, including every field of a ship whose
`SEntityInsuranceProperties` has no `shipInsuranceParams`. The struct has exactly these fields: two
times, one fee, `restoreOnClaim`, the class name and the `displayParams`. The numbers are those of
the Record, unchanged; the Basetool applies any scaling or rule of its own. The `displayParams`
(name, role, career, crew size, manufacturer) repeat the ship's own fields, are mostly
placeholders, and are not exported.

`expediteFee` is the only fee in aUEC the Archive holds for a ship; the multipliers that relate to
claims are in `itemRecovery` (see "Item recovery rules"), which also states what the data does and
does not say about a claim fee.

On LIVE all 1,066 catalog ships carry `insurance`, each with a fee;
AEGS_Gladius has a `baseWaitTimeMinutes` of 5.83, a `mandatoryWaitTimeMinutes` of 1.94 and an
`expediteFee` of 2,335 (real-Archive test `real_archive_insurance`).

#### Ship base data

`mass`, `massBasis`, `hullHealth`, `cargoScu`, `cargoGridsWithIssue`, `size` and `vehicleKind` are
facts only (no game text) and come from the ship base data of `p4kx ships`
([ship-base-data.md](ship-base-data.md), ADR-0061): the catalog build reads each ship once with
`p4kx_export::ShipReader` in the same pass that reads the other ship fields, so the ship base walk
runs once, not twice. Every value is `null` when the Archive does not state it, and a reader never
sees 0 for unknown. The exports agree value for value; the catalog leaves out the per-value
sources (file and field), the thresholds, the Loadout and the cells, which stay in `p4kx ships`.
The shapes are additive within version 1 (ADR-0054, ADR-0063); `vehicleKind` and `massBasis` are
closed sets that the schema keeps as strings.

| Field | Rule and source |
|-------|-----------------|
| `mass` | The sum of the `mass` of every part of the vehicle implementation (`vehicleDefinition`) after the ship's Modification is applied, rounded to four decimals; a sum too large to round to a finite number counts as unknown (so does `hullHealth`). Landing gear, Loadout items and docked vehicles are not part of it (#305). Without part masses, the mass of the rigid physics controller (`Components/SEntityPhysicsControllerParams/PhysType/Mass`, `massBasis` `physics`) |
| `hullHealth` | The sum of the positive `damageMax` of the parts, except parts holding an item port flagged `uneditable` or `invisible`; `null` without a vehicle implementation or when no part states one |
| `cargoScu` | The sum of the capacity of the `CargoGrid` Entities of the default Loadout (docked vehicles and templates are not entered): an open container is `x * y * z / 1.953125` of its `interiorDimensions`, a closed one its `capacity` in standard cargo units. 0 for a ship with no grid, `null` when it has grids and none is readable |
| `cargoGridsWithIssue` | The grids whose capacity cannot be read, e.g. a dangling Reference to the `InventoryContainer`. Above 0, `cargoScu` counts only the readable grids and is a lower bound |
| `vehicleKind` | `groundVehicle` for a Record under `entities/groundvehicles`, `spaceship` for one under `entities/spaceships` whose vehicle implementation has no wheel (`SubPartWheel`) or tread (`Tread`) part; a missing or unreadable vehicle implementation counts as no wheels, so such a Record under `entities/spaceships` is a `spaceship`. `null` when the evidence disagrees (a wheeled or tracked Record under `entities/spaceships`) or the path names neither folder |

LIVE anchors, read by the real-Archive test: Harbinger 230,404 kg, 48,490 hull, 0 SCU; Freelancer
MAX 341,408 kg, 35,650 hull, 120 SCU; Carrack 3,275,858 kg, 88,000 hull, 456 SCU; Constellation
Andromeda 386,857 kg, 180,844 hull, 96 SCU (without the docked snub craft). The Javelin and the
Hornet F7C have `cargoScu` `null`: their cargo grids refer to `InventoryContainer` Records that do
not exist.

A ship with `cargoGridsWithIssue` above 0 and a `cargoScu` number may carry more than the number
says.

On LIVE the 1,066 catalog ships split into 1,027 `spaceship` and 39 `groundVehicle`, none `null`:
no Record under `entities/spaceships` has a wheel or tread. The folder `entities/groundvehicles`
holds 40 Records; the 40th, `RSI_Ursa_Rover_Unmanned_Template`, is left out of the catalog as a
development leftover (the whole word `template`, see "Selection").

**Minimum crew.** The game has none. A survey of the field names of all 1,144 ship Records on LIVE
finds only `crewSize` (on `VehicleComponentParams` and on `StaticEntityClassData/displayParams`) and
`crewManifest` (the NPC crew of AI variants, used by `playability`). No field named minimum, seat
or crew minimum exists, so `crewSize` stays the only crew value.

**Outer dimensions.** No field states the outer size of a ship in metres. The survey finds the
item-inventory fields `AttachDef/inventoryOccupancyDimensions` and
`inventoryOccupancyLocalBoundsMin` / `Max` on every ship Record, plus `maxBoundingBoxSize` and
`hasLocalBounds`. They describe how a ship takes up inventory space, not its hull, and their values
for ships are not verified, so the catalog exports no dimensions.

#### Playability

The catalog lists every ship and item Record of the Build, among them AI, mission and event
variants that no player can own, buy or fit. `playability` tells them apart. It is a closed set of
strings; the schema keeps a string, so a new value is additive and a reader maps a value it does
not know to `null` (ADR-0054). It is `null` where no rule below applies: the value is never
guessed. The rules read Record data only (components, fields, References, Tags, record-path
folders); no class name or Record name enters them.

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
`DoNotInclude` leads to `null`, unless rule 1 or 2 holds.

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
5. Also `null` when the crew manifest or a tag the Record links to could not be read (the
   catalog reports that Record as a failure), since its evidence is missing; checked before
   rule 1.
6. Otherwise `null`: no `EAEntityDataParams`, or another `inclusionMode`.

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

**The Basetool seeds only `player` ships and items, and treats them as candidates.** `player`
means the Records carry the structural markers of a player entity (ships: hangar-ready inclusion
mode and no AI marker; items: a shop display name). It is not proof that a player can obtain
it: items carry no AI marker. The Basetool
matches every other value and `null` against what it knows and seeds none of them.

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

A commodity is an `EntityClassDefinition` under `entities/commodities` or a `ResourceType` Record that
a cost of any tier or a kept quality quantization uses and no commodity Record carries (see below). A commodity Record takes its fields
from the entity:

| Field | Source |
|-------|--------|
| `name` | `SCItemPurchasableParams.displayName` |
| `description` | `EntityUIDisplayParams.displayDescription` in `StaticEntityClassData` |
| `typeGuid` | The GUID of the `ResourceTypeGroup` Record that directly lists the `ResourceType` in its `resources` (19 groups on LIVE: Metal, Mineral, Gas, …), the `guid` of a `resourceGroups` entry; `null` without a `ResourceType` or a group |
| `subtypeGuid` | The GUID of the `ResourceType` Record that the first entry of `ResourceContainer.defaultComposition` links to; `null` without one, e.g. `Special_HolidayLetter` |
| `displayType` | A text: `SCItemPurchasableParams.displayType`, e.g. `@items_commodities_type_metal` |
| `densityGramsPerCubicCentimetre` | The number `densityType.densityUnit.gramsPerCubicCentimeter` of the `ResourceType` holds (a fact, no game text); `null` without a `ResourceType`, a density or a density in another unit |
| `refinedSubtypeGuid` | The GUID `refinedVersion` of the `ResourceType` links to; `null` without a `ResourceType` or a `refinedVersion`. It equals the `subtypeGuid` of the refined commodity only when the catalog has that commodity |

The refining fields are described in [commodity-refining.md](commodity-refining.md), with what the
DataForge does not hold (instability per commodity, yields, counting in SCU or units). The refined
`ResourceType` may have no commodity of its own; then `refinedSubtypeGuid` matches no `subtypeGuid`.

A name or description that is missing or a placeholder key is taken from the `ResourceType` Record
`subtypeGuid` links to (`displayName`, `description`) instead. The game holds no field named type
or subtype for a commodity; the groups of `ResourceType` Records are the types, the `ResourceType`
is the subtype.

A `ResourceType` Record that the costs of a blueprint use in any tier (LIVE has one tier per
blueprint: Aslarite, Ouratite, Iron, Stileron, Lindinium, Riccite, Tin, Torite, Savrilium and
Pressurized Ice), selections opened, or that a kept quality quantization links to
(`resourceTypeGuids`), and that no commodity Record carries is listed as a commodity as well, so
that every resource of a cost and every `resourceTypeGuids` entry points at a catalog entry. A
development leftover `ResourceType` never links to a quantization, so none is backfilled for one.
These entries fall under the game-text policy above like every commodity: `name` is a short name,
`description` is a game text (`x-gameText`). Its `guid` is the GUID of the `ResourceType`, `className`, `recordPath`, `name`
and `description` are those of the `ResourceType`, `typeGuid` is its group, `subtypeGuid` is its own
GUID and `displayType` has no key. With a `--filter`, such an entry is listed only when its record
path matches.

The join from an ingredient to its commodity is `resourceGuid` = `subtypeGuid`. A `ResourceType`
that a commodity Record carries yields no second entry, so every `resourceGuid` of the catalog
matches exactly one commodity (checked against LIVE). A `ResourceType` that several commodity
Records share (on LIVE the Slam variants and the med pens) matches several; no recipe uses one.

A blueprint (`CraftingBlueprintRecord`) takes its fields from `blueprint`:

| Field | Source |
|-------|--------|
| `className`, `key` | The Record name without its type, e.g. `BP_CRAFT_behr_lmg_ballistic_01_mag`; both hold it |
| `name` | `blueprintName` |
| `categoryGuid` | The GUID `category` links to, the `guid` of a `blueprintCategories` entry; `null` without a category or when that Record is a left-out development leftover or unreadable |
| `producedItemGuid` | The GUID `processSpecificData.entityClass` links to; `null` for a Reference to a Record the DataForge lacks |
| `craftTimeSeconds` | `costs.craftTime` of the first tier (`days`, `hours`, `minutes`, `seconds`) in whole seconds, rounded |
| `ingredients` | The costs of the first tier, see below |
| `tiers` | Every tier in stored order, see "Tiers" below; the first one is the tier `craftTimeSeconds` and `ingredients` describe |
| `defaultAvailable` | `true` when the blueprint is in `CraftingGlobalParams.defaultBlueprintSelection`, the blueprints a player has by default |
| `dismantle` | What dismantling the produced item returns, see "Dismantle returns" below; `null` without one |

`craftTimeSeconds` and `ingredients` keep their meaning; they are derived from the first tier, so
the cost tree is read once. A blueprint with a `CraftingCost_Select` nested deeper than 32 levels is
not cut off silently: it is left out of `blueprints` and counted as a failure (printed on stderr by
`p4kx catalog`, under `failed` in `--json` and in the summary; the catalog file has no such field).
`p4kx catalog` then exits with code 1, where an earlier build wrote the cut tree silently. No
blueprint of LIVE is that deep, so no catalog of LIVE changes (a change of the selection, part of
version 2).

An ingredient is one `CraftingCost_Resource` or `CraftingCost_Item` leaf of `mandatoryCost` and
`optionalCosts`. Nested `CraftingCost_Select` costs are flattened depth first in stored order:

| Field | Meaning |
|-------|---------|
| `resourceGuid` | The GUID of the `ResourceType` Record of a resource line, otherwise `null` |
| `itemGuid` | The GUID of the entity of an item line, otherwise `null` |
| `quantityScu` | `quantity.standardCargoUnits` of a resource line, otherwise `null` |
| `quantityUnits` | `quantity` of an item line, otherwise `null` |
| `minQuality` | `minQuality` |
| `slot`, `slotKey` | `debugName` and `displayName` of the nearest enclosing selection that has a name; a placeholder `slotKey` is `null` |

A resource group (`ResourceTypeGroup`, 19 on LIVE: Metal, Mineral, Gas, …) is the type of a
commodity; a commodity's `typeGuid` is the `guid` of the group that lists its `ResourceType`. It
takes its fields from the Record:

| Field | Source |
|-------|--------|
| `className` | The Record name without its type, e.g. `Metal` |
| `name` | `displayName`, e.g. `@items_commodities_type_metal`; a placeholder key (`@LOC_PLACEHOLDER`, 7 groups on LIVE) leaves `en` and `de` `null`, and the Basetool falls back to `className` |
| `parentGuid` | The GUID of the group that lists this group in its `groups`, the `guid` of another `resourceGroups` entry; `null` for a top-level group; the first group in Record order wins when several list it. Only the groups of the kind list: a group left out as a development leftover or unreadable names no parent and no `ResourceType` |

The groups form a forest, and `parentGuid` is the direct parent only. The import must not assume
a fixed depth and must guard against a cycle: the catalog exports the lists as the game stores
them, so a group that lists itself has no parent and two groups that list each other (A lists B, B
lists A) are each other's parent, with no root.

Groups nest one level on LIVE: Metal lists Alloy and UnrefinedOres, Mineral lists Raw_Minerals,
Nonmetal lists Halogen, Organic lists Food, ProcessedGoods lists SyntheticMaterials, Vice and
Bulk_Supplies. A commodity points at the group that lists its `ResourceType` directly, so a
commodity of Alloy has the child as `typeGuid`, not Metal. `description`, the thumbnail paths and
the lists `resources` and `groups` are not exported: the descriptions are game texts, the paths are
media, and the membership is `typeGuid` of the commodities. All groups share one `recordPath`, the
Record file `resourcetypedatabase`; `className` and `guid` tell them apart.

A blueprint category (`BlueprintCategoryRecord`, 21 on LIVE) is the target of the `categoryGuid` of
a blueprint. The Record holds no field at all, so the category has `guid`, `className` (e.g.
`FPSArmours`, `VehicleComponentS2`), `recordPath` and a `name` without a key, which stays `null`
until the game adds one; the game localises no category. Categories do not nest: the Record has no
parent, and the `BlueprintCategoryDatabaseRecord` beside them only lists them. Some categories
carry no blueprint on LIVE (`FuseBattery`, `Medical`, `Utility`, and the examples `Example1`,
`Example2`, `DismantleExample1` and `RefiningExample1`); they stay, as the kind is complete by
Record.

Every `typeGuid` of a commodity and every `categoryGuid` of a blueprint resolves to a `guid` of
`resourceGroups` or `blueprintCategories` (checked against LIVE; a commodity without a
`ResourceType` or group, 1 on LIVE, and a blueprint without a category or whose category is a
left-out leftover or unreadable have `null`; a commodity whose `ResourceType` only a leftover or
unreadable group lists has a `null` `typeGuid`). The groups are found whatever the `--filter`; a
category is only found missing when the `--filter` selects it, so under a `--filter` a
`categoryGuid` may still name a leftover category outside it. With a
`--filter`, a kind is partial and the references need not resolve.

### Tiers

`tiers` lists every `CraftingBlueprintTier` in stored order. Every tier has:

| Field | Meaning |
|-------|---------|
| `craftTimeSeconds` | `costs.craftTime` of the tier in whole seconds, rounded; `null` without one |
| `mandatory` | The mandatory cost as a cost line, `null` without one |
| `optional` | The optional costs as cost lines in stored order |

The stored `research` of a tier (`CraftingResearch` with `researchCosts` and `unlockRequirements`,
both strong pointers) is not exported: on LIVE 4.10 it exists on 926 of the 1,618 tiers and both
pointers are empty on every one of them, so the Archive shows neither the type nor a field of what
they would point to. A field typed without ever seeing a value would be a guess, and the type could
not change later (ADR-0054). The field joins the tier as an additive change once a Build fills it
(ADR-0243).

A cost line is a selection, a resource line or an item line. The tree keeps what `ingredients`
flattens, so a selection stays a selection:

| Field | Meaning |
|-------|---------|
| `kind` | `select`, `resource` or `item`. A closed set the game may extend; the schema keeps it a string and a reader maps an unknown value to nothing |
| `count` | `select`: how many options the player chooses ("choose n"); fewer than `options` offers means the player may leave options out (3 of the 1,618 LIVE blueprints) |
| `slot`, `slotKey` | `select`: `debugName` and `displayName` of `nameInfo`; a placeholder `slotKey` is `null` |
| `options` | `select`: the options as cost lines in stored order, otherwise `[]` |
| `resourceGuid`, `quantityScu` | `resource`: the `ResourceType` Record and the quantity in SCU, before the quantity multiplier |
| `itemGuid`, `quantityUnits` | `item`: the entity and the number of units, before the quantity multiplier |
| `minQuality` | `resource` and `item`: `minQuality` |
| `quantityMultiplier` | The factor of `CraftingCostContext_QuantityMultiplier` (1.15 on 3 lines of LIVE), `null` without one. The quantities stay the stored ones; the consumer applies the factor |
| `compositionInclusion` | The option of `CraftingCostContext_ResultCompositionInclusion`, e.g. `Include`; `null` without one |
| `modifiers` | `CraftingCostContext_ResultGameplayPropertyModifiers`: per modifier `propertyGuid` (a `gameplayProperties` entry) and `ranges`, each with `kind` (`multiplier` for factors, `additive` for summands; a closed set the game may extend), `startQuality`, `endQuality`, `atStart` and `atEnd`, linear in the quality between the two ends |

A selection carries the context of the selection itself in the same fields.

LIVE holds exactly one tier per blueprint on all five Channels, so the real-Archive test checks
that tiers and first-tier fields agree, and the multi-tier case is covered by a synthetic
Archive (`catalog_crafting_tiers`), which checks a blueprint with two tiers, selections,
multipliers and modifiers.

### Quality, pools and dismantle

Five kinds hold what a blueprint or a resource refers to; each entry has `guid`, `className` and
`recordPath`, and the ordering, completeness and leftover rules of every other kind apply (a `--filter` matches
the record path and marks the kind partial).

| Kind | Fields |
|------|--------|
| `gameplayProperties` | `name`: the localisation key `propertyName` and its texts; a placeholder key stays as `key` and leaves `en` and `de` `null`, as for `items` and `blueprints`. The target of a modifier's `propertyGuid` |
| `qualityDistributions` | `distribution`: a normal distribution `kind`, `min`, `max`, `mean`, `stddev` (`null` for an override Record or a value beyond a 32-bit float), and `overrides`: per location a `locationGuid` and its `distribution` (empty for a default Record). Both Record types of `crafting/qualitydistribution` are listed |
| `qualityQuantizations` | `bands` and `resourceTypeGuids`, described under "Quality quantizations" below |
| `blueprintPools` | `entries`: per entry `blueprintGuid` and `weight`; `totalWeight`, the sum of the positive weights; `rewardSources`: every struct of any Record with a Reference to the pool (on LIVE the `BlueprintRewards` result of a contract) with `recordGuid`, `recordPath`, `contractId`, `contractName` (both from the nearest enclosing struct that has an `id`) and `chance`, ordered by `recordGuid` and then in stored order. A `blueprintGuid` may point at a blueprint that is left out of `blueprints` as a failure |
| `dismantleProcesses` | `efficiency` (0.5 on LIVE) and `timeSeconds` (15 on LIVE) of the `GenericCraftingProcess_Dismantle` |

A blueprint's chance from a source is `chance` x the entry's `weight` / `totalWeight`; a source
without a chance or with one outside 0 to 1, a pool without a positive weight and an entry with a
weight of zero or less give none. The quality distributions carry no link to a blueprint or source:
the Records do not hold one, so the catalog lists them as they are.

#### Quality quantizations

`qualityQuantizations` lists the `CraftingQualityQuantizationRecord` Records under
`crafting/qualityquantization` (63 on LIVE, 62 after the `Quantization_TEMPLATE` leftover): the
quantised quality steps of a resource (ADR-0177). Each entry has `guid`, `className`, `recordPath`
and

| Field | Meaning |
|-------|---------|
| `bands` | `qualityQuantization.bands` in stored order: per band `start`, `end` (inclusive) and `mappedValue`, whole numbers. Every quality from `start` to `end` counts as `mappedValue`. LIVE has 8 bands per Record with the same ranges (0-399, 400-599, 600-699, 700-799, 800-899, 900-949, 950-998, 999-1000) that together cover the qualities 0 to 1000; only the `mappedValue`s differ per resource |
| `resourceTypeGuids` | The GUIDs of the `ResourceType` Records whose crafting data link to the Record, without repeats and ordered by GUID. The link goes from the `ResourceType` (`properties[].qualityQuantization.qualityQuantizationRecord`); the quantization Record holds none. On LIVE 43 links reach 42 Records (`Carinite` and `CarinitePure` share one); every linked `ResourceType` is the `subtypeGuid` of a commodity of the catalog, because a `ResourceType` a kept quantization links to is backfilled as a commodity (ADR-0184); with a `--filter`, only those whose record path matches |

The links are read from every `ResourceType` Record whatever the Filter, except those that are
development leftovers: such a `ResourceType` neither links nor counts. A Record no `ResourceType`
uses is ordinary game data and stays in the kind with an empty `resourceTypeGuids` (20 on LIVE);
`CatalogSummary::quantizations().unused` and `quantizations.unused` of `p4kx catalog --json` count
them, with no effect on the exit code. The same summary counts `unresolved_links`, the links to a
Record that is no readable quantization of the catalog (another type, a failed Record or a
leftover; a Reference to a GUID no Record has reads as null); it is 0 on LIVE. A Record without bands, or with a band that lacks a whole
`start`, `end` or `mappedValue` or ends before it starts, cannot be read: it is left out and
listed in the failures. No description, text or media is exported.

#### Dismantle returns

`dismantle` of a blueprint is `{"processGuid", "tierIndex"}`: the dismantle process that applies to
the item the blueprint produces and the index into `tiers` of the tier whose costs the item
returns, `null` for a blueprint without a tier. `craftingGlobal` holds
`dismantleExcludedResourceGuids` (the `ResourceType` Records a dismantle never returns) and
`dismantleExcludedItemGuids` (the entities that cannot be dismantled; their blueprints have no
`dismantle`). The contract:

- The Build must hold exactly one dismantle process. With several, no blueprint gets a
  `dismantle`, the processes are still listed, and `p4kx catalog` warns and exits with code 1.
- The item returns the share `efficiency` of the costs of the first tier (`tierIndex` 0), except
  the resources in `dismantleExcludedResourceGuids`.
- A `select` is not resolved: the Records do not say which options a dismantle returns, so the
  consumer applies the `count` itself. The `quantityMultiplier` is not applied.

These are assumptions of the library, not verified against the game.

#### Variant groups

Many ships and items exist in cosmetic variants with the same function. `variantOf` is the `guid`
of the entry of the same kind that is the base of the entity's group, and `null` for the base
itself and for an entity that is alone in its group. Every `variantOf` resolves to an entry of the
same kind. The grouping is a derived fact (ADR-0066): it reads Record data and the vehicle
implementation, never a name, and it ignores `playability`, so an AI variant that is structurally a
player ship is grouped with it.

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
implementation cannot be read in full (a missing file or patch file, a broken document), is
ungrouped, because equality cannot be shown.

**Items** share a group when all of these are equal:

- `type`, `subType`, `size`, `grade` and `manufacturerGuid` of the `AttachDef`;
- the stat blocks (`stats`, ADR-0065), including `resourceStates`; a laser whose beam range, or a
  module whose modifier, differs from another is therefore no variant of it;
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
(compared byte-wise), then the smallest `guid`. A group holds at least two members with different
`guid`s, so a repeated `guid` in a malformed Archive makes no group; `p4kx catalog` prints `variants` and `groups` per kind and `--json` lists them.

Size: one `variantOf` per ship and item, 4 bytes (`null`) or 38 bytes (a GUID) uncompressed; the
GUIDs repeat and compress well.

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
assigned, 1,025 ships with paints (1,046 before ADR-0167). Of the 25, 16 are paints of the ARGO ATLS, an actor rather than
a ship, 2 are weapon skins and 7 are the default livery of a ship (`Paint_Ballista_Base`,
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

| Field | Source |
|-------|--------|
| `entityGuid` | The `guid` of the entry in `ships` or `items` the ports belong to |
| `entityKind` | `ship` or `item`; a closed set the game may extend, so the schema keeps a string |
| `issues` | The codes of what kept part of the Loadout from being read, sorted, without duplicates: `dangling_reference`, `invalid_reference`, `unknown_class`, `ambiguous_class`, `not_an_entity`, `unreadable_record`, `cycle`, `too_deep`, `too_many_ports`, `no_loadout_files`, `xml_loadout`, `assortment`, `vehicle_implementation`; empty when all was read. `no_loadout_files` (an XML Loadout is named but no Archive was given to read it from) cannot occur in a catalog, which always has its Archive. A vehicle implementation or a patch file the Archive lacks is a `vehicle_implementation` issue too, a fact of the game data |
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

**Ports.** Each port has:

| Field | Source |
|-------|--------|
| `name` | The port name as the Loadout or the port definition spells it; port names match ignoring ASCII case |
| `source` | Where the attachment comes from: `loadout` (the entity's own Default Loadout), `override` (the Loadout of the entry that attached the entity, which replaces the entity's own per port), `port_default` (the default item of the port definition), `unset` (defined, holds nothing) or `vehicle` (known only from the vehicle implementation, holds nothing). Closed set, string |
| `minSize`, `maxSize` | `MinSize` and `MaxSize` of the `SItemPortDef`, or `minSize` and `maxSize` of the `ItemPort`; `null` when the data gives none |
| `types` | `{type, subTypes}` per entry of `Types` of the `SItemPortDef`; for an `ItemPort` the `type` of each `Types/Type` and no subtypes |
| `flags` | The words of `Flags` as stored, without a leading `$` (`invisible`, `uneditable`) |
| `portTags`, `requiredTags` | The words of `PortTags` and `RequiredPortTags` of the `SItemPortDef`, or of `portTags` and `requiredTags` of the `ItemPort`: split at white space and otherwise as stored, like `tags` of an item (a leading `$` stays). An item fits a port when it carries the required tags (see "Paints" for the tag tree rule) |
| `displayNameKey` | `DisplayName` of the `SItemPortDef`, a localisation key; `null` for none and for a placeholder key. The key is exported, not its text |
| `visibility` | `visible`, `invisible` or `unspecified`: whether the vehicle implementation's part of that name carries the flag `invisible`; without such a part `invisible` when the port definition carries that flag, else `unspecified`. Closed set, string |
| `itemGuid` | The `guid` of the entry in `items` (a docked vehicle: in `ships`) that the port holds by default; `null` for an empty port. For an attachment that cannot be resolved the GUID the Loadout names, or `null` when it names a class name only |
| `itemClassName` | The class name of the attached entity, or the class name the Loadout names; `null` for an empty port and an unresolved Reference |
| `dockedVehicle` | `true` when the attached entity is a docked vehicle (`AttachDef.Type` starts with `NOITEM_Vehicle`, ignoring case, e.g. a snub craft in a hangar port). It is a reference to a ship and is not expanded: `ports` stays empty (#305) |
| `unresolved` | The code of the error that kept the attachment from being resolved (`issues` lists the codes), otherwise `null` |
| `ports` | The ports of the attached entity under the Loadout that attached it, or the ports of the nested Loadout of an empty port; empty otherwise |

The ports of an entity are, in this order: those a Loadout names in Loadout order, the other
ports the Record defines in definition order, and the `ItemPort` elements of the vehicle
implementation (the Modification applied, ADR-0050) that no port above names, in file order, with
the source `vehicle`. The definition of a port is the Record's `SItemPortDef`; a port the Record
does not define takes it from the `ItemPort` of that name. The visibility comes from the
vehicle implementation (#289), else from the flags of the port definition (#748). A ship's
Loadout overrides the default Loadout of the turrets and racks it carries per port, so the nested ports of a ship are the ones that apply to that ship;
the entry of the turret itself shows the turret's own defaults. The walk is the one of
`EntityNode::loadout` (#316).

**References.** Every `entityGuid` is a `guid` of `ships` or `items`. An `itemGuid` is one of
them too, except for an Entity outside both kinds: a development leftover, or an Entity of another
folder (`p4kx catalog` counts them as the Entities without an entry in items or ships,
`CatalogSummary::loadouts().outside`; the key `outside` of `--json`). Such a GUID stays, so the
Basetool can tell an unknown Entity from an empty port. An `itemGuid` of an unresolved attachment
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

`contracts` lists every contract of the `ContractGenerator` Records and every sub contract of a
contract, one entry each (issue #416). A contract with sub contracts has its own entry and one
more per sub contract; the entries of a contract share its `guid`, a sub contract's entry names its
`id` in `subContractGuid`. Entries are ordered by `guid`, then `subContractGuid` (`null` first),
then `recordGuid`; of two entries with the same `guid` and `subContractGuid` the first is kept.

A contract is an element of the arrays `contracts`, `introContracts`, `legacyContracts` and
`PVPBountyContract` of a handler in `generators`. One without an `id` (or with a nil one), and one
whose debug name or Record is a development leftover, is left out and counted in `skipped`; so is a
sub contract without an `id` (it cannot be told apart) or with a leftover debug name, and an entry
that the rule on equal `guid` and `subContractGuid` above drops.

| Field | Meaning |
|-------|---------|
| `guid`, `subContractGuid` | The `id` of the contract and of the sub contract |
| `recordGuid`, `recordPath` | The `ContractGenerator` Record |
| `debugName`, `handler` | The `debugName` of the contract and of its handler |
| `templateGuid` | The `ContractTemplate` Record the contract names, an entry of `contractTemplates` |
| `brokerEntryGuid` | The `MissionBrokerEntry` Record the contract names in `missionBrokerEntry`, an entry of `missionBrokerEntries`; `null` when it names none (on LIVE the 419 contracts that name one all resolve) |
| `notForRelease`, `workInProgress` | The flags as stored, `null` when absent; the catalog does not filter on them |
| `title` | The resolved title (game text) |
| `description` | The resolved description (game text; must not be passed on to third parties) |
| `missionTypeGuid`, `missionType` | The `MissionType` Record and its localised name (game text) |
| `contractor` | The `Contractor` string parameter (game text) |
| `pickupPlaces` | Up to five starmap objects the contract is offered at, each `starmapObjectGuid`, `objectType`, `place` (game text) |
| `offeredAt` | Every place the contract is offered at, uncapped (see "Offered at"): `localityGuids` (entries of `missionLocalities`) and `locations` (starmap objects, each `starmapObjectGuid`, `objectType`, `place`), and `brokerFallback` (`true` when `locations` is the broker entry's location because no place is stated) |
| `faction` | The `factionReputation` of the handler as stored: `guid`, Record `name` and `displayName` (game text), `null` when the handler names none |
| `giverFaction` | The faction of the contract and where it was found: `source` (`handler`, `reputationResult` or `reputationPrerequisite`) and `faction` (`guid`, `name`, `displayName`), `null` when none names one (see "Giver faction") |
| `blueprintRewards` | The blueprint pools the contract hands out, each `poolGuid` (an entry of `blueprintPools`), `source` (`result` or `temporaryModifier`) and `chance` as stored, `null` for a modifier (see "Blueprint rewards") |
| `reputationGains` | The reputation granted on success; an amount may be negative |
| `minStandingGuid`, `maxStandingGuid` | The `minStanding` and `maxStanding` of the contract: the standings between which it is offered, entries of `reputationStandings`; `null` when absent. A sub contract shares the contract's |
| `standingRequirements` | The reputation prerequisites (see "Standing") |
| `cooldownLayer` | The nearest level that states any cooldown field: `subContract`, `contract` or `handler`; `null` when none is stated (see "Cooldowns") |
| `cooldownLayers` | The level of each of the five cooldown fields, `hasPersonalCooldown`, `personalCooldownTime`, `personalCooldownTimeVariation`, `abandonedCooldownTime` and `abandonedCooldownTimeVariation`: `subContract`, `contract` or `handler`; `null` for a field no level states |
| `hasPersonalCooldown` | The `hasPersonalCooldown` flag, `null` when absent |
| `personalCooldownTime`, `personalCooldownTimeVariation` | The personal cooldown and its variation, as stored (unit: see "Cooldowns"); `null` when absent |
| `abandonedCooldownTime`, `abandonedCooldownTimeVariation` | The cooldown after the contract was abandoned and its variation, as stored; `null` when absent |
| `timeToComplete`, `buyInAmount` | The `timeToComplete` (floating point) and the `contractBuyInAmount` of the contract's `contractResults`, as stored; a sub contract shares the contract's. The Archive states no unit |
| `canBeShared`, `illegal`, `onceOnly`, `maxPlayersPerInstance`, `availableInPrison`, `canReacceptAfterAbandoning`, `canReacceptAfterFailing`, `hideInMobiGlas` | Each `{value, layer}` with `layer` `subContract`, `contract`, `handler` or `template` (see "Contract facts"), `null` when no level states it |
| `requiredIntros` | The completion tags the contract requires with the intro contracts that grant each (see "Contract facts") |
| `fixedRewards` | The payouts the contract states itself on success (`ContractResult_Reward`), each `currency` (`currencyType`, e.g. `UEC`), `amount` (`reward`), `max` and `plusBonuses` as stored; empty for a contract that pays through a calculated reward (see "Reward") |
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

**Title.** The `Title` string parameter of the layers, else the first display string of the
template's `contractDisplayInfo`. A `~mission(Token)` or `~mission(Token|Variant)` in it is
replaced, in each language, by what the first layer (sub contract, contract, handler, template
`contractProperties`) with that `extendedTextToken` says:

- a string-hash property: the text of the first option's `textId`;
- an organisation property: the first organisation's `stringVariants` entry, with a variant the one
  whose tag's `tagName` equals it, without a variant the only entry of an organisation that has
  exactly one (on LIVE 31 of 69 organisations), else none, so the token stays unresolved: the first
  of several entries is tied to a tag (`_from`, `_title_001`, `_desc_001`) and is not the plain
  name of the organisation;
- a location, AI name or ship spawn property, which the game picks when the mission starts:
  `[LOCATION]`, `[TARGET]` or `[SHIP]`;
- anything else, a token no layer states and a text the language lacks: the token in upper case in
  brackets, e.g. `[CARGO]`.

A replacement that itself holds a `~mission(…)` is expanded up to three levels deep; below that,
and past 64 replacements or about 4 KiB in one title, a token becomes `[TOKEN]`. Leading and
trailing blanks are removed from the result; the `key` is that of the title text. The tokens are free text of the game, not facts of the Archive;
the placeholders are the catalog's own convention.

**Description (ADR-0264).** The `Description` string parameter of the first layer (sub contract,
contract, handler), else the `description` of the broker entry; the template states none, its
`displayString` slots carry no label. `~mission(…)` tokens, limits and trimming are those of the
title. It is a game text (`x-gameText`) like the title, and like every game text it must not be
passed on to third parties: the Basetool keeps it for its logged-in members.

**Broker entry.** A contract that names a `missionBrokerEntry` and has no template (on LIVE the 419
`Covalex_HaulCargo_Career` contracts and their two sub contracts) takes what the layers and the
template leave open from that `MissionBrokerEntry` Record: its `title`, its `type` and, when no
pickup place is left from the prerequisites (none named, or only stars or objects that are not in
`starmapObjects`), its `locationMissionAvailable` as the pickup place.

**Mission type.** `missionTypeOverride` of the first layer that states one, else the `type` of the
template's `contractDisplayInfo`, else of the broker entry; `missionType` is the
`LocalisedTypeName` of that Record.

**Reputation.** Of the results of the contract (`contractResults.contractResults`), those of type
`ContractResult_LegacyReputation` whose first `missionResults` entry (success) is set, one
`reputationGains` entry per `contractResultReputationAmounts` element, in stored order: the
faction and scope Records with their Record names (the last part of the record path) and the
`reputationAmount` of the reward Record. A result that does not fire on success is not listed. An
amount can be negative: on LIVE the 48 negative amounts (of 2,809 success amounts) all belong to
the `CFP_OOS`, `CFP_OOS_Intro`, `HH_OOS` and `HH_OOS_Intro` handlers, twelve contracts each. In
every one the success result names the rival faction (a `CFP_` handler deducts
`factionreputation_unlawful_headhunters`, a `HH_` handler `factionreputation_lawful_citizensforprosperity`)
with the reward `repuationrewardamount_negative_bigdrop` (-190500); they are listed as stored, next
to the handler's own faction in `faction`. The catalog does not add the amounts up (ADR-0065): a
faction scope and an affinity scope are separate entries. All sub contracts of a contract list the
contract's results.

**Contractor.** The `Contractor` string parameter of the first layer (sub contract, contract,
handler) that states one: the text the game shows as the giver, e.g. of an agency that has no
faction reputation (game text).

**Pickup places.** The starmap objects named by the prerequisites of the sub contract
(`additionalPrerequisites`), or, when it names none, of the contract and then of the handler
(`defaultAvailability.prerequisites`): the `availableLocations` of each
`ContractPrerequisite_Locality` (a `MissionLocality` Record) and the object of each
`ContractPrerequisite_Location`, in stored order, none twice. A star (`type` `Star`) is left out and
what is not in `starmapObjects` is skipped. The list is ordered by rank, stably: the star system
(`SolarSystem`) first, then planets and moons (`Planet`, `Moon`), then every other type, and holds
at most **5** places (`MAX_PICKUP_PLACES`). Each place has the `starmapObjectGuid`, its `objectType`
and the `place` name (game text). The places come from the starmap objects of the same build: a
Filter that leaves them out leaves `pickupPlaces` empty.

**Offered at (ADR-0273).** The same prerequisites as the pickup places, read without the cap and
without a filter on the object type. `localityGuids` lists the `MissionLocality` Record of each
`ContractPrerequisite_Locality`, in stored order and none twice; a consumer reads its places in
`missionLocalities[].starmapObjectGuids` and their names in `starmapObjects`. `locations` lists the
object of each `ContractPrerequisite_Location` as stated: a star stays, an object missing from
`starmapObjects` has `objectType` and `place` `null`. As for the pickup places, the places of a sub
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
for `reputationGains`, so a faction Record outside the faction folder stays with a `null`
`displayName`. A negative amount names a rival that loses standing and never counts. The catalog
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
`blueprintPools[].rewardSources` names only the `id` that a contract and its sub contracts share.
Three reward sources of `blueprintPools` are in no entry: `BlueprintRewards` results that fire on the
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
  an empty list the game fills in); the catalog does not read `MissionItem` Records;
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
  success, so no `blueprintRewards` entry lists them although `blueprintPools[].rewardSources`
  does (ADR-0273).

**Reward (ADR-0268).** `fixedRewards` lists the `ContractResult_Reward` results (a
`MissionReward`) that fire on success. Most contracts pay through `ContractResult_CalculatedReward`,
which holds only `missionResults`: no amount, no reference, no rule. The game computes that
payout outside the data, so the catalog does not state it and `fixedRewards` is empty for such a
contract.

Not exported: a computed reward (see above).

**Standing (ADR-0237).** `minStandingGuid` and `maxStandingGuid` are the `minStanding` and
`maxStanding` References of the contract, entries of `reputationStandings`; a sub contract has none
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
can take its fields from different levels: `cooldownLayers` names the level of each, and
`cooldownLayer` is the nearest of them, `null` when no level states any. On LIVE 4.10, 215 entries
take a field from the contract's overrides, 6 from the sub contract's, and 4,954 from the handler
alone. The numbers are as stored, in floating point. The Archive does not state their unit: no
type, attribute or game text shows it. The range (`personalCooldownTime` up to 2880 in a handler
and 10080 in a contract override, `abandonedCooldownTime` up to 60 at every level) fits minutes,
2880 being 48 hours and 10080 a week, but that is a reading, not a fact of the Archive. On LIVE 4.10 all 5,175 entries state a cooldown.

**Contract facts (ADR-0263).** `timeToComplete` and `buyInAmount` are numbers as stored. Each of
the other eight values is `{value, layer}`: the first level that states it, nearest first. A
level states it through a boolean or integer parameter (`CanBeShared`, `Illegal`, `OnceOnly`,
`MaxPlayersPerInstance`, `AvailableInPrison`, `CanReacceptAfterAbandoning`,
`CanReacceptAfterFailing`, `HideInMobiGlas`, matched without regard to case) of the sub contract,
the contract (`paramOverrides`) or the handler (`contractParams`); when none does, the base value
is the one of the Record that owns the default: the `defaultAvailability` of the handler (layer
`handler`) for `onceOnly`, `maxPlayersPerInstance`, `availableInPrison`, the two re-accept flags
and `hideInMobiGlas`; the template (layer `template`) for `illegal` (`contractDisplayInfo`) and
`canBeShared` (`ActiveContractSettings` of the contract class). A contract without a template
(the legacy `Covalex_HaulCargo_Career` contracts) has `null` for `illegal` and `canBeShared`
unless a parameter states them. A negative `maxPlayersPerInstance` is as stored (-1 on 41 LIVE
entries).

`requiredIntros` lists the `ContractPrerequisite_CompletedContractTags` required tags of the handler
(`defaultAvailability`), the contract and the sub contract (`additionalPrerequisites`), in that order
and as stored: `level`, `tagGuid` and `introContracts`, the `id` of every contract in an
`introContracts` array whose success result grants the tag (`ContractResult_CompletionTags`), ordered
by `id`; empty when no intro contract grants it. The excluded tags are not exported, and the
catalog does not decide how the levels combine. On LIVE 4.10 (2026-10-30) 2,464 of the 5,175 entries
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

Seven kinds (ADR-0237, schema version 2, no bump) hold what the contracts refer to. Each is the
Records of one type under one folder (see "Selection"), ordered by GUID, with `guid`, `className`
(the Record name without its type) and `recordPath`. Facts only: identifiers, short names, numbers
and links. A Reference to no Record is left out (`null`, or missing from the list); a GUID of an existing Record that the catalog does not list is kept, so `missionLocalities[].starmapObjectGuids` may name an object missing from `starmapObjects` under a Filter.
Descriptions, perk texts, logos, icons and the faction property texts (headquarters, founding,
leadership, focus) are not exported.

On LIVE 4.10 (2026-10-28) the kinds hold 39 factions, 46 scopes, 380 standings, 27 contexts, 42
mission types, 20 localities and 2 scenarios, none skipped.

**Factions (`factionReputations`).** `displayName` is a game text (37 of 39 have an English text).
`isNpc` is the Record's flag (false for all 39). `lawful` is the value of the boolean property
`entityLawful` among the `propertiesBB` of the Record, `null` for a faction without it
(`FactionReputation_IASI`): 29 are lawful, 9 not. It is a field and not the class name, so
`FactionReputation_Wikelo` is lawful and `FactionReputation_Ruto` is not although neither name says
so. `contextGuid` is the `reputationContextPropertiesUI`; `allyGuids` and `enemyGuids` are the
References as stored (two factions have enemies on LIVE, none has allies); `hostility` and
`allied` hold the `scopeGuid` and `standingGuid` of the `hostilityParams` and `alliedParams`, `null`
when both are absent.

**Scopes, standings and contexts.** A scope (`reputationScopes`) has `scopeName`, `displayName` (a
game text), `reputationCeiling`, `initialReputation` and `standingGuids`, its standings in stored
order. A standing (`reputationStandings`) has `displayName` (the rank's name, a game text; 196 of
380 have a text, the rest are placeholders), `minReputation`, `driftReputation`, `driftTimeHours`
and `gated` (47 standings). A context (`reputationContexts`) has `primaryScopeGuid` and `scopeGuids`
(the `scopeContextList`). The scope of a gain in `reputationGains` is an entry of
`reputationScopes` (10 of the 46 are used by a contract).

**Mission types (`missionTypes`).** `typeName` is the `LocalisedTypeName`, a game text (41 of 42
have an English one). Every `missionTypeGuid` of `contracts` (26 types on LIVE) is an entry.

**Mission localities (`missionLocalities`).** `starmapObjectGuids` are the `availableLocations` in
stored order (218 on LIVE, all in `starmapObjects`). The game does not localise the name of a
locality; the `className` (`Pyro`) is the only name.

**Contract scenarios (`contractScenarios`).** A `ScenarioProgress` Record. `factionRewardTiers`
hold, per `Faction` Record (`factionGuid`, `factionName` the last part of its record path),
`factionReputationGuid` (the `factionReputationRef` of that `Faction` Record, an entry of `factionReputations`; no kind lists the `Faction` Records themselves), `pointsAsPercent` and the `progressions` (`minMultiplierThreshold`, `completionType` the name of
the struct, `periodScenarioGuid`), each with `tiers`: `minPoints`, `badgeToAward` (an identifier;
`null` for a tier without a badge, which the game stores as the text `None`), `globalPointsMultiplier` and
`blueprintPoolGuids`, entries of `blueprintPools`. The progression texts and the multiplier rewards
(empty on LIVE) are not exported.

Not exported, for want of a consumer: the sandbox triggers and perks of the reputation Records.
Contract templates are described in "Contract templates", mission broker entries in "Mission broker
entries", ship spawns and the locations of contracts in "Ship spawns and location queries" and NPC
and entity spawns in "NPC and entity spawns".

### Ship spawns and location queries

Three kinds and three contract fields (#902, ADR-0266, additive, schema version 2, no bump) state
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
`excludeProperty`, `propertyTags`, `specificOrganizations`, `specificItems` and `other`
(`structName` of a condition the catalog does not read; none on LIVE). A condition's weak pointer
is the `missionVariableName` of the property it points to; on LIVE 83 of 431 distance conditions
point to a property that has no variable name and have `null`. The game runs the query against the
location templates when the mission starts, depending on the other properties of the mission
(distance, organisation, items); the catalog exports the query as stored and never a list of
resulting places or starmap objects. The Archive links a query to a starmap object only through the
pickup `MissionLocality` Records (see "Contracts").

**Location templates (`missionLocationTemplates`).** A `MissionLocationTemplate` Record under
`missiondata/pu_locations` (one more of the type lies outside that folder, a test platform, and is
not part of the kind): `guid`, `className`, `recordPath`, `disabled`, `isSecurityNetworkHost`, the
`tags` (`generalTags`) a query matches, `producesTags`, `consumesTags` (`positiveTags` and
`negativeTags`), `aiSpawnTags`, `names` (the `stringVariants`: `tag` and `displayName`, a game
text; the German file lacks 3,246 of the 3,297 variants on LIVE, so `de` is `null` and the Basetool
falls back to `en`) and `entityClusterMemberGuid`. Not exported of a location template: the NPC auto spawn settings of its
`locationData` (not the NPC spawn descriptions of contracts, see "NPC and entity spawns"), the
mission modules and limits and the security network manifest.

Not exported: SCMDB's own pools, waves and region lists, which are derived and not stated by the
Archive.

### Contract templates

`contractTemplates` lists the `ContractTemplate` Records (#812, ADR-0313, additive, schema version
2, no bump), ordered by GUID. `contracts[].templateGuid` is the `guid` of an entry; every one
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
`contractTemplates` was first released (v0.4.2 is unreleased), so no consumer has seen them and the
additive rule of ADR-0054 is not touched.

| Field | Meaning |
|-------|---------|
| `guid`, `className`, `recordPath` | The Record |
| `notForRelease`, `workInProgress` | The flags as stored, `null` when absent |
| `title` | The first display string (game text); `null` texts for the placeholder most templates have |
| `missionTypeGuid` | The `MissionType` of the display info, an entry of `missionTypes` |
| `illegal`, `showLifeTimeInMobiGlas`, `preShowObjectives` | The flags of the display info |
| `classType`, `settings` | The struct of the contract class (`ContractClass_Contract`, one template is a `ContractClass_PVPBounty`) and its `ActiveContractSettings`: `canBeShared`, `hasCompleteButton`, `handlesAbandonRequest`, `displayAlliedMarkers`, `onlyOwnerCanComplete`; `null` when absent |
| `autoFinish` | The `ContractAutoFinishSettings`: `failIfSentToPrison`, `failIfBecameCriminal`, `failIfLeavePrison` and the deadline as stored: `completionTime`, `autoEnd`, `resultAfterTimerEnd` (an objective state), `remainingTimeToShowTimer`; the Archive does not state the unit of the times; `null` without the block |
| `contractProperties` | The mission properties in stored order: `variable`, `token` (`null` when empty), `valueType`, the struct of the value without `MissionPropertyValue_` (`StringHash`, `Location`, `ShipSpawnDescriptions`, `HaulingOrders`, ...), and `value`, the union of the objective properties (see "Objective properties"); `null` when the property has no value |
| `shipSpawns`, `npcSpawns`, `entitySpawns`, `locationQueries` | The links of ADR-0266 and ADR-0324, one per variable the template states, with `level` always `template`, into `contractShipSpawns`, `contractNpcSpawns`, `contractEntitySpawns` and `contractLocationQueries` |
| `haulingOrders` | The hauling orders of the template as in "Hauling orders": the properties (`source` `template`) and the orders of its hauling objectives (`templateObjective`) |
| `objectives` | The objective tokens depth first, each phase after its parent (see "Objectives") |
| `partialPayout` | The `PartialContractRewards`: `thresholds`, each `minPercentage`, `maxPercentage`, `currencyRewardMultiplier` and `reputationMultipliers` (`scopeGuid`, an entry of `reputationScopes`, and `reputationRewardMultiplier`); `null` without one |
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
`hideMarkers`, `securityManifestGuid`, `navPoint` (`entityClassGuid`, `displayName`, a game text,
and `extendedTextTokens`), `travelObjective` and `returnObjective` (displays as above), `hauling`
(the `Hauling_ObjectiveSettings`: five extended text tokens and seven displays) and `instance`
(`completePhaseOnPlayerEnter`, `enterTravelRadiusKm`, `hudObjectiveText`, a game text,
`tagIdentifier`, `searchTags`, `objectiveMarker`). The orders of a hauling handler are
`haulingOrders` of the template.

**Objective properties.** An entry of `objectiveProperties` has `kind` (`referenced`, `output`,
`embedded` or `input`, the struct `ObjectiveProperty_*`), `variable` and `token` (the
`missionVariableName` and `extendedTextToken`, `null` when empty), `contractProperty` (for a
referenced property the `missionVariableName` of the property of the template it points to),
`value` (for an output, embedded or input property) and `linkedOutput` (for an input: the
`objectiveId` of the objective that holds the output and its `variable`). The `value` is tagged by
`type`: `object` and `deliveryOrder` (no fields), `boolean` (`value`), `stringHash` (`options`, each
`displayName`, a game text, `value`, `weighting` and `dependentProperties`, the variables of the
properties the option makes dependent), `integer` and `float` (`options`, each `displayName`,
`value`, `variation`, `weighting` and `dependentProperties`), `tags` (`tags` and `negativeTags`),
`shipSpawnDescriptions` (`poolGuid`, an entry of `contractShipSpawns`, `null` for a description
without a group), `location` (`queryGuid`, an entry of `contractLocationQueries`),
`npcSpawnDescriptions` and `entitySpawnDescriptions` (`poolGuid`, an entry of `contractNpcSpawns` or
`contractEntitySpawns`, `null` for a description without a group) and `organization` (`conditions`,
the `matchConditions` as for a location query, and `forcedOrganizationGuid`, the
`debugForceChosenOrganization`, a `MissionOrganization` Record no kind lists). The contract-level
properties add `missionItem` (`conditions`, the `matchConditions` as for a location query,
`forcedItemGuid`, the `debugForceChosenItem`, a `MissionItem` Record no kind lists, `minItemsToFind`
and `maxItemsToFind`), `aiName` (the fields of an embedded NPC name, see "NPC and entity spawns":
`randomName`, `randomLastName`, `randomNickName`, `givenName`, `givenLastName`, `givenNickName`,
`chanceOfNickName` and `nameDataGuid`), `combinedDataSetEntries` (`entryProperties`, the variables
of the `dataSetEntryProperties`), `haulingOrders` (`requirements` as in "Hauling orders"), `reward`
(`reward`, the `rewardDef` as the `reward` of a broker entry, `null` without one), and `timeTrialRace`
and `starMapLocation` (no fields). The spawn descriptions and location queries are read as for the
contracts and added to their kinds. A value of any other struct fails the Record (on LIVE the
values of all properties are of the types above); so does an organisation or a mission item whose
condition is of a struct the catalog does not know.

**Contract-level properties.** The `value` of an entry of `contractProperties` of a template and of
`missionProperties` of a broker entry is read by the reader of the objective values and states the
same union; `valueType` stays. On LIVE every property that has a value struct states one.

**Mission flow.** A trigger has `note` (the developer's `description`), a `condition` and an
`action`, either `null` when absent. A condition is tagged by `type`: `tokenState` (`objectiveId`,
the `id` of an entry of `objectives`, and `tokenState`), `allTokensState` and `anyTokensState`
(`tokenState`), `onMissionStart` (no fields), `and` and `or` (`conditions`), `not` (`condition`) and
`propertyIsSet` (`propertyVariable`). An action is tagged by `type`: `stateChange` (`toState`),
`tokenStateChange` (`objectiveId`, `toState`) and `pickRandomMissionPhase` (`phases`, each
`objectiveId` and `probability`, and `toState`). A state is `active`,
`completed`, `failed` or `abandoned`. A stored enum name the catalog does not know is `null`, like
the values of every closed set, and is counted in the warning `unknown_template_values` of the
build. A weak pointer that points to nothing, to a token without an `id` or to a struct outside the
Record has a `null` link.

A token without an `id` (or a nil one) and the phases below it are left out. A Record whose phases
nest deeper than 32 levels or whose conditions nest deeper than 16 cannot be read, and so is one
with a condition, an action or a modifier of a struct the catalog does not know: it is a failure of
the build and not an entry (none on LIVE). A future Build that meets such a struct therefore
lacks the whole Record: it is not in the kind, the summary counts it in the `failed` list of the
build (`p4kx catalog` prints it, with the record path and the struct name as reason, and exits with
code 1), and the contracts that name it keep a GUID that resolves to no entry, so a consumer must
check the failure list before it relies on the links being complete.

### Mission broker entries

`missionBrokerEntries` lists the `MissionBrokerEntry` Records (#813, ADR-0320, additive, schema
version 2, no bump), ordered by GUID: the offers of the mission board. `contracts[].brokerEntryGuid`
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
| `missionTypeGuid`, `locationGuid`, `localityGuid` | Links to `missionTypes`, `starmapObjects` (the `locationMissionAvailable`) and `missionLocalities` |
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
mission broker entry (#815, ADR-0324, additive, schema version 2, no bump) state the people and the
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

The `embeddedName` (`MissionPropertyValue_AIName`, `null` when absent; 22 options on LIVE, all of
them with placeholder texts) has `randomName`, `randomLastName`, `randomNickName`, `givenName`,
`givenLastName` and `givenNickName` (game texts, the keys with their English and German texts; a
placeholder key such as `@LOC_UNINITIALIZED` has a `null` key and no texts), `chanceOfNickName` and
`nameDataGuid` (the `CharacterRandomNameParams` Record, no kind lists these).

**Entity spawn descriptions (`contractEntitySpawns`).** An entry has `guid` and `groups`
(`SpawnDescription_EntityGroup`: `name` and `slots`; a slot is a `SpawnDescription_EntityOptions`,
the alternatives for one entity, each an `options` entry with `amount`, `weight` and the tag sets
`tags`, `negativeTags`, `markupTags` and `entityTags`).

### Cargo manifests

`cargoManifests` lists the `CargoManifest` Records under `cargomanifest` (ADR-0265), ordered by GUID,
with `guid`, `className` and `recordPath`. A manifest tells the game how to fill the cargo grid of a
ship it spawns. Facts only; the tags and the description tags are not exported.

| Field | Meaning |
|-------|---------|
| `fillType` | `even`, `random` or `custom`: the type of `cargoFillCapacity` (4 `custom`, 34 `even`, 43 `random` on LIVE); `null` for another type |
| `minCapacityRange`, `maxCapacityRange` | The share of the cargo capacity that is filled, 0 to 1; `null` for `custom`, which states shares per resource |
| `lockCargoGridsOnSpawn` | The flag as stored, `null` when absent |
| `resources` | The resources of the fill in stored order: `resourceGuid` (the `subtypeGuid` of a commodity), `probability`, and for `custom` `minAllocation` and `maxAllocation` (the allocation percentages as stored, else `null`) |
| `spawnRules` | The `cargoSpawnRules` in stored order: `kind` (`entities` or `resources`), `probability`, `entityGuids`, `resourceGuids`, the struct type of the quantity algorithm in `quantityAlgorithm` and its `quantity` (fixed) or `proportion` |

The Archive refers to a manifest only from the members of an `AIWaveCollection` (the ship
encounters), never from a contract, so the kind has no link from `contracts`. On LIVE 4.10
(2026-10-30) the kind holds 81 manifests with 515 resources, 4 of which (4 distinct `ResourceType`
Records) have no commodity in the catalog, and 2 spawn rules, both of one manifest.

### Starmap objects

A starmap object (`starmapObjects`) is a `StarMapObject` Record under `starmap/pu` (ADR-0054: a
new kind, so `schemaVersion` stays 1). The kind is facts only: no description, no lore, no media.
The `name` is a game text, the localisation key with its English and German text, and carries
`"x-gameText": true` like a description; the Basetool keeps it from third parties.

| Field | Source |
|-------|--------|
| `guid`, `className`, `recordPath` | The Record, as for every entity |
| `name` | `name`, a localisation key |
| `type`, `typeGuid` | The `StarMapObjectType` Record the Reference `type` links to: `type` is its `name` when that is a plain string, else its Record name without the type, and `typeGuid` its GUID; `null` for a null or dangling Reference. The schema keeps `type` a string, so a new type is additive |
| `parentGuid` | The Reference `parent`, the `guid` of another entry unless that Record is missing or left out; `null` for none |
| `hideInStarmap` | `hideInStarmap`; `null` when the Record states none |
| `system` | The folder of the star system below `Data/ObjectContainers/PU/system` that places the object, e.g. `stanton`; `null` when no container places it |
| `entityName` | The `entityName` of the Child element that places the object, e.g. `jumppoint_stanton_pyro`; `null` when it is not placed or the Child names none |
| `placedUnderGuid` | The `guid` of the nearest object above this one in the container hierarchy; `null` when it is not placed or hangs directly below the system. It is a fact of the containers; `parentGuid` is the Record's own and the two need not agree |
| `position` | `[x, y, z]` in metres in the frame of the system container, rounded to a millimetre; `null` when no container places the object |

**Where a position comes from.** The main container of a star system is
`Data/ObjectContainers/PU/system/<folder>/<folder>system.socpak` (else the first `*system.socpak`
of the folder). It is a container (ZIP) whose XML, named like the container, lists
`ChildObjectContainers/Child` elements. A Child places an object with `starMapRecord` (the GUID of
a `StarMapObject`), `pos` (`x,y,z`) and `rot` (a quaternion `w,x,y,z`), may nest further
`ChildObjectContainers`, and with `external="1"` names another container (`name`) whose Children
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
below it are not placed. The same holds for the walk limits: Children nested deeper than 64,
more than 4,194,304 Children in one system, an external container reached through more than 32
others, and an external container that is already open above the Child that names it (a cycle) are
counted and not followed. All of these, and a container that cannot be read, are warned about by
`p4kx catalog` (exit code 1); the counts `unreadable_containers` and `malformed_children` are
counted per reference, not per distinct container, so one container named by ten Children
counts ten times. A container that a Child names and the Archive does
not hold (`missing_containers`, also counted per reference; LIVE: two, an event container and a
module of a Lorville entry) is a fact of the game data, like a patch file the Archive lacks: it is
counted and listed in `CatalogSummary::starmap_report()` but is no warning. The report keeps at
most 300 lines of containers and 300 of issues per star system, each at most 300 characters.
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

These objects keep `position` `null`; the catalog never guesses a place from the parent.
The kind is selected by the prefix `libs/foundry/records/starmap/pu/` and the type `StarMapObject`
like the others, with the development leftovers left out. The positions do not depend on the
Filter: the containers are read whole. The `StarMapObjectType`, `Jurisdiction`, amenity and radar
Records are not kinds; the type enters as `type` and `typeGuid`.

Size, measured on LIVE (2026-10-22): the 2,093 objects are about 1.2 MB of JSON (570 bytes each), 4.5 % of the 26.5 MB of the
catalog, and a smaller share compressed.

**No released marker.** The Archive states no structural marker that tells a released starmap
object from one the game has not released (ADR-0067, #417): the three star system Records carry the same flags,
`hideInStarmap` is set on helper content of the released systems too, and the type suffixes
(`_ValidQT`, `_InvalidQT`) describe quantum travel. The catalog therefore has no such field; the
Basetool decides from `system`, `position`, `hideInStarmap` and `type`.

### Mineable elements and refining processes

The raw facts only (ADR-0065): the catalog states no instability per commodity, because the
relation is not one to one. The Basetool applies its own rule.

A mineable element (`mineableElements`) is a `MineableElement` Record under `mining/mineableelements`
(46 Records on LIVE, 43 entries). `resourceTypeGuid` is the `ResourceType` its Reference
`resourceType` links to; `null` for a null or dangling Reference. It equals a commodity's
`subtypeGuid` only when the catalog has that commodity: commodities are backfilled only for the
`ResourceType`s that a blueprint ingredient uses or that a kept quality quantization links to, so a
mined `ResourceType` that neither does has none (LIVE: 25 of the 43 linked `ResourceType`s were backfilled for a quantization alone).
Several elements may share one `ResourceType` (41 distinct for the 43 entries), so a commodity can
have several elements with different values; every element is listed and none is picked. The numbers
are those of the Record, unchanged, and `null` when it states none. The ranges are those of the 43
entries on LIVE:

| Field | Record field | LIVE range |
|-------|--------------|------------|
| `instability` | `elementInstability` | 0 to 1000 |
| `resistance` | `elementResistance` | -1 to 1 |
| `optimalWindowMidpoint` | `elementOptimalWindowMidpoint` | 0.3 to 1 |
| `optimalWindowMidpointRandomness` | `elementOptimalWindowMidpointRandomness` | 0 to 0.3 |
| `optimalWindowThinness` | `elementOptimalWindowThinness` | -1 to 10 |
| `explosionMultiplier` | `elementExplosionMultiplier` | -1000 to 260 |
| `clusterFactor` | `elementClusterFactor` | 0 to 1 |

A refining process (`refiningProcesses`) is a `RefiningProcess` Record under `refiningprocess`
(9 on LIVE, one per pair of speed and quality). `refiningSpeed` (`Slow`, `Normal`, `Fast`) and
`refiningQuality` (`Wasteful`, `Normal`, `Careful`) are closed sets kept as strings, so a new value
is additive; `processName` is the localised label of the method, a game text. On LIVE (2026-10-22)
the English `global.ini` holds all 9 keys and the German one none, so `de` is `null` there. The
DataForge holds no yield per commodity and method: yields are not in the Archive and stay with
other sources.

The 46 element Records of LIVE give 43 entries: the development-leftover rule leaves out
`MinableElement_Ship_template`, `MinableElement_GroundVehicle_template` and
`MinableElement_FPS_template`, counted as skipped. The 9 `RefiningProcess` Records are all entries.

### Mining deposits and harvestable locations

Facts only (ADR-0236): numbers, GUIDs, names of Records and paths. Texts other than `depositName`
are not exported and no media. The loot of a slot is its `lootConfig` (see "Loot").

**Compositions (`mineableCompositions`).** A `MineableComposition` Record under
`mining/rockcompositionpresets` (249 on LIVE 4.10, 231 entries; the leftover rule skips the rest).
`depositName` is the HUD label of the deposit, a game text. `minimumDistinctElements` and each part
of `compositionArray` are as stored: `mineableElementGuid` (the element of `mineableElements`;
`null` for a null or dangling Reference), `minPercentage`, `maxPercentage`, `probability`,
`curveExponent` and `qualityScale`, in stored order.

**Parameters (`miningGlobalParams`).** The `MiningGlobalParams` Records under `mining` (7 on LIVE,
6 entries). Every number of the Record is a field of the same name in camel case (`cScuPerVolume`
for `cSCUPerVolume`); `hitConsistencyParams`, `mineableInstabilityParams` and the numbers
`dangerPoolFactor` and `defaultVolume` of `mineableExplosionParams` are the objects `hitConsistency`,
`instability` and `explosion`. Particle effects, audio triggers and the default explosion are not
exported. `wasteResourceTypeGuid` is the `ResourceType` of the waste.

**Entities (`mineableEntities`).** The `EntityClassDefinition` Records under `entities/mineable`
(269 on LIVE 4.10, 231 entries; the leftover rule skips the 38 `test` and `template` Records,
`test_miningdestroyable` among them). A provider preset reaches an entity through the
`entityGuid` of a harvestable. `mineableCompositionGuid` and `miningGlobalParamsGuid` are the
References of `Components[MineableParams]`, `radarContactTypeGuid` the Reference
`radarProperties.contactType` of `Components[SSCSignatureSystemParams]` (a `RadarContactTypeEntry`
Record, no kind). `signatures` is `radarProperties.baseSignatureParams.signatures` as stored, one
number per signature type (8 on LIVE), empty when the Record has none. `scanSignature` is the
value the mining scanner shows, from 3000 to 4900 on LIVE. The slot is named by the Archive
(ADR-0242): the `RadarSystemGlobalParams` Record lists one `signatureTypeParams` entry per slot of
`signatures`, and the slot whose `displayName` is `@hud_scanning_info_rs_signature` is the scan
signature (index 4 on LIVE 4.10, after IR, EM, CS and DB). `scanSignature` is `null`
when the Record states no signatures, every signature is zero (the FPS blockers and the pearls,
7 of the 231), `signatures` has not as many numbers as the radar parameters have signature
types, or the Archive names no such slot (no radar parameters, no entry with the key, or the key
on several entries): an unfiltered build then warns (`unknown_scan_signature_slot`) when any
entity states signatures. A consumer that wants another slot reads `signatures`, in stored
order; a number that is not a finite 32-bit float is exported as 0. Entities are selected by
record path and type alone, so one without the components has `null` links and empty
`signatures`.

**Provider presets (`harvestableProviderPresets`).** The `HarvestableProviderPreset` Records under
`harvestable/providerpresets` (49 on LIVE). `groups` keep `groupName`, `groupProbability` and the
`harvestables`; each has `relativeProbability`, `clusteringGuid`, `setupGuid`, the GUIDs of its
geometry tags and `harvestable`, which follows the element to what it spawns: the
`HarvestablePreset` Record (`presetGuid`, `presetClassName`), the entity (`entityGuid`, the
element's `harvestableEntityClass`, else the preset's `entityClass`) and the `MineableParams` of
that entity (`mineableCompositionGuid`, `miningGlobalParamsGuid`). A harvestable that is no rock
has both `null`. `areas` keep `debugGroupName` as `name`, `globalModifier`, the `objectPresetPaths`
of the area type and the `modifiers`, whose weak pointer gives `groupIndex` and `harvestableIndex`
into `groups` (`null` for a pointer into another Record). The surface presets of the planets and
moons (`HPP_Stanton1a`, `HPP_Pyro5b`, 25 of the 49) are tied to a place by these object preset
paths (for Hurston they name the star system and the body, for the others no body) and by no
container.

**Multi configs (`subHarvestableMultiConfigs`).** The `SubHarvestableMultiConfigRecord` Records
under `harvestable/slotpresets` (103 on LIVE). `defaultConfigGuid` is the Reference of a default
config that is a reference, `taggedConfigs` keep `name`, the tag GUIDs, `configGuid` for a config
that is a reference, and for a config written out `respawnTimeMultiplier`,
`initialSlotsProbability` and the `subHarvestables` (`harvestable` as above, `setupGuid`, multiplier,
`relativeProbability`, `relativeProbabilityDeepest`, geometry tags and `lootConfig`).

**Locations (`harvestableLocations`).** ScDataDumper's `SocpakMappingGenerator` finds the preset
of a container in the first `.soc` file of the container: the GUID in the string that follows the
string `preset` (strings are separated by a zero byte). The catalog does the same for every
container below `Data/ObjectContainers/PU/system` and keeps the rows whose GUID is a kept
provider preset. The `.soc` is scanned as a stream, up to 256 MiB (the largest of LIVE holds
116 MB); a container with a larger `.soc` and no preset in the first 256 MiB is warned about, not
taken for one without a preset. A row has `presetGuid`, `system` (the folder of the star system), `container` (the Archive
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
(it is not in `starmapObjects`, 64 or more mining claim markers lie above, or a marker has no
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

**Pool filters (`lootPoolFilters`).** A `PoolFilterRecord` under `lootgeneration/filters` (15 on
LIVE). `filter` is a node with the struct as stored in `nodeType`: a `PoolFilter_Sequence` holds
`filters`, each an instance with `name`, `mode` (`Cumulative` or `Additive`) and a `filter` node
(`null` for none); a `PoolFilter_RecordRef` names another Record in `filterGuid`; a
`PoolFilter_Tags` holds `positiveTagGuids` and `negativeTagGuids`. Nodes nest at most 16 deep. The
secondary choices Records (manufacturer weights, 25 on LIVE) are not exported. `nodeType` and
`selectorType` (of the selectors of "Archetypes") state the struct as stored and are informational:
a consumer selects by the fields (`filterGuid`, the tag lists, `filters`), never by those names.

**Older loot Records (`legacyLootTables`, `legacyLootArchetypes`).** The `LootTable` (161 on LIVE)
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
`poolFilterGuid` (an entry of `lootPoolFilters`), `totalResultsLimit`, `chanceToGenerate`,
`chanceToGenerateAdditionalInventories` and the `fullness` range of `fullnessFactorRange`.

**Placed containers (`lootContainers`).** A container below `ObjectContainers/PU/system`, or one
it places, holds the property file of an entity (`entdata/<n>.entxml`) that names the container it
places (`objectContainer`) and a `multiConfigRef`, a `SubHarvestableMultiConfigRecord`. The placed
container holds tag points, entities that carry tags, in its `.soc` files. The tags of those points
are the `tagGuids` of the tagged configs of the multi config (the 13 points of a Stanton derelict
carry the Rare, Common, Uncommon and Reward tags of `LootGeneration_MultiSlotPreset_Derelict` 4, 4,
2 and 3 times); the slots of a tagged config carry the `lootConfig`. The catalog states these
facts and not how the game picks a slot for a point. A row is one placed container with the
`multiConfigGuid`, `container` (the Archive path, else the path as written when the Archive lacks
it), `system`, `starmapObjectGuid`, `entityCount` and `tagCounts`: how many of the entities carry
each tag, summed over all `.soc` files of the container. A container without a `.soc` has
`entityCount` 0 (no tag points; this is intended and not warned about); one whose `.soc` cannot be
read, or that the Archive lacks, has `null` and no counts.

The walk starts at every container below `ObjectContainers/PU/system` and follows the containers
the entity files place, each container read once. The location is that of the container the walk
started in: `starmapObjectGuid` is the object the Child that reaches it hangs below (as for
"Locations" above, with `null` for a container no Child reaches or an object the catalog lacks). A
container placed under several roots gives a row for each, and containers that place each other
(A places B places A) are read once. The warning `unreadable_loot_containers` names a container or
entity file that cannot be read, a `.soc` that cannot be counted, a placed container the Archive
lacks, and containers beyond the limit of the walk, each with its reason. An entity file that names
several different containers or multi configs uses the first of each in document order and is
warned about (`ambiguous_loot_entity_files`). The cost of the walk is the size of the subgraph below
each distinct root; it is not memoised, as it is negligible on LIVE.

On LIVE 4.10 (2026-10-28) 3,004 rows name 41 multi configs and 189 containers in 3 systems, 2,797
of them with an object.

### Crafters

A crafter (`crafters`) is an entity whose `Components` hold a `CrafterComponentParams` (ADR-0243):
a crafting station. The Archive holds no per-player blueprint cap; the only limits of crafting
are the ones these components state. LIVE 4.10 has three, all below
`entities/basebuilding/interactables`; one of them is a `_template` and, like every development
leftover, left out and counted as skipped. The kind is facts only: identifiers and numbers.

| Field | Source |
|-------|--------|
| `guid`, `className`, `recordPath` | The entity Record |
| `type`, `subType`, `size` | `AttachDef.Type`, `AttachDef.SubType` and `AttachDef.Size` of the entity (`Crafter`, `UNDEFINED` and 1 on LIVE); `null` without an `AttachDef`. They are what a ship's crafter port is matched against, see "Ports for crafters" below |
| `maxJobsInHistory` | `maxJobsInHistory`: how many finished jobs the station lists (10); `null` when the value is not a number |
| `baseCraftingTime` | `baseCraftingTime`: a number; `null` when the Record has none, as on LIVE 4.10 |
| `queues` | `queues` in stored order (two on every station of LIVE), `[]` without any |
| `modifiers` | `modifiers` in stored order, `[]` without any (all of LIVE 4.10); see "Modifiers" below |
| `issues` | A per-crafter summary, not per modifier or queue: the codes of what the export did not understand or could not read anywhere in this crafter's fields, modifiers and queues, sorted, without duplicates; `[]` when everything was. The entry itself shows where (`kind` or `type` `unknown`, a `null` or empty value). A closed set the game may extend, so the schema keeps strings. `unknown_modifier_type`, `unknown_modifier_condition` and `unknown_category_availability` mean a struct this export does not know, kept by name. `missing_modifier_type` is a modifier without a type. The `unreadable_*` codes mean a stored value of an unexpected type, see below |

| `hostGuids` | The `ships` entries whose Default Loadout mounts the crafter in any port, at any depth, ordered by GUID; `[]` for a crafter no ship mounts, such as a station. Derived from `loadouts`, so a crafter of a building part has no host |

An absent field (or a null pointer) and a wrong-typed one differ. A field the Record does not
store, such as `baseCraftingTime` before the game adds it, reads as `null` or `[]` and sets no
issue. A field that is stored with another type than expected reads the same way and sets an
`unreadable_*` code:

- `unreadable_base_crafting_time`: `baseCraftingTime` is not a number;
- `unreadable_attach_def`: the `AttachDef`, or its `Type`, `SubType` or `Size`, has another type;
- `unreadable_queues`: `queues` is no array; `unreadable_queue`: a queue is no struct (it stays in
  its position with null values);
- `unreadable_modifiers`: `modifiers` is no array; `unreadable_modifier`: a modifier is no struct
  or its type cannot be read; `unreadable_time_multiplier`: a `time` modifier without a finite
  number;
- `unreadable_process_types`: `processTypeAvailability` is no array or has an entry that is no
  boolean;
- `unreadable_category`: `categories` is no array or has an element that is a Reference to no
  Record.

A queue has `name` (`debugName`), `maxJobsInProgress` (how many jobs it crafts at once, 2 and 4 on
LIVE) and `maxJobsWaiting` (how many further jobs it holds, 4 on LIVE); each is `null` when the
Record stores no number or text. A queue also has the blueprint categories and the process types
it accepts:

| Field | Source |
|-------|--------|
| `blueprintCategoryAvailability` | The struct `blueprintCategoryAvailability` holds, as a rule (see below). `kind` is `whitelist` for a `BlueprintCategoryAvailability_Whitelist`, whose `categories` are the accepted ones; `none` when the Record stores no struct; `unknown` for any other struct, kept with its `structName` and no GUIDs |
| `processTypeAvailability` | `processTypeAvailability` as stored: one boolean per process type, in the order of the game's process types. The Archive names no process type, so a reader takes the position as the identity; `[]` when the Record has none |

A rule (`blueprintCategoryAvailability` of a queue and `condition` of a modifier) has `kind`, the
name of the struct as `structName` (`null` for `none`) and `blueprintCategoryGuids`, the `guid`s of
the Records the struct's `categories` link to, in stored order; these are `blueprintCategories`
entries, unless a Filter left the entry out of that kind. A rule that is `none` or `unknown` has
no GUIDs.

#### Modifiers

A modifier is a `CraftingModifierParams` of the crafter: a change of its crafting time, with the
blueprint categories it applies to.

| Field | Source |
|-------|--------|
| `type` | What it changes: `time` for a `CraftingModifierType_Time`, `none` when the Record stores no `modifierType`, `unknown` for any other struct. A closed set the game may extend, so the schema keeps a string |
| `structName` | The name of the struct `modifierType` holds, `null` for `none`. An unknown type is kept with it and has no values |
| `timeMultiplier` | `timeMultiplier` of a `time` modifier: the factor on the crafting time, below 1 is faster; `null` for any other type or when the Record stores no finite number |
| `condition` | A rule for `modifierCondition`: `kind` is `blueprintCategory` for a `CraftingModifierCondition_BlueprintCategory`, whose `categories` are the blueprint categories the modifier applies to; `none` when the Record stores no condition; `unknown` for any other struct |

A new modifier type adds a `type` value and its value fields, a new condition adds a `kind`; both
are additive and keep `schemaVersion` 2 (ADR-0054). A reader treats an unknown `type` or `kind`
as "not understood" and does not apply the modifier; nothing unknown is dropped from the list.

#### Ports for crafters

A crafter that a ship mounts sits in a `loadouts` port whose `types` hold `Crafter`. To find the
crafters that fit an empty crafter port of a ship, a reader takes the port from the `loadouts`
entry of the ship and keeps the `crafters` entries that fit it:

- A port with several `types` entries takes a crafter that fits any one of them.
- A `types` entry fits when its `type` is the crafter's `type` (`Crafter`) and, when its
  `subTypes` is not empty, the crafter's `subType` is one of `subTypes`.
- The crafter's `size` must lie between the port's `minSize` and `maxSize`; a `null` bound means
  no bound on that side (a crafter port has size 1 on both).

A crafter that is an item has the same `type`, `subType` and `size` in `items`; a crafter that is
not an item (the fabricators below `basebuilding`) is only in `crafters`, which is why the three
fields are part of the entry. A crafter that a ship already mounts is joined the other way round,
`hostGuids` against `ships[].guid` or `itemGuid` of the port against `crafters[].guid`.

The Records are selected from every entity Record: an item, a ship or a commodity that has the
component is a crafter as well and stays what it is, and any other `EntityClassDefinition` under
`entities` is a crafter when it has the component. The selection is structural, never by name or
by folder, so a crafter fitted to a ship or vehicle or part of a building is found like a station.

### Shops

A shop (`shops`) is one `ShopBackup` entry of the shop registry `Data\Libs\CosmoTools\
LayerRecovery\LayerBackups.xml` (a CryXml file), one placement of an `SCShop` Entity in the world
(ADR-0069; a new kind, so `schemaVersion` stays 1). The kind is facts only: identifiers, internal
names and places. It carries no price, no stock and no description; the item GUIDs of a shop come only from an
inventory file that its containers name (below). The names in it
are names of containers and entities, not localisation keys, so no field is a game text.

| Field | Source |
|-------|--------|
| `key` | The `ID` attribute of the entry: the CryGUIDs of the containers from the system root down to the `SCShop` Entity, comma separated, each element in lower case. Unique (1,184 distinct on LIVE) and the identity of the shop |
| `entityGuid` | The last element of `key`, the CryGUID of the `SCShop` Entity, lower case. A shop module placed in several places keeps it (235 distinct for 1,184 shops on LIVE), so it is no identity; it is the CryGUID that `metadata\entitycomponentshop.xml` of the shop container names |
| `system` | `PlatformName` of the `LayerBackup` without the suffix `system`, lower case: `stanton`, `pyro` |
| `entityName` | The last element of `SuperName`, the name of the `SCShop` Entity, e.g. `SCShop_CommEx_TDD_NewBabbage` |
| `registryName` | `ShopName`, e.g. `Pharmacy_RestStop_ARC-LEO1`; `null` when empty. A name, not a localisation key; many shops share one (360 distinct non-empty on LIVE) |
| `defaultValue` | `DefaultValue`, a GUID; `null` when empty. Shops with the same `registryName` share it (1 to 1 on LIVE). What it refers to is not known |
| `containers` | The elements of `SuperName` before the entity, each without its `_{GUID}` or `_{GUID}_socpak` suffix and surrounding blanks, from the system root down |
| `body`, `location` | The second and third element of `containers` (`OOC_Stanton_4_Microtech`, `Area18_City_objectContainer`; Pyro: `pyro5`, `rs_ext_pyro6_l3`); `null` when the chain is shorter |
| `starmapObjectGuid` | The `guid` of the `starmapObjects` entry whose `entityName` or `className` equals `location` ignoring ASCII case; `null` when none does or when more than one object does |
| `kind`, `brand` | From the path of the shop container whose `entitycomponentshop.xml` names `entityGuid`: below `Data\ObjectContainers\PU\Shops` the first folder is the kind (`commex`, `clothing`, ...), the second the brand (`tdd`, `mcraes`), the brand `null` when the file lies directly in the kind folder; a container in a `shopcomm` or `shopfood` folder below `PU\loc` has that folder as kind and no brand. `null` for both when no container names the Entity or containers that differ in kind or brand do. The kind is a closed set the game may extend; a reader maps an unknown value to `null` |
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

| Field | Source |
|-------|--------|
| `inventory` | The lower-case stem of that file, e.g. `inv_admin_newbabbage`; `null` when no place names a file, the places disagree (or only some name one), the file is a placeholder, or the file is missing or unreadable |
| `itemGuids` | The `ID.ID` GUIDs of the elements of the file (`Collection.Inventory`, or `Inventory` in the shape without `Collection`), lower case, ordered, without repeats, and possibly `[]` for a file without items; an `ID` value that is no GUID is left out and counted in `item_ids_dropped`; `null` exactly when `inventory` is. `BuyPrice`, `SellPrice`, `CurrentInventory`, `MaxInventory` and `RentalOfferings` are never read or exported |

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

`p4kx_export::read_crafting(&forge, filter, cancel)` returns a `Crafting` with everything the
`blueprints` kind and the crafting kinds carry; the catalog is built from it, so library and
catalog never drift. It reads the DataForge in parallel (0.3 s on LIVE, the scan for the reward sources included,
against 14 s for the whole catalog) and leaves out a Record it cannot read, listing it under
`failures`. Every list is ordered by Record GUID. The values are facts and identifiers only: no description, no
lore, no media.

The blueprints are selected by the one rule behind the `blueprints` kind (`selects_blueprint`:
prefix, type, Filter, leftover tokens). A `Filter` keeps only the blueprints and pools it matches;
the other lists are always read, and the catalog then keeps the properties, quality distributions
and dismantle processes whose record path matches.

The reward sources come from one scan of the Record tree of every Record for a Reference to a pool
(ADR-0154), so nothing is capped. A Record the scan cannot read is no `failures` entry, because it
is not known to be selected; it could hide a source, so `rewardSources` may be incomplete exactly
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
| `blueprints` | The same set as the `blueprints` kind. Each has `produced_item` and `tiers`: per tier `craft_time_seconds`, `mandatory` and `optional` as a tree of `Cost` |
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

A blueprint's `name` is almost always a placeholder key. The Basetool names a blueprint through
`producedItemGuid`, the item it produces, not through `name`.

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
  suffixes of a display, test, event or reward variant (`_BIS…`, `_Display`, `_TEMP…`, `_Unmanned`,
  `_AI_Super`, `_Tutorial`, `_Collector_…`, `_Prison`, `_NoInterior`, `_Hijacked…`, `_Exec_…` and the
  like) is taken; if more than one is left, the slug stays unmapped;
- a store name that is a part of exactly one catalog name (`M50` for "Origin M50 Interceptor", `C2
  Hercules`, `Mercury`, `Ares Ion`, `85X`, `C8R Pisces`) is accepted after review of each such slug,
  and so is `F7C-M Super Hornet Heartseeker Mk I` for `ANVL_Hornet_F7CM_Heartseeker`, the only ship
  of that name apart from the word "Super".

On `LIVE` 4.7 the 256 slugs of the store (227 flight-ready, 29 in concept) give 205 mapped slugs
and 51 unmapped: the 29 concept ships, 5 whose name fits several equal ships (the four Constellation
Mk IV variants and the Hammerhead, each present once as `_GS` and once without), and 17 flight-ready
ones the Archive holds under another name or not at all (e.g. `Mustang Alpha Vindicator`, `Carrack
w/C8X`, `600i Explorer`, `ATLS`).

Seven of the unmapped slugs have a ship under the store name that the player-only rule excluded,
because its `playability` is `null` (no rule classifies it): `GAMA_Railen`, `GAMA_Tyilui`,
`GLSN_Basher`, `KRIG_S65_Stingray`, `RSI_Skylark`, `RSI_Constellation_Mk5_Centaurus` and
`AEGS_Sabre_Raven_EX`. They can be mapped once their playability is settled. Before ADR-0167, 119 ships with
`playability` `player` have no slug; most are event, reward and display variants of a ship that
has one.

To refresh the file, a human fetches the public ship list of the store, reduces it to the
identifiers above (`id`, name, pledge URL), and adds new slugs to `ship-store-slugs.json` with
`className` and `guid` `null`; the tool never fetches anything. Then `p4kx catalog` on the
current `LIVE` Archive and its `store_slugs.stale` and `unmapped` lists show what is left to do: a
new ship is mapped by the rules above, a mapping that went stale is corrected, or marked
`"retired": true` when the ship is gone. The real-Archive test must pass before the file is
committed.

### Item recovery rules

`itemRecovery` is one object, not a kind and not in `completeness`: the
`ItemRecoveryConfigurationParams` Record `entitlementpolicies/globalitemrecovery` (LIVE has
exactly one), its `economyParams` (ADR-0218, #323). The Record is selected by the rule of a kind:
the type `ItemRecoveryConfigurationParams` under `libs/foundry/records/entitlementpolicies/`, and,
with a Filter, a record path the Filter matches. `itemRecovery` is `null` when no Record is
selected or none can be read. With several readable Records the one with the smallest record path
is taken. A selected Record that cannot be read is left out and listed in the failures of the
summary, like any other Record, so `null` with a failure means an unreadable Record and `null`
without one means there is none. `guid`, `className` and `recordPath` name the Record. Every other
field carries the data name of the `economyParams` field, as the Record states it, as a number;
it is `null` when the Record states none or states a value that is no number:

| Field | LIVE |
|-------|------|
| `globalBrickTimer`, `baseReclaimTime`, `baseDeliveryTime`, `scalingPriceFloor` | 10, 0, 0, 0 |
| `aUECperSecond`, `deliverySecondsPerAUEC`, `expeditedDeliverySecondsPerAUEC` | 1e-6, 1e-6, 5e-7 |
| `claimCostMultiplier`, `maxExpeditedClaimCostMultiplier` | 0.003, 0.0003 |
| `defaultLoadoutCooldownMultiplier`, `defaultLoadoutDeliveryTimeMultiplier`, `defaultLoadoutExpeditedDeliveryTimeMultiplier` | 200, 200, 100 |
| `vehicleChassisRepairCostMultiplier`, `vehicleComponentRepairCostMultiplier`, `vehicleRepairMissingItemCostMultiplier` | 1, 1, 1 |
| `flatMinimumDeliveryTimerSeconds` | 10 |
| `escalatingClaimsParams` | `escalatingClaimChainLimit` 1 and the multipliers `escalationTimerMultiplier`, `escalatingClaimCostMultiplier`, `escalatingDeliveryTimerMultiplier`, all 1 |
| `overrides` | 7 groups, see below |

An override (`overrides[]`, an `ItemRecoveryOverrideGroupDef`) holds `classGuids`, the `guid` of
each `EntityClassDefinition` in the group's `classes` (a ship is an entry of `ships`; on LIVE the 7
groups hold 27, 7, 9, 29, 7, 10 and 10 classes, 99 in all, every one a ship of the catalog; a
Reference that links to no Record is left out), and the group's multipliers
`claimCooldownMultiplier`, `claimCostMultiplier`, `deliveryTimeMultiplier`,
`maxExpediteCostMultiplier`, `expeditedDeliveryTimeMultiplier`,
`flatMinimumDeliveryTimerSecondsMultiplier`, `escalationTimerMultiplier`,
`escalatingClaimCostMultiplier` and `escalatingDeliveryTimerMultiplier`. The groups keep the order
of the Record. That a class in no group uses the defaults of `economyParams` is an inference; the
Record does not say so.

No fee in aUEC. The Record holds multipliers and rates and no fee in aUEC, and no field named
like a price was found in the `EntityClassDefinition` Records of ships and items. The only fee in
aUEC next to it is `insurance.expediteFee`. The catalog therefore exports no claim fee, and no
formula over the exported facts is documented as one (ADR-0218).

### Texts

`key` is the localisation key as the Record holds it, e.g. `@manufacturer_NameACAS`; an empty key
is `null`. `en` and `de` are its texts from the Archive's `global.ini` of that language:

| Language | Entry |
|----------|-------|
| `en` | `Data\Localization\english\global.ini` |
| `de` | `Data\Localization\german_(germany)\global.ini` |

Each line `key=value` of a `global.ini` holds one text. Keys match without the leading `@` and
ignoring case; a suffix after a comma (`ASD_01,P`) is not part of the key. When a `global.ini`
holds both `key=` and `key,P=`, the lookup keeps both lines and uses the text of the plain key,
unless that is empty and the suffixed one is not (the Archive's LIVE `global.ini` files hold no
such pair; English has 13,504 `,P` keys, all without a plain twin). The text is the rest of
the line as the game stores it, except that the escape `\n` becomes a line feed; markup, a lone
carriage return and every other sequence stay as they are.

A `global.ini` is read as UTF-8 with or without a byte order mark, or as UTF-16. Bytes that are no
valid text become U+FFFD and lines without `=` are skipped, so a damaged line costs only itself;
`p4kx catalog` names every such Entry in a `note:` line on stderr and the catalog stays as it is.
Only a `global.ini` over 64 MiB or with more than 1,048,576 keys fails the catalog, with exit
code 3.

A key the language's `global.ini` lacks or holds with an empty text (`key=`), or a language the
Archive holds no `global.ini` for, leaves that text `null` and keeps the key, so the basetool can
resolve it later; it counts as unresolved.

The placeholder keys `@LOC_EMPTY`, `@LOC_PLACEHOLDER`, `@LOC_UNINITIALIZED` and `@blank_space`
(the game's filler for an empty line, which Records use 372 times; ignoring case,
`PLACEHOLDER_KEYS` in `crates/p4kx-localization/src/lookup.rs`) mean "no text": the key stays,
`en` and `de` are `null` even where the `global.ini` holds a text for them such as
`<= PLACEHOLDER =>`, and they count as placeholders, neither resolved nor unresolved.

Leading and trailing white space, line breaks included, is removed from every text, after the
`\n` escapes are decoded (`display_text` in `crates/p4kx-localization/src/lookup.rs`), so
`Antium Arms Moss Camo ` becomes `Antium Arms Moss Camo`. A text that is white space only counts as
unresolved. White space inside a text stays. The trim is the catalog's and the window's alone: Record
XML and JSON, `p4kx loc` and `dcb export --lang` keep the text byte for byte as the `global.ini`
stores it. It does not change the catalog's determinism: the same Archive gives the same bytes.

`p4kx catalog` counts the resolved, unresolved and placeholder texts per language in its summary.

## Selection

| Kind | Record-path prefix | Type |
|------|--------------------|------|
| `manufacturers` | `libs/foundry/records/scitemmanufacturer/` | `SCItemManufacturer` |
| `items` | `libs/foundry/records/entities/scitem/`, `libs/foundry/records/entities/legacyitems/` | `EntityClassDefinition` |
| `ships` | `libs/foundry/records/entities/spaceships/`, `libs/foundry/records/entities/groundvehicles/` | `EntityClassDefinition` |
| `commodities` | `libs/foundry/records/entities/commodities/` | `EntityClassDefinition` |
| `blueprints` | `libs/foundry/records/crafting/blueprints/crafting/` | `CraftingBlueprintRecord` |
| `resourceGroups` | `libs/foundry/records/resourcetypedatabase/` | `ResourceTypeGroup` |
| `blueprintCategories` | `libs/foundry/records/crafting/blueprintcategories/` | `BlueprintCategoryRecord` |
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
| `subHarvestableMultiConfigs` | `libs/foundry/records/harvestable/slotpresets/` | `SubHarvestableMultiConfigRecord` |
| `harvestableLocations` | None: the `.soc` files of the containers below `ObjectContainers/PU/system`, no Records | |
| `shops` | None: the `ShopBackup` entries of `LayerBackups.xml`, no Records | |
| `factionReputations` | `libs/foundry/records/factions/factionreputation/` | `FactionReputation` |
| `reputationScopes` | `libs/foundry/records/reputation/scopes/` | `SReputationScopeParams` |
| `reputationStandings` | `libs/foundry/records/reputation/standings/` | `SReputationStandingParams` |
| `reputationContexts` | `libs/foundry/records/reputation/contexts/` | `SReputationContextUI` |
| `missionTypes` | `libs/foundry/records/missiontype/` | `MissionType` |
| `missionLocalities` | `libs/foundry/records/missiondata/pu_missionlocality/` | `MissionLocality` |
| `contractScenarios` | `libs/foundry/records/contracts/contractscenarios/` | `ScenarioProgress` |
| `cargoManifests` | `libs/foundry/records/cargomanifest/` | `CargoManifest` |
| `missionLocationTemplates` | `libs/foundry/records/missiondata/pu_locations/` | `MissionLocationTemplate` |
| `contractShipSpawns` | None: derived from the properties of the contracts and their templates, no Records | |
| `contractLocationQueries` | None: derived from the properties of the contracts and their templates, no Records | |
| `contractNpcSpawns` | None: derived from the properties of the contracts, templates and broker entries, no Records | |
| `contractEntitySpawns` | None: derived from the properties of the contracts, templates and broker entries, no Records | |
| `contractTemplates` | `libs/foundry/records/` (every folder; see "Contract templates") | `ContractTemplate` |
| `missionBrokerEntries` | `libs/foundry/records/` (every folder; see "Mission broker entries") | `MissionBrokerEntry` |
| `lootTables` | `libs/foundry/records/lootgeneration/loottables/` | `LootTableV3Record` |
| `lootArchetypes` | `libs/foundry/records/lootgeneration/lootarchetypes/` | `LootArchetypeV3Record` |
| `lootPoolFilters` | `libs/foundry/records/lootgeneration/filters/` | `PoolFilterRecord` |
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

A selected Record that cannot be read, such as one whose struct layout is corrupt, does not stop
the catalog: it is left out, counted as a failure and reported with its record path, GUID and
reason (a ship whose loadout cannot be resolved is the exception: it stays in `ships`, see
"Loadouts"). `p4kx catalog` prints each failure on stderr, lists them under `failed` in `--json` and
exits with code 1, with the catalog written; the desktop app lists them in the summary. The
catalog file itself, its schema and its completeness are not affected.

## Determinism and size

Equal Archives and an equal export time give byte-identical catalogs. The export time is the
current time unless `--exported-at` or the `SOURCE_DATE_EPOCH` environment variable gives it in
seconds since 1970-01-01 UTC.

### Upload limit

`UPLOAD_LIMIT` in `crates/p4kx-export/src/summary.rs` is the largest catalog file the basetool
accepts, 64,000,000 bytes (64 MB), compressed if it is; `DECOMPRESSED_LIMIT` is the cap of the
decompressed content, 1,000,000,000 bytes (1 GB). They must match the basetool's limits: when one
changes, the other changes with it. The owner kept the upload limit at 64 MB after the first
measurement (21.6 MB). The crafting detail (all tiers, quality modifiers, pools and their reward
sources) grew the largest plain catalog to 26.6 MB, 41 % of the upload limit; compressed it is a
small fraction of it. The `loadouts` kind (ADR-0064) then grew the plain catalog of LIVE to
132 MB (153 MB by 2026-10-26), which is **over the 64 MB upload limit: the upload must be
`.json.zst`** (2.7 MB), and
plain `.json` is for inspection only. `p4kx catalog` and the window warn about a plain file over
the limit and advise `.json.zst`. The real-Archive tests write the catalog as `.json.zst`, assert
that the compressed size is within 64 MB and the decompressed size within 1 GB, decode the frame
and compare it with the plain catalog, and print both sizes and the time per zstd level.

Measured on LIVE on 2026-10-26 (complete catalog of every kind, English and German texts; Bytes
is the whole catalog, the other kinds are in `p4kx catalog --json`):

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

Level 19 stays: 12.7 s is acceptable and it saves 15 % over level 15.

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
- Every `manufacturerGuid` of an item or ship, every `producedItemGuid`, every ingredient
  `resourceGuid` and `itemGuid` resolves to a catalog entry; no GUID occurs twice.
- Every commodity `typeGuid` resolves to a `resourceGroups` entry, every `parentGuid` of a group to
  another one, and every blueprint `categoryGuid` to a `blueprintCategories` entry. On LIVE the
  catalog holds 19 groups (8 of them nested) and 21 categories, 14 of which carry blueprints.
- Texts that carry a key (placeholder keys excluded), checked per kind against the Archive's own
  `global.ini` files (ADR-0270): a key a file holds resolves in the catalog, a key it lacks is
  `null` and the Basetool falls back to `en`. The verification prints the numbers per kind and
  holds each kind to a baseline of the share it may lack: the share measured on LIVE, rounded up to
  the hundredth. LIVE, every `name` and every game text
  field (`x-gameText`) that carries a key, wherever it sits in an entry:

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

  A contract text that holds a `~mission(…)` token is rewritten by the exporter, so the
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
