# Released catalog field migration

The frozen [v0.4.2 schema](../crates/p4kx-export/tests/fixtures/catalog-v0.4.2.schema.json)
matches `schemas/basetool-catalog.schema.json` at the `v0.4.2` release tag. Use that schema to
inventory the released fields. A synthetic v2 instance may leave entire lists empty.

These mappings cover Blueprint Categories and Gameplay Properties. The migration inventory
also needs one verified disposition for every remaining released field, including fields in
nested tagged alternatives. P4KX 0.5.0 writes catalog schema v3. Re-export the original
inputs to obtain the physical identity and per-value provenance carried by v3.

| Released field | Catalog v3 |
| --- | --- |
| `blueprintCategories[].guid`, `className`, `recordPath` | Corresponding sourced field's `value` under `definitions.blueprintCategories.rows[]`. |
| `blueprintCategories[].name` and its `key`, `en`, `de` | The released writer always used an empty placeholder. The original category Records have no name field. Use the sourced class name when an application needs a category label. |
| `gameplayProperties[].guid`, `className`, `recordPath` | Corresponding sourced field's `value` under `definitions.gameplayProperties.rows[]`. |
| `gameplayProperties[].name` | Join `definitions.gameplayProperties.rows[].propertyName.value` to `texts.publicNames[].id.value`. |
| `gameplayProperties[].name.key` | `key.value` of that public-name entry. |
| `gameplayProperties[].name.en`, `name.de` | `translations.value.en.displayText.value` or `translations.value.de.displayText.value` on that entry, when the language and value are available. |

The released English and German fields used null for missing languages, missing keys,
placeholders and empty display values. Read the sourced cell's state to preserve those
distinctions when importing v3 data.
The text registry carries the decoded source text separately from its display form. For
the full field origins and policies see the [Gameplay Property contract](catalog-gameplay-properties.md)
and the [shared Record header](catalog-record-headers.md).
