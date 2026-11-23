// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, RecordTree, SourcedValue, StructValue, Value};

use super::Observed;
use super::ship_presentation::{self, ShipPresentation};
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::{CapturedNumber, NumberCapture};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::contract_insurance::ShipInsurance;
use crate::playability_projection::OwnerProof;
use crate::record_observation::RecordObservation;
use crate::resource_number::ResourceNumber;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

#[path = "ship_aggregate_source.rs"]
mod aggregate;
#[path = "ship_source_fields.rs"]
pub(super) mod fields;
#[path = "ship_insurance_source.rs"]
mod insurance;
#[path = "ship_playability_inputs.rs"]
mod playability_inputs;
#[path = "ship_source_selection.rs"]
mod selection;
#[path = "ship_source_standalone.rs"]
pub(crate) mod standalone;

pub(crate) use aggregate::ShipAggregateOperands;
pub use playability_inputs::ShipPlayabilityInputs;
use selection::SelectedSource;

#[cfg(test)]
#[path = "ship_source_tests.rs"]
mod tests;

pub(crate) struct ShipSourceRead<'read, 'source> {
    pub(crate) header: CanonicalRecordHeader<'source>,
    pub(crate) vehicle: Option<&'read StructValue<'source>>,
    pub(crate) presentation: ShipPresentation<'source>,
    pub(crate) crew_size: SourcedValue<ResourceNumber>,
    pub(crate) vehicle_definition: SourcedValue<&'source str>,
    pub(crate) insurance: SourcedValue<Box<ShipInsurance<'source>>>,
    pub(crate) playability_inputs: ShipPlayabilityInputs<'source>,
    pub(crate) playability_proof: OwnerProof,
    pub(crate) notes: Vec<FieldError>,
    pub(crate) aggregate: ShipAggregateOperands,
    pub(crate) physics_notes: Vec<FieldError>,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) fn read<'read, 'source>(
    tree: &'read RecordTree<'source>,
    projection: &'read CatalogProjection<'_, 'source>,
    admission: &'read RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ShipSourceRead<'read, 'source>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let vehicle = SelectedSource::read(&originals, "Components", crate::ship::VEHICLE_COMPONENT)?;
    let attachment = SelectedSource::read(&originals, "Components", "SAttachableComponentParams")?;
    let crew = SelectedSource::read(
        &originals,
        "Components",
        "SEntityComponentCrewManifestParams",
    )?;
    let insured = SelectedSource::read(
        &originals,
        "StaticEntityClassData",
        "SEntityInsuranceProperties",
    )?;
    let (presentation, mut notes) = ship_presentation::read(
        fields,
        projection,
        vehicle.value,
        &vehicle.prefix,
        attachment.value,
        &attachment.prefix,
    )?;
    let (aggregate, physics_notes) = aggregate::read(fields, &vehicle, &mut notes)?;
    let crew_size = fields::selected(fields, &vehicle, |owner| {
        owner.authored_number("crewSize", &mut notes)
    })?;
    let vehicle_definition = fields::selected(fields, &vehicle, |owner| {
        fields::text(owner, "vehicleDefinition", &mut notes)
    })?;
    let insurance = insurance::read(fields, &insured, &mut notes)?;
    let playability_inputs = fields::crew(fields, &crew, &mut notes)?;
    let playability_proof = OwnerProof::capture(
        input,
        admission.record(),
        &originals,
        vehicle.value,
        crew.value,
    )?;
    Ok(ShipSourceRead {
        header: CanonicalRecordHeader::read(tree, input.origins())?,
        vehicle: vehicle.value.and_then(|value| match value {
            Value::Struct(value) => Some(value),
            _ => None,
        }),
        presentation,
        crew_size,
        vehicle_definition,
        insurance,
        playability_inputs,
        playability_proof,
        notes,
        aggregate,
        physics_notes,
        numbers: NumberCapture::deduplicate(numbers.into_inner())?,
    })
}

#[cfg(test)]
pub(crate) fn aggregate_for_test(
    tree: &RecordTree<'_>,
    input: &crate::build::original_input::OriginalCatalogInput<'_, '_>,
    admission: &RecordObservation<'_, '_>,
) -> Result<ShipAggregateOperands, CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, None)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let vehicle = SelectedSource::read(&originals, "Components", crate::ship::VEHICLE_COMPONENT)?;
    aggregate::read(fields, &vehicle, &mut Vec::new()).map(|(operands, _)| operands)
}
