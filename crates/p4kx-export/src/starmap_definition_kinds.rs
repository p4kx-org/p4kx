// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;

use super::prepared_definition::PreparedDefinition;
use super::starmap_amenity_definition::{self, StarmapAmenityDefinition};
use super::starmap_preset_definition::{self, StarmapPresetDefinition};
use super::starmap_type_definition::{self, StarmapTypeDefinition};
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, Kind};

/// The canonical dataset of a concrete standalone starmap Record type.
pub(crate) fn kind_for_type(source_type: &str) -> Option<Kind> {
    match source_type {
        starmap_type_definition::SOURCE_TYPE => Some(Kind::StarmapTypes),
        starmap_preset_definition::SOURCE_TYPE => Some(Kind::StarmapPresets),
        starmap_amenity_definition::SOURCE_TYPE => Some(Kind::StarmapAmenities),
        _ => None,
    }
}

pub(crate) enum StarmapDefinitionPick<'source> {
    Type(Box<PreparedDefinition<'source, StarmapTypeDefinition<'source>>>),
    Preset(Box<PreparedDefinition<'source, StarmapPresetDefinition<'source>>>),
    Amenity(Box<PreparedDefinition<'source, StarmapAmenityDefinition<'source>>>),
}

impl<'source> StarmapDefinitionPick<'source> {
    pub(crate) fn read(
        kind: Kind,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<FieldError>), CatalogError> {
        if kind_for_type(tree.type_name()) != Some(kind) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        match kind {
            Kind::StarmapTypes => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                StarmapTypeDefinition::read,
            )
            .map(|(row, notes)| (Self::Type(Box::new(row)), notes)),
            Kind::StarmapPresets => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                StarmapPresetDefinition::read,
            )
            .map(|(row, notes)| (Self::Preset(Box::new(row)), notes)),
            Kind::StarmapAmenities => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                StarmapAmenityDefinition::read,
            )
            .map(|(row, notes)| (Self::Amenity(Box::new(row)), notes)),
            _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }
}

#[derive(Default)]
pub(crate) struct StarmapDefinitions<'source> {
    types: Vec<PreparedDefinition<'source, StarmapTypeDefinition<'source>>>,
    presets: Vec<PreparedDefinition<'source, StarmapPresetDefinition<'source>>>,
    amenities: Vec<PreparedDefinition<'source, StarmapAmenityDefinition<'source>>>,
}

/// Canonical starmap definition tables.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct StarmapDefinitionTables<'source> {
    #[serde(rename = "starmapTypes")]
    types: CatalogTable<StarmapTypeDefinition<'source>>,
    #[serde(rename = "starmapPresets")]
    presets: CatalogTable<StarmapPresetDefinition<'source>>,
    #[serde(rename = "starmapAmenities")]
    amenities: CatalogTable<StarmapAmenityDefinition<'source>>,
}

impl<'source> StarmapDefinitions<'source> {
    pub(crate) fn push(&mut self, pick: StarmapDefinitionPick<'source>) {
        match pick {
            StarmapDefinitionPick::Type(row) => self.types.push(*row),
            StarmapDefinitionPick::Preset(row) => self.presets.push(*row),
            StarmapDefinitionPick::Amenity(row) => self.amenities.push(*row),
        }
    }

    pub(crate) fn order(&mut self) {
        PreparedDefinition::order(&mut self.types);
        PreparedDefinition::order(&mut self.presets);
        PreparedDefinition::order(&mut self.amenities);
    }

    pub(crate) const fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::StarmapTypes => self.types.len(),
            Kind::StarmapPresets => self.presets.len(),
            Kind::StarmapAmenities => self.amenities.len(),
            _ => 0,
        }
    }

    pub(crate) fn sources(
        &self,
    ) -> impl Iterator<Item = (Kind, Guid, &'source str, &[CapturedNumber])> {
        PreparedDefinition::sources(Kind::StarmapTypes, &self.types)
            .chain(PreparedDefinition::sources(
                Kind::StarmapPresets,
                &self.presets,
            ))
            .chain(PreparedDefinition::sources(
                Kind::StarmapAmenities,
                &self.amenities,
            ))
    }

    pub(crate) fn number_groups(&self) -> impl Iterator<Item = (Kind, &[CapturedNumber])> {
        self.sources().map(|(kind, _, _, numbers)| (kind, numbers))
    }

    pub(crate) fn into_tables(self, forge: &DataForge<'_>) -> StarmapDefinitionTables<'source> {
        StarmapDefinitionTables {
            types: PreparedDefinition::table(
                forge,
                starmap_type_definition::SOURCE_TYPE,
                self.types,
            ),
            presets: PreparedDefinition::table(
                forge,
                starmap_preset_definition::SOURCE_TYPE,
                self.presets,
            ),
            amenities: PreparedDefinition::table(
                forge,
                starmap_amenity_definition::SOURCE_TYPE,
                self.amenities,
            ),
        }
    }
}
