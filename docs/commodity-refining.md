# Commodity refining in the DataForge
Catalog schema 3 keeps source resource definitions in `definitions.resourceTypes` and Commodity
choices in `projections.commodityPresentation`. Fields below describe the presentation's `value`
payloads; raw units, exact numbers and authored references retain their own ResourceType owners.
See the [canonical Commodity contract](catalog-commodity-types.md).

Result of the spike #397 against the LIVE Archive: what the DataForge holds of the raw to refined
relation, density, instability, counting in SCU or units, and refining methods and yields. Counts
are Records of LIVE 4.10.197; the `ResourceType` Records number 207 and the `ResourceTypeGroup` Records 19.
Only facts are used (no description, no media), as the legal review of 2026-10-20 allows.

| Attribute | Found | Where | Coverage on LIVE |
|-----------|:-----:|-------|------------------|
| Raw to refined relation | yes | `ResourceType.refinedVersion`, a Reference to the refined `ResourceType` | 30 of 207 `ResourceType` Records; the other 177 are empty (refined, unrefinable or unrelated) |
| Density | yes | `ResourceType.densityType` (`ResourceTypeDensity`) `.densityUnit` (`GramsPerCubicCentimeter`) `.gramsPerCubicCentimeter` | 207 of 207, from 0.25 to 20.45 g/cm3 |
| Instability | partly | `MineableElement.elementInstability`, linked to a `ResourceType` by `resourceType` | 46 `MineableElement` Records under `mining/mineableelements`, values 0 to 1000, 42 distinct `ResourceType`s |
| Counting in SCU or units | no flag | see below | not a field |
| Refining methods | yes, as speed and quality only | `RefiningProcess` Records under `refiningprocess` | 9 Records |
| Refining yields | no | none, also per station ([searched](#per-station-yield-profiles)) | none |

## Raw to refined

All 30 targets of `refinedVersion` resolve to a `ResourceType`; none refines into itself and no
target has a `refinedVersion` of its own, so the relation is one step. One refined `ResourceType`
is the target of three raw ones. The catalog
carries it as `refinedSubtypeGuid` on the commodity of the raw `ResourceType`; the refined
commodity is the one whose `subtypeGuid` equals it, when the Archive has such a commodity.

## Density

Every `ResourceType` states the density in one unit only, `GramsPerCubicCentimeter`; the catalog
carries it as `densityGramsPerCubicCentimetre`. The Wiki states other units for some commodities;
a Basetool conversion is its own decision.

## Instability

Only the `MineableElement` Record holds `elementInstability` (with resistance and the optimal
window), so it belongs to a mineable element, not to the commodity. 27 of the 46 elements link to
a `ResourceType` that has a `refinedVersion`. Several elements may share a `ResourceType`, which
makes a flat commodity field ambiguous, so the catalog carries no such field. The kind
`mineableElements` exports every element with its `resourceTypeGuid` and the Basetool applies its
own rule (ADR-0065, #460). One `ResourceType` carries `properties/ResourceTypeVolatility` (1 Record,
with a `healthDecayPerSecond`), a decay of cargo, not the instability of a rock. The mining laser
`laserInstability` modifiers (names matching `*nstab*`) are item stats, not commodity data.

## SCU or units

No `ResourceType` or commodity field says whether the commodity is counted in SCU or in pieces.
Two indirect signals exist and are not a flag, so they are not exported: `ResourceContainer.capacity`
is a `SStandardCargoUnit` (1,842 of 2,336 containers), `SMicroCargoUnit` (379) or `SCentiCargoUnit`
(115), and 195 `ResourceType` Records list carryable boxes per SCU size in `defaultCargoContainers`.

## Refining methods and yields

`RefiningProcess` Records have `refiningSpeed` (`Slow`, `Normal`, `Fast`), `refiningQuality`
(`Wasteful`, `Normal`, `Careful`) and `processName`, a localisation key. Nine of them exist, one
for every speed and quality pair. They identify a method by GUID and record path. The DataForge
holds no method names of the UEX kind, no yield per commodity and method, and no price: the only
number near it is a `refiningQualityUnitMultiplier` on one Record. Refinery yields stay with UEX.

## Per-station yield profiles

Third-party databases list refinery yield profiles per station: an ore mapped to a signed integer
bonus, with one profile shared by several refineries. The LIVE Archive 4.10.197 holds no such
table, and the Basetool catalog carries none. Only the LIVE channel was searched; the other
channels were not. Result: none found. What was searched, with the commands (`<Data.p4k>` is the
LIVE `Data.p4k`):

```text
p4kx search <Data.p4k> refin
p4kx search <Data.p4k> yield
p4kx grep <Data.p4k> yield --in records --limit 40 --per-item 1
p4kx grep <Data.p4k> refinery --in records --limit 100 --per-item 1
p4kx grep <Data.p4k> Hephaestanite --in entries --limit 50
```

- Records (`p4kx grep --in records`): the 7 Records that mention `yield` are UI (`RefineryMFD_*`
  labels, a salvage and a structural HUD binding), none holds ore bonuses. The 46
  `MissionLocationTemplate` and 7 `StarMapObject` Records that mention a refinery are locations
  (`starmap/pu/station/reststop/rr_*` states "Also features a Refinery" in its description;
  `starmap/pu/station/refinery/refinery_0001` is a placeholder named "WIP Refinery_0001"). The only
  other Records are the 9 `RefiningProcess` Records, one `SCItemManufacturer` (`shop_refinery`) and
  UI audio and popup Records.
- Text and CryXml Entries (`p4kx grep --in entries`): the ore name (`Hephaestanite`) appears only
  in the tag database, `Libs/RoboTools/RoboTrucker/ItemMarks.xml`, `global.ini`, a UI icon
  material and `Scripts/Loadouts/Player/DEV/Inventory/ThumbnailEntities.xml`; no file lists an ore
  beside a station or a bonus. `p4kx search` for `refin` finds meshes, textures, audio, UI movies,
  prefabs and loadouts, and for `yield` nothing; no data file holds a yield table.
- The station names are in the Archive (`StarMapObject.name`, localisation keys `RR_*_L*`), the
  profiles are not found. The Archive gives no source for them, so the catalog cannot carry them
  and no catalog kind is added.

## Follow-up

- Catalog fields added with this spike: `densityGramsPerCubicCentimetre`, `refinedSubtypeGuid`.
- Catalog kinds added by #460: `mineableElements` (instability and the other plain numbers per
  element, no pick per commodity, ADR-0065) and `refiningProcesses` (the 9 Records).
