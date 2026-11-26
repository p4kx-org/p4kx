# Reduced Basetool Catalog scope

The field-selection workbook marked 76 rows for inclusion and 157 for omission.
Five selected rows are containers for selected child tables. The field sheet has
no individual overrides. The dataset notes narrow the retained fields below.
This selection was made against Catalog v3 at `6f3cda47`. Validate the source path
for each field against the release candidate before fixing the reduced schema.

## Envelope, text and completeness

- Retain `schemaVersion`, `source`, `exportHealth` and `completeness`. The
  `source` block includes the Channel, Build (including change number and public
  patch when known), tool version, export timestamp, key access and unread
  encrypted Entry count. Health assesses the source closure needed for this
  export. An incomplete selected source can block publication.
- For every published dataset, retain the selection status and filter needed
  to validate import and decide whether to remove absent rows. Report the exact
  selected populations. Ship-only Loadouts must carry that scope explicitly.
- Preserve canonical order and stable identities. Keep physical Record indexes
  where an emitted relationship uses them. Compute selected projections with
  their canonical inputs, including inputs whose datasets are omitted.
- Publish only referenced `texts.publicNames` and `texts.gameTexts` in English
  and German. Preserve protected game-text classification, typed media
  references where selected and `containsEncryptedSource`'s three states.
- Keep canonical sourced `value`, `state` and `containsEncryptedSource` cells.
  Omit the detailed proof inventory, source-use lists and closure flags.
  Preserve the declared numeric value and unit. Omit raw float bits and keep
  non-finite data distinguishable from a real finite value.
- Omit raw `authored` and `additionalFields` trees, `originalError` payloads
  and `sources` / `source.roots` arenas. Keep typed field states and loadout
  resolution issues. Any selected link into an omitted arena needs a typed,
  resolvable reduced reference.

## Top-level data

| Dataset | Retained scope |
| --- | --- |
| `items` | GUID, class name, Record path, item class, playability and variant link. |
| `ships` | GUID, class name, Record path, name, description, role, career, crew, mass, hull health, size, vehicle kind, playability including ATLS, variant, manufacturer and insurance restore, delay, expedite and ship entity fields. |
| `loadouts` | Ship and vehicle roots with recursively resolved installed-part slots. Include names and display keys, accepted types and subtypes, sizes, flags, tags and required tags, resolved item GUID and class, source, unresolved state and issues. Exclude standalone Item roots and `sourceConstraints`. |
| `paints` | Item GUID and associated Ship GUIDs. |
| `shipStorePledges` | Snapshot identity, RSI ID, slug, store name, manufacturer code, production status and the other pledge fields. |
| `shipStoreSlugs` | Slug, RSI ID, store name, class name, Ship GUID and retirement state. |
| `shops` | Shop key, entity and starmap identities, system, entity and registry names, location, kind, brand, commodity exchange, inventory and container links. Omit `body` and `defaultValue`. |
| `contractScenarios` | Reputation faction and reward tiers with Blueprint Pool links. |

## Configurations

| Dataset | Retained scope |
| --- | --- |
| `attachableModifiers` | Salvage radius and speed multipliers and extraction efficiency. |
| `blueprintTiers` | Every tier, craft time, required and optional material costs, quantity distinctions, resource and item links, count, quality, options, slot debug name, quantity context, modifier property GUIDs and ranges. Research and unlock data are optional. |
| `cargoLoadingGlobals` | Base loading and unloading times, box-size times by SCU and cargo-deck loading time per SCU. |
| `clothing` | Minimum and maximum temperature protection and radiation capacity and dissipation. G-force resistance is optional. |
| `craftingGlobal` | Dismantle resource blacklist. Entity-class blacklist and default-known blueprints are optional. |
| `durability` | Item health. |
| `facilities` | CargoGrid kind, SCU capacity, `miningOnly` and `invisible`. Keep the physical configuration keys used by placement links. |
| `itemAttachmentConstraints` | Authored rules used for fitting parts, including restrictions on types, sizes and tags. |
| `itemPorts` | Ship and Item port names, labels, size, types and subtypes, flags, port tags and required tags. Use resolved Loadouts for the default Quantum Drive and installed occupants. |
| `itemRadars` | Aim-assist minimum and maximum assignment distances. |
| `jumpPointConfigurations` | Largest admitted ship size link. |
| `missionBoards` | Board type and faction links. |
| `quantumDrives` | Fuel requirement, standard drive speed, both acceleration stages, engage speed, spool and cooldown time with their units. Jump range and calibration are optional. |
| `resourceComponents` | Resource state names, deltas and power ranges. |
| `shieldGenerators` | Maximum health, regeneration, delays and resistances. |
| `shopCommodityGlobals` | Automated loading base and box-size prices by SCU. |
| `suitArmor` | Damage-resistance macro link. |
| `vehicleArmor` | Authored vehicle armor fields used by the Items and Ships views. |
| `weaponActions` | Per-mode fire rate, beam damage per damage type and hit type, and tractor force, range, full-strength range and volume. |

## Declarations

| Dataset | Retained scope |
| --- | --- |
| `blueprintRewardSources` | Contract identity and title reference, chance and Blueprint Pool GUID. |
| `resourceGroupMembers` | Ordered member Resource Types and child groups. |
| `shopInventoryResolution` | Per-Shop status, preserving resolved, placeholder, ambiguous and missing outcomes. |

## Definitions

Retain GUID, class name and Record path on each definition that has those fields.
`shopInventories` uses its authored Shop ID. Include the physical Record index
where an emitted reference needs it. The following columns describe the other
selected data.

| Dataset | Retained scope |
| --- | --- |
| `ammoDefinitions` | Damage by type. |
| `blueprintCategories` | Identity. |
| `blueprintPools` | Ordered Blueprint Record references and weights, retaining duplicates. |
| `blueprints` | Identity and physical tier join key. |
| `commodities` | Identity. |
| `contractScopes` | Scope labels. |
| `damageResistanceMacros` | Resistance channels including the physical multiplier. |
| `dismantleProcesses` | Efficiency and time in seconds. |
| `factionReputations` | Name, display name, NPC status, allies, enemies, alliance and hostility rules, reputation UI context and perk reward links. |
| `factions` | Name, type, default reaction, policing and arrest rights, relationships and faction-reputation link. |
| `gameplayProperties` | Property name, unit formatting and display transformation. |
| `items` | Item type, subtype, size, grade, mass, manufacturer, text references and fit tags. |
| `manufacturers` | Code, names and description. |
| `reputationContexts` | Primary and other scopes and scope sort order. |
| `reputationPerks` | Perks by scope and standing. |
| `reputationScopes` | Names, initial reputation, ceiling and ordered standings. |
| `reputationStandings` | Names, thresholds, gating, drift value and interval, perk description. |
| `resourceGroups` | Name. |
| `resourceTypes` | Name, density, refined version, groups, default cargo containers and resource properties. |
| `shopInventories` | Shop ID and ordered inventory IDs, buy and sell prices, current and maximum stock. These values describe design data. |
| `starmapAmenities` | Name. |
| `starmapObjects` | Names, description, type, parent, size, visibility, obstruction/arrival/adoption radii, travel block, amenities, jurisdiction and affiliation. |
| `starmapTypes` | Name, classification, quantum destination validity and obstacle policy. |

## Placements and projections

| Dataset | Retained scope |
| --- | --- |
| `placements.facilityInstances` | Ship GUID, CargoGrid configuration kind and physical owner, and resolved Loadout port path. No transforms. |
| `placements.jumpEndpoints` | System, starmap Record GUID and type, world position, target path, candidates and declared partner. |
| `placements.starmapPlacements` | Object and placed-under GUIDs, entity name, system and position in system-frame metres. Cover container sources supported by the canonical reader. |
| `projections.blueprintPoolWeights` | Total weight by pool. |
| `projections.blueprintPresentation` | Blueprint identity, name, category, product, default availability, dismantle process and tier, first tier and ingredients. Resolve tier and ingredient links without publishing raw arenas. The v2 Wiki key has no corresponding field on the v3 presentation row. Verify its source before deriving a compatibility value. |
| `projections.commodityPresentation` | Commodity GUID, name, description, display type, density, type, subtype and refined-subtype GUIDs. |
| `projections.contracts` | Parent and sub-contract GUIDs, debug name, handler, title/giver/type references, faction and giver faction, standing and reputation data, cooldowns, fixed and Blueprint rewards, layered values, offer locations and pickup points. Keep additional domain pointers as opaque GUIDs only when their target tables are outside the published scope. |
| `projections.jumpConnections` | Preserve the table and its availability, including an empty surveyed table while no connection is established. |
| `projections.missionTitles` | Title and description per contract, with English and German text closure. |
| `projections.resourceGroupParents` | Parent per group. |
| `projections.shipBaseFacts` | Ship GUID, cargo SCU, counts of grids with issues, unresolved Loadout state and capacity per mount. Preserve the inputs needed for freight capacity excluding mining grids. |
| `projections.weaponCombatBases` | Owner and mode, formula version, damage, fire rate, projectiles, nominal flight distance and alpha/fire-rate/nominal-DPS units. |
| `projections.weaponProjectileAmmoSelections` | Record indexes for ammo and magazine, selector role and the originating weapon occurrence. |

Mission boards, scope labels, armor, attachment constraints and designer shop
inventory are lower priority in the workbook. They remain within the selection.
The canonical ship cargo total includes both ordinary and mining grids. Resolve
freight capacity through CargoGrid placement and configuration links, counting
each mounted occurrence and checking its stated `miningOnly` flag. Missing flags
or capacities leave freight coverage incomplete. Keep each canonical Cargo
total's original meaning.

Starmap placements describe bound Starmap Object Records in the relevant system's
container frame. Read the supported 4.11 container sources to discover those
bindings. Unbound container instances are outside the selected table.

Unselected fields and datasets can contribute to a retained projection. Close
the Blueprint tiers, physical ammo and port/facility, reputation and text joins
over the available canonical results before checking reduced size.

## Acceptance

#1177 tracks the wire contract, generation and import. Use a distinct versioned
format or profile for the reduced field shapes and omission rules. The complete
v3 schema describes a different payload. Record exact scope and import support
for the new profile before publication.

Produce from scoped reads, measure peak memory including helper processes, and
verify less than 2,000,000,000 B peak under the release criterion. Record the
Build, elapsed time, peak private commit and working set, temporary storage and
minimum available host memory. Verify a LIVE `.json.zst` upload within
64,000,000 compressed and 1,000,000,000 decoded bytes. Test the actual importer,
including expected refusal for an ineligible Channel or incomplete selected
data. Compare ordered selected values, reference closure, source states and
encryption markers against the complete same-Build Catalog and SDK.
