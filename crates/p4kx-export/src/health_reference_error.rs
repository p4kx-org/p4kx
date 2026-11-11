// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Guid, SourceUseRole, SourceValue, Value, ValueError};

use super::health_reference_capture::ReferenceCapture;
use super::health_reference_data::ReferencePointer;
use super::health_reference_link::ReferenceLink;
use super::health_reference_observation::{ReferenceObservation, ReferencePointerBody};
use super::health_reference_receipt::{ReferenceDatum, ReferenceUse};
use super::{OriginalErrorParameter, OriginalValueFact, OriginalValuePart, OriginalValueSite};
use crate::{CatalogError, ExportAccountingError};

pub(super) fn read(
    capture: &ReferenceCapture<'_, '_, '_>,
    site: &OriginalValueSite<'_, '_, '_, '_>,
) -> Result<Option<ReferenceObservation>, CatalogError> {
    capture.check_site(site)?;
    let Value::Error(error) = site.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let Some(link) = ReferenceLink::from_error(*error) else {
        return Ok(None);
    };
    let fact = site.fact();
    let mut body = capture.body(site)?;
    match *error {
        ValueError::DanglingReference { .. } => body.target_guid = dangling_guid(&fact)?,
        ValueError::UnexpectedReferenceType { guid, .. } => {
            // Validation GUIDs have no retained target-header admission.
            body.target_guid = ReferenceDatum::unproved(guid);
        }
        _ => {}
    }
    body.source_pointer = link
        .pointer
        .map(|pointer| pointer_body(&fact, pointer, *error))
        .transpose()?;
    Ok(Some(capture.observation(Some(link.status), body)?))
}

fn pointer_body(
    fact: &OriginalValueFact<'_, '_, '_>,
    pointer: ReferencePointer,
    error: ValueError,
) -> Result<ReferencePointerBody, CatalogError> {
    let structure = match (error, pointer.structure) {
        (ValueError::UnexpectedReferenceType { .. }, Some(value)) => {
            ReferenceDatum::unproved(value)
        }
        (_, value) => parameter(fact, OriginalErrorParameter::StructIndex, value)?,
    };
    Ok(ReferencePointerBody {
        structure,
        instance: parameter(
            fact,
            OriginalErrorParameter::InstanceIndex,
            pointer.instance,
        )?,
        declared_type: declaration(fact, pointer.declared_type)?,
    })
}

fn parameter(
    fact: &OriginalValueFact<'_, '_, '_>,
    parameter: OriginalErrorParameter,
    value: Option<u32>,
) -> Result<ReferenceDatum<u32>, CatalogError> {
    let Some(value) = value else {
        return Ok(ReferenceDatum::absent());
    };
    let (part, role) = fact.pointer_error_input(parameter, value)?;
    Ok(ReferenceDatum::captured(
        SourceValue::Present(value),
        vec![ReferenceUse::captured(fact.location(part), role)],
    ))
}

fn declaration(
    fact: &OriginalValueFact<'_, '_, '_>,
    value: Option<u32>,
) -> Result<ReferenceDatum<u32>, CatalogError> {
    let Some(value) = value else {
        return Ok(ReferenceDatum::absent());
    };
    if u32::from(fact.field().storage().target()) != value {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    Ok(ReferenceDatum::captured(
        SourceValue::Present(value),
        vec![ReferenceUse::captured(
            fact.location(OriginalValuePart::Target),
            SourceUseRole::Selector,
        )],
    ))
}

pub(super) fn dangling_guid(
    fact: &OriginalValueFact<'_, '_, '_>,
) -> Result<ReferenceDatum<Guid>, CatalogError> {
    let Value::Error(ValueError::DanglingReference { guid }) = fact.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    if fact.field().storage().data_type() != DataType::Reference {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    Ok(ReferenceDatum::captured(
        SourceValue::Present(*guid),
        vec![ReferenceUse::captured(
            fact.location(OriginalValuePart::Error(OriginalErrorParameter::Guid)),
            SourceUseRole::Payload,
        )],
    ))
}
