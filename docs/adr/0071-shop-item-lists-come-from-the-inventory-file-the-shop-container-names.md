# Shop item lists come from the inventory file the shop container names

Decided for issue #468. Builds on ADR-0054 and ADR-0069. Amended by ADR-0073: the component is read
in every container and found by anchor, and placeholder inventories give no list.

ADR-0069 left the item list out because no rule assigned an inventory file to a shop. The
`ShopID` chain inside a file is the wrong end to look at; the shop side names its file.

## Catalog v3 amendment

The physical inventory definition retains the full original JSON and provides typed navigation to
ordered lines, all authored ID alternatives, exact price and stock number tokens, and rental
variants. The same parsed source supplies the ordered, unique `itemGuids` of a resolved shop.
The all-sources-agree matching rule remains the source of that resolution.

## Decision

- **The rule.** The `.soc` of a shop container holds the `EntityComponentShop` component of its
  `SCShop` Entity: the CryGUID, the component name `EntityComponentShop`, the parameter block
  `SShopComponentParams` and its properties as name and value strings, e.g.
  `offlineInventoryJSON` followed by `Data/scripts/shopinventories/inv_admin_grimhex.json`, then
  `shopInventoryType`, `ShopRackGUIDs` or `ShopKioskGUIDs`. The value is a path set by the
  designers; `offlineInventoryXMLName` is the older name for it and may end in `.xml` although only
  the `.json` file exists. The CryGUID is the one `metadata\entitycomponentshop.xml` names, so it
  equals the `entityGuid` of the registry shops.
- **A shop gets a list** when every shop container that names its `entityGuid` names the same
  file, compared by lower-case stem (folder and extension ignored), and
  `Data\Scripts\ShopInventories\<stem>.json` is in the Archive and readable. A module that two
  containers name with different files, or that only some of them give a file, is ambiguous and
  gets none. Nothing is guessed from names or from the `ShopID` chain.
- **The fields.** `inventory` (the stem) and `itemGuids` (`ID.ID` GUIDs, lower case, ordered, no
  repeats), both `null` without a list. The file has two shapes, `Collection.Inventory` (with
  `ShopID`) and a top-level `Inventory` (the 31 files without `ShopID`); both are read. No
  `BuyPrice`, `SellPrice`, `CurrentInventory`, `MaxInventory` or `RentalOfferings`. The kind
  changes additively (ADR-0054, `schemaVersion` stays 1).
- **Reading.** The `.soc` is read together with the metadata of the container the catalog already
  opens for kind and brand; a container that cannot be read is already counted. A `.soc` that
  cannot be read only leaves the inventory of its container unknown, kind and brand stay. The
  component is found by its fixed name sequence and a parameter window that ends at the rack,
  kiosk or landing area list or at the next component, so a value is taken only inside the
  component of its own CryGUID, the first `offlineInventory*` value that ends in `.json` or `.xml`
  wins; the file is then checked in the Archive. Several `.soc` files of one container follow the
  same all-must-agree rule as several containers.
- **Not silent.** What the scan finds is told apart from what it does not: a component that names
  no file is not a component that was not found. `ShopSummary` counts `with_items`,
  `inventory_ambiguous`, `inventory_unscanned` (no readable component for the shop),
  `inventory_missing`, `inventory_unreadable`, `soc_components`, `inventories_missing`,
  `inventories_unreadable` and `item_ids_dropped`. Warnings: `unreadable_shop_inventories` and
  `missing_shop_inventories` (each with a `count` of files; a Build that moves or renames the
  files must not turn every list into `null` with exit code 0) and `no_shop_components` (shop
  containers name shops, yet no `.soc` holds a component). `itemGuids` may be `[]` for a file
  without items.

## Evidence

LIVE, Build 4.10.197, 2026-10-23.

- 166 `.soc` files hold an `EntityComponentShop`; 68 shop components name a `.json` or `.xml`
  file, 40 distinct names, all of which are inventory files of the Archive (the Archive holds 119
  JSON files).
- The `ShopID` of the inventory equals the CryGUID of the component for only 16 of the 63
  containers whose file has a `ShopID`: the chain is a stale design-time value, as ADR-0069 found.
- Through `entityGuid`, 331 of 1,184 registry shops get an item list (Stanton 208, Pyro 123) from
  28 files; 28 shops are ambiguous (e.g. the module `43e732b6-...` is named with three different
  files in five containers); none has a missing or unreadable file. 173 of the 331 use
  `temp_outpost_shopinventory`, which the rest-stop bars carry.
- 2,192 of the 2,219 item GUIDs (distinct inventories counted once) are the `guid` of an entry of
  `items`, `ships`, `commodities`, `paints` or `loadouts` of the same catalog; the 27 others are
  no entry of those kinds.
- 91 of the 119 files are named by no container the catalog opens: the anniversary sales, the
  `landingservices`, `mining` and city and station shop files. The prefabs `Data\Prefabs\shops\...`
  name 8 files with `offlineInventoryXML` attributes; the shops of the cities lie in containers
  below other folders of `PU\loc` that the catalog does not open for shops.

## Consequences

- The Basetool matches shops with UEX terminals as before and may add the item GUIDs of a shop it
  recognises; a shop without a list says nothing about what it sells.
- The coverage is 28 % of the shops. Reaching the city and station shops needs the containers that
  hold them, which are not known by path; a follow-up can find the shop components in all
  containers the registry chains lead through.
- The `.soc` is an engine chunk file whose property encoding the tool does not parse in full; the
  scan relies on the string sequence above and is covered by tests with synthetic bytes and a
  real-Archive test that prints the counts first.
