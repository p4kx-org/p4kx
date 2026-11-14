// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataType, Guid, ReferenceOrigin, SourceUseRole, SourceValue, Value, ValueError,
};

use super::health_reference_capture::ReferenceCapture;
use super::health_reference_observation::ReferenceObservation;
use super::health_reference_receipt::{ReferenceDatum, ReferenceUse};
use super::{OriginalValueFact, OriginalValuePart, OriginalValueSite};
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

pub(super) fn read(
    capture: &ReferenceCapture<'_, '_, '_>,
    site: &OriginalValueSite<'_, '_, '_, '_>,
) -> Result<ReferenceObservation, CatalogError> {
    capture.check_site(site)?;
    let fact = site.fact();
    let admitted = fact.admit_reference()?;
    let source_type = admitted
        .origin
        .record_type()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let mut body = capture.body(site)?;
    let selector = caller(&fact, SourceUseRole::Selector);
    body.target_guid = guid(&fact, admitted.origin, admitted.storage, &selector)?;
    body.target_record_index = ReferenceDatum::captured(
        SourceValue::Present(admitted.origin.record_index()),
        vec![
            header(
                admitted.origin,
                RecordHeaderField::RecordIndex,
                SourceUseRole::Selector,
            ),
            selector.clone(),
        ],
    );
    let path = capture
        .texts
        .retain_text(admitted.origin.record_path())?
        .text()
        .clone();
    body.target_record_path = ReferenceDatum::captured(
        SourceValue::Present(path),
        vec![
            header(
                admitted.origin,
                RecordHeaderField::FileName,
                SourceUseRole::Conversion,
            ),
            selector.clone(),
        ],
    );
    let source_type = capture.texts.retain_text(source_type)?.text().clone();
    body.target_type = ReferenceDatum::captured(
        SourceValue::Present(source_type),
        vec![
            header(
                admitted.origin,
                RecordHeaderField::SourceType,
                SourceUseRole::Selector,
            ),
            selector,
        ],
    );
    capture.observation(None, body)
}

pub(super) fn guid_for_fact(
    fact: &OriginalValueFact<'_, '_, '_>,
) -> Result<ReferenceDatum<Guid>, CatalogError> {
    match fact.value() {
        Value::Reference(_) => {
            let admitted = fact.admit_reference()?;
            let selector = caller(fact, SourceUseRole::Selector);
            Ok(guid(fact, admitted.origin, admitted.storage, &selector)?)
        }
        Value::Error(ValueError::DanglingReference { .. }) => {
            super::health_reference_error::dangling_guid(fact)
        }
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}
fn guid(
    fact: &OriginalValueFact<'_, '_, '_>,
    origin: ReferenceOrigin<'_, '_>,
    storage: DataType,
    selector: &ReferenceUse,
) -> Result<ReferenceDatum<Guid>, ExportAccountingError> {
    let inputs = match storage {
        DataType::Reference => vec![caller(fact, SourceUseRole::Payload)],
        DataType::WeakPointer => vec![
            header(origin, RecordHeaderField::Guid, SourceUseRole::Payload),
            selector.clone(),
            caller(fact, SourceUseRole::Conversion),
        ],
        _ => return Err(ExportAccountingError::ConflictingSourceDatum),
    };
    Ok(ReferenceDatum::captured(
        SourceValue::Present(origin.record_guid()),
        inputs,
    ))
}

fn caller(fact: &OriginalValueFact<'_, '_, '_>, role: SourceUseRole) -> ReferenceUse {
    ReferenceUse::captured(fact.location(OriginalValuePart::Value), role)
}

fn header(
    origin: ReferenceOrigin<'_, '_>,
    field: RecordHeaderField,
    role: SourceUseRole,
) -> ReferenceUse {
    ReferenceUse::captured(field.location(origin.record_index()), role)
}
