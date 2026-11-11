// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use super::quantum_boost::QuantumBoost;
use super::quantum_fields::QuantumFields;
use super::quantum_heat::QuantumHeat;
use super::quantum_profile::QuantumProfile;
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const SOURCE_TYPE: &str = "SCItemQuantumDriveParams";

/// The stored configuration of one physical quantum drive component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct QuantumDrive<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    fuel_requirement: SourcedValue<ResourceNumber>,
    jump_range: SourcedValue<ResourceNumber>,
    disconnect_range: SourcedValue<ResourceNumber>,
    standard: SourcedValue<QuantumProfile<'source>>,
    spline: SourcedValue<QuantumProfile<'source>>,
    heat: SourcedValue<QuantumHeat<'source>>,
    boost: SourcedValue<QuantumBoost<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumDrive<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            owner,
            source_type: fields.source_type()?,
            fuel_requirement: fields.number("quantumFuelRequirement")?,
            jump_range: fields.number("jumpRange")?,
            disconnect_range: fields.number("disconnectRange")?,
            standard: fields.class("params", QuantumProfile::read)?,
            spline: fields.class("splineJumpParams", QuantumProfile::read)?,
            heat: fields.class("heatParams", QuantumHeat::read)?,
            boost: fields.class("quantumBoostParams", QuantumBoost::read)?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<QuantumDrive<'source>>,
) -> CatalogTable<QuantumDrive<'source>> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, &[SOURCE_TYPE], rows)
}
