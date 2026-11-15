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

## Item presentation

Join `items[].recordIndex.value` to `definitions.items.rows[].recordIndex.value` in the
same Catalog. The sourced index carries the same physical origin on both sides. GUID and Record
path can both repeat. Follow the first stored `attachment` entry for
`value.value.localization.value.name` and `value.value.localization.value.description`.
Follow the first stored `purchasable` entry for `value.displayType`. A missing or unreadable
value in the first matching entry stays missing or unreadable. Later entries supply their own
authored values.

Resolve `name.value` through `texts.publicNames`, and `description.value` and
`displayType.value` through `texts.gameTexts`. The original key is `key.value` in that row.
For a given language use `translations.value[language].displayText.value`; the exact decoded
text is in `sourceText.value`. A language map entry can be absent. An authored empty key or
translation remains distinguishable from a missing value. The three inline `key/en/de` objects
of the released v2 Item shape are replaced by these references in schema v3.

The Item projection carries `itemClass`, `playability` and `variantOf`. Item class is
a projection of the English description. Variant grouping uses private comparison
inputs. [Item stats](catalog.md#item-stats) describes the canonical component tables.

## Installed Item text check

Set `P4KX_DATA_P4K` to the installed LIVE Archive and use the configured Archive
key. Set `P4KX_CATALOG_ITEM_TEXT_RECORD` to the exact original Record path of an
Item with authored name and description Locale fields and at least one nonempty
German translation. The check runs the Catalog writer with that Record Filter
and keeps the output in memory. It compares the physical Record and component
slots, the original Locale keys and the source and display forms with the
English and German localization files. The full
`installed_live_catalog_texts_match_the_independent_source_oracle` check covers
the LIVE per-kind baselines.

```text
cargo test --release --locked -p p4kx-export --test real_archive -- --ignored --exact --nocapture real_archive_canonical_texts::extras::installed_items::installed_live_item_texts_match_original_record_and_languages
```
