// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Guid, RecordTree, SourceUse, SourceUseRole, Value};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::CatalogRecordContext;
use crate::commodity_backfill_input::CommodityInputCapture;
use crate::record_header_field::RecordHeaderField;

use super::evidence::Evidence;

/// One resource's original relationship declarations, including evaluated negative slots.
pub(crate) struct ResourceLinks {
    pub(crate) resource: Guid,
    pub(crate) targets: Vec<Guid>,
    pub(crate) evidence: Evidence,
    pub(crate) guid_source: SourceUse,
}

pub(crate) fn read(
    tree: &RecordTree<'_>,
    origins: &CatalogOrigins,
    capture: &mut CommodityInputCapture,
    cancel: Option<&CancelToken>,
) -> Result<ResourceLinks, CatalogError> {
    let context = origins.record(tree.record_index());
    let mut evidence = Evidence::default();
    let original = tree.field("properties");
    evidence.field(&context, "properties", original)?;
    let mut targets = Vec::new();
    if let Some(Value::Array(values)) = original {
        for (ordinal, value) in values.iter().enumerate() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let path = format!("properties[{ordinal}]");
            if let Some(target) = target(value, &path, &context, &mut evidence)? {
                let field = format!("{path}.qualityQuantization.qualityQuantizationRecord");
                let selectors = [
                    "properties".to_owned(),
                    format!("{path}.$type"),
                    format!("{path}.qualityQuantization"),
                    format!("{path}.qualityQuantization.$type"),
                ];
                capture.observe(tree.guid(), Some(target), &field, &selectors);
                targets.push(target);
            }
        }
    }
    Ok(ResourceLinks {
        resource: tree.guid(),
        targets,
        evidence,
        guid_source: context.observe_header(RecordHeaderField::Guid, SourceUseRole::Payload)?,
    })
}

fn target(
    value: &Value<'_>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    evidence: &mut Evidence,
) -> Result<Option<Guid>, CatalogError> {
    evidence.field(context, path, Some(value))?;
    let Value::Struct(property) = value else {
        return Ok(None);
    };
    evidence.field(context, &format!("{path}.$type"), None)?;
    let path = format!("{path}.qualityQuantization");
    let value = property.field("qualityQuantization");
    evidence.field(context, &path, value)?;
    let Some(Value::Struct(source)) = value else {
        return Ok(None);
    };
    evidence.field(context, &format!("{path}.$type"), None)?;
    let path = format!("{path}.qualityQuantizationRecord");
    let value = source.field("qualityQuantizationRecord");
    evidence.field(context, &path, value)?;
    Ok(match value {
        Some(Value::Reference(reference)) if reference.location().is_none() => {
            Some(reference.guid())
        }
        _ => None,
    })
}
