// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    ShipAttachmentManufacturer, ShipInsurance, ShipInsuranceParams, ShipManufacturerValue,
    ShipPlayabilityInputs, ShipSource, ShipVehicleManufacturer,
};
use p4kx_extraction::SourceInventory;
use serde_json::Value;

use crate::canonical_header::originals;
use crate::canonical_header::wire::Wire;
use crate::catalog_resources::resource_consumer_walk;
use crate::commodity_v3_fixture::TestResult;

pub(super) fn row(
    row: &ShipSource<'_>,
    inventory: &SourceInventory,
    canonical: &Value,
) -> TestResult {
    let wire = Wire::new(canonical, inventory);
    resource_consumer_walk::visit_header(row.source(), wire)?;
    wire.field("name")?.leaf(row.name())?;
    wire.field("description")?.leaf(row.description())?;
    wire.field("role")?.leaf(row.role())?;
    wire.field("career")?.leaf(row.career())?;
    wire.field("crewSize")?
        .cell(row.crew_size(), |value, wire| wire.number(value))?;
    wire.field("vehicleDefinition")?
        .leaf(row.vehicle_definition())?;
    wire.field("size")?
        .cell(row.size(), |value, wire| wire.number(value))?;
    wire.field("mass")?.leaf(row.mass())?;
    wire.field("massBasis")?.leaf(row.mass_basis())?;
    wire.field("hullHealth")?.leaf(row.hull_health())?;
    vehicle_manufacturer(
        row.vehicle_manufacturer(),
        wire.field("vehicleManufacturer")?,
    )?;
    attachment_manufacturer(
        row.attachment_manufacturer(),
        wire.field("attachmentManufacturer")?,
    )?;
    wire.field("insurance")?
        .cell(row.insurance(), |value, wire| insurance(value, wire))?;
    playability_inputs(row.playability_inputs(), wire.field("playabilityInputs")?)
}

fn vehicle_manufacturer(row: &ShipVehicleManufacturer<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("componentOrdinal")?
        .leaf(row.component_ordinal())?;
    wire.field("manufacturer")?
        .cell(row.manufacturer(), manufacturer)
}

fn attachment_manufacturer(row: &ShipAttachmentManufacturer<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("componentOrdinal")?
        .leaf(row.component_ordinal())?;
    wire.field("attachDefPresent")?
        .leaf(row.attach_def_present())?;
    wire.field("manufacturer")?
        .cell(row.manufacturer(), manufacturer)
}

fn manufacturer(value: &ShipManufacturerValue<'_>, wire: Wire<'_>) -> TestResult {
    match value {
        ShipManufacturerValue::Record(reference) => originals::visit_record(reference, wire),
        ShipManufacturerValue::Authored { authored } => {
            originals::visit_arena(authored, wire.field("authored")?)
        }
        _ => Err("a new manufacturer storage needs a typed Ship traversal".into()),
    }
}

fn insurance(row: &ShipInsurance<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("staticElementOrdinal")?
        .leaf(row.static_element_ordinal())?;
    wire.field("restoreOnClaim")?.leaf(row.restore_on_claim())?;
    wire.field("shipInsuranceParams")?
        .cell(row.params(), insurance_params)?;
    originals::visit_arena(row.additional_fields(), wire.field("additionalFields")?)
}

fn insurance_params(row: &ShipInsuranceParams<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("shipEntityClassName")?
        .leaf(row.ship_entity_class_name())?;
    wire.field("baseWaitTimeMinutes")?
        .cell(row.base_wait_time_minutes(), |value, wire| {
            wire.number(value)
        })?;
    wire.field("mandatoryWaitTimeMinutes")?
        .cell(row.mandatory_wait_time_minutes(), |value, wire| {
            wire.number(value)
        })?;
    wire.field("expediteFee")?
        .cell(row.expedite_fee(), |value, wire| wire.number(value))?;
    originals::visit_arena(row.additional_fields(), wire.field("additionalFields")?)
}

fn playability_inputs(row: &ShipPlayabilityInputs<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("crewManifestComponentOrdinal")?
        .leaf(row.crew_manifest_component_ordinal())?;
    wire.field("crewManifest")?
        .cell(row.crew_manifest(), |value, wire| {
            originals::visit_record(value, wire)
        })
}
