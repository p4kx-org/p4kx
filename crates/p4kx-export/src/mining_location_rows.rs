// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::{CancelToken, Record, SourcedValue, TreeBuilder};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_query_session::checkpoint;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::{CanonicalText, GameTextRef, TextForms};
use crate::catalog_v3::starmap_object_definition;
use crate::catalog_v3::starmap_placement::PlacementBinding;
use crate::contract_starmap::StarmapObject;
use crate::mining_locations::{
    HarvestableLocation, MiningGameText, MiningLocationCoverage, MiningRecordSource,
    MiningStarmapObject, MiningTextForms,
};
use crate::record_observation::RecordObservation;
use crate::starmap::{self, TypeNames};
use crate::starmap_containers::Placements;
use crate::starmap_containers::proof::PlacementField;
use crate::{CatalogError, Kind, contract_harvest};

pub(super) struct PlacedObject<'source> {
    source: MiningRecordSource,
    parent: SourcedValue<CatalogRecordReference>,
    pub(super) object: StarmapObject<'source>,
}

pub(super) fn place_starmap<'source>(
    input: &'source OriginalCatalogInput<'source, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    records: Vec<Record<'source>>,
    placements: &Placements,
    cancel: Option<&CancelToken>,
    coverage: &mut MiningLocationCoverage,
) -> Result<Vec<PlacedObject<'source>>, CatalogError> {
    let forge = input.forge();
    let mut trees = TreeBuilder::new();
    let types = TypeNames::default();
    let projection = projection.for_kind(Kind::StarmapObjects);
    let mut objects = Vec::with_capacity(records.len());
    for record in records {
        checkpoint(cancel)?;
        let Ok(tree) = trees.build(&record) else {
            coverage.unreadable_record_bodies += 1;
            continue;
        };
        let admission = RecordObservation::capture(input, &record)?;
        let (authored, notes) =
            starmap_object_definition::read_navigation(&tree, &projection, &admission, cancel)?;
        coverage.unreadable_text_fields += authored.unreadable_name_fields;
        coverage.unreadable_starmap_relationships += notes.len() - authored.unreadable_name_fields;
        let object =
            starmap::starmap_object(&mut trees, &tree, authored.name, forge, &types, placements);
        objects.push(PlacedObject {
            source: authored.source.into(),
            parent: authored.parent,
            object,
        });
    }
    objects.sort_by_cached_key(|row| row.source.order_key());
    Ok(objects)
}

impl PlacedObject<'_> {
    pub(super) fn into_sdk(
        self,
        origins: &CatalogOrigins,
    ) -> Result<MiningStarmapObject, CatalogError> {
        let row = self.object;
        let proof = PlacementBinding::new(&origins.uses, row.placement_proof.as_ref())?;
        Ok(MiningStarmapObject {
            source: self.source,
            name: row.name,
            type_name: row.r#type.map(str::to_owned),
            system: proof.cell(row.system, PlacementField::System)?,
            entity_name: proof.cell(row.entity_name, PlacementField::Entity)?,
            parent: self.parent,
            placed_under_guid: proof
                .cell(row.placed_under_guid, PlacementField::Parent)?
                .map(|guid| guid.0),
            position: proof.cell(row.position, PlacementField::Position)?,
            mining_claim: row.mining_claim,
        })
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

#[cfg(test)]
#[path = "mining_starmap_parent_tests.rs"]
mod tests;
