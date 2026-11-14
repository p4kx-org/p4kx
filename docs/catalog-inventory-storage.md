# Inventory storage and item bounds

The schema v3 catalog exposes the authored storage definition, each physical owner that selects it,
and the original item bounds. Each stored datum uses a sourced value cell, including the archive
encryption marker and the original DataForge receipt.

| Dataset | Original owner and fields |
| --- | --- |
| `definitions.inventoryContainers` | Each selected physical `InventoryContainer` Record, with its excluded type/subtype filters, `interiorDimensions` and concrete `inventoryType`. |
| `configurations.inventoryConfigurations` | Item storage from `SCItemInventoryContainerComponentParams`, including `containerParams`, both interaction WeakPointers, `canAutoStoreRestrictedItems` and additional original members. Vehicle storage retains `VehicleComponentParams.inventoryContainerParams`. |
| `configurations.itemBounds` | `AttachDef` stores the item occupancy amount, dimensions, local lower and upper vectors, optional UI override and fixed UI grid. Separate rows retain `VehicleComponentParams.maxBoundingBoxSize` and `SEntityComponentObjectMetadataParams.hasLocalBounds`. |

Container definitions use exact Record types for selection across DataForge folders. Within the
identified build, join a present storage reference's `targetRecordIndex.value` to the definition's
`recordIndex.value` and verify the GUID. A reference can identify an original Record omitted by
the requested catalog filter. The cell retains the authored GUID and distinguishes a dangling
or unexpected target type from a null or absent Reference. Configuration row identity includes
`ownerRecordIndex` and the original `componentOrdinal`. Repeated components retain separate
positions. Assembly placements are a separate relation. [Interaction declarations](catalog-inventory-interactions.md)
retain the physical targets of the storage component's interaction links.

Open storage retains the authored grid cell unit and amount, minimum and maximum item vectors,
external flag, grid offset, destruction parameters and ordered item-type filters. The subtype
arrays carry one sourced enum label per original slot, retaining order and duplicates. Closed
capacity and item occupancy volume retain the concrete cargo unit. The known
amount fields are `standardCargoUnits` in SCU, `centiSCU` in hundredths and `microSCU` in
millionths. Open grid sizes retain authored integer centimetres or metres. Unknown variants
preserve the concrete source type and original members.

Vector members keep the original x/y/z order and IEEE-754 Single bits. The UI grid uses x/y.
The coordinates describe the authoring definition's dimensions, offsets or local bounds.
Zero, signed zero and non-finite bits remain distinguishable; the finite projection is null
for a non-finite original. Stored volume is independent of vector dimensions. The existing
ShipBase cargo calculation retains its existing numeric and geometry rules.
