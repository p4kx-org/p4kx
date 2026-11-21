// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Guid, SourceUseRole, Value, ValueError};

use super::health_catalog_reference_proof::{CatalogReferenceData, CatalogReferenceProof};
use super::health_reference_capture::ReferenceCapture;

use super::health_reference_receipt::ReferenceDatum;
use super::health_reference_recipe::{self, ReferenceCaller, ReferenceRecipe};
use super::{OriginalValueFact, OriginalValueSite};
use crate::{CatalogError, ExportAccountingError};

pub(super) fn read(
    capture: &ReferenceCapture<'_, '_, '_>,
    site: &OriginalValueSite<'_, '_, '_, '_>,
) -> Result<CatalogReferenceProof, CatalogError> {
    capture.check_site(site)?;
    let fact = site.fact();
    let admitted = fact.admit_reference()?;
    let target_type = admitted
        .origin
        .record_type()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let base = capture.path_receipt(site)?;
    let target_record_path = capture
        .texts
        .retain_text(admitted.origin.record_path())?
        .text()
        .clone();
    let target_type = capture.texts.retain_text(target_type)?.text().clone();
    let data = CatalogReferenceData {
        field: base.field,
        source_type: base.owner.type_name,
        target_guid: admitted.origin.record_guid(),
        target_record_index: admitted.origin.record_index(),
        target_record_path,
        target_type,
    };
    let recipe = ReferenceRecipe {
        path: base.uses,
        owner_type: base.owner.input,
        caller: ReferenceCaller::from_fact(&fact),
        storage: admitted.storage,
    };
    Ok(CatalogReferenceProof::admitted(
        data,
        Arc::clone(&capture.frame),
        recipe,
    ))
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
