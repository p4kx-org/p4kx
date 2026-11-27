# Reduced Basetool Catalog scope

The field-selection workbook marks 76 rows for inclusion and 157 for omission.
Five selected rows are containers for selected child tables. All 60,512 field
rows inherit the dataset choice. The dataset notes narrow the retained fields
below. Omission applies to the delivered payload. Canonical readers still use
the inputs that selected projections require.
This selection was made against Catalog v3 at `6f3cda47`. Validate the source path
for each field against the release candidate before fixing the reduced schema.

## Export and import

```text
p4kx catalog basetool <archive> --out <file.json.zst>
```

The command writes one Zstandard frame with the decoded content size. The output
is limited to 64,000,000 compressed bytes and 1,000,000,000 decoded bytes,
including the final JSON newline. The previous file remains in place until
preparation and the final archive stamp check succeed.

The outer document contains `format: "p4kx-basetool-catalog"`,
`formatVersion: 1` and a `catalog` object with `schemaVersion: 3`. Consumers
select the wire contract by the outer format and version. Generate the
structural schema with `p4kx catalog schema --profile basetool` or Rust's
`p4kx_export::basetool_catalog_schema()`. It validates the envelope, source
metadata, required selected datasets, coverage entries and text containers.
The per-dataset field shapes and reference joins are specified below and
exercised by the projection tests.

The producer identifies the Channel and Build when available. The importer
checks LIVE eligibility and selected completeness before applying the upload.

## Envelope, text and completeness

- Retain `schemaVersion`, `source`, `exportHealth` and `completeness`. The
  `source` block includes the Channel, Build (including change number and public
  patch when known), tool version, export timestamp, key access and unread
  encrypted Entry count. Health assesses the source closure needed for this
  export. A Normal export requires complete selected data, and the importer
  requires a healthy export before applying it.
- `catalog.exportHealth.status` is `complete` for the selected profile.
  `catalog.exportHealth.scope` lists all 76 accepted dataset and container
  paths. `catalog.completeness` lists the 65 staged data leaves in scope order
  with `path`, emitted `rows` and `selection`. The selection is `all`,
  `shipAndVehicleRoots` for Loadouts or `boundStarmapObjects` for Starmap
  Placements. The text and pledge snapshots, source metadata and section
  containers use the overall scope.
- The top-level data leaves are arrays. Child datasets in `configurations`,
  `declarations`, `definitions`, `placements` and `projections` have
  `availability` and `rows`. An absent source family has an empty row array.
  A surveyed family can have zero selected rows and remain available.
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
| `projections.blueprintPresentation` | Blueprint identity, name, category, product, default availability, dismantle process and tier, first tier, ingredients and the class name used as the v2 Wiki key. Resolve tier and ingredient links without publishing raw arenas. |
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

## Canonical readers and reduced references

- Resolve Ship and Vehicle roots with `visit_loadout_owners(Ships)`. Project
  nested port cells from the resolved Loadout so their value states and
  encryption markers survive. Feed the same root and port traversal to the
  CargoGrid placement join. Keep the physical owner, component ordinal and
  port path required to identify each mounted grid.
- Read the starmap systems once for placements and jump endpoints. Use the
  complete inventory survey for endpoint/partner evidence. Only bound Object
  Records enter the placement table. Preserve endpoint and connection
  availability even when there are zero connections.
- Process each Blueprint using the canonical tier and recipe readers. Translate
  original arena addresses before dropping the owner. Use the physical Blueprint
  Record index, tier position and cost position to link ingredients, nested
  options and dismantle tiers. Preserve authored order, duplicate Blueprint
  GUIDs and gaps where an original slot has no interpreted cost.
- The v2 `blueprints[].key` is the class name copied from the same Record that
  supplies `blueprints[].className`. Get the reduced value from the sourced
  `className` in `definitions.blueprints`. Join the presentation's
  `blueprint.recordIndex` to the definition's `recordIndex.value` in the same
  Catalog generation. The producer verifies the original source identities
  before emitting the physical index. This keeps repeated GUIDs distinct.
  Preserve source casing and state. Skip Wiki matching when the class name is
  empty or the join is unavailable.
- Resolve a retained recipe slot's debug name from its actual original value
  and retain protected text classification. Accumulate text selectors for the
  included fields by canonical public-name or game-text domain and key before
  materializing English and German. A key can occur in both domains. Preserve
  absent translations and the marker derived from the selected contributions.
- Resolve Shop inventory value addresses within each original file before
  discarding its raw value arena. Preserve inventory order, physical candidates,
  cell states and price/stock values. An unreadable selected inventory blocks
  Normal export. Shop resolution coverage cannot stand in for the full
  selected inventory coverage.

## Internal Shop producer slice

The Shop slice uses normalized inventory paths and physical Entry order. It
shares the canonical registry and inventory-resolution readers. It resolves original JSON cells before discarding their
arena and retains the exact JSON number tokens and classification of text cells.
It checks the reduced byte limits through the same zstd framing as the full Catalog.
The complete reduced wire contract requires every selected dataset container.

## Internal entity producer slice

The Item and Ship tables use their canonical unfiltered equipment readers
within one parsed session. Physical Record indices keep variant and
manufacturer links unambiguous when two Records share a GUID. The Ship reader
also supplies the original ATLS availability and the admitted identities for
the Store slug mapping. Pledges retain their reviewed snapshot identity.

The Paint projector shares the canonical matcher across one fleet and tag
survey. Its physical Ship indices align with the compatible GUID list. A
missing original Paint selector or compatibility dependency blocks this
slice.

The selected Ship and Manufacturer references contribute to the shared public
and protected text tables. English and German use canonical language paths,
with distinct states for missing values. More than one selected kind may
reference a key. In that case the text values and states must agree, and the
encryption marker incorporates all contributing inputs.

## Internal Ship equipment producer slice

A single resolved Ship-root visitor feeds the nested Loadout rows and the
CargoGrid placement join. The CargoGrid definition survey includes unmounted
grids. The placement uses the host Record index, installed port ordinals and
physical configuration owner. Repeated mounts of one grid definition remain
separate occurrences.

Canonical Ship base facts supply the original cargo total and capacities per
mount. A separate `cargo.freight` result excludes grids whose observed
`miningOnly` flag is true. It reports an incomplete value when an occurrence
lacks a unique physical configuration, matching capacity or known mining flag.
The original cargo total and its marker still describe all canonical mounts.

## Internal cargo loading and contract scope slices

The cargo loading and commodity fee tables use their canonical SDK
readers. The reduced rows keep exact authored times, their SCU size keys,
signed prices, and the numeric state and marker of each selected cell.
Non-finite times remain distinguishable with their numeric type.

Contract scope rows retain each of the seven original label slots. A
null pointer differs from an absent declaration. The label references feed
the shared English and German public-name closure. Icons and additional
source members are outside this selection.

## Internal ResourceGroup producer slice

The three ResourceGroup tables use the canonical SDK's definitions, ordered
memberships and first-parent projection. Name references feed the selected
English and German public-name closure. Membership slots keep their direct
Record references, including repeated and null slots and the source status
of dangling links. A physically resolved wrong-family Record keeps its
original target type and index. A link to the selected target family must
resolve to the same physical Record in that reduced table.

The parent projection checks the group survey it depends on. It retains
separate physical rows for aliases and preserves the source state and
encryption marker of each selected parent choice.

## Internal ResourceType producer slice

The canonical ResourceType reader supplies the selected names, density and
molar mass, refinement targets, direct group memberships, cargo slots and
ordered resource properties. Numeric values retain their source widths and
units, including distinguishable non-finite states. Quality data retains
inline payloads, Record references and empty base declarations. Name
references feed the same English and German closure as the group names.

The refinement links and ordered group memberships validate physical
targets against the staged ResourceType rows. The writer preserves both an
absent declaration and an explicitly null slot. Raw float bits, thumbnails,
validation flags and description references are outside this selection.

## Internal Commodity producer slice

The canonical Commodity query supplies both the physical definition and its
presentation. The definition retains identity. A presentation carries the
matching Commodity GUID and Record index, so physical aliases sharing one
GUID remain distinguishable. Presentation density keeps its canonical
grams-per-cubic-centimetre value. Nullable resource, group and refinement
choices preserve their states and markers. Several Commodities may still
use the same ResourceType link.

Selected names enter the public-name closure. Descriptions and display
types enter the protected game-text closure in English and German.
Original authoring bodies and proof inventories are outside this selection.

## Internal jump producer slice

The jump placement table retains system, physical Starmap Record and
container-instance identities, world positions, target paths, ordered
Archive Entry candidates and the original partner state. Repeated physical
occurrences remain distinct. The connection table preserves the canonical
survey availability and row count. An incomplete traversal cannot certify
an empty connection inventory.

The jump configuration table retains the original Entity component ordinal
and the sourced largest-ship-size link. A null link and an undeclared link
keep their states. A resolved LandingPadSize target retains its physical
identity, path and type even though its definition is outside the selection.

## Internal Quantum Drive producer slice

The selected configuration keeps each physical drive component, its fuel
requirement and jump range, and the standard speed, acceleration stages,
engage speed, cooldown, spool and calibration fields. The original
numeric types, signed zero and source states survive the reduction. Speeds,
distances and times keep the units in the [canonical Quantum Drive
contract](catalog-quantum-drives.md). An absent standard profile stays
absent. Spline, heat, boost and effects are outside the selection.

## Internal Blueprint category and pool producer slice

Category rows retain their physical Record identities. Pool definitions
and total weights come from one canonical query and join by their physical
pool positions. Pool slots retain their order, repetitions, nulls and
dangling references, along with each original relative weight. Every
resolved reference to an included Blueprint must resolve to the matching
physical definition in the reduced table.

## Internal Blueprint definition, presentation and tier producer slice

The canonical Blueprint query supplies the three tables in one pass. The
category and pool readers share the parsed input and verify every resolved
pool reward against the staged physical Blueprint definitions. Presentation
rows refer to physical Blueprint, tier and cost positions within
the generation. Cost paths distinguish mandatory, optional and nested
selection slots. The selected tiers retain authored times and ingredient
quantities in units or SCU, quality limits, and cost modifiers. An unsupported
modifier range prevents a complete publication. Selected text references use
the shared English and German catalogs, with protected debug strings retained
as game text.

## Internal item health and suit armor producer slice

Keep the original Entity Record and component ordinal for each durability and
suit armor row. Item durability retains the health value, including non-finite
values. Keep suit damage-resistance links with their authored state and require
each resolved macro link to match the physical target in the reduced table.
The six resistance channels retain their multipliers, thresholds and damage
caps, with the selected numeric types and encryption markers.

## Internal clothing and item radar producer slice

Clothing keeps the canonical minimum and maximum temperature resistance,
maximum radiation capacity and radiation dissipation rate. Item Radar keeps
the minimum and maximum aim-assist assignment distances. Both preserve
source-value state and the original Record and component occurrence.
Flight resistance, scan-ping fields and the radar buffer are outside the
reduced slice.

## Internal dismantle and crafting globals producer slice

Dismantle definitions keep efficiency and the canonical duration in seconds.
A Blueprint with a dismantle choice carries both the selected process GUID
and the physical index of the staged process. An ambiguous choice remains
absent. Crafting globals preserve the ordered dismantle resource blacklist
and reconcile resolved Resource Type links with the staged physical
definitions. The optional entity blacklist and default Blueprint list are
outside this reduced payload.

## Internal item definition producer slice

Stage one physical Item definition at a time and restore canonical GUID and
Record-index order when writing. Keep attachment type, subtype, fit tags,
size, grade and manufacturer, physical mass and the selected presentation
texts. The selected manufacturer link must resolve in the same generation.
Purchase and attachment text references use the canonical public-name and
protected game-text domains, including when a key occurs in both.

## Internal reputation and mission-board producer slice

Read faction reputation, ranks, scopes, contexts and perks in one SDK
snapshot. Keep physical links among the staged tables. Preserve standing
order, exact thresholds, drift hours, and the selected protected texts.
Retain the source class and typed details of each authored perk. Keep a
career board's guild affiliation and faction slots in order, with exact
resolved links to the staged faction reputation rows. Preserve the source type
for other board variants.

## Acceptance

#1177 tracks the wire contract, generation and import for
`p4kx-basetool-catalog` format v1. Record the verified scope and importer
revision before publication.

Produce from scoped reads and verify at most 40,000,000,000 bytes of peak
memory, including helper processes. Estimate headroom on a 64 GiB system.
Record the Build, elapsed time, peak private commit and working set, temporary
storage and minimum available host memory. Verify a LIVE `.json.zst` upload within
64,000,000 compressed and 1,000,000,000 decoded bytes. Test the actual importer,
including expected refusal for an ineligible Channel or incomplete selected
data. Compare ordered selected values, reference closure, source states and
encryption markers against the complete same-Build Catalog and SDK.
