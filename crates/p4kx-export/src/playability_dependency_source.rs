// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, FieldOrigin, Guid, Record, RecordTree, SourceLocation, SourceUse, SourceUseRole,
    Value,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::record_header_field::RecordHeaderField;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

use super::{Evidence, MANIFEST_TYPE, TAG_TYPE};

pub(super) fn observe<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    tree: Option<&RecordTree<'source>>,
    cancel: Option<&CancelToken>,
) -> Result<FieldOrigin, CatalogError> {
    let sources = &input.origins().uses;
    let Some(tree) = tree else {
        let attempted = input.origins().dataforge.at(
            crate::playability_projection::attempted_body(admission.record().index()),
            SourceUseRole::Attempted,
        )?;
        return Ok(sources.origin(&[attempted], false));
    };
    let scope = input.record_scope(admission.record(), tree)?;
    let fields = OriginalFlowFields::read(&scope, cancel)?;
    let name = if tree.type_name() == MANIFEST_TYPE {
        "crew"
    } else {
        "tagName"
    };
    let Some((_, fact)) = fields.field(None, name)? else {
        return fields.absent(None);
    };
    let recognized = match fact.value() {
        Value::Null => true,
        Value::Array(_) => name == "crew",
        Value::String(_) | Value::Enum(_) => name == "tagName",
        _ => false,
    };
    let role = if recognized {
        SourceUseRole::Selector
    } else {
        SourceUseRole::Attempted
    };
    let source = fact.observe(OriginalValuePart::Value, role)?;
    Ok(sources.origin(&[source], recognized))
}

impl Evidence {
    /// Binds the exhausted original GUID lookup and the physical outcomes of its reached aliases.
    pub(crate) fn provenance(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        targets: &[Guid],
    ) -> Result<FieldOrigin, CatalogError> {
        let sources = &input.origins().uses;
        if targets.is_empty() {
            return Ok(sources.origin(&[], true));
        }
        let forge = input.forge();
        let span = forge
            .record_table_span()
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        let table = input.origins().dataforge.at(
            SourceLocation::Subresource {
                namespace: "dataforgeRecordTable".to_owned(),
                indices: vec![span.start, span.end],
                field: "definitions".to_owned(),
            },
            SourceUseRole::Selector,
        )?;
        let mut uses = vec![table];
        let mut complete = true;
        let mut origins = Vec::new();
        for target in targets {
            for record in forge.records_by_guid(*target) {
                let source_type = candidate(input, &record, &mut uses)?;
                match source_type {
                    Some(MANIFEST_TYPE | TAG_TYPE) => {
                        origins.push(
                            self.proofs
                                .get(&record.index())
                                .ok_or(ExportAccountingError::MissingFieldInspection)?,
                        );
                    }
                    Some(_) => {}
                    None => complete = false,
                }
            }
        }
        let selection = sources.origin(&uses, complete);
        origins.push(&selection);
        Ok(sources.origin_with_origins(origins, true)?)
    }
}

fn candidate<'source>(
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'source>,
    uses: &mut Vec<SourceUse>,
) -> Result<Option<&'source str>, CatalogError> {
    let context = input.record_context(record)?;
    for field in [RecordHeaderField::RecordIndex, RecordHeaderField::Guid] {
        uses.push(context.observe_header(field, SourceUseRole::Selector)?);
    }
    let source_type = record.type_name().ok();
    uses.push(context.observe_header(
        RecordHeaderField::SourceType,
        if source_type.is_some() {
            SourceUseRole::Selector
        } else {
            SourceUseRole::Attempted
        },
    )?);
    Ok(source_type)
}
