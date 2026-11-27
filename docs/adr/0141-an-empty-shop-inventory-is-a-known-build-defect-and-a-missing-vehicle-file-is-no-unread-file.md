# An empty shop inventory is a known Build defect and a missing vehicle file is no unread file

Decided for issue #606. Builds on ADR-0071.

The real-Archive catalog tests failed on `unreadable_shop_inventories` and
`unreadable_vehicle_files`. Neither was a reader fault.

## Decision

- **The shop inventory** `Data\Scripts\ShopInventories\inv_shopping_test_featuretest.json` has no
  content at all (0 bytes) in the Build checked, LIVE 4.10.197. An empty file
  is no inventory JSON, so the reader keeps reporting it as unreadable, with the reason `the file
  is empty`. It is not read as an empty list: that would say the shop sells nothing, where the
  Build says nothing. The warning stays; it is the signal that a Build ships a broken file.
- **The vehicle file** that a ship names under
  `Scripts/Entities/Vehicles/Implementations/Xml/` can be missing from the Archive. A file the Archive lacks is a fact of
  the game data, as the paint derivation already holds for Modification patch files; it was counted as an
  unread file only when it was the implementation itself. It no longer is: `unreadable_vehicle_files`
  counts ships whose file exists and cannot be read.
- **The warnings name the files.** `unreadable_vehicle_files` and `unreadable_shop_inventories`
  carry `details`, one string per file with its path and why it cannot be read, ordered and
  without repeats; `p4kx catalog` prints the first eight on stderr. The `count` and the kinds are
  unchanged, so the JSON summary changes additively.
- **The real-Archive tests** accept exactly the known defect: a warning
  `unreadable_shop_inventories` whose files are `inv_shopping_test_featuretest.json` with the
  reason `the file is empty`. Any other warning or failure fails them.

## Catalog v3 amendment

A complete read of a declared zero-byte Shop Inventory Entry establishes a present physical
source with no authored document. Catalog v3 retains the file definition with a sourced absent
document. A shop that uniquely selects it records `emptySource` and its exact physical and
payload evidence. The compatibility `inventory` and `itemGuids` fields stay null.

The file and selected shop operations are complete for normal publication. The existing
`unreadable_shop_inventories` warning, reason and legacy counters still include the empty
source. The library summary distinguishes the exact empty cases. Export health preserves the
aggregate warning as information and classifies each required file operation separately.
An authored JSON inventory
with an empty array resolves with an empty item list. A missing Entry, a failed read and
nonzero malformed content keep their existing distinct source states. The empty source
classification depends on the verified complete read.
