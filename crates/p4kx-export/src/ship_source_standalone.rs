// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, RecordTree, StructValue, Value};

use super::selection::SelectedSource;
use super::{ShipAggregateOperands, aggregate};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::authored_fields::Observed;
use crate::playability_projection::OwnerProof;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) struct ShipInputs<'read, 'source> {
    pub(crate) header: CanonicalRecordHeader<'source>,
    pub(crate) operands: ShipAggregateOperands,
    pub(crate) playability: OwnerProof,
    pub(crate) vehicle: Option<&'read StructValue<'source>>,
}

pub(crate) fn read<'read, 'source>(
    tree: &'read RecordTree<'source>,
    input: &'read OriginalCatalogInput<'_, 'source>,
    admission: &'read RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ShipInputs<'read, 'source>, CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let vehicle = SelectedSource::read(&originals, "Components", crate::ship::VEHICLE_COMPONENT)?;
    let crew = SelectedSource::read(
        &originals,
        "Components",
        "SEntityComponentCrewManifestParams",
    )?;
    let mut notes = Vec::new();
    let modification = aggregate::modification(fields, &vehicle, &mut notes)?;
    let (operands, _) = aggregate::read(fields, &vehicle, modification, &mut notes)?;
    let playability = OwnerProof::capture(
        input,
        admission.record(),
        &originals,
        vehicle.value,
        crew.value,
    )?;
    let vehicle = match vehicle.value {
        Some(Value::Struct(value)) => Some(value),
        _ => None,
    };
    Ok(ShipInputs {
        header: CanonicalRecordHeader::read(tree, input.origins())?,
        operands,
        playability,
        vehicle,
    })
}
