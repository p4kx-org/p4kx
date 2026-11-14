// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::catalog_unit_source::UnitSource;
use super::super::interaction_source;
use super::super::prepared_definition::PreparedDefinition;
use super::med_bed_resource_consumption::{self, MedBedResourceConsumption};
use super::medical_tier::{self, MedBedTier, MedicalList};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) const SOURCE_TYPE: &str = "MedicalItemTierConfig";
const DRUG_EFFICACY: &str = "DrugEfficacyForItemType";
const DRUG_TYPE: &str = "DrugTypeToApply";

/// One physical tier and consumption Record shared by medical facilities.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MedicalItemTierConfig<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    med_bed_configs: MedicalList<MedBedTier<'source>>,
    drug_efficacy_config: MedicalList<UnitSource<'source>>,
    available_drug_types: MedicalList<UnitSource<'source>>,
    resource_consumption: SourcedValue<MedBedResourceConsumption<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

pub(crate) type PreparedMedicalTier<'source> =
    PreparedDefinition<'source, MedicalItemTierConfig<'source>>;

impl<'source> MedicalItemTierConfig<'source> {
    fn read(
        header: CanonicalRecordHeader<'source>,
        source: Observed<'_, '_, '_, 'source>,
        _projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let med_bed_configs = source.class_array_to(
            "medBedConfigs",
            medical_tier::SOURCE_TYPE,
            notes,
            MedBedTier::read,
        )?;
        let drug_efficacy_config = source.class_array_to(
            "drugEfficacyConfig",
            DRUG_EFFICACY,
            notes,
            interaction_source::original,
        )?;
        let available_drug_types = source.class_array_to(
            "availableDrugTypes",
            DRUG_TYPE,
            notes,
            interaction_source::original,
        )?;
        let resource_consumption = source.declared_class_to(
            "resourceConsumption",
            med_bed_resource_consumption::SOURCE_TYPE,
            notes,
            MedBedResourceConsumption::read,
        )?;
        let additional_fields = source.authored_members_except([
            ("medBedConfigs", med_bed_configs.state()),
            ("drugEfficacyConfig", drug_efficacy_config.state()),
            ("availableDrugTypes", available_drug_types.state()),
            ("resourceConsumption", resource_consumption.state()),
        ])?;
        Ok(Self {
            source: header,
            med_bed_configs,
            drug_efficacy_config,
            available_drug_types,
            resource_consumption,
            additional_fields,
        })
    }
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedMedicalTier<'source>, Vec<FieldError>), CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        MedicalItemTierConfig::read,
    )
}

pub(crate) fn order(rows: &mut [PreparedMedicalTier<'_>]) {
    PreparedDefinition::order(rows);
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedMedicalTier<'source>>,
) -> CatalogTable<MedicalItemTierConfig<'source>> {
    PreparedDefinition::table(forge, SOURCE_TYPE, rows)
}

pub(crate) fn sources<'rows, 'source>(
    rows: &'rows [PreparedMedicalTier<'source>],
) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
    PreparedDefinition::sources(Kind::MedicalItemTierConfigs, rows)
}
