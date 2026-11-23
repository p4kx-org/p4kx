// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, StructValue, Value};

use super::super::Observed;
use super::aggregate;
use super::fields;
use super::selection::SelectedSource;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::playability_projection::OwnerProof;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;

/// Original operands of a physical Ship's implementation membership and playability rank.
pub(crate) struct VariantOperands<'read, 'source> {
    pub(crate) vehicle: Option<&'read StructValue<'source>>,
    pub(crate) definition: SourcedValue<&'source str>,
    pub(crate) modification: SourcedValue<bool>,
    pub(crate) playability: OwnerProof,
}

pub(super) fn read_selected<'read, 'source>(
    originals: Observed<'_, '_, 'read, 'source>,
    input: &OriginalCatalogInput<'_, '_>,
    admission: &RecordObservation<'_, 'source>,
    vehicle: &SelectedSource<'read, 'source>,
    crew: &SelectedSource<'read, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<VariantOperands<'read, 'source>, CatalogError> {
    let definition = fields::selected(originals, vehicle, |owner| {
        fields::text(owner, "vehicleDefinition", notes)
    })?;
    let modification = aggregate::modification(originals, vehicle, notes)?;
    let playability = OwnerProof::capture(
        input,
        admission.record(),
        originals.originals,
        vehicle.value,
        crew.value,
    )?;
    let vehicle = match vehicle.value {
        Some(Value::Struct(body)) => Some(body),
        _ => None,
    };
    Ok(VariantOperands {
        vehicle,
        definition,
        modification,
        playability,
    })
}

#[path = "ship_variant_operand_probe.rs"]
pub(super) mod probe;
