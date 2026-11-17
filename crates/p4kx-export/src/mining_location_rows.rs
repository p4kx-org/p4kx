// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use crate::catalog_origins::CatalogOrigins;
use crate::catalog_text_contract::{CanonicalText, GameTextRef, TextForms};
use crate::catalog_v3::starmap_placement::PlacementBinding;
use crate::contract_starmap::StarmapObject;
use crate::mining_locations::{
    HarvestableLocation, MiningGameText, MiningRecordSource, MiningStarmapObject, MiningTextForms,
};
use crate::starmap_containers::proof::PlacementField;
use crate::{CatalogError, contract_harvest};

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
