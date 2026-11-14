// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use super::quantum_fields::QuantumFields;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference, SourceWeakReference};

#[path = "jump_drive_effects.rs"]
mod effects;

use effects::JumpDriveEffects;

pub(super) const SOURCE_TYPE: &str = "SCItemJumpDriveParams";
pub(super) const FLIGHT_PARAMS: &str = "JumpDriveFlightParams";
pub(super) const TUNNEL_FORCES: &str = "JumpTunnelForcesParams";
const DEVICE_STATE: &str = "ItemResourceState";

/// Stored settings of one item-side jump drive component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct JumpDrive<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    alignment_decay_rate: SourcedValue<ResourceNumber>,
    alignment_rate: SourcedValue<ResourceNumber>,
    fuel_usage_efficiency_multiplier: SourcedValue<ResourceNumber>,
    tuning_decay_rate: SourcedValue<ResourceNumber>,
    tuning_rate: SourcedValue<ResourceNumber>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    flight_tuning: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    tunnel_forces: SourcedValue<Box<SourceRecordReference<'source>>>,
    idle_state: SourcedValue<Box<SourceWeakReference<'source>>>,
    transiting_state: SourcedValue<Box<SourceWeakReference<'source>>>,
    effects: SourcedValue<JumpDriveEffects<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> JumpDrive<'source> {
    pub(super) fn linked_definitions(&self) -> impl Iterator<Item = u32> + '_ {
        [&self.flight_tuning, &self.tunnel_forces]
            .into_iter()
            .filter_map(|reference| {
                let record = reference.value()?;
                if matches!(
                    record.status,
                    crate::definition_reference::DefinitionStatus::Present
                ) {
                    record.target_record_index.value().copied()
                } else {
                    None
                }
            })
    }

    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            owner,
            source_type: fields.source_type()?,
            alignment_decay_rate: fields.number("alignmentDecayRate")?,
            alignment_rate: fields.number("alignmentRate")?,
            fuel_usage_efficiency_multiplier: fields.number("fuelUsageEfficiencyMultiplier")?,
            tuning_decay_rate: fields.number("tuningDecayRate")?,
            tuning_rate: fields.number("tuningRate")?,
            flight_tuning: fields.record_link_to("flightTuning", FLIGHT_PARAMS)?,
            tunnel_forces: fields.record_link_to("tunnelForces", TUNNEL_FORCES)?,
            idle_state: fields.weak_link_to("idleState", DEVICE_STATE)?,
            transiting_state: fields.weak_link_to("transitingState", DEVICE_STATE)?,
            effects: fields.class("jumpDriveEffectParams", JumpDriveEffects::read)?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<JumpDrive<'source>>,
) -> CatalogTable<JumpDrive<'source>> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, &[SOURCE_TYPE], rows)
}
