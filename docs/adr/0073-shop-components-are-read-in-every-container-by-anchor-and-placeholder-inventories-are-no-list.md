# Shop components are read in every container by anchor, and placeholder inventories are no list

Decided for issue #481. Amends ADR-0071; builds on ADR-0054 and ADR-0069.

ADR-0071 gave a shop its item list from the inventory file that the `.soc` of its container names.
Measured on LIVE, that rule saw only part of what the files say, and it opened only the shop
folders. This ADR keeps the rule (every place that names the Entity must name the same existing
file) and widens what counts as a place.

## Catalog v3 amendment

Placeholder and orphan inventory files are retained as physical offline definitions. A shop that
names a placeholder still has null `inventory` and `itemGuids`. The physical definition preserves
its authored values and original provenance independently of the shop's projection.
`declarations.shopInventoryResolution` records the status, original component or prefab
candidates and physical target identities for each emitted shop.

## Decision

- **The `.soc` stores each string once.** A container file is a pool of strings in order of first
  use. The component name `EntityComponentShop` and the parameter type `SShopComponentParams`
  occur once per file, in its first shop component; the property name `offlineInventoryJSON` too.
  A later component of the same file shows only its new strings: object name, position, the
  `CryGUID` of its Entity if that text was not stored earlier, and the inventory path if no
  earlier component named it. The `CryGUID` text before the component name is therefore the
  Entity's own only sometimes; in the rest-stop and landing-service containers it is the class
  GUID `2845a83c-...` that every `SCShop` shares. That, and not a differing GUID in
  `entitycomponentshop.xml`, is why 144 shops were unscanned.
- **Anchors.** The component is tied to its Entity by two identifiers the container itself
  states: the `CryGUID`s of its `entitycomponentshop.xml`, and the names of the `SCShop` objects
  in its `<container>_editor.xml` (names that are unique in the container). An inventory path
  (the value after an `offlineInventory*` name, or a path in a `ShopInventories` folder) belongs
  to the closest anchor before it, provided that among the twelve strings before the path nothing
  but these stands between anchor and path: the component name (once), the parameter type,
  `offlineInventory*` names, `SCShop`, `PropertiesDataCore`, `__type`, `__version`,
  `__fullyInstanced`, `CastShadowMinSpec`, `AllowBakedRendering`, `RenderProxy`, at most one
  position, and a GUID only directly after `SCShop` (the class GUID). Two positions mean two
  objects: an object name, its position, another object's position and a path never tie the path
  to the first object. A path with anything else in between is counted in
  `soc_paths_unattributed`, its Entity stays unknown, and a component name that stands right in
  front of such a path is not read as "names no file" either. Only a component name behind an
  anchor with no path nearby names no file. The strings are read in one pass with a window of the
  last twelve, so memory does not grow with the file.
- **Every container.** The `.soc` of every socpak below `PU` and `EA` is read, not only the shop
  folders; kind and brand keep coming from the shop folders alone (`kind` stays `null` for a shop
  that only another container names).
- **Prefabs.** `Data\Prefabs\shops\*.xml` name inventory files through `offlineInventory*`
  attributes of `EntityComponentShop` on `Object Type="Entity"`. The editor `Id`
  `{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}` is the container `CryGUID` with the bytes reordered
  (`CCCCBBBB-AAAA-AAAA-` and `DDDDEEEEEEEEEEEE` reversed). A prefab is one more place per
  Entity. Two different files named by one Entity, in one object (`offlineInventoryJSON` and
  `offlineInventoryXMLName`) or in two prefab objects with the same `Id`, are a conflict and make
  the Entity ambiguous; the same file twice counts once.
- **Placeholders are no list.** `temp_outpost_shopinventory` (six commodities: waste, etam, maze,
  neon, processed food, medical supplies) is named for 264 shops of food courts, refineries,
  clothing stores, showrooms and an expo hall alike, and `inv_cryastro_dummy` (three GUIDs no
  Record has) for 136: they are designer placeholders and say nothing of what a shop sells. A shop
  that names one gets `inventory` and `itemGuids` `null` (where #479 exported the placeholder
  list); `ShopSummary.inventory_placeholder` counts them. The two files are listed in code.
- **Not silent.** New summary fields: `other_containers_scanned`, `other_containers_unreadable`,
  `other_containers_with_shop`, `prefabs_listed`, `prefabs_unreadable`, `prefab_entities`,
  `prefab_ambiguous`, `prefab_only_shops`, `soc_paths_unattributed`, `inventory_placeholder`,
  `inventories_named`, `inventories_in_archive`. The catalog shape does not change.
- **Not done, on purpose.** The `ShopID` of a file and the file names (`inv_<shop>_<place>`) are
  not used: see the evidence.

## Evidence

LIVE, Build 4.10.197, 2026-10-24.

- 9,668 socpaks; 345 `.soc` files hold the component name; 335 containers have an
  `entitycomponentshop.xml` (413 `CryGUID`s), 164 of them in the shop folders (317 opened for
  kind), 171 elsewhere (8,224 opened). The 12 other containers with a component have no
  `entitycomponentshop.xml` (rest-stop exteriors) and stay unlinked. No socpak holds a socpak.
  None of the 8,541 containers opened is unreadable.
- 12 inventory paths follow no anchor through the allowed strings (prefab-baked Entities whose
  GUID and name strings were stored earlier, and paths behind two positions); they stay unknown.
- 119 inventory JSON files (and one empty `Inv_Shopping_Test_FeatureTest.json`, counted as
  unreadable where named). 57 are named by a container or prefab (placeholders included), 62 by
  none: the 19 anniversary sales, `landingservices_a18|grimhex|levski|rs_full_0002-0008`, the
  `mining_*` files, the other city and station shop files. Nothing in the Archive names them: no
  `.soc`, no prefab, no Record (a search for the name of one over Records, texts and text entries
  finds nothing). They have no reliable link to a shop.
- The `ShopID` chain: 87 files have one. For the 44 unnamed of them it equals a registry key in 3
  cases and ends in a registry `entityGuid` in 16, among them clearly foreign ones
  (`inv_dumpersdepot_levski` ends in `SCShop_ShipComponents_GrimHex`). It stays unusable.
- Prefabs: 30 libraries, 11 shop Entities. Three (`dumpersdepot_franchise`, `dumpersdepot_portolisar`,
  `independent_grimhex_med`) have an empty `offlineInventory*` value and name nothing; the other 8
  name a file. Six of the 8 are the `CryGUID` of a container (two of them registry shops); the
  other two (`Inv_Admin_Levski`, `Inv_Skutters_GrimHex`) are in no container. No `Id` occurs
  twice. One of the 8 Entities is ambiguous (prefab and containers name different files); no shop
  gets its list from a prefab alone, so the prefabs confirm the container rule and add no shop.
- Result: 294 shops get a list from 33 files (Stanton 275, Pyro 19). The first version of this
  rule, which allowed any number of positions and one further GUID between anchor and path, gave
  310; 16 of those were not safe to keep. Before this ADR: 331 shops, of which 173 had the
  placeholder, so 158 real lists. 399 shops name a placeholder, 43 are ambiguous (containers or
  prefab name different files, or only some name one), 64 are unscanned (144 before), 1 names the
  empty file, none names a missing one. Every list was checked by eye as shop name next to file
  name in the real test; the entries whose names share no word with the file are the 8 casaba
  small-base shops (`Shop_lt_a_casaba_small_base_a` for `inv_casabaoutlet_lt_a_small_base_a`),
  rest-stop and city carts (`SCShop_noodle_cart_b` for the hot dog file, as the container
  names it), `Gate_Garage_Services` for `inv_landingservices_lorville` and the prison ore deposit
  for `inv_admin_hurston_lorville`; they are what the containers say.
- 2,395 of 2,422 item GUIDs (distinct inventories counted once) are an entry of `items`, `ships`,
  `commodities`, `paints` or `loadouts`. The 27 others are no Record of the DataForge at all
  (checked per GUID): all 21 of `inv_commex_tdd_area18` and 6 of 41 of `inv_astroarmada_area18`.
  They are item and commodity definitions that the game removed; the lists keep them, a consumer
  finds no entry for them.
- Cost, release build, LIVE, three runs each: reading the containers took 0.55 s with only the
  317 shop-folder containers and 1.45 s with all 8,541; the whole catalog build 12.3 s against
  13.0 s. The peak working set of the process was 1.9 to 2.1 GB in both cases, set by the catalog
  itself; the `_editor.xml` read (capped at 256 MiB) did not change it.

## Consequences

- The coverage is 25 % of the shops (294 of 1,184); the cities' shops are mostly in the 62
  files that nothing names. A further step needs a source the Archive does not appear to hold or a
  name rule that the owner accepts as a guess.
- A shop that names a placeholder looks like a shop without a list to the Basetool, as intended.
