// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Location, Reference, ReferenceOrigin, SourceEvidenceError, SourceLocation, SourceUse,
    SourceUseRole, SourceValue, Step,
};

use super::{OriginalDatum, OriginalInstanceLocation, OriginalTargetShape, OriginalValueFact};
use crate::{CatalogError, ExportAccountingError};

pub(super) fn read<'source>(
    site: &OriginalValueFact<'_, '_, 'source>,
    reference: &Reference<'source>,
    admitted: ReferenceOrigin<'_, 'source>,
    location: &Location<'source>,
    selector: &SourceUse,
) -> Result<OriginalTargetShape<'source>, CatalogError> {
    if admitted.record_index() != site.scope().record().index() {
        return Err(SourceEvidenceError::ForeignEvidence.into());
    }
    let positions = admitted
        .instance_positions()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    if positions.len() != location.steps().len() || positions.is_empty() {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let scope = site.scope().dataforge();
    let at = tuple(
        admitted.record_index(),
        positions,
        format!("{}.$type", location.as_str()),
    );
    let input = scope.at(at, SourceUseRole::Selector)?;
    let source_type = OriginalDatum::observed(
        SourceValue::Present(reference.type_name()),
        vec![input, selector.clone()],
        true,
    );
    let retained = admitted
        .shared_instance_location()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    if !std::ptr::eq(retained.as_ref(), location) {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let location = location_fact(site, admitted.record_index(), positions, location, selector)?
        .map(|_| OriginalInstanceLocation::new(retained))?;
    Ok(OriginalTargetShape::Instance {
        source_type,
        location,
    })
}

fn location_fact(
    site: &OriginalValueFact<'_, '_, '_>,
    record_index: u32,
    positions: &[u64],
    location: &Location<'_>,
    selector: &SourceUse,
) -> Result<OriginalDatum<String>, CatalogError> {
    let mut field = String::new();
    let mut inputs = vec![selector.clone()];
    for (index, step) in location.steps().iter().enumerate() {
        let part = next_part(&mut field, *step, positions.get(index).copied())?;
        let prefix = positions
            .get(..=index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        inputs.push(
            site.scope()
                .dataforge()
                .at(tuple(record_index, prefix, part), SourceUseRole::Conversion)?,
        );
    }
    if field != location.as_str() {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    Ok(OriginalDatum::observed(
        SourceValue::Present(field),
        inputs,
        true,
    ))
}

fn next_part(
    field: &mut String,
    step: Step<'_>,
    position: Option<u64>,
) -> Result<String, ExportAccountingError> {
    use std::fmt::Write as _;

    Ok(match step {
        Step::Field(name) => {
            if !field.is_empty() {
                field.push('.');
            }
            field.push_str(name);
            format!("{field}.$fieldName")
        }
        Step::Element(ordinal) => {
            if position != Some(u64::from(ordinal)) {
                return Err(ExportAccountingError::ConflictingSourceDatum);
            }
            write!(field, "[{ordinal}]").map_err(ExportAccountingError::FieldPath)?;
            field.clone()
        }
    })
}

fn tuple(record_index: u32, positions: &[u64], field: String) -> SourceLocation {
    let mut indices = Vec::with_capacity(positions.len() + 1);
    indices.push(u64::from(record_index));
    indices.extend_from_slice(positions);
    SourceLocation::Subresource {
        namespace: "dataforgeTraversal".to_owned(),
        indices,
        field,
    }
}
