// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{CancelToken, Guid, RecordTree, SourceUse, SourceUseRole, Value};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::CatalogRecordContext;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::record_observation::RecordObservation;

use super::declaration_survey::Surveys;
use super::evidence::Evidence;

/// One original Resource's complete ordered probes and positive per-target receipts.
pub(crate) struct ResourceLinks {
    pub(crate) resource: Guid,
    pub(crate) targets: BTreeMap<Guid, Evidence>,
    pub(crate) inventory: Evidence,
    pub(crate) admission: Evidence,
    pub(crate) guid_source: SourceUse,
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, '_>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ResourceLinks, CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let context = input.record_context(scope.record())?;
    let mut admission = Evidence::default();
    for field in [
        RecordHeaderInput::NormalizedPath,
        RecordHeaderInput::Field(RecordHeaderField::SourceType),
        RecordHeaderInput::Field(RecordHeaderField::RecordName),
    ] {
        admission.header(&context, field, true)?;
    }
    let root = scope.tree().root();
    let mut surveys = Surveys::new(&scope, cancel);
    let properties = admission.lookup(&context, root, "properties", "properties", || {
        surveys.missing(root)
    })?;
    let mut inventory = admission.copy()?;
    let mut targets = BTreeMap::<Guid, Evidence>::new();
    let mut observed = Evidence::default();
    if let Some(Value::Array(values)) = properties {
        for (ordinal, value) in values.iter().enumerate() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            observed.clear();
            let path = format!("properties[{ordinal}]");
            let found = target(value, &path, &context, &mut observed, &mut surveys)?;
            inventory.append(&observed)?;
            if let Some(guid) = found {
                targets.entry(guid).or_default().append(&observed)?;
            }
        }
    }
    Ok(ResourceLinks {
        resource: scope.tree().guid(),
        targets,
        inventory,
        admission,
        guid_source: context.observe_header(RecordHeaderField::Guid, SourceUseRole::Payload)?,
    })
}

fn target<'source>(
    value: &Value<'source>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    evidence: &mut Evidence,
    surveys: &mut Surveys<'_, '_, 'source>,
) -> Result<Option<Guid>, CatalogError> {
    evidence.field(context, path, value)?;
    let Value::Struct(property) = value else {
        return Ok(None);
    };
    evidence.concrete_type(context, path, property)?;
    let path = format!("{path}.qualityQuantization");
    let value = evidence.lookup(context, property, "qualityQuantization", &path, || {
        surveys.missing(property)
    })?;
    let Some(Value::Struct(source)) = value else {
        return Ok(None);
    };
    evidence.concrete_type(context, &path, source)?;
    let path = format!("{path}.qualityQuantizationRecord");
    let value = evidence.lookup(context, source, "qualityQuantizationRecord", &path, || {
        surveys.missing(source)
    })?;
    Ok(value
        .and_then(crate::record_fields::record_reference)
        .map(|guid| guid.0))
}
