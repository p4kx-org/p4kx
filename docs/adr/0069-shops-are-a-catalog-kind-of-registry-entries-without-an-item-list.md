# Shops are a catalog kind of registry entries, without an item list

Decided for issue #462. Builds on ADR-0054 and the spike #398 (`docs/shops-and-terminals.md`).

The Basetool wants shop identity, kind and place from the Archive; prices, stock and demand stay
server-side, and UEX IDs never enter the catalog. The spike found a registry of shops, shop
containers and item lists without a link to the shops. This ADR fixes what the kind `shops` is and
what it leaves out.

## Catalog v3 amendment

`definitions.shopInventories` selects physical offline inventory files independently of registry
rows. The `ShopBackup.ID` chain remains the shop identity. The inventory's authored `ShopID` is
retained with the physical file as evidence.

## Decision

- **`shops` is an additive kind** (ADR-0054, `schemaVersion` stays 1), derived from Archive files,
  not from Records. One entry per `ShopBackup` of
  `Data\Libs\CosmoTools\LayerRecovery\LayerBackups.xml`, read with the CryXml reader, in every
  `LayerBackup` of the file.
- **The identity is the `ID` chain** (`key`): the CryGUIDs from the system root to the `SCShop`
  Entity. It is unique (1,184 distinct for 1,184 entries). The other candidates are not identities:
  the CryGUID of the `SCShop` Entity (last element, `entityGuid`) recurs for a module placed in
  several places (235 distinct), and `DefaultValue` and `ShopName` are shared by shops of the same
  kind (360 distinct non-empty values each, one for one). They are exported as facts
  (`entityGuid`, `defaultValue`, `registryName`). Every `ID` element must be a GUID and is lower-cased,
  so `entityGuid` is the last element of `key` and a case-variant duplicate is dropped.
- **Kind and brand come from the shop container** whose `entitycomponentshop.xml` names the
  Entity's CryGUID, taken from its path (`PU\Shops\<kind>\<brand>\...`; `shopcomm` and `shopfood`
  below `PU\loc` as kind, without a brand). They are `null` when no container or differing
  containers name the Entity. The registry's own `SuperName` names containers, not the folders of
  the kind, so the path is the only source.
- **The place is the container chain.** `containers` lists it, `body` and `location` are its second
  and third element. `starmapObjectGuid` links the location to the starmap object whose
  `entityName` or `className` equals it, ignoring case, and is `null` when none or several do.
- **No item list.** See below. No price, stock or description is exported either.
- **Closed sets are strings:** `kind` and `brand` are strings the game may extend, and a reader maps
  an unknown value to `null`. No field is a game text: `registryName`, `entityName` and the container
  names are names, not localisation keys.
- **An empty `shops` is never silent.** A registry that exists but cannot be read
  (`unreadable_shop_registry`), a missing registry while the Archive holds starmap objects
  (`missing_shop_registry`) and unreadable shop containers (`unreadable_shop_containers`) are
  warnings: the exit code of `p4kx catalog` is 1 and `warnings` lists them. An importer that treats
  a vanished Record as removed must not drop the shops on a read failure.
- **A Filter selects no shop.** Shops have no record path; with a Filter none is read and the kind
  is `partial`.

## Evidence

LIVE, Build 4.10.197, 2026-10-22.

- **The registry holds Pyro as well.** The one file has two `LayerBackup` elements, `stantonsystem`
  (813 shops) and `pyrosystem` (371); the spike's "one platform" and "Pyro open" are settled: Pyro is
  there (its containers start at `pyrostar`, bodies such as `pyro5`, Lagrange points `P2_L4`,
  jump point `jumppoint_pyro_stanton`). `ShopName` and `DefaultValue` are empty for 2 Pyro entries.
  A grep for `ShopBackup` over the whole Archive finds no other file.
- **The chains.** `ID` and `SuperName` have the same length (4 to 9 elements) for every entry. The
  `ID` element is the CryGUID of the container in the byte order the container XML uses
  (`SuperName` shows the same GUID in braces, with the bytes swizzled). The last `ID` element of
  `SCShop_CommEx_TDD_NewBabbage` equals the CryGUID in `tdd_newbab`'s `entitycomponentshop.xml`,
  which confirms the link from the registry to the shop container.
- **The inventories are not assignable.** `Data\Scripts\ShopInventories` holds 120 files. 119 are
  JSON (the 120th is an XML test file); 87 of them have a `ShopID` chain and 31 have none (the event
  sales and some rental offices). Of the 87:
  the chain equals a registry `ID` for 4; the last element equals the `entityGuid` of some shop for
  39, and then often of a shop of another name and place (`Inv_Aparelli_NewBabbage` and
  `Inv_CenterMass_NewBabbage` end in the `SCShop` of `DumpersDepot_GrimHEX`; `Inv_Admin_Area18` and
  `Inv_Admin_NewBabbage` carry the same chain); 48 match nothing. No `DefaultValue` occurs in any
  inventory, and none of the 360 distinct `DefaultValue`s is the GUID of a DataForge Record. The file names carry brand and place (`Inv_Mining_Lorville`), but the registry
  `ShopName`s do not follow that scheme (`L_AdminOffice_Area18`, `CasabaOutlet_Stanton3_L2`), so no
  rule on names assigns a file to a shop either.
- **Coverage of the exported fields.** 1,000 of the 1,184 shops link to a starmap object (Pyro 307
  of 371); the 184 others have location names no starmap record carries (Pyro asteroid clusters and
  stashes, `ObjectContainer_RestStop`, `Jumptown_EchoIsle`) or names that more than one object
  carries (54 shops). 590 shops have a kind and brand, 60 none because containers of different
  kinds name their Entity, 534 none because no container below `PU\Shops` or the `shopcomm` and
  `shopfood` folders names it; 16 are commodity exchanges.
- **What the exported fields would invent.** Giving a shop the items of an inventory whose last
  element equals its `entityGuid` would give a shop in Grim HEX the items of a New Babbage clothing
  store. A wrong item list is worse than none for a consumer that matches it with UEX data.

## Consequences

- The Basetool matches shops with UEX terminals by `registryName`, `entityName`, `kind`, `brand`
  and the starmap object of `location`; the catalog says nothing about what a shop sells.
- Item lists per shop need a rule the Archive does not state yet. Follow-up #468 (v0.3.0) records
  the open question: how `DefaultValue` and `ShopID` relate to a shop, and whether the DataForge
  holds the shop inventory.
- The identity is stable only as long as the containers are not rebuilt; it was not compared
  between two LIVE Builds because the Archive holds one. The first Build after this one shows how
  often `key` changes.
- A shop that no `SCShop` container names (`without_container`) has no kind; the summary counts
  them, so the Basetool can see how much of the registry the containers cover.
