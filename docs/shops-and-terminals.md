# Shops and terminals in the Archive

The archive findings below record the [#398](https://github.com/p4kx-org/p4kx/issues/398) spike on
LIVE Build 4.10.197. The current export is described in the
[catalog v3 inventory contract](basetool-catalog.md#physical-offline-shop-inventories).
Live prices, stock and demand remain server-side. UEX IDs are outside the catalog.

## Short answer from the spike

- Shop definitions exist, but not as Records. A shop is an `SCShop` Entity inside an object
  container (socpak), and the set of all shops with their place in the world is a registry Entry.
- The registry gives every shop a name and a path through the container hierarchy (system, body,
  location, interior, shop). That path leads to a starmap object by container name.
- What a shop sells is in `Data\Scripts\ShopInventories\Inv_<Brand>_<Place>.json`. The link from an
  inventory to its shop is not resolved yet.
- A terminal is not a separate definition. Commodity terminals are the `SCShop_CommEx_TDD_*` and
  `SCShop_Platinumbay_*_Commodity` shops; a commodity has no per-terminal list in the Archive.
- A catalog kind `shops` is feasible for identity, kind, place and starmap link. Item lists are a
  second step. Both go to the follow-up issue.

## Where the definitions are

| Source | Count (LIVE) | Content |
| --- | --- | --- |
| `Data\ObjectContainers\PU\Shops\<kind>\<brand>\*.socpak` | 259 | One shop container per brand and layout |
| `Data\ObjectContainers\PU\loc\...\shopcomm\*`, `shopfood\*` | 58 (35 and 25 listed, 2 overlap) | Shop modules of stations |
| `metadata\entitycomponentshop.xml` in those containers | 164 containers, 190 shop references | The `SCShop` Entity of the container |
| `Data\Libs\CosmoTools\LayerRecovery\LayerBackups.xml` | 1,184 `ShopBackup` entries (Stanton) | Shop registry with hierarchy path and name |
| `Data\Scripts\ShopInventories\Inv_*.json` | 120 files (and one `.xml` test file) | Item lists per shop |
| `libs/foundry/records/globalshopparams/*` | 4 Records | Global tuning values only |
| `libs/foundry/records/entities/scitem/shopdisplays/*` | 687 Records | Mannequins and racks |
| `libs/foundry/records/scitemmanufacturer/shop/{shopkiosk,commoditykiosk}` | 58 and 3 Records | Kiosk brand skins |
| `libs/foundry/records/ui/map/sectiondefinitions/**/shops/*` | 32 Records | `InteriorMapSectionDefinition` with an empty name; no shop data |

The 317 containers (259 `Shops`, 58 `loc`) split by kind under `Shops` (29 kinds): market_stalls 67, food 27,
admin 17, clothing 16, medical 16, personalweapon 15, bounty 13, service 12, armor 11, bar 11,
dealership 8, shippart 8, cargo_admin 6, insurance 5, commex 4 and 14 smaller kinds.

The 36 EntityClassDefinitions with `Components/ShoppingProviderEntityComponentParams` are in-world
shopping providers (`brand`, `globalInventoryPersistentQueryDef`). They carry no shop identity and no
item list.

## What identifies a shop

- **In a container:** the `SCShop` Entity. `metadata\entitycomponentshop.xml` names its CryGUID, and
  the `*_editor.xml` of the container names it (`type="SCShop"`, a name such as `SCShop_..._01`). In
  the 180 editor files that hold one, 174 distinct CryGUIDs occur: a shop module reused in several
  places keeps the CryGUID of its container.
- **In the world:** the `ShopBackup` entry of `LayerBackups.xml`. It has an `ID` (the chain of
  swizzled container GUIDs from the system root to the shop, comma separated), a `SuperName` (the
  same chain as container names with their GUIDs in braces, e.g. `OOC_Stanton_{...}`,
  `OOC_Stanton_3_ArcCorp_{...}`, ..., `SCShop_...`), a `DefaultValue` GUID and a `ShopName` such as
  `Pharmacy_RestStop_ARC-LEO1` (a name, not a localisation key).
- **Stability across Builds:** the identifiers are container and entity GUIDs. They are stable as
  long as the container is not rebuilt. The Entries under `PU\Shops` and the Records
  `globalshopparams` and `entities/shopkiosk` were compared between two consecutive Builds with
  `p4kx compare`; the comparison is too close to show how identities behave over a longer time. The
  follow-up has to compare the registry between two LIVE Builds when a second one exists.

## Where a shop is

The `SuperName` path is the location. For the 77 distinct location containers that stand in the
third place of the paths, 63 equal the `entityName` of a starmap placement or the `className`
of its authored definition (case-insensitively), e.g. `Area18_City_objectContainer`, `Drlct_Stmnt_005_Windmill_RastarLE`,
`LOC_RR_S1_L1` (the Lagrange-point stations). The path also names the body: `OOC_Stanton_1_Hurston`
(180 entries), `..._4_Microtech` (165), `..._2_Crusader` (98), `..._3_ArcCorp` (89), the jump-point
stations and the Lagrange points. Rest stops and cities enter as containers whose inner name has
the shop kind (`pharmacy_*`, `food_foodcourt_*`, `admin_*`, `service_platinumbay_*`).

Pyro: the same file holds a second platform, `pyrosystem` with 371 shops (#462), so the registry
has 813 Stanton and 371 Pyro entries. The catalog measured it on all 1,184 shops (#462): 184 have
a location container without a starmap match and need a manual mapping.

The catalog's starmap placements carry world positions for 1,239 of 2,093 objects in this Build. A shop inherits the
position of its location object; a position inside the interior needs the `.soc` of the container
and is out of scope here.

## What a shop trades

`Inv_<Brand>_<Place>.json` is `{"ShopID": "<chain>", "Collection": {"Inventory": [...]}}`. Each
inventory element has an item `ID` (the GUID of an `EntityClassDefinition`; verified for a shotgun
with `p4kx dcb refs`), `BuyPrice`, `SellPrice`, `CurrentInventory`, `MaxInventory` and
`RentalOfferings`. The spike's three samples contained 132, 230 and 397 items. Catalog v3
exports these fields from the physical inventory as offline design-time data.

The `ShopID` chain has the same form as the registry `ID`. For two of three samples the first two
elements (system and body) occur in registry chains; the deeper elements did not match any registry
chain. How an inventory file is assigned to a `ShopBackup` therefore stays open: it is either a
second hierarchy for the same shops or a different set of shops. The follow-up must settle it with
the converted registry and all 120 inventories.

Settled in #462 (ADR-0069): the `ShopID` of an inventory assigns it to no shop. Of 87 inventories
with a `ShopID`, 4 equal a registry `ID`; the last element matches the `SCShop` of some shop for 39,
often a shop of another place; 31 have no `ShopID` (they use a top-level `Inventory` list instead
of `Collection.Inventory`). The registry `DefaultValue`s (360 distinct) are no DataForge Record
GUIDs.

Settled in #468 (ADR-0071) and #481 (ADR-0073): the link sits on the shop side. The `.soc` of a
container holds the `SCShop` component with `offlineInventoryJSON` (or `offlineInventoryXMLName`),
the path of the inventory file. The `.soc` stores each string once, so only the first component of
a file carries the component name and often the GUID text before it is the class GUID shared by
all `SCShop`; the path is tied to the Entity whose `CryGUID` (`entitycomponentshop.xml`) or object
name (`<container>_editor.xml`) is the closest anchor before it, with only known structure strings
and one position in between. 345 `.soc` files hold the component; 335 containers have an
`entitycomponentshop.xml`, 171 of them outside the shop folders, which the catalog now reads too.
The prefabs under `Data\Prefabs\shops` name 8 files with `offlineInventory*` attributes; the
`Id` of their Entity is the container `CryGUID` with the bytes reordered (6 of the 8 are in a
container). Through `entityGuid` 294 of the 1,184 registry shops get an item list from 33 files;
43 are ambiguous, 64 unscanned, 399 name a designer placeholder (`temp_outpost_shopinventory`,
`inv_cryastro_dummy`) and get none. 62 of the 119 files are named by nothing in the Archive.

## Terminals

There is no Record for a terminal. The relevant `SCShop` names are `SCShop_CommEx_TDD_<Place>`
(commodity exchange, four `commex` containers) and `SCShop_Platinumbay_small_Commodity` (28 shops of
`SCShop_Platinumbay`). `globalshopcommoditydata` only holds the supply and demand level thresholds
and box-size base prices. The `genericmobiterminals` Records are mission props.

## Mapping to UEX terminal IDs

The Archive has no UEX ID. A mapping has to go by name and place: `ShopName`, the brand of the
container path, and the starmap object name against the UEX terminal name and star system. Out of
the 77 location containers 63 reach a starmap object by name; a shop name is the open end. The
Basetool keeps the mapping and the manual remainder.

## Decision

A catalog kind `shops` is added additively (ADR-0054, #462, ADR-0069): shop key (chain), names,
kind and brand from the container path, starmap link by location container, body and system. The
item list per shop follows once the inventory link is settled (#468); prices are never exported.
