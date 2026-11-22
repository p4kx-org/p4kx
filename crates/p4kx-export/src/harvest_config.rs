// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordTree, StructValue, TreeBuilder};

use crate::contract_harvest_config::{SubHarvestable, SubHarvestableMultiConfig, TaggedConfig};
use crate::guid_text::CatalogGuid;
use crate::harvest_link::Links;
use crate::harvest_preset::geometry_tags;
use crate::loot_records::loot_config;
use crate::record_fields::{
    boolean, class_name, number, reference, references, structs, structure, text,
};

/// Whether a path selects the harvestable slot preset projection.
pub(crate) fn is_slotpreset_path(path: &str) -> bool {
    crate::selection::starts_with_folder(path, "libs/foundry/records/harvestable/slotpresets/")
}

/// Projects an admitted slot preset and follows its selected harvestable dependencies.
pub(crate) fn slotpreset_config<'f>(
    trees: &mut TreeBuilder<'f>,
    tree: &RecordTree<'f>,
    forge: &'f DataForge<'f>,
    links: &Links<'f>,
) -> Option<SubHarvestableMultiConfig<'f>> {
    let path = tree.record_path();
    if !is_slotpreset_path(path) || crate::selection::is_leftover(path, tree.name()) {
        return None;
    }
    let multi = structure(Some(tree.root()), "multiConfig");
    Some(SubHarvestableMultiConfig {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        default_config_guid: reference(structure(multi, "defaultConfig"), "subConfigRef"),
        ignore_attachable_tags: boolean(multi, "ignoreAttachableTagsForTaggedConfigs"),
        tagged_configs: structs(multi, "taggedConfigs")
            .map(|tagged| tagged_of(trees, tagged, forge, links))
            .collect(),
    })
}

fn tagged_of<'f>(
    trees: &mut TreeBuilder<'f>,
    tagged: &StructValue<'f>,
    forge: &'f DataForge<'f>,
    links: &Links<'f>,
) -> TaggedConfig<'f> {
    let value = Some(tagged);
    let config = structure(value, "subConfig");
    let manual = structure(config, "subConfigManual");
    TaggedConfig {
        name: text(value, "name"),
        tag_guids: references(structure(value, "tagList"), "tags"),
        config_guid: reference(config, "subConfigRef"),
        respawn_time_multiplier: number(manual, "configRespawnTimeMultiplier"),
        initial_slots_probability: number(manual, "initialSlotsProbability"),
        sub_harvestables: structs(manual, "subHarvestables")
            .map(|sub| sub_of(trees, sub, forge, links))
            .collect(),
    }
}

fn sub_of<'f>(
    trees: &mut TreeBuilder<'f>,
    sub: &StructValue<'f>,
    forge: &'f DataForge<'f>,
    links: &Links<'f>,
) -> SubHarvestable<'f> {
    let value = Some(sub);
    SubHarvestable {
        harvestable: links.of_element(trees, forge, sub),
        setup_guid: reference(value, "harvestableSetup"),
        respawn_time_multiplier: number(value, "harvestableRespawnTimeMultiplier"),
        relative_probability: number(value, "relativeProbability"),
        relative_probability_deepest: number(
            structure(value, "relativeProbabilityDeepest"),
            "probability",
        ),
        geometry_tag_guids: geometry_tags(value),
        loot_config: loot_config(structure(value, "lootConfig")),
    }
}
