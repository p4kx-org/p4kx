// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};

use p4kx_extraction::{DataForge, RecordTree, StructValue, TreeBuilder};

use crate::contract_harvest::HarvestableLink;
use crate::crafting_warning::CraftingWarning;
use crate::guid_text::CatalogGuid;
use crate::memo::Memo;
use crate::record_fields::{class_name, component, reference};

pub(crate) const MINEABLE_PARAMS: &str = "MineableParams";

/// What a `HarvestablePreset` Record says: its name and the entity it spawns.
#[derive(Clone, Copy)]
struct PresetFacts<'f> {
    class_name: Option<&'f str>,
    entity: Option<CatalogGuid>,
}

/// What the `MineableParams` of an entity link to.
#[derive(Clone, Copy)]
struct MiningFacts {
    composition: Option<CatalogGuid>,
    global_params: Option<CatalogGuid>,
}

/// Follows a harvestable from its `HarvestablePreset` Record to the entity and the mining
/// parameters of the entity, each Record read once. A Record that is missing or cannot be built
/// is kept for [`Links::warning`].
#[derive(Default)]
pub(crate) struct Links<'f> {
    presets: Memo<PresetFacts<'f>>,
    minings: Memo<MiningFacts>,
    unreadable: Mutex<BTreeSet<String>>,
}

impl<'f> Links<'f> {
    /// Returns what the element `element` spawns, from its References `harvestable` and
    /// `harvestableEntityClass`; `None` when it names neither.
    pub(crate) fn of_element(
        &self,
        trees: &mut TreeBuilder<'f>,
        forge: &'f DataForge<'f>,
        element: &StructValue<'f>,
    ) -> Option<HarvestableLink<'f>> {
        let preset_guid = reference(Some(element), "harvestable");
        let element_entity = reference(Some(element), "harvestableEntityClass");
        if preset_guid.is_none() && element_entity.is_none() {
            return None;
        }
        let preset = preset_guid.map(|guid| self.preset(trees, forge, guid));
        let entity_guid = element_entity.or_else(|| preset.and_then(|facts| facts.entity));
        let mining = entity_guid.map(|guid| self.mining(trees, forge, guid));
        Some(HarvestableLink {
            preset_guid,
            preset_class_name: preset.and_then(|facts| facts.class_name),
            entity_guid,
            mineable_composition_guid: mining.and_then(|facts| facts.composition),
            mining_global_params_guid: mining.and_then(|facts| facts.global_params),
        })
    }

    /// Returns the warning for the Records that were missing or could not be built, `None` when
    /// every Record was read.
    pub(crate) fn warning(&self) -> Option<CraftingWarning> {
        let details: Vec<String> = self
            .unreadable
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect();
        if details.is_empty() {
            return None;
        }
        Some(CraftingWarning::UnreadableHarvestables {
            count: details.len(),
            details,
        })
    }

    fn preset(
        &self,
        trees: &mut TreeBuilder<'f>,
        forge: &'f DataForge<'f>,
        guid: CatalogGuid,
    ) -> PresetFacts<'f> {
        self.presets.get_or_make(guid.0, || {
            let tree = self.record_tree(trees, forge, guid, "HarvestablePreset");
            PresetFacts {
                class_name: tree.as_ref().map(class_name),
                entity: tree
                    .as_ref()
                    .and_then(|tree| reference(Some(tree.root()), "entityClass")),
            }
        })
    }

    fn mining(
        &self,
        trees: &mut TreeBuilder<'f>,
        forge: &'f DataForge<'f>,
        guid: CatalogGuid,
    ) -> MiningFacts {
        self.minings.get_or_make(guid.0, || {
            let tree = self.record_tree(trees, forge, guid, "entity");
            let params = tree
                .as_ref()
                .and_then(|tree| component(tree, MINEABLE_PARAMS));
            MiningFacts {
                composition: reference(params, "composition"),
                global_params: reference(params, "globalParams"),
            }
        })
    }

    /// Builds the Record `guid` of the role `role`, or notes why it cannot be.
    fn record_tree(
        &self,
        trees: &mut TreeBuilder<'f>,
        forge: &'f DataForge<'f>,
        guid: CatalogGuid,
        role: &str,
    ) -> Option<RecordTree<'f>> {
        let reason = match forge.record_by_guid(guid.0) {
            None => "is not in the DataForge",
            Some(record) => match trees.build(&record) {
                Ok(tree) => return Some(tree),
                Err(_) => "cannot be read",
            },
        };
        self.unreadable
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(format!("{role} {} {reason}", guid.0));
        None
    }
}
