# Ship cargo projection retains physical mount evidence

Schema v3 represents interpreted CargoGrid capacity in
[projections.shipBaseFacts](../catalog.md#projected-cargo). This supersedes the
flat cargo fields from [ADR-0063](0063-the-catalog-ships-carry-the-ship-base-values-as-flat-nullable-fields.md).

The row joins to a Ship by its physical Record index within the same Build.
Its nominal GUID remains available. Each reached grid retains the physical
Loadout root and the ordered path of port ordinals. Distinct mounts of the same
grid each contribute to the sum. The grid attachment, storage selector,
selected inventory Record and capacity operands carry their own source uses.

The canonical producer and the scoped Ship SDK expose the same typed row built
during the existing Ship Base walk. The standalone Ship Base report keeps its
detailed diagnostic representation. Physical identity allows a selected SDK
query to retrieve one Ship without building the catalog.

The projection preserves the established capacity arithmetic. A subtotal of
readable grids remains present with an explicit count of unavailable capacities.
An unresolved Loadout can leave additional grids unaccounted for. Aggregate
origins retain observed contributors and remain incomplete while exclusion
decisions lack evidence. A proven encrypted input sets the encryption marker to
true even with incomplete coverage.
