// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceUseRole};

use crate::CatalogError;
use crate::catalog_read::CatalogRecordContext;
use crate::catalog_read::original::OriginalFlowFields;
use crate::contract_harvest_config::{SubHarvestable, SubHarvestableMultiConfig, TaggedConfig};
use crate::contract_loot::{LootConfig, QuantityRange};
use crate::guid_text::CatalogGuid;
use crate::harvest_link::{DependencyReader, Links, SelectedReference};
use crate::harvest_projection_fields::{Object, Projection, ReferenceElements};
use crate::record_fields::{self, boolean_value, integer_value, record_reference, text_value};
use crate::record_header_field::RecordHeaderField;

/// Whether a path selects the harvestable slot preset projection.
pub(crate) fn is_slotpreset_path(path: &str) -> bool {
    crate::selection::starts_with_folder(path, "libs/foundry/records/harvestable/slotpresets/")
}

/// Whether the original owner is an admitted slot preset.
pub(crate) fn admits(tree: &RecordTree<'_>) -> bool {
    let path = tree.record_path();
    is_slotpreset_path(path) && !crate::selection::is_leftover(path, tree.name())
}

/// Projects the admitted original owner and the dependencies selected by its slots.
pub(crate) fn slotpreset_config<'source>(
    reader: &mut SlotDependencies<'_, 'source>,
    tree: &RecordTree<'source>,
    originals: &OriginalFlowFields<'_, '_, 'source>,
    header: &CatalogRecordContext<'_>,
) -> Result<SubHarvestableMultiConfig<'source>, CatalogError> {
    use RecordHeaderField::{FileName, Guid, RecordName, SourceType};
    use SourceUseRole::{Conversion, Payload, Selector};

    let mut projection = Projection::new(originals);
    let source = originals.sources();
    for (name, uses) in [
        ("guid", vec![header.observe_header(Guid, Payload)?]),
        (
            "className",
            vec![
                header.observe_header(RecordName, Conversion)?,
                header.observe_header(SourceType, Selector)?,
            ],
        ),
        (
            "recordPath",
            vec![header.observe_header(FileName, Conversion)?],
        ),
    ] {
        projection.named(name, source.origin(&uses, true))?;
    }
    let root = projection.root();
    let multi = projection.object(&root, "multiConfig")?;
    let default = projection.object(&multi, "defaultConfig")?;
    let default_config_guid = projection.scalar(
        &default,
        "subConfigRef",
        "defaultConfigGuid",
        record_reference,
    )?;
    let ignore_attachable_tags = projection.scalar(
        &multi,
        "ignoreAttachableTagsForTaggedConfigs",
        "ignoreAttachableTags",
        boolean_value,
    )?;
    let tagged_configs = projection.structures(
        &multi,
        "taggedConfigs",
        "taggedConfigs",
        |projection, tagged| tagged_of(projection, tagged, reader),
    )?;
    Ok(SubHarvestableMultiConfig {
        guid: CatalogGuid(tree.guid()),
        class_name: record_fields::class_name(tree),
        record_path: tree.record_path(),
        default_config_guid,
        ignore_attachable_tags,
        tagged_configs,
        field_origins: projection.finish(),
    })
}

/// Shared Record reader state for the dependencies selected by a slot.
pub(crate) struct SlotDependencies<'read, 'source> {
    pub(crate) reader: DependencyReader<'read, 'source>,
    pub(crate) links: &'read Links<'source>,
}

fn tagged_of<'tree, 'source>(
    projection: &mut Projection<'_, '_, 'tree, 'source>,
    tagged: &Object<'tree, 'source>,
    reader: &mut SlotDependencies<'_, 'source>,
) -> Result<TaggedConfig<'source>, CatalogError> {
    let config = projection.object(tagged, "subConfig")?;
    let manual = projection.object(&config, "subConfigManual")?;
    let tags = projection.object(tagged, "tagList")?;
    Ok(TaggedConfig {
        name: projection.scalar(tagged, "name", "name", text_value)?,
        tag_guids: projection.references(&tags, "tags", "tagGuids", ReferenceElements::Direct)?,
        config_guid: projection.scalar(&config, "subConfigRef", "configGuid", record_reference)?,
        respawn_time_multiplier: number(
            projection,
            &manual,
            "configRespawnTimeMultiplier",
            "respawnTimeMultiplier",
        )?,
        initial_slots_probability: number(
            projection,
            &manual,
            "initialSlotsProbability",
            "initialSlotsProbability",
        )?,
        sub_harvestables: projection.structures(
            &manual,
            "subHarvestables",
            "subHarvestables",
            |projection, slot| sub_of(projection, slot, reader),
        )?,
    })
}

fn sub_of<'tree, 'source>(
    projection: &mut Projection<'_, '_, 'tree, 'source>,
    slot: &Object<'tree, 'source>,
    reader: &mut SlotDependencies<'_, 'source>,
) -> Result<SubHarvestable<'source>, CatalogError> {
    let preset = projection.value(slot, "harvestable", record_reference)?;
    let entity = projection.value(slot, "harvestableEntityClass", record_reference)?;
    let mut membership = preset.inputs.clone();
    membership.extend(&entity.inputs);
    projection.named_origin("harvestable", &membership)?;
    let preset = SelectedReference {
        guid: preset.value,
        origin: projection.origin(&preset.inputs),
    };
    let resolved = reader.links.of_references(
        &mut reader.reader,
        &preset,
        SelectedReference {
            guid: entity.value,
            origin: projection.origin(&entity.inputs),
        },
    )?;
    let harvestable = if let Some(resolved) = resolved {
        projection.in_field("harvestable", |projection| {
            projection.named("presetGuid", preset.origin)?;
            for (field, origin) in resolved.origins {
                projection.named(field, origin)?;
            }
            Ok(())
        })?;
        Some(resolved.value)
    } else {
        None
    };

    let deepest = projection.object(slot, "relativeProbabilityDeepest")?;
    Ok(SubHarvestable {
        harvestable,
        setup_guid: projection.scalar(slot, "harvestableSetup", "setupGuid", record_reference)?,
        respawn_time_multiplier: number(
            projection,
            slot,
            "harvestableRespawnTimeMultiplier",
            "respawnTimeMultiplier",
        )?,
        relative_probability: number(
            projection,
            slot,
            "relativeProbability",
            "relativeProbability",
        )?,
        relative_probability_deepest: number(
            projection,
            &deepest,
            "probability",
            "relativeProbabilityDeepest",
        )?,
        geometry_tag_guids: projection.references(
            slot,
            "geometries",
            "geometryTagGuids",
            ReferenceElements::ObjectField("tag"),
        )?,
        loot_config: loot(projection, slot)?,
    })
}

fn loot<'tree, 'source>(
    projection: &mut Projection<'_, '_, 'tree, 'source>,
    slot: &Object<'tree, 'source>,
) -> Result<Option<LootConfig>, CatalogError> {
    let config = projection.object(slot, "lootConfig")?;
    projection.named_origin("lootConfig", &config.inputs)?;
    if config.structure.is_none() {
        return Ok(None);
    }
    projection.in_field("lootConfig", |projection| {
        let constraints = projection.object(&config, "lootConstraints")?;
        Ok(Some(LootConfig {
            loot_table_guid: projection.scalar(
                &config,
                "lootTable",
                "lootTableGuid",
                record_reference,
            )?,
            loot_table_v3_guid: projection.scalar(
                &config,
                "lootTableV3",
                "lootTableV3Guid",
                record_reference,
            )?,
            pool_filter_guid: projection.scalar(
                &constraints,
                "poolFilter",
                "poolFilterGuid",
                record_reference,
            )?,
            total_results_limit: projection.scalar(
                &constraints,
                "totalResultsLimit",
                "totalResultsLimit",
                integer_value,
            )?,
            chance_to_generate: number(
                projection,
                &constraints,
                "chanceToGenerate",
                "chanceToGenerate",
            )?,
            chance_to_generate_additional_inventories: number(
                projection,
                &constraints,
                "chanceToGenerateAdditionalAttachedInventories",
                "chanceToGenerateAdditionalInventories",
            )?,
            fullness: fullness(projection, &constraints)?,
        }))
    })
}

fn fullness<'tree, 'source>(
    projection: &mut Projection<'_, '_, 'tree, 'source>,
    constraints: &Object<'tree, 'source>,
) -> Result<Option<QuantityRange>, CatalogError> {
    let range = projection.object(constraints, "fullnessFactorRange")?;
    let min = projection.number(&range, "min")?;
    let max = projection.number(&range, "max")?;
    let mut membership = min.inputs.clone();
    membership.extend(&max.inputs);
    projection.named_origin("fullness", &membership)?;
    if min.value.is_none() && max.value.is_none() {
        return Ok(None);
    }
    projection.in_field("fullness", |projection| {
        projection.named_origin("min", &min.inputs)?;
        projection.named_origin("max", &max.inputs)
    })?;
    Ok(Some(QuantityRange {
        min: min.value,
        max: max.value,
    }))
}

fn number<'tree, 'source>(
    projection: &mut Projection<'_, '_, 'tree, 'source>,
    owner: &Object<'tree, 'source>,
    field: &str,
    output: &str,
) -> Result<Option<f32>, CatalogError> {
    let selected = projection.number(owner, field)?;
    projection.named_origin(output, &selected.inputs)?;
    Ok(selected.value)
}
