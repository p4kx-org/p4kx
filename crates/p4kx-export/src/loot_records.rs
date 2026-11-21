// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue};

use crate::CatalogError;
use crate::catalog_read::original::OriginalFlowFields;
use crate::contract_loot::{LootArchetype, LootConfig, LootTable, LootTableEntry};
use crate::guid_text::CatalogGuid;
use crate::loot_entry::{
    archetype_entries, dupe_limit, optional_integer, optional_number, quantity, weight,
};
use crate::record_fields::{class_name, integer, number, reference, structs, structure, text};

/// Returns the loot table the Record tree `tree` describes.
pub(crate) fn loot_table<'f>(
    tree: &RecordTree<'f>,
    source: &OriginalFlowFields<'_, '_, 'f>,
) -> Result<LootTable<'f>, CatalogError> {
    let table = structure(Some(tree.root()), "lootTable");
    let entries = structs(table, "lootArchetypes")
        .map(|entry| table_entry(entry, source))
        .collect::<Result<_, _>>()?;
    Ok(LootTable {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        record_index: tree.record_index(),
        entries,
    })
}

fn table_entry<'f>(
    entry: &StructValue<'f>,
    source: &OriginalFlowFields<'_, '_, 'f>,
) -> Result<LootTableEntry<'f>, CatalogError> {
    let value = Some(entry);
    let archetype = structure(value, "archetype");
    let inline = archetype
        .filter(|inline| inline.field("entries").is_some())
        .map(|inline| archetype_entries(Some(inline), source))
        .transpose()?;
    let (weight, weight_source) = weight(source, entry)?;
    let (dupe_limit, dupe_limit_source) = dupe_limit(source, entry)?;
    Ok(LootTableEntry {
        name: text(value, "name"),
        weight,
        weight_source,
        archetype_guid: reference(archetype, "lootArchetypeRecord"),
        archetype: inline,
        priority_count: optional_integer(value, "priorityCount"),
        choice_limit: optional_integer(value, "choiceLimit"),
        dupe_limit,
        dupe_limit_source,
        chance_to_exist: optional_number(value, "chanceToExist"),
    })
}

/// Returns the loot archetype the Record tree `tree` describes.
pub(crate) fn loot_archetype<'f>(
    tree: &RecordTree<'f>,
    source: &OriginalFlowFields<'_, '_, 'f>,
) -> Result<LootArchetype<'f>, CatalogError> {
    let archetype = structure(Some(tree.root()), "lootArchetype");
    Ok(LootArchetype {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        record_index: tree.record_index(),
        entries: archetype_entries(archetype, source)?,
    })
}

/// Returns the loot the struct `config` (a `LootConfig`) generates, `None` for no struct.
pub(crate) fn loot_config(config: Option<&StructValue<'_>>) -> Option<LootConfig> {
    let config = config?;
    let value = Some(config);
    let constraints = structure(value, "lootConstraints");
    Some(LootConfig {
        loot_table_guid: reference(value, "lootTable"),
        loot_table_v3_guid: reference(value, "lootTableV3"),
        pool_filter_guid: reference(constraints, "poolFilter"),
        total_results_limit: integer(constraints, "totalResultsLimit"),
        chance_to_generate: number(constraints, "chanceToGenerate"),
        chance_to_generate_additional_inventories: number(
            constraints,
            "chanceToGenerateAdditionalAttachedInventories",
        ),
        fullness: quantity(structure(constraints, "fullnessFactorRange")),
    })
}
