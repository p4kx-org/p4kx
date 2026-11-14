# Item definitions

Catalog v3 exports selected Item Records in `definitions.items`. The selection
uses the existing Item paths and exclusions, and an archive filter keeps its
usual meaning. Each row carries the common eight-field Record header. Within
one source DataForge, `recordIndex` identifies a physical row. Equal GUIDs remain
separate rows in original Record order. GUID remains the authored identity
for matching across builds, where a match can be ambiguous.

`components` reports the authored component occurrences that own the fields
below. Its entries carry the original zero-based `Components` slot and concrete
source type. Other component tables can reference the same Record and slot.
The list retains the original order and reports incomplete source evidence if
a component could not be decoded.

| Kind | Original source | Data and joins |
| --- | --- | --- |
| `attachment` | `SAttachableComponentParams.AttachDef` | Original `Grade`, `Manufacturer`, and `Localization`. `Name` refers to `texts.publicNames` and `Description` to `texts.gameTexts`. Join `configurations.itemAttachmentConstraints` by `ownerRecordIndex` equal to `recordIndex` and the same `componentOrdinal` for the same AttachDef's `Type`, `SubType`, `Size`, `Tags`, `RequiredTags` and alignment fields. |
| `physics` | `SEntityPhysicsControllerParams.PhysType` | Exact stored `Mass` in kilograms as a Single with IEEE bits. Repeated controllers retain separate slots. |
| `purchasable` | `SCItemPurchasableParams.displayType` | Original Locale occurrence in `texts.gameTexts`. |

A stored Manufacturer Reference retains its authored GUID and independently
observed physical target metadata. Other stored shapes retain their original
value representation. A readable manufacturer target outside the export filter
remains a reference while `definitions.manufacturers` follows its own selection.
See `exportHealth` for the separately measured target materialization.

The transitional Item projection carries `itemClass`, `playability`, `variantOf`
and `stats`. Item class is a projection of the English description. Variant
grouping uses its established private comparison inputs.
