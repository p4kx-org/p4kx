// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Guid, SourceUseRole, Value, ValueError};

use super::health_reference_capture::ReferenceCapture;
use super::health_reference_data::ReferenceData;
use super::health_reference_observation::ReferenceObservation;
use super::health_reference_receipt::ReferenceDatum;
use super::health_reference_recipe::{self, ReferenceCaller, ReferenceRecipe};
use super::{OriginalValueFact, OriginalValueSite};
use crate::{CatalogError, ExportAccountingError};

#[derive(Clone, Copy)]
pub(super) enum Retention {
    Immediate,
    Deferred,
}

pub(super) fn read(
    capture: &ReferenceCapture<'_, '_, '_>,
    site: &OriginalValueSite<'_, '_, '_, '_>,
    retention: Retention,
) -> Result<ReferenceObservation, CatalogError> {
    capture.check_site(site)?;
    let fact = site.fact();
    let admitted = fact.admit_reference()?;
    let target_type = admitted
        .origin
        .record_type()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let base = capture.body(site)?;
    let (field, path) = base.field.into_present_inputs()?;
    let (source_type, type_uses) = base.source_type.into_present_inputs()?;
    let [owner_type] = type_uses
        .try_into()
        .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
    let target_record_path = capture
        .texts
        .retain_text(admitted.origin.record_path())?
        .text()
        .clone();
    let target_type = capture.texts.retain_text(target_type)?.text().clone();
    let data = ReferenceData {
        field,
        source_type,
        target_guid: Some(admitted.origin.record_guid()),
        target_record_index: Some(admitted.origin.record_index()),
        target_record_path: Some(target_record_path),
        target_type: Some(target_type),
        target_location: None,
        source_pointer: None,
    };
    let recipe = ReferenceRecipe {
        path,
        owner_type,
        caller: ReferenceCaller::from_fact(&fact),
        storage: admitted.storage,
    };
    let frame = Arc::clone(&capture.frame);
    Ok(match retention {
        Retention::Immediate => {
            let body = recipe.body(&data)?;
            ReferenceObservation::captured(data, None, frame, body)
        }
        Retention::Deferred => ReferenceObservation::deferred(data, frame, recipe),
    })
}

pub(super) fn guid_for_fact(
    fact: &OriginalValueFact<'_, '_, '_>,
) -> Result<ReferenceDatum<Guid>, CatalogError> {
    match fact.value() {
        Value::Reference(_) => {
            let admitted = fact.admit_reference()?;
            let caller = ReferenceCaller::from_fact(fact);
            let selector = caller.operand(SourceUseRole::Selector);
            Ok(health_reference_recipe::guid_datum(
                admitted.origin.record_guid(),
                admitted.origin.record_index(),
                admitted.storage,
                caller,
                &selector,
            )?)
        }
        Value::Error(ValueError::DanglingReference { .. }) => {
            super::health_reference_error::dangling_guid(fact)
        }
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}

#[cfg(test)]
#[path = "health_reference_recipe_tests.rs"]
mod tests;
