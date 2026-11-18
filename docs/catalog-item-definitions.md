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
| `attachment` | `SAttachableComponentParams.AttachDef` | Immediately sourced `itemType`, `itemSubtype`, exact numeric `size` and `grade`, unsplit `tags` and `requiredTags`, original `manufacturer`, and `localization`. Name refers to `texts.publicNames` and Description to `texts.gameTexts`. String and Enum labels retain their original value. Valid foreign AttachDef structures retain these source facts without entering the separate SItemDefinition constraint family. Join admitted constraints by physical owner Record and the same component ordinal for restriction and alignment meaning. |
| `physics` | `SEntityPhysicsControllerParams.PhysType` | Exact stored `Mass` in kilograms as a Single with IEEE bits. Repeated controllers retain separate slots. |
| `purchasable` | `SCItemPurchasableParams` | Original `displayName` refers to `texts.publicNames` and `displayType` to `texts.gameTexts`. |

A stored Manufacturer Reference retains its authored GUID and independently
observed physical target metadata. Other stored shapes retain their original
value representation. A readable manufacturer target outside the export filter
remains a reference while `definitions.manufacturers` follows its own selection.
See `exportHealth` for the separately measured target materialization.

## Item presentation

Join `items[].recordIndex.value` to `definitions.items.rows[].recordIndex.value` within the same
Build/DataForge source. The root and definition share the original eight-field header cells.
GUID and Record path can both repeat. Follow `items[].selection.attachmentComponentOrdinal.value`,
`physicsComponentOrdinal.value` or `purchasableComponentOrdinal.value` to the original component's
`componentOrdinal.value`. Inspect selection state and completeness before using the target.

Attachment payload paths are `value.value.itemType`, `itemSubtype`, `size`, `grade`, `manufacturer`,
`tags`, `requiredTags` and `localization`; every named field retains its own immediate source
carrier or typed original branch. `localization.value.name` and `description` are text references.
Physics `value.value.mass` preserves exact Single bits. Purchasable `value.displayName` and
`displayType` remain independent authored references. Tags are the stored label, including
whitespace, case and repeated words; split whitespace only when a consumer needs tokens.

The root's selected ordinal stays on the first physical component even if its definition or
Localization is null or unreadable. A valid Class or StrongPointer Localization uses the same
original reader. A later component retains its own original values and never fills the earlier
selection. Unknown prefix slots make that choice incomplete. The constraint table continues to
admit only its own SItemDefinition source family, so a missing constraint row cannot erase
a valid foreign original Attachment payload.

Read `value.displayName` from each `purchasable` entry for the independently authored purchase
name of that component. The reference records the source of the authored key. Purchase names
convey no information about current shop stock.

Resolve each reference through its text table above. The original key is `key.value` in that row.
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

For the all-language check, repeat the following command with `P4KX_DATA_P4K` set to
each installed LIVE and PTU Archive. The test selects a unique Item Record with
the required original Locale fields in that Build. Set `P4KX_CATALOG_ITEM_TEXT_RECORD`
to check a specific Record. The test compares the catalog to the actual language Entry
inventory, checks source and display forms in both text domains, and resolves each
translation's origin back to the corresponding Entry. The result reports counts per language.

```text
cargo test --release --locked -p p4kx-export --test real_archive -- --ignored --exact --nocapture real_archive_canonical_texts::extras::installed_items::all_languages::installed_item_texts_match_every_available_archive_language
```
