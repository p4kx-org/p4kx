# scunpacked-data comparison

Research date: 2026-11-06.

P4KX already covers most of the reference dataset's major domains. The strongest opportunities
are additional raw fields in the typed catalog, new relationships between existing entities,
and an export-health contract that survives transfer of the catalog file. This investigation
found no conclusively demonstrated new P4KX parsing bug. It did establish the export-health
problem below by source inspection and identify concrete typed-export omissions.

## Scope and reproducibility

| Source | Inspected revision |
| --- | --- |
| Local P4KX source | `1ee9c0edbfc34e616d2f140951e4eb5bc1c67558`, version 0.4.2; working tree initially clean |
| [scunpacked-data](https://github.com/StarCitizenWiki/scunpacked-data/commit/1340c8303709904165e2da0fa7060bce0624260b) | `1340c8303709904165e2da0fa7060bce0624260b`, 2026-10-28; commit identifies `4.10.2-LIVE.12881860` |
| [ScDataDumper](https://github.com/octfx/ScDataDumper/commit/120a728639bdd1b06525a0f8574cb0e91d0fbf8d) | `120a728639bdd1b06525a0f8574cb0e91d0fbf8d`; inspected to explain source paths and transformations, not proven to be the exact generator revision of the data commit |
| Installed LIVE manifest | `RequestedP4ChangeNum=12913470`, branch `sc-alpha-4.10.2`; differs from the reference Build |

This is a source and representative-schema comparison, not an exhaustive numeric comparison
of two exports of the same Build. No build or test suite was run. The installed executables
were older than the inspected source, so they were not used to establish current behavior.
No external assets, game texts or value dumps were saved into the repository.

## Existing coverage

The catalog contains 47 selected or derived `Kind` variants, plus curated store slugs and global
crafting/recovery objects. Its coverage already includes manufacturers, items, ships, loadouts,
paints, commodities, all blueprint tiers, crafters, dismantling, mission contracts/templates,
broker entries, spawn descriptions, reputation, mining, refining, harvesting, loot, shops and
the starmap. These are not new export proposals.
Sources: [selection](../crates/p4kx-export/src/selection.rs#L159),
[catalog contract](../crates/p4kx-export/src/contract.rs#L60),
[catalog documentation](basetool-catalog.md).

There are two different coverage questions:

- Generic Record XML/JSON/NDJSON and flat exports can already expose fields outside the catalog.
  Queries, Reference traversal, field statistics and tag discovery make these accessible today.
  Sources: [commands](commands.md#commands), [Query language](query.md),
  [Record JSON](record-json.md).
- The typed catalog intentionally projects a stable, smaller set of fields. The opportunities
  below concern that consumer-facing contract, unless explicitly stated otherwise.
  [ADR-0065](adr/0065-item-stat-blocks-are-raw-facts-without-derived-values.md) reserves derived
  combat and flight formulas for consumers.

## 1. Preserve export health in the delivered artifact

**Classification: confirmed contract/robustness problem; downstream deletion not reproduced.**

A failed selected Record can be absent from a written catalog while its kind remains
`completeness.status=complete`. The failure list and warnings are returned separately in the
summary. This behavior is deliberate: ADR-0320 defines completeness as selection scope rather
than successful reading. At the same time, the general catalog documentation allows an importer
to treat a missing Record in a complete kind as removed. A catalog transferred without its
summary cannot distinguish those situations.

Evidence:

1. [`build.rs:238`](../crates/p4kx-export/src/build.rs#L238) initializes all kinds from
   [`completeness_of`](../crates/p4kx-export/src/build.rs#L354), which checks only the Filter.
   Failures and warnings go into the separate summary at
   [`build.rs:306`](../crates/p4kx-export/src/build.rs#L306).
2. The existing [crafting-depth fixture test](../crates/p4kx-export/tests/it/crafting_depth.rs#L53)
   constructs an unfiltered catalog with a selected blueprint omitted after a reading failure.
   It checks the omission and failed summary, but not the JSON completeness field.
   Combined with the builder, the misleading standalone artifact is statically established;
   this test was inspected, not executed here.
3. [CLI writing](../crates/p4kx/src/catalog.rs#L92) commits the file before the
   [summary controls the exit code](../crates/p4kx/src/catalog.rs#L61).
   The [GUI job](../crates/p4kx-gui/src/catalog_export/job.rs#L271) also commits the artifact
   without a successful-summary gate.
4. The [general guarantee](basetool-catalog.md#shape) and the explicit failure behavior
   are documented in `docs/basetool-catalog.md` at lines 200 and 2528.
   [ADR-0320](adr/0320-mission-broker-entries-are-a-catalog-kind-selected-by-their-struct-and-an-unknown-struct-fails-the-record.md#consequences)
   expressly requires consumers to check the separate failures.

Recommended follow-up: define an artifact-carried `exportHealth` with per-kind reading status,
failed GUIDs and relevant warnings, and update the consumer to require it before removal
detection. Adding metadata is compatible with schema v2, but old consumers ignore new fields.
A changed completeness meaning or stronger mandatory import guarantee needs a coordinated
schema version change under [ADR-0054](adr/0054-the-basetool-catalog-grows-additively-within-version-1.md).
Consider keeping the normal target file unchanged on partial failure and writing diagnostic
partial exports only through an explicit workflow. Do not silently change existing semantics.

## 2. Complete the raw fire-mode inputs

**Classification: verified typed-export gaps; high-value first implementation.**

| Missing raw inputs | Reference sample | Consumer impact |
| --- | --- | --- |
| Burst `shotCount` | [P8-SC](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/behr_smg_ballistic_01.json) | Fire rate and cooldown are present locally, but the burst size cannot be recovered from the typed block. |
| `maxChargeModifier` damage, projectile-speed and ammo-cost multipliers; `interpolateChargeBonus`; automatic/full-charge firing flags; `overchargedTime` | [Volt sniper](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/volt_sniper_energy_01.json) | Charged damage and ammunition requirements need inputs that the catalog omits. |
| `spinUpTime`, `spinDownTime`, `fireDuringSpinUp` | [Revenant](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/apar_ballisticgatling_s4.json) | Start-up behavior cannot be modeled from the exported steady fire rate. |
| Sequence/parallel parent grouping and sequencing metadata | The existing [collector](../crates/p4kx-export/src/item_stats/weapon.rs#L16) deliberately flattens actions. | A fuller future action model needs structure in addition to leaf fields. Preserve the existing flat field for compatibility. |

Local sources: [fire-mode reader](../crates/p4kx-export/src/item_stats/weapon.rs#L35),
[`FireAction` contract](../crates/p4kx-export/src/contract_stats.rs#L124).

`overchargeTime` and `overchargedTime` are separate source fields. Their difference is not evidence
of a spelling bug. Exporting these inputs does not require a P4KX DPS formula and fits ADR-0065.
Use additive nullable fields and synthetic fixtures for burst, charged and spinning actions.

## 3. Expand component statistics

**Classification: verified typed-export gaps.**

| Area | Additional facts visible in the reference | P4KX seam |
| --- | --- | --- |
| Shields | Resistance/absorption arrays, reserve-pool ratios, stun parameters, electrical-charge resistance | [Rhada sample](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/shld_behr_s01_5sa_scitem.json); [current shield reader](../crates/p4kx-export/src/item_stats/parts.rs#L34) exports health, regeneration, decay and delays. |
| Quantum drives | Separate `splineJumpParams`, calibration parameters, interdiction effect and heat parameters | [Expedition sample](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/qdrv_tars_s01_expedition_scitem.json); [current reader](../crates/p4kx-export/src/item_stats/parts.rs#L47) reads its flight values only from `params`. |
| Missiles | Lifetime/arming/ignition, signal thresholds and resilience, fuel, guidance-phase acceleration/rotation | [Ignite II sample](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/misl_s02_ir_fski_ignite.json); [current reader](../crates/p4kx-export/src/item_stats/parts.rs#L65). |
| Resources and durability | Minimum consumption, power ranges, self-repair, conversion pairing, damage resistances and repair/salvage flags | [Rhada sample](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/shld_behr_s01_5sa_scitem.json); [resource projection](../crates/p4kx-export/src/item_stats/resources.rs#L20), [health reader](../crates/p4kx-export/src/item_stats/parts.rs#L84). |

Keep resource unit structs and source states. Do not equate a stored power segment with another
unit. The current flow projection separates generation and consumption; a richer representation
can preserve the original delta grouping alongside the existing fields.

Items with only health/resource components currently receive `stats:null` unless they have a
recognized specialist block. That gate is explicit in
[`item_stats/mod.rs`](../crates/p4kx-export/src/item_stats/mod.rs#L48) and ADR-0065.
Making shared durability/resource facts generally available is a scope extension, not a fix to
an accidental parser failure.

## 4. Add flight, fuel and vehicle-armour blocks

**Classification: verified typed-export gaps; separate from the existing suit-armour block.**

- [Avenger flight controller](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/controller_flight_aegs_avenger_titan.json):
  IFCS speed limits, angular limits, boost/afterburner parameters and distinct parameter sets.
- [Avenger hydrogen tank](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/htnk_aegs_avenger_titan.json):
  typed resource capacity, resource References and fuel-flow multipliers.
- [Avenger armour item](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/armr_aegs_avenger_titan.json):
  damage/signature multipliers, penetration resistance and deflection.

Start with component-item blocks; join the ship through the existing resolved loadout rather
than duplicate traversal. The current [Ship contract](../crates/p4kx-export/src/contract_entities.rs#L89)
and [Ship base data](ship-base-data.md) cover hull/cargo/loadout/insurance rather than these
operating properties. Preserve the stored distinction between old/new IFCS parameter sets.
A component's maximum speed is not by itself a measured ship performance guarantee.

## 5. Additional domain exports worth surveying

These are distinct from another generic Record dump.

| Candidate | Evidence and concrete boundary |
| --- | --- |
| Commodity packaging and volatility facts | [`resources/commodities.json`](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/resources/commodities.json) exposes default cargo-container References, validation flags and volatility/decay. These are absent from the [current commodity reader](../crates/p4kx-export/src/commodity.rs#L50). Instability and resistance are already exported per [MineableElement](../crates/p4kx-export/src/mining.rs#L82), linked by ResourceType; they are not missing raw data. The reference generator folds them into a commodity through a ResourceType-keyed index, while [P4KX preserves multiple elements per resource](../crates/p4kx-export/src/contract_mining.rs#L10). |
| Inventory occupancy and container details | [Hat sample](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/987_hat_01_01_01.json) includes occupancy, UI dimensions, overrides and interactions. Export stored bounds/dimensions/units separately; an inventory box is not a verified hull dimension. |
| Stable global tags and Entity-tag GUIDs in the catalog | [`tags.json`](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/tags.json) and the [hat sample](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/items/987_hat_01_01_01.json) preserve hierarchy and Entity tags. [`Item.tags`](../crates/p4kx-export/src/item.rs#L54) contains AttachDef words. Generic tag discovery already exists; the proposal is catalog identity/relationships, not a new tag parser. |
| Faction behavior and legal authority | [Hurston Security](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/factions/faction_lawfulsecurity_hurstonsecurity.json) exposes reaction/type/arrest/policing/legal-right flags. P4KX has [FactionReputation](../crates/p4kx-export/src/contract_reputation.rs#L16), but no general Faction kind; existing mission modifier/scenario references could resolve to it. |
| Navigation semantics | [Starmap dataset](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/starmap.json) exposes type flags, jurisdiction, affiliation, amenities and other navigation facts. [P4KX starmap](../crates/p4kx-export/src/contract_starmap.rs#L16) currently retains identity/type/parent/hide/placement. Read `validQuantumTravelDestination` from the type Record; do not infer release availability from it. |
| Jump-point facts and connections | [Position dataset](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/starmap_positions.json) carries connections; [`JumpPointPermanent`](https://github.com/octfx/ScDataDumper/blob/120a728639bdd1b06525a0f8574cb0e91d0fbf8d/src/DocumentTypes/JumpPointPermanent.php) identifies stored fuel/size/linking parameters. Expose raw parameters first; establish connection identity from archive evidence before adopting name pairing. |
| Ship facilities and topology | [Avenger ship](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/ships/aegs_avenger_titan.json) includes seats/beds, individual inventory/cargo/storage containers and relay links. Prefer declared ports, References and `SInternalHardpointLink` edges to inferred seat roles or fuse counts. |
| Food/medical/consumable effects | The generator contains [Food](https://github.com/octfx/ScDataDumper/blob/120a728639bdd1b06525a0f8574cb0e91d0fbf8d/src/Formats/ScUnpacked/Food.php) and [Medical](https://github.com/octfx/ScDataDumper/blob/120a728639bdd1b06525a0f8574cb0e91d0fbf8d/src/Formats/ScUnpacked/Medical.php) projections. [P4KX coverage](coverage.md) already discovers consumable components, but [ItemStats](../crates/p4kx-export/src/contract_stats.rs#L23) has no corresponding typed block. This candidate still needs representative current-Build Record sampling before a field contract is fixed. |

The inspected 0.4.2 baseline omitted offline shop prices, stock and rental offerings.
[Catalog v3](basetool-catalog.md#physical-offline-shop-inventories) exports these values
with their physical source file and offline design-time meaning. Live market prices,
server stock and rental availability are outside that projection.

## 6. Reference behavior that should not be copied

- **Starmap positions:** [`LoadStarmap.php:397`](https://github.com/octfx/ScDataDumper/blob/120a728639bdd1b06525a0f8574cb0e91d0fbf8d/src/Commands/LoadStarmap.php#L397)
  adds parent and child positions without composing rotations and caps external recursion at
  three levels. P4KX already composes rotations and handles ambiguity; preserve that behavior
  ([starmap documentation](basetool-catalog.md#starmap-objects)).
  Different repeated placements also mean raw entity counts are not comparable to unique GUID counts.
- **Jump connections:** the generator [pairs endpoint names](https://github.com/octfx/ScDataDumper/blob/120a728639bdd1b06525a0f8574cb0e91d0fbf8d/src/Commands/LoadStarmap.php#L539)
  with a regular expression and normalizes a global fuel parameter. This is not proof of a stored
  per-route connection record. Its missing-lookup QT default is also not evidence of travel validity.
- **Trade locations:** [`trade_locations.json`](https://github.com/StarCitizenWiki/scunpacked-data/blob/1340c8303709904165e2da0fa7060bce0624260b/trade_locations.json)
  describes disabled flags and produce/consume tags, not actual shop offers or prices.
  P4KX already exports the corresponding
  [MissionLocationTemplate conditions](../crates/p4kx-export/src/location_template.rs#L26).
  The [trade resolver](https://github.com/octfx/ScDataDumper/blob/120a728639bdd1b06525a0f8574cb0e91d0fbf8d/src/Services/Resource/CommodityTradeLocationResolver.php#L58)
  matches normalized tag names and ancestors; its positive-tag paths do not evaluate negatives,
  and the resolver reads `disabled` without filtering on it. Treat resulting SoldAt/BoughtAt
  relationships as heuristic candidates, not verified active sales. The aggregate
  `commodity_trade_locations.json` was not exhaustively inspected.
- **Curated and derived values:** [ship manufacturer resolution](https://github.com/octfx/ScDataDumper/blob/120a728639bdd1b06525a0f8574cb0e91d0fbf8d/src/Formats/ScUnpacked/Ship.php#L81)
  can consult overrides. [Occupancy](https://github.com/octfx/ScDataDumper/blob/120a728639bdd1b06525a0f8574cb0e91d0fbf8d/src/Formats/ScUnpacked/InventoryOccupancy.php),
  [seating](https://github.com/octfx/ScDataDumper/blob/120a728639bdd1b06525a0f8574cb0e91d0fbf8d/src/Services/Vehicle/SeatingAnalyzer.php)
  and [relay formatting](https://github.com/octfx/ScDataDumper/blob/120a728639bdd1b06525a0f8574cb0e91d0fbf8d/src/Formats/ScUnpacked/RelayNetwork.php)
  contain normalization or naming heuristics. Compare owning raw fields, not all formatted values.
  Retain P4KX source units, nullable unknowns and distinction between inventory bounds and hull size.

## Suggested implementation order and verification

1. Agree the export-health/import-removal contract and capture the existing partial-failure
   case in an artifact-plus-consumer test. This is a coordinated contract task, not a silent
   change to `complete`.
2. Add the small missing fire-mode inputs, then shield/quantum/missile raw parameters.
   Use synthetic fixtures, generated-schema checks and nullable/unknown-value cases.
3. Add flight/fuel/vehicle-armour component blocks and join them through current loadouts.
4. Add commodity volatility/packaging and Faction/navigation facts.
5. Survey consumables, individual facilities, inventory occupancy and relay topology before
   deciding their exact typed shapes.

For an independent regression comparison, extend the existing
[reference-test approach](differential-verification.md), rather than add a network dependency
to the shipped application. A reference adapter should pin data revision and require the same
channel/change number as the Archive. Join by GUID and retain the kind selection rules and
documented leftover exclusions.

Compare raw paths and units, reporting matched, changed, reference-only, P4KX-only and
uncomparable fields separately. Missing on one or both sides must never count as equal.
Keep explicit reasons for normalized units, derived values and curated overrides.
Test comparator logic using synthetic data; keep external reference/game files outside the
tracked repository and CI. The installed Build mismatch currently prevents claiming any
same-Build numerical regression result.
