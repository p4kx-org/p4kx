// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_text_contract::{CanonicalText, GameTextRef, TextForms};
use crate::catalog_v3::starmap_placement::PlacementBinding;
use crate::contract_starmap::StarmapObject;
use crate::mining_locations::{
    AreaModifier, CompositionPart, HarvestableArea, HarvestableElement, HarvestableGroup,
    HarvestableLink, HarvestableLocation, HarvestableProviderPreset, MineableComposition,
    MiningGameText, MiningRecordSource, MiningStarmapObject, MiningTextForms,
};
use crate::starmap_containers::proof::PlacementField;
use crate::{CatalogError, contract_harvest, contract_mining};

pub(super) fn starmap(
    row: StarmapObject<'_>,
    source: MiningRecordSource,
    origins: &CatalogOrigins,
) -> Result<MiningStarmapObject, CatalogError> {
    let proof = PlacementBinding::new(&origins.uses, row.placement_proof.as_ref())?;
    Ok(MiningStarmapObject {
        source,
        name: row.name,
        type_name: row.r#type.map(str::to_owned),
        system: proof.cell(row.system, PlacementField::System)?,
        entity_name: proof.cell(row.entity_name, PlacementField::Entity)?,
        placed_under_guid: proof
            .cell(row.placed_under_guid, PlacementField::Parent)?
            .map(|guid| guid.0),
        position: proof.cell(row.position, PlacementField::Position)?,
        mining_claim: row.mining_claim,
    })
}

impl From<CanonicalRecordHeader<'_>> for MiningRecordSource {
    fn from(source: CanonicalRecordHeader<'_>) -> Self {
        Self {
            guid: source.guid.map(|guid| guid.0),
            record_name: source.record_name.map(str::to_owned),
            class_name: source.class_name.map(str::to_owned),
            record_path: source.record_path.map(str::to_owned),
            record_index: source.record_index,
            source_type: source.source_type.map(str::to_owned),
            file_name: source.file_name.map(str::to_owned),
            metadata_tag: source.metadata_tag.map(str::to_owned),
        }
    }
}

impl From<CanonicalText<'_, GameTextRef>> for MiningGameText {
    fn from(row: CanonicalText<'_, GameTextRef>) -> Self {
        Self {
            id: row.id,
            key: row.key.map(str::to_owned),
            translations: row.translations.map(|translations| {
                translations
                    .into_iter()
                    .map(|(language, forms)| (language, forms.into()))
                    .collect()
            }),
        }
    }
}

impl From<TextForms<'_>> for MiningTextForms {
    fn from(forms: TextForms<'_>) -> Self {
        Self {
            source_text: forms.source_text.map(Cow::into_owned),
            display_text: forms.display_text.map(Cow::into_owned),
        }
    }
}

impl From<(contract_mining::MineableComposition<'_>, MiningRecordSource)> for MineableComposition {
    fn from((row, source): (contract_mining::MineableComposition<'_>, MiningRecordSource)) -> Self {
        Self {
            source,
            deposit_name: row.deposit_name,
            minimum_distinct_elements: row.minimum_distinct_elements,
            parts: row.parts.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<contract_mining::CompositionPart> for CompositionPart {
    fn from(row: contract_mining::CompositionPart) -> Self {
        Self {
            mineable_element_guid: row.mineable_element_guid.map(|guid| guid.0),
            min_percentage: row.min_percentage,
            max_percentage: row.max_percentage,
            probability: row.probability,
            curve_exponent: row.curve_exponent,
            quality_scale: row.quality_scale,
        }
    }
}

impl
    From<(
        contract_harvest::HarvestableProviderPreset<'_>,
        MiningRecordSource,
    )> for HarvestableProviderPreset
{
    fn from(
        (row, source): (
            contract_harvest::HarvestableProviderPreset<'_>,
            MiningRecordSource,
        ),
    ) -> Self {
        Self {
            source,
            groups: row.groups.into_iter().map(Into::into).collect(),
            areas: row.areas.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<contract_harvest::HarvestableGroup<'_>> for HarvestableGroup {
    fn from(row: contract_harvest::HarvestableGroup<'_>) -> Self {
        Self {
            group_name: row.group_name.map(str::to_owned),
            group_probability: row.group_probability,
            harvestables: row.harvestables.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<contract_harvest::HarvestableElement<'_>> for HarvestableElement {
    fn from(row: contract_harvest::HarvestableElement<'_>) -> Self {
        Self {
            harvestable: row.harvestable.map(Into::into),
            setup_guid: row.setup_guid.map(|guid| guid.0),
            clustering_guid: row.clustering_guid.map(|guid| guid.0),
            relative_probability: row.relative_probability,
            geometry_tag_guids: row
                .geometry_tag_guids
                .into_iter()
                .map(|guid| guid.0)
                .collect(),
        }
    }
}

impl From<contract_harvest::HarvestableLink<'_>> for HarvestableLink {
    fn from(row: contract_harvest::HarvestableLink<'_>) -> Self {
        Self {
            preset_guid: row.preset_guid.map(|guid| guid.0),
            preset_class_name: row.preset_class_name.map(str::to_owned),
            entity_guid: row.entity_guid.map(|guid| guid.0),
            mineable_composition_guid: row.mineable_composition_guid.map(|guid| guid.0),
            mining_global_params_guid: row.mining_global_params_guid.map(|guid| guid.0),
        }
    }
}

impl From<contract_harvest::HarvestableArea<'_>> for HarvestableArea {
    fn from(row: contract_harvest::HarvestableArea<'_>) -> Self {
        Self {
            name: row.name.map(str::to_owned),
            global_modifier: row.global_modifier,
            object_preset_paths: row
                .object_preset_paths
                .into_iter()
                .map(str::to_owned)
                .collect(),
            modifiers: row.modifiers.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<contract_harvest::AreaModifier> for AreaModifier {
    fn from(row: contract_harvest::AreaModifier) -> Self {
        Self {
            group_index: row.group_index,
            harvestable_index: row.harvestable_index,
            modifier: row.modifier,
            geometry_tag_guids: row
                .geometry_tag_guids
                .into_iter()
                .map(|guid| guid.0)
                .collect(),
        }
    }
}

impl From<contract_harvest::HarvestableLocation> for HarvestableLocation {
    fn from(row: contract_harvest::HarvestableLocation) -> Self {
        Self {
            preset_guid: row.preset_guid.0,
            starmap_object_guid: row.starmap_object_guid.map(|guid| guid.0),
            system: row.system,
            container: row.container,
        }
    }
}
