// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::iter::once;
use std::sync::{Mutex, PoisonError};

use p4kx_extraction::{FieldOrigin, SourceUses};

use crate::CatalogError;
use crate::contract_harvest::HarvestableLink;
use crate::crafting_warning::CraftingWarning;
use crate::guid_text::CatalogGuid;
use crate::memo::Memo;

#[path = "harvest_link_read.rs"]
mod read;
pub(crate) use read::DependencyReader;
use read::{Dependency, MiningFacts, PresetFacts, mining, preset};

pub(crate) const MINEABLE_PARAMS: &str = "MineableParams";

/// A slot's already projected authored reference and its exact local contributors.
#[derive(Clone)]
pub(crate) struct SelectedReference {
    pub(crate) guid: Option<CatalogGuid>,
    pub(crate) origin: FieldOrigin,
}

/// A resolved link and origins for its four dependency-derived fields.
pub(crate) struct ResolvedLink<'source> {
    pub(crate) value: HarvestableLink<'source>,
    pub(crate) origins: [(&'static str, FieldOrigin); 4],
}

/// Follows selected preset and mining dependencies within one original inventory.
#[derive(Default)]
pub(crate) struct Links<'source> {
    presets: Memo<Dependency<PresetFacts<'source>>>,
    minings: Memo<Dependency<MiningFacts>>,
    unreadable: Mutex<BTreeSet<String>>,
}

impl<'source> Links<'source> {
    /// Resolves the direct Entity override or the selected preset fallback for one slot.
    pub(crate) fn of_references(
        &self,
        reader: &mut DependencyReader<'_, 'source>,
        preset_ref: &SelectedReference,
        entity_ref: SelectedReference,
    ) -> Result<Option<ResolvedLink<'source>>, CatalogError> {
        if preset_ref.guid.is_none() && entity_ref.guid.is_none() {
            return Ok(None);
        }

        let preset = preset_ref
            .guid
            .map(|guid| {
                self.presets.get_or_try_make(guid.0, || {
                    reader.selected(guid, "HarvestablePreset", &self.unreadable, preset)
                })
            })
            .transpose()?;
        let preset_facts = preset.as_ref().and_then(|preset| preset.facts.as_ref());
        let sources = &reader.input.origins().uses;
        let preset_origin = sources.origin_with_origins(
            once(&preset_ref.origin)
                .chain(preset.iter().map(|preset| &preset.selection))
                .chain(preset_facts.map(|facts| &facts.class_origin)),
            true,
        )?;
        let entity = selected_entity(sources, entity_ref, &preset_ref.origin, preset.as_ref())?;

        let mining = entity
            .guid
            .map(|guid| {
                self.minings.get_or_try_make(guid.0, || {
                    reader.selected(guid, "entity", &self.unreadable, mining)
                })
            })
            .transpose()?;
        let facts = mining.as_ref().and_then(|mining| mining.facts.as_ref());
        let mining_routes =
            once(&entity.origin).chain(mining.iter().map(|mining| &mining.selection));
        let composition = facts.map(|facts| &facts.composition);
        let global_params = facts.map(|facts| &facts.global_params);
        let composition_origin = sources.origin_with_origins(
            mining_routes
                .clone()
                .chain(composition.map(|field| &field.origin)),
            true,
        )?;
        let global_origin = sources.origin_with_origins(
            mining_routes.chain(global_params.map(|field| &field.origin)),
            true,
        )?;

        Ok(Some(ResolvedLink {
            value: HarvestableLink {
                preset_guid: preset_ref.guid,
                preset_class_name: preset_facts.and_then(|facts| facts.class_name),
                entity_guid: entity.guid,
                mineable_composition_guid: composition.and_then(|field| field.guid),
                mining_global_params_guid: global_params.and_then(|field| field.guid),
            },
            origins: [
                ("presetClassName", preset_origin),
                ("entityGuid", entity.origin),
                ("mineableCompositionGuid", composition_origin),
                ("miningGlobalParamsGuid", global_origin),
            ],
        }))
    }

    /// Returns the warning for selected Records whose original body could not be read.
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
}

fn selected_entity(
    sources: &SourceUses,
    direct: SelectedReference,
    local_preset: &FieldOrigin,
    preset: Option<&Dependency<PresetFacts<'_>>>,
) -> Result<SelectedReference, CatalogError> {
    if direct.guid.is_some() {
        return Ok(direct);
    }
    let fallback = preset
        .and_then(|preset| preset.facts.as_ref())
        .map(|facts| facts.entity.resolve(sources))
        .transpose()?;
    let origin = sources.origin_with_origins(
        once(&direct.origin)
            .chain(once(local_preset))
            .chain(preset.map(|preset| &preset.selection))
            .chain(fallback.iter().map(|reference| &reference.origin)),
        true,
    )?;
    Ok(SelectedReference {
        guid: fallback.and_then(|reference| reference.guid),
        origin,
    })
}
